use std::process::ExitCode;

use crate::cmd::Config;
use crate::kwin::{KWin, KWinClient};
use crate::proc;
use crate::service::Service;

// Exit code used when the KWin script reports a failure ("ERROR <message>"),
// mirroring "command not found" semantics for scripts.
const SCRIPT_ERROR: u8 = 127;

// Exit code used when the KWin script never replies within TIMEOUT.
const TIMED_OUT: u8 = 255;

/// Drives one focus-or-start request end to end: serialise the config into the
/// wire format, wake the KWin script, wait for its reply and translate that into
/// a process exit code (launching the program when nothing matched).
pub fn run(config: &Config) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let kwin = KWinClient::new()?;

    // With neither a program nor any search criteria there is nothing to focus,
    // so the round-trip would be a no-op: stop before bothering the script.
    if config.program().is_none() && config.search().next().is_none() {
        return Ok(ExitCode::SUCCESS);
    }

    // Everything below drives the bundled KWin script, which must be loaded.
    if !kwin.is_script_loaded()? {
        return Err("The KWinTool KWin script is not loaded".into());
    }

    // Serialise the search criteria and actions into the wire format the script
    // parses (search && search && action;action), validating any geometry.
    let command = config.command()?;
    config.vprintln(format!("fetchNextAction({command})"));

    // Own the service name first so it exists when the script calls back, wake
    // the script via its shortcut, then process the fetchNextAction/sendReply
    // round-trip until sendReply reports the outcome.
    let service = Service::register(command)?;
    kwin.invoke_shortcut()?;
    let reply = service.serve()?;

    match reply.as_deref() {
        // "OK <window-id>": success. Surface the id on stdout only when the
        // caller asked for it with --id.
        Some(reply) if reply.starts_with("OK") => {
            if config.id() {
                let id = reply[3..].trim();
                println!("{id}");
            }
            Ok(ExitCode::SUCCESS)
        }
        // "NotFound": no open window matched. When a program was provided this
        // is the "or start it" half of focus-or-start, so launch it detached.
        // The script is the sole authority on whether a window exists, so we
        // never guess from the process list: browser PWAs share one long-lived
        // process whose presence says nothing about whether the app is open.
        Some("NotFound") => {
            if let Some(program) = config.program() {
                config.vprintln(format!("no window matched, starting {program}"));
                proc::launch(program, config.args())?;
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::FAILURE)
            }
        }
        // "ERROR <message>": the script itself failed; relay the message.
        Some(reply) if reply.starts_with("ERROR") => {
            let message = reply["ERROR".len()..].trim_start_matches([':', ' ']);
            eprintln!("=> ERROR: {message}");
            Ok(ExitCode::from(SCRIPT_ERROR))
        }
        // No reply arrived before TIMEOUT elapsed.
        None => {
            eprintln!("=> ERROR: timeout");
            Ok(ExitCode::from(TIMED_OUT))
        }
        Some(other) => Err(other.into()),
    }
}

/// Handles `--update-config`: asks the KWin script to re-fetch its target list
/// from the background service by triggering the reconfigure shortcut. The list
/// itself is served (fresh) by the activated service's `GetTargets`, so there is
/// nothing to write here yet.
pub fn update_config() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let kwin = KWinClient::new()?;
    if !kwin.is_script_loaded()? {
        return Err("The KWinTool KWin script is not loaded".into());
    }
    kwin.invoke_reconfigure()?;
    Ok(ExitCode::SUCCESS)
}

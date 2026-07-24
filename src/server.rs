use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::{Crossroads, MethodErr};

use crate::config;

// DBus identity of the systemd/D-Bus-activated background service, kept in sync
// with the SERVER_* constants at the top of kwin/contents/code/main.js and with
// the `Name=`/`BusName=` in dbus/ and systemd/. The KWin script owns no name of
// its own, so it always calls *into* this service.
const BUS_NAME: &str = "uk.tvidal.server";
const OBJECT_PATH: &str = "/KWinTool";
const INTERFACE: &str = "uk.tvidal.server";

const POLL_INTERVAL: Duration = Duration::from_millis(500);

// Stay resident briefly between calls so a burst of window events (or repeated
// invocations while cycling through matching windows) reuse a single activation
// instead of paying the spawn cost each time. Once idle for this long the
// process exits, keeping it genuinely short-lived rather than a resident daemon.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Runs the D-Bus-activated background service. Owns `uk.tvidal.server`, answers
/// the KWin script's `GetTargets`/`WindowAction` queries, and exits cleanly once
/// it has been idle for [`IDLE_TIMEOUT`]. Each query is answered from a fresh
/// read of the configuration, so the service holds no state between activations
/// and a `--update-config` needs nothing invalidated here.
pub fn serve() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let connection = Connection::new_session()?;
    match connection.request_name(BUS_NAME, false, false, true)? {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
        reply => return Err(format!("could not acquire {BUS_NAME}: {reply:?}").into()),
    }
    eprintln!("KWinTool: service activated on {BUS_NAME}");

    // Every handled call bumps this instant; the serve loop exits once the gap
    // since the last call exceeds IDLE_TIMEOUT.
    let last_activity = Arc::new(Mutex::new(Instant::now()));

    let mut crossroads = Crossroads::new();
    let interface = crossroads.register(INTERFACE, |builder| {
        register_methods(builder, &last_activity);
    });
    crossroads.insert(OBJECT_PATH, &[interface], ());

    connection.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |msg, conn| crossroads.handle_message(msg, conn).is_ok()),
    );

    loop {
        connection.process(POLL_INTERVAL)?;
        if last_activity.lock().expect("activity mutex poisoned").elapsed() >= IDLE_TIMEOUT {
            break;
        }
    }
    connection.release_name(BUS_NAME)?;
    Ok(ExitCode::SUCCESS)
}

/// Registers the two service methods, each of which records activity so the
/// idle-exit clock resets on every call.
fn register_methods(
    builder: &mut dbus_crossroads::IfaceBuilder<()>,
    last_activity: &Arc<Mutex<Instant>>,
) {
    let touch = |slot: &Arc<Mutex<Instant>>| {
        *slot.lock().expect("activity mutex poisoned") = Instant::now();
    };

    // GetTargets() -> targets: the newline-separated search expressions the KWin
    // script watches `windowAdded` against, one per configured rule, in the wire
    // format the script already parses (e.g. "class=alacritty").
    let get_targets_activity = Arc::clone(last_activity);
    builder.method(
        "GetTargets",
        (),
        ("targets",),
        move |_, _, _: ()| -> Result<(String,), MethodErr> {
            touch(&get_targets_activity);
            match config::load().and_then(|config| config.targets()) {
                Ok(targets) => Ok((targets,)),
                Err(err) => {
                    eprintln!("KWinTool: GetTargets failed: {err}");
                    Err(MethodErr::failed(&format!("GetTargets: {err}")))
                }
            }
        },
    );

    // WindowAction(window) -> action: given a "caption:class" for a window the
    // script has matched, return the merged action list (no search filter) that
    // the script applies to that window, or an empty string when nothing matches.
    let window_action_activity = Arc::clone(last_activity);
    builder.method(
        "WindowAction",
        ("window",),
        ("action",),
        move |_, _, (window,): (String,)| -> Result<(String,), MethodErr> {
            touch(&window_action_activity);
            match config::load().and_then(|config| config.action_for(&window)) {
                Ok(action) => Ok((action,)),
                Err(err) => {
                    eprintln!("KWinTool: WindowAction({window:?}) failed: {err}");
                    Err(MethodErr::failed(&format!("WindowAction: {err}")))
                }
            }
        },
    );
}

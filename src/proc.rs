use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::Command;

use sysinfo::{ProcessesToUpdate, System};
use which::which;

/// Reports whether a process other than this one is already running with the
/// given executable name, matching against the process name the way `pgrep`
/// does by default.
pub fn is_running(program: &str) -> bool {
    let name = Path::new(program)
        .file_name()
        .unwrap_or_else(|| OsStr::new(program));

    let current = sysinfo::get_current_pid().ok();

    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .processes()
        .values()
        .filter(|process| Some(process.pid()) != current)
        .any(|process| process.name() == name)
}

/// Resolves `program` against the PATH and starts it detached in its own
/// session with `setsid`, so the child outlives this tool. Returns an error if
/// the executable cannot be found on the PATH or the process cannot be spawned.
pub fn launch(program: &str, args: &[String]) -> io::Result<()> {
    let exe = which(program)
        .map_err(|err| io::Error::new(io::ErrorKind::NotFound, format!("{program}: {err}")))?;
    Command::new("setsid")
        .arg(exe)
        .args(args)
        .spawn()?;
    Ok(())
}

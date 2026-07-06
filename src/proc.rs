use std::io;
use std::process::Command;

use which::which;

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

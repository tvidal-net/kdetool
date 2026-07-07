use std::process::ExitCode;

use clap::Parser;

use kwintool::cmd::Config;

fn main() -> ExitCode {
    let config = Config::parse();
    kwintool::run(&config).unwrap_or_else(|error| {
        eprintln!("=> ERROR: {error}");
        ExitCode::FAILURE
    })
}

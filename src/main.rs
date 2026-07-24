use std::process::ExitCode;

use clap::Parser;

use kwintool::cmd::Config;

fn main() -> ExitCode {
    let config = Config::parse();
    let result = if config.service() {
        kwintool::server::serve()
    } else if config.update_config() {
        kwintool::update_config()
    } else {
        kwintool::run(&config)
    };
    result.unwrap_or_else(|error| {
        eprintln!("=> ERROR: {error}");
        ExitCode::FAILURE
    })
}

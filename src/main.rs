use clap::Parser;
use client::KWinClient;
use cmd::Config;
use std::process::ExitCode;

mod client;
mod cmd;

fn main() -> ExitCode {
    let _args = Config::parse();

    let client = match KWinClient::new() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("failed to connect to the session bus: {error}");
            return ExitCode::FAILURE;
        }
    };

    match client.active_output_name() {
        Ok(name) => {
            println!("{name}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("activeOutputName failed: {error}");
            ExitCode::FAILURE
        }
    }
}

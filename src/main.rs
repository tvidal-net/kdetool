use clap::Parser;
use cmd::Config;

mod cmd;

fn main() {
    let args = Config::parse();
    println!("Config: {:?}", args);
}

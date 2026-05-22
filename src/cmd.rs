use clap::Parser;
use regex::Regex;

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Config {
    /// executable program name
    program: Option<String>,

    /// command line arguments
    args: Vec<String>,

    /// set the target window geometry
    #[arg(short, long)]
    geometry: Option<String>,

    /// match the window resource class name
    #[arg(short, long, value_name = "CLASS REGEX")]
    class: Option<Regex>,

    /// match the window title
    #[arg(short, long, value_name = "TITLE REGEX")]
    title: Option<Regex>,

    /// match the screen name
    #[arg(short, long, value_name = "SCREEN REGEX")]
    screen: Option<Regex>,

    /// match the target screen name
    #[arg(long, value_name = "SCREEN REGEX")]
    to_screen: Option<Regex>,

    /// desktop index
    #[arg(short, long, value_name = "INDEX")]
    desktop: Option<i8>,

    /// target desktop index
    #[arg(long, value_name = "INDEX")]
    to_desktop: Option<i8>,

    /// print diagnostic messages to standard error
    #[arg(short, long)]
    verbose: bool,

    /// list available desktops
    #[arg(long)]
    list_desktops: bool,

    /// list available screens
    #[arg(long)]
    list_screens: bool,
}

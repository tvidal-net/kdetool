use crate::{Action, Search};
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

impl Config {
    pub fn search(&self) -> impl Iterator<Item = Search> {
        let mut search = Vec::new();
        if let Some(class) = &self.class {
            search.push(Search::ClassName(class.clone()));
        }
        if let Some(title) = &self.title {
            search.push(Search::Title(title.clone()));
        }
        if let Some(screen) = &self.screen {
            search.push(Search::Screen(screen.clone()));
        }
        if let Some(desktop) = &self.desktop {
            search.push(Search::Desktop(desktop.clone()));
        }
        search.into_iter()
    }

    pub fn action(&self) -> impl Iterator<Item = Action> {
        let mut action = Vec::new();
        if let Some(to_desktop) = &self.to_desktop {
            action.push(Action::ToDesktop(to_desktop.clone()));
        }
        if let Some(to_screen) = &self.to_screen {
            action.push(Action::ToScreen(to_screen.clone()));
        }
        action.into_iter()
    }
}

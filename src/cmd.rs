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
    // TODO: applied once window manipulation is wired through the KWin script.
    #[allow(dead_code)]
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
    // TODO: implemented once desktop queries are routed through the script.
    #[allow(dead_code)]
    #[arg(long)]
    list_desktops: bool,

    /// list available screens
    // TODO: implemented once screen queries are routed through the script.
    #[allow(dead_code)]
    #[arg(long)]
    list_screens: bool,
}

impl Config {
    /// Name of the executable to focus or launch, if one was provided.
    pub fn program(&self) -> Option<&str> {
        self.program.as_deref()
    }

    /// Arguments forwarded to the executable when it has to be launched.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Whether diagnostic messages should be printed to standard error.
    pub fn verbose(&self) -> bool {
        self.verbose
    }

    // TODO: consumed by the upcoming window search/action pipeline.
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

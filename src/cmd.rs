use crate::{Action, Pattern, Search};
use clap::Parser;
use regex::{Error, Regex};

#[derive(Parser, Debug)]
#[command(version, about, allow_negative_numbers = true)]
pub struct Config {
    /// executable program name
    program: Option<String>,

    /// command line arguments
    args: Vec<String>,

    /// match the window resource class name
    #[arg(short, long, value_name = "CLASS REGEX")]
    class: Option<Regex>,

    /// match the window resource name
    #[arg(short, long, value_name = "NAME REGEX")]
    name: Option<Regex>,

    /// match the window title
    #[arg(short, long, value_name = "TITLE REGEX")]
    title: Option<Regex>,

    /// desktop index
    #[arg(short, long, value_name = "INDEX")]
    desktop: Option<i8>,

    /// move the window to the target desktop index
    #[arg(short = 'D', long, value_name = "INDEX")]
    to_desktop: Option<i8>,

    /// move the window to the first screen matching the regex
    #[arg(short = 'S', long, value_name = "SCREEN REGEX")]
    to_screen: Option<Regex>,

    /// set the target window geometry
    #[arg(short, long)]
    geometry: Option<String>,

    /// print the matched window id instead of activating it
    #[arg(short, long)]
    id: bool,

    /// print diagnostic messages to standard error
    #[arg(short, long)]
    verbose: bool,
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

    /// Whether the matched window id should be printed instead of activating it.
    pub fn id(&self) -> bool {
        self.id
    }

    /// Search criteria explicitly requested on the command line, in the order
    /// the KWin script expects them.
    pub fn search(&self) -> impl Iterator<Item = Search> {
        let mut search = Vec::new();
        if let Some(class) = &self.class {
            search.push(Search::Class(Pattern::new(class)));
        }
        if let Some(name) = &self.name {
            search.push(Search::Name(Pattern::new(name)));
        }
        search.into_iter()
    }

    /// Actions applied to the matched window. `activate` is always appended last
    /// unless `--id` was given, in which case the caller only wants the id.
    pub fn actions(&self) -> Result<Vec<Action>, Error> {
        let mut actions = Vec::new();
        if let Some(to_desktop) = self.to_desktop {
            actions.push(Action::ToDesktop(to_desktop));
        }
        if !self.id {
            actions.push(Action::Activate);
        }
        Ok(actions)
    }

    /// Serialises the request into the wire format consumed by the KWin script:
    /// `search && search && action;action`. When no explicit search criteria are
    /// given the executable name is matched against the resource class, as
    /// documented (e.g. `dolphin` becomes `^dolphin$`).
    pub fn command(&self) -> Result<String, Error> {
        let mut parts: Vec<String> = self.search().map(|s| s.to_string()).collect();
        if parts.is_empty() {
            if let Some(program) = &self.program {
                parts.push(format!("class=^{}$", regex::escape(program)));
            }
        }
        let actions: Vec<String> = self
            .actions()?
            .iter()
            .map(|a| a.to_string())
            .collect();
        parts.push(actions.join(";"));
        Ok(parts.join("&&"))
    }
}

#[cfg(test)]
mod test {
    use super::Config;
    use clap::Parser;

    fn config(args: &[&str]) -> Config {
        Config::parse_from(std::iter::once("kwintool").chain(args.iter().copied()))
    }

    #[test]
    fn class_defaults_to_program_name() {
        assert_eq!(
            config(&["dolphin"]).command().unwrap(),
            "class=^dolphin$&&activate",
        );
    }

    #[test]
    fn explicit_class_overrides_program_name() {
        assert_eq!(
            config(&["-c", "konsole", "dolphin"])
                .command()
                .unwrap(),
            "class=konsole&&activate",
        );
    }

    #[test]
    fn negated_class_is_serialised_with_bang() {
        assert_eq!(
            config(&["--class", "!fleet"]).command().unwrap(),
            "class!=fleet&&activate",
        );
    }

    #[test]
    fn name_criterion_is_serialised() {
        assert_eq!(
            config(&["--name", "jetbrains"])
                .command()
                .unwrap(),
            "name=jetbrains&&activate",
        );
    }

    #[test]
    fn class_and_name_combine_with_negation() {
        assert_eq!(
            config(&["--class", "!fleet", "--name", "jetbrains"])
                .command()
                .unwrap(),
            "class!=fleet&&name=jetbrains&&activate",
        );
    }

    #[test]
    fn to_desktop_action_precedes_activate() {
        assert_eq!(
            config(&["-D", "1", "dolphin"]).command().unwrap(),
            "class=^dolphin$&&desktop=1;activate",
        );
    }

    #[test]
    fn to_desktop_all_uses_negative_index() {
        assert_eq!(
            config(&["-D", "-1", "dolphin"])
                .command()
                .unwrap(),
            "class=^dolphin$&&desktop=-1;activate",
        );
    }

    #[test]
    fn id_omits_the_activate_action() {
        assert_eq!(
            config(&["-i", "dolphin"]).command().unwrap(),
            "class=^dolphin$&&",
        );
    }
}

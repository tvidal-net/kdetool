use crate::{Action, Geometry, Pattern, Search};
use clap::Parser;
use regex::{Error, Regex};

/// Translates a user-facing, 1-based desktop number into the 0-based index the
/// KWin script uses to address the `workspace.desktops` array. This keeps the
/// CLI aligned with the numbering KDE shows the user while the wire protocol
/// consumed by the script stays 0-based. Non-positive values are sentinels
/// (negative means "all desktops"), so they pass through unchanged.
fn wire_desktop(desktop: i8) -> i8 {
    if desktop > 0 { desktop - 1 } else { desktop }
}

#[derive(Parser, Debug)]
#[command(version, about, allow_negative_numbers = true)]
pub struct Config {
    /// executable program name
    program: Option<String>,

    /// command line arguments
    ///
    /// Option parsing stops at the program name: every token after it is
    /// forwarded to the program verbatim, even if it looks like an option
    /// (e.g. `brave --profile-directory=Default`). A literal `--` may be used
    /// to force the boundary explicitly.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
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
        if let Some(title) = &self.title {
            search.push(Search::Title(Pattern::new(title)));
        }
        if let Some(desktop) = self.desktop {
            search.push(Search::Desktop(wire_desktop(desktop)));
        }
        search.into_iter()
    }

    /// Actions applied to the matched window. `activate` is always appended last
    /// unless `--id` was given, in which case the caller only wants the id.
    pub fn actions(&self) -> Result<Vec<Action>, Error> {
        let mut actions = Vec::new();
        if let Some(to_desktop) = self.to_desktop {
            actions.push(Action::ToDesktop(wire_desktop(to_desktop)));
        }
        if let Some(to_screen) = &self.to_screen {
            actions.push(Action::ToScreen(to_screen.as_str().to_string()));
        }
        if let Some(geometry) = &self.geometry {
            actions.push(Action::Geometry(Geometry::parse(geometry)?));
        }
        if !self.id {
            actions.push(Action::Activate);
        }
        Ok(actions)
    }

    /// Serialises the request into the wire format consumed by the KWin script:
    /// `search && search && action;action`. When no class/name/title pattern is
    /// given the executable name is matched against the resource class, as
    /// documented (e.g. `dolphin` becomes `^dolphin$`); a bare `--desktop` filter
    /// does not displace this default.
    pub fn command(&self) -> Result<String, Error> {
        let mut parts = Vec::new();
        let has_pattern = self.class.is_some() || self.name.is_some() || self.title.is_some();
        if !has_pattern && let Some(program) = &self.program {
            parts.push(format!("name=^{}$", regex::escape(program)));
        }
        parts.extend(self.search().map(|s| s.to_string()));

        let actions: Vec<String> = self
            .actions()?
            .iter()
            .map(|a| a.to_string())
            .collect();
        parts.push(actions.join(";"));
        Ok(parts.join("&&"))
    }

    pub fn vprintln(&self, msg: String) {
        if self.verbose() {
            eprintln!("KWinTool: {}", msg);
        }
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
    fn name_defaults_to_program_name() {
        assert_eq!(
            config(&["dolphin"]).command().unwrap(),
            "name=^dolphin$&&activate",
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
        // The CLI is 1-based; desktop 1 becomes the 0-based wire index 0.
        assert_eq!(
            config(&["-D", "1", "dolphin"]).command().unwrap(),
            "name=^dolphin$&&desktop=0;activate",
        );
    }

    #[test]
    fn to_desktop_all_uses_negative_index() {
        assert_eq!(
            config(&["-D", "-1", "dolphin"])
                .command()
                .unwrap(),
            "name=^dolphin$&&desktop=-1;activate",
        );
    }

    #[test]
    fn to_desktop_converts_one_based_cli_to_zero_based_wire() {
        assert_eq!(
            config(&["-D", "3", "dolphin"]).command().unwrap(),
            "name=^dolphin$&&desktop=2;activate",
        );
    }

    #[test]
    fn title_criterion_is_serialised() {
        assert_eq!(
            config(&["--title", "Vim"]).command().unwrap(),
            "title=Vim&&activate",
        );
    }

    #[test]
    fn to_screen_action_precedes_activate() {
        assert_eq!(
            config(&["-S", "edp", "dolphin"])
                .command()
                .unwrap(),
            "name=^dolphin$&&screen=edp;activate",
        );
    }

    #[test]
    fn to_desktop_and_to_screen_keep_order() {
        assert_eq!(
            config(&["-D", "2", "-S", "edp", "dolphin"])
                .command()
                .unwrap(),
            "name=^dolphin$&&desktop=1;screen=edp;activate",
        );
    }

    #[test]
    fn desktop_criterion_is_serialised() {
        assert_eq!(
            config(&["--desktop", "2"]).command().unwrap(),
            "desktop=1&&activate",
        );
    }

    #[test]
    fn desktop_filter_keeps_the_program_name_default() {
        assert_eq!(
            config(&["-d", "3", "dolphin"]).command().unwrap(),
            "name=^dolphin$&&desktop=2&&activate",
        );
    }

    #[test]
    fn explicit_pattern_suppresses_the_program_class_default() {
        assert_eq!(
            config(&["-t", "Vim", "vim"]).command().unwrap(),
            "title=Vim&&activate",
        );
    }

    #[test]
    fn geometry_action_is_serialised() {
        assert_eq!(
            config(&["-g", "w60%x20%m", "dolphin"])
                .command()
                .unwrap(),
            "name=^dolphin$&&geometry=w60%x20%m;activate",
        );
    }

    #[test]
    fn invalid_geometry_is_rejected() {
        assert!(
            config(&["-g", "a3", "dolphin"])
                .command()
                .is_err()
        );
    }

    #[test]
    fn program_args_after_the_program_are_passed_through_verbatim() {
        // Regression: option-looking tokens after the program name must be
        // forwarded to it, not parsed as kwintool's own options.
        let cfg = config(&[
            "--class",
            "fmpnliohjhemenmnlpbfagaolkdacoja",
            "brave",
            "--profile-directory=Default",
            "--app-id=fmpnliohjhemenmnlpbfagaolkdacoja",
        ]);
        assert_eq!(cfg.program(), Some("brave"));
        assert_eq!(
            cfg.args(),
            &[
                "--profile-directory=Default",
                "--app-id=fmpnliohjhemenmnlpbfagaolkdacoja",
            ],
        );
    }

    #[test]
    fn double_dash_forces_the_program_boundary() {
        let cfg = config(&["--", "brave", "--app-id=x"]);
        assert_eq!(cfg.program(), Some("brave"));
        assert_eq!(cfg.args(), &["--app-id=x"]);
    }

    #[test]
    fn id_omits_the_activate_action() {
        assert_eq!(
            config(&["-i", "dolphin"]).command().unwrap(),
            "name=^dolphin$&&",
        );
    }
}

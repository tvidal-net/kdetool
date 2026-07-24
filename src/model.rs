use std::fmt;

use regex::Regex;

use crate::geometry::Geometry;

/// Translates a user-facing, 1-based desktop number into the 0-based index the
/// KWin script uses to address the `workspace.desktops` array. This keeps both
/// the CLI (`-d`/`-D`) and the config-file rules aligned with the numbering KDE
/// shows the user while the wire protocol stays 0-based. Non-positive values are
/// sentinels (negative means "all desktops") and pass through unchanged.
pub fn wire_desktop(desktop: i8) -> i8 {
    if desktop > 0 { desktop - 1 } else { desktop }
}

/// A regular-expression criterion that may be negated with a leading `!`,
/// mirroring the `field!=value` form understood by the KWin script.
#[derive(Debug)]
pub struct Pattern {
    negated: bool,
    source: String,
}

impl Pattern {
    pub fn new(regex: &Regex) -> Self {
        match regex.as_str().strip_prefix('!') {
            Some(source) => Pattern {
                negated: true,
                source: source.to_string(),
            },
            None => Pattern {
                negated: false,
                source: regex.as_str().to_string(),
            },
        }
    }

    /// Writes the criterion as `field=source`, or `field!=source` when negated.
    fn write(&self, f: &mut fmt::Formatter<'_>, field: &str) -> fmt::Result {
        let negation = if self.negated { "!" } else { "" };
        write!(f, "{field}{negation}={}", self.source)
    }
}

#[derive(Debug)]
pub enum Search {
    Class(Pattern),
    Name(Pattern),
    Title(Pattern),
    Desktop(i8),
}

impl fmt::Display for Search {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Search::Class(pattern) => pattern.write(f, "class"),
            Search::Name(pattern) => pattern.write(f, "name"),
            Search::Title(pattern) => pattern.write(f, "title"),
            Search::Desktop(desktop) => write!(f, "desktop={desktop}"),
        }
    }
}

#[derive(Debug)]
pub enum Action {
    ToDesktop(i8),
    ToScreen(String),
    NoBorder(bool),
    Maximize(bool),
    KeepBelow(bool),
    Geometry(Vec<Geometry>),
    Activate,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::ToDesktop(desktop) => write!(f, "desktop={desktop}"),
            Action::ToScreen(screen) => write!(f, "screen={screen}"),
            Action::NoBorder(on) => write!(f, "noborder={on}"),
            Action::Maximize(on) => write!(f, "maximize={on}"),
            Action::KeepBelow(on) => write!(f, "keepbelow={on}"),
            Action::Geometry(parts) => {
                write!(f, "geometry=")?;
                for part in parts {
                    write!(f, "{part}")?;
                }
                Ok(())
            }
            Action::Activate => write!(f, "activate"),
        }
    }
}

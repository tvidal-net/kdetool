use crate::cmd::Config;
use crate::kwin::{KWin, KWinClient};
use crate::service::Service;
use clap::Parser;
use regex::{Error, Regex};
use std::process::ExitCode;
use std::time::Duration;
use std::{fmt, sync};

mod cmd;
mod kwin;
mod proc;
mod service;

pub const TIMEOUT: Duration = Duration::from_secs(3);

/// Window maximize state, encoded as part of the geometry mini-language: `v`
/// maximizes vertically while `m` maximizes in both directions.
#[derive(Debug, PartialEq)]
pub enum Maximize {
    Vertical,
    Both,
}

impl fmt::Display for Maximize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Maximize::Vertical => write!(f, "v"),
            Maximize::Both => write!(f, "m"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Length {
    Pixels(u32),
    Percent(u32),
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Length::Pixels(value) => write!(f, "{value}"),
            Length::Percent(value) => write!(f, "{value}%"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Geometry {
    Width(Length),
    Height(Length),
    Left(Length),
    Top(Length),
    Maximize(Maximize),
}

// Matches a single geometry token anchored at the start of the remaining input:
// a `w`/`h`/`x`/`y` coordinate with a value (and optional `%`), or a bare `v`/`m`
// maximize flag. Anchoring lets the parser reject undefined coordinates such as
// `a3` instead of silently skipping them.
static GEOMETRY_TOKEN: sync::LazyLock<Regex> =
    sync::LazyLock::new(|| Regex::new(r"(?i)^(?:([whxy])(\d+)(%?)|([vm]))").unwrap());

impl Geometry {
    /// Parses the geometry mini-language, returning an error if any part of the
    /// string is not a valid token (e.g. an unknown coordinate like `a3`).
    pub fn parse(s: &str) -> Result<Vec<Geometry>, Error> {
        let mut geometry = Vec::new();
        let mut rest = s;
        while !rest.is_empty() {
            let cap = GEOMETRY_TOKEN
                .captures(rest)
                .ok_or_else(|| Error::Syntax(format!("invalid geometry token: {rest:?}")))?;
            if let Some(prefix) = cap.get(1) {
                let digits = &cap[2];
                let value: u32 = digits.parse().map_err(|err| {
                    Error::Syntax(format!("invalid geometry value {digits:?}: {err}"))
                })?;
                let length = if cap[3].is_empty() {
                    Length::Pixels(value)
                } else {
                    Length::Percent(value)
                };
                geometry.push(match prefix.as_str().to_ascii_lowercase().as_str() {
                    "w" => Geometry::Width(length),
                    "h" => Geometry::Height(length),
                    "x" => Geometry::Left(length),
                    "y" => Geometry::Top(length),
                    _ => unreachable!("geometry regex only captures w, h, x, or y"),
                });
            } else if let Some(maximize) = cap.get(4) {
                geometry.push(Geometry::Maximize(
                    match maximize.as_str().to_ascii_lowercase().as_str() {
                        "v" => Maximize::Vertical,
                        "m" => Maximize::Both,
                        _ => unreachable!("maximize regex only captures v or m"),
                    },
                ));
            }
            rest = &rest[cap.get(0).unwrap().end()..];
        }
        Ok(geometry)
    }
}

impl fmt::Display for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Geometry::Width(width) => write!(f, "w{width}"),
            Geometry::Height(height) => write!(f, "h{height}"),
            Geometry::Left(left) => write!(f, "x{left}"),
            Geometry::Top(top) => write!(f, "y{top}"),
            Geometry::Maximize(maximize) => write!(f, "{maximize}"),
        }
    }
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
    Geometry(Vec<Geometry>),
    Activate,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::ToDesktop(desktop) => write!(f, "desktop={desktop}"),
            Action::ToScreen(screen) => write!(f, "screen={screen}"),
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

// Exit code used when the target program is running but no matching window
// could be activated, mirroring "command not found" semantics for scripts.
const NO_WINDOW: u8 = 127;

fn main() -> ExitCode {
    let config = Config::parse();
    match run(&config) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("kwintool: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(config: &Config) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let kwin = KWinClient::new()?;

    // When a target program is given but is not running yet, launch it
    // detached and stop here: there is no existing window to focus.
    if let Some(program) = config.program() {
        if !proc::is_running(program) {
            if config.verbose() {
                eprintln!("kwintool: {program} is not running, launching it");
            }
            proc::launch(program, config.args())?;
            return Ok(ExitCode::SUCCESS);
        }
    }

    // With neither a program nor any search criteria there is nothing to focus,
    // so the round-trip would be a no-op: stop before bothering the script.
    if config.program().is_none() && config.search().next().is_none() {
        return Ok(ExitCode::SUCCESS);
    }

    // Everything below drives the bundled KWin script, which must be loaded.
    if !kwin.is_script_loaded()? {
        return Err("the KWinTool KWin script is not loaded".into());
    }

    // Serialise the search criteria and actions into the wire format the script
    // parses (search && search && action;action), validating any geometry.
    let command = config.command()?;
    if config.verbose() {
        eprintln!("kwintool: command {command}");
    }

    // Own the service name first so it exists when the script calls back, wake
    // the script via its shortcut, then process the fetchNextAction/sendReply
    // round-trip until sendReply reports the outcome.
    let service = Service::register(command)?;
    kwin.invoke_shortcut()?;
    let reply = service.serve()?;

    match reply.as_deref() {
        // The script replies "OK <window-id>" on success; surface the id on
        // stdout only when the caller asked for it with --id.
        Some(reply) if reply.starts_with("OK") => {
            if config.id() {
                let id = reply["OK".len()..].trim();
                if !id.is_empty() {
                    println!("{id}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("NotFound") => {
            eprintln!("kwintool: no window matched the search criteria");
            Ok(ExitCode::from(NO_WINDOW))
        }
        None => Ok(ExitCode::SUCCESS),
        Some(other) => Err(other.into()),
    }
}

#[cfg(test)]
mod test {
    use crate::Geometry;
    use crate::Length;
    use crate::Maximize;

    #[test]
    fn maximize_vertical() {
        assert_eq!(Maximize::Vertical.to_string(), "v");
    }

    #[test]
    fn maximize_both() {
        assert_eq!(Maximize::Both.to_string(), "m");
    }

    #[test]
    fn geometry_width() {
        assert_eq!(Geometry::Width(Length::Pixels(100)).to_string(), "w100");
    }

    #[test]
    fn geometry_height() {
        assert_eq!(Geometry::Height(Length::Pixels(100)).to_string(), "h100");
    }

    #[test]
    fn geometry_left() {
        assert_eq!(Geometry::Left(Length::Pixels(100)).to_string(), "x100");
    }

    #[test]
    fn geometry_top() {
        assert_eq!(Geometry::Top(Length::Pixels(100)).to_string(), "y100");
    }

    #[test]
    fn length_percent_display() {
        assert_eq!(Geometry::Width(Length::Percent(60)).to_string(), "w60%");
    }

    #[test]
    fn geometry_parse_round_trip() {
        let parsed: String = Geometry::parse("w1280h720x0y0")
            .unwrap()
            .iter()
            .map(|g| g.to_string())
            .collect();
        assert_eq!(parsed, "w1280h720x0y0");
    }

    #[test]
    fn geometry_parse_empty() {
        assert_eq!(Geometry::parse("").unwrap().len(), 0);
    }

    #[test]
    fn geometry_parse_lowercase() {
        assert_eq!(
            Geometry::parse("w60%h50%x10y20m").unwrap(),
            vec![
                Geometry::Width(Length::Percent(60)),
                Geometry::Height(Length::Percent(50)),
                Geometry::Left(Length::Pixels(10)),
                Geometry::Top(Length::Pixels(20)),
                Geometry::Maximize(Maximize::Both),
            ],
        );
    }

    #[test]
    fn geometry_parse_uppercase_matches_lowercase() {
        assert_eq!(
            Geometry::parse("W60%H50%X10Y20M").unwrap(),
            Geometry::parse("w60%h50%x10y20m").unwrap(),
        );
    }

    #[test]
    fn geometry_parse_maximize_vertical() {
        assert_eq!(
            Geometry::parse("v").unwrap(),
            vec![Geometry::Maximize(Maximize::Vertical)],
        );
    }

    #[test]
    fn geometry_parse_rejects_unknown_coordinate() {
        assert!(Geometry::parse("a3").is_err());
    }

    #[test]
    fn geometry_parse_rejects_trailing_garbage() {
        assert!(Geometry::parse("w100z").is_err());
    }

    #[test]
    fn geometry_parse_rejects_coordinate_without_value() {
        assert!(Geometry::parse("w").is_err());
    }
}

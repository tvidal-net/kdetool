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
mod service;

pub const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, PartialEq)]
pub struct Maximize {
    horizontal: bool,
    vertical: bool,
}

impl fmt::Display for Maximize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.vertical {
            write!(f, "v")?;
        }
        if self.horizontal {
            write!(f, "h")?;
        }
        Ok(())
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

static GEOMETRY_PARSER: sync::LazyLock<Regex> =
    sync::LazyLock::new(|| Regex::new(r"(?i)([whxy])(\d+)(%?)|m([vh]+)").unwrap());

impl Geometry {
    pub fn parse(s: &str) -> Result<impl Iterator<Item = Geometry>, Error> {
        let mut geometry = Vec::new();
        for cap in GEOMETRY_PARSER.captures_iter(s) {
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
            } else if let Some(directions) = cap.get(4) {
                let mut maximize = Maximize {
                    horizontal: false,
                    vertical: false,
                };
                for direction in directions.as_str().chars() {
                    match direction.to_ascii_lowercase() {
                        'v' => maximize.vertical = true,
                        'h' => maximize.horizontal = true,
                        _ => unreachable!("maximize regex only captures v or h"),
                    }
                }
                geometry.push(Geometry::Maximize(maximize));
            }
        }
        Ok(geometry.into_iter())
    }
}

impl fmt::Display for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Geometry::Width(width) => write!(f, "w{width}"),
            Geometry::Height(height) => write!(f, "h{height}"),
            Geometry::Left(left) => write!(f, "x{left}"),
            Geometry::Top(top) => write!(f, "y{top}"),
            Geometry::Maximize(maximize) => write!(f, "m{maximize}"),
        }
    }
}

#[derive(Debug)]
pub enum Search {
    ClassName(Regex),
    Title(Regex),
    Screen(Regex),
    Desktop(i8),
}

impl fmt::Display for Search {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Search::ClassName(class_name) => write!(f, "class={class_name}"),
            Search::Title(title) => write!(f, "title={title}"),
            Search::Screen(screen) => write!(f, "screen={screen}"),
            Search::Desktop(desktop) => write!(f, "desktop={desktop}"),
        }
    }
}

#[derive(Debug)]
pub enum Action {
    ToDesktop(i8),
    ToScreen(Regex),
    Geometry(Geometry),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::ToDesktop(desktop) => write!(f, "to-desktop={desktop}"),
            Action::ToScreen(screen) => write!(f, "to-screen={screen}"),
            Action::Geometry(geometry) => write!(f, "{geometry}"),
        }
    }
}

fn main() -> ExitCode {
    let _config = Config::parse();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("kdetool: {error}");
            ExitCode::FAILURE
        }
    }
}

// Registers the DBus service first so the name is owned, then wakes the KWin
// script via the shortcut, then serves the fetchNextAction/sendReply round-trip
// until sendReply ends the loop.
fn run() -> Result<(), dbus::Error> {
    let kwin = KWinClient::new();
    let service = Service::register()?;
    kwin.invoke_shortcut()?;
    service.serve()
}

#[cfg(test)]
mod test {
    use crate::Geometry;
    use crate::Length;
    use crate::Maximize;

    #[test]
    fn maximize_none() {
        let m = Maximize {
            horizontal: false,
            vertical: false,
        };
        assert_eq!(m.to_string(), "");
    }

    #[test]
    fn maximize_vertical() {
        let m = Maximize {
            horizontal: false,
            vertical: true,
        };
        assert_eq!(m.to_string(), "v");
    }

    #[test]
    fn maximize_horizontal() {
        let m = Maximize {
            horizontal: true,
            vertical: false,
        };
        assert_eq!(m.to_string(), "h");
    }

    #[test]
    fn maximize_both() {
        let m = Maximize {
            horizontal: true,
            vertical: true,
        };
        assert_eq!(m.to_string(), "vh");
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
            .map(|g| g.to_string())
            .collect();
        assert_eq!(parsed, "w1280h720x0y0");
    }

    #[test]
    fn geometry_parse_empty() {
        assert_eq!(Geometry::parse("").unwrap().count(), 0);
    }

    #[test]
    fn geometry_parse_lowercase() {
        assert_eq!(
            Geometry::parse("w60%h50%x10y20mvh")
                .unwrap()
                .collect::<Vec<_>>(),
            vec![
                Geometry::Width(Length::Percent(60)),
                Geometry::Height(Length::Percent(50)),
                Geometry::Left(Length::Pixels(10)),
                Geometry::Top(Length::Pixels(20)),
                Geometry::Maximize(Maximize {
                    vertical: true,
                    horizontal: true,
                }),
            ],
        );
    }

    #[test]
    fn geometry_parse_uppercase_matches_lowercase() {
        assert_eq!(
            Geometry::parse("W60%H50%X10Y20MVH")
                .unwrap()
                .collect::<Vec<_>>(),
            Geometry::parse("w60%h50%x10y20mvh")
                .unwrap()
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn geometry_parse_maximize_only() {
        assert_eq!(
            Geometry::parse("MV").unwrap().collect::<Vec<_>>(),
            vec![Geometry::Maximize(Maximize {
                vertical: true,
                horizontal: false,
            })],
        );
    }
}

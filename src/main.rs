use crate::cmd::Config;
use clap::Parser;
use regex::{Error, Regex};
use std::process::ExitCode;
use std::time::Duration;
use std::{fmt, sync};

mod cmd;
mod kwin;
mod service;

pub const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
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

#[derive(Debug)]
pub enum Geometry {
    Width(u32),
    Height(u32),
    Left(u32),
    Top(u32),
    Maximize(Maximize),
}

static GEOMETRY_PARSER: sync::LazyLock<Regex> =
    sync::LazyLock::new(|| Regex::new(r"([whxy])(\d+)(%?)").unwrap());

impl Geometry {
    pub fn parse(s: &str) -> Result<impl Iterator<Item = Geometry>, Error> {
        let mut geometry = Vec::new();
        for cap in GEOMETRY_PARSER.captures_iter(s) {
            let (prefix, value, percent) = (&cap[1], &cap[2], !cap[3].is_empty());
            match prefix {
                "w" => geometry.push(Geometry::Width(value.parse().unwrap())),
                "h" => geometry.push(Geometry::Height(value.parse().unwrap())),
                "x" => geometry.push(Geometry::Left(value.parse().unwrap())),
                "y" => geometry.push(Geometry::Top(value.parse().unwrap())),
                _ => return Err(Error::Syntax(String::from_str("Invalid Geometry Prefix {prefix}")?)),
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
            Geometry::Maximize(maximize) => write!(f, "{maximize}"),
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
    let config = Config::parse();
    println!("{config:?}");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod test {
    use crate::Geometry;
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
        assert_eq!(Geometry::Width(100).to_string(), "w100");
    }

    #[test]
    fn geometry_height() {
        assert_eq!(Geometry::Height(100).to_string(), "h100");
    }

    #[test]
    fn geometry_left() {
        assert_eq!(Geometry::Left(100).to_string(), "x100");
    }

    #[test]
    fn geometry_top() {
        assert_eq!(Geometry::Top(100).to_string(), "y100");
    }
}

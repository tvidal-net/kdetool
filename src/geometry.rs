use std::fmt;
use std::sync::LazyLock;

use regex::{Error, Regex};

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
static GEOMETRY_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:([whxy])(\d+)(%?)|([vm]))").unwrap());

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

#[cfg(test)]
mod test {
    use super::{Geometry, Length, Maximize};

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

//! A small, self-contained HOCON reader built on `serde`.
//!
//! The module depends only on `serde` and holds no references to the rest of
//! `kwintool`, so it can be lifted into its own crate later. Point a
//! `#[derive(Deserialize)]` type at [`from_str`] and read the subset documented
//! on [`parser::parse`].

mod de;
mod error;
mod parser;
mod value;

pub use de::from_str;
pub use error::Error;
pub use value::Value;

#[cfg(test)]
mod test {
    use super::{from_str, Value};
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Rule {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        class: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default, rename = "to-desktop")]
        to_desktop: Option<i8>,
        #[serde(default)]
        geometry: Option<String>,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct Config {
        rules: Vec<Rule>,
    }

    #[test]
    fn parses_a_rule_list_without_root_braces() {
        let config: Config = from_str(
            r#"
            # a couple of rules
            rules = [
              { name = browsers, class = "google-chrome|firefox", geometry = "x17%w67%v" }
              { class = mpv, title = ipcam1, to-desktop = 2 }  // trailing comment
            ]
            "#,
        )
        .unwrap();

        assert_eq!(
            config.rules,
            vec![
                Rule {
                    name: Some("browsers".to_string()),
                    class: Some("google-chrome|firefox".to_string()),
                    title: None,
                    to_desktop: None,
                    geometry: Some("x17%w67%v".to_string()),
                },
                Rule {
                    name: None,
                    class: Some("mpv".to_string()),
                    title: Some("ipcam1".to_string()),
                    to_desktop: Some(2),
                    geometry: None,
                },
            ],
        );
    }

    #[test]
    fn supports_nested_object_shorthand_and_colon() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Screens {
            main: String,
            other: String,
        }
        #[derive(Debug, PartialEq, Deserialize)]
        struct Root {
            screens: Screens,
        }

        let root: Root = from_str(
            r#"
            screens {
              main:  "HDMI"
              other: "DP"
            }
            "#,
        )
        .unwrap();

        assert_eq!(root.screens.main, "HDMI");
        assert_eq!(root.screens.other, "DP");
    }

    #[test]
    fn later_keys_override_earlier_ones() {
        let value = super::parser::parse("a = 1\na = 2\n").unwrap();
        assert_eq!(value.get("a"), Some(&Value::Integer(2)));
    }

    #[test]
    fn reports_unterminated_object() {
        let err = from_str::<Config>("rules = [ { class = mpv ").unwrap_err();
        assert!(err.to_string().contains("unterminated"), "{err}");
    }

    #[test]
    fn negative_and_bare_values_parse() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Item {
            #[serde(rename = "to-desktop")]
            to_desktop: i8,
            enabled: bool,
        }
        let item: Item = from_str("to-desktop = -1\nenabled = true\n").unwrap();
        assert_eq!(item.to_desktop, -1);
        assert!(item.enabled);
    }
}

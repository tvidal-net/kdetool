use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

use regex::Regex;
use serde::Deserialize;

use crate::geometry::Geometry;
use crate::hocon;
use crate::model::{Action, Pattern, Search, wire_desktop};

/// One window rule: optional match criteria (`class`/`title`) plus the actions
/// applied to a matching window. `to-desktop` is 1-based like the CLI. Fields
/// such as `no-border` are intentionally not here yet; they land with the wire
/// actions that back them.
#[derive(Debug, Deserialize)]
struct Rule {
    /// Human-readable label, ignored by matching (kept for readability/logging).
    #[serde(default)]
    #[allow(dead_code)]
    label: Option<String>,
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "to-desktop")]
    to_desktop: Option<i8>,
    #[serde(default, rename = "to-screen")]
    to_screen: Option<String>,
    #[serde(default)]
    geometry: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    rules: Vec<Rule>,
}

/// The parsed window-rule configuration. The background service reads this fresh
/// on every `GetTargets`/`WindowAction`, so it holds no long-lived state.
pub struct Config {
    rules: Vec<Rule>,
}

/// Location of the config file: `$XDG_CONFIG_HOME/kwintool/config.conf`, falling
/// back to `~/.config/kwintool/config.conf`.
fn config_path() -> PathBuf {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("kwintool").join("config.conf")
}

/// Reads and parses the config file. A missing file is not an error — it yields
/// an empty rule set — but a present, malformed file is, so the failure surfaces
/// (verbosely, per the service's no-restart policy) instead of being swallowed.
pub fn load() -> Result<Config, Box<dyn Error>> {
    let path = config_path();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Config { rules: Vec::new() }),
        Err(err) => return Err(format!("{}: {err}", path.display()).into()),
    };
    Config::from_str(&text).map_err(|err| format!("{}: {err}", path.display()).into())
}

/// Compiles a matcher, honouring a leading `!` as negation (mirroring the CLI
/// and the KWin script), then reports whether `text` matches.
fn matches(matcher: &str, text: &str) -> Result<bool, regex::Error> {
    let (negated, source) = match matcher.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, matcher),
    };
    Ok(Regex::new(source)?.is_match(text) != negated)
}

impl Rule {
    fn has_criteria(&self) -> bool {
        self.class.is_some() || self.title.is_some()
    }

    /// The rule's match criteria as `Search` values, for the target list.
    fn search(&self) -> Result<Vec<Search>, regex::Error> {
        let mut search = Vec::new();
        if let Some(class) = &self.class {
            search.push(Search::Class(Pattern::new(&Regex::new(class)?)));
        }
        if let Some(title) = &self.title {
            search.push(Search::Title(Pattern::new(&Regex::new(title)?)));
        }
        Ok(search)
    }

    /// The rule's actions, in the same order the CLI emits them (minus the
    /// trailing `activate`, since the service places an already-open window).
    fn actions(&self) -> Result<Vec<Action>, Box<dyn Error>> {
        let mut actions = Vec::new();
        if let Some(desktop) = self.to_desktop {
            actions.push(Action::ToDesktop(wire_desktop(desktop)));
        }
        if let Some(screen) = &self.to_screen {
            actions.push(Action::ToScreen(screen.clone()));
        }
        if let Some(geometry) = &self.geometry {
            actions.push(Action::Geometry(Geometry::parse(geometry)?));
        }
        Ok(actions)
    }

    /// Whether this rule applies to a window with the given caption and class.
    /// A rule with no criteria never matches (it produces no target either).
    fn matches_window(&self, caption: &str, class: &str) -> Result<bool, regex::Error> {
        if !self.has_criteria() {
            return Ok(false);
        }
        if let Some(pattern) = &self.class {
            if !matches(pattern, class)? {
                return Ok(false);
            }
        }
        if let Some(pattern) = &self.title {
            if !matches(pattern, caption)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl Config {
    /// Parses configuration from a HOCON string (the filesystem-free path used by
    /// [`load`] and the tests).
    pub fn from_str(text: &str) -> Result<Config, Box<dyn Error>> {
        let file: ConfigFile = hocon::from_str(text)?;
        Ok(Config { rules: file.rules })
    }

    /// The target list served by `GetTargets`: one search expression per rule
    /// with criteria, in the wire format the KWin script parses, newline
    /// separated. Rules without criteria are skipped.
    pub fn targets(&self) -> Result<String, Box<dyn Error>> {
        let mut lines = Vec::new();
        for rule in &self.rules {
            if !rule.has_criteria() {
                continue;
            }
            let search: Vec<String> = rule.search()?.iter().map(Search::to_string).collect();
            lines.push(search.join("&&"));
        }
        Ok(lines.join("\n"))
    }

    /// The merged action list served by `WindowAction` for a `caption:class`
    /// window: every matching rule's actions concatenated in file order (so a
    /// later rule overrides an earlier one), `;` separated, or empty when nothing
    /// matches. The class is the segment after the last `:` (a resource class
    /// never contains one; a caption might).
    pub fn action_for(&self, window: &str) -> Result<String, Box<dyn Error>> {
        let (caption, class) = match window.rsplit_once(':') {
            Some((caption, class)) => (caption, class),
            None => ("", window),
        };
        let mut actions = Vec::new();
        for rule in &self.rules {
            if rule.matches_window(caption, class)? {
                actions.extend(rule.actions()?.iter().map(Action::to_string));
            }
        }
        Ok(actions.join(";"))
    }
}

#[cfg(test)]
mod test {
    use super::Config;

    fn config(text: &str) -> Config {
        Config::from_str(text).expect("valid config")
    }

    #[test]
    fn targets_lists_one_search_expression_per_rule_with_criteria() {
        let config = config(
            r#"
            rules = [
              { class = "google-chrome|firefox" }
              { class = mpv, title = ipcam1 }
              { geometry = "m" }               # no criteria -> skipped
            ]
            "#,
        );
        assert_eq!(
            config.targets().unwrap(),
            "class=google-chrome|firefox\nclass=mpv&&title=ipcam1",
        );
    }

    #[test]
    fn action_for_merges_matching_rules_in_file_order() {
        let config = config(
            r#"
            rules = [
              { class = alacritty, to-desktop = 3 }
              { class = alacritty, geometry = "m" }
            ]
            "#,
        );
        // window arrives as caption:class; to-desktop is 1-based -> wire 2.
        assert_eq!(
            config.action_for("Terminal:alacritty").unwrap(),
            "desktop=2;geometry=m",
        );
    }

    #[test]
    fn action_for_is_empty_when_nothing_matches() {
        let config = config(r#"rules = [ { class = mpv, geometry = "m" } ]"#);
        assert_eq!(config.action_for("Editor:code").unwrap(), "");
    }

    #[test]
    fn title_criterion_matches_the_caption_half_only() {
        let config = config(r#"rules = [ { class = mpv, title = ipcam1, to-desktop = 1 } ]"#);
        assert_eq!(config.action_for("ipcam1 stream:mpv").unwrap(), "desktop=0");
        assert_eq!(config.action_for("ipcam2 stream:mpv").unwrap(), "");
    }

    #[test]
    fn negated_class_becomes_not_equals_on_the_wire() {
        let config = config(r#"rules = [ { class = "!fleet" } ]"#);
        assert_eq!(config.targets().unwrap(), "class!=fleet");
    }

    #[test]
    fn missing_config_body_yields_no_rules() {
        let config = config("rules = []");
        assert_eq!(config.targets().unwrap(), "");
        assert_eq!(config.action_for("x:y").unwrap(), "");
    }
}

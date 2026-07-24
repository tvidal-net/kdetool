use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    #[serde(default, rename = "no-border")]
    no_border: Option<bool>,
    /// `false` un-maximizes a window that opens maximized; `true` maximizes it.
    #[serde(default)]
    maximize: Option<bool>,
    #[serde(default, rename = "keep-below")]
    keep_below: Option<bool>,
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

fn home_dir() -> PathBuf {
    PathBuf::from(env::var_os("HOME").unwrap_or_default())
}

fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
}

/// The ordered default config candidates, most preferred first:
/// `$XDG_CONFIG_HOME/kwintool.cfg` (i.e. `~/.config/kwintool.cfg`) then
/// `~/.kwintool.cfg`. Split out so the ordering is unit-testable.
fn candidates(config_home: &Path, home: &Path) -> Vec<PathBuf> {
    vec![config_home.join("kwintool.cfg"), home.join(".kwintool.cfg")]
}

fn default_candidates() -> Vec<PathBuf> {
    candidates(&config_home(), &home_dir())
}

/// Reads and parses the window-rules config.
///
/// With an explicit `--config` path the file must exist (a typo should not be
/// silently ignored). With the default search path the first existing candidate
/// wins; if none exist the result is an empty rule set. Either way a present but
/// malformed file is a loud error, per the service's no-restart policy.
pub fn load(explicit: Option<&Path>) -> Result<Config, Box<dyn Error>> {
    if let Some(path) = explicit {
        let text =
            fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
        return Config::from_str(&text).map_err(|err| format!("{}: {err}", path.display()).into());
    }

    for path in default_candidates() {
        match fs::read_to_string(&path) {
            Ok(text) => {
                return Config::from_str(&text)
                    .map_err(|err| format!("{}: {err}", path.display()).into());
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(format!("{}: {err}", path.display()).into()),
        }
    }
    Ok(Config { rules: Vec::new() })
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

    /// The rule's actions. Geometry is emitted last (it defers to a timer on the
    /// script side); the toggles and maximize apply synchronously before it. The
    /// trailing `activate` is omitted, since the service places an already-open
    /// window rather than focusing it.
    fn actions(&self) -> Result<Vec<Action>, Box<dyn Error>> {
        let mut actions = Vec::new();
        if let Some(desktop) = self.to_desktop {
            actions.push(Action::ToDesktop(wire_desktop(desktop)));
        }
        if let Some(screen) = &self.to_screen {
            actions.push(Action::ToScreen(screen.clone()));
        }
        if let Some(no_border) = self.no_border {
            actions.push(Action::NoBorder(no_border));
        }
        if let Some(maximize) = self.maximize {
            actions.push(Action::Maximize(maximize));
        }
        if let Some(keep_below) = self.keep_below {
            actions.push(Action::KeepBelow(keep_below));
        }
        if let Some(geometry) = &self.geometry {
            actions.push(Action::Geometry(Geometry::parse(geometry)?));
        }
        Ok(actions)
    }

    /// Whether this rule applies to a window. `class` matches the resource class;
    /// `title` matches the full `caption:class` composite (so patterns can anchor
    /// on the class, matching the KWin script and CLI). A rule with no criteria
    /// never matches (it produces no target either).
    fn matches_window(&self, window: &str, class: &str) -> Result<bool, regex::Error> {
        if !self.has_criteria() {
            return Ok(false);
        }
        if let Some(pattern) = &self.class {
            if !matches(pattern, class)? {
                return Ok(false);
            }
        }
        if let Some(pattern) = &self.title {
            if !matches(pattern, window)? {
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
        let class = match window.rsplit_once(':') {
            Some((_, class)) => class,
            None => window,
        };
        let mut actions = Vec::new();
        for rule in &self.rules {
            if rule.matches_window(window, class)? {
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
    fn title_matches_the_caption_class_composite() {
        let config = config(r#"rules = [ { class = mpv, title = ipcam1, to-desktop = 1 } ]"#);
        assert_eq!(config.action_for("ipcam1 stream:mpv").unwrap(), "desktop=0");
        assert_eq!(config.action_for("ipcam2 stream:mpv").unwrap(), "");
    }

    #[test]
    fn title_can_anchor_on_the_class_suffix() {
        // The class is appended after the caption, so `:class$` style patterns
        // work — the reason the JetBrains rule's original composite matters.
        let config = config(r#"rules = [ { title = ":mpv$", to-desktop = 1 } ]"#);
        assert_eq!(config.action_for("ipcam1:mpv").unwrap(), "desktop=0");
        assert_eq!(config.action_for("ipcam1:code").unwrap(), "");
    }

    #[test]
    fn border_maximize_and_keep_below_serialise() {
        let config = config(
            r#"rules = [ { class = x, no-border = true, maximize = false, keep-below = true } ]"#,
        );
        assert_eq!(
            config.action_for("cap:x").unwrap(),
            "noborder=true;maximize=false;keepbelow=true",
        );
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

    #[test]
    fn bundled_example_config_parses_and_translates() {
        let config = Config::from_str(include_str!("../examples/kwintool.cfg"))
            .expect("example config parses");

        let targets = config.targets().unwrap();
        assert!(targets.contains("class=google-chrome|brave-browser|firefox"));
        assert!(targets.contains("class=mpv&&title=ipcam1"));

        // Browsers get the centred band.
        assert_eq!(
            config.action_for("anything:firefox").unwrap(),
            "geometry=x17%w67%v",
        );
        // A camera window is positioned and also caught by the catch-all mpv
        // rule (DP, all desktops, kept below) — the cumulative behaviour of the
        // original, reproduced by merging matching rules in file order.
        assert_eq!(
            config.action_for("ipcam1:mpv").unwrap(),
            "geometry=x640y352;desktop=-1;screen=DP;keepbelow=true",
        );
        // JetBrains/Fleet is borderless; matched via the class suffix.
        assert_eq!(
            config.action_for("x - Fleet:jetbrains-fleet").unwrap(),
            "noborder=true;geometry=x17%w67%v",
        );
        // DIYLC opens maximized and gets un-maximized.
        assert_eq!(
            config.action_for("DIY Layout Creator:org-diylc-DIYLC").unwrap(),
            "maximize=false",
        );
    }

    #[test]
    fn default_candidates_are_ordered_config_home_then_home() {
        use std::path::{Path, PathBuf};
        let candidates = super::candidates(Path::new("/home/u/.config"), Path::new("/home/u"));
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/home/u/.config/kwintool.cfg"),
                PathBuf::from("/home/u/.kwintool.cfg"),
            ],
        );
    }
}

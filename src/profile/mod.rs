//! Profiles: named, partial configs stored as `profiles/<name>.toml` in the
//! config dir. A profile file is any subset of `config.toml` — the keys it
//! lists are the settings it controls; everything else is left alone.
//!
//! Several profiles can be active at once as long as they control disjoint
//! settings. Activating a profile deactivates every active profile that
//! shares a setting with it, and a manual change to a setting deactivates the
//! profiles that set it, so an active profile always matches the live config.

pub mod menu;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{
    Config, FONT_SIZE_RANGE, Keyboard, LINES_RANGE, MinWpm, Pace, PbEffect, WORDS_PER_LINE_RANGE,
};
use crate::test::mode::Mode;

pub use menu::{Editor, ProfileMenu};

/// Results-screen sections a profile can set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResultsSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_breakdown: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consistency: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<bool>,
}

impl ResultsSettings {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The settings a profile controls: `Config` with every field optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProfileSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub punctuation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words_per_line: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fullscreen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "celebrate")]
    pub pb_effect: Option<PbEffect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pace: Option<Pace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sudden_death: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_wpm: Option<MinWpm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<Keyboard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_syntax: Option<bool>,
    #[serde(skip_serializing_if = "ResultsSettings::is_empty")]
    pub results: ResultsSettings,
}

/// How a setting value reads in the profile menu.
trait Show {
    fn show(&self) -> String;
}

impl Show for String {
    fn show(&self) -> String {
        if self.is_empty() {
            "system".into()
        } else {
            self.clone()
        }
    }
}

impl Show for bool {
    fn show(&self) -> String {
        if *self { "on" } else { "off" }.into()
    }
}

impl Show for u8 {
    fn show(&self) -> String {
        self.to_string()
    }
}

impl Show for Pace {
    fn show(&self) -> String {
        self.label()
    }
}

impl Show for MinWpm {
    fn show(&self) -> String {
        self.label()
    }
}

impl Show for Keyboard {
    fn show(&self) -> String {
        self.label().into()
    }
}

impl Show for PbEffect {
    fn show(&self) -> String {
        self.label().into()
    }
}

impl Show for Mode {
    fn show(&self) -> String {
        self.label()
    }
}

/// Declares every profile-able setting once: its key, its label, and the
/// field path shared by `Config` and `ProfileSettings`. Adding a setting to
/// `Config` means adding a field to `ProfileSettings` and a line here.
macro_rules! settings {
    ($($key:ident, $label:literal => $($field:ident).+;)*) => {
        /// One modifiable setting.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum SettingKey {
            $($key),*
        }

        impl SettingKey {
            /// Every setting, in menu order.
            pub const ALL: &[SettingKey] = &[$(SettingKey::$key),*];

            pub fn label(self) -> &'static str {
                match self {
                    $(SettingKey::$key => $label),*
                }
            }

            /// The live value, formatted for display.
            pub fn show(self, config: &Config) -> String {
                match self {
                    $(SettingKey::$key => config.$($field).+.show()),*
                }
            }

            /// Whether the two configs disagree on this setting.
            pub fn differs(self, a: &Config, b: &Config) -> bool {
                match self {
                    $(SettingKey::$key => a.$($field).+ != b.$($field).+),*
                }
            }
        }

        impl ProfileSettings {
            pub fn is_set(&self, key: SettingKey) -> bool {
                match key {
                    $(SettingKey::$key => self.$($field).+.is_some()),*
                }
            }

            pub fn clear(&mut self, key: SettingKey) {
                match key {
                    $(SettingKey::$key => self.$($field).+ = None),*
                }
            }

            /// Copy `key`'s live value from `config` into the profile.
            pub fn capture(&mut self, key: SettingKey, config: &Config) {
                match key {
                    $(SettingKey::$key => self.$($field).+ = Some(config.$($field).+.clone())),*
                }
            }

            /// Copy `key`'s value (or its absence) from another profile.
            pub fn copy_from(&mut self, key: SettingKey, other: &ProfileSettings) {
                match key {
                    $(SettingKey::$key => self.$($field).+ = other.$($field).+.clone()),*
                }
            }

            /// The profile's value for `key`, formatted for display.
            pub fn show(&self, key: SettingKey) -> Option<String> {
                match key {
                    $(SettingKey::$key => self.$($field).+.as_ref().map(Show::show)),*
                }
            }

            /// Overwrite the settings this profile controls.
            pub fn apply_to(&self, config: &mut Config) {
                $(
                    if let Some(v) = &self.$($field).+ {
                        config.$($field).+ = v.clone();
                    }
                )*
            }

            /// Whether every setting this profile controls has its value in `config`.
            pub fn matches(&self, config: &Config) -> bool {
                $(self.$($field).+.as_ref().is_none_or(|v| *v == config.$($field).+))&&*
            }
        }
    };
}

settings! {
    Mode, "mode" => mode;
    Language, "language" => language;
    Punctuation, "punctuation" => punctuation;
    Numbers, "numbers" => numbers;
    Theme, "theme" => theme;
    FontSize, "font size" => font_size;
    WordsPerLine, "words per line" => words_per_line;
    Lines, "lines" => lines;
    Zen, "zen" => zen;
    Fullscreen, "fullscreen" => fullscreen;
    Font, "font" => font;
    Pace, "pace" => pace;
    SuddenDeath, "sudden death" => sudden_death;
    MinWpm, "min wpm" => min_wpm;
    PbEffect, "pb effect" => pb_effect;
    Keyboard, "keyboard" => keyboard;
    TrimSyntax, "trim syntax" => trim_syntax;
    ResultsChart, "results chart" => results.chart;
    ResultsBreakdown, "results breakdown" => results.char_breakdown;
    ResultsConsistency, "results consistency" => results.consistency;
    ResultsRaw, "results raw" => results.raw;
    ResultsKeys, "results keys" => results.keys;
}

impl SettingKey {
    /// Changing this setting invalidates the running test.
    pub fn restarts_test(self) -> bool {
        matches!(
            self,
            SettingKey::Mode
                | SettingKey::Language
                | SettingKey::Punctuation
                | SettingKey::Numbers
                | SettingKey::TrimSyntax
        )
    }

    /// Changing this setting needs the graphics backend reconfigured.
    pub fn affects_graphics(self) -> bool {
        matches!(self, SettingKey::Font)
    }
}

impl fmt::Display for SettingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl ProfileSettings {
    pub fn parse(text: &str) -> Result<Self, String> {
        let s: Self = toml::from_str(text).map_err(|e| e.message().to_string())?;
        s.validate()?;
        Ok(s)
    }

    /// The settings this profile controls, in menu order.
    pub fn keys(&self) -> Vec<SettingKey> {
        SettingKey::ALL
            .iter()
            .copied()
            .filter(|k| self.is_set(*k))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.keys().is_empty()
    }

    /// Whether both profiles control at least one common setting.
    pub fn overlaps(&self, other: &ProfileSettings) -> bool {
        SettingKey::ALL
            .iter()
            .any(|k| self.is_set(*k) && other.is_set(*k))
    }

    /// Reject values `Config` would clamp or can't run with, so a profile
    /// that loads is one that still matches the config after applying it.
    pub fn validate(&self) -> Result<(), String> {
        let in_range = |v: Option<u8>, (lo, hi): (u8, u8), what: &str| match v {
            Some(n) if n < lo || n > hi => Err(format!("{what} must be between {lo} and {hi}")),
            _ => Ok(()),
        };
        in_range(self.font_size, FONT_SIZE_RANGE, "font_size")?;
        in_range(self.words_per_line, WORDS_PER_LINE_RANGE, "words_per_line")?;
        in_range(self.lines, LINES_RANGE, "lines")?;
        if self.mode.is_some_and(|m| m.value() == 0) {
            return Err("mode needs a value above 0".into());
        }
        if self.theme.as_deref() == Some("") || self.language.as_deref() == Some("") {
            return Err("theme and language can't be empty".into());
        }
        Ok(())
    }

    /// `key = value` pairs, e.g. `mode time 15 · theme nord`.
    pub fn summary(&self) -> String {
        self.keys()
            .into_iter()
            .map(|k| format!("{k} {}", self.show(k).unwrap_or_default()))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub settings: ProfileSettings,
}

/// Profile names double as file names, so keep them to a safe charset.
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("config needs a name".into());
    }
    if name.len() > 32 {
        return Err("config name is too long (32 max)".into());
    }
    if !name.chars().all(is_name_char) {
        return Err("config names use letters, digits, - and _".into());
    }
    Ok(())
}

pub fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// All profiles on disk, keyed by name.
#[derive(Debug, Clone, Default)]
pub struct ProfileRegistry {
    dir: PathBuf,
    profiles: BTreeMap<String, Profile>,
}

impl ProfileRegistry {
    /// Every `*.toml` in `dir`. Bad files are reported through `warn` and
    /// skipped, so one typo can't break startup.
    pub fn load(dir: &Path, mut warn: impl FnMut(String)) -> Self {
        let mut reg = Self {
            dir: dir.to_path_buf(),
            profiles: BTreeMap::new(),
        };
        let Ok(entries) = fs::read_dir(dir) else {
            return reg;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Err(e) = validate_name(name) {
                warn(format!("{}: {e}", path.display()));
                continue;
            }
            let parsed = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| ProfileSettings::parse(&text));
            match parsed {
                Ok(settings) => {
                    reg.profiles.insert(
                        name.to_string(),
                        Profile {
                            name: name.to_string(),
                            settings,
                        },
                    );
                }
                Err(e) => warn(format!("config {name}: {e}")),
            }
        }
        reg
    }

    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.profiles.keys().map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Profile> {
        self.profiles.values()
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.toml"))
    }

    /// Write `profile` to disk and the registry. With `replacing`, the
    /// profile of that name is removed first (a rename).
    pub fn save(&mut self, profile: Profile, replacing: Option<&str>) -> Result<(), String> {
        validate_name(&profile.name)?;
        profile.settings.validate()?;
        if profile.settings.is_empty() {
            return Err("pick at least one setting".into());
        }
        if replacing != Some(profile.name.as_str()) && self.profiles.contains_key(&profile.name) {
            return Err(format!("config `{}` already exists", profile.name));
        }
        let text = toml::to_string_pretty(&profile.settings).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let path = self.path(&profile.name);
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(old) = replacing.filter(|old| *old != profile.name) {
            self.delete(old)?;
        }
        self.profiles.insert(profile.name.clone(), profile);
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        let path = self.path(name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
        self.profiles.remove(name);
        Ok(())
    }

    /// Active profiles `profile` would replace: those sharing a setting.
    pub fn conflicts<'a>(&self, profile: &Profile, active: &'a [String]) -> Vec<&'a str> {
        active
            .iter()
            .map(String::as_str)
            .filter(|n| *n != profile.name)
            .filter(|n| {
                self.get(n)
                    .is_some_and(|p| p.settings.overlaps(&profile.settings))
            })
            .collect()
    }

    /// Apply `name` to `config` and mark it active, deactivating every active
    /// profile that shares a setting with it. Returns the replaced profiles.
    pub fn activate(&self, name: &str, config: &mut Config) -> Result<Vec<String>, String> {
        let profile = self
            .get(name)
            .ok_or_else(|| format!("unknown config `{name}`"))?;
        let replaced: Vec<String> = self
            .conflicts(profile, &config.profiles)
            .into_iter()
            .map(str::to_string)
            .collect();
        config
            .profiles
            .retain(|n| n != name && !replaced.contains(n));
        profile.settings.apply_to(config);
        config.profiles.push(name.to_string());
        Ok(replaced)
    }

    /// Drop active profiles that no longer exist or no longer match the
    /// config (a setting they control was changed). Returns the dropped names.
    pub fn reconcile(&self, config: &mut Config) -> Vec<String> {
        let mut dropped = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        let mut keep = Vec::new();
        for name in &config.profiles {
            let ok = !seen.contains(&name.as_str())
                && self.get(name).is_some_and(|p| p.settings.matches(config));
            if ok {
                seen.push(name);
                keep.push(name.clone());
            } else if !seen.contains(&name.as_str()) {
                dropped.push(name.clone());
            }
        }
        config.profiles = keep;
        dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, text: &str) -> Profile {
        Profile {
            name: name.into(),
            settings: ProfileSettings::parse(text).unwrap(),
        }
    }

    fn registry(profiles: &[Profile]) -> ProfileRegistry {
        let mut reg = ProfileRegistry::default();
        for p in profiles {
            reg.profiles.insert(p.name.clone(), p.clone());
        }
        reg
    }

    #[test]
    fn parses_subset_of_config() {
        let p = profile(
            "sprint",
            "mode = { time = 15 }\npunctuation = true\n[results]\nchart = false\n",
        );
        assert_eq!(
            p.settings.keys(),
            vec![
                SettingKey::Mode,
                SettingKey::Punctuation,
                SettingKey::ResultsChart
            ]
        );
        assert_eq!(
            p.settings.summary(),
            "mode time 15 · punctuation on · results chart off"
        );
    }

    #[test]
    fn rejects_unknown_keys_and_bad_values() {
        assert!(ProfileSettings::parse("them = \"nord\"").is_err());
        assert!(ProfileSettings::parse("font_size = 99").is_err());
        assert!(ProfileSettings::parse("lines = 0").is_err());
        assert!(ProfileSettings::parse("words_per_line = 2").is_err());
        assert!(ProfileSettings::parse("mode = { time = 0 }").is_err());
        assert!(ProfileSettings::parse("[results]\nbogus = true").is_err());
        assert!(ProfileSettings::parse("").unwrap().is_empty());
    }

    #[test]
    fn every_key_roundtrips_through_capture_and_toml() {
        let config = Config {
            theme: "nord".into(),
            mode: Mode::Words(25),
            zen: true,
            font: "Fira Code".into(),
            ..Config::default()
        };
        let mut s = ProfileSettings::default();
        for k in SettingKey::ALL {
            assert!(!s.is_set(*k));
            s.capture(*k, &config);
            assert!(s.is_set(*k));
            assert_eq!(s.show(*k), Some(k.show(&config)));
        }
        assert_eq!(s.keys(), SettingKey::ALL);
        let text = toml::to_string_pretty(&s).unwrap();
        assert_eq!(ProfileSettings::parse(&text).unwrap(), s);

        let mut fresh = Config::default();
        assert!(!s.matches(&fresh));
        s.apply_to(&mut fresh);
        assert!(s.matches(&fresh));
        fresh.profiles = config.profiles.clone();
        assert_eq!(fresh, config);

        for k in SettingKey::ALL {
            s.clear(*k);
        }
        assert!(s.is_empty());
        assert_eq!(toml::to_string(&s).unwrap(), "");
    }

    #[test]
    fn apply_only_touches_listed_settings() {
        let p = profile("dark", "theme = \"nord\"");
        let mut c = Config {
            punctuation: true,
            ..Config::default()
        };
        p.settings.apply_to(&mut c);
        assert_eq!(c.theme, "nord");
        assert!(c.punctuation);
    }

    #[test]
    fn activating_replaces_overlapping_profiles_only() {
        let reg = registry(&[
            profile("sprint", "mode = { time = 15 }\npunctuation = true"),
            profile("marathon", "mode = { time = 120 }"),
            profile("dark", "theme = \"nord\""),
            profile("light", "theme = \"light\"\nzen = true"),
        ]);
        let mut c = Config::default();
        assert!(reg.activate("sprint", &mut c).unwrap().is_empty());
        assert!(reg.activate("dark", &mut c).unwrap().is_empty());
        assert_eq!(c.profiles, vec!["sprint", "dark"]);
        assert_eq!(c.mode, Mode::Time(15));

        // marathon shares `mode` with sprint: sprint goes, dark stays.
        assert_eq!(reg.activate("marathon", &mut c).unwrap(), vec!["sprint"]);
        assert_eq!(c.profiles, vec!["dark", "marathon"]);
        assert_eq!(c.mode, Mode::Time(120));
        // sprint's other setting is left as it was.
        assert!(c.punctuation);

        assert_eq!(reg.activate("light", &mut c).unwrap(), vec!["dark"]);
        assert_eq!(c.profiles, vec!["marathon", "light"]);
        assert_eq!((c.theme.as_str(), c.zen), ("light", true));

        // Re-activating is idempotent.
        assert!(reg.activate("light", &mut c).unwrap().is_empty());
        assert_eq!(c.profiles, vec!["marathon", "light"]);
        assert!(reg.activate("nope", &mut c).is_err());
        assert!(reg.reconcile(&mut c).is_empty());
    }

    #[test]
    fn manual_change_deselects_profiles_that_set_it() {
        let reg = registry(&[
            profile("sprint", "mode = { time = 15 }"),
            profile("dark", "theme = \"nord\""),
        ]);
        let mut c = Config::default();
        reg.activate("sprint", &mut c).unwrap();
        reg.activate("dark", &mut c).unwrap();
        c.theme = "gruvbox".into();
        assert_eq!(reg.reconcile(&mut c), vec!["dark"]);
        assert_eq!(c.profiles, vec!["sprint"]);
        // Unknown and duplicate names are cleaned up too.
        c.profiles = vec!["sprint".into(), "gone".into(), "sprint".into()];
        assert_eq!(reg.reconcile(&mut c), vec!["gone"]);
        assert_eq!(c.profiles, vec!["sprint"]);
    }

    #[test]
    fn registry_saves_renames_and_deletes() {
        let dir = std::env::temp_dir().join(format!("ttyp-profiles-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut reg = ProfileRegistry::load(&dir, |w| panic!("{w}"));
        assert!(reg.is_empty());

        let p = profile("fast", "mode = { words = 10 }");
        reg.save(p.clone(), None).unwrap();
        assert!(reg.save(p.clone(), None).unwrap_err().contains("exists"));
        let empty = Profile {
            name: "empty".into(),
            settings: ProfileSettings::default(),
        };
        assert!(reg.save(empty, None).is_err());
        let bad = Profile {
            name: "../x".into(),
            ..p.clone()
        };
        assert!(reg.save(bad, None).is_err());

        let renamed = Profile {
            name: "faster".into(),
            ..p.clone()
        };
        reg.save(renamed, Some("fast")).unwrap();
        let reloaded = ProfileRegistry::load(&dir, |w| panic!("{w}"));
        assert_eq!(reloaded.names().collect::<Vec<_>>(), vec!["faster"]);
        assert_eq!(reloaded.get("faster").unwrap().settings, p.settings);

        fs::write(dir.join("broken.toml"), "theme = 3").unwrap();
        let mut warnings = Vec::new();
        let reloaded = ProfileRegistry::load(&dir, |w| warnings.push(w));
        assert_eq!(reloaded.len(), 1);
        assert_eq!(warnings.len(), 1);

        reg.delete("faster").unwrap();
        assert!(reg.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}

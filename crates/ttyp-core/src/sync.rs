//! Account sync: the configs (profiles) and personal bests a logged-in
//! player keeps on the server, so every machine they log in on has the
//! same ones. The shapes, limits and the merge live here so the client and
//! the server agree on them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::test::mode::Mode;

/// Most configs one account keeps.
pub const MAX_CONFIGS: usize = 100;
/// Longest config file, in bytes.
pub const MAX_CONFIG_BYTES: usize = 8 * 1024;
/// Most bests one account keeps (one per mode and language).
pub const MAX_BESTS: usize = 2000;
/// Longest language key a best is kept for (`code_python+numpy`, `custom:<name>`).
pub const MAX_LANGUAGE_CHARS: usize = 128;
/// Fastest wpm a best may claim.
pub const MAX_WPM: f64 = 500.0;

/// A player's configs: every config file and which ones are switched on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configs {
    /// Config name → its TOML file, as written. Kept as text so a newer
    /// client's settings pass through an older one untouched.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// The active configs, in the order they were switched on.
    #[serde(default)]
    pub active: Vec<String>,
}

/// A config name: 1–32 letters, digits, `-` and `_` (they're file names).
pub fn valid_config_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Configs {
    /// Why the server won't keep these, if it won't.
    pub fn problem(&self) -> Option<String> {
        if self.files.len() > MAX_CONFIGS {
            return Some(format!("at most {MAX_CONFIGS} configs"));
        }
        for (name, text) in &self.files {
            if !valid_config_name(name) {
                return Some(format!("bad config name `{name}`"));
            }
            if text.len() > MAX_CONFIG_BYTES {
                return Some(format!("config {name} is too big"));
            }
        }
        if let Some(name) = self.active.iter().find(|n| !self.files.contains_key(*n)) {
            return Some(format!("active config `{name}` doesn't exist"));
        }
        None
    }

    /// Three-way merge of this machine's configs (`local`) with the
    /// server's (`remote`), from the state both last agreed on (`base`).
    /// A config takes whichever side changed it, this machine winning when
    /// both did (a deletion counts as a change); the active list likewise.
    ///
    /// With no `base` (this machine's first sync), nothing is lost: the
    /// result is every config from both sides, the server's copy when both
    /// have one, and the server's active list unless it has no configs.
    pub fn merge(base: Option<&Configs>, local: &Configs, remote: &Configs) -> Configs {
        let names: Vec<&String> = local.files.keys().chain(remote.files.keys()).collect();
        let mut files = BTreeMap::new();
        for name in names {
            let l = local.files.get(name);
            let r = remote.files.get(name);
            let pick = match base {
                Some(b) if l == b.files.get(name) => r,
                Some(_) => l,
                None => r.or(l),
            };
            if let Some(text) = pick {
                files.insert(name.clone(), text.clone());
            }
        }
        let active = match base {
            Some(b) if local.active == b.active => &remote.active,
            Some(_) => &local.active,
            None if remote.files.is_empty() => &local.active,
            None => &remote.active,
        };
        let mut seen = Vec::new();
        for name in active {
            if files.contains_key(name) && !seen.contains(name) {
                seen.push(name.clone());
            }
        }
        Configs {
            files,
            active: seen,
        }
    }
}

/// `GET /sync/configs` and the reply to `POST /sync/configs`: the server's
/// configs and their revision (0 before the first push).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSync {
    pub rev: i64,
    pub configs: Configs,
}

/// `POST /sync/configs`: replace the server's configs, if they're still at
/// `base_rev`. Otherwise the reply is the newer state, to merge and retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigPush {
    pub base_rev: i64,
    pub configs: Configs,
}

/// A personal best: the fastest counted run on a mode in a language
/// (`language` is the run's language key). `GET /sync/bests`; `POST
/// /sync/bests` takes a list of these, keeps the faster of each and answers
/// with them all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Best {
    pub mode: Mode,
    pub language: String,
    pub wpm: f64,
}

impl Best {
    /// Why the server won't keep this, if it won't.
    pub fn problem(&self) -> Option<&'static str> {
        if self.mode.value() == 0 {
            Some("mode needs a value above 0")
        } else if self.language.is_empty() || self.language.chars().count() > MAX_LANGUAGE_CHARS {
            Some("bad language")
        } else if !(self.wpm.is_finite() && self.wpm > 0.0 && self.wpm <= MAX_WPM) {
            Some("bad wpm")
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configs(files: &[(&str, &str)], active: &[&str]) -> Configs {
        Configs {
            files: files
                .iter()
                .map(|(n, t)| (n.to_string(), t.to_string()))
                .collect(),
            active: active.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn each_side_keeps_what_it_changed() {
        let base = configs(&[("a", "1"), ("b", "1"), ("c", "1")], &["a"]);
        // Here: edited a, deleted b. There: edited c, added d, switched to c.
        let local = configs(&[("a", "2"), ("c", "1")], &["a"]);
        let remote = configs(&[("a", "1"), ("b", "1"), ("c", "3"), ("d", "1")], &["c"]);
        let m = Configs::merge(Some(&base), &local, &remote);
        assert_eq!(m, configs(&[("a", "2"), ("c", "3"), ("d", "1")], &["c"]));
    }

    #[test]
    fn this_machine_wins_a_clash() {
        let base = configs(&[("a", "1")], &[]);
        let local = configs(&[("a", "2")], &["a"]);
        let remote = configs(&[], &[]);
        let m = Configs::merge(Some(&base), &local, &remote);
        assert_eq!(m, local);
        // Deleted here, untouched there: gone.
        let local = configs(&[], &[]);
        let remote = configs(&[("a", "1")], &["a"]);
        let m = Configs::merge(Some(&base), &local, &remote);
        assert_eq!(m, configs(&[], &[]), "the active name goes with it");
    }

    #[test]
    fn first_sync_loses_nothing() {
        let local = configs(&[("a", "local"), ("mine", "1")], &["mine"]);
        let remote = configs(&[("a", "remote"), ("theirs", "1")], &["theirs"]);
        let m = Configs::merge(None, &local, &remote);
        assert_eq!(
            m,
            configs(
                &[("a", "remote"), ("mine", "1"), ("theirs", "1")],
                &["theirs"]
            )
        );
        // An empty account takes this machine's configs as they are.
        assert_eq!(Configs::merge(None, &local, &Configs::default()), local);
    }

    #[test]
    fn limits() {
        assert!(configs(&[("a", "x")], &["a"]).problem().is_none());
        assert!(configs(&[("a", "x")], &["b"]).problem().is_some());
        assert!(configs(&[("a b", "x")], &[]).problem().is_some());
        assert!(
            configs(&[("a", &"x".repeat(MAX_CONFIG_BYTES + 1))], &[])
                .problem()
                .is_some()
        );
        let best = |wpm| Best {
            mode: Mode::Time(30),
            language: "english".into(),
            wpm,
        };
        assert!(best(120.0).problem().is_none());
        assert!(best(0.0).problem().is_some());
        assert!(best(f64::NAN).problem().is_some());
        assert!(best(900.0).problem().is_some());
    }
}

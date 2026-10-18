//! Account sync on this machine: reading and writing the config files as
//! `sync::Configs`, the state last agreed with the server (`Paths::sync_file`)
//! and the account's bests from every machine (`Paths::bests_file`). The
//! merge itself is `ttyp_core::sync::Configs::merge`; `app/sync.rs` drives it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ttyp_core::sync::{Best, Configs, valid_config_name};
use ttyp_core::test::Mode;

use crate::stats::TestRecord;

/// The configs this machine and the server last agreed on, at `rev`: the
/// base of the next merge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Synced {
    pub rev: i64,
    pub configs: Configs,
}

impl Synced {
    /// `None` before the first sync (or after logging out).
    /// A bad file is logged and moved aside: the next sync is then a
    /// first sync (a union with the server's configs), which loses nothing.
    pub fn load(path: &Path) -> Option<Self> {
        crate::config::load_state("sync", path, |t| serde_json::from_str(t))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        write(
            path,
            &serde_json::to_string(self).map_err(|e| e.to_string())?,
        )
    }
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    crate::config::write_atomic(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Remove `path`; a file that isn't there is fine.
pub fn forget(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// The config files in `dir` as they are on disk (even ones this version
/// can't parse, so they pass through), and the active list. A file that
/// can't be read at all is an error: left out, it would look deleted and
/// the deletion would be pushed to every machine.
pub fn local_configs(dir: &Path, active: &[String]) -> Result<Configs, String> {
    let mut files = BTreeMap::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => Some(entries),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    for entry in entries.into_iter().flatten() {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if valid_config_name(name) {
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            files.insert(name.to_string(), text);
        }
    }
    Ok(Configs {
        files,
        active: active.to_vec(),
    })
}

/// Make the files in `dir` those of `configs`: write the new and changed
/// ones, delete the ones it doesn't have. The active list is the caller's.
pub fn write_configs(dir: &Path, from: &Configs, to: &Configs) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = |name: &str| -> PathBuf { dir.join(format!("{name}.toml")) };
    for (name, text) in &to.files {
        if from.files.get(name) != Some(text) {
            write(&path(name), text)?;
        }
    }
    for name in from.files.keys().filter(|n| !to.files.contains_key(*n)) {
        forget(&path(name))?;
    }
    Ok(())
}

/// The account's bests from every machine, as last fetched; kept on disk
/// so they count offline too.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bests {
    list: Vec<Best>,
    path: Option<PathBuf>,
}

impl Bests {
    pub fn load(path: &Path) -> Self {
        // A bad cache is logged and moved aside; the next fetch refills it.
        let list = crate::config::load_state("sync", path, |t| serde_json::from_str(t))
            .unwrap_or_default();
        Self {
            list,
            path: Some(path.to_path_buf()),
        }
    }

    pub fn list(&self) -> &[Best] {
        &self.list
    }

    /// Replace them with the server's and save them.
    pub fn set(&mut self, list: Vec<Best>) -> Result<(), String> {
        self.list = list;
        match &self.path {
            Some(p) => write(
                p,
                &serde_json::to_string(&self.list).map_err(|e| e.to_string())?,
            ),
            None => Ok(()),
        }
    }

    /// Forget them (logging out), on disk too.
    pub fn clear(&mut self) -> Result<(), String> {
        self.list.clear();
        self.path.as_deref().map_or(Ok(()), forget)
    }

    pub fn get(&self, mode: Mode, language: &str) -> Option<f64> {
        self.list
            .iter()
            .find(|b| b.mode == mode && b.language == language)
            .map(|b| b.wpm)
    }

    /// The best wpm on `mode` in any language (the stats screen's bests).
    pub fn by_mode(&self) -> impl Iterator<Item = (Mode, f64)> + '_ {
        self.list.iter().map(|b| (b.mode, b.wpm))
    }

    /// This machine's bests (from `records`) the server doesn't have yet.
    pub fn missing(&self, records: &[TestRecord]) -> Vec<Best> {
        let mut local: BTreeMap<(&str, u16, &str), Best> = BTreeMap::new();
        for r in records.iter().filter(|r| r.counts() && r.wpm > 0.0) {
            let key = (r.mode.kind(), r.mode.value(), r.language.as_str());
            let e = local.entry(key).or_insert_with(|| Best {
                mode: r.mode,
                language: r.language.clone(),
                wpm: 0.0,
            });
            e.wpm = e.wpm.max(r.wpm);
        }
        local
            .into_values()
            .filter(|b| b.problem().is_none())
            .filter(|b| self.get(b.mode, &b.language).is_none_or(|w| b.wpm > w))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ttyp-sync-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn configs_round_trip_through_files() {
        let d = dir("files");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("a.toml"), "zen = true\n").unwrap();
        fs::write(d.join("b.toml"), "lines = 3\n").unwrap();
        fs::write(d.join("not a name.toml"), "x").unwrap();
        fs::write(d.join("notes.txt"), "x").unwrap();
        let local = local_configs(&d, &["a".into()]).unwrap();
        assert_eq!(local.files.keys().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(local.active, ["a"]);

        let mut to = local.clone();
        to.files.remove("b");
        to.files.insert("a".into(), "zen = false\n".into());
        to.files.insert("c".into(), "future_setting = 1\n".into());
        write_configs(&d, &local, &to).unwrap();
        assert_eq!(local_configs(&d, &[]).unwrap().files, to.files);

        // A config that can't be read stops the sync rather than reading
        // as deleted.
        fs::write(d.join("broken.toml"), [0xff, 0xfe]).unwrap();
        let e = local_configs(&d, &[]).unwrap_err();
        assert!(e.contains("broken.toml"), "{e}");
        fs::remove_dir_all(&d).unwrap();
        assert!(
            local_configs(&d, &[]).unwrap().files.is_empty(),
            "no dir yet"
        );
    }

    #[test]
    fn bests_send_only_what_the_server_lacks() {
        let d = dir("bests");
        let path = d.join("bests.json");
        let mut bests = Bests::load(&path);
        let best = |mode, wpm| Best {
            mode,
            language: "english".into(),
            wpm,
        };
        bests
            .set(vec![
                best(Mode::Time(30), 100.0),
                best(Mode::Words(10), 50.0),
            ])
            .unwrap();
        assert_eq!(Bests::load(&path), bests, "kept on disk");
        assert_eq!(bests.get(Mode::Time(30), "english"), Some(100.0));
        assert_eq!(bests.get(Mode::Time(30), "german"), None);

        let rec = |mode, wpm, invalid: bool| {
            let mut r: TestRecord = serde_json::from_value(serde_json::json!({
                "schema": 4, "ts": "2026-01-01T00:00:00Z", "mode": mode,
                "language": "english", "punctuation": false, "numbers": false,
                "wpm": wpm, "raw": wpm, "acc": 99.0, "consistency": 80.0,
                "chars": {"correct": 1, "incorrect": 0, "extra": 0, "missed": 0},
                "duration_s": 30.0
            }))
            .unwrap();
            if invalid {
                r.invalid = Some(crate::stats::Invalid::Afk);
            }
            r
        };
        let records = [
            rec(Mode::Time(30), 90.0, false),
            rec(Mode::Words(10), 60.0, false),
            rec(Mode::Words(10), 55.0, false),
            rec(Mode::Time(15), 300.0, true),
            rec(Mode::Time(60), 70.0, false),
        ];
        assert_eq!(
            bests.missing(&records),
            [best(Mode::Time(60), 70.0), best(Mode::Words(10), 60.0)]
        );
        bests.clear().unwrap();
        assert!(!path.exists());
        fs::remove_dir_all(&d).unwrap();
    }
}

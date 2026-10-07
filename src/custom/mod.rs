//! Custom word sets on this machine: ones you made and ones you installed
//! from the server, one TOML file each in the config dir's `custom/`. The
//! `custom` setting names the set to type instead of the language. The
//! rules for a valid set are `ttyp_core::custom`'s; `menu.rs` holds the
//! custom page and the editor's state.

pub mod menu;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use ttyp_core::custom::{Addition, addition, check, valid_name};

/// One set, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomSet {
    pub name: String,
    /// Who published it, when installed from the server; `None` for sets
    /// made here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub words: Vec<String>,
}

impl CustomSet {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            author: None,
            words: Vec::new(),
        }
    }

    /// Made here, so it can be published under our name.
    pub fn is_mine(&self) -> bool {
        self.author.is_none()
    }
}

/// Every set in the `custom/` dir, by name.
#[derive(Debug, Default)]
pub struct CustomStore {
    dir: PathBuf,
    sets: BTreeMap<String, CustomSet>,
}

impl CustomStore {
    /// Read every `*.toml` in `dir`; bad files are reported to `warn` and
    /// skipped.
    pub fn load(dir: &Path, mut warn: impl FnMut(String)) -> Self {
        let mut sets = BTreeMap::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "toml") {
                    continue;
                }
                let parsed = fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| toml::from_str::<CustomSet>(&t).map_err(|e| e.to_string()));
                match parsed {
                    Ok(set) if valid_name(&set.name) => {
                        sets.insert(set.name.clone(), set);
                    }
                    Ok(set) => warn(format!("custom set `{}` has a bad name, skipped", set.name)),
                    Err(e) => warn(format!("skipped {}: {e}", path.display())),
                }
            }
        }
        Self {
            dir: dir.to_path_buf(),
            sets,
        }
    }

    pub fn get(&self, name: &str) -> Option<&CustomSet> {
        self.sets.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.sets.contains_key(name)
    }

    /// Every set, by name.
    pub fn all(&self) -> impl Iterator<Item = &CustomSet> {
        self.sets.values()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.sets.keys().map(String::as_str)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.toml"))
    }

    /// Write `set` to disk and keep it.
    pub fn save(&mut self, set: CustomSet) -> io::Result<()> {
        if !valid_name(&set.name) {
            return Err(io::Error::other(format!("bad name `{}`", set.name)));
        }
        fs::create_dir_all(&self.dir)?;
        let text = toml::to_string(&set).map_err(io::Error::other)?;
        fs::write(self.path(&set.name), text)?;
        self.sets.insert(set.name.clone(), set);
        Ok(())
    }

    /// Delete `name`'s file.
    pub fn remove(&mut self, name: &str) -> io::Result<()> {
        if self.sets.remove(name).is_some() {
            match fs::remove_file(self.path(name)) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_are_saved_loaded_and_removed() {
        let dir = std::env::temp_dir().join(format!("ttyp-custom-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut store = CustomStore::load(&dir, |w| panic!("{w}"));
        assert_eq!(store.all().count(), 0);
        let mut set = CustomSet::new("birds");
        set.words = vec!["owl".into(), "wren".into()];
        store.save(set.clone()).unwrap();
        let mut theirs = CustomSet::new("rust");
        theirs.author = Some("ann".into());
        theirs.words = vec!["fn".into()];
        store.save(theirs).unwrap();
        assert!(store.save(CustomSet::new("Bad Name")).is_err());
        fs::write(dir.join("junk.toml"), "not = [valid").unwrap();

        let mut warnings = Vec::new();
        let mut again = CustomStore::load(&dir, |w| warnings.push(w));
        assert_eq!(warnings.len(), 1, "the junk file");
        assert_eq!(again.get("birds"), Some(&set));
        assert!(again.get("birds").unwrap().is_mine());
        assert!(!again.get("rust").unwrap().is_mine());
        assert_eq!(again.names().collect::<Vec<_>>(), ["birds", "rust"]);
        again.remove("birds").unwrap();
        assert!(!dir.join("birds.toml").exists());
        assert!(!again.contains("birds"));
        let _ = fs::remove_dir_all(dir);
    }
}

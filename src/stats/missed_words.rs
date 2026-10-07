//! Words you got wrong, per language (or custom set): a word counts as
//! missed when any key typed in it was wrong, even if you fixed it. Each
//! language keeps its `MAX_WORDS` most-missed words (ties: most recent),
//! and `:pm` practises the top ones. Stored as JSON in the data dir.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Words kept per language.
pub const MAX_WORDS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub misses: u32,
    /// Unix seconds of the latest miss.
    pub last: i64,
}

#[derive(Debug, Default)]
pub struct MissedWords {
    /// Language key → word → entry.
    languages: BTreeMap<String, BTreeMap<String, Entry>>,
    path: Option<PathBuf>,
}

impl MissedWords {
    /// From `path`; a missing or unreadable file is an empty record.
    pub fn load(path: &Path) -> Self {
        let languages = fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            languages,
            path: Some(path.to_path_buf()),
        }
    }

    /// Count each of `words` as missed once in `language` at `now`, then
    /// drop the least-missed beyond `MAX_WORDS`.
    pub fn record<'a>(
        &mut self,
        language: &str,
        words: impl IntoIterator<Item = &'a str>,
        now: i64,
    ) {
        let table = self.languages.entry(language.to_string()).or_default();
        let mut any = false;
        for w in words {
            let e = table.entry(w.to_string()).or_insert(Entry {
                misses: 0,
                last: now,
            });
            e.misses += 1;
            e.last = now;
            any = true;
        }
        if table.len() > MAX_WORDS {
            let mut ranked: Vec<(String, Entry)> = std::mem::take(table).into_iter().collect();
            ranked.sort_by(|a, b| rank(&a.1, &b.1));
            ranked.truncate(MAX_WORDS);
            *table = ranked.into_iter().collect();
        }
        if !any && table.is_empty() {
            self.languages.remove(language);
        }
    }

    /// The `n` most-missed words of `language`, most missed first.
    pub fn top(&self, language: &str, n: usize) -> Vec<(String, Entry)> {
        let Some(table) = self.languages.get(language) else {
            return Vec::new();
        };
        let mut v: Vec<(String, Entry)> = table.iter().map(|(w, e)| (w.clone(), *e)).collect();
        v.sort_by(|a, b| rank(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }

    /// How many words `language` has on record.
    pub fn count(&self, language: &str) -> usize {
        self.languages.get(language).map_or(0, BTreeMap::len)
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string(&self.languages).map_err(io::Error::other)?;
        fs::write(path, json)
    }
}

/// Most misses first, then the most recent.
fn rank(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    b.misses.cmp(&a.misses).then(b.last.cmp(&a.last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_counted_per_language_and_ranked() {
        let mut m = MissedWords::default();
        m.record("english", ["the", "their"], 10);
        m.record("english", ["their"], 20);
        m.record("english", ["which"], 30);
        m.record("code_rust", ["impl"], 5);
        let top: Vec<String> = m.top("english", 10).into_iter().map(|(w, _)| w).collect();
        assert_eq!(
            top,
            ["their", "which", "the"],
            "most missed, then most recent"
        );
        assert_eq!(m.top("english", 1)[0].1.misses, 2);
        assert_eq!(m.count("code_rust"), 1);
        assert!(m.top("french", 5).is_empty());
    }

    #[test]
    fn each_language_keeps_its_most_missed() {
        let mut m = MissedWords::default();
        let keep: Vec<String> = (0..MAX_WORDS).map(|i| format!("k{i}")).collect();
        m.record("en", keep.iter().map(String::as_str), 1);
        m.record("en", keep.iter().map(String::as_str), 2);
        m.record("en", ["once"], 3);
        assert_eq!(m.count("en"), MAX_WORDS);
        assert!(m.top("en", MAX_WORDS).iter().all(|(w, _)| w != "once"));
    }

    #[test]
    fn it_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("ttyp-missed-{}", std::process::id()));
        let path = dir.join("missed_words.json");
        let mut m = MissedWords::load(&path);
        m.record("english", ["their"], 1);
        m.save().unwrap();
        assert_eq!(MissedWords::load(&path).count("english"), 1);
        let _ = fs::remove_dir_all(dir);
    }
}

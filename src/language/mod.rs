//! Languages: a named word list loaded from TOML. Same registry pattern as
//! themes — built-ins compiled in, user files in `~/.config/ttyp/languages/`
//! override by name.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum LanguageError {
    #[error("invalid language: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("language `{0}` has no words")]
    Empty(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language {
    /// Identifier used in commands and config, e.g. `english_1k`.
    pub name: String,
    /// Human-readable label, e.g. `English 1k`.
    #[serde(default)]
    pub display: String,
    pub words: Vec<String>,
}

impl Language {
    pub fn parse(text: &str) -> Result<Self, LanguageError> {
        let mut lang: Language = toml::from_str(text)?;
        if lang.words.is_empty() {
            return Err(LanguageError::Empty(lang.name));
        }
        if lang.display.is_empty() {
            lang.display = lang.name.clone();
        }
        Ok(lang)
    }
}

const BUILTIN: &[&str] = &[
    include_str!("../../assets/languages/english.toml"),
    include_str!("../../assets/languages/english_1k.toml"),
];

#[derive(Debug, Clone, Default)]
pub struct LanguageRegistry {
    languages: BTreeMap<String, Language>,
}

impl LanguageRegistry {
    pub fn builtin() -> Self {
        let mut reg = Self::default();
        for src in BUILTIN {
            reg.insert(Language::parse(src).expect("built-in language is valid"));
        }
        reg
    }

    pub fn load(user_dir: &Path, mut warn: impl FnMut(String)) -> Self {
        let mut reg = Self::builtin();
        let Ok(entries) = std::fs::read_dir(user_dir) else {
            return reg;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()) {
                Ok(text) => match Language::parse(&text) {
                    Ok(lang) => reg.insert(lang),
                    Err(e) => warn(format!("{}: {e}", path.display())),
                },
                Err(e) => warn(format!("{}: {e}", path.display())),
            }
        }
        reg
    }

    pub fn insert(&mut self, lang: Language) {
        self.languages.insert(lang.name.clone(), lang);
    }

    pub fn get(&self, name: &str) -> Option<&Language> {
        self.languages.get(name)
    }

    pub fn get_or_default(&self, name: &str) -> &Language {
        self.get(name)
            .or_else(|| self.get("english"))
            .expect("english is built in")
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.languages.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_parse() {
        let reg = LanguageRegistry::builtin();
        assert_eq!(reg.get("english").unwrap().words.len(), 200);
        assert!(reg.get("english_1k").unwrap().words.len() > 900);
    }

    #[test]
    fn empty_rejected() {
        assert!(Language::parse("name = \"x\"\nwords = []").is_err());
    }

    #[test]
    fn display_defaults_to_name() {
        let l = Language::parse("name = \"x\"\nwords = [\"a\"]").unwrap();
        assert_eq!(l.display, "x");
    }
}

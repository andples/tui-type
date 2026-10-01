//! Language modules: optional extra word lists for a language, such as
//! Python's `numpy` or `pandas`. A module is an ordinary language file
//! (`name`, `display`, `words`, optional `trim`) kept next to its language:
//! `catalog/languages/<language>/<module>.toml` in the catalogue, and the
//! same path under the config dir once installed. The language registry
//! only reads the top level of that dir, so modules never show up as
//! languages of their own.
//!
//! Which installed modules a test mixes in is `Config::modules`; a run
//! with modules is recorded under `language_key` (`code_python+numpy`), so
//! its bests are kept apart from the plain language's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::language::Language;

/// Installed modules, by language, each list sorted by name.
#[derive(Debug, Clone, Default)]
pub struct ModuleRegistry {
    by_language: BTreeMap<String, Vec<Language>>,
}

impl ModuleRegistry {
    /// Read every `<languages_dir>/<language>/*.toml`.
    pub fn load(languages_dir: &Path, mut warn: impl FnMut(String)) -> Self {
        let mut reg = Self::default();
        let Ok(entries) = std::fs::read_dir(languages_dir) else {
            return reg;
        };
        for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            let Some(language) = dir.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(files) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut modules: Vec<Language> = Vec::new();
            for path in files.flatten().map(|e| e.path()) {
                if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
                }
                match std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| Language::parse(&t).map_err(|e| e.to_string()))
                {
                    Ok(m) => modules.push(m),
                    Err(e) => warn(format!("{}: {e}", path.display())),
                }
            }
            if !modules.is_empty() {
                modules.sort_by(|a, b| a.name.cmp(&b.name));
                reg.by_language.insert(language.to_string(), modules);
            }
        }
        reg
    }

    /// The installed modules of `language`.
    pub fn of(&self, language: &str) -> &[Language] {
        self.by_language.get(language).map_or(&[], Vec::as_slice)
    }

    pub fn get(&self, language: &str, module: &str) -> Option<&Language> {
        self.of(language).iter().find(|m| m.name == module)
    }

    /// The names in `wanted` that are installed for `language`, sorted.
    pub fn selected<'a>(&'a self, language: &str, wanted: &[String]) -> Vec<&'a Language> {
        self.of(language)
            .iter()
            .filter(|m| wanted.contains(&m.name))
            .collect()
    }
}

/// Where `language`'s modules are installed.
pub fn dir(languages_dir: &Path, language: &str) -> PathBuf {
    languages_dir.join(language)
}

/// What a run is recorded as: the language, then `+module` for each one
/// mixed in, sorted (`code_python+numpy+pandas`).
pub fn language_key(language: &str, modules: &[&Language]) -> String {
    let mut names: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
    names.sort_unstable();
    std::iter::once(language)
        .chain(names)
        .collect::<Vec<_>>()
        .join("+")
}

/// The words a test draws from: the language's, then each module's. With
/// `trim` on, every `trim` string of the language and its modules is cut
/// out of each word (`print()` → `print`); words that end up empty or
/// repeated are dropped.
pub fn word_pool(base: &Language, modules: &[&Language], trim: bool) -> Vec<String> {
    let cuts: Vec<&str> = if trim {
        std::iter::once(base)
            .chain(modules.iter().copied())
            .flat_map(|l| l.trim.iter().map(String::as_str))
            .filter(|t| !t.is_empty())
            .collect()
    } else {
        Vec::new()
    };
    let mut seen = std::collections::HashSet::new();
    std::iter::once(base)
        .chain(modules.iter().copied())
        .flat_map(|l| l.words.iter())
        .map(|w| cuts.iter().fold(w.clone(), |w, cut| w.replace(cut, "")))
        .filter(|w| !w.is_empty() && seen.insert(w.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(name: &str, words: &[&str], trim: &[&str]) -> Language {
        Language {
            name: name.into(),
            display: name.into(),
            words: words.iter().map(|w| w.to_string()).collect(),
            trim: trim.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn pool_mixes_modules_in() {
        let py = lang("code_python", &["def", "print()"], &["()"]);
        let np = lang("numpy", &["np.array()", "def"], &[]);
        assert_eq!(
            word_pool(&py, &[&np], false),
            ["def", "print()", "np.array()"]
        );
    }

    #[test]
    fn trim_cuts_boilerplate_and_drops_repeats() {
        let py = lang("code_python", &["print()", "print", "()", "len()"], &["()"]);
        assert_eq!(word_pool(&py, &[], true), ["print", "len"]);
        let np = lang("numpy", &["np.zeros()"], &[]);
        assert_eq!(
            word_pool(&py, &[&np], true),
            ["print", "len", "np.zeros"],
            "the language's trim applies to its modules"
        );
    }

    #[test]
    fn keys_sort_modules() {
        let (a, b) = (lang("pandas", &["x"], &[]), lang("numpy", &["x"], &[]));
        assert_eq!(language_key("code_python", &[]), "code_python");
        assert_eq!(
            language_key("code_python", &[&a, &b]),
            "code_python+numpy+pandas"
        );
    }

    #[test]
    fn loads_modules_from_subdirectories() {
        let root = std::env::temp_dir().join(format!("ttyp-modules-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("code_python")).unwrap();
        std::fs::write(
            root.join("code_python/numpy.toml"),
            "name = \"numpy\"\nwords = [\"np.array()\"]\n",
        )
        .unwrap();
        std::fs::write(root.join("code_python.toml"), "not a module").unwrap();
        let reg = ModuleRegistry::load(&root, |w| panic!("{w}"));
        assert_eq!(reg.of("code_python").len(), 1);
        assert!(reg.get("code_python", "numpy").is_some());
        assert!(reg.of("english").is_empty());
        let wanted = vec!["numpy".to_string(), "pandas".to_string()];
        assert_eq!(reg.selected("code_python", &wanted).len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}

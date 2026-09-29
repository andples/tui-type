//! The catalogue: languages and themes that aren't built in, installed on
//! demand into the config dir. Only `english`, `english_1k` and the
//! `default` theme ship in the binary; everything else lives under
//! `catalog/` in the repository and is fetched from `Config::catalog`
//! (an `https://` base URL, or a local directory for development).
//!
//! `catalog/index.toml` lists what's available: each language's name,
//! label and word count, and every theme in full so the menu can preview a
//! theme before it's installed. Files are `languages/<name>.toml` and
//! `themes/<name>.toml` next to it. Installing writes the file into the
//! matching config dir, where the registries pick it up like any user file.

pub mod menu;
pub mod worker;

pub use menu::{CatalogMenu, Item, ItemStatus};
pub use worker::{CatalogEvent, Fetcher};

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::language::Language;
use crate::theme::Theme;

/// Where every build fetches the catalogue from. Override per build with
/// `TTYP_DEFAULT_CATALOG`; users override with `catalog` in the config.
pub const BUILT_IN_CATALOG: &str = match option_env!("TTYP_DEFAULT_CATALOG") {
    Some(s) => s,
    None => "https://raw.githubusercontent.com/andples/tui-type/main/catalog",
};

const NOT_FOUND: &str = "not in the catalogue";

/// Largest file the catalogue will hand over (big word lists are ~1 MB).
const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Language,
    Theme,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Language, Kind::Theme];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Language => "language",
            Kind::Theme => "theme",
        }
    }

    /// Directory name, both in the catalogue and in the config dir.
    pub fn dir(self) -> &'static str {
        match self {
            Kind::Language => "languages",
            Kind::Theme => "themes",
        }
    }

    pub fn other(self) -> Kind {
        match self {
            Kind::Language => Kind::Theme,
            Kind::Theme => Kind::Language,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A language as the index describes it (the words stay in its own file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageEntry {
    pub name: String,
    pub display: String,
    /// Number of words in the list.
    pub words: usize,
}

/// `catalog/index.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Index {
    pub languages: Vec<LanguageEntry>,
    pub themes: Vec<Theme>,
}

impl Index {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("bad catalogue index: {e}"))
    }

    pub fn names(&self, kind: Kind) -> Vec<&str> {
        match kind {
            Kind::Language => self.languages.iter().map(|l| l.name.as_str()).collect(),
            Kind::Theme => self.themes.iter().map(|t| t.name.as_str()).collect(),
        }
    }

    pub fn contains(&self, kind: Kind, name: &str) -> bool {
        self.names(kind).contains(&name)
    }

    /// Which list `name` is in, languages first.
    pub fn kind_of(&self, name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| self.contains(*k, name))
    }

    pub fn theme(&self, name: &str) -> Option<&Theme> {
        self.themes.iter().find(|t| t.name == name)
    }

    pub fn language(&self, name: &str) -> Option<&LanguageEntry> {
        self.languages.iter().find(|l| l.name == name)
    }
}

/// Catalogue names double as file names, so keep them to a safe alphabet.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Check that `text` is a valid `kind` file for `name`.
pub fn validate(kind: Kind, name: &str, text: &str) -> Result<(), String> {
    let found = match kind {
        Kind::Language => Language::parse(text)
            .map(|l| l.name)
            .map_err(|e| e.to_string()),
        Kind::Theme => Theme::parse(text)
            .map(|t| t.name)
            .map_err(|e| e.to_string()),
    }
    .map_err(|e| format!("{kind} {name}: {e}"))?;
    if found != name {
        return Err(format!("{kind} {name}: file is named `{found}`"));
    }
    Ok(())
}

/// Write a fetched file into `dir` (the config dir for its kind). Goes
/// through a temporary file so a half-written file never gets loaded.
pub fn install(dir: &Path, name: &str, text: &str) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(format!("bad name `{name}`"));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("{name}.toml"));
    let tmp = dir.join(format!(".{name}.toml.tmp"));
    std::fs::write(&tmp, text)
        .and_then(|()| std::fs::rename(&tmp, &path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", path.display())
        })?;
    Ok(path)
}

/// Remove every file in `dir` that defines `name` (whatever the file is
/// called). Returns how many were removed.
pub fn uninstall(dir: &Path, name: &str) -> Result<usize, String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    let mut removed = 0;
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if defines(&text) != Some(name) {
            continue;
        }
        std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        removed += 1;
    }
    Ok(removed)
}

/// The `name = "…"` a language or theme file declares, without parsing
/// the rest (a word list can be large).
fn defines(text: &str) -> Option<&str> {
    text.lines().find_map(|l| {
        let rest = l
            .trim()
            .strip_prefix("name")?
            .trim_start()
            .strip_prefix('=')?;
        rest.trim().strip_prefix('"')?.strip_suffix('"')
    })
}

/// Where catalogue files come from.
#[derive(Clone)]
pub struct Source {
    base: String,
    agent: Option<ureq::Agent>,
}

impl Source {
    /// An `http(s)://` base URL, or a local directory.
    pub fn new(base: &str) -> Self {
        let base = base.trim_end_matches('/').to_string();
        let agent = base.starts_with("http").then(|| {
            ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(20)))
                .user_agent(concat!("ttyp/", env!("CARGO_PKG_VERSION")))
                .build()
                .new_agent()
        });
        Self { base, agent }
    }

    fn get(&self, rel: &str) -> Result<String, String> {
        let Some(agent) = &self.agent else {
            let path = Path::new(&self.base).join(rel);
            return std::fs::read_to_string(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    NOT_FOUND.to_string()
                } else {
                    format!("{}: {e}", path.display())
                }
            });
        };
        let url = format!("{}/{rel}", self.base);
        let mut resp = agent.get(&url).call().map_err(|e| match e {
            ureq::Error::StatusCode(404) => NOT_FOUND.to_string(),
            e => format!("catalogue unreachable: {e}"),
        })?;
        resp.body_mut()
            .with_config()
            .limit(MAX_BYTES)
            .read_to_string()
            .map_err(|e| format!("catalogue: {e}"))
    }

    pub fn index(&self) -> Result<Index, String> {
        let text = self.get("index.toml").map_err(|e| {
            if e == NOT_FOUND {
                format!("no catalogue at {}", self.base)
            } else {
                e
            }
        })?;
        Index::parse(&text)
    }

    /// A catalogue file, checked to be a valid `kind` named `name`.
    pub fn fetch(&self, kind: Kind, name: &str) -> Result<String, String> {
        if !valid_name(name) {
            return Err(format!("bad name `{name}`"));
        }
        let text = self
            .get(&format!("{}/{name}.toml", kind.dir()))
            .map_err(|e| format!("{kind} {name}: {e}"))?;
        validate(kind, name, &text)?;
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ttyp-catalog-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const NORD: &str = "name = \"nord\"\n[colors]\nbg=\"#2e3440\"\nfg=\"#d8dee9\"\nsub=\"#616e88\"\nmain=\"#88c0d0\"\ncorrect=\"#d8dee9\"\nerror=\"#bf616a\"\nerror_extra=\"#d08770\"\n";

    #[test]
    fn names_are_file_safe() {
        assert!(valid_name("english_1k"));
        assert!(valid_name("catppuccin-mocha"));
        for bad in ["", "../x", "a/b", "Nord", "x.toml", "a b"] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn validate_checks_kind_and_name() {
        assert!(validate(Kind::Theme, "nord", NORD).is_ok());
        assert!(validate(Kind::Theme, "gruvbox", NORD).is_err());
        assert!(validate(Kind::Language, "nord", NORD).is_err());
        let lang = "name = \"x\"\nwords = [\"a\", \"b\"]";
        assert!(validate(Kind::Language, "x", lang).is_ok());
        assert!(validate(Kind::Language, "x", "name = \"x\"\nwords = []").is_err());
    }

    #[test]
    fn install_then_uninstall() {
        let dir = temp_dir("install");
        let path = install(&dir, "nord", NORD).unwrap();
        assert_eq!(path, dir.join("nord.toml"));
        // A hand-written copy under another file name goes too.
        std::fs::write(dir.join("my-nord.toml"), NORD).unwrap();
        std::fs::write(dir.join("other.toml"), NORD.replace("nord", "other")).unwrap();
        assert_eq!(uninstall(&dir, "nord").unwrap(), 2);
        assert!(dir.join("other.toml").exists());
        assert_eq!(uninstall(&dir, "nord").unwrap(), 0);
        assert!(install(&dir, "../evil", NORD).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn local_source() {
        let dir = temp_dir("source");
        std::fs::create_dir_all(dir.join("themes")).unwrap();
        std::fs::write(dir.join("themes/nord.toml"), NORD).unwrap();
        std::fs::write(
            dir.join("index.toml"),
            "[[languages]]\nname = \"x\"\ndisplay = \"X\"\nwords = 2\n",
        )
        .unwrap();
        let src = Source::new(dir.to_str().unwrap());
        let index = src.index().unwrap();
        assert_eq!(index.kind_of("x"), Some(Kind::Language));
        assert_eq!(index.kind_of("nord"), None);
        assert!(src.fetch(Kind::Theme, "nord").is_ok());
        let missing = src.fetch(Kind::Theme, "gruvbox").unwrap_err();
        assert!(missing.contains("not in the catalogue"), "{missing}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `catalog/index.toml` must describe exactly the files next to it.
    /// Regenerate it with `TTYP_BLESS=1 cargo test catalog_index`.
    #[test]
    fn catalog_index_is_current() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("catalog");
        if !root.exists() {
            return; // packaged crate: the catalogue isn't shipped
        }
        let src = Source::new(root.to_str().unwrap());
        let mut expected = Index::default();
        for kind in Kind::ALL {
            let mut names: Vec<String> = std::fs::read_dir(root.join(kind.dir()))
                .unwrap()
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    (p.extension()? == "toml").then(|| p.file_stem()?.to_str().map(String::from))?
                })
                .collect();
            names.sort();
            for name in names {
                let text = src.fetch(kind, &name).unwrap();
                match kind {
                    Kind::Language => {
                        let l = Language::parse(&text).unwrap();
                        expected.languages.push(LanguageEntry {
                            name: l.name,
                            display: l.display,
                            words: l.words.len(),
                        });
                    }
                    Kind::Theme => expected.themes.push(Theme::parse(&text).unwrap()),
                }
            }
        }
        let text = format!(
            "# Generated from the files in this directory: TTYP_BLESS=1 cargo test catalog_index\n\n{}",
            toml::to_string_pretty(&expected).unwrap()
        );
        if std::env::var_os("TTYP_BLESS").is_some() {
            std::fs::write(root.join("index.toml"), &text).unwrap();
        }
        let current = src.index().expect("catalog/index.toml parses");
        assert_eq!(
            current, expected,
            "catalog/index.toml is stale; run TTYP_BLESS=1 cargo test catalog_index"
        );
        // No catalogue entry may shadow a built-in.
        for l in &expected.languages {
            assert!(
                crate::language::LanguageRegistry::builtin()
                    .get(&l.name)
                    .is_none(),
                "{} is built in",
                l.name
            );
        }
        for t in &expected.themes {
            assert!(
                crate::theme::ThemeRegistry::builtin()
                    .get(&t.name)
                    .is_none(),
                "{} is built in",
                t.name
            );
        }
    }
}

//! Themes: a small set of named color roles loaded from TOML. Built-ins are
//! compiled into the binary; user themes in `~/.config/ttyp/themes/*.toml`
//! are loaded on top and override built-ins with the same name.

use std::collections::BTreeMap;
use std::path::Path;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("invalid theme: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid color `{0}`: expected #rrggbb")]
    Color(String),
}

/// Color roles. Everything in the UI maps onto one of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Colors {
    /// Screen background.
    pub bg: Hex,
    /// Primary text (headings, results numbers).
    pub fg: Hex,
    /// Dimmed UI: untyped words, mode line, help.
    pub sub: Hex,
    /// Accent: caret, timer, selected item, brand.
    pub main: Hex,
    /// Correctly typed characters.
    pub correct: Hex,
    /// Incorrectly typed characters.
    pub error: Hex,
    /// Characters typed past the end of a word.
    pub error_extra: Hex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub colors: Colors,
}

/// A `#rrggbb` string that converts to a ratatui color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hex(pub u8, pub u8, pub u8);

impl Hex {
    pub fn parse(s: &str) -> Result<Self, ThemeError> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() != 6 || !hex.is_ascii() {
            return Err(ThemeError::Color(s.to_string()));
        }
        let byte = |i: usize| {
            u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ThemeError::Color(s.to_string()))
        };
        Ok(Hex(byte(0)?, byte(2)?, byte(4)?))
    }

    pub fn color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }
}

impl From<Hex> for Color {
    fn from(h: Hex) -> Self {
        h.color()
    }
}

impl Serialize for Hex {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2))
    }
}

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Hex::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl Theme {
    pub fn parse(text: &str) -> Result<Self, ThemeError> {
        Ok(toml::from_str(text)?)
    }
}

const BUILTIN: &[&str] = &[
    include_str!("../../assets/themes/default.toml"),
    include_str!("../../assets/themes/gruvbox.toml"),
    include_str!("../../assets/themes/catppuccin-mocha.toml"),
    include_str!("../../assets/themes/nord.toml"),
    include_str!("../../assets/themes/rose-pine.toml"),
    include_str!("../../assets/themes/light.toml"),
];

/// All known themes, keyed by name.
#[derive(Debug, Clone, Default)]
pub struct ThemeRegistry {
    themes: BTreeMap<String, Theme>,
}

impl ThemeRegistry {
    /// Built-ins only.
    pub fn builtin() -> Self {
        let mut reg = Self::default();
        for src in BUILTIN {
            let theme = Theme::parse(src).expect("built-in theme is valid");
            reg.insert(theme);
        }
        reg
    }

    /// Built-ins plus every `*.toml` in `user_dir`. Files that fail to parse
    /// are reported through `warn` and skipped so one bad file can't break startup.
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
                Ok(text) => match Theme::parse(&text) {
                    Ok(theme) => reg.insert(theme),
                    Err(e) => warn(format!("{}: {e}", path.display())),
                },
                Err(e) => warn(format!("{}: {e}", path.display())),
            }
        }
        reg
    }

    pub fn insert(&mut self, theme: Theme) {
        self.themes.insert(theme.name.clone(), theme);
    }

    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    /// `name` if present, otherwise `default` (always exists among built-ins).
    pub fn get_or_default(&self, name: &str) -> &Theme {
        self.get(name)
            .or_else(|| self.get("default"))
            .expect("default theme is built in")
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.themes.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.themes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.themes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_parse() {
        let reg = ThemeRegistry::builtin();
        assert_eq!(reg.len(), BUILTIN.len());
        assert!(reg.get("default").is_some());
        assert!(reg.get("gruvbox").is_some());
    }

    #[test]
    fn hex_parse() {
        assert_eq!(Hex::parse("#d79921").unwrap(), Hex(0xd7, 0x99, 0x21));
        assert_eq!(Hex::parse("ffffff").unwrap(), Hex(255, 255, 255));
        assert!(Hex::parse("#fff").is_err());
        assert!(Hex::parse("#gggggg").is_err());
    }

    #[test]
    fn user_overrides_builtin() {
        let dir = std::env::temp_dir().join(format!("ttyp-theme-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("gruvbox.toml"),
            "name = \"gruvbox\"\n[colors]\nbg=\"#000000\"\nfg=\"#ffffff\"\nsub=\"#888888\"\nmain=\"#ff0000\"\ncorrect=\"#ffffff\"\nerror=\"#ff0000\"\nerror_extra=\"#aa0000\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("broken.toml"), "name = 1").unwrap();
        let mut warnings = vec![];
        let reg = ThemeRegistry::load(&dir, |w| warnings.push(w));
        assert_eq!(reg.get("gruvbox").unwrap().colors.main, Hex(255, 0, 0));
        assert_eq!(warnings.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

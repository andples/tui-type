//! User configuration: loaded from `~/.config/ttyp/config.toml`, every field
//! optional with sane defaults. Settings changed at runtime are written back
//! immediately so the file always mirrors the live state.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::test::mode::Mode;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid config {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("could not serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Which sections of the results screen are shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResultsConfig {
    pub chart: bool,
    pub char_breakdown: bool,
    pub consistency: bool,
    pub raw: bool,
}

impl Default for ResultsConfig {
    fn default() -> Self {
        Self {
            chart: true,
            char_breakdown: true,
            consistency: true,
            raw: true,
        }
    }
}

impl ResultsConfig {
    pub const SECTIONS: [&'static str; 4] = ["chart", "breakdown", "consistency", "raw"];

    pub fn get(&self, section: &str) -> Option<bool> {
        match section {
            "chart" => Some(self.chart),
            "breakdown" | "char_breakdown" => Some(self.char_breakdown),
            "consistency" => Some(self.consistency),
            "raw" => Some(self.raw),
            _ => None,
        }
    }

    pub fn set(&mut self, section: &str, value: bool) -> bool {
        match section {
            "chart" => self.chart = value,
            "breakdown" | "char_breakdown" => self.char_breakdown = value,
            "consistency" => self.consistency = value,
            "raw" => self.raw = value,
            _ => return false,
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub language: String,
    pub mode: Mode,
    pub punctuation: bool,
    pub numbers: bool,
    /// Glyph scale for the typing text, see `FontSize`.
    pub font_size: u8,
    /// Width of the word box, in words (× 6 characters per word).
    pub words_per_line: u8,
    /// Words only: hide brand, timer and mode line while typing.
    pub zen: bool,
    /// Font for enlarged text when drawn as images: a family name, a file in
    /// the config `fonts/` dir, or a path. Empty uses the system monospace.
    pub font: String,
    pub graphics: Graphics,
    pub results: ResultsConfig,
}

/// How font sizes above 1 are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Graphics {
    /// Real font images when the terminal is known to support them.
    #[default]
    Auto,
    /// Always use the kitty graphics protocol.
    Kitty,
    /// Always use block characters.
    Off,
}

impl Graphics {
    pub const NAMES: [&'static str; 3] = ["auto", "kitty", "off"];

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "auto" => Ok(Graphics::Auto),
            "kitty" | "on" => Ok(Graphics::Kitty),
            "off" | "blocks" => Ok(Graphics::Off),
            _ => Err(format!("expected auto, kitty or off, got `{s}`")),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Graphics::Auto => "auto",
            Graphics::Kitty => "kitty",
            Graphics::Off => "off",
        }
    }
}

pub const FONT_SIZE_RANGE: (u8, u8) = (1, 5);
pub const DEFAULT_FONT_SIZE: u8 = 2;
pub const WORDS_PER_LINE_RANGE: (u8, u8) = (4, 30);
/// The conventional word length used to turn words-per-line into columns.
pub const CHARS_PER_WORD: u16 = 6;

/// Which block characters rasterize the pixel font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSet {
    /// 2×3 pixels per cell (Unicode 13 "Symbols for Legacy Computing").
    Sextant,
    /// 1×2 pixels per cell (`▀ ▄ █`), available everywhere.
    Half,
}

impl BlockSet {
    pub fn pixels_per_cell(self) -> (u16, u16) {
        match self {
            BlockSet::Sextant => (2, 3),
            BlockSet::Half => (1, 2),
        }
    }
}

/// How the typing text is drawn. Size 1 is the terminal's own font; the
/// rest rasterize the 4×6 pixel font (`ui::font`) at increasing scales so
/// each step is a gentle one: glyphs are 1, 2, 3, 4 and 6 rows tall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontSize(u8);

impl FontSize {
    pub fn from_level(level: u8) -> Self {
        FontSize(level.clamp(FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1))
    }

    pub fn level(self) -> u8 {
        self.0
    }

    pub fn is_native(self) -> bool {
        self.0 == 1
    }

    /// Block set and pixel scale for rasterized sizes; `None` for native.
    pub fn raster(self) -> Option<(BlockSet, u16)> {
        match self.0 {
            1 => None,
            2 => Some((BlockSet::Sextant, 1)),
            3 => Some((BlockSet::Half, 1)),
            4 => Some((BlockSet::Sextant, 2)),
            _ => Some((BlockSet::Half, 2)),
        }
    }

    /// (columns, rows) one glyph occupies.
    pub fn cell_dims(self) -> (u16, u16) {
        match self.raster() {
            None => (1, 1),
            Some((set, scale)) => {
                let (pw, ph) = set.pixels_per_cell();
                (4 * scale / pw, 6 * scale / ph)
            }
        }
    }
}

impl Config {
    pub fn font_size(&self) -> FontSize {
        FontSize::from_level(self.font_size)
    }

    pub fn set_font_size(&mut self, level: u8) {
        self.font_size = level.clamp(FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1);
    }

    pub fn set_words_per_line(&mut self, n: u8) {
        self.words_per_line = n.clamp(WORDS_PER_LINE_RANGE.0, WORDS_PER_LINE_RANGE.1);
    }

    /// Width of the typing box in terminal columns.
    pub fn typing_width(&self) -> u16 {
        let (gw, _) = self.font_size().cell_dims();
        self.words_per_line as u16 * CHARS_PER_WORD * gw
    }

    /// Width of every other screen: at least room for the results row.
    pub fn content_width(&self) -> u16 {
        (self.words_per_line as u16 * CHARS_PER_WORD).max(80)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "default".into(),
            language: "english".into(),
            mode: Mode::Time(30),
            punctuation: false,
            numbers: false,
            font_size: DEFAULT_FONT_SIZE,
            words_per_line: 13,
            zen: false,
            font: String::new(),
            graphics: Graphics::Auto,
            results: ResultsConfig::default(),
        }
    }
}

impl Config {
    /// Load from `path`; a missing file yields the defaults.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        }
        fs::write(path, text).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Generic `:set key value` escape hatch. Returns an error message for the
    /// user on unknown keys or unparsable values.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let parse_bool = |v: &str| match v {
            "true" | "on" | "yes" | "1" => Ok(true),
            "false" | "off" | "no" | "0" => Ok(false),
            _ => Err(format!("expected on/off, got `{v}`")),
        };
        match key {
            "theme" => self.theme = value.to_string(),
            "language" | "lang" => self.language = value.to_string(),
            "punctuation" => self.punctuation = parse_bool(value)?,
            "numbers" => self.numbers = parse_bool(value)?,
            "zen" => self.zen = parse_bool(value)?,
            "font" => self.font = value.to_string(),
            "graphics" => self.graphics = Graphics::parse(value)?,
            "font_size" | "fontsize" => {
                self.set_font_size(parse_range(value, FONT_SIZE_RANGE)?);
            }
            "words_per_line" | "wpl" => {
                self.set_words_per_line(parse_range(value, WORDS_PER_LINE_RANGE)?);
            }
            "time" => {
                self.mode = Mode::Time(
                    value
                        .parse()
                        .map_err(|_| format!("bad seconds `{value}`"))?,
                )
            }
            "words" => {
                self.mode = Mode::Words(value.parse().map_err(|_| format!("bad count `{value}`"))?)
            }
            k if k.starts_with("results.") => {
                let section = &k["results.".len()..];
                if !self.results.set(section, parse_bool(value)?) {
                    return Err(format!("unknown results section `{section}`"));
                }
            }
            _ => return Err(format!("unknown setting `{key}`")),
        }
        Ok(())
    }
}

/// Parse an integer and check it lies in `range` (inclusive).
pub fn parse_range(value: &str, range: (u8, u8)) -> Result<u8, String> {
    let n: u8 = value
        .parse()
        .map_err(|_| format!("expected a number {}-{}, got `{value}`", range.0, range.1))?;
    if n < range.0 || n > range.1 {
        return Err(format!("must be between {} and {}", range.0, range.1));
    }
    Ok(n)
}

/// Filesystem locations. Everything lives under the XDG dirs for `ttyp`.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_file: PathBuf,
    pub themes_dir: PathBuf,
    pub languages_dir: PathBuf,
    pub fonts_dir: PathBuf,
    pub history_file: PathBuf,
}

impl Paths {
    pub fn discover() -> Self {
        let dirs = ProjectDirs::from("", "", "ttyp");
        let (config_dir, data_dir) = match &dirs {
            Some(d) => (d.config_dir().to_path_buf(), d.data_dir().to_path_buf()),
            None => (PathBuf::from(".ttyp"), PathBuf::from(".ttyp")),
        };
        Self::from_dirs(&config_dir, &data_dir)
    }

    pub fn from_dirs(config_dir: &Path, data_dir: &Path) -> Self {
        Self {
            config_file: config_dir.join("config.toml"),
            themes_dir: config_dir.join("themes"),
            languages_dir: config_dir.join("languages"),
            fonts_dir: config_dir.join("fonts"),
            history_file: data_dir.join("history.jsonl"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_size_ladder_is_gentle() {
        let rows: Vec<u16> = (1..=5)
            .map(|l| FontSize::from_level(l).cell_dims().1)
            .collect();
        assert_eq!(rows, vec![1, 2, 3, 4, 6]);
        assert_eq!(FontSize::from_level(2).cell_dims(), (2, 2));
        assert_eq!(FontSize::from_level(5).cell_dims(), (8, 6));
    }

    #[test]
    fn empty_file_is_default() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn partial_file_fills_defaults() {
        let c =
            Config::parse("theme = \"nord\"\nmode = { words = 25 }\n[results]\nchart = false\n")
                .unwrap();
        assert_eq!(c.theme, "nord");
        assert_eq!(c.mode, Mode::Words(25));
        assert!(!c.results.chart);
        assert!(c.results.raw);
        assert_eq!(c.language, "english");
    }

    #[test]
    fn roundtrip() {
        let c = Config {
            punctuation: true,
            mode: Mode::Time(60),
            ..Default::default()
        };
        let text = toml::to_string_pretty(&c).unwrap();
        assert_eq!(Config::parse(&text).unwrap(), c);
    }

    #[test]
    fn set_generic() {
        let mut c = Config::default();
        c.set("results.chart", "off").unwrap();
        assert!(!c.results.chart);
        c.set("time", "15").unwrap();
        assert_eq!(c.mode, Mode::Time(15));
        assert!(c.set("bogus", "1").is_err());
        c.set("wpl", "10").unwrap();
        c.set("fontsize", "1").unwrap();
        assert_eq!(c.typing_width(), 60);
        c.set("fontsize", "3").unwrap();
        assert_eq!(c.typing_width(), 240);
        assert!(c.set("fontsize", "9").is_err());
        assert!(c.set("wpl", "2").is_err());
        assert!(c.set("numbers", "maybe").is_err());
        c.set("graphics", "off").unwrap();
        assert_eq!(c.graphics, Graphics::Off);
        assert!(c.set("graphics", "sixel").is_err());
    }
}

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
    /// How many lines of words the typing box shows.
    pub lines: u8,
    /// Words only: hide brand, timer and mode line while typing.
    pub zen: bool,
    /// Fill the terminal: pick the largest font size at which
    /// `words_per_line` × `lines` words still fit, then show as many lines
    /// as fill the height, with the chrome (brand, timer, mode line)
    /// hidden. Overrides `font_size` and the shown line count.
    pub fullscreen: bool,
    /// Font for enlarged text when drawn as images: a family name, a file in
    /// the config `fonts/` dir, or a path. Empty uses the system monospace.
    pub font: String,
    pub graphics: Graphics,
    pub results: ResultsConfig,
    /// Active profiles (see `crate::profile`), in activation order. Each one
    /// controls a distinct set of settings and matches the values above.
    pub profiles: Vec<String>,
}

/// How font sizes above 1 are drawn. Only set in the config file; there is
/// no command for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Graphics {
    /// Real font images via the kitty graphics protocol when the terminal
    /// supports it; block characters otherwise.
    #[default]
    #[serde(alias = "auto")]
    Kitty,
    /// Always use block characters.
    Off,
}

impl Graphics {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "kitty" | "auto" | "on" => Ok(Graphics::Kitty),
            "off" | "blocks" => Ok(Graphics::Off),
            _ => Err(format!("expected kitty or off, got `{s}`")),
        }
    }
}

pub const FONT_SIZE_RANGE: (u8, u8) = (1, 16);
pub const DEFAULT_FONT_SIZE: u8 = 2;
pub const WORDS_PER_LINE_RANGE: (u8, u8) = (4, 30);
pub const LINES_RANGE: (u8, u8) = (1, 10);
pub const DEFAULT_LINES: u8 = 3;
/// Most lines fullscreen fills the screen with.
const LINES_MAX_FULLSCREEN: u16 = 60;
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
/// each step is a gentle one: glyphs are 1, 2, 3, 4 and 6 rows tall, then
/// two rows taller per size (8, 10, … 28). Fullscreen isn't limited to
/// the numbered sizes: it picks from every block set and scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontSize(Option<(BlockSet, u16)>);

/// Tallest glyph any size draws, in rows (matches the top numbered size).
const MAX_GLYPH_ROWS: u16 = 28;

impl FontSize {
    pub fn from_level(level: u8) -> Self {
        FontSize(match level.clamp(FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1) {
            1 => None,
            2 => Some((BlockSet::Sextant, 1)),
            3 => Some((BlockSet::Half, 1)),
            4 => Some((BlockSet::Sextant, 2)),
            5 => Some((BlockSet::Half, 2)),
            n => Some((BlockSet::Sextant, n as u16 - 2)),
        })
    }

    pub fn is_native(self) -> bool {
        self.0.is_none()
    }

    /// Block set and pixel scale for rasterized sizes; `None` for native.
    pub fn raster(self) -> Option<(BlockSet, u16)> {
        self.0
    }

    /// The terminal font plus every block set at every scale up to
    /// `MAX_GLYPH_ROWS`, numbered or not.
    fn all() -> impl Iterator<Item = FontSize> {
        let scales = |set: BlockSet| {
            (1..)
                .map(move |s| FontSize(Some((set, s))))
                .take_while(|f| f.cell_dims().1 <= MAX_GLYPH_ROWS)
        };
        std::iter::once(FontSize(None))
            .chain(scales(BlockSet::Sextant))
            .chain(scales(BlockSet::Half))
    }

    /// Rows between the tops of consecutive lines: a gap row for big fonts.
    pub fn line_stride(self) -> u16 {
        let (_, gh) = self.cell_dims();
        if gh > 1 { gh + 1 } else { 1 }
    }

    /// Rows a box of `lines` lines needs.
    pub fn box_height(self, lines: u8) -> u16 {
        let (_, gh) = self.cell_dims();
        let stride = self.line_stride();
        lines.max(1) as u16 * stride - (stride - gh)
    }

    /// How many lines of this size fit in `rows`.
    pub fn lines_in(self, rows: u16) -> u16 {
        let (_, gh) = self.cell_dims();
        let stride = self.line_stride();
        (rows + stride - gh) / stride
    }

    /// Fullscreen sizing: the largest size (tallest, then widest) at which
    /// `chars` characters of text fit in `cols` × `rows`, and how many lines
    /// fill the height at that size. `glyph_cols` is how many columns a
    /// character takes at a size. Falls back to the terminal font.
    pub fn fit(
        cols: u16,
        rows: u16,
        chars: u16,
        glyph_cols: impl Fn(FontSize) -> f32,
    ) -> (FontSize, u8) {
        // Wrapping on word boundaries leaves roughly a word's worth of
        // space at the end of each line.
        const WRAP_SLACK: f32 = 5.0;
        let lines = |f: FontSize| f.lines_in(rows).min(LINES_MAX_FULLSCREEN);
        let capacity = |f: FontSize| {
            let per_line = (cols as f32 / glyph_cols(f)).floor() - WRAP_SLACK;
            per_line.max(0.0) * lines(f) as f32
        };
        let font = Self::all()
            .filter(|f| lines(*f) >= 1 && capacity(*f) >= chars as f32)
            .max_by_key(|f| {
                let (w, h) = f.cell_dims();
                (h, w)
            })
            .unwrap_or(FontSize(None));
        (font, lines(font).max(1) as u8)
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

    pub fn set_lines(&mut self, n: u8) {
        self.lines = n.clamp(LINES_RANGE.0, LINES_RANGE.1);
    }

    /// Characters a line should hold: the word-count target × 6.
    pub fn line_chars(&self) -> u16 {
        self.words_per_line as u16 * CHARS_PER_WORD
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
            lines: DEFAULT_LINES,
            zen: false,
            fullscreen: false,
            font: String::new(),
            graphics: Graphics::Kitty,
            results: ResultsConfig::default(),
            profiles: Vec::new(),
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
            "fullscreen" | "full" => self.fullscreen = parse_bool(value)?,
            "lines" => self.set_lines(parse_range(value, LINES_RANGE)?),
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
    pub profiles_dir: PathBuf,
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
            profiles_dir: config_dir.join("profiles"),
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
        let big: Vec<u16> = (5..=FONT_SIZE_RANGE.1)
            .map(|l| FontSize::from_level(l).cell_dims().1)
            .collect();
        assert!(big.windows(2).all(|w| w[1] > w[0]), "{big:?}");
        assert_eq!(FontSize::from_level(16).cell_dims(), (28, 28));
        assert_eq!(FontSize::from_level(2).cell_dims(), (2, 2));
        assert_eq!(FontSize::from_level(5).cell_dims(), (8, 6));
    }

    #[test]
    fn fullscreen_fit_fills_the_screen() {
        let blocks = |f: FontSize| f.cell_dims().0 as f32;
        // 160×44 with 13 words × 3 lines (234 chars): 6×6 glyphs hold only
        // 21×6 = 126, 4×4 hold 34×8 = 272 — so 4×4 across 8 lines.
        let (f, lines) = FontSize::fit(158, 43, 234, blocks);
        assert_eq!((f.cell_dims(), lines), ((4, 4), 8));
        assert!(f.box_height(lines) <= 43);
        // Less text → bigger glyphs.
        let (big, _) = FontSize::fit(158, 43, 60, blocks);
        assert!(big.cell_dims().1 > 4);
        // Nothing fits: terminal font, one line minimum.
        let (f, lines) = FontSize::fit(20, 1, 500, blocks);
        assert!(f.is_native());
        assert_eq!(lines, 1);
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
    fn graphics_defaults_to_kitty_and_reads_old_auto() {
        assert_eq!(Config::default().graphics, Graphics::Kitty);
        let c = Config::parse("graphics = \"auto\"").unwrap();
        assert_eq!(c.graphics, Graphics::Kitty);
        let c = Config::parse("graphics = \"off\"").unwrap();
        assert_eq!(c.graphics, Graphics::Off);
        assert!(
            toml::to_string(&Config::default())
                .unwrap()
                .contains("graphics = \"kitty\"")
        );
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
        assert!(c.set("fontsize", "17").is_err());
        assert!(c.set("wpl", "2").is_err());
        c.set("lines", "5").unwrap();
        assert_eq!(c.lines, 5);
        assert!(c.set("lines", "0").is_err());
        c.set("fullscreen", "on").unwrap();
        assert!(c.fullscreen);
        assert!(c.set("numbers", "maybe").is_err());
        c.set("graphics", "off").unwrap();
        assert_eq!(c.graphics, Graphics::Off);
        assert!(c.set("graphics", "sixel").is_err());
    }
}

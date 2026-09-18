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
    pub results: ResultsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "default".into(),
            language: "english".into(),
            mode: Mode::Time(30),
            punctuation: false,
            numbers: false,
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

/// Filesystem locations. Everything lives under the XDG dirs for `ttyp`.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_file: PathBuf,
    pub themes_dir: PathBuf,
    pub languages_dir: PathBuf,
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
            history_file: data_dir.join("history.jsonl"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(c.set("numbers", "maybe").is_err());
    }
}

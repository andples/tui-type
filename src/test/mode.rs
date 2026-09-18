//! Test modes. Presets are what the palette offers; any value is accepted.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Fixed duration in seconds; words are generated endlessly.
    Time(u16),
    /// Fixed number of words; ends on the last character.
    Words(u16),
}

impl Mode {
    pub const TIME_PRESETS: [u16; 4] = [15, 30, 60, 120];
    pub const WORD_PRESETS: [u16; 4] = [10, 25, 50, 100];

    /// Short identifier used in stats and the mode line, e.g. `time 30`.
    pub fn label(&self) -> String {
        match self {
            Mode::Time(s) => format!("time {s}"),
            Mode::Words(n) => format!("words {n}"),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Mode::Time(_) => "time",
            Mode::Words(_) => "words",
        }
    }

    pub fn value(&self) -> u16 {
        match self {
            Mode::Time(v) | Mode::Words(v) => *v,
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_shape() {
        assert_eq!(
            toml::to_string(&Mode::Time(30)).unwrap().trim(),
            "time = 30"
        );
        let m: Mode = toml::from_str("words = 25").unwrap();
        assert_eq!(m, Mode::Words(25));
    }
}

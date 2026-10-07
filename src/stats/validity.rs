//! Whether a finished run counts. A run where the typist walked away,
//! barely typed or mashed keys is kept in the history, marked, but never
//! sets a personal best, a pace target or the stats screen's numbers.

use serde::{Deserialize, Serialize};

use crate::test::Metrics;

/// Seconds in a row without a single key that make a run AFK.
pub const AFK_SECS: usize = 8;
/// Slowest raw speed that still counts, in wpm.
pub const MIN_RAW: f64 = 10.0;
/// Lowest accuracy that still counts, in percent.
pub const MIN_ACC: f64 = 50.0;

/// Why a run doesn't count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Invalid {
    /// `AFK_SECS` or more without a key.
    Afk,
    /// Raw speed under `MIN_RAW`.
    Slow,
    /// Accuracy under `MIN_ACC`.
    Inaccurate,
    /// Failed: a wrong key with `sudden_death` on.
    SuddenDeath,
    /// Failed: under the `min_wpm` speed.
    BelowMinWpm,
    /// A reason from a newer ttyp.
    #[serde(other)]
    Other,
}

impl Invalid {
    pub fn label(self) -> &'static str {
        match self {
            Invalid::Afk => "afk",
            Invalid::Slow => "too slow",
            Invalid::Inaccurate => "too inaccurate",
            Invalid::SuddenDeath => "sudden death: a wrong key",
            Invalid::BelowMinWpm => "dropped under the minimum speed",
            Invalid::Other => "invalid",
        }
    }
}

impl Invalid {
    /// A difficulty setting ended the run, rather than how it went.
    pub fn is_failure(self) -> bool {
        matches!(self, Invalid::SuddenDeath | Invalid::BelowMinWpm)
    }
}

/// The reason a run with these metrics doesn't count, if any.
pub fn check(m: &Metrics) -> Option<Invalid> {
    let longest_gap = m
        .raw_per_second
        .split(|r| *r > 0.0)
        .map(<[f64]>::len)
        .max()
        .unwrap_or(0);
    if longest_gap >= AFK_SECS {
        Some(Invalid::Afk)
    } else if m.accuracy < MIN_ACC {
        Some(Invalid::Inaccurate)
    } else if m.raw < MIN_RAW {
        Some(Invalid::Slow)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(raw_per_second: Vec<f64>, raw: f64, accuracy: f64) -> Metrics {
        Metrics {
            wpm: raw,
            raw,
            accuracy,
            consistency: 100.0,
            chars: Default::default(),
            duration: std::time::Duration::from_secs(raw_per_second.len() as u64),
            wpm_per_second: raw_per_second.clone(),
            errors_per_second: vec![0; raw_per_second.len()],
            raw_per_second,
        }
    }

    #[test]
    fn a_normal_run_counts() {
        assert_eq!(check(&metrics(vec![60.0; 30], 60.0, 95.0)), None);
    }

    #[test]
    fn walking_away_is_afk() {
        let mut secs = vec![60.0; 5];
        secs.extend([0.0; AFK_SECS]);
        secs.push(12.0);
        assert_eq!(check(&metrics(secs, 30.0, 95.0)), Some(Invalid::Afk));
        let mut short = vec![60.0; 5];
        short.extend([0.0; AFK_SECS - 1]);
        assert_eq!(check(&metrics(short, 30.0, 95.0)), None);
    }

    #[test]
    fn mashing_and_crawling_dont_count() {
        let secs = vec![40.0; 10];
        assert_eq!(
            check(&metrics(secs.clone(), 40.0, 30.0)),
            Some(Invalid::Inaccurate)
        );
        assert_eq!(check(&metrics(secs, 8.0, 90.0)), Some(Invalid::Slow));
    }

    #[test]
    fn unknown_reasons_still_parse() {
        let r: Invalid = serde_json::from_str("\"bot\"").unwrap();
        assert_eq!(r, Invalid::Other);
        assert_eq!(serde_json::to_string(&Invalid::Afk).unwrap(), "\"afk\"");
    }
}

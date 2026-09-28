//! The keystroke log of a daily run and its replay. The client records one
//! while typing; the server feeds it back through the same engine and
//! trusts only the metrics it computes itself.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::engine::{Status, TestEngine};
use super::generator::FixedGenerator;
use super::metrics::Metrics;
use super::mode::Mode;

/// Faster than this and the run is not a human typing.
const MAX_RAW_WPM: f64 = 400.0;
/// A time-mode test may end this long after its limit (the client ticks
/// every 100 ms; more means the log was doctored or the machine froze).
const TIME_SLACK: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    DeleteWord,
    /// The timer tick that ended a time-mode test.
    Tick,
}

/// Serialized as a string: the character itself, or `backspace`,
/// `delete_word`, `tick` (all longer than one character, so unambiguous).
impl Serialize for Key {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut buf = [0u8; 4];
        s.serialize_str(match self {
            Key::Char(c) => c.encode_utf8(&mut buf),
            Key::Backspace => "backspace",
            Key::DeleteWord => "delete_word",
            Key::Tick => "tick",
        })
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Ok(Key::Char(c)),
            _ => match s.as_str() {
                "backspace" => Ok(Key::Backspace),
                "delete_word" => Ok(Key::DeleteWord),
                "tick" => Ok(Key::Tick),
                other => Err(serde::de::Error::custom(format!("unknown key `{other}`"))),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    /// Milliseconds since the first keystroke.
    pub ms: u32,
    pub key: Key,
}

/// Why a log was not accepted as a real run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Rejected {
    #[error("empty keylog")]
    Empty,
    #[error("keylog goes back in time")]
    OutOfOrder,
    #[error("test did not finish")]
    Unfinished,
    #[error("duration does not match the mode")]
    WrongDuration,
    #[error("too fast to be typed")]
    TooFast,
}

/// Run `log` through a fresh engine over `words` in `mode` and score it.
pub fn replay(words: &[String], mode: Mode, log: &[KeyEvent]) -> Result<Metrics, Rejected> {
    if log.is_empty() {
        return Err(Rejected::Empty);
    }
    if log.windows(2).any(|w| w[1].ms < w[0].ms) {
        return Err(Rejected::OutOfOrder);
    }
    let mut engine = TestEngine::new(mode, Box::new(FixedGenerator::new(words.to_vec())));
    let t0 = Instant::now();
    for e in log {
        let at = t0 + Duration::from_millis(e.ms as u64);
        match e.key {
            Key::Char(c) => engine.type_char_at(c, at),
            Key::Backspace => engine.backspace(),
            Key::DeleteWord => engine.delete_word(),
            Key::Tick => {
                engine.tick(at);
            }
        }
    }
    if engine.status() != Status::Finished {
        return Err(Rejected::Unfinished);
    }
    let metrics = Metrics::from_engine(&engine);
    if let Mode::Time(secs) = mode {
        let limit = Duration::from_secs(secs as u64);
        if metrics.duration < limit || metrics.duration > limit + TIME_SLACK {
            return Err(Rejected::WrongDuration);
        }
    }
    if metrics.raw > MAX_RAW_WPM {
        return Err(Rejected::TooFast);
    }
    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split(' ').map(String::from).collect()
    }

    /// Type `text` at a steady pace on a recording engine; returns the
    /// engine after finishing.
    fn run(mode: Mode, list: &[String], text: &str, ms_per_key: u64) -> TestEngine {
        let mut e = TestEngine::new(mode, Box::new(FixedGenerator::new(list.to_vec())));
        e.record_keys();
        // Real clients pass arbitrary instants; add sub-millisecond noise
        // to prove the recording is what makes the replay exact.
        let t0 = Instant::now();
        let mut t = t0;
        for (i, c) in text.chars().enumerate() {
            t = t0 + Duration::from_millis(ms_per_key * i as u64) + Duration::from_micros(731);
            match c {
                '<' => e.backspace(),
                '^' => e.delete_word(),
                c => e.type_char_at(c, t),
            }
        }
        // Time mode: keep ticking until it ends.
        while e.status() == Status::Running {
            t += Duration::from_millis(100);
            e.tick(t);
        }
        e
    }

    #[test]
    fn replay_matches_recording_in_words_mode() {
        let list = words("the quick brown fox");
        let e = run(Mode::Words(4), &list, "the quikc<<ck brown fox", 90);
        let shown = Metrics::from_engine(&e);
        let scored = replay(&list, Mode::Words(4), e.keylog().unwrap()).unwrap();
        assert_eq!(shown, scored);
        assert!(shown.chars.incorrect > 0 || shown.accuracy < 100.0);
    }

    #[test]
    fn replay_matches_recording_in_time_mode() {
        let list: Vec<String> = (0..200).map(|i| format!("w{i}")).collect();
        let text: String = list.iter().map(|w| format!("{w} ")).collect();
        // 1 s test, 45 ms per key (~270 raw wpm): the tick ends it mid-text.
        let e = run(Mode::Time(1), &list, &text, 45);
        assert_eq!(e.status(), Status::Finished);
        let shown = Metrics::from_engine(&e);
        let log = e.keylog().unwrap();
        assert_eq!(log.last().unwrap().key, Key::Tick);
        let scored = replay(&list, Mode::Time(1), log).unwrap();
        assert_eq!(shown, scored);
        // The finishing tick, not the limit, sets the duration.
        assert!(shown.duration >= Duration::from_secs(1));
    }

    #[test]
    fn rejects_bad_logs() {
        let list = words("ab cd");
        let ev = |ms, key| KeyEvent { ms, key };
        assert_eq!(replay(&list, Mode::Words(2), &[]), Err(Rejected::Empty));
        let unfinished = [ev(0, Key::Char('a')), ev(50, Key::Char('b'))];
        assert_eq!(
            replay(&list, Mode::Words(2), &unfinished),
            Err(Rejected::Unfinished)
        );
        let backwards = [ev(50, Key::Char('a')), ev(0, Key::Char('b'))];
        assert_eq!(
            replay(&list, Mode::Words(2), &backwards),
            Err(Rejected::OutOfOrder)
        );
        let instant: Vec<KeyEvent> = "ab cd"
            .chars()
            .enumerate()
            .map(|(i, c)| ev(i as u32, Key::Char(c)))
            .collect();
        assert_eq!(
            replay(&list, Mode::Words(2), &instant),
            Err(Rejected::TooFast)
        );
        // Time mode must end at its limit, not whenever the log says.
        let late = [ev(0, Key::Char('a')), ev(5000, Key::Tick)];
        assert_eq!(
            replay(&list, Mode::Time(1), &late),
            Err(Rejected::WrongDuration)
        );
    }

    #[test]
    fn key_serde_shape() {
        let log = vec![
            KeyEvent {
                ms: 0,
                key: Key::Char('é'),
            },
            KeyEvent {
                ms: 3,
                key: Key::Backspace,
            },
            KeyEvent {
                ms: 3,
                key: Key::DeleteWord,
            },
            KeyEvent {
                ms: 9,
                key: Key::Tick,
            },
        ];
        let json = serde_json::to_string(&log).unwrap();
        assert_eq!(
            json,
            r#"[{"ms":0,"key":"é"},{"ms":3,"key":"backspace"},{"ms":3,"key":"delete_word"},{"ms":9,"key":"tick"}]"#
        );
        let back: Vec<KeyEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, log);
        assert!(serde_json::from_str::<Key>("\"nope\"").is_err());
    }
}

//! The landing screen's screensaver: after `IDLE_AFTER` without a key, a
//! ghost types random words in the big font (with the odd typo, fixed); a
//! key makes the text crumble and the landing screen comes back. Timing is
//! pure like the intro's: frames come from an `Instant`, the words from the
//! caller, and the script of keystrokes is made up front.

use std::time::{Duration, Instant};

use rand::RngExt;
use rand::seq::IndexedRandom;

/// Quiet time on the landing screen before the ghost starts typing.
pub const IDLE_AFTER: Duration = Duration::from_secs(5);
/// Quiet time on any other screen before ttyp goes back to the landing
/// screen (only when `splash` is on and nothing is under way).
pub const HOME_AFTER: Duration = Duration::from_secs(30);
/// How long the crumble plays before the landing screen is back.
pub const CRUMBLE: Duration = Duration::from_millis(1600);
/// Frame interval while something moves.
pub const FRAME: Duration = Duration::from_millis(33);
/// Longest phrase, in characters, so it fits the big font.
const PHRASE_CHARS: usize = 18;
/// Pause on a finished phrase before the next one.
const HOLD: Duration = Duration::from_millis(1400);

/// One keystroke of the ghost: from `at` on, `typed` characters are right
/// and `wrong` (if any) sits after them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    at: Duration,
    typed: usize,
    wrong: Option<char>,
}

/// What to draw: the phrase, how much of it is typed, and a typo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhostFrame {
    pub phrase: String,
    pub typed: usize,
    pub wrong: Option<char>,
}

#[derive(Debug, Clone)]
struct Ghost {
    started: Instant,
    phrase: String,
    steps: Vec<Step>,
}

impl Ghost {
    fn new(started: Instant, phrase: String, rng: &mut impl rand::Rng) -> Self {
        let mut steps = Vec::new();
        let mut at = Duration::from_millis(300);
        let chars: Vec<char> = phrase.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            at += Duration::from_millis(rng.random_range(55..150));
            // Now and then a wrong letter, a beat, a backspace.
            if c != ' ' && rng.random_bool(0.07) {
                let wrong = (b'a' + rng.random_range(0..26u8)) as char;
                if wrong != c {
                    steps.push(Step {
                        at,
                        typed: i,
                        wrong: Some(wrong),
                    });
                    at += Duration::from_millis(rng.random_range(220..380));
                    steps.push(Step {
                        at,
                        typed: i,
                        wrong: None,
                    });
                    at += Duration::from_millis(rng.random_range(90..160));
                }
            }
            steps.push(Step {
                at,
                typed: i + 1,
                wrong: None,
            });
        }
        Self {
            started,
            phrase,
            steps,
        }
    }

    fn frame_at(&self, now: Instant) -> GhostFrame {
        let t = now.saturating_duration_since(self.started);
        let step = self.steps.iter().take_while(|s| s.at <= t).last();
        GhostFrame {
            phrase: self.phrase.clone(),
            typed: step.map_or(0, |s| s.typed),
            wrong: step.and_then(|s| s.wrong),
        }
    }

    fn done_at(&self) -> Instant {
        self.started + self.steps.last().map_or(Duration::ZERO, |s| s.at) + HOLD
    }
}

#[derive(Debug, Clone)]
enum Scene {
    Waiting,
    Typing(Ghost),
    /// The frame a key interrupted, falling apart since `started`.
    Crumbling {
        started: Instant,
        frame: GhostFrame,
    },
}

/// What the landing screen shows instead of itself.
#[derive(Debug, Clone, PartialEq)]
pub enum IdleView {
    Typing(GhostFrame),
    /// `t` is how far into the crumble (0 to `CRUMBLE`).
    Crumbling {
        frame: GhostFrame,
        t: Duration,
    },
}

#[derive(Debug, Clone)]
pub struct Screensaver {
    last_input: Instant,
    scene: Scene,
}

impl Screensaver {
    pub fn new(now: Instant) -> Self {
        Self {
            last_input: now,
            scene: Scene::Waiting,
        }
    }

    /// A key that isn't waking the ghost: start the quiet time over.
    pub fn touch(&mut self, now: Instant) {
        self.last_input = now;
        self.scene = Scene::Waiting;
    }

    /// The ghost is typing, so a key should crumble it rather than act.
    pub fn typing(&self) -> bool {
        matches!(self.scene, Scene::Typing(_))
    }

    /// A key while the ghost types: freeze its frame and crumble it.
    pub fn wake(&mut self, now: Instant) {
        if let Scene::Typing(g) = &self.scene {
            self.scene = Scene::Crumbling {
                started: now,
                frame: g.frame_at(now),
            };
        }
    }

    /// Advance the scene. `words` is asked for a word list only when a new
    /// phrase is needed.
    pub fn tick<'w>(&mut self, now: Instant, words: impl FnOnce() -> &'w [String]) {
        let new_phrase = match &self.scene {
            Scene::Waiting => now.saturating_duration_since(self.last_input) >= IDLE_AFTER,
            Scene::Typing(g) => now >= g.done_at(),
            Scene::Crumbling { started, .. } => {
                if now.saturating_duration_since(*started) >= CRUMBLE {
                    self.touch(now);
                }
                false
            }
        };
        if new_phrase {
            let mut rng = rand::rng();
            let phrase = phrase(words(), &mut rng);
            self.scene = Scene::Typing(Ghost::new(now, phrase, &mut rng));
        }
    }

    pub fn view_at(&self, now: Instant) -> Option<IdleView> {
        match &self.scene {
            Scene::Waiting => None,
            Scene::Typing(g) => Some(IdleView::Typing(g.frame_at(now))),
            Scene::Crumbling { started, frame } => Some(IdleView::Crumbling {
                frame: frame.clone(),
                t: now.saturating_duration_since(*started).min(CRUMBLE),
            }),
        }
    }

    /// How long the event loop may sleep.
    pub fn next_change_at(&self, now: Instant) -> Duration {
        match self.scene {
            Scene::Waiting => IDLE_AFTER
                .saturating_sub(now.saturating_duration_since(self.last_input))
                .max(Duration::from_millis(1)),
            _ => FRAME,
        }
    }
}

/// A few random plain words, at most `PHRASE_CHARS` long in all. Words the
/// pixel font can't draw well (symbols, capitals, accents) are skipped.
fn phrase(words: &[String], rng: &mut impl rand::Rng) -> String {
    let plain: Vec<&String> = words
        .iter()
        .filter(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase()))
        .collect();
    let mut out = String::new();
    for _ in 0..8 {
        let Some(w) = plain.choose(rng) else { break };
        let extra = w.len() + usize::from(!out.is_empty());
        if out.len() + extra > PHRASE_CHARS {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    if out.is_empty() {
        out = super::splash::LOGO.to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn words() -> Vec<String> {
        ["alpha", "beta", "gamma", "Delta", "x+y", "a"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn waits_types_crumbles_and_comes_back() {
        let t0 = Instant::now();
        let w = words();
        let mut s = Screensaver::new(t0);
        s.tick(t0 + IDLE_AFTER - Duration::from_millis(1), || &w);
        assert!(s.view_at(t0).is_none());
        assert!(!s.typing());

        let start = t0 + IDLE_AFTER;
        s.tick(start, || &w);
        assert!(s.typing());
        let Some(IdleView::Typing(f)) = s.view_at(start + Duration::from_secs(10)) else {
            panic!("typing");
        };
        assert_eq!(f.typed, f.phrase.chars().count(), "finishes the phrase");

        let key = start + Duration::from_secs(1);
        s.wake(key);
        assert!(!s.typing());
        assert!(matches!(s.view_at(key), Some(IdleView::Crumbling { .. })));
        s.tick(key + CRUMBLE, || &w);
        assert!(s.view_at(key + CRUMBLE).is_none(), "landing is back");
        // And the quiet time starts over from there.
        s.tick(key + CRUMBLE + IDLE_AFTER - Duration::from_millis(1), || &w);
        assert!(!s.typing());
    }

    #[test]
    fn a_key_before_idle_restarts_the_wait() {
        let t0 = Instant::now();
        let w = words();
        let mut s = Screensaver::new(t0);
        s.touch(t0 + Duration::from_secs(4));
        s.tick(t0 + IDLE_AFTER, || &w);
        assert!(!s.typing());
        assert_eq!(s.next_change_at(t0 + IDLE_AFTER), Duration::from_secs(4));
    }

    #[test]
    fn phrases_are_plain_and_short() {
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..50 {
            let p = phrase(&words(), &mut rng);
            assert!(p.len() <= PHRASE_CHARS);
            assert!(p.chars().all(|c| c.is_ascii_lowercase() || c == ' '));
        }
        assert_eq!(phrase(&[], &mut rng), "ttyp");
    }

    #[test]
    fn typos_are_fixed_and_typing_only_moves_forward_overall() {
        let mut rng = StdRng::seed_from_u64(9);
        let g = Ghost::new(Instant::now(), "the quick brown".into(), &mut rng);
        for pair in g.steps.windows(2) {
            assert!(pair[0].at < pair[1].at);
            if pair[0].wrong.is_some() {
                assert_eq!((pair[1].typed, pair[1].wrong), (pair[0].typed, None));
            }
        }
        assert_eq!(g.steps.last().unwrap().typed, 15);
    }
}

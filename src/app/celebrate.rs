//! The personal-best confetti on the results screen (`celebrate = true`).
//! A short burst of sparks thrown up from the "new best" text and pulled
//! down by gravity. Pure like the landing intro: the sparks are rolled up
//! front from a seed, `frame_at` places them for an `Instant`, and
//! `next_change_at` tells the event loop when to wake until it's over.

use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// How long the burst plays.
pub const DURATION: Duration = Duration::from_millis(1500);
/// Frame interval while it plays.
pub const FRAME: Duration = Duration::from_millis(33);
/// How many sparks are thrown.
const SPARKS: usize = 44;
/// Downward pull, in rows per second².
const GRAVITY: f32 = 22.0;
/// Characters a spark is drawn with while it flies.
const GLYPHS: [char; 6] = ['*', '+', '·', '•', '✦', '°'];
/// What a spark turns into as it burns out.
const EMBER: char = '·';

/// Which theme colour a spark takes; the renderer maps it to the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    Main,
    Fg,
    Correct,
    ErrorExtra,
    Sub,
}

const TINTS: [Tint; 5] = [
    Tint::Main,
    Tint::Main,
    Tint::Correct,
    Tint::Fg,
    Tint::ErrorExtra,
];

/// One spark as rolled: where on the text it starts, how it's thrown, and
/// when it appears and burns out (seconds from the start).
#[derive(Debug, Clone, Copy)]
struct Spark {
    x0: f32,
    vx: f32,
    vy: f32,
    delay: f32,
    life: f32,
    glyph: char,
    tint: Tint,
}

/// A spark on screen: its cell relative to the middle of the "new best"
/// text (`dy` grows downward).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Particle {
    pub dx: i16,
    pub dy: i16,
    pub glyph: char,
    pub tint: Tint,
}

#[derive(Debug, Clone)]
pub struct Celebration {
    started: Instant,
    stopped: bool,
    sparks: Vec<Spark>,
}

impl Celebration {
    pub fn new(started: Instant, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let sparks = (0..SPARKS)
            .map(|i| {
                // Two waves: most at once, a few a beat later.
                let delay = if i % 4 == 3 {
                    rng.random_range(0.12..0.35)
                } else {
                    0.0
                };
                Spark {
                    x0: rng.random_range(-4.0..4.0),
                    // Cells are about twice as tall as wide, so throw
                    // further sideways than up.
                    vx: rng.random_range(-26.0..26.0),
                    vy: rng.random_range(-11.0..-2.0),
                    delay,
                    life: rng.random_range(0.8..(DURATION.as_secs_f32() - delay)),
                    glyph: GLYPHS[rng.random_range(0..GLYPHS.len())],
                    tint: TINTS[rng.random_range(0..TINTS.len())],
                }
            })
            .collect();
        Self {
            started,
            stopped: false,
            sparks,
        }
    }

    /// End the burst now (a key was pressed).
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    pub fn is_done_at(&self, now: Instant) -> bool {
        self.stopped || now.saturating_duration_since(self.started) >= DURATION
    }

    /// The sparks in flight at `now`; empty once it's over.
    pub fn frame_at(&self, now: Instant) -> Vec<Particle> {
        if self.is_done_at(now) {
            return Vec::new();
        }
        let t = now.saturating_duration_since(self.started).as_secs_f32();
        self.sparks
            .iter()
            .filter_map(|s| {
                let age = t - s.delay;
                if age < 0.0 || age >= s.life {
                    return None;
                }
                // Air drag slows the sideways flight; gravity wins in the end.
                let x = s.x0 + s.vx * (1.0 - (-2.2 * age).exp()) / 2.2;
                let y = s.vy * age + 0.5 * GRAVITY * age * age;
                let burning_out = age > s.life * 0.75;
                Some(Particle {
                    dx: x.round() as i16,
                    dy: y.round() as i16,
                    glyph: if burning_out { EMBER } else { s.glyph },
                    tint: if burning_out { Tint::Sub } else { s.tint },
                })
            })
            .collect()
    }

    /// How long until the next frame; `None` once it's over, so the event
    /// loop goes back to sleeping until a key.
    pub fn next_change_at(&self, now: Instant) -> Option<Duration> {
        if self.is_done_at(now) {
            return None;
        }
        let left = DURATION - now.saturating_duration_since(self.started);
        Some(FRAME.min(left))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn same_seed_same_frames() {
        let t0 = Instant::now();
        let a = Celebration::new(t0, 7);
        let b = Celebration::new(t0, 7);
        let c = Celebration::new(t0, 8);
        for n in [0, 100, 500, 1200] {
            assert_eq!(a.frame_at(t0 + ms(n)), b.frame_at(t0 + ms(n)));
        }
        assert_ne!(a.frame_at(t0 + ms(400)), c.frame_at(t0 + ms(400)));
    }

    #[test]
    fn starts_on_the_text_then_scatters() {
        let t0 = Instant::now();
        let c = Celebration::new(t0, 1);
        let first = c.frame_at(t0 + ms(1));
        assert!(!first.is_empty());
        for p in &first {
            assert!(p.dx.abs() <= 5 && p.dy.abs() <= 1, "{p:?}");
        }
        let later = c.frame_at(t0 + ms(400));
        let spread = later.iter().map(|p| p.dx.abs()).max().unwrap();
        assert!(spread > 5, "sparks fly sideways: {spread}");
    }

    #[test]
    fn gravity_pulls_them_down() {
        let t0 = Instant::now();
        let c = Celebration::new(t0, 2);
        let mean_dy = |n| {
            let f = c.frame_at(t0 + ms(n));
            f.iter().map(|p| p.dy as f32).sum::<f32>() / f.len() as f32
        };
        // Thrown up first, then falling below where they started.
        assert!(mean_dy(150) < 0.0);
        assert!(mean_dy(1100) > mean_dy(400));
        assert!(mean_dy(1100) > 0.0);
    }

    #[test]
    fn uses_only_the_listed_glyphs() {
        let t0 = Instant::now();
        let c = Celebration::new(t0, 3);
        for n in (0..1500).step_by(50) {
            for p in c.frame_at(t0 + ms(n)) {
                assert!(GLYPHS.contains(&p.glyph), "{p:?}");
            }
        }
    }

    #[test]
    fn sparks_burn_out_to_embers() {
        let t0 = Instant::now();
        let c = Celebration::new(t0, 4);
        let late = c.frame_at(t0 + ms(1450));
        assert!(late.iter().all(|p| p.glyph == EMBER && p.tint == Tint::Sub));
    }

    #[test]
    fn ends_after_its_duration_and_stops_waking() {
        let t0 = Instant::now();
        let c = Celebration::new(t0, 5);
        assert_eq!(c.next_change_at(t0), Some(FRAME));
        assert_eq!(c.next_change_at(t0 + ms(1490)), Some(ms(10)));
        assert!(c.frame_at(t0 + DURATION).is_empty());
        assert_eq!(c.next_change_at(t0 + DURATION), None);
        assert!(c.is_done_at(t0 + DURATION));
        // Waking where it says always ends, in about DURATION / FRAME steps.
        let mut now = t0;
        let mut wakes = 0;
        while let Some(d) = c.next_change_at(now) {
            now += d;
            wakes += 1;
        }
        assert_eq!(
            wakes,
            DURATION.as_millis().div_ceil(FRAME.as_millis()) as usize
        );
    }

    #[test]
    fn stop_ends_it_at_once() {
        let t0 = Instant::now();
        let mut c = Celebration::new(t0, 6);
        c.stop();
        assert!(c.frame_at(t0 + ms(100)).is_empty());
        assert_eq!(c.next_change_at(t0 + ms(100)), None);
    }
}

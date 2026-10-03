//! Which keys a finished test went wrong on, for the results screen's
//! keyboard heatmap and the `:missed` screen. Pure: counted from the
//! engine's words (`typed` against `target`), so it never touches scoring
//! or the keylog.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};

use crate::config::Keyboard;
use crate::stats::TestRecord;
use crate::test::engine::Word;

/// Mistyped target characters, as they were in the words. Which key each
/// one is on depends on the keyboard (`on`), so changing the layout on the
/// results screen redraws the same run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Misses {
    pub chars: BTreeMap<char, u32>,
    /// How often each target character was typed at all, right or wrong:
    /// what a miss rate is out of.
    pub typed: BTreeMap<char, u32>,
}

impl Misses {
    /// Count every target character that was typed as something else.
    /// Characters not typed yet (a time test's last word, a skipped word's
    /// tail) and extra characters past the end of a word have no target
    /// and aren't counted.
    pub fn from_words(words: &[Word]) -> Self {
        let mut m = Self::default();
        for w in words {
            for (target, typed) in w.target.iter().zip(&w.typed) {
                *m.typed.entry(*target).or_default() += 1;
                if target != typed {
                    *m.chars.entry(*target).or_default() += 1;
                }
            }
        }
        m
    }

    /// The misses of `runs` added up (pick them with `MissedRange::runs`).
    /// Runs from before misses were recorded add nothing.
    pub fn from_records<'a>(runs: impl IntoIterator<Item = &'a TestRecord>) -> Self {
        let mut m = Self::default();
        for r in runs {
            for (c, n) in &r.missed_chars {
                *m.chars.entry(*c).or_default() += n;
            }
            for (c, n) in &r.typed_chars {
                *m.typed.entry(*c).or_default() += n;
            }
        }
        m
    }

    pub fn total(&self) -> u32 {
        self.chars.values().sum()
    }

    /// The misses per key of `kb`.
    pub fn on(&self, kb: Keyboard) -> KeyMisses {
        let mut m = KeyMisses {
            keyboard: kb,
            ..KeyMisses::default()
        };
        for (c, n) in &self.chars {
            match kb.key_for(*c) {
                Some(k) => *m.keys.entry(k).or_default() += n,
                None => m.other += n,
            }
        }
        for (c, n) in &self.typed {
            if let Some(k) = kb.key_for(*c) {
                *m.typed.entry(k).or_default() += n;
            }
        }
        m
    }
}

/// How far back the `:missed` screen looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MissedRange {
    /// The newest run, whether or not it counts.
    Last,
    Day,
    #[default]
    Week,
    Month,
    All,
}

impl MissedRange {
    pub const ALL: [MissedRange; 5] = [
        MissedRange::Last,
        MissedRange::Day,
        MissedRange::Week,
        MissedRange::Month,
        MissedRange::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MissedRange::Last => "last test",
            MissedRange::Day => "last day",
            MissedRange::Week => "7 days",
            MissedRange::Month => "30 days",
            MissedRange::All => "all time",
        }
    }

    /// The `:missed` argument for it.
    pub fn arg(self) -> &'static str {
        match self {
            MissedRange::Last => "last",
            MissedRange::Day => "day",
            MissedRange::Week => "week",
            MissedRange::Month => "month",
            MissedRange::All => "all",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "last" | "test" | "last test" => Ok(MissedRange::Last),
            "day" | "1" | "1d" | "today" | "24h" => Ok(MissedRange::Day),
            "week" | "7" | "7d" => Ok(MissedRange::Week),
            "month" | "30" | "30d" => Ok(MissedRange::Month),
            "all" | "all time" | "alltime" | "ever" => Ok(MissedRange::All),
            _ => Err(format!("expected last, day, week, month or all, got `{s}`")),
        }
    }

    /// The runs of `records` it covers, counting back from `now`: the
    /// newest one for `Last`, else every run that counts since the cutoff.
    pub fn runs(self, records: &[TestRecord], now: DateTime<Utc>) -> Vec<&TestRecord> {
        let days = match self {
            MissedRange::Last => return records.iter().max_by_key(|r| r.ts).into_iter().collect(),
            MissedRange::Day => 1,
            MissedRange::Week => 7,
            MissedRange::Month => 30,
            MissedRange::All => {
                return records.iter().filter(|r| r.counts()).collect();
            }
        };
        let since = now - Duration::days(days);
        records
            .iter()
            .filter(|r| r.counts() && r.ts >= since)
            .collect()
    }

    /// The next range, `by` steps on, wrapping.
    pub fn step(self, by: i8) -> Self {
        let n = Self::ALL.len() as i8;
        let i = Self::ALL.iter().position(|r| *r == self).unwrap_or(0) as i8;
        Self::ALL[(i + by).rem_euclid(n) as usize]
    }
}

/// Mistyped characters per key of one keyboard, keyed by the key's
/// unshifted character (on QWERTY `A` and `a` are both `a`, `!` is `1`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyMisses {
    pub keyboard: Keyboard,
    pub keys: BTreeMap<char, u32>,
    /// How often each key's characters were typed, right or wrong.
    pub typed: BTreeMap<char, u32>,
    /// Misses on characters that aren't on the drawn keyboard (accents,
    /// other scripts, space).
    pub other: u32,
}

impl KeyMisses {
    pub fn get(&self, key: char) -> u32 {
        self.keys.get(&key).copied().unwrap_or(0)
    }

    pub fn total(&self) -> u32 {
        self.keys.values().sum::<u32>() + self.other
    }

    /// The share of `key`'s characters that were missed, 0–100.
    pub fn percent(&self, key: char) -> Option<f64> {
        let typed = self.typed.get(&key).copied().unwrap_or(0);
        (typed > 0).then(|| f64::from(self.get(key)) * 100.0 / f64::from(typed))
    }

    /// Most misses on one key.
    pub fn max(&self) -> u32 {
        self.keys.values().copied().max().unwrap_or(0)
    }

    pub fn on_number_row(&self) -> bool {
        self.keyboard.rows()[0].0.chars().any(|k| self.get(k) > 0)
    }
}

/// Columns one key takes on screen: ` q ` plus a gap.
pub const KEY_COLS: u16 = 4;
/// How far each drawn row is shifted right, in quarter keys, so the rows
/// stagger like a real keyboard (number row first).
const ROW_SHIFT: [u16; 4] = [0, 2, 3, 5];

/// How big each key is drawn: `width` × `height` cells, one every `pitch`
/// columns and every `row_pitch` rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySize {
    pub pitch: u16,
    pub width: u16,
    pub height: u16,
    pub row_pitch: u16,
}

/// The results screen's keys: ` q ` on one row.
pub const SMALL: KeySize = KeySize {
    pitch: KEY_COLS,
    width: KEY_COLS - 1,
    height: 1,
    row_pitch: 1,
};

/// The `:missed` screen's sizes, biggest first; it takes the first that
/// fits. Keys two or more rows tall have room for their miss rate under
/// the letter.
pub const SIZES: [KeySize; 5] = [
    KeySize {
        pitch: 10,
        width: 9,
        height: 3,
        row_pitch: 4,
    },
    KeySize {
        pitch: 8,
        width: 7,
        height: 3,
        row_pitch: 4,
    },
    KeySize {
        pitch: 6,
        width: 5,
        height: 3,
        row_pitch: 4,
    },
    KeySize {
        pitch: 5,
        width: 4,
        height: 2,
        row_pitch: 3,
    },
    SMALL,
];

/// Where each key of `kb` is drawn: (key, column, row) from the
/// keyboard's top left. The number row is only drawn when `numbers` is set; without it the
/// letter rows move up.
pub fn layout(kb: Keyboard, numbers: bool) -> Vec<(char, u16, u16)> {
    layout_sized(kb, numbers, SMALL)
}

/// `layout` with keys of `size`.
pub fn layout_sized(kb: Keyboard, numbers: bool, size: KeySize) -> Vec<(char, u16, u16)> {
    let rows = kb
        .rows()
        .into_iter()
        .zip(ROW_SHIFT)
        .map(|((keys, _), shift)| (keys, shift * size.pitch / 4))
        .skip(usize::from(!numbers));
    rows.enumerate()
        .flat_map(|(y, (keys, shift))| {
            keys.chars()
                .enumerate()
                .map(move |(x, k)| (k, shift + x as u16 * size.pitch, y as u16 * size.row_pitch))
        })
        .collect()
}

/// Width and height of the drawn keyboard.
pub fn layout_size(kb: Keyboard, numbers: bool) -> (u16, u16) {
    layout_size_sized(kb, numbers, SMALL)
}

/// `layout_size` with keys of `size`.
pub fn layout_size_sized(kb: Keyboard, numbers: bool, size: KeySize) -> (u16, u16) {
    let cells = layout_sized(kb, numbers, size);
    let w = cells
        .iter()
        .map(|(_, x, _)| x + size.width)
        .max()
        .unwrap_or(0);
    let h = cells
        .iter()
        .map(|(_, _, y)| y + size.height)
        .max()
        .unwrap_or(0);
    (w, h)
}

impl KeyMisses {
    /// The worst keys, most misses first (alphabetical on a tie).
    pub fn worst(&self, n: usize) -> Vec<(char, u32)> {
        let mut v: Vec<(char, u32)> = self.keys.iter().map(|(k, c)| (*k, *c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }
}

/// How hot a key is drawn: 0 clean, then 1–3 by its share of the worst
/// key's count (3 is the worst).
pub fn heat(count: u32, max: u32) -> u8 {
    if count == 0 || max == 0 {
        return 0;
    }
    match count * 3 {
        n if n <= max => 1,
        n if n <= max * 2 => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qwerty(words: &[Word]) -> KeyMisses {
        Misses::from_words(words).on(Keyboard::Qwerty)
    }

    fn word(target: &str, typed: &str) -> Word {
        Word {
            target: target.chars().collect(),
            typed: typed.chars().collect(),
        }
    }

    #[test]
    fn counts_wrong_target_characters() {
        let m = qwerty(&[
            word("the", "teh"),
            word("quick", "quick"),
            word("brown", "briwn"),
            word("hat", "hag"),
        ]);
        assert_eq!(m.get('h'), 1);
        assert_eq!(m.get('e'), 1);
        assert_eq!(m.get('o'), 1);
        assert_eq!(m.get('t'), 1);
        assert_eq!(m.get('i'), 0, "the key that was hit isn't the miss");
        assert_eq!(m.total(), 4);
        assert_eq!(m.max(), 1);
    }

    #[test]
    fn untyped_and_extra_characters_are_not_misses() {
        let m = qwerty(&[word("about", "ab"), word("go", "gooo"), word("later", "")]);
        assert!(m.keys.is_empty());
        assert_eq!(m.other, 0);
        assert_eq!(Misses::from_words(&[word("go", "gooo")]).total(), 0);
        assert_eq!(m.total(), 0);
    }

    #[test]
    fn shifted_characters_count_on_their_key() {
        let m = qwerty(&[word("Hello!", "hello1"), word("x?", "x/")]);
        assert_eq!(m.get('h'), 1);
        assert_eq!(m.get('1'), 1);
        assert_eq!(m.get('/'), 1);
        assert!(m.on_number_row());
    }

    #[test]
    fn characters_off_the_keyboard_are_counted_apart() {
        let m = qwerty(&[word("café", "cafe")]);
        assert_eq!(m.other, 1);
        assert!(m.keys.is_empty());
        assert_eq!(m.total(), 1);
    }

    #[test]
    fn repeated_misses_add_up() {
        let m = qwerty(&[word("aaa", "sss"), word("ab", "sb")]);
        assert_eq!(m.get('a'), 4);
        assert_eq!(m.max(), 4);
        assert!(!m.on_number_row());
    }

    #[test]
    fn heat_scales_to_the_worst_key() {
        assert_eq!(heat(0, 0), 0);
        assert_eq!(heat(0, 5), 0);
        assert_eq!(heat(1, 1), 3);
        assert_eq!(heat(1, 6), 1);
        assert_eq!(heat(2, 6), 1);
        assert_eq!(heat(3, 6), 2);
        assert_eq!(heat(4, 6), 2);
        assert_eq!(heat(5, 6), 3);
        assert_eq!(heat(6, 6), 3);
    }

    #[test]
    fn keyboard_layout_staggers_rows() {
        let letters = layout(Keyboard::Qwerty, false);
        assert_eq!(letters.len(), 12 + 11 + 10);
        let at = |cells: &[(char, u16, u16)], k| *cells.iter().find(|c| c.0 == k).unwrap();
        assert_eq!(at(&letters, 'q'), ('q', 2, 0));
        assert_eq!(at(&letters, 'w'), ('w', 6, 0));
        assert_eq!(at(&letters, 'a'), ('a', 3, 1));
        assert_eq!(at(&letters, 'z'), ('z', 5, 2));
        let all = layout(Keyboard::Qwerty, true);
        assert_eq!(at(&all, '1'), ('1', 0, 0));
        assert_eq!(at(&all, 'q'), ('q', 2, 1));
        assert_eq!(layout_size(Keyboard::Qwerty, false), (2 + 11 * 4 + 3, 3));
        assert_eq!(layout_size(Keyboard::Qwerty, true), (2 + 11 * 4 + 3, 4));
    }

    #[test]
    fn worst_keys_first() {
        let m = qwerty(&[word("aab", "ssc"), word("eed", "ffx")]);
        assert_eq!(m.worst(2), vec![('a', 2), ('e', 2)]);
        assert_eq!(m.worst(10).len(), 4);
    }

    #[test]
    fn the_same_run_lands_on_other_layouts() {
        let m = Misses::from_words(&[word("rst;", "rrt:")]);
        let q = m.on(Keyboard::Qwerty);
        assert_eq!((q.get('r'), q.get('s'), q.get(';')), (0, 1, 1));
        let c = m.on(Keyboard::Colemak);
        assert_eq!(c.get('s'), 1);
        assert_eq!(c.get(';'), 1);
        let d = m.on(Keyboard::DvorakProgrammer);
        assert_eq!(d.get(';'), 1, "`:` is shifted `;` there too");
        let at = |kb, k| *layout(kb, false).iter().find(|c| c.0 == k).unwrap();
        assert_eq!(at(Keyboard::Dvorak, 'a'), ('a', 3, 1));
        assert_eq!(at(Keyboard::ColemakDh, 'x'), ('x', 5, 2));
    }

    #[test]
    fn number_row_follows_the_layout() {
        let m = Misses::from_words(&[word("7", "8")]);
        assert!(m.on(Keyboard::Qwerty).on_number_row());
        let dp = m.on(Keyboard::DvorakProgrammer);
        assert_eq!(dp.get('['), 1);
        assert!(dp.on_number_row());
    }

    #[test]
    fn typed_counts_give_the_miss_rate() {
        let m = qwerty(&[word("aab", "sab"), word("Ab", "Ab")]);
        assert_eq!(m.typed.get(&'a'), Some(&3));
        assert_eq!(m.percent('a'), Some(100.0 / 3.0));
        assert_eq!(m.percent('b'), Some(0.0));
        assert_eq!(m.percent('z'), None, "never typed");
    }

    #[test]
    fn history_adds_up_counted_runs_in_range() {
        let now = Utc::now();
        let run = |days_ago: i64, missed: &[(char, u32)], typed: &[(char, u32)]| {
            let mut r: TestRecord = serde_json::from_str(
                r#"{"schema":4,"ts":"2026-01-01T00:00:00Z","mode":{"time":30},"language":"english",
                "punctuation":false,"numbers":false,"wpm":80.0,"raw":82.0,"acc":97.0,
                "consistency":80.0,"chars":{"correct":1,"incorrect":0,"extra":0,"missed":0},
                "duration_s":30.0}"#,
            )
            .unwrap();
            r.ts = now - Duration::days(days_ago);
            r.missed_chars = missed.iter().copied().collect();
            r.typed_chars = typed.iter().copied().collect();
            r
        };
        let mut invalid = run(0, &[('e', 50)], &[('e', 50)]);
        invalid.invalid = Some(crate::stats::Invalid::Afk);
        let records = vec![
            run(40, &[('e', 1)], &[('e', 10)]),
            run(3, &[('e', 2), ('T', 1)], &[('e', 10), ('t', 4), ('T', 1)]),
            run(0, &[], &[('e', 5)]),
            invalid,
        ];
        let all = Misses::from_records(MissedRange::All.runs(&records, now));
        assert_eq!(all.chars.get(&'e'), Some(&3));
        assert_eq!(all.typed.get(&'e'), Some(&25));
        let week = Misses::from_records(MissedRange::Week.runs(&records, now)).on(Keyboard::Qwerty);
        assert_eq!(week.get('e'), 2);
        assert_eq!(week.get('t'), 1, "`T` lands on the t key");
        assert_eq!(week.percent('t'), Some(20.0));
        let day = Misses::from_records(MissedRange::Day.runs(&records, now));
        assert_eq!(day.total(), 0);
        assert_eq!(day.typed.get(&'e'), Some(&5));
        // The newest run (`invalid`, 0 days ago, last in the list), counted or not.
        let last = MissedRange::Last.runs(&records, now);
        assert_eq!(last.len(), 1);
        assert_eq!(Misses::from_records(last).chars.get(&'e'), Some(&50));
        assert!(MissedRange::Last.runs(&[], now).is_empty());
    }

    #[test]
    fn ranges_parse_and_cycle() {
        for r in MissedRange::ALL {
            assert_eq!(MissedRange::parse(r.arg()), Ok(r));
        }
        assert_eq!(MissedRange::parse("30d"), Ok(MissedRange::Month));
        assert!(MissedRange::parse("year").is_err());
        assert_eq!(MissedRange::All.step(1), MissedRange::Last);
        assert_eq!(MissedRange::Last.step(-1), MissedRange::All);
        assert_eq!(MissedRange::Week.step(1), MissedRange::Month);
    }

    #[test]
    fn bigger_keys_keep_the_stagger() {
        let big = SIZES[0];
        let at = |k| {
            *layout_sized(Keyboard::Qwerty, false, big)
                .iter()
                .find(|c| c.0 == k)
                .unwrap()
        };
        assert_eq!(at('q'), ('q', 5, 0));
        assert_eq!(at('a'), ('a', 7, 4));
        assert_eq!(at('z'), ('z', 12, 8));
        assert_eq!(
            layout_size_sized(Keyboard::Qwerty, false, big),
            (5 + 11 * 10 + 9, 11)
        );
        assert_eq!(
            layout_size_sized(Keyboard::Qwerty, true, big),
            (5 + 11 * 10 + 9, 15)
        );
        assert_eq!(
            layout_size_sized(Keyboard::Qwerty, true, SMALL),
            layout_size(Keyboard::Qwerty, true)
        );
    }
}

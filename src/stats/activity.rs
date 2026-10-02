//! Activity over time: tests per local day, streaks, totals and a daily wpm
//! trend. Pure functions over `TestRecord`s; the caller passes the time zone
//! and "today" so tests can use fixed dates.

use std::collections::BTreeMap;

use chrono::{Datelike, Days, NaiveDate, TimeZone};

use super::TestRecord;

/// What happened on one local day.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Day {
    pub tests: usize,
    /// Seconds typed.
    pub secs: f64,
    pub wpm_sum: f64,
    pub best_wpm: f64,
}

impl Day {
    pub fn avg_wpm(&self) -> f64 {
        if self.tests == 0 {
            0.0
        } else {
            self.wpm_sum / self.tests as f64
        }
    }
}

/// Everything the stats screen's activity view shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Activity {
    pub today: NaiveDate,
    pub days: BTreeMap<NaiveDate, Day>,
    /// Consecutive days with a test, ending today (or yesterday, so a streak
    /// doesn't read 0 before today's first test).
    pub current_streak: u32,
    pub longest_streak: u32,
    pub tests: usize,
    pub secs: f64,
    /// The day with the most tests (the latest such day on a tie).
    pub best_day: Option<(NaiveDate, usize)>,
}

impl Activity {
    pub fn from_records<Tz: TimeZone>(records: &[TestRecord], tz: &Tz, today: NaiveDate) -> Self {
        let records: Vec<TestRecord> = records.iter().filter(|r| r.counts()).cloned().collect();
        let records = records.as_slice();
        let days = by_day(records, tz);
        let best_day = days
            .iter()
            .map(|(d, s)| (*d, s.tests))
            .max_by_key(|(d, n)| (*n, *d));
        Self {
            today,
            current_streak: current_streak(&days, today),
            longest_streak: longest_streak(&days),
            tests: records.len(),
            secs: records.iter().map(|r| r.duration_s.max(0.0)).sum(),
            best_day,
            days,
        }
    }

    pub fn tests_on(&self, date: NaiveDate) -> usize {
        self.days.get(&date).map_or(0, |d| d.tests)
    }

    /// Most tests on any single day in `from..=to`.
    pub fn max_tests_between(&self, from: NaiveDate, to: NaiveDate) -> usize {
        self.days
            .range(from..=to)
            .map(|(_, d)| d.tests)
            .max()
            .unwrap_or(0)
    }

    /// Average wpm for each of the last `n` days, oldest first, ending today.
    pub fn trend(&self, n: usize) -> Vec<Option<f64>> {
        (0..n)
            .rev()
            .map(|back| {
                let d = self.today.checked_sub_days(Days::new(back as u64))?;
                self.days.get(&d).map(Day::avg_wpm)
            })
            .collect()
    }
}

/// Records grouped by their local date in `tz`.
pub fn by_day<Tz: TimeZone>(records: &[TestRecord], tz: &Tz) -> BTreeMap<NaiveDate, Day> {
    let mut days: BTreeMap<NaiveDate, Day> = BTreeMap::new();
    for r in records {
        let date = r.ts.with_timezone(tz).date_naive();
        let d = days.entry(date).or_default();
        d.tests += 1;
        d.secs += r.duration_s.max(0.0);
        d.wpm_sum += r.wpm;
        d.best_wpm = d.best_wpm.max(r.wpm);
    }
    days
}

/// Days in a row with a test, ending today, or ending yesterday when today
/// has none yet.
pub fn current_streak(days: &BTreeMap<NaiveDate, Day>, today: NaiveDate) -> u32 {
    let mut d = today;
    if !days.contains_key(&d) {
        match d.pred_opt() {
            Some(y) => d = y,
            None => return 0,
        }
    }
    let mut n = 0;
    while days.contains_key(&d) {
        n += 1;
        match d.pred_opt() {
            Some(p) => d = p,
            None => break,
        }
    }
    n
}

pub fn longest_streak(days: &BTreeMap<NaiveDate, Day>) -> u32 {
    let mut best = 0;
    let mut run = 0;
    let mut prev: Option<NaiveDate> = None;
    for &d in days.keys() {
        run = match prev {
            Some(p) if p.succ_opt() == Some(d) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(d);
    }
    best
}

/// Shade level 0–4 for `tests` on a day, relative to the busiest day shown.
pub fn level(tests: usize, max: usize) -> u8 {
    if tests == 0 || max == 0 {
        0
    } else {
        (tests * 4).div_ceil(max).clamp(1, 4) as u8
    }
}

/// The Monday starting the leftmost of `weeks` calendar columns whose last
/// column holds `today`.
pub fn calendar_start(today: NaiveDate, weeks: usize) -> NaiveDate {
    let monday = today - Days::new(today.weekday().num_days_from_monday() as u64);
    monday - Days::new(7 * weeks.saturating_sub(1) as u64)
}

/// Sparkline glyphs for `values`, scaled between their min and max; `None`
/// stays `None` (a day without tests).
pub fn spark_levels(values: &[Option<f64>]) -> Vec<Option<u8>> {
    let present = values.iter().flatten();
    let lo = present.clone().copied().fold(f64::INFINITY, f64::min);
    let hi = present.copied().fold(f64::NEG_INFINITY, f64::max);
    values
        .iter()
        .map(|v| {
            v.map(|v| {
                if hi - lo < 1e-9 {
                    3
                } else {
                    (((v - lo) / (hi - lo)) * 7.0).round() as u8
                }
            })
        })
        .collect()
}

/// Month names as the calendar labels them.
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Labels over the calendar's week columns: the week each month starts in
/// (its first Monday on or after the 1st, or the first column for the
/// month already under way), as (column, name). A label that would sit
/// within `min_gap` columns of the previous one is dropped, unless the
/// previous one is the leading month cut off at the left edge: that one
/// gives way instead.
pub fn month_labels(start: NaiveDate, weeks: usize, min_gap: usize) -> Vec<(usize, &'static str)> {
    let mut out: Vec<(usize, &'static str)> = Vec::new();
    let mut prev_month = None;
    for w in 0..weeks {
        let Some(monday) = start.checked_add_days(Days::new(7 * w as u64)) else {
            break;
        };
        let m = monday.month0();
        if prev_month != Some(m) {
            match out.last() {
                Some((c, _)) if w < c + min_gap => {
                    if *c == 0 && start.day() > 7 {
                        out.pop();
                        out.push((w, MONTHS[m as usize]));
                    }
                }
                _ => out.push((w, MONTHS[m as usize])),
            }
            prev_month = Some(m);
        }
    }
    out
}

/// `sep 14`.
pub fn short_date(d: NaiveDate) -> String {
    format!("{} {}", MONTHS[d.month0() as usize], d.day())
}

/// `1h 05m`, `12m`, `45s`.
pub fn format_duration(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    match (s / 3600, (s % 3600) / 60) {
        (0, 0) => format!("{s}s"),
        (0, m) => format!("{m}m"),
        (h, m) => format!("{h}h {m:02}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::CharRecord;
    use crate::test::mode::Mode;
    use chrono::{FixedOffset, Utc};

    fn at(ts: &str, wpm: f64) -> TestRecord {
        TestRecord {
            schema: 2,
            ts: ts.parse::<chrono::DateTime<Utc>>().unwrap(),
            mode: Mode::Time(30),
            language: "english".into(),
            punctuation: false,
            numbers: false,
            wpm,
            raw: wpm,
            acc: 95.0,
            consistency: 70.0,
            chars: CharRecord {
                correct: 1,
                incorrect: 0,
                extra: 0,
                missed: 0,
            },
            duration_s: 30.0,
            daily_id: None,
            invalid: None,
            missed_chars: Default::default(),
            typed_chars: Default::default(),
        }
    }

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn days_follow_the_local_zone() {
        // 23:30 UTC on the 1st is the 2nd in UTC+2 and still the 1st in UTC-5.
        let rs = [at("2026-03-01T23:30:00Z", 50.0)];
        let east = by_day(&rs, &FixedOffset::east_opt(2 * 3600).unwrap());
        let west = by_day(&rs, &FixedOffset::west_opt(5 * 3600).unwrap());
        assert!(east.contains_key(&date("2026-03-02")));
        assert!(west.contains_key(&date("2026-03-01")));
    }

    #[test]
    fn streaks_and_totals() {
        let rs = [
            at("2026-01-01T10:00:00Z", 40.0),
            at("2026-01-02T10:00:00Z", 50.0),
            at("2026-01-03T10:00:00Z", 60.0),
            at("2026-01-03T11:00:00Z", 80.0),
            // gap on the 4th
            at("2026-01-05T10:00:00Z", 70.0),
            at("2026-01-06T10:00:00Z", 70.0),
        ];
        let a = Activity::from_records(&rs, &Utc, date("2026-01-06"));
        assert_eq!(a.current_streak, 2);
        assert_eq!(a.longest_streak, 3);
        assert_eq!(a.tests, 6);
        assert_eq!(a.secs, 180.0);
        assert_eq!(a.best_day, Some((date("2026-01-03"), 2)));
        assert_eq!(a.days[&date("2026-01-03")].avg_wpm(), 70.0);
        assert_eq!(a.days[&date("2026-01-03")].best_wpm, 80.0);

        // Nothing today yet: yesterday's streak still counts.
        let a = Activity::from_records(&rs, &Utc, date("2026-01-07"));
        assert_eq!(a.current_streak, 2);
        // A day missed: the streak is over.
        let a = Activity::from_records(&rs, &Utc, date("2026-01-08"));
        assert_eq!(a.current_streak, 0);
        assert_eq!(a.longest_streak, 3);

        let empty = Activity::from_records(&[], &Utc, date("2026-01-08"));
        assert_eq!((empty.current_streak, empty.longest_streak), (0, 0));
        assert_eq!(empty.best_day, None);
    }

    #[test]
    fn best_day_tie_goes_to_the_latest() {
        let rs = [
            at("2026-01-01T10:00:00Z", 40.0),
            at("2026-01-09T10:00:00Z", 40.0),
        ];
        let a = Activity::from_records(&rs, &Utc, date("2026-01-10"));
        assert_eq!(a.best_day, Some((date("2026-01-09"), 1)));
    }

    #[test]
    fn trend_is_daily_average_oldest_first() {
        let rs = [
            at("2026-01-08T10:00:00Z", 40.0),
            at("2026-01-10T10:00:00Z", 60.0),
            at("2026-01-10T12:00:00Z", 80.0),
        ];
        let a = Activity::from_records(&rs, &Utc, date("2026-01-10"));
        assert_eq!(a.trend(3), vec![Some(40.0), None, Some(70.0)]);
        assert_eq!(spark_levels(&a.trend(3)), vec![Some(0), None, Some(7)]);
        assert_eq!(
            spark_levels(&[Some(5.0), Some(5.0)]),
            vec![Some(3), Some(3)]
        );
        assert_eq!(spark_levels(&[None]), vec![None]);
    }

    #[test]
    fn levels_and_calendar() {
        assert_eq!(level(0, 10), 0);
        assert_eq!(level(1, 10), 1);
        assert_eq!(level(3, 10), 2);
        assert_eq!(level(6, 10), 3);
        assert_eq!(level(10, 10), 4);
        assert_eq!(level(1, 1), 4);
        // 2026-10-01 is a Thursday; its week starts Monday 2026-09-28.
        assert_eq!(calendar_start(date("2026-10-01"), 1), date("2026-09-28"));
        assert_eq!(calendar_start(date("2026-10-01"), 52), date("2025-10-06"));
        assert_eq!(calendar_start(date("2026-09-28"), 2), date("2026-09-21"));
        let rs = [
            at("2026-09-01T10:00:00Z", 40.0),
            at("2026-09-01T11:00:00Z", 40.0),
            at("2026-08-01T10:00:00Z", 40.0),
        ];
        let a = Activity::from_records(&rs, &Utc, date("2026-10-01"));
        assert_eq!(a.tests_on(date("2026-09-01")), 2);
        assert_eq!(
            a.max_tests_between(date("2026-08-02"), date("2026-10-01")),
            2
        );
        assert_eq!(
            a.max_tests_between(date("2026-07-01"), date("2026-08-31")),
            1
        );
    }

    #[test]
    fn month_labels_mark_where_months_start() {
        // Mondays: 2026-08-24, 08-31, 09-07, 09-14, 09-21, 09-28, 10-05.
        let start = date("2026-08-24");
        assert_eq!(
            month_labels(start, 7, 2),
            vec![(0, "aug"), (2, "sep"), (6, "oct")]
        );
        // The cut-off month at the left edge gives way to the next.
        assert_eq!(month_labels(start, 7, 3), vec![(2, "sep"), (6, "oct")]);
        // Otherwise a label too close to the previous one is dropped.
        assert_eq!(month_labels(date("2026-09-07"), 5, 5), vec![(0, "sep")]);
        // A year of columns names every month once (or twice for the one
        // at both ends).
        let year = month_labels(calendar_start(date("2026-10-01"), 52), 52, 3);
        assert!(year.len() >= 12 && year.len() <= 13, "{year:?}");
        assert_eq!(short_date(date("2026-09-14")), "sep 14");
    }

    #[test]
    fn durations() {
        assert_eq!(format_duration(45.0), "45s");
        assert_eq!(format_duration(12.0 * 60.0), "12m");
        assert_eq!(format_duration(3600.0 + 5.0 * 60.0), "1h 05m");
    }
}

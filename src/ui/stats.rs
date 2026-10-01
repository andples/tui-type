//! History screen: an activity calendar with streaks and a 30-day wpm
//! trend, summary numbers, and a scrollable table of recent tests.

use chrono::{Days, Local};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column, mix};
use super::widgets::{Cell, Column, Row, SelectTable, Width, hints};
use crate::app::App;
use crate::stats::activity::{
    self, Activity, calendar_start, format_duration, month_labels, short_date, spark_levels,
};
use crate::test::Mode;

/// Weeks the calendar shows at most (a year).
const CALENDAR_WEEKS: usize = 52;
/// Fewest weeks worth drawing a calendar for.
const MIN_WEEKS: usize = 8;
/// Columns for the weekday labels left of the calendar.
const DAY_LABEL_W: u16 = 4;
/// Columns per week: a square and a gap.
const WEEK_W: u16 = 2;
/// Rows the calendar takes: month labels, seven days, a gap.
const CALENDAR_H: u16 = 1 + 7 + 1;
/// Rows the summary and trend lines take, with a gap after.
const SUMMARY_H: u16 = 3;
/// Days in the wpm trend.
const TREND_DAYS: usize = 30;
/// Fewest table rows to keep when deciding how much activity to show.
const MIN_TABLE: u16 = 6;
const SPARKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    let s = &app.summary;
    let records = app.stats.all();

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled("stats", p.main_bold())));
    lines.push(Line::default());

    if records.is_empty() {
        lines.push(Line::from(Span::styled("no tests yet", p.sub())));
    } else {
        lines.push(kv_row(
            p,
            &[
                ("tests", s.tests.to_string()),
                ("avg", format!("{:.0} wpm  {:.0}%", s.avg_wpm, s.avg_acc)),
                (
                    "last 10",
                    format!("{:.0} wpm  {:.0}%", s.recent_avg_wpm, s.recent_avg_acc),
                ),
            ],
        ));
        let mut bests: Vec<(String, String)> = Mode::TIME_PRESETS
            .iter()
            .map(|t| Mode::Time(*t))
            .chain(Mode::WORD_PRESETS.iter().map(|w| Mode::Words(*w)))
            .filter_map(|m| s.best.get(&m).map(|b| (m.label(), format!("{b:.0}"))))
            .collect();
        if bests.is_empty() {
            bests.push(("best".into(), "—".into()));
        }
        let refs: Vec<(&str, String)> =
            bests.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        lines.push(kv_row(p, &refs));
        lines.push(Line::default());
    }

    // Activity on top (as much as leaves room for the table), then the
    // summary, then the runs, newest first.
    let top = Rect::new(col.x, col.y + 1, col.width, col.height);
    // The table stops a row above the hint line.
    let table_bottom = area.bottom().saturating_sub(3);
    let title = lines.drain(..2).collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(title), top);
    let mut y = top.y + 2;
    if !records.is_empty() {
        // Rows left for the table's runs after the title, the summary
        // lines and the table header.
        let free = table_bottom.saturating_sub(top.y + 2 + lines.len() as u16 + 1);
        let weeks = calendar_weeks(col.width);
        let (calendar, summary) =
            if weeks >= MIN_WEEKS && free >= CALENDAR_H + SUMMARY_H + MIN_TABLE {
                (true, true)
            } else {
                (false, free >= SUMMARY_H + MIN_TABLE)
            };
        if calendar {
            render_calendar(
                frame,
                &app.activity,
                Rect::new(col.x, y, col.width, 8),
                weeks,
                p,
            );
            y += CALENDAR_H;
        }
        if summary {
            render_activity_lines(frame, &app.activity, Rect::new(col.x, y, col.width, 2), p);
            y += SUMMARY_H;
        }
    }
    let used = y - top.y + lines.len() as u16;
    frame.render_widget(
        Paragraph::new(lines),
        Rect::new(col.x, y, col.width, top.bottom().saturating_sub(y)),
    );
    if !records.is_empty() {
        let table = Rect::new(
            col.x,
            top.y + used,
            col.width,
            table_bottom.saturating_sub(top.y + used),
        );
        let columns = [
            Column::new("when", Width::Fixed(11)),
            Column::new("mode", Width::Min(9)),
            Column::new("language", Width::Min(8)),
            Column::new("wpm", Width::Fixed(5)).right(),
            Column::new("raw", Width::Fixed(5)).right(),
            Column::new("acc", Width::Fixed(5)).right(),
            Column::new("con", Width::Fixed(5)).right(),
        ];
        let rows = records
            .iter()
            .rev()
            .map(|r| {
                let when = r.ts.with_timezone(&Local).format("%m-%d %H:%M").to_string();
                let mut mode = r.mode.label();
                if r.punctuation {
                    mode.push_str(" p");
                }
                if r.numbers {
                    mode.push_str(" n");
                }
                Row::new(vec![
                    Cell::dim(when),
                    Cell::normal(mode),
                    Cell::normal(r.language.clone()),
                    Cell::accent(format!("{:.0}", r.wpm)),
                    Cell::normal(format!("{:.0}", r.raw)),
                    Cell::normal(format!("{:.0}%", r.acc)),
                    Cell::normal(format!("{:.0}%", r.consistency)),
                ])
            })
            .collect();
        SelectTable::new(&columns, rows, &app.history).render(frame, table, p);
    }
    hints::render(frame, area, col, p, "↑↓ move · g/G top/bottom · esc back");
}

/// How many weeks of calendar fit in `width` columns.
fn calendar_weeks(width: u16) -> usize {
    (width.saturating_sub(DAY_LABEL_W) / WEEK_W).min(CALENDAR_WEEKS as u16) as usize
}

/// Month labels over a grid of days (weeks across, Monday to Sunday down),
/// each square shaded from the background towards the accent by how many
/// tests that day had; today is marked.
fn render_calendar(frame: &mut Frame, a: &Activity, area: Rect, weeks: usize, p: &Palette) {
    let start = calendar_start(a.today, weeks);
    let max = a.max_tests_between(start, a.today);
    let empty = mix(p.bg, p.sub, 0.45);
    let shade = |level: u8| match level {
        0 => empty,
        n => mix(p.bg, p.main, [0.3, 0.5, 0.75, 1.0][n as usize - 1]),
    };
    let x0 = area.x + DAY_LABEL_W;
    let buf = frame.buffer_mut();
    for (w, name) in month_labels(start, weeks, 3) {
        let x = x0 + w as u16 * WEEK_W;
        if x + 3 <= area.right() {
            buf.set_string(x, area.y, name, p.sub());
        }
    }
    for (row, name) in [(0, "mon"), (2, "wed"), (4, "fri")] {
        buf.set_string(area.x, area.y + 1 + row, name, p.sub());
    }
    for w in 0..weeks {
        for row in 0..7u16 {
            let Some(day) = start.checked_add_days(Days::new(7 * w as u64 + row as u64)) else {
                continue;
            };
            if day > a.today {
                continue;
            }
            let level = activity::level(a.tests_on(day), max);
            let (glyph, style) = if day == a.today {
                let c = if level == 0 { p.main } else { shade(level) };
                ("▣", Style::default().fg(c).add_modifier(Modifier::BOLD))
            } else {
                ("■", Style::default().fg(shade(level)))
            };
            buf.set_string(x0 + w as u16 * WEEK_W, area.y + 1 + row, glyph, style);
        }
    }
}

/// `streak 5 days · longest 12 days · typed 4h 12m · best day sep 14 (23)`
/// over the last 30 days' average wpm as a sparkline.
fn render_activity_lines(frame: &mut Frame, a: &Activity, area: Rect, p: &Palette) {
    let days = |n: u32| format!("{n} day{}", if n == 1 { "" } else { "s" });
    let mut items = vec![
        ("streak", days(a.current_streak)),
        ("longest", days(a.longest_streak)),
        ("typed", format_duration(a.secs)),
    ];
    if let Some((d, n)) = a.best_day {
        items.push(("best day", format!("{} ({n})", short_date(d))));
    }
    let summary = kv_row(p, &items);

    let trend = a.trend(TREND_DAYS);
    let mut spark = vec![Span::styled(format!("last {TREND_DAYS} days  "), p.sub())];
    // Days without a test are a quiet dot.
    for l in spark_levels(&trend) {
        spark.push(match l {
            Some(l) => Span::styled(SPARKS[l.min(7) as usize].to_string(), p.main()),
            None => Span::styled("·", p.sub()),
        });
    }
    let wpms = trend.iter().flatten();
    let lo = wpms.clone().copied().fold(f64::INFINITY, f64::min);
    let hi = wpms.copied().fold(f64::NEG_INFINITY, f64::max);
    if lo.is_finite() {
        let range = if (hi - lo).abs() < 0.5 {
            format!("{lo:.0} wpm")
        } else {
            format!("{lo:.0}–{hi:.0} wpm")
        };
        spark.push(Span::styled(format!("  {range}"), p.sub()));
    } else {
        spark.push(Span::styled("  no tests lately", p.sub()));
    }
    frame.render_widget(Paragraph::new(vec![summary, Line::from(spark)]), area);
}

fn kv_row<'a>(p: &Palette, items: &[(&str, String)]) -> Line<'a> {
    let mut spans = Vec::new();
    for (i, (k, v)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", p.sub()));
        }
        spans.push(Span::styled(format!("{k} "), p.sub()));
        spans.push(Span::styled(v.clone(), p.fg()));
    }
    Line::from(spans)
}

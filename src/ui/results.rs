//! Results screen: headline numbers, a detail row, a wpm-over-time chart and
//! a keyboard heatmap of missed keys, with confetti over a new personal
//! best. Sections are gated by `config.results`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use std::time::Instant;

use ratatui::style::{Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Paragraph};

use super::style::{Palette, content_column, vcenter};
use crate::app::celebrate::{Particle, Tint};
use crate::app::misses::{self, KeyMisses};
use crate::app::{App, DailyOutcome, DailyStatus, Outcome};
use crate::test::Mode;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let Some(outcome) = &app.outcome else {
        return;
    };
    let col = content_column(area, app.config.content_width());
    let cfg = &app.config.results;

    // headline (2) + gap + detail (1) [+ daily (1)] [+ gap + chart (n)]
    // [+ gap + keys label (1) + keyboard (n)] + gap + hint (1)
    let chart_h: u16 = if cfg.chart { 10 } else { 0 };
    let daily_h: u16 = u16::from(outcome.daily.is_some());
    let total = 2 + 1 + 1 + daily_h + if cfg.chart { 1 + chart_h } else { 0 } + 1 + 1;
    // The keyboard goes first when the terminal is too short for it.
    let keys = outcome.misses.on(app.config.keyboard);
    let numbers = keys.on_number_row();
    let (kb_w, kb_h) = misses::layout_size(keys.keyboard, numbers);
    let show_keys =
        cfg.keys && keys.total() > 0 && total + 2 + kb_h <= area.height && kb_w <= col.width;
    let total = total + if show_keys { 2 + kb_h } else { 0 };
    let block = vcenter(col, total);

    let mut constraints = vec![
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
    ];
    if outcome.daily.is_some() {
        constraints.push(Constraint::Length(1));
    }
    if cfg.chart {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(chart_h));
    }
    if show_keys {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1 + kb_h));
    }
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Length(1));
    let rows = Layout::vertical(constraints).split(block);

    render_headline(frame, outcome, rows[0], p);
    if outcome.is_pb && app.config.pb_effect.trophy() {
        render_trophy(frame, trophy_at(outcome, rows[0], area), p);
    }
    render_detail(frame, app, outcome, rows[2], p);
    let mut next = 3;
    if let Some(d) = &outcome.daily {
        render_daily(frame, d, rows[next], p);
        next += 1;
    }
    if cfg.chart {
        let m = &outcome.metrics;
        render_chart(
            frame,
            &m.raw_per_second,
            &m.wpm_per_second,
            outcome.record.mode,
            rows[next + 1],
            p,
        );
        next += 2;
    }
    if show_keys {
        render_keys(frame, &keys, rows[next + 1], p);
    }
    let hint = "tab  next   ·   s  stats   ·   m  missed keys   ·   :  command";
    frame.render_widget(Paragraph::new(hint).style(p.sub()), rows[rows.len() - 1]);

    if let Some(c) = &outcome.celebration {
        let sparks = c.frame_at(Instant::now());
        if !sparks.is_empty() {
            render_confetti(frame, &sparks, new_best_center(outcome, rows[0]), area, p);
        }
    }
}

/// The middle of the "new best" text, where the confetti is thrown from.
fn new_best_center(o: &Outcome, headline: Rect) -> (u16, u16) {
    let acc = format!("{:.0}%", o.metrics.accuracy);
    let x = headline.x + 8 + 3 + acc.chars().count() as u16 + 2 + 4;
    (x, headline.y + 1)
}

/// Sparks over the screen, only on empty cells away from text, so nothing
/// is hidden or crowded.
fn render_confetti(
    frame: &mut Frame,
    sparks: &[Particle],
    (cx, cy): (u16, u16),
    area: Rect,
    p: &Palette,
) {
    let buf = frame.buffer_mut();
    let blank = |x: i32, y: i32| {
        x < 0
            || y < 0
            || buf
                .cell((x as u16, y as u16))
                .is_none_or(|c| c.symbol() == " ")
    };
    let placed: Vec<(u16, u16, &Particle)> = sparks
        .iter()
        .filter_map(|s| {
            let x = cx as i32 + s.dx as i32;
            let y = cy as i32 + s.dy as i32;
            let inside = x >= area.left() as i32
                && x < area.right() as i32
                && y >= area.top() as i32
                && y < area.bottom() as i32;
            (inside && blank(x, y) && blank(x - 1, y) && blank(x + 1, y))
                .then_some((x as u16, y as u16, s))
        })
        .collect();
    for (x, y, s) in placed {
        let color = match s.tint {
            Tint::Main => p.main,
            Tint::Fg => p.fg,
            Tint::Correct => p.correct,
            Tint::ErrorExtra => p.error_extra,
            Tint::Sub => p.sub,
        };
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char(s.glyph).set_fg(color);
        }
    }
}

/// `missed keys  e 4 · t 2` over the configured keyboard shaded by misses.
fn render_keys(frame: &mut Frame, m: &KeyMisses, area: Rect, p: &Palette) {
    let mut label = vec![Span::styled("missed keys", p.sub())];
    let mut worst: Vec<(String, u32)> = m
        .worst(5)
        .into_iter()
        .map(|(k, n)| (k.to_string(), n))
        .collect();
    if m.other > 0 {
        worst.push(("other".into(), m.other));
    }
    for (i, (k, n)) in worst.iter().enumerate() {
        label.push(Span::styled(if i == 0 { "   " } else { "  ·  " }, p.sub()));
        label.push(Span::styled(format!("{k} "), p.fg()));
        label.push(Span::styled(n.to_string(), p.sub()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(label)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let max = m.max();
    let buf = frame.buffer_mut();
    for (k, x, y) in misses::layout(m.keyboard, m.on_number_row()) {
        let style = match misses::heat(m.get(k), max) {
            0 => p.sub(),
            1 => Style::default().fg(p.error_extra),
            2 => p.error().add_modifier(Modifier::BOLD),
            _ => Style::default()
                .bg(p.error)
                .fg(p.bg)
                .add_modifier(Modifier::BOLD),
        };
        let (x, y) = (area.x + x, area.y + 1 + y);
        if y < area.bottom() && x + 3 <= area.right() {
            buf.set_string(x, y, format!(" {k} "), style);
        }
    }
}

/// The trophy drawn after "new best", in half blocks: a bowl whose handles
/// curve in from its rim, on a stem and a foot. It takes the empty row
/// above the headline too; `TROPHY_SHORT` is for when there isn't one.
const TROPHY: [&str; 3] = ["█▀███▀█", " ▀███▀ ", "  ▄█▄  "];
const TROPHY_SHORT: [&str; 2] = ["█▀███▀█", "  ▄█▄  "];

/// Where the trophy goes: two columns past the "new best" text, its foot
/// on the score's row, reaching one row above the headline when that row
/// is free (not the brand line at the top of `area`).
fn trophy_at(o: &Outcome, headline: Rect, area: Rect) -> (Rect, &'static [&'static str]) {
    let acc = format!("{:.0}%", o.metrics.accuracy);
    let x = headline.x + 8 + 3 + acc.chars().count() as u16 + "  new best".len() as u16 + 2;
    let width = TROPHY[0].chars().count() as u16;
    let fits = |r: Rect| r.right() <= area.right();
    if headline.y >= area.y + 2 {
        let r = Rect::new(x, headline.y - 1, width, 3);
        if fits(r) {
            return (r, &TROPHY);
        }
    }
    (
        Rect::new(x, headline.y, width, 2).intersection(area),
        &TROPHY_SHORT,
    )
}

fn render_trophy(frame: &mut Frame, (area, rows): (Rect, &[&str]), p: &Palette) {
    let style = p.main_bold();
    let lines: Vec<Line> = rows.iter().map(|r| Line::styled(*r, style)).collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_headline(frame: &mut Frame, o: &Outcome, area: Rect, p: &Palette) {
    let m = &o.metrics;
    let (note, note_style) = match o.record.invalid {
        Some(why) => (format!("  invalid · {}", why.label()), p.error()),
        None if o.is_pb => ("  new best".to_string(), p.main()),
        None => (String::new(), p.main()),
    };
    let label = Line::from(vec![
        Span::styled("wpm", p.sub()),
        Span::raw("        "),
        Span::styled("acc", p.sub()),
    ]);
    let value = Line::from(vec![
        Span::styled(format!("{:<8}", format!("{:.0}", m.wpm)), p.main_bold()),
        Span::raw("   "),
        Span::styled(format!("{:.0}%", m.accuracy), p.main_bold()),
        Span::styled(note, note_style),
    ]);
    frame.render_widget(Paragraph::new(vec![label, value]), area);
}

fn render_detail(frame: &mut Frame, app: &App, o: &Outcome, area: Rect, p: &Palette) {
    let m = &o.metrics;
    let cfg = &app.config.results;
    let mut spans: Vec<Span> = Vec::new();
    let push = |spans: &mut Vec<Span>, k: &str, v: String| {
        if !spans.is_empty() {
            spans.push(Span::styled("  ·  ", p.sub()));
        }
        if !k.is_empty() {
            spans.push(Span::styled(format!("{k} "), p.sub()));
        }
        spans.push(Span::styled(v, p.fg()));
    };
    if cfg.raw {
        push(&mut spans, "raw", format!("{:.0}", m.raw));
    }
    if cfg.consistency {
        push(&mut spans, "con", format!("{:.0}%", m.consistency));
    }
    if cfg.char_breakdown {
        let c = m.chars;
        push(
            &mut spans,
            "chars",
            format!("{}/{}/{}/{}", c.correct, c.incorrect, c.extra, c.missed),
        );
    }
    push(&mut spans, "", o.record.mode.label());
    push(&mut spans, "", o.record.language.clone());
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// `daily · first try · #12 first · #30 best`, or where the submission is at.
fn render_daily(frame: &mut Frame, d: &DailyOutcome, area: Rect, p: &Palette) {
    let mut spans = vec![Span::styled("daily", p.main())];
    let mut part = |text: String, style| {
        spans.push(Span::styled("  ·  ", p.sub()));
        spans.push(Span::styled(text, style));
    };
    match &d.status {
        DailyStatus::Submitting => part("submitting…".into(), p.sub()),
        DailyStatus::Queued => part("offline, queued for next start".into(), p.sub()),
        DailyStatus::Failed(e) => part(e.clone(), p.error()),
        DailyStatus::Ranked(r) if !r.valid => part(
            format!(
                "rejected: {}",
                r.rejected.as_deref().unwrap_or("invalid run")
            ),
            p.error(),
        ),
        DailyStatus::Ranked(r) => {
            let attempt = match r.attempt {
                1 => "first try".to_string(),
                n => format!("attempt {n}"),
            };
            part(attempt, p.fg());
            if let Some(n) = r.rank_first {
                part(format!("#{n} first"), p.fg());
            }
            if let Some(n) = r.rank_best {
                part(format!("#{n} best"), p.fg());
            }
            let following = match (r.following_first, r.following_best) {
                (Some(f), Some(b)) => Some(format!("following #{f} first · #{b} best")),
                (None, Some(b)) => Some(format!("following #{b} best")),
                (Some(f), None) => Some(format!("following #{f} first")),
                (None, None) => None,
            };
            if let Some(text) = following {
                part(text, p.main());
            }
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Raw (subdued) and net wpm (accent) per second, shared with the
/// leaderboard's graph view.
/// Per-second raw and wpm lines. Point `i` is the second ending at `i + 1`,
/// on an axis from 0 to the run's length. A timed run shows exactly its
/// seconds (runs saved before the engine stopped the clock at the limit
/// carry a sliver of an extra one).
pub fn render_chart(
    frame: &mut Frame,
    raw: &[f64],
    wpm: &[f64],
    mode: Mode,
    area: Rect,
    p: &Palette,
) {
    let (raw, wpm) = match mode {
        Mode::Time(s) => {
            let n = s as usize;
            (&raw[..raw.len().min(n)], &wpm[..wpm.len().min(n)])
        }
        Mode::Words(_) => (raw, wpm),
    };
    let raw: Vec<(f64, f64)> = raw
        .iter()
        .enumerate()
        .map(|(i, v)| ((i + 1) as f64, *v))
        .collect();
    let wpm: Vec<(f64, f64)> = wpm
        .iter()
        .enumerate()
        .map(|(i, v)| ((i + 1) as f64, *v))
        .collect();
    let secs = raw.len().max(1) as f64;
    let y_max = raw
        .iter()
        .chain(&wpm)
        .map(|(_, y)| *y)
        .fold(0.0, f64::max)
        .max(10.0);
    let y_max = (y_max / 10.0).ceil() * 10.0;

    let datasets = vec![
        Dataset::default()
            .name("raw")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(p.sub())
            .data(&raw),
        Dataset::default()
            .name("wpm")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(p.main())
            .data(&wpm),
    ];
    let x_labels: Vec<Span> = [0.0, secs]
        .iter()
        .map(|s| Span::styled(format!("{s:.0}s"), p.sub()))
        .collect();
    let y_labels: Vec<Span> = [0.0, y_max / 2.0, y_max]
        .iter()
        .map(|v| Span::styled(format!("{v:.0}"), p.sub()))
        .collect();
    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .bounds([0.0, secs])
                .labels(x_labels)
                .style(p.sub()),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, y_max])
                .labels(y_labels)
                .style(p.sub()),
        )
        .style(Style::default())
        .hidden_legend_constraints((Constraint::Ratio(1, 4), Constraint::Ratio(1, 4)));
    frame.render_widget(chart, area);
}

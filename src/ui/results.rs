//! Results screen: headline numbers, a detail row, and a wpm-over-time chart.
//! Sections are gated by `config.results`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Paragraph};

use super::style::{Palette, content_column, vcenter};
use crate::app::{App, DailyOutcome, DailyStatus, Outcome};

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let Some(outcome) = &app.outcome else {
        return;
    };
    let col = content_column(area, app.config.content_width());
    let cfg = &app.config.results;

    // headline (2) + gap + detail (1) [+ daily (1)] + gap + chart (n) + gap + hint (1)
    let chart_h: u16 = if cfg.chart { 10 } else { 0 };
    let daily_h: u16 = u16::from(outcome.daily.is_some());
    let total = 2 + 1 + 1 + daily_h + if cfg.chart { 1 + chart_h } else { 0 } + 1 + 1;
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
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Length(1));
    let rows = Layout::vertical(constraints).split(block);

    render_headline(frame, outcome, rows[0], p);
    render_detail(frame, app, outcome, rows[2], p);
    let mut next = 3;
    if let Some(d) = &outcome.daily {
        render_daily(frame, d, rows[next], p);
        next += 1;
    }
    if cfg.chart {
        render_chart(frame, outcome, rows[next + 1], p);
    }
    let hint = "tab  next   ·   s  stats   ·   :  command";
    frame.render_widget(Paragraph::new(hint).style(p.sub()), rows[rows.len() - 1]);
}

fn render_headline(frame: &mut Frame, o: &Outcome, area: Rect, p: &Palette) {
    let m = &o.metrics;
    let pb = if o.is_pb { "  new best" } else { "" };
    let label = Line::from(vec![
        Span::styled("wpm", p.sub()),
        Span::raw("        "),
        Span::styled("acc", p.sub()),
    ]);
    let value = Line::from(vec![
        Span::styled(format!("{:<8}", format!("{:.0}", m.wpm)), p.main_bold()),
        Span::raw("   "),
        Span::styled(format!("{:.0}%", m.accuracy), p.main_bold()),
        Span::styled(pb, p.main()),
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
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_chart(frame: &mut Frame, o: &Outcome, area: Rect, p: &Palette) {
    let m = &o.metrics;
    let raw: Vec<(f64, f64)> = m
        .raw_per_second
        .iter()
        .enumerate()
        .map(|(i, v)| ((i + 1) as f64, *v))
        .collect();
    let wpm: Vec<(f64, f64)> = m
        .wpm_per_second
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
    let x_labels: Vec<Span> = [1.0, secs]
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
                .bounds([1.0, secs.max(2.0)])
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

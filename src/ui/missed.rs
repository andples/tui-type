//! Missed-keys screen (`:missed`): every counted run's misses over a range
//! (last day, 7 days, 30 days, all time), on a big keyboard in the
//! configured layout, with the five worst keys and their miss rates.

use chrono::Utc;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, mix};
use super::widgets::hints;
use crate::app::App;
use crate::app::misses::{self, KeyMisses, KeySize, MissedRange};

/// Keys in the "most missed" list.
const TOP: usize = 5;
/// Columns kept free either side of the keyboard.
const MARGIN: u16 = 2;
/// Rows above the keyboard: title, gap, ranges, gap, summary, gap.
const HEAD_H: u16 = 6;
/// The same without the gaps between the head's lines, so short
/// terminals keep big keys.
const HEAD_TIGHT: u16 = 4;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let keys = app.missed.on(app.config.keyboard);
    let numbers = keys.on_number_row();
    let worst = keys.worst(TOP);
    let list_h = list_height(&keys, worst.len());

    // Rows for everything but the keyboard: the head, a gap, the list and
    // the hint line with the notice row under it.
    let room = area.height.saturating_sub(2);
    let width = area.width.saturating_sub(MARGIN * 2);
    let fits = |size: KeySize, head: u16, list: u16| {
        let (w, h) = misses::layout_size_sized(keys.keyboard, numbers, size);
        w <= width && head + h + 1 + list <= room
    };
    // The biggest keys that leave room for the list (squeezing the head
    // first), else the biggest that fit at all with the list cut short.
    let pick = |list: u16| {
        misses::SIZES.into_iter().find_map(|s| {
            [HEAD_H, HEAD_TIGHT]
                .into_iter()
                .find(|head| fits(s, *head, list))
                .map(|head| (s, head))
        })
    };
    let picked = pick(list_h).or_else(|| pick(0));
    let head = picked.map_or(HEAD_TIGHT, |(_, h)| h);
    let size = picked.map(|(s, _)| s);
    let (kb_w, kb_h) = size.map_or((0, 0), |s| {
        misses::layout_size_sized(keys.keyboard, numbers, s)
    });
    let list_h = match size {
        Some(_) => list_h.min(room.saturating_sub(head + kb_h + 1)),
        None => list_h.min(room.saturating_sub(head)),
    };
    let total = head + if size.is_some() { kb_h + 1 } else { 0 } + list_h;
    let free = room.saturating_sub(total);
    let mut y = area.y + free * 2 / 5;

    let center = |frame: &mut Frame, line: Line, y: u16| {
        if y < area.y + room {
            let row = Rect::new(area.x, y, area.width, 1);
            frame.render_widget(Paragraph::new(line).centered(), row);
        }
    };

    let step = if head == HEAD_H { 2 } else { 1 };
    center(
        frame,
        Line::from(Span::styled("missed keys", p.main_bold())),
        y,
    );
    center(frame, ranges(app.missed_range, area.width, p), y + step);
    center(frame, summary(app, &keys, p), y + step * 2);
    y += head;

    if let Some(size) = size {
        let x = area.x + area.width.saturating_sub(kb_w) / 2;
        render_keyboard(frame, &keys, size, Rect::new(x, y, kb_w, kb_h), p);
        y += kb_h + 1;
    }

    let lines = list(&keys, &worst, p);
    let list_w = lines.iter().map(Line::width).max().unwrap_or(0) as u16;
    let x = area.x + area.width.saturating_sub(list_w) / 2;
    for (i, line) in lines.into_iter().take(list_h as usize).enumerate() {
        let row = Rect::new(x, y + i as u16, list_w.min(area.width), 1);
        frame.render_widget(Paragraph::new(line), row);
    }

    let hint = "←→ range  ·  1–5 pick  ·  :keyboard layout  ·  s stats  ·  esc back";
    let hint = if hint.chars().count() as u16 <= width {
        hint
    } else {
        "←→ range  ·  esc back"
    };
    let hint_w = (hint.chars().count() as u16).min(area.width);
    let col = Rect::new(
        area.x + (area.width - hint_w) / 2,
        area.y,
        hint_w,
        area.height,
    );
    hints::render(frame, area, col, p, hint);
}

/// `last test    last day    7 days    30 days    all time`, the shown one picked out;
/// `last  day  week  month  all` when that doesn't fit in `width`.
fn ranges(current: MissedRange, width: u16, p: &Palette) -> Line<'static> {
    let long: usize = MissedRange::ALL.iter().map(|r| r.label().len() + 4).sum();
    let short = long - 4 > width as usize;
    let mut spans = Vec::new();
    for (i, r) in MissedRange::ALL.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(if short { "  " } else { "    " }));
        }
        let style = if r == current { p.selected() } else { p.sub() };
        spans.push(Span::styled(if short { r.arg() } else { r.label() }, style));
    }
    Line::from(spans)
}

/// `12 tests  ·  37 misses in 2 960 keys  ·  1.2%`.
fn summary(app: &App, keys: &KeyMisses, p: &Palette) -> Line<'static> {
    let tests = app.missed_range.runs(app.stats.all(), Utc::now()).len();
    let typed: u32 = app.missed.typed.values().sum();
    let missed = app.missed.total();
    if tests == 0 {
        let what = match app.missed_range {
            MissedRange::All | MissedRange::Last => "no tests yet".to_string(),
            r => format!("no tests in the {}", r.label().trim_start_matches("last ")),
        };
        return Line::from(Span::styled(what, p.sub()));
    }
    let tests = format!("{tests} test{}", if tests == 1 { "" } else { "s" });
    let misses = format!("{missed} miss{}", if missed == 1 { "" } else { "es" });
    let mut spans = vec![
        Span::styled(tests, p.fg()),
        Span::styled("  ·  ", p.sub()),
        Span::styled(misses, p.fg()),
    ];
    if typed > 0 {
        spans.push(Span::styled(format!(" in {typed} keys"), p.sub()));
        spans.push(Span::styled("  ·  ", p.sub()));
        spans.push(Span::styled(
            format!("{:.1}%", f64::from(missed) * 100.0 / f64::from(typed)),
            p.fg(),
        ));
    }
    if keys.total() > 0 && keys.max() == 0 {
        spans.push(Span::styled("  ·  none on this keyboard", p.sub()));
    }
    Line::from(spans)
}

/// Rows the list takes: a heading and a row per key, plus one for misses
/// off the keyboard, or a single line when there's nothing to list.
fn list_height(keys: &KeyMisses, worst: usize) -> u16 {
    if worst == 0 {
        1
    } else {
        1 + worst as u16 + u16::from(keys.other > 0)
    }
}

/// `most missed` over `1   e   4.2%   37 of 880` rows.
fn list(keys: &KeyMisses, worst: &[(char, u32)], p: &Palette) -> Vec<Line<'static>> {
    if worst.is_empty() {
        return vec![Line::from(Span::styled("nothing missed", p.sub()))];
    }
    let rows: Vec<(String, String, String, String)> = worst
        .iter()
        .enumerate()
        .map(|(i, (k, n))| {
            let pct = keys
                .percent(*k)
                .map_or("–".to_string(), |v| format!("{v:.1}%"));
            let typed = keys.typed.get(k).copied().unwrap_or(0);
            (
                format!("{}", i + 1),
                k.to_string(),
                pct,
                format!("{n} of {typed}"),
            )
        })
        .collect();
    let pct_w = rows.iter().map(|r| r.2.chars().count()).max().unwrap_or(0);
    let mut lines = vec![Line::from(Span::styled("most missed", p.sub()))];
    for (rank, key, pct, count) in rows {
        lines.push(Line::from(vec![
            Span::styled(format!("{rank}   "), p.sub()),
            Span::styled(format!("{key}   "), p.fg().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{pct:>pct_w$}   "), p.error()),
            Span::styled(count, p.sub()),
        ]));
    }
    if keys.other > 0 {
        lines.push(Line::from(vec![
            Span::styled("·   ", p.sub()),
            Span::styled(format!("{} ", keys.other), p.fg()),
            Span::styled("off the keyboard", p.sub()),
        ]));
    }
    lines
}

/// The keyboard shaded by misses like the results screen's. Keys three
/// rows tall are filled blocks with the miss rate under the letter.
fn render_keyboard(frame: &mut Frame, m: &KeyMisses, size: KeySize, area: Rect, p: &Palette) {
    let max = m.max();
    let buf = frame.buffer_mut();
    for (k, x, y) in misses::layout_sized(m.keyboard, m.on_number_row(), size) {
        let (x, y) = (area.x + x, area.y + y);
        if x + size.width > buf.area.right() || y + size.height > buf.area.bottom() {
            continue;
        }
        let heat = misses::heat(m.get(k), max);
        if size.height == 1 {
            let style = match heat {
                0 => p.sub(),
                1 => Style::default().fg(p.error_extra),
                2 => p.error().add_modifier(Modifier::BOLD),
                _ => Style::default()
                    .bg(p.error)
                    .fg(p.bg)
                    .add_modifier(Modifier::BOLD),
            };
            buf.set_string(x, y, format!(" {k} "), style);
            continue;
        }
        let (bg, fg) = match heat {
            0 => (mix(p.bg, p.sub, 0.18), p.sub),
            1 => (mix(p.bg, p.error, 0.3), p.fg),
            2 => (mix(p.bg, p.error, 0.6), p.fg),
            _ => (p.error, p.bg),
        };
        let fill = Style::default().bg(bg).fg(fg);
        let blank = " ".repeat(size.width as usize);
        for row in 0..size.height {
            buf.set_string(x, y + row, &blank, fill);
        }
        let mid = y + (size.height - 1) / 2;
        let label_x = x + size.width / 2;
        let bold = if heat > 0 {
            fill.add_modifier(Modifier::BOLD)
        } else {
            fill
        };
        buf.set_string(label_x, mid, k.to_string(), bold);
        if heat > 0
            && let Some(pct) = m.percent(k)
        {
            let text = if pct < 1.0 {
                "<1%".to_string()
            } else {
                format!("{pct:.0}%")
            };
            let w = text.chars().count() as u16;
            if w <= size.width {
                let tx = x + (size.width - w).div_ceil(2);
                buf.set_string(tx, y + size.height - 1, text, fill);
            }
        }
    }
}

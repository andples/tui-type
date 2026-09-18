//! History screen: summary numbers plus a scrollable table of recent tests.

use chrono::Local;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use crate::app::App;
use crate::test::Mode;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.zoom_level().0);
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

        lines.push(Line::from(Span::styled(
            format!(
                "{:<12} {:<10} {:<12} {:>5} {:>5} {:>5} {:>5}",
                "when", "mode", "language", "wpm", "raw", "acc", "con"
            ),
            p.sub(),
        )));
        let visible = area.height.saturating_sub(8) as usize;
        let total = records.len();
        let start = app.scroll.min(total.saturating_sub(1));
        for r in records.iter().rev().skip(start).take(visible.max(1)) {
            let when = r.ts.with_timezone(&Local).format("%m-%d %H:%M").to_string();
            let mut mode = r.mode.label();
            if r.punctuation {
                mode.push_str(" p");
            }
            if r.numbers {
                mode.push_str(" n");
            }
            lines.push(Line::from(vec![
                Span::styled(format!("{when:<12} "), p.sub()),
                Span::styled(format!("{mode:<10} "), p.fg()),
                Span::styled(format!("{:<12} ", truncate(&r.language, 12)), p.fg()),
                Span::styled(format!("{:>5.0} ", r.wpm), p.main()),
                Span::styled(format!("{:>5.0} ", r.raw), p.fg()),
                Span::styled(format!("{:>4.0}% ", r.acc), p.fg()),
                Span::styled(format!("{:>4.0}%", r.consistency), p.fg()),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(lines),
        Rect::new(col.x, col.y + 1, col.width, col.height),
    );
    let hint = "j/k  scroll   ·   esc  back";
    frame.render_widget(
        Paragraph::new(hint).style(p.sub()),
        Rect::new(col.x, area.bottom().saturating_sub(2), col.width, 1),
    );
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

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).chain(std::iter::once('…')).collect()
    }
}

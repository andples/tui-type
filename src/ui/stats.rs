//! History screen: summary numbers plus a scrollable table of recent tests.

use chrono::Local;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use super::widgets::{Cell, Column, Row, SelectTable, Width, hints};
use crate::app::App;
use crate::test::Mode;

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

    // Summary on top, then the runs, newest first.
    let top = Rect::new(col.x, col.y + 1, col.width, col.height);
    let used = lines.len() as u16;
    frame.render_widget(Paragraph::new(lines), top);
    if !records.is_empty() {
        let table = Rect::new(
            col.x,
            top.y + used,
            col.width,
            top.height.saturating_sub(used + 2),
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

//! `:hud` screen: the live readouts above the words as a checklist in
//! line order (shown ones first), with a preview of the line as it reads
//! right now.

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use super::typing::hud_line;
use super::widgets::{Cell, Column, Row, SelectTable, Width, hints};
use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));
    let hud = &app.config.hud;

    let mut preview = vec![Span::styled("  ", p.sub())];
    if hud.items().is_empty() {
        preview.push(Span::styled("nothing shown", p.sub()));
    } else {
        preview.extend(hud_line(app, Instant::now(), p).spans);
    }
    let top = vec![
        Line::from(vec![
            Span::styled("hud", p.main_bold()),
            Span::styled("   live readouts above the words, left to right", p.sub()),
        ]),
        Line::default(),
        Line::from(preview),
        Line::default(),
    ];
    let top_h = top.len() as u16;
    frame.render_widget(Paragraph::new(top), body);

    let columns = [
        Column::new("", Width::Fixed(3)),
        Column::new("", Width::Fixed(2)).right(),
        Column::new("", Width::Fixed(10)),
        Column::new("", Width::Min(0)),
    ];
    let rows: Vec<Row> = hud
        .menu_order()
        .into_iter()
        .enumerate()
        .map(|(i, (item, shown))| {
            let (check, place) = if shown {
                (Cell::accent("[x]"), Cell::dim((i + 1).to_string()))
            } else {
                (Cell::dim("[ ]"), Cell::dim(""))
            };
            Row::new(vec![
                check,
                place,
                Cell::normal(item.label()),
                Cell::dim(item.about()),
            ])
        })
        .collect();
    let want = SelectTable::height_for(&columns, rows.len());
    let table_h = want.min(body.height.saturating_sub(top_h + 1)).max(1);
    let table = Rect::new(body.x, body.y + top_h, body.width, table_h);
    SelectTable::new(&columns, rows, &app.hud_selection).render(frame, table, p);
    hints::render(
        frame,
        area,
        col,
        p,
        "space show/hide · ←→ move along the line · esc back",
    );
}

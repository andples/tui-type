//! Modules screen: one language's modules as a checklist, to download
//! (from `:install`) or to mix into its tests.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use super::widgets::{Cell, Column, Row, SelectTable, Width, hints};
use crate::app::App;
use crate::catalog::module_menu::Purpose;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));
    let menu = &app.module_menu;
    let display = app
        .languages
        .get(&menu.language)
        .map(|l| l.display.as_str())
        .or_else(|| {
            let entry = app.catalog_menu.index()?.language(&menu.language)?;
            Some(entry.display.as_str())
        })
        .unwrap_or(menu.language.as_str());
    let (title, about) = match menu.purpose {
        Purpose::Download => (
            "modules to download",
            "extra word lists for common libraries; ticked ones are downloaded and used",
        ),
        Purpose::Pick => ("modules", "ticked modules are mixed into the words"),
    };
    let top = vec![
        Line::from(vec![
            Span::styled(title, p.main_bold()),
            Span::styled(format!("   {display}"), p.sub()),
        ]),
        Line::default(),
        Line::from(Span::styled(format!("  {about}"), p.sub())),
        Line::default(),
    ];
    let top_h = top.len() as u16;
    frame.render_widget(Paragraph::new(top), body);

    let name_w = menu
        .rows
        .iter()
        .map(|r| r.name.len())
        .max()
        .unwrap_or(0)
        .max(6) as u16;
    let columns = [
        Column::new("", Width::Fixed(3)),
        Column::new("", Width::Fixed(name_w + 1)),
        Column::new("", Width::Min(0)),
        Column::new("", Width::Fixed(11)).right(),
    ];
    let rows: Vec<Row> = menu
        .rows
        .iter()
        .map(|r| {
            let check = if r.checked {
                Cell::accent("[x]")
            } else {
                Cell::dim("[ ]")
            };
            let status = match (menu.purpose, r.installed, r.checked) {
                (Purpose::Download, false, true) => Cell::accent("download"),
                (Purpose::Download, true, false) => Cell::error("remove"),
                (Purpose::Download, true, true) => Cell::dim("installed"),
                (Purpose::Download, false, false) => Cell::dim("available"),
                (Purpose::Pick, ..) => Cell::dim(""),
            };
            Row::new(vec![
                check,
                Cell::normal(&r.name),
                Cell::dim(format!("{} · {} words", r.display, r.words)),
                status,
            ])
        })
        .collect();
    let want = SelectTable::height_for(&columns, rows.len());
    let table_h = want.min(body.height.saturating_sub(top_h + 1)).max(1);
    let table = Rect::new(body.x, body.y + top_h, body.width, table_h);
    if menu.rows.is_empty() {
        frame.render_widget(
            Paragraph::new("  no modules for this language").style(p.sub()),
            table,
        );
    } else {
        SelectTable::new(&columns, rows, &menu.selection).render(frame, table, p);
    }
    let hint = match menu.purpose {
        Purpose::Download => "space tick · a all · enter download and use · esc back",
        Purpose::Pick => "space tick · a all · enter use · esc keep as it was",
    };
    hints::render(frame, area, col, p, hint);
}

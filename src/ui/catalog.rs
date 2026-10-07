//! Install screen: installed and available languages and themes, one tab
//! per kind, narrowed by what's typed. Browsing the themes tab previews
//! each theme.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use super::widgets::{Cell, Column, Row, SelectTable, Width, hints};
use crate::app::App;
use crate::catalog::menu::IndexState;
use crate::catalog::{Item, ItemStatus, Kind};

const STATUS_W: u16 = 11;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));
    let menu = &app.catalog_menu;
    let tab = menu.tab;
    let items = menu.items(tab, &app.themes, &app.languages);

    // Heading, tabs with installed/total counts, then the catalogue's state.
    let mut tabs = vec![Span::styled("install", p.main_bold()), Span::raw("    ")];
    for kind in Kind::ALL {
        let all = menu.all_items(kind, &app.themes, &app.languages);
        let installed = all.iter().filter(|i| i.status.is_installed()).count();
        let style = if kind == tab { p.selected() } else { p.sub() };
        tabs.push(Span::styled(kind.dir(), style));
        tabs.push(Span::styled(
            format!(" {installed}/{}    ", all.len()),
            p.sub(),
        ));
    }
    tabs.push(Span::styled("custom", p.sub()));
    tabs.push(Span::styled(
        format!(" {}    ", app.customs.all().count()),
        p.sub(),
    ));
    // The search line: a prompt until something is typed.
    let search = if menu.query.is_empty() {
        Line::from(Span::styled("  type to search", p.sub()))
    } else {
        Line::from(vec![
            Span::styled("  / ", p.main()),
            Span::styled(menu.query.clone(), p.fg()),
            Span::styled(" ", p.caret()),
            Span::styled(format!("   {} found", items.len()), p.sub()),
        ])
    };
    let mut top = vec![Line::from(tabs), Line::default(), search, Line::default()];
    let state = match &menu.index {
        IndexState::Loading => Some(Span::styled("  fetching the catalogue…", p.sub())),
        IndexState::Failed(e) => Some(Span::styled(format!("  {e} · ctrl+r retries"), p.error())),
        IndexState::Disabled => Some(Span::styled(
            "  the catalogue is off · set catalog in the config to install more",
            p.sub(),
        )),
        IndexState::Ready(_) | IndexState::NotLoaded => None,
    };
    if let Some(s) = state {
        top.push(Line::from(s));
        top.push(Line::default());
    }
    let top_h = top.len() as u16;
    frame.render_widget(Paragraph::new(top), body);

    let current = match tab {
        Kind::Language => app.config.language.as_str(),
        Kind::Theme => app.config.theme.as_str(),
    };
    let name_w = items.iter().map(|i| i.name.len()).max().unwrap_or(0).max(8) as u16;
    let mut columns = vec![
        Column::new("", Width::Fixed(1)),
        Column::new("", Width::Fixed(name_w + 1)),
    ];
    match tab {
        Kind::Language => columns.push(Column::new("", Width::Min(0))),
        Kind::Theme => {
            columns.extend((0..SWATCHES).map(|_| Column::new("", Width::Fixed(2))));
            columns.push(Column::new("", Width::Min(0)));
        }
    }
    columns.push(Column::new("", Width::Fixed(STATUS_W)).right());
    let rows: Vec<Row> = items
        .iter()
        .map(|item| row(app, item, tab, item.name == current))
        .collect();

    let selection = menu.selection(tab);
    let want = SelectTable::height_for(&columns, rows.len());
    let table_h = want.min(body.height.saturating_sub(top_h + 2)).max(1);
    let table_area = Rect::new(body.x, body.y + top_h, body.width, table_h);
    if items.is_empty() && !menu.query.is_empty() {
        let text = format!("  no {} match · esc clears", tab.dir());
        frame.render_widget(Paragraph::new(text).style(p.sub()), table_area);
    } else {
        SelectTable::new(&columns, rows, selection).render(frame, table_area, p);
    }

    // What the highlighted row would do.
    let selected = items.get(selection.selected);
    let note: Vec<Span> = match selected {
        Some(item) if menu.confirm_remove => vec![
            Span::styled("  remove ", p.fg()),
            Span::styled(item.name.clone(), p.error()),
            Span::styled("?  y / n", p.fg()),
        ],
        Some(item) => {
            let text: String = match item.status {
                _ if item.name == current => "  in use".into(),
                ItemStatus::Available => {
                    "  enter installs and uses it · ctrl+s only installs".into()
                }
                ItemStatus::Installing => "  downloading…".into(),
                ItemStatus::BuiltIn => "  built in · enter uses it".into(),
                ItemStatus::Installed | ItemStatus::Local => {
                    "  enter uses it · ctrl+d removes it".into()
                }
            };
            vec![Span::styled(text, p.sub())]
        }
        None => vec![],
    };
    let note_y = table_area.bottom() + 1;
    if !note.is_empty() && note_y < body.bottom() {
        let note_area = Rect::new(body.x, note_y, body.width, 1);
        frame.render_widget(Paragraph::new(Line::from(note)), note_area);
    }

    let hint = if menu.confirm_remove {
        "y remove · any other key cancels"
    } else {
        let full =
            "enter use · ctrl+s install · ctrl+d remove · ctrl+r refresh · tab switch · esc back";
        if full.chars().count() <= col.width as usize {
            full
        } else {
            "enter use · ctrl+s install · ctrl+d remove · esc back"
        }
    };
    hints::render(frame, area, col, p, hint);
}

/// Colours shown for each theme: accent, text, dimmed text, error.
const SWATCHES: usize = 4;

fn row(app: &App, item: &Item, tab: Kind, current: bool) -> Row {
    let dot = if current {
        Cell::accent("●")
    } else if item.status.is_installed() {
        Cell::dim("○")
    } else {
        Cell::dim(" ")
    };
    let name = if item.status.is_installed() {
        Cell::normal(&item.name)
    } else {
        Cell::dim(&item.name)
    };
    let status = match item.status {
        ItemStatus::Installing => Cell::accent(item.status.label()),
        s => Cell::dim(s.label()),
    };
    let mut cells = vec![dot, name];
    match tab {
        Kind::Language => cells.push(Cell::dim(&item.detail)),
        Kind::Theme => {
            let theme = app
                .themes
                .get(&item.name)
                .or_else(|| app.catalog_menu.index()?.theme(&item.name));
            match theme {
                Some(t) => {
                    let c = &t.colors;
                    for hex in [c.main, c.fg, c.sub, c.error] {
                        cells.push(Cell::swatch("██", hex.color()));
                    }
                }
                None => cells.extend((0..SWATCHES).map(|_| Cell::dim(""))),
            }
            cells.push(Cell::dim(""));
        }
    }
    cells.push(status);
    Row::new(cells)
}

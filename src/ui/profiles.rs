//! Profile screen: the list of profiles with the active ones marked, or the
//! editor that picks which settings a profile controls.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use super::widgets::{Cell, Column, Row, SelectTable, Selection, Width, cursor, hints};
use crate::app::App;
use crate::profile::{Editor, ProfileMenu, SettingKey};

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    // Heading and body start below the brand row; hints sit above the
    // bottom (notice) line.
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));
    let hints = match &app.profile_menu {
        ProfileMenu::List {
            selected,
            confirm_delete,
        } => list(frame, app, p, selected, *confirm_delete, body),
        ProfileMenu::Edit(e) => {
            let (lines, hints) = editor(app, p, e, body);
            frame.render_widget(Paragraph::new(lines), body);
            hints
        }
    };
    hints::render(frame, area, col, p, hints);
}

/// Heading, the profile table and a line about the highlighted row.
fn list(
    frame: &mut Frame,
    app: &App,
    p: &Palette,
    selected: &Selection,
    confirm_delete: bool,
    body: Rect,
) -> &'static str {
    let mut top = vec![
        Line::from(Span::styled("profiles", p.main_bold())),
        Line::default(),
    ];
    let reg = &app.profiles;
    if reg.is_empty() {
        top.push(Line::from(Span::styled(
            "  no profiles yet — a profile saves any subset of your settings",
            p.sub(),
        )));
        top.push(Line::default());
    }
    let top_h = top.len() as u16;
    frame.render_widget(Paragraph::new(top), body);

    // Active dot, name, then what the profile sets.
    let name_w = reg.names().map(str::len).max().unwrap_or(0).max(8);
    let columns = [
        Column::new("", Width::Fixed(1)),
        Column::new("", Width::Fixed(name_w as u16 + 1)),
        Column::new("", Width::Min(0)),
    ];
    let mut rows: Vec<Row> = reg
        .iter()
        .map(|profile| {
            let active = app.config.profiles.contains(&profile.name);
            let dot = if active {
                Cell::accent("●")
            } else {
                Cell::dim("○")
            };
            let name = if active {
                Cell::normal(&profile.name)
            } else {
                Cell::dim(&profile.name)
            };
            Row::new(vec![dot, name, Cell::dim(profile.settings.summary())])
        })
        .collect();
    rows.push(Row::new(vec![Cell::dim("+ new profile").span(3)]));

    // Leave a blank line and one for the note below the table.
    let want = SelectTable::height_for(&columns, rows.len());
    let table_h = want.min(body.height.saturating_sub(top_h + 2)).max(1);
    let table_area = Rect::new(body.x, body.y + top_h, body.width, table_h);
    SelectTable::new(&columns, rows, selected).render(frame, table_area, p);

    // What the highlighted row would do.
    let mut note = Vec::new();
    if let Some(profile) = app.profile_menu.selected_profile(reg) {
        let active = app.config.profiles.contains(&profile.name);
        if confirm_delete {
            note = vec![
                Span::styled("  delete ", p.fg()),
                Span::styled(profile.name.clone(), p.error()),
                Span::styled("?  y / n", p.fg()),
            ];
        } else if active {
            note = vec![Span::styled(
                "  active · enter deselects it (settings stay as they are)",
                p.sub(),
            )];
        } else {
            let replaced = reg.conflicts(profile, &app.config.profiles);
            if !replaced.is_empty() {
                note = vec![
                    Span::styled("  enabling replaces ", p.sub()),
                    Span::styled(replaced.join(", "), p.error()),
                ];
            }
        }
    }
    let note_y = table_area.bottom() + 1;
    if !note.is_empty() && note_y < body.bottom() {
        let note_area = Rect::new(body.x, note_y, body.width, 1);
        frame.render_widget(Paragraph::new(Line::from(note)), note_area);
    }

    let on_new = selected.selected >= reg.len();
    if confirm_delete {
        "y delete · any other key cancels"
    } else if on_new {
        "enter create · ↑↓ move · esc back"
    } else {
        "enter toggle · n new · e edit · d delete · esc back"
    }
}

fn editor<'a>(
    app: &'a App,
    p: &Palette,
    e: &'a Editor,
    body: Rect,
) -> (Vec<Line<'a>>, &'static str) {
    let title = match &e.original {
        Some(n) => format!("edit {n}"),
        None => "new profile".into(),
    };
    let mut lines = vec![
        Line::from(Span::styled(title, p.main_bold())),
        Line::default(),
    ];

    let mut name = vec![
        cursor::lead(p, e.on_name()),
        Span::styled("name  ", cursor::style(p, e.on_name(), p.sub())),
    ];
    if e.name.is_empty() && !e.on_name() {
        name.push(Span::styled("(unnamed)", p.error()));
    } else {
        name.push(Span::styled(
            e.name.clone(),
            cursor::style(p, e.on_name(), p.fg()),
        ));
    }
    if e.on_name() {
        name.push(Span::styled(" ", p.caret()));
    }
    lines.push(Line::from(name));
    lines.push(Line::default());

    // Keep the focused setting in view on short terminals.
    let fixed = lines.len() + 2;
    let room = (body.height as usize).saturating_sub(fixed).max(1);
    let focus = e.row.saturating_sub(1);
    let first = focus.saturating_sub(room.saturating_sub(1));
    let label_w = SettingKey::ALL
        .iter()
        .map(|k| k.label().len())
        .max()
        .unwrap_or(0);
    for (i, k) in SettingKey::ALL
        .iter()
        .copied()
        .enumerate()
        .skip(first)
        .take(room)
    {
        let is_sel = e.row == i + 1;
        let live = k.show(&app.config);
        let mut row = vec![cursor::lead(p, is_sel)];
        match e.settings.show(k) {
            Some(value) => {
                row.push(Span::styled("[x] ", p.main()));
                row.push(Span::styled(
                    format!("{:<label_w$}  ", k.label()),
                    cursor::style(p, is_sel, p.fg()),
                ));
                row.push(Span::styled(
                    value.clone(),
                    cursor::style(p, is_sel, p.fg()),
                ));
                if value != live {
                    row.push(Span::styled(format!("  (now {live})"), p.sub()));
                }
            }
            None => {
                row.push(Span::styled("[ ] ", p.sub()));
                row.push(Span::styled(
                    format!("{:<label_w$}  ", k.label()),
                    cursor::style(p, is_sel, p.sub()),
                ));
                row.push(Span::styled(live, cursor::style(p, is_sel, p.sub())));
            }
        }
        lines.push(Line::from(row));
    }
    lines.push(Line::default());
    let n = e.settings.keys().len();
    lines.push(Line::from(Span::styled(
        match n {
            0 => "  no settings picked".to_string(),
            1 => "  controls 1 setting".to_string(),
            n => format!("  controls {n} settings"),
        },
        p.sub(),
    )));

    let hints = if e.on_name() {
        "type a name · ↓ settings · enter save · esc cancel"
    } else {
        "space include · ←→ value · u current · A all · enter save · esc cancel"
    };
    (lines, hints)
}

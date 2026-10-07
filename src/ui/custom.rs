//! The custom page (the install screen's custom tab): your word sets, then
//! shared ones from the server, most installed first. And the editor: a
//! name, then words typed with spaces between them, asked about before
//! they're added, with the set's words listed underneath.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::style::{Palette, content_column};
use super::widgets::{Cell, Column, Row, SelectTable, Width, hints};
use crate::app::App;
use crate::custom::menu::{Confirm, EditStep};

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));
    let menu = &app.custom_menu;
    let entries = menu.entries(&app.customs);

    let mine = app.customs.all().count();
    let tabs = Line::from(vec![
        Span::styled("install", p.main_bold()),
        Span::raw("    "),
        Span::styled("languages    themes    ", p.sub()),
        Span::styled("custom", p.selected()),
        Span::styled(format!(" {mine}    "), p.sub()),
    ]);
    let search = if menu.editing {
        Line::from(vec![
            Span::styled("  / ", p.main()),
            Span::styled(menu.query.clone(), p.fg()),
            Span::styled(" ", p.caret()),
        ])
    } else if menu.query.is_empty() {
        Line::from(Span::styled(
            "  your sets, then the most installed shared ones · / to search",
            p.sub(),
        ))
    } else {
        Line::from(vec![
            Span::styled("  / ", p.sub()),
            Span::styled(menu.query.clone(), p.fg()),
        ])
    };
    let mut top = vec![tabs, Line::default(), search, Line::default()];
    if let Some(e) = &menu.error {
        top.push(Line::from(Span::styled(format!("  {e}"), p.sub())));
        top.push(Line::default());
    }
    let top_h = top.len() as u16;
    frame.render_widget(Paragraph::new(top), body);

    let list_area = Rect::new(
        body.x,
        body.y + top_h,
        body.width,
        body.height.saturating_sub(top_h + 2),
    );
    if entries.is_empty() {
        let msg = if menu.loading {
            "  loading…"
        } else if menu.query.is_empty() {
            "  no sets yet · n makes one"
        } else {
            "  nothing matches"
        };
        frame.render_widget(Paragraph::new(msg).style(p.sub()), list_area);
    } else {
        let columns = [
            Column::new("", Width::Fixed(1)),
            Column::new("set", Width::Min(12)),
            Column::new("words", Width::Fixed(5)).right(),
            Column::new("by", Width::Min(8)),
            Column::new("installs", Width::Fixed(8)).right(),
            Column::new("", Width::Fixed(10)),
        ];
        let rows = entries
            .iter()
            .map(|e| {
                let in_use = e.local && app.config.custom == e.name;
                let status = match (in_use, e.local, &e.author) {
                    (true, _, _) => Cell::accent("in use"),
                    (_, true, None) => Cell::dim("yours"),
                    (_, true, Some(_)) => Cell::dim("installed"),
                    (_, false, _) => Cell::dim("shared"),
                };
                Row::new(vec![
                    if in_use {
                        Cell::accent("•")
                    } else {
                        Cell::normal("")
                    },
                    Cell::normal(&e.name),
                    Cell::normal(e.words.to_string()),
                    Cell::dim(e.author.clone().unwrap_or_else(|| "you".into())),
                    match e.installs {
                        Some(n) => Cell::normal(n.to_string()),
                        None => Cell::dim("-"),
                    },
                    status,
                ])
            })
            .collect();
        SelectTable::new(&columns, rows, &menu.selection).render(frame, list_area, p);
    }

    // What the highlighted row would do, or the question being asked.
    let note: Line = match &menu.confirm {
        Some(Confirm::Remove(n)) => Line::from(vec![
            Span::styled("  delete ", p.fg()),
            Span::styled(n.clone(), p.error()),
            Span::styled(" from this machine?  y / n", p.fg()),
        ]),
        Some(Confirm::Unpublish(n)) => Line::from(vec![
            Span::styled("  take ", p.fg()),
            Span::styled(n.clone(), p.error()),
            Span::styled(" off the server? your copy stays  y / n", p.fg()),
        ]),
        None => {
            let text = match menu.selected(&app.customs) {
                Some(e) if !e.local => "  enter installs and types it".to_string(),
                Some(e) if e.author.is_none() && e.installs.is_some() => {
                    "  enter types it · e edit · p publishes the new version · u takes it down"
                        .to_string()
                }
                Some(e) if e.author.is_none() => {
                    "  enter types it · e edit · p publish for others · x delete".to_string()
                }
                Some(_) => "  enter types it · e edit your copy · x delete".to_string(),
                None => String::new(),
            };
            Line::from(Span::styled(text, p.sub()))
        }
    };
    let note_y = list_area.bottom() + 1;
    if note_y < body.bottom() + 2 {
        frame.render_widget(
            Paragraph::new(note),
            Rect::new(
                body.x,
                note_y.min(area.bottom().saturating_sub(4)),
                body.width,
                1,
            ),
        );
    }

    let hint = if menu.confirm.is_some() {
        "y yes · any other key cancels"
    } else if menu.editing {
        "type to search · enter or esc done"
    } else {
        "enter use · n new · e edit · / search · tab languages · esc back"
    };
    hints::render(frame, area, col, p, hint);
}

pub fn render_edit(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    let Some(ed) = &app.custom_editor else {
        return;
    };
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));

    let title = match ed.step {
        EditStep::Name => Line::from(Span::styled("new custom set", p.main_bold())),
        EditStep::Words => Line::from(vec![
            Span::styled(ed.set.name.clone(), p.main_bold()),
            Span::styled(
                format!(
                    "  ·  {} {}",
                    ed.set.words.len(),
                    if ed.set.words.len() == 1 {
                        "word"
                    } else {
                        "words"
                    }
                ),
                p.sub(),
            ),
        ]),
    };
    let prompt = match ed.step {
        EditStep::Name => "  name ",
        EditStep::Words => "  add  ",
    };
    let typing = !ed.focus_list && ed.pending.is_none();
    let mut input = vec![
        Span::styled(prompt, p.sub()),
        Span::styled(ed.input.clone(), p.fg()),
    ];
    if typing {
        input.push(Span::styled(" ", p.caret()));
    }
    let status = match App::pending_summary(ed) {
        Some(q) => Line::from(vec![
            Span::styled(format!("  {q}"), p.main()),
            Span::styled("  y add · n keep editing", p.sub()),
        ]),
        None => Line::from(Span::styled(
            format!(
                "  {}",
                ed.message.clone().unwrap_or_else(|| default_help(ed.step))
            ),
            p.sub(),
        )),
    };
    let top = vec![title, Line::default(), Line::from(input), status];
    let top_h = top.len() as u16 + 1;
    frame.render_widget(Paragraph::new(top).wrap(Wrap { trim: false }), body);

    if ed.step == EditStep::Words {
        let list_area = Rect::new(
            body.x,
            body.y + top_h + 1,
            body.width,
            body.height.saturating_sub(top_h + 1),
        );
        if ed.set.words.is_empty() {
            frame.render_widget(Paragraph::new("  no words yet").style(p.sub()), list_area);
        } else {
            let columns = [
                Column::new("#", Width::Fixed(4)).right(),
                Column::new("word", Width::Min(10)),
            ];
            let rows = ed
                .set
                .words
                .iter()
                .enumerate()
                .map(|(i, w)| Row::new(vec![Cell::dim((i + 1).to_string()), Cell::normal(w)]))
                .collect();
            SelectTable::new(&columns, rows, &ed.selection)
                .focused(ed.focus_list)
                .render(frame, list_area, p);
        }
    }

    let hint = if ed.pending.is_some() {
        "y add them · n keep editing"
    } else if ed.focus_list {
        "↑↓ move · x remove word · tab back to typing · esc done"
    } else if ed.step == EditStep::Name {
        "enter take name · esc cancel"
    } else {
        "enter add words · tab remove words · esc done"
    };
    hints::render(frame, area, col, p, hint);
}

fn default_help(step: EditStep) -> String {
    match step {
        EditStep::Name => "lowercase letters, digits, _ or -, then enter".into(),
        EditStep::Words => {
            "type words separated by spaces, then enter · you'll be asked before they're added"
                .into()
        }
    }
}

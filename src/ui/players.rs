//! Players screen: the profile search and the follow list, one tab each.
//! Every row is a profile's headline: personal bests on english time
//! 15/30/60 and the medal counts. Enter or `p` opens the profile.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ttyp_core::api::PlayerSummary;

use super::style::{Palette, content_column};
use super::widgets::{Cell, Column, Row, SelectTable, Width, hints};
use crate::app::App;
use crate::online::PlayersTab;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));
    let Some(view) = &app.players else {
        return;
    };
    let follows = app.follows.list();
    let logged_in = app.online.as_ref().is_some_and(|o| o.logged_in());

    // Heading and tabs, then the search line or what the list is.
    let mut tabs = vec![Span::styled("players", p.main_bold()), Span::raw("    ")];
    for (tab, name) in [
        (PlayersTab::Search, "search"),
        (PlayersTab::Following, "following"),
    ] {
        let style = if tab == view.tab {
            p.selected()
        } else {
            p.sub()
        };
        tabs.push(Span::styled(name, style));
        let count = match tab {
            PlayersTab::Search if view.loading && view.found.is_empty() => String::new(),
            PlayersTab::Search => format!(" {}", view.total),
            PlayersTab::Following if app.follows.loaded() => format!(" {}", follows.len()),
            PlayersTab::Following => String::new(),
        };
        tabs.push(Span::styled(format!("{count}    "), p.sub()));
    }
    let second = match view.tab {
        PlayersTab::Search if view.editing => Line::from(vec![
            Span::styled("  / ", p.main()),
            Span::styled(view.query.clone(), p.fg()),
            Span::styled(" ", p.caret()),
        ]),
        PlayersTab::Search if view.query.is_empty() => Line::from(Span::styled(
            "  every public profile · / to search",
            p.sub(),
        )),
        PlayersTab::Search => Line::from(vec![
            Span::styled("  / ", p.sub()),
            Span::styled(view.query.clone(), p.fg()),
        ]),
        PlayersTab::Following => Line::from(Span::styled("  most recently viewed first", p.sub())),
    };
    let top = vec![Line::from(tabs), Line::default(), second, Line::default()];
    let top_h = top.len() as u16;
    frame.render_widget(Paragraph::new(top), body);

    let (players, selection) = match view.tab {
        PlayersTab::Search => (view.found.as_slice(), &view.search),
        PlayersTab::Following => (follows, &view.following),
    };
    let empty = match view.tab {
        PlayersTab::Search => match &view.error {
            Some(e) => Some((e.clone(), p.error())),
            None if view.loading && players.is_empty() => Some(("searching…".into(), p.sub())),
            None if players.is_empty() => Some(("no public profiles match".into(), p.sub())),
            None => None,
        },
        PlayersTab::Following if !logged_in => {
            Some(("log in to follow players (:login)".into(), p.sub()))
        }
        PlayersTab::Following if !app.follows.loaded() => Some(("loading…".into(), p.sub())),
        PlayersTab::Following if players.is_empty() => Some((
            "you don't follow anyone yet · f on a player in search follows them".into(),
            p.sub(),
        )),
        PlayersTab::Following => None,
    };
    let list_area = Rect::new(
        body.x,
        body.y + top_h,
        body.width,
        body.height.saturating_sub(top_h),
    );
    match empty {
        Some((msg, style)) => {
            frame.render_widget(
                Paragraph::new(format!("  {msg}"))
                    .style(style)
                    .wrap(Wrap { trim: false }),
                list_area,
            );
        }
        None => {
            let columns = [
                Column::new("", Width::Fixed(1)),
                Column::new("player", Width::Min(10)),
                Column::new("15s", Width::Fixed(4)).right(),
                Column::new("30s", Width::Fixed(4)).right(),
                Column::new("60s", Width::Fixed(4)).right(),
                Column::new("gold", Width::Fixed(4)).right(),
                Column::new("silver", Width::Fixed(6)).right(),
                Column::new("bronze", Width::Fixed(6)).right(),
                Column::new("other", Width::Fixed(5)).right(),
            ];
            let rows = players.iter().map(row).collect();
            SelectTable::new(&columns, rows, selection).render(frame, list_area, p);
        }
    }

    let hint = match view.tab {
        PlayersTab::Search if view.editing => {
            "type a name · enter profile · ↑↓ move · tab following · esc done"
        }
        PlayersTab::Search if app.selected_player().is_some_and(|p| p.following) => {
            "enter/p profile · f unfollow · / search · tab following · esc back"
        }
        PlayersTab::Search => "enter/p profile · f follow · / search · tab following · esc back",
        PlayersTab::Following => "enter/p profile · f unfollow · tab search · esc back",
    };
    hints::render(frame, area, col, p, hint);
}

/// A player's headline: following mark, name, bests, medals.
fn row(player: &PlayerSummary) -> Row {
    let mark = if player.following {
        Cell::accent("•")
    } else {
        Cell::normal("")
    };
    if !player.public {
        return Row::new(vec![
            mark,
            Cell::normal(&player.login),
            Cell::dim("private profile").span(7),
        ]);
    }
    let mut cells = vec![mark, Cell::normal(&player.login)];
    cells.extend(player.bests.iter().map(|b| match b {
        Some(wpm) => Cell::normal(format!("{wpm:.0}")),
        None => Cell::dim("-"),
    }));
    let b = &player.badges;
    for n in [b.gold, b.silver, b.bronze, b.other] {
        cells.push(if n == 0 {
            Cell::dim("0")
        } else {
            Cell::normal(n.to_string())
        });
    }
    Row::new(cells)
}

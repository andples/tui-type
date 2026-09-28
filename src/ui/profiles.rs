//! Profile screen: the list of profiles with the active ones marked, or the
//! editor that picks which settings a profile controls.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use crate::app::App;
use crate::profile::{Editor, ProfileMenu, SettingKey};

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    // Heading and body start below the brand row; hints sit above the
    // bottom (notice) line.
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));
    let (lines, hints) = match &app.profile_menu {
        ProfileMenu::List {
            selected,
            confirm_delete,
        } => list(app, p, *selected, *confirm_delete, body),
        ProfileMenu::Edit(e) => editor(app, p, e, body),
    };
    frame.render_widget(Paragraph::new(lines), body);
    frame.render_widget(
        Paragraph::new(hints).style(p.sub()),
        Rect::new(col.x, area.bottom().saturating_sub(2), col.width, 1),
    );
}

fn marker(p: &Palette, selected: bool) -> Span<'static> {
    Span::styled(if selected { "› " } else { "  " }, p.main())
}

fn list<'a>(
    app: &'a App,
    p: &Palette,
    selected: usize,
    confirm_delete: bool,
    body: Rect,
) -> (Vec<Line<'a>>, &'static str) {
    let mut lines = vec![
        Line::from(Span::styled("profiles", p.main_bold())),
        Line::default(),
    ];
    let reg = &app.profiles;
    if reg.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no profiles yet — a profile saves any subset of your settings",
            p.sub(),
        )));
        lines.push(Line::default());
    }

    let name_w = reg.names().map(str::len).max().unwrap_or(0).max(8);
    let detail_w = (body.width as usize).saturating_sub(name_w + 8);
    for (i, profile) in reg.iter().enumerate() {
        let is_sel = i == selected;
        let active = app.config.profiles.contains(&profile.name);
        let (dot, dot_style) = if active {
            ("● ", p.main())
        } else {
            ("○ ", p.sub())
        };
        let name_style = if is_sel {
            p.selected()
        } else if active {
            p.fg()
        } else {
            p.sub()
        };
        lines.push(Line::from(vec![
            marker(p, is_sel),
            Span::styled(dot, dot_style),
            Span::styled(format!("{:<name_w$}  ", profile.name), name_style),
            Span::styled(truncate(&profile.settings.summary(), detail_w), p.sub()),
        ]));
    }
    let on_new = selected >= reg.len();
    lines.push(Line::from(vec![
        marker(p, on_new),
        Span::styled("+ new profile", if on_new { p.selected() } else { p.sub() }),
    ]));
    lines.push(Line::default());

    // What the highlighted row would do.
    if let Some(profile) = app.profile_menu.selected_profile(reg) {
        let active = app.config.profiles.contains(&profile.name);
        if confirm_delete {
            lines.push(Line::from(vec![
                Span::styled("  delete ", p.fg()),
                Span::styled(profile.name.clone(), p.error()),
                Span::styled("?  y / n", p.fg()),
            ]));
        } else if active {
            lines.push(Line::from(Span::styled(
                "  active · enter deselects it (settings stay as they are)",
                p.sub(),
            )));
        } else {
            let replaced = reg.conflicts(profile, &app.config.profiles);
            if !replaced.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("  enabling replaces ", p.sub()),
                    Span::styled(replaced.join(", "), p.error()),
                ]));
            }
        }
    }

    let hints = if confirm_delete {
        "y delete · any other key cancels"
    } else if on_new {
        "enter create · ↑↓ move · esc back"
    } else {
        "enter toggle · n new · e edit · d delete · esc back"
    };
    (lines, hints)
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
        marker(p, e.on_name()),
        Span::styled("name  ", if e.on_name() { p.selected() } else { p.sub() }),
    ];
    if e.name.is_empty() && !e.on_name() {
        name.push(Span::styled("(unnamed)", p.error()));
    } else {
        name.push(Span::styled(e.name.clone(), p.fg()));
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
        let mut row = vec![marker(p, is_sel)];
        match e.settings.show(k) {
            Some(value) => {
                row.push(Span::styled("[x] ", p.main()));
                row.push(Span::styled(
                    format!("{:<label_w$}  ", k.label()),
                    if is_sel { p.selected() } else { p.fg() },
                ));
                if value != live {
                    row.push(Span::styled(value, p.fg()));
                    row.push(Span::styled(format!("  (now {live})"), p.sub()));
                } else {
                    row.push(Span::styled(value, p.fg()));
                }
            }
            None => {
                row.push(Span::styled("[ ] ", p.sub()));
                row.push(Span::styled(
                    format!("{:<label_w$}  ", k.label()),
                    if is_sel { p.selected() } else { p.sub() },
                ));
                row.push(Span::styled(live, p.sub()));
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

fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

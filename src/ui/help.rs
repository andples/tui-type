//! Help screen: keys and the command reference, generated from `COMMANDS`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use crate::app::App;
use crate::command::COMMANDS;

const KEYS: &[(&str, &str)] = &[
    ("esc", "open command line"),
    (":", "open command line (when not typing)"),
    ("tab", "restart with new words"),
    ("ctrl+w / ctrl+backspace", "delete word"),
    ("ctrl+= / ctrl+-", "zoom in / out"),
    ("ctrl+c", "quit"),
    ("↑ ↓ / ctrl+p ctrl+n", "move in the palette"),
    ("tab (in palette)", "complete"),
    ("enter (in palette)", "run"),
];

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.zoom_level().0);
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled("keys", p.main_bold())),
        Line::default(),
    ];
    let kw = KEYS.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, d) in KEYS {
        lines.push(Line::from(vec![
            Span::styled(format!("{k:<kw$}  "), p.fg()),
            Span::styled(*d, p.sub()),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("commands", p.main_bold())));
    lines.push(Line::default());
    let cw = COMMANDS
        .iter()
        .map(|c| c.name.len() + 1 + c.usage.len())
        .max()
        .unwrap_or(0);
    for c in COMMANDS {
        let sig = format!("{} {}", c.name, c.usage);
        let aliases = if c.aliases.is_empty() {
            String::new()
        } else {
            format!("  ({})", c.aliases.join(", "))
        };
        lines.push(Line::from(vec![
            Span::styled(":", p.main()),
            Span::styled(format!("{sig:<cw$}  "), p.fg()),
            Span::styled(c.help, p.sub()),
            Span::styled(aliases, p.sub()),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("config  ", p.sub()),
        Span::styled(app.paths.config_file.display().to_string(), p.fg()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("themes  ", p.sub()),
        Span::styled(app.paths.themes_dir.display().to_string(), p.fg()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("history ", p.sub()),
        Span::styled(app.paths.history_file.display().to_string(), p.fg()),
    ]));

    let skip = app.scroll.min(lines.len().saturating_sub(1));
    let shown: Vec<Line> = lines.into_iter().skip(skip).collect();
    frame.render_widget(
        Paragraph::new(shown),
        Rect::new(col.x, col.y + 1, col.width, col.height.saturating_sub(3)),
    );
    frame.render_widget(
        Paragraph::new("esc  back").style(p.sub()),
        Rect::new(col.x, area.bottom().saturating_sub(2), col.width, 1),
    );
}

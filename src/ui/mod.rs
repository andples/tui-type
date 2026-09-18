//! Rendering. `render` paints the background, dispatches to the active
//! screen, then overlays the command line or a transient notice.

pub mod command_line;
pub mod help;
pub mod results;
pub mod stats;
pub mod style;
pub mod typing;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{App, Screen};
use style::{Palette, content_column};

pub fn render(frame: &mut Frame, app: &App) {
    let p = Palette::from_theme(app.theme());
    let area = frame.area();
    frame.render_widget(Block::default().style(p.base()), area);

    // Brand mark, top-left of the content column (hidden in zen mode).
    let col = content_column(area, app.config.zoom_level().0);
    if !(app.config.zen && app.screen == Screen::Typing) {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("t", p.main_bold()),
                Span::styled("typ", p.sub()),
            ])),
            Rect::new(col.x, area.y, col.width, 1),
        );
    }

    // Reserve the bottom row for the command line / notice.
    let body = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));
    match app.screen {
        Screen::Typing => typing::render(frame, app, body, &p),
        Screen::Results => results::render(frame, app, body, &p),
        Screen::Stats => stats::render(frame, app, body, &p),
        Screen::Help => help::render(frame, app, body, &p),
    }

    if app.cmd_open {
        command_line::render(frame, app, area, &p);
    } else if let Some(msg) = app.notice_text() {
        let bottom = Rect::new(col.x, area.bottom().saturating_sub(1), col.width, 1);
        frame.render_widget(Paragraph::new(msg).style(p.sub()), bottom);
    }
}

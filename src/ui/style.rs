//! Theme → ratatui styles, plus shared layout helpers.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::theme::Theme;

/// Minimum side gutter.
pub const GUTTER: u16 = 4;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub fg: Color,
    pub sub: Color,
    pub main: Color,
    pub correct: Color,
    pub error: Color,
    pub error_extra: Color,
}

impl Palette {
    pub fn from_theme(t: &Theme) -> Self {
        let c = &t.colors;
        Self {
            bg: c.bg.color(),
            fg: c.fg.color(),
            sub: c.sub.color(),
            main: c.main.color(),
            correct: c.correct.color(),
            error: c.error.color(),
            error_extra: c.error_extra.color(),
        }
    }

    pub fn base(&self) -> Style {
        Style::default().bg(self.bg).fg(self.fg)
    }
    pub fn fg(&self) -> Style {
        Style::default().fg(self.fg)
    }
    pub fn sub(&self) -> Style {
        Style::default().fg(self.sub)
    }
    pub fn main(&self) -> Style {
        Style::default().fg(self.main)
    }
    pub fn main_bold(&self) -> Style {
        self.main().add_modifier(Modifier::BOLD)
    }
    pub fn correct(&self) -> Style {
        Style::default().fg(self.correct)
    }
    pub fn error(&self) -> Style {
        Style::default().fg(self.error)
    }
    pub fn error_extra(&self) -> Style {
        Style::default()
            .fg(self.error_extra)
            .add_modifier(Modifier::UNDERLINED)
    }
    /// Block caret: inverted accent.
    pub fn caret(&self) -> Style {
        Style::default().bg(self.main).fg(self.bg)
    }
    pub fn selected(&self) -> Style {
        Style::default().fg(self.main).add_modifier(Modifier::BOLD)
    }
}

/// The centered content column inside `area`, at most `max_width` wide.
pub fn content_column(area: Rect, max_width: u16) -> Rect {
    let width = area
        .width
        .saturating_sub(GUTTER * 2)
        .clamp(10, max_width.max(10));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    Rect::new(x, area.y, width, area.height)
}

/// A block of `height` rows vertically centered in `area` (biased slightly
/// upward, which reads better than true center).
pub fn vcenter(area: Rect, height: u16) -> Rect {
    let height = height.min(area.height);
    let free = area.height.saturating_sub(height);
    let y = area.y + free * 2 / 5;
    Rect::new(area.x, y, area.width, height)
}

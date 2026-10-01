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

/// A colour `t` of the way from `a` to `b` (0 is `a`, 1 is `b`), for
/// shades between two theme colours. Non-RGB colours snap to the nearer end.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            Color::Rgb(m(ar, br), m(ag, bg), m(ab, bb))
        }
        _ if t < 0.5 => a,
        _ => b,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_blends_rgb() {
        let a = Color::Rgb(0, 100, 200);
        let b = Color::Rgb(100, 200, 0);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), Color::Rgb(50, 150, 100));
        assert_eq!(mix(a, b, 7.0), b);
        assert_eq!(mix(Color::Red, b, 0.2), Color::Red);
        assert_eq!(mix(Color::Red, b, 0.8), b);
    }
}

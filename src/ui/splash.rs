//! Landing screen: the logo in the big pixel font, typed out with a caret,
//! then a tagline and the keys that fit the online state. Falls back to a
//! one-row text logo when no block size fits. When nobody types for a
//! while, the screensaver (`app::idle`) takes over the same space.

use std::time::Instant;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::bigtext;
use super::style::{Palette, content_column, vcenter};
use super::widgets::cursor;
use crate::app::App;
use crate::app::idle::{CRUMBLE, GhostFrame, IdleView};
use crate::app::splash::LOGO;
use crate::config::{BlockSet, FontSize};

const TAGLINE: &str = "a minimal typing test for the terminal";
/// Tallest logo, in rows, fullscreen or not: calm, not a billboard.
const LOGO_ROWS: u16 = 8;

/// The largest block size at which the logo (plus a caret slot) fits in
/// `width` × `rows`; `None` when only the terminal font does.
fn logo_size(width: u16, rows: u16) -> Option<FontSize> {
    text_size(LOGO.len() as u16 + 1, width, rows)
}

/// The largest block size at which `slots` glyphs fit in `width` × `rows`.
/// Half blocks only: they draw crisply in every terminal font, unlike
/// sextants.
fn text_size(slots: u16, width: u16, rows: u16) -> Option<FontSize> {
    (1..=9)
        .rev()
        .map(|s| FontSize::blocks(BlockSet::Half, s))
        .find(|f| {
            let (w, h) = f.cell_dims();
            h <= rows && w * slots <= width
        })
}

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let state = app.splash_frame();
    let items = app.splash_menu().items();
    let col = if app.config.fullscreen {
        Rect::new(
            area.x + 1,
            area.y,
            area.width.saturating_sub(2),
            area.height,
        )
    } else {
        content_column(area, app.config.content_width())
    };

    // Leave the brand row at the top and the hint line at the bottom.
    let usable = Rect::new(col.x, area.y + 1, col.width, area.height.saturating_sub(3));
    // Rows under the logo: a gap and the tagline, a gap and the keys. They
    // keep their room during the intro so the logo doesn't jump when they
    // appear. In a short terminal the tagline goes first, then the keys
    // (they still work).
    let menu_rows = 1 + items.len() as u16;
    let mut show_menu = !app.config.zen;
    let mut show_tagline = true;
    let below = |tagline: bool, menu: bool| 2 * tagline as u16 + menu_rows * menu as u16;
    if 1 + below(show_tagline, show_menu) > usable.height {
        show_tagline = false;
    }
    if 1 + below(show_tagline, show_menu) > usable.height {
        show_menu = false;
    }
    if let Some(view) = app.idle.view_at(Instant::now()) {
        draw_idle(frame, usable, col, &view, p);
        return;
    }
    let below = below(show_tagline, show_menu);
    let size = logo_size(
        col.width,
        usable.height.saturating_sub(below).min(LOGO_ROWS),
    );
    let logo_h = size.map_or(1, |f| f.cell_dims().1);
    let block = vcenter(usable, logo_h + below);

    let logo_area = Rect::new(block.x, block.y, block.width, logo_h);
    match size {
        Some(f) => draw_big_logo(frame, logo_area, f, state.letters, p),
        None => draw_text_logo(frame, logo_area, state.letters, p),
    }
    if !state.done {
        return;
    }

    let centred = |text: &'static str, y: u16| {
        let row = Rect::new(col.x, y, col.width, 1).intersection(area);
        (
            Paragraph::new(text)
                .style(p.sub())
                .alignment(Alignment::Center),
            row,
        )
    };
    let mut y = block.y + logo_h;
    if show_tagline {
        let (w, row) = centred(fitting(&[TAGLINE, "a typing test"], col.width), y + 1);
        frame.render_widget(w, row);
        y += 2;
    }
    if !show_menu {
        return;
    }
    y += 1;
    // `› label   key`: the selection look follows the cursor (enter runs
    // it); the key is the shortcut.
    let selected = app.splash.map_or(0, |s| s.selected);
    let label_w = items.iter().map(|it| it.label.len()).max().unwrap_or(0);
    let key_w = items.iter().map(|it| it.key.len()).max().unwrap_or(0);
    let menu_w = cursor::SELECTED_GUTTER + (label_w + 3 + key_w) as u16;
    let x = col.x + col.width.saturating_sub(menu_w) / 2;
    for (i, it) in items.iter().enumerate() {
        let on = i == selected;
        let row = Rect::new(x, y, menu_w.min(col.width), 1).intersection(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                cursor::lead(p, on),
                Span::styled(
                    format!("{:<label_w$}   ", it.label),
                    cursor::style(p, on, p.fg()),
                ),
                Span::styled(it.key, cursor::style(p, on, p.sub())),
            ])),
            row,
        );
        y += 1;
    }
    // The key line, centred like the rest of the screen.
    let hint = fitting(
        &[
            "↑↓ enter choose · any other key to start · : commands",
            "any key to start",
        ],
        col.width,
    );
    let (w, row) = centred(hint, area.bottom().saturating_sub(2));
    frame.render_widget(w, row);
}

/// The first of `options` that fits in `width` columns, else nothing.
fn fitting(options: &[&'static str], width: u16) -> &'static str {
    options
        .iter()
        .find(|t| t.chars().count() <= width as usize)
        .copied()
        .unwrap_or("")
}

/// The logo in the pixel font, centred as four letters; the caret sits in
/// the slot after the last letter shown.
fn draw_big_logo(frame: &mut Frame, area: Rect, size: FontSize, letters: usize, p: &Palette) {
    let (gw, _) = size.cell_dims();
    let x0 = area.x + area.width.saturating_sub(gw * LOGO.len() as u16) / 2;
    let clip = area.intersection(frame.area());
    let buf = frame.buffer_mut();
    for (i, ch) in LOGO.chars().take(letters).enumerate() {
        let style = if i == 0 { p.main_bold() } else { p.sub() };
        bigtext::draw_glyph(buf, clip, x0 + i as u16 * gw, area.y, ch, style, size);
    }
    let caret_x = x0 + letters as u16 * gw;
    bigtext::draw_glyph(buf, clip, caret_x, area.y, '|', p.main(), size);
}

/// The screensaver: the ghost's phrase in the big font, or that frame
/// falling apart.
fn draw_idle(frame: &mut Frame, usable: Rect, col: Rect, view: &IdleView, p: &Palette) {
    let (ghost, t) = match view {
        IdleView::Typing(g) => (g, None),
        IdleView::Crumbling { frame, t } => (frame, Some(*t)),
    };
    let slots = ghost.phrase.chars().count() as u16 + 1;
    let size = text_size(slots, col.width, usable.height.min(LOGO_ROWS));
    let h = size.map_or(1, |f| f.cell_dims().1);
    let row = vcenter(Rect::new(col.x, usable.y, col.width, usable.height), h);
    let mut scratch = Buffer::empty(row);
    paint_ghost(&mut scratch, size, ghost, p);
    let screen = frame.area();
    let buf = frame.buffer_mut();
    for y in row.top()..row.bottom() {
        for x in row.left()..row.right() {
            let cell = &scratch[(x, y)];
            if cell.symbol() == " " && cell.bg == Color::Reset {
                continue;
            }
            let Some(t) = t else {
                if let Some(c) = buf.cell_mut((x, y)) {
                    *c = cell.clone();
                }
                continue;
            };
            // Each piece lets go after its own delay, then falls and
            // drifts a little, dimming as it goes.
            let h = hash(x, y);
            let delay = (h % 450) as f32 / 1000.0;
            let s = t.as_secs_f32() - delay;
            if s <= 0.0 {
                if let Some(c) = buf.cell_mut((x, y)) {
                    *c = cell.clone();
                }
                continue;
            }
            let fall = 0.5 * 60.0 * s * s;
            let drift = ((h >> 9) % 5) as f32 - 2.0;
            let (nx, ny) = (x as f32 + drift * s * 2.5, y as f32 + fall);
            if nx < screen.left() as f32
                || ny >= screen.bottom() as f32
                || nx >= screen.right() as f32
            {
                continue;
            }
            const CRUMBS: [char; 7] = ['▘', '▝', '▖', '▗', '▪', '·', '.'];
            let crumb = CRUMBS[(h >> 4) as usize % CRUMBS.len()];
            let colour = if cell.bg != Color::Reset {
                cell.bg
            } else {
                cell.fg
            };
            let style = if s > CRUMBLE.as_secs_f32() * 0.3 {
                p.sub()
            } else {
                Style::default().fg(colour)
            };
            if let Some(c) = buf.cell_mut((nx as u16, ny as u16)) {
                c.set_char(crumb).set_style(style);
            }
        }
    }
}

/// The ghost's frame into `buf`: typed letters, a typo in the error
/// colour, the block caret, then the rest dimmed; centred.
fn paint_ghost(buf: &mut Buffer, size: Option<FontSize>, g: &GhostFrame, p: &Palette) {
    let area = buf.area;
    let chars: Vec<char> = g.phrase.chars().collect();
    let caret = g.typed + usize::from(g.wrong.is_some());
    let styled = |i: usize| -> (char, Style) {
        if i < g.typed {
            (chars[i], p.fg())
        } else if i == g.typed && g.wrong.is_some() {
            (g.wrong.unwrap_or(' '), p.error())
        } else if i == caret {
            (chars.get(i).copied().unwrap_or(' '), p.caret())
        } else {
            (chars.get(i).copied().unwrap_or(' '), p.sub())
        }
    };
    let slots = chars.len() + 1;
    match size {
        Some(f) => {
            let (gw, _) = f.cell_dims();
            let x0 = area.x + area.width.saturating_sub(gw * slots as u16) / 2;
            for i in 0..slots {
                let (ch, style) = styled(i);
                bigtext::draw_glyph(buf, area, x0 + i as u16 * gw, area.y, ch, style, f);
            }
        }
        None => {
            let x0 = area.x + area.width.saturating_sub(slots as u16) / 2;
            for i in 0..slots {
                let (ch, style) = styled(i);
                if let Some(c) = buf.cell_mut((x0 + i as u16, area.y)) {
                    c.set_char(ch).set_style(style);
                }
            }
        }
    }
}

/// A cheap, stable scramble of a cell position.
fn hash(x: u16, y: u16) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B9) ^ (y as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

/// One row: the brand mark as text with a block caret.
fn draw_text_logo(frame: &mut Frame, area: Rect, letters: usize, p: &Palette) {
    let mut spans: Vec<Span> = LOGO
        .chars()
        .take(letters)
        .enumerate()
        .map(|(i, c)| {
            let style = if i == 0 { p.main_bold() } else { p.sub() };
            Span::styled(c.to_string(), style)
        })
        .collect();
    // Pad to the full word so the text doesn't shift as it types.
    spans.push(Span::styled(" ", p.caret()));
    spans.push(Span::raw(" ".repeat(LOGO.len() - letters)));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_shrinks_to_fit_then_gives_up() {
        let (w, h) = logo_size(80, 8).unwrap().cell_dims();
        assert!(h <= 8 && w * 5 <= 80);
        assert_eq!(logo_size(80, 8), Some(FontSize::from_level(5)));
        let small = logo_size(52, 6).unwrap().cell_dims();
        assert!(small.1 <= 6 && small.0 * 5 <= 52);
        assert_eq!(logo_size(80, 3), Some(FontSize::from_level(3)));
        assert_eq!(logo_size(19, 8), None, "too narrow for any block size");
        assert_eq!(logo_size(80, 2), None, "too short for any block size");
    }

    #[test]
    fn text_falls_back_to_what_fits() {
        assert_eq!(fitting(&["long one", "short"], 8), "long one");
        assert_eq!(fitting(&["long one", "short"], 7), "short");
        assert_eq!(fitting(&["long one", "short"], 4), "");
    }
}

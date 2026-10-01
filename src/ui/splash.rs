//! Landing screen: the logo in the big pixel font, typed out with a caret,
//! then a tagline and the keys that fit the online state. Falls back to a
//! one-row text logo when no block size fits. When nobody types for a
//! while, the screensaver (`app::idle`) takes over the same space.

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column, vcenter};
use super::widgets::cursor;
use super::{bigtext, typing};
use crate::app::App;
use crate::app::idle::{CRUMBLE, GhostFrame, IdleView};
use crate::app::splash::LOGO;
use crate::config::{BlockSet, FONT_SIZE_RANGE, FontSize};
use crate::gfx::ImageLine;

const TAGLINE: &str = "a minimal typing test for the terminal";
/// Tallest logo, in rows, fullscreen or not: calm, not a billboard.
const LOGO_ROWS: u16 = 8;
/// How many font sizes above the typing text the screensaver draws.
const IDLE_GROW: u8 = 3;

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

/// Draws the landing screen. Returns the screensaver's text as real-font
/// images when the terminal shows them (empty otherwise).
pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) -> Vec<ImageLine> {
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
        return draw_idle(frame, app, usable, col, &view, p);
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
        return Vec::new();
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
        return Vec::new();
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
    Vec::new()
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

/// The screensaver's text size: the typing screen's font a few sizes up,
/// as large as fits `slots` glyphs in `width` × `rows`.
fn idle_size(app: &App, slots: u16, width: u16, rows: u16) -> FontSize {
    let top = (app.config.font_size + IDLE_GROW).min(FONT_SIZE_RANGE.1);
    (FONT_SIZE_RANGE.0..=top)
        .rev()
        .map(FontSize::from_level)
        .find(|f| f.cell_dims().1 <= rows && app.glyph_cols(*f) * slots as f32 <= width as f32)
        .unwrap_or(FontSize::from_level(1))
}

/// The ghost's cells, styled like the typing screen: typed letters, a typo
/// in the error colour, the block caret, then the rest dimmed.
fn ghost_cells(g: &GhostFrame, p: &Palette) -> Vec<(char, Style)> {
    let chars: Vec<char> = g.phrase.chars().collect();
    let caret = g.typed + usize::from(g.wrong.is_some());
    (0..=chars.len())
        .map(|i| {
            if i < g.typed {
                (chars[i], p.correct())
            } else if i == g.typed && g.wrong.is_some() {
                (g.wrong.unwrap_or(' '), p.error())
            } else if i == caret {
                (chars.get(i).copied().unwrap_or(' '), p.caret())
            } else {
                (chars.get(i).copied().unwrap_or(' '), p.sub())
            }
        })
        .collect()
}

/// When glyph or cell `h` lets go in the crumble and where it has got to
/// `t` in: (seconds falling, rows fallen, columns drifted); `None` while it
/// still hangs on.
fn crumble_at(h: u32, t: Duration) -> Option<(f32, f32, f32)> {
    let delay = (h % 450) as f32 / 1000.0;
    let s = t.as_secs_f32() - delay;
    if s <= 0.0 {
        return None;
    }
    let drift = ((h >> 9) % 5) as f32 - 2.0;
    Some((s, 0.5 * 60.0 * s * s, drift * s * 2.5))
}

/// Dimmed once a falling piece is well on its way.
fn crumble_faded(s: f32) -> bool {
    s > CRUMBLE.as_secs_f32() * 0.3
}

/// The screensaver: the ghost's phrase drawn like the typing text, a few
/// sizes larger, or that frame falling apart. Real-font images when the
/// terminal shows them, else the same cells the typing screen would use.
fn draw_idle(
    frame: &mut Frame,
    app: &App,
    usable: Rect,
    col: Rect,
    view: &IdleView,
    p: &Palette,
) -> Vec<ImageLine> {
    let (ghost, t) = match view {
        IdleView::Typing(g) => (g, None),
        IdleView::Crumbling { frame, t } => (frame, Some(*t)),
    };
    let cells = ghost_cells(ghost, p);
    let slots = cells.len() as u16;
    let size = idle_size(app, slots, col.width, usable.height);
    let (gw, gh) = size.cell_dims();
    let row = vcenter(Rect::new(col.x, usable.y, col.width, usable.height), gh);
    if app.gfx.metrics(size).is_some() {
        return idle_images(frame.area(), app, size, row, &cells, t, p);
    }
    let width = slots * gw;
    let x0 = row.x + row.width.saturating_sub(width) / 2;
    let mut scratch = Buffer::empty(row);
    for (i, (ch, style)) in cells.iter().enumerate() {
        let x = x0 + i as u16 * gw;
        if size.is_native() {
            if let Some(c) = scratch.cell_mut((x, row.y)) {
                c.set_char(*ch).set_style(*style);
            }
        } else {
            bigtext::draw_glyph(&mut scratch, row, x, row.y, *ch, *style, size);
        }
    }
    let screen = frame.area();
    let buf = frame.buffer_mut();
    for y in row.top()..row.bottom() {
        for x in row.left()..row.right() {
            let cell = &scratch[(x, y)];
            if cell.symbol() == " " && cell.bg == Color::Reset {
                continue;
            }
            // Each piece lets go after its own delay, then falls and
            // drifts a little, dimming as it goes.
            let Some((s, fall, drift)) = t.and_then(|t| crumble_at(hash(x, y), t)) else {
                if let Some(c) = buf.cell_mut((x, y)) {
                    *c = cell.clone();
                }
                continue;
            };
            let (nx, ny) = (x as f32 + drift, y as f32 + fall);
            if nx < screen.left() as f32
                || ny >= screen.bottom() as f32
                || nx >= screen.right() as f32
            {
                continue;
            }
            const CRUMBS: [char; 7] = ['▘', '▝', '▖', '▗', '▪', '·', '.'];
            let crumb = CRUMBS[(hash(x, y) >> 4) as usize % CRUMBS.len()];
            let colour = if cell.bg != Color::Reset {
                cell.bg
            } else {
                cell.fg
            };
            let style = if crumble_faded(s) {
                p.sub()
            } else {
                Style::default().fg(colour)
            };
            if let Some(c) = buf.cell_mut((nx as u16, ny as u16)) {
                c.set_char(crumb).set_style(style);
            }
        }
    }
    Vec::new()
}

/// The ghost as real-font images, centred in `row`. Crumbling, each letter
/// drops out of the line as its own image and falls off the screen.
fn idle_images(
    screen: Rect,
    app: &App,
    size: FontSize,
    row: Rect,
    cells: &[(char, Style)],
    t: Option<Duration>,
    p: &Palette,
) -> Vec<ImageLine> {
    let gc = app.glyph_cols(size);
    let width = (gc * cells.len() as f32).ceil() as u16;
    let x0 = row.x + row.width.saturating_sub(width) / 2;
    let mut line = Vec::with_capacity(cells.len());
    let mut images = Vec::new();
    for (i, (ch, style)) in cells.iter().enumerate() {
        let fallen = t.and_then(|t| crumble_at(hash(i as u16, 0), t));
        let Some((s, fall, drift)) = fallen else {
            line.push(typing::to_glyph((*ch, *style), p));
            continue;
        };
        // Its place in the line stays, empty.
        line.push(typing::to_glyph((' ', p.sub()), p));
        let x = x0 as f32 + i as f32 * gc + drift;
        let y = row.y as f32 + fall;
        let (gh, gw) = (row.height as f32, gc.ceil());
        if x < screen.left() as f32
            || x + gw > screen.right() as f32
            || y + gh > screen.bottom() as f32
        {
            continue;
        }
        let style = if crumble_faded(s) { p.sub() } else { *style };
        images.push(ImageLine {
            area: Rect::new(x as u16, y as u16, gw as u16, row.height),
            glyphs: vec![typing::to_glyph((*ch, style), p)],
        });
    }
    images.insert(
        0,
        ImageLine {
            area: Rect::new(x0, row.y, width, row.height),
            glyphs: line,
        },
    );
    images
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

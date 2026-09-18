//! Rasterizes the pixel font in `font.rs` into block characters so the
//! typing text can be shown larger than the terminal font. The style's
//! foreground paints "on" pixels and its background fills the whole glyph
//! box, which is how the inverted caret works.
//!
//! Two block sets are used: sextants (2×3 pixels per cell, Unicode 13) for
//! the compact sizes and half blocks (1×2) for the larger ones. Both keep the
//! glyph's aspect ratio in a terminal whose cells are about twice as tall as
//! they are wide.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::font::{self, GLYPH_H, GLYPH_W};
use crate::config::{BlockSet, FontSize};

/// Sextant character for a 6-bit pattern, bits in reading order
/// (top-left = bit 0 … bottom-right = bit 5).
fn sextant(bits: u8) -> char {
    match bits {
        0 => ' ',
        0b010101 => '▌',
        0b101010 => '▐',
        0b111111 => '█',
        n => {
            // U+1FB00.. skips the two half-block patterns above.
            let skip = (n > 0b010101) as u32 + (n > 0b101010) as u32;
            char::from_u32(0x1FB00 + n as u32 - 1 - skip).unwrap_or('?')
        }
    }
}

fn half_block(top: bool, bottom: bool) -> char {
    match (top, bottom) {
        (true, true) => '█',
        (true, false) => '▀',
        (false, true) => '▄',
        (false, false) => ' ',
    }
}

/// Draw `ch` with its top-left at (`x`, `y`), clipped to `clip`.
pub fn draw_glyph(
    buf: &mut Buffer,
    clip: Rect,
    x: u16,
    y: u16,
    ch: char,
    style: Style,
    size: FontSize,
) {
    let Some((set, scale)) = size.raster() else {
        return;
    };
    let (cols, rows) = size.cell_dims();
    let (pw, ph) = set.pixels_per_cell();
    let g = font::glyph(ch);
    // Pixel (px, py) of the scaled glyph box maps back to the 4×6 source.
    let on = |px: u16, py: u16| {
        let (sx, sy) = (px / scale, py / scale);
        sx < GLYPH_W && sy < GLYPH_H && font::pixel(&g, sx, sy)
    };
    for cy in 0..rows {
        for cx in 0..cols {
            let (tx, ty) = (x + cx, y + cy);
            if tx < clip.x || tx >= clip.right() || ty < clip.y || ty >= clip.bottom() {
                continue;
            }
            let (x0, y0) = (cx * pw, cy * ph);
            let glyph = match set {
                BlockSet::Sextant => {
                    let mut bits = 0u8;
                    for dy in 0..3 {
                        for dx in 0..2 {
                            if on(x0 + dx, y0 + dy) {
                                bits |= 1 << (dy * 2 + dx);
                            }
                        }
                    }
                    sextant(bits)
                }
                BlockSet::Half => half_block(on(x0, y0), on(x0, y0 + 1)),
            };
            if let Some(cell) = buf.cell_mut((tx, ty)) {
                cell.set_char(glyph).set_style(style);
            }
        }
    }
}

/// ASCII dump of a glyph at a given size, for tests and eyeballing.
#[cfg(test)]
pub fn dump(ch: char, size: FontSize) -> String {
    let (cols, rows) = size.cell_dims();
    let mut buf = Buffer::empty(Rect::new(0, 0, cols, rows));
    let clip = buf.area;
    draw_glyph(&mut buf, clip, 0, 0, ch, Style::default(), size);
    (0..rows)
        .map(|y| {
            (0..cols)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sextant_mapping_matches_unicode() {
        let cp = |c: char| c as u32;
        assert_eq!(cp(sextant(0b000001)), 0x1FB00); // top-left only
        assert_eq!(cp(sextant(0b000010)), 0x1FB01); // top-right only
        assert_eq!(cp(sextant(0b010100)), 0x1FB13); // left column minus top
        assert_eq!(cp(sextant(0b010110)), 0x1FB14); // first after the ▌ skip
        assert_eq!(cp(sextant(0b101011)), 0x1FB28); // first after the ▐ skip
        assert_eq!(cp(sextant(0b111110)), 0x1FB3B); // last sextant
        assert_eq!(sextant(0b010101), '▌');
        assert_eq!(sextant(0b101010), '▐');
    }

    #[test]
    fn half_block_render_of_l() {
        // 'l' is a vertical bar with a foot: 4 cols × 3 rows.
        let out = dump('l', FontSize::from_level(3));
        assert_eq!(out, " █  \n █  \n ▀▀ ");
    }

    #[test]
    fn space_is_blank_and_caret_bg_fills_box() {
        let out = dump(' ', FontSize::from_level(2));
        assert!(out.chars().all(|c| c == ' ' || c == '\n'));
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 2));
        let clip = buf.area;
        let style = Style::default().bg(ratatui::style::Color::Red);
        draw_glyph(&mut buf, clip, 0, 0, ' ', style, FontSize::from_level(2));
        assert_eq!(buf[(1, 1)].bg, ratatui::style::Color::Red);
    }
}

#[cfg(test)]
mod eyeball {
    use super::*;

    fn row_of(chars: &str, size: FontSize) -> String {
        let (cols, rows) = size.cell_dims();
        let mut buf = Buffer::empty(Rect::new(0, 0, cols * chars.chars().count() as u16, rows));
        let clip = buf.area;
        for (i, c) in chars.chars().enumerate() {
            draw_glyph(
                &mut buf,
                clip,
                i as u16 * cols,
                0,
                c,
                Style::default(),
                size,
            );
        }
        (0..rows)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    #[ignore]
    fn print_alphabet() {
        for line in [
            "abcdefghijklm",
            "nopqrstuvwxyz",
            "ABCDEFGHIJKLM",
            "NOPQRSTUVWXYZ",
            "0123456789",
            ".,:;!?'\"()[]-_",
            "the quick brown fox jumps over the lazy dog",
        ] {
            println!("{}\n", row_of(line, FontSize::from_level(3)));
        }
        println!(
            "{}\n",
            row_of(
                "the quick brown fox jumps over the lazy dog",
                FontSize::from_level(2)
            )
        );
    }
}

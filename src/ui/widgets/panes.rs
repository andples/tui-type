//! Fit N panes into a content column: side by side when each gets at least
//! its minimum width, otherwise one at a time behind a tab strip.

use ratatui::layout::Rect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Panes {
    /// One rect per pane, left to right.
    SideBySide(Vec<Rect>),
    /// Not enough room: draw the active pane in this rect under a tab strip.
    Tabs(Rect),
}

/// Split `area` into `n` equal panes separated by `gap` columns, or fall
/// back to tabs when a pane would be narrower than `min_width`.
pub fn split(area: Rect, n: usize, min_width: u16, gap: u16) -> Panes {
    let n = n.max(1) as u16;
    let gaps = gap * (n - 1);
    let each = area.width.saturating_sub(gaps) / n;
    if n == 1 || each < min_width {
        return Panes::Tabs(area);
    }
    let rects = (0..n)
        .map(|i| Rect::new(area.x + i * (each + gap), area.y, each, area.height))
        .collect();
    Panes::SideBySide(rects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_when_wide_enough() {
        let area = Rect::new(2, 0, 101, 10);
        match split(area, 2, 40, 3) {
            Panes::SideBySide(r) => {
                assert_eq!(r[0], Rect::new(2, 0, 49, 10));
                assert_eq!(r[1], Rect::new(54, 0, 49, 10));
            }
            Panes::Tabs(_) => panic!("expected two panes"),
        }
        assert_eq!(split(area, 2, 60, 3), Panes::Tabs(area));
        assert_eq!(split(area, 1, 10, 3), Panes::Tabs(area));
    }
}

//! Cursor over a list of `len` rows, with the scroll window that keeps it in
//! view. No rendering: the profile menu and leaderboard own one of these and
//! `SelectTable` draws it.

use std::cell::Cell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub selected: usize,
    pub len: usize,
    /// Moving past either end wraps around instead of stopping.
    pub wrap: bool,
    // The renderer only has `&Selection`, so the two values it feeds back
    // (where the window starts and how many rows it showed) are cells.
    offset: Cell<usize>,
    viewport: Cell<usize>,
}

impl Selection {
    /// A cursor that wraps at both ends (menus).
    pub fn wrapping(len: usize) -> Self {
        Self::new(len, true)
    }

    /// A cursor that stops at both ends (long tables).
    pub fn clamped(len: usize) -> Self {
        Self::new(len, false)
    }

    fn new(len: usize, wrap: bool) -> Self {
        Self {
            selected: 0,
            len,
            wrap,
            offset: Cell::new(0),
            viewport: Cell::new(0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// First visible row, as of the last render.
    pub fn offset(&self) -> usize {
        self.offset.get()
    }

    /// Rows the last render had room for (0 before the first render).
    pub fn viewport(&self) -> usize {
        self.viewport.get()
    }

    pub fn select(&mut self, i: usize) {
        self.selected = i.min(self.len.saturating_sub(1));
        self.ensure_visible(self.viewport());
    }

    /// Resize the list, keeping the cursor in range.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.select(self.selected);
    }

    pub fn move_by(&mut self, delta: isize) {
        if self.len == 0 {
            return;
        }
        let target = self.selected as isize + delta;
        let i = if self.wrap {
            target.rem_euclid(self.len as isize) as usize
        } else {
            target.clamp(0, self.len as isize - 1) as usize
        };
        self.select(i);
    }

    /// Move by one screenful in `dir`; never wraps.
    pub fn page(&mut self, dir: isize) {
        let rows = self.viewport().max(1) as isize;
        let target = self.selected as isize + dir.signum() * rows;
        self.select(target.clamp(0, self.len.saturating_sub(1) as isize) as usize);
    }

    pub fn home(&mut self) {
        self.select(0);
    }

    pub fn end(&mut self) {
        self.select(self.len.saturating_sub(1));
    }

    /// True when the cursor is within `margin` rows of the end (time to
    /// fetch more).
    pub fn near_end(&self, margin: usize) -> bool {
        self.selected + margin + 1 >= self.len
    }

    /// Scroll the window of `height` rows the least amount that shows the
    /// cursor, and remember the height for `page`.
    pub fn ensure_visible(&self, height: usize) {
        if height == 0 {
            return;
        }
        self.viewport.set(height);
        let mut offset = self.offset.get();
        if self.selected < offset {
            offset = self.selected;
        } else if self.selected >= offset + height {
            offset = self.selected + 1 - height;
        }
        // Don't leave blank rows below the list after it shrinks.
        offset = offset.min(self.len.saturating_sub(height));
        self.offset.set(offset);
    }

    /// Is row `i` inside the window as of the last render?
    pub fn is_visible(&self, i: usize) -> bool {
        let offset = self.offset();
        i >= offset && i < offset + self.viewport()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_and_clamping() {
        let mut w = Selection::wrapping(3);
        w.move_by(-1);
        assert_eq!(w.selected, 2);
        w.move_by(1);
        assert_eq!(w.selected, 0);
        let mut c = Selection::clamped(3);
        c.move_by(-1);
        assert_eq!(c.selected, 0);
        c.move_by(10);
        assert_eq!(c.selected, 2);
        let mut e = Selection::wrapping(0);
        e.move_by(1);
        assert_eq!(e.selected, 0);
    }

    #[test]
    fn window_follows_cursor() {
        let mut s = Selection::clamped(10);
        s.ensure_visible(4);
        assert_eq!(s.offset(), 0);
        s.select(5);
        assert_eq!(s.offset(), 2);
        assert!(s.is_visible(5) && !s.is_visible(1));
        s.select(1);
        assert_eq!(s.offset(), 1);
        s.end();
        assert_eq!(s.offset(), 6);
        // Shrinking the list pulls the window back up.
        s.set_len(5);
        assert_eq!(s.selected, 4);
        assert_eq!(s.offset(), 1);
    }

    #[test]
    fn paging_uses_last_viewport() {
        let mut s = Selection::clamped(20);
        s.page(1);
        assert_eq!(s.selected, 1, "unknown viewport pages one row");
        s.ensure_visible(6);
        s.page(1);
        assert_eq!(s.selected, 7);
        s.page(-1);
        assert_eq!(s.selected, 1);
        s.home();
        assert_eq!((s.selected, s.offset()), (0, 0));
        assert!(!s.near_end(2));
        s.select(17);
        assert!(s.near_end(2));
    }
}

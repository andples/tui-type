//! `:hud`: which live readouts sit above the words and in what order
//! (`config::Hud`). Every change applies and saves at once, so a config
//! profile picks it up like any other setting; the screen shows a preview
//! of the line. The list is `Hud::menu_order` (shown items first, in
//! order, then hidden ones) and `App::hud_selection` is the cursor in it.

use super::action::HudAction;
use super::{App, Screen};
use crate::config::HudItem;

impl App {
    /// `:hud` with no item: the menu.
    pub(super) fn open_hud(&mut self) {
        self.hud_selection.set_len(HudItem::ALL.len());
        self.hud_selection.home();
        self.push_screen(Screen::Hud);
    }

    /// The item under the cursor.
    pub fn hud_selected(&self) -> Option<HudItem> {
        self.config
            .hud
            .menu_order()
            .get(self.hud_selection.selected)
            .map(|(item, _)| *item)
    }

    pub(super) fn hud_action(&mut self, action: HudAction) {
        use HudAction as H;
        let item = self.hud_selected();
        match action {
            H::Move(d) => self.hud_selection.move_by(d),
            H::Top => self.hud_selection.home(),
            H::Bottom => self.hud_selection.end(),
            H::Toggle => {
                if let Some(item) = item {
                    self.config.hud.toggle(item);
                    self.hud_changed(item);
                }
            }
            H::Shift(d) => {
                if let Some(item) = item.filter(|i| self.config.hud.shows(*i)) {
                    self.config.hud.shift(item, d);
                    self.hud_changed(item);
                }
            }
        }
    }

    /// Keep the cursor on `item` where it moved to, and save.
    fn hud_changed(&mut self, item: HudItem) {
        if let Some(i) = self
            .config
            .hud
            .menu_order()
            .iter()
            .position(|(x, _)| *x == item)
        {
            self.hud_selection.select(i);
        }
        self.save_config();
    }

    /// `:hud <item> [on|off]`.
    pub(super) fn set_hud_item(&mut self, item: HudItem, on: Option<bool>) {
        let on = on.unwrap_or(!self.config.hud.shows(item));
        self.config.hud.set(item, on);
        self.notify(format!(
            "{} {} · hud: {}",
            item.label(),
            if on { "on" } else { "off" },
            self.config.hud.label()
        ));
    }
}

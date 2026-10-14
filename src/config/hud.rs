//! The HUD: live readouts in the typing screen's top-left corner, in the
//! order the player put them. Written to TOML as a list of names
//! (`hud = ["timer", "wpm"]`); an item not in the list is hidden.
//!
//! Adding an item: a `HudItem` variant with its `name`/`label`/`about`, an
//! entry in `HudItem::ALL`, and its text in `ui::typing::hud_text`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudItem {
    /// Time left (seconds), or words typed out of the total.
    Timer,
    /// Net wpm so far.
    Wpm,
}

impl HudItem {
    /// Every item, in the order the menu lists hidden ones.
    pub const ALL: [HudItem; 2] = [HudItem::Timer, HudItem::Wpm];

    /// The name in the config file and on the command line.
    pub fn name(self) -> &'static str {
        match self {
            HudItem::Timer => "timer",
            HudItem::Wpm => "wpm",
        }
    }

    /// The name in the menu.
    pub fn label(self) -> &'static str {
        match self {
            HudItem::Timer => "timer",
            HudItem::Wpm => "live wpm",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            HudItem::Timer => "seconds left, or words typed out of the total",
            HudItem::Wpm => "your speed so far, updated as you type",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim().to_lowercase();
        let found = match s.as_str() {
            "time" | "timeleft" | "counter" => Some(HudItem::Timer),
            "speed" | "livewpm" | "live_wpm" => Some(HudItem::Wpm),
            _ => HudItem::ALL.into_iter().find(|i| i.name() == s),
        };
        found.ok_or_else(|| {
            let names: Vec<&str> = HudItem::ALL.iter().map(|i| i.name()).collect();
            format!("unknown hud item `{s}` (one of {})", names.join(", "))
        })
    }
}

/// The shown items, left to right. Names this version doesn't know (from
/// a newer ttyp, through a synced config) are skipped when read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<String>", into = "Vec<String>")]
pub struct Hud(Vec<HudItem>);

impl Default for Hud {
    fn default() -> Self {
        Hud(vec![HudItem::Timer])
    }
}

impl From<Vec<String>> for Hud {
    fn from(names: Vec<String>) -> Self {
        let mut items = Vec::new();
        for item in names.iter().filter_map(|n| HudItem::parse(n).ok()) {
            if !items.contains(&item) {
                items.push(item);
            }
        }
        Hud(items)
    }
}

impl From<Hud> for Vec<String> {
    fn from(hud: Hud) -> Self {
        hud.0.iter().map(|i| i.name().to_string()).collect()
    }
}

impl Hud {
    /// The shown items, left to right.
    pub fn items(&self) -> &[HudItem] {
        &self.0
    }

    pub fn shows(&self, item: HudItem) -> bool {
        self.0.contains(&item)
    }

    /// Show or hide `item`; a newly shown one goes on the right.
    pub fn set(&mut self, item: HudItem, on: bool) {
        self.0.retain(|i| *i != item);
        if on {
            self.0.push(item);
        }
    }

    pub fn toggle(&mut self, item: HudItem) {
        self.set(item, !self.shows(item));
    }

    /// Move a shown item `delta` places left (−) or right (+).
    pub fn shift(&mut self, item: HudItem, delta: isize) {
        let Some(i) = self.0.iter().position(|x| *x == item) else {
            return;
        };
        let j = (i as isize + delta).clamp(0, self.0.len() as isize - 1) as usize;
        let it = self.0.remove(i);
        self.0.insert(j, it);
    }

    /// Every item as the menu lists it: shown ones in order, then hidden.
    pub fn menu_order(&self) -> Vec<(HudItem, bool)> {
        self.0
            .iter()
            .map(|i| (*i, true))
            .chain(
                HudItem::ALL
                    .into_iter()
                    .filter(|i| !self.shows(*i))
                    .map(|i| (i, false)),
            )
            .collect()
    }

    /// `timer · wpm`, or `off` when nothing is shown.
    pub fn label(&self) -> String {
        if self.0.is_empty() {
            return "off".into();
        }
        self.0
            .iter()
            .map(|i| i.name())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// `timer,wpm`, `off`, or names split by commas or spaces.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if matches!(s, "off" | "none" | "\"\"" | "[]") {
            return Ok(Hud(Vec::new()));
        }
        let mut hud = Hud(Vec::new());
        for name in s
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|n| !n.is_empty())
        {
            hud.set(HudItem::parse(name)?, true);
        }
        Ok(hud)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct Wrap {
        hud: Hud,
    }

    #[test]
    fn toml_round_trip_skips_unknown_and_duplicates() {
        let w: Wrap = toml::from_str(r#"hud = ["wpm", "future", "timer", "wpm"]"#).unwrap();
        assert_eq!(w.hud.items(), [HudItem::Wpm, HudItem::Timer]);
        let text = toml::to_string(&w).unwrap();
        assert_eq!(text.trim(), r#"hud = ["wpm", "timer"]"#);
    }

    #[test]
    fn toggle_and_shift() {
        let mut h = Hud::default();
        h.toggle(HudItem::Wpm);
        assert_eq!(h.items(), [HudItem::Timer, HudItem::Wpm]);
        h.shift(HudItem::Wpm, -1);
        assert_eq!(h.items(), [HudItem::Wpm, HudItem::Timer]);
        h.shift(HudItem::Wpm, -5);
        assert_eq!(h.items(), [HudItem::Wpm, HudItem::Timer]);
        h.toggle(HudItem::Timer);
        assert_eq!(h.items(), [HudItem::Wpm]);
        assert_eq!(
            h.menu_order(),
            [(HudItem::Wpm, true), (HudItem::Timer, false)]
        );
    }

    #[test]
    fn parses_lists() {
        assert_eq!(Hud::parse("wpm,timer").unwrap().label(), "wpm · timer");
        assert_eq!(Hud::parse("time wpm").unwrap().label(), "timer · wpm");
        assert_eq!(Hud::parse("off").unwrap().label(), "off");
        assert!(Hud::parse("wpm, nope").is_err());
    }
}

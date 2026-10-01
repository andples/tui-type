//! State for the modules screen: one language's modules as a checklist.
//! Opened from `:install` to choose which to download (`Purpose::Download`)
//! and when switching to a language that has some installed, to choose
//! which to mix in (`Purpose::Pick`). UI-free; `ui::modules` draws it.

use crate::app::Screen;
use crate::ui::widgets::Selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Checked modules get downloaded, unchecked installed ones removed.
    Download,
    /// Checked modules are mixed into the language's tests.
    Pick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRow {
    pub name: String,
    pub display: String,
    pub words: usize,
    pub installed: bool,
    pub checked: bool,
}

#[derive(Debug, Clone)]
pub struct ModuleMenu {
    pub language: String,
    pub purpose: Purpose,
    pub rows: Vec<ModuleRow>,
    pub selection: Selection,
    /// Where enter or esc goes back to.
    pub back: Screen,
}

impl Default for ModuleMenu {
    fn default() -> Self {
        Self::new(String::new(), Purpose::Pick, Vec::new(), Screen::Typing)
    }
}

impl ModuleMenu {
    pub fn new(language: String, purpose: Purpose, rows: Vec<ModuleRow>, back: Screen) -> Self {
        let mut selection = Selection::wrapping(0);
        selection.set_len(rows.len());
        Self {
            language,
            purpose,
            rows,
            selection,
            back,
        }
    }

    /// Check or uncheck the highlighted module.
    pub fn toggle(&mut self) {
        if let Some(r) = self.rows.get_mut(self.selection.selected) {
            r.checked = !r.checked;
        }
    }

    /// Check every module, or uncheck them all when they all are.
    pub fn toggle_all(&mut self) {
        let all = self.rows.iter().all(|r| r.checked);
        for r in &mut self.rows {
            r.checked = !all;
        }
    }

    pub fn checked(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| r.checked)
            .map(|r| r.name.clone())
            .collect()
    }

    /// For a download: what to fetch and what to remove.
    pub fn changes(&self) -> (Vec<String>, Vec<String>) {
        let fetch = self
            .rows
            .iter()
            .filter(|r| r.checked && !r.installed)
            .map(|r| r.name.clone())
            .collect();
        let remove = self
            .rows
            .iter()
            .filter(|r| !r.checked && r.installed)
            .map(|r| r.name.clone())
            .collect();
        (fetch, remove)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, installed: bool) -> ModuleRow {
        ModuleRow {
            name: name.into(),
            display: name.into(),
            words: 10,
            installed,
            checked: installed,
        }
    }

    #[test]
    fn toggling_decides_downloads_and_removals() {
        let mut m = ModuleMenu::new(
            "code_python".into(),
            Purpose::Download,
            vec![
                row("numpy", true),
                row("pandas", false),
                row("stdlib", false),
            ],
            Screen::Catalog,
        );
        m.toggle(); // numpy off
        m.selection.move_by(1);
        m.toggle(); // pandas on
        assert_eq!(m.changes(), (vec!["pandas".into()], vec!["numpy".into()]));
        assert_eq!(m.checked(), ["pandas"]);
        m.toggle_all();
        assert_eq!(m.checked().len(), 3);
        m.toggle_all();
        assert!(m.checked().is_empty());
    }
}

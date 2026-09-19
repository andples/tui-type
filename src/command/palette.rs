//! Command-line editing state plus fuzzy suggestions. The palette suggests
//! command names until a space is typed, then argument values for the
//! recognised command.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use super::{ArgKind, CommandSpec, arg_candidates, find_spec};

const MAX_SUGGESTIONS: usize = 8;

/// Live data the palette needs to complete arguments.
#[derive(Debug, Clone, Default)]
pub struct Completions {
    pub themes: Vec<String>,
    pub languages: Vec<String>,
    pub fonts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Text shown in the popup (name or value).
    pub label: String,
    /// Right-hand description (usage + help for commands, empty for args).
    pub detail: String,
    /// Full command line after accepting this suggestion.
    pub completion: String,
}

/// What the palette is currently completing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Command,
    Arg(ArgKind),
}

pub struct CommandLine {
    pub input: String,
    /// Cursor position in chars.
    pub cursor: usize,
    pub selected: usize,
    pub suggestions: Vec<Suggestion>,
    /// Message from the last executed command, shown in place of the palette.
    pub message: Option<String>,
    stage: Stage,
    matcher: Matcher,
}

impl Default for CommandLine {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandLine {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            selected: 0,
            suggestions: Vec::new(),
            message: None,
            stage: Stage::Command,
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    pub fn open(&mut self, comps: &Completions) {
        self.input.clear();
        self.cursor = 0;
        self.message = None;
        self.refresh(comps);
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.suggestions.clear();
        self.selected = 0;
    }

    pub fn insert(&mut self, c: char, comps: &Completions) {
        let byte = self.byte_index(self.cursor);
        self.input.insert(byte, c);
        self.cursor += 1;
        self.refresh(comps);
    }

    pub fn backspace(&mut self, comps: &Completions) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let byte = self.byte_index(self.cursor - 1);
        self.input.remove(byte);
        self.cursor -= 1;
        self.refresh(comps);
        true
    }

    pub fn delete_word(&mut self, comps: &Completions) {
        let chars: Vec<char> = self.input.chars().collect();
        let mut i = self.cursor;
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        while i > 0 && chars[i - 1] != ' ' {
            i -= 1;
        }
        self.input = chars[..i].iter().chain(&chars[self.cursor..]).collect();
        self.cursor = i;
        self.refresh(comps);
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.input.chars().count());
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.input.chars().count();
    }

    pub fn select_next(&mut self) {
        if !self.suggestions.is_empty() {
            self.selected = (self.selected + 1) % self.suggestions.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.suggestions.is_empty() {
            self.selected = (self.selected + self.suggestions.len() - 1) % self.suggestions.len();
        }
    }

    pub fn selected(&self) -> Option<&Suggestion> {
        self.suggestions.get(self.selected)
    }

    /// Tab: replace the input with the selected suggestion's completion.
    /// Returns true if something changed.
    pub fn accept(&mut self, comps: &Completions) -> bool {
        let Some(s) = self.selected() else {
            return false;
        };
        if s.completion == self.input {
            return false;
        }
        self.input = s.completion.clone();
        self.cursor = self.input.chars().count();
        self.refresh(comps);
        true
    }

    /// Enter: the line to execute. If a command needing an argument is
    /// selected but no argument typed, completes instead and returns `None`.
    pub fn submit(&mut self, comps: &Completions) -> Option<String> {
        if let (Stage::Command, Some(s)) = (self.stage, self.selected()) {
            let spec = find_spec(s.label.as_str()).expect("suggestion is a spec");
            if spec.requires_arg {
                self.accept(comps);
                return None;
            }
            return Some(spec.name.to_string());
        }
        if self.input.trim().is_empty() {
            return None;
        }
        // Constrained arguments take the highlighted candidate (`theme gb` →
        // gruvbox). Free arguments run exactly as typed — presets are only
        // suggestions, and `time 3` must not become `time 30`; Tab completes.
        if let (Stage::Arg(kind), Some(s)) = (self.stage, self.selected())
            && !kind.accepts_free_text()
        {
            return Some(s.completion.clone());
        }
        Some(self.input.trim().to_string())
    }

    /// The theme name the user is hovering in a `theme ...` completion, so the
    /// UI can preview it live.
    pub fn preview_theme(&self) -> Option<&str> {
        match self.stage {
            Stage::Arg(ArgKind::Themes) => self.selected().map(|s| s.label.as_str()),
            _ => None,
        }
    }

    fn byte_index(&self, char_idx: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.input.len())
    }

    fn refresh(&mut self, comps: &Completions) {
        self.selected = 0;
        let input = self.input.trim_start().to_string();
        let (head, rest) = match input.split_once(char::is_whitespace) {
            Some((h, r)) => (h, Some(r.trim_start())),
            None => (input.as_str(), None),
        };

        match rest {
            None => {
                self.stage = Stage::Command;
                self.suggestions = self.rank(head, super::COMMANDS.iter().map(command_item));
            }
            Some(arg) => {
                let Some(spec) = find_spec(head) else {
                    self.stage = Stage::Command;
                    self.suggestions.clear();
                    return;
                };
                self.stage = Stage::Arg(spec.arg);
                let name = spec.name;
                let items = arg_candidates(spec.arg, comps)
                    .into_iter()
                    .map(|v| Suggestion {
                        completion: format!("{name} {v}"),
                        label: v,
                        detail: String::new(),
                    });
                self.suggestions = self.rank(arg, items);
            }
        }
    }

    fn rank(&mut self, needle: &str, items: impl Iterator<Item = Suggestion>) -> Vec<Suggestion> {
        let pattern = Pattern::parse(needle, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(u32, Suggestion)> = items
            .filter_map(|s| {
                let score = pattern.score(Utf32Str::new(&s.label, &mut buf), &mut self.matcher)?;
                Some((score, s))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));
        scored
            .into_iter()
            .map(|(_, s)| s)
            .take(MAX_SUGGESTIONS)
            .collect()
    }
}

fn command_item(spec: &CommandSpec) -> Suggestion {
    let detail = if spec.usage.is_empty() {
        spec.help.to_string()
    } else {
        format!("{}  {}", spec.usage, spec.help)
    };
    Suggestion {
        label: spec.name.to_string(),
        detail,
        completion: if spec.arg == ArgKind::None {
            spec.name.to_string()
        } else {
            format!("{} ", spec.name)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comps() -> Completions {
        Completions {
            themes: vec!["default".into(), "gruvbox".into(), "nord".into()],
            languages: vec!["english".into(), "english_1k".into()],
            fonts: vec![],
        }
    }

    fn type_in(cl: &mut CommandLine, s: &str) {
        for c in s.chars() {
            cl.insert(c, &comps());
        }
    }

    #[test]
    fn empty_lists_all_commands() {
        let mut cl = CommandLine::new();
        cl.open(&comps());
        assert_eq!(cl.suggestions.len(), MAX_SUGGESTIONS);
        assert_eq!(cl.stage, Stage::Command);
    }

    #[test]
    fn fuzzy_command_then_arg_completion() {
        let mut cl = CommandLine::new();
        cl.open(&comps());
        type_in(&mut cl, "thm");
        assert_eq!(cl.selected().unwrap().label, "theme");
        assert!(cl.accept(&comps()));
        assert_eq!(cl.input, "theme ");
        assert_eq!(cl.stage, Stage::Arg(ArgKind::Themes));
        assert_eq!(cl.suggestions.len(), 3);
        type_in(&mut cl, "gb");
        assert_eq!(cl.selected().unwrap().label, "gruvbox");
        assert_eq!(cl.preview_theme(), Some("gruvbox"));
        assert_eq!(cl.submit(&comps()), Some("theme gruvbox".into()));
    }

    #[test]
    fn enter_on_arg_command_completes_instead_of_running() {
        let mut cl = CommandLine::new();
        cl.open(&comps());
        type_in(&mut cl, "time");
        assert_eq!(cl.submit(&comps()), None);
        assert_eq!(cl.input, "time ");
        type_in(&mut cl, "6");
        assert_eq!(cl.submit(&comps()), Some("time 6".into()));
        assert!(cl.accept(&comps()));
        assert_eq!(cl.submit(&comps()), Some("time 60".into()));
    }

    #[test]
    fn typed_arg_wins_over_fuzzy_match() {
        let mut cl = CommandLine::new();
        cl.open(&comps());
        type_in(&mut cl, "t 3");
        assert_eq!(cl.selected().unwrap().label, "30");
        assert_eq!(cl.submit(&comps()), Some("t 3".into()));
        let mut cl = CommandLine::new();
        cl.open(&comps());
        type_in(&mut cl, "theme GRUV");
        assert_eq!(cl.submit(&comps()), Some("theme gruvbox".into()));
    }

    #[test]
    fn enter_on_plain_command_runs() {
        let mut cl = CommandLine::new();
        cl.open(&comps());
        type_in(&mut cl, "q");
        assert_eq!(cl.submit(&comps()), Some("quit".into()));
    }

    #[test]
    fn free_text_submits_verbatim() {
        let mut cl = CommandLine::new();
        cl.open(&comps());
        type_in(&mut cl, "set results.chart off");
        assert!(cl.suggestions.is_empty());
        assert_eq!(cl.submit(&comps()), Some("set results.chart off".into()));
    }

    #[test]
    fn editing_keys() {
        let mut cl = CommandLine::new();
        cl.open(&comps());
        type_in(&mut cl, "theme nord");
        cl.delete_word(&comps());
        assert_eq!(cl.input, "theme ");
        cl.move_home();
        cl.insert('x', &comps());
        assert_eq!(cl.input, "xtheme ");
        assert!(cl.backspace(&comps()));
        assert_eq!(cl.input, "theme ");
        cl.move_end();
        assert_eq!(cl.cursor, 6);
    }
}

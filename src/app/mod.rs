//! Application state and event loop.

pub mod action;
pub mod input;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event};
use ratatui::DefaultTerminal;

use crate::command::{self, Command, CommandLine, Completions};
use crate::config::{Config, Paths};
use crate::language::LanguageRegistry;
use crate::stats::{LocalJsonlStore, StatsStore, Summary, TestRecord, personal_best};
use crate::test::{Metrics, Mode, Modifiers, RandomGenerator, Status, TestEngine};
use crate::theme::{Theme, ThemeRegistry};
use crate::ui;
use action::Action;
use input::InputContext;

/// How long a transient notice stays on the bottom line.
const NOTICE_TTL: Duration = Duration::from_millis(2500);
/// Redraw cadence while a timed test is running.
const TIMER_TICK: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Typing,
    Results,
    Stats,
    Help,
}

/// Everything the results screen needs about the last finished test.
pub struct Outcome {
    pub metrics: Metrics,
    pub record: TestRecord,
    pub is_pb: bool,
}

pub struct App {
    pub config: Config,
    pub paths: Paths,
    pub themes: ThemeRegistry,
    pub languages: LanguageRegistry,
    pub engine: TestEngine,
    pub outcome: Option<Outcome>,
    pub stats: Box<dyn StatsStore>,
    pub summary: Summary,
    pub screen: Screen,
    pub cmdline: CommandLine,
    pub cmd_open: bool,
    pub completions: Completions,
    pub notice: Option<(String, Instant)>,
    pub scroll: usize,
    pub should_quit: bool,
    /// Screen to return to from stats/help.
    previous_screen: Screen,
    dirty: bool,
}

impl App {
    pub fn new(paths: Paths, theme_override: Option<String>) -> Result<Self> {
        let mut warnings = Vec::new();
        let mut config = Config::load(&paths.config_file)
            .with_context(|| format!("loading {}", paths.config_file.display()))?;
        if let Some(t) = theme_override {
            config.theme = t;
        }
        let themes = ThemeRegistry::load(&paths.themes_dir, |w| warnings.push(w));
        let languages = LanguageRegistry::load(&paths.languages_dir, |w| warnings.push(w));
        let (store, skipped) = LocalJsonlStore::open(&paths.history_file)
            .with_context(|| format!("loading {}", paths.history_file.display()))?;
        if skipped > 0 {
            warnings.push(format!("skipped {skipped} corrupt history line(s)"));
        }
        if themes.get(&config.theme).is_none() {
            warnings.push(format!("unknown theme `{}`, using default", config.theme));
        }
        if languages.get(&config.language).is_none() {
            warnings.push(format!(
                "unknown language `{}`, using english",
                config.language
            ));
        }

        let completions = Completions {
            themes: themes.names().map(str::to_string).collect(),
            languages: languages.names().map(str::to_string).collect(),
        };
        let engine = Self::build_engine(&config, &languages);
        let summary = Summary::from_records(store.all());
        let mut app = Self {
            config,
            paths,
            themes,
            languages,
            engine,
            outcome: None,
            stats: Box::new(store),
            summary,
            screen: Screen::Typing,
            cmdline: CommandLine::new(),
            cmd_open: false,
            completions,
            notice: None,
            scroll: 0,
            should_quit: false,
            previous_screen: Screen::Typing,
            dirty: true,
        };
        if let Some(w) = warnings.first() {
            app.notify(w.clone());
        }
        Ok(app)
    }

    fn build_engine(config: &Config, languages: &LanguageRegistry) -> TestEngine {
        let lang = languages.get_or_default(&config.language);
        let generator = RandomGenerator::new(
            lang.words.clone(),
            Modifiers {
                punctuation: config.punctuation,
                numbers: config.numbers,
            },
        );
        TestEngine::new(config.mode, Box::new(generator))
    }

    /// The theme to render with: a live palette preview wins over config.
    pub fn theme(&self) -> &Theme {
        if self.cmd_open
            && let Some(name) = self.cmdline.preview_theme()
            && let Some(t) = self.themes.get(name)
        {
            return t;
        }
        self.themes.get_or_default(&self.config.theme)
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), Instant::now()));
        self.dirty = true;
    }

    /// Current notice text, if it hasn't expired.
    pub fn notice_text(&self) -> Option<&str> {
        self.notice
            .as_ref()
            .filter(|(_, at)| at.elapsed() < NOTICE_TTL)
            .map(|(m, _)| m.as_str())
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            if self.dirty {
                terminal.draw(|f| ui::render(f, self))?;
                self.dirty = false;
            }
            let timeout = self.poll_timeout();
            let ready = match timeout {
                Some(t) => event::poll(t)?,
                None => true,
            };
            if ready {
                let action = match event::read()? {
                    Event::Key(k) => input::map_key(k, self.input_context()),
                    Event::Resize(_, _) => Action::Redraw,
                    _ => Action::Nop,
                };
                self.dispatch(action);
            } else {
                self.dispatch(Action::Tick);
            }
        }
        Ok(())
    }

    /// Only wake up on a timer when something on screen is time-dependent.
    fn poll_timeout(&self) -> Option<Duration> {
        let timed_running = self.engine.status() == Status::Running
            && matches!(self.engine.mode(), Mode::Time(_))
            && self.screen == Screen::Typing;
        if timed_running {
            return Some(TIMER_TICK);
        }
        self.notice.as_ref().map(|(_, at)| {
            NOTICE_TTL
                .saturating_sub(at.elapsed())
                .max(Duration::from_millis(50))
        })
    }

    fn input_context(&self) -> InputContext {
        InputContext {
            screen: self.screen,
            command_line_open: self.cmd_open,
            test_status: self.engine.status(),
        }
    }

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::Nop => return,
            Action::Tick => {
                let now = Instant::now();
                if self.engine.tick(now) {
                    self.finish_test();
                }
                if let Some((_, at)) = self.notice
                    && at.elapsed() >= NOTICE_TTL
                {
                    self.notice = None;
                }
            }
            Action::Redraw => {}
            Action::Quit => self.should_quit = true,

            Action::TypeChar(c) => {
                let was_finished = self.engine.is_finished();
                self.engine.type_char(c);
                if self.engine.is_finished() && !was_finished {
                    self.finish_test();
                }
            }
            Action::Backspace => self.engine.backspace(),
            Action::DeleteWord => self.engine.delete_word(),
            Action::Restart => self.restart(),

            Action::OpenCommandLine => {
                self.cmd_open = true;
                self.cmdline.open(&self.completions);
            }
            Action::CloseCommandLine => self.close_command_line(),
            Action::CmdInsert(c) => self.cmdline.insert(c, &self.completions),
            Action::CmdBackspace => {
                if !self.cmdline.backspace(&self.completions) {
                    self.close_command_line();
                }
            }
            Action::CmdDeleteWord => self.cmdline.delete_word(&self.completions),
            Action::CmdLeft => self.cmdline.move_left(),
            Action::CmdRight => self.cmdline.move_right(),
            Action::CmdHome => self.cmdline.move_home(),
            Action::CmdEnd => self.cmdline.move_end(),
            Action::CmdSelectNext => self.cmdline.select_next(),
            Action::CmdSelectPrev => self.cmdline.select_prev(),
            Action::CmdAccept => {
                self.cmdline.accept(&self.completions);
            }
            Action::CmdSubmit => {
                if let Some(line) = self.cmdline.submit(&self.completions) {
                    self.close_command_line();
                    self.execute_line(&line);
                }
            }

            Action::Back => {
                self.screen = self.previous_screen;
                self.scroll = 0;
            }
            Action::ShowStats => {
                self.summary = Summary::from_records(self.stats.all());
                self.push_screen(Screen::Stats);
            }
            Action::ShowHelp => self.push_screen(Screen::Help),
            Action::ScrollDown => self.scroll += 1,
            Action::ScrollUp => self.scroll = self.scroll.saturating_sub(1),
        }
        self.dirty = true;
    }

    /// Open an overlay screen, remembering where to come back to.
    fn push_screen(&mut self, screen: Screen) {
        if matches!(self.screen, Screen::Typing | Screen::Results) {
            self.previous_screen = self.screen;
        }
        self.screen = screen;
        self.scroll = 0;
    }

    fn close_command_line(&mut self) {
        self.cmd_open = false;
        self.cmdline.clear();
    }

    fn restart(&mut self) {
        self.engine = Self::build_engine(&self.config, &self.languages);
        self.outcome = None;
        self.screen = Screen::Typing;
        self.previous_screen = Screen::Typing;
        self.scroll = 0;
    }

    fn finish_test(&mut self) {
        let metrics = Metrics::from_engine(&self.engine);
        let record = TestRecord::new(
            &metrics,
            self.engine.mode(),
            &self.config.language,
            self.config.punctuation,
            self.config.numbers,
        );
        let prev_best = personal_best(self.stats.all(), record.mode, &record.language);
        let is_pb = prev_best.is_none_or(|b| record.wpm > b) && record.wpm > 0.0;
        if let Err(e) = self.stats.append(&record) {
            self.notify(format!("could not save result: {e}"));
        }
        self.outcome = Some(Outcome {
            metrics,
            record,
            is_pb,
        });
        self.screen = Screen::Results;
    }

    fn execute_line(&mut self, line: &str) {
        match command::parse(line) {
            Ok(cmd) => self.execute(cmd),
            Err(msg) if msg.is_empty() => {}
            Err(msg) => self.notify(msg),
        }
    }

    pub fn execute(&mut self, cmd: Command) {
        let mut changed_test = false;
        match cmd {
            Command::Time(s) => {
                self.config.mode = Mode::Time(s);
                changed_test = true;
            }
            Command::Words(n) => {
                self.config.mode = Mode::Words(n);
                changed_test = true;
            }
            Command::Language(name) => {
                if self.languages.get(&name).is_none() {
                    self.notify(format!("unknown language `{name}`"));
                    return;
                }
                self.config.language = name;
                changed_test = true;
            }
            Command::Theme(name) => {
                if self.themes.get(&name).is_none() {
                    self.notify(format!("unknown theme `{name}`"));
                    return;
                }
                self.config.theme = name;
            }
            Command::Punctuation(v) => {
                self.config.punctuation = v.unwrap_or(!self.config.punctuation);
                self.notify(format!("punctuation {}", on_off(self.config.punctuation)));
                changed_test = true;
            }
            Command::Numbers(v) => {
                self.config.numbers = v.unwrap_or(!self.config.numbers);
                self.notify(format!("numbers {}", on_off(self.config.numbers)));
                changed_test = true;
            }
            Command::Restart => self.restart(),
            Command::Stats => self.dispatch(Action::ShowStats),
            Command::Help => self.dispatch(Action::ShowHelp),
            Command::Quit => self.should_quit = true,
            Command::Results { section, value } => {
                let current = self.config.results.get(&section).unwrap_or(true);
                let new = value.unwrap_or(!current);
                self.config.results.set(&section, new);
                self.notify(format!("results {section} {}", on_off(new)));
            }
            Command::Set { key, value } => {
                if let Err(e) = self.config.set(&key, &value) {
                    self.notify(e);
                    return;
                }
                if self.themes.get(&self.config.theme).is_none() {
                    self.notify(format!("unknown theme `{}`", self.config.theme));
                }
                changed_test = !matches!(key.as_str(), "theme") && !key.starts_with("results.");
            }
        }
        if changed_test {
            self.restart();
        }
        self.save_config();
    }

    fn save_config(&mut self) {
        if let Err(e) = self.config.save(&self.paths.config_file) {
            self.notify(format!("could not save config: {e}"));
        }
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

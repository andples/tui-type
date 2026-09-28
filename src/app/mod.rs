//! Application state and event loop.

pub mod action;
pub mod input;
mod online;
mod profiles;

use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event};
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;

use crate::command::{self, Command, CommandLine, Completions};
use crate::config::{Config, Paths};
use crate::config::{FONT_SIZE_RANGE, FontSize, LINES_RANGE, WORDS_PER_LINE_RANGE};
use crate::gfx::{self, Gfx};
use crate::language::LanguageRegistry;
use crate::online::Online;
use crate::profile::{ProfileMenu, ProfileRegistry};
use crate::stats::{LocalJsonlStore, StatsStore, Summary, TestRecord, personal_best};
use crate::test::{Metrics, Mode, Modifiers, RandomGenerator, Status, TestEngine};
use crate::theme::{Theme, ThemeRegistry};
use crate::ui;
use crate::ui::style::content_column;
use action::Action;
use input::InputContext;
use ttyp_core::api::{Daily, SubmitResponse};

/// How long a transient notice stays on the bottom line.
const NOTICE_TTL: Duration = Duration::from_millis(2500);
/// Redraw cadence while a timed test is running.
const TIMER_TICK: Duration = Duration::from_millis(100);
/// How often the event loop checks for network replies while one is due.
const REMOTE_POLL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Typing,
    Results,
    Stats,
    Help,
    Profiles,
    Login,
}

/// Which config value a slider edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderTarget {
    FontSize,
    WordsPerLine,
    Lines,
}

impl SliderTarget {
    pub fn label(self) -> &'static str {
        match self {
            SliderTarget::FontSize => "font size",
            SliderTarget::WordsPerLine => "words per line",
            SliderTarget::Lines => "lines",
        }
    }

    pub fn range(self) -> (u8, u8) {
        match self {
            SliderTarget::FontSize => FONT_SIZE_RANGE,
            SliderTarget::WordsPerLine => WORDS_PER_LINE_RANGE,
            SliderTarget::Lines => LINES_RANGE,
        }
    }
}

/// An open bottom-line slider. Changes apply live; Esc restores `original`.
#[derive(Debug, Clone, Copy)]
pub struct Slider {
    pub target: SliderTarget,
    pub value: u8,
    pub original: u8,
}

impl Slider {
    pub fn min(&self) -> u8 {
        self.target.range().0
    }
    pub fn max(&self) -> u8 {
        self.target.range().1
    }
}

/// Everything the results screen needs about the last finished test.
pub struct Outcome {
    pub metrics: Metrics,
    pub record: TestRecord,
    pub is_pb: bool,
    /// Set when the run was a daily: how the submission is going.
    pub daily: Option<DailyOutcome>,
}

pub struct DailyOutcome {
    pub daily_id: i64,
    pub status: DailyStatus,
}

pub enum DailyStatus {
    Submitting,
    Ranked(SubmitResponse),
    /// Saved in the data dir, retried on the next start.
    Queued,
    Failed(String),
}

pub struct App {
    pub config: Config,
    pub paths: Paths,
    pub themes: ThemeRegistry,
    pub languages: LanguageRegistry,
    pub profiles: ProfileRegistry,
    pub profile_menu: ProfileMenu,
    pub engine: TestEngine,
    pub outcome: Option<Outcome>,
    pub stats: Box<dyn StatsStore>,
    pub summary: Summary,
    pub screen: Screen,
    pub cmdline: CommandLine,
    pub cmd_open: bool,
    pub slider: Option<Slider>,
    pub completions: Completions,
    pub notice: Option<(String, Instant)>,
    pub scroll: usize,
    pub should_quit: bool,
    pub gfx: Gfx,
    /// Present only when `server` is configured.
    pub online: Option<Online>,
    /// The GitHub device code and URL to show while `:login` waits.
    pub login_prompt: Option<(String, String)>,
    /// The daily being typed, if the current test is one.
    pub daily: Option<Daily>,
    /// `:daily` waiting for today's list: (language, mode).
    pending_daily: Option<(String, Mode)>,
    /// Screen to return to from stats/help.
    previous_screen: Screen,
    /// A notice was raised while handling the current action.
    fresh_notice: bool,
    dirty: bool,
}

impl App {
    pub fn new(paths: Paths, theme_override: Option<String>) -> Result<Self> {
        let mut warnings = Vec::new();
        let mut config = Config::load(&paths.config_file)
            .with_context(|| format!("loading {}", paths.config_file.display()))?;
        let themes = ThemeRegistry::load(&paths.themes_dir, |w| warnings.push(w));
        let languages = LanguageRegistry::load(&paths.languages_dir, |w| warnings.push(w));
        let profiles = ProfileRegistry::load(&paths.profiles_dir, |w| warnings.push(w));
        // Profiles deleted or edited outside ttyp no longer describe the
        // config; deselect them before anything else reads the list.
        let dropped = profiles.reconcile(&mut config);
        if !dropped.is_empty() {
            warnings.push(format!(
                "profile {} no longer matches the config, deselected",
                dropped.join(", ")
            ));
            if let Err(e) = config.save(&paths.config_file) {
                warnings.push(format!("could not save config: {e}"));
            }
        }
        if let Some(t) = theme_override {
            config.theme = t;
        }
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

        let mut gfx = Gfx::disabled();
        if let Err(e) = gfx.configure(&config, &paths.fonts_dir) {
            warnings.push(e);
        }
        let completions = Completions {
            themes: themes.names().map(str::to_string).collect(),
            languages: languages.names().map(str::to_string).collect(),
            fonts: gfx::fonts::list(&paths.fonts_dir),
            profiles: profiles.names().map(str::to_string).collect(),
        };
        let engine = Self::build_engine(&config, &languages);
        let summary = Summary::from_records(store.all());
        let online = Self::connect(&config, &paths);
        let mut app = Self {
            config,
            paths,
            themes,
            languages,
            profiles,
            profile_menu: ProfileMenu::default(),
            engine,
            outcome: None,
            stats: Box::new(store),
            summary,
            screen: Screen::Typing,
            cmdline: CommandLine::new(),
            cmd_open: false,
            slider: None,
            completions,
            notice: None,
            scroll: 0,
            should_quit: false,
            gfx,
            online,
            login_prompt: None,
            daily: None,
            pending_daily: None,
            previous_screen: Screen::Typing,
            fresh_notice: false,
            dirty: true,
        };
        if let Some(w) = warnings.first() {
            app.notify(w.clone());
        }
        app.retry_queued_submissions();
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
        if let (Screen::Profiles, ProfileMenu::Edit(e)) = (self.screen, &self.profile_menu)
            && let Some(t) = e.preview_theme().and_then(|n| self.themes.get(n))
        {
            return t;
        }
        if self.cmd_open
            && let Some(name) = self.cmdline.preview_theme()
            && let Some(t) = self.themes.get(name)
        {
            return t;
        }
        self.themes.get_or_default(&self.config.theme)
    }

    /// Max width of the content column for the screen being shown.
    pub fn screen_width(&self) -> u16 {
        match self.screen {
            // Fullscreen spans the terminal; the notice line uses the
            // regular gutter.
            Screen::Typing if self.config.fullscreen => u16::MAX,
            Screen::Typing => self.typing_width(),
            _ => self.config.content_width(),
        }
    }

    /// Width of the word box in columns; with a real font it follows the
    /// font's average character width.
    pub fn typing_width(&self) -> u16 {
        let cols = self.config.line_chars() as f32 * self.glyph_cols(self.config.font_size());
        cols.ceil() as u16
    }

    /// Average terminal columns one character of the typing text takes.
    fn glyph_cols(&self, font: FontSize) -> f32 {
        match self.gfx.metrics(font) {
            Some(m) => self.gfx.mean_advance(m.geom.px) / m.cell_w as f32,
            None => font.cell_dims().0 as f32,
        }
    }

    /// Font size, word-box column and line count for the typing screen in
    /// `area` (the screen minus the bottom line). Fullscreen leaves one
    /// column of margin each side, picks the largest size at which
    /// `words_per_line` × `lines` words still fit, and fills the height
    /// with as many lines as fit at that size.
    pub fn typing_frame(&self, area: Rect) -> (FontSize, Rect, u8) {
        if !self.config.fullscreen {
            let font = self.config.font_size();
            let col = content_column(area, self.typing_width());
            return (font, col, self.config.lines);
        }
        let col = Rect::new(
            area.x + 1,
            area.y,
            area.width.saturating_sub(2).max(1),
            area.height,
        );
        let chars = self.config.line_chars() * self.config.lines as u16;
        let (font, lines) = FontSize::fit(col.width, col.height, chars, |f| self.glyph_cols(f));
        (font, col, lines)
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), Instant::now()));
        self.fresh_notice = true;
        self.dirty = true;
    }

    /// Add to a notice raised by the same action rather than replacing it.
    fn notify_more(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        match self.notice.as_ref().filter(|_| self.fresh_notice) {
            Some((current, _)) => self.notify(format!("{current} · {msg}")),
            None => self.notify(msg),
        }
    }

    /// Current notice text, if it hasn't expired.
    pub fn notice_text(&self) -> Option<&str> {
        self.notice
            .as_ref()
            .filter(|(_, at)| at.elapsed() < NOTICE_TTL)
            .map(|(m, _)| m.as_str())
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let result = self.event_loop(terminal);
        self.gfx.clear(terminal.backend_mut())?;
        result
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            self.drain_remote();
            if self.dirty {
                self.draw(terminal)?;
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
                    Event::Resize(_, _) => {
                        // The terminal clears on resize, taking images with it.
                        self.gfx.refresh_cell_size();
                        self.gfx.invalidate();
                        Action::Redraw
                    }
                    _ => Action::Nop,
                };
                self.dispatch(action);
            } else {
                self.dispatch(Action::Tick);
            }
        }
        Ok(())
    }

    /// Text first, then images over it, painted as one synchronized update.
    fn draw(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let sync = self.gfx.supported();
        if sync {
            terminal.backend_mut().write_all(gfx::kitty::SYNC_BEGIN)?;
        }
        let mut images = Vec::new();
        terminal.draw(|f| images = ui::render(f, self))?;
        self.gfx.present(&images, terminal.backend_mut())?;
        if sync {
            let out = terminal.backend_mut();
            out.write_all(gfx::kitty::SYNC_END)?;
            out.flush()?;
        }
        Ok(())
    }

    /// Only wake up on a timer when something on screen is time-dependent
    /// or a network reply is expected.
    fn poll_timeout(&self) -> Option<Duration> {
        let timed_running = self.engine.status() == Status::Running
            && matches!(self.engine.mode(), Mode::Time(_))
            && self.screen == Screen::Typing;
        if timed_running {
            return Some(TIMER_TICK);
        }
        if self.online.as_ref().is_some_and(Online::busy) {
            return Some(REMOTE_POLL);
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
            slider_open: self.slider.is_some(),
            test_status: self.engine.status(),
            profile_menu: self.profile_input(),
        }
    }

    pub fn dispatch(&mut self, action: Action) {
        self.fresh_notice = false;
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
            Action::FontBigger => self.execute(Command::FontSize(Some(
                self.config.font_size.saturating_add(1),
            ))),
            Action::FontSmaller => self.execute(Command::FontSize(Some(
                self.config.font_size.saturating_sub(1),
            ))),

            Action::SliderDec => self.slide(|s| s.value.saturating_sub(1).max(s.min())),
            Action::SliderInc => self.slide(|s| s.value.saturating_add(1).min(s.max())),
            Action::SliderMin => self.slide(|s| s.min()),
            Action::SliderMax => self.slide(|s| s.max()),
            Action::SliderConfirm => {
                if let Some(s) = self.slider.take() {
                    self.apply_slider(s.target, s.value);
                    self.notify(format!("{} {}", s.target.label(), s.value));
                    self.save_config();
                }
            }
            Action::SliderCancel => {
                if let Some(s) = self.slider.take() {
                    self.apply_slider(s.target, s.original);
                }
            }
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
            Action::ShowProfiles => self.open_profiles(),
            Action::Profile(a) => self.profile_action(a),
            Action::Login => self.login(),
            Action::CancelLogin => self.cancel_login(),
            Action::Logout => self.logout(),
            Action::Remote(ev) => self.remote_event(ev),
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

    fn open_slider(&mut self, target: SliderTarget) {
        let current = match target {
            SliderTarget::FontSize => self.config.font_size,
            SliderTarget::WordsPerLine => self.config.words_per_line,
            SliderTarget::Lines => self.config.lines,
        };
        self.slider = Some(Slider {
            target,
            value: current,
            original: current,
        });
    }

    /// Update the open slider's value and apply it live.
    fn slide(&mut self, next: impl FnOnce(&Slider) -> u8) {
        if let Some(mut s) = self.slider {
            s.value = next(&s);
            self.slider = Some(s);
            self.apply_slider(s.target, s.value);
        }
    }

    fn apply_slider(&mut self, target: SliderTarget, value: u8) {
        match target {
            SliderTarget::FontSize => self.config.set_font_size(value),
            SliderTarget::WordsPerLine => self.config.set_words_per_line(value),
            SliderTarget::Lines => self.config.set_lines(value),
        }
    }

    fn close_command_line(&mut self) {
        self.cmd_open = false;
        self.cmdline.clear();
    }

    /// A fresh random test; leaves any daily in progress.
    fn restart(&mut self) {
        self.daily = None;
        self.engine = Self::build_engine(&self.config, &self.languages);
        self.outcome = None;
        self.screen = Screen::Typing;
        self.previous_screen = Screen::Typing;
        self.scroll = 0;
    }

    /// Start a fresh test after a test setting changed. From the profile
    /// screen the new test waits underneath instead of taking over.
    fn rebuild_test(&mut self) {
        if self.screen == Screen::Profiles {
            self.daily = None;
            self.engine = Self::build_engine(&self.config, &self.languages);
            self.outcome = None;
            self.previous_screen = Screen::Typing;
        } else {
            self.restart();
        }
    }

    fn finish_test(&mut self) {
        let metrics = Metrics::from_engine(&self.engine);
        let daily = self.daily.take();
        let (language, punctuation, numbers) = match &daily {
            Some(d) => (d.language.clone(), false, false),
            None => (
                self.config.language.clone(),
                self.config.punctuation,
                self.config.numbers,
            ),
        };
        let mut record = TestRecord::new(
            &metrics,
            self.engine.mode(),
            &language,
            punctuation,
            numbers,
        );
        record.daily_id = daily.as_ref().map(|d| d.id);
        let prev_best = personal_best(self.stats.all(), record.mode, &record.language);
        let is_pb = prev_best.is_none_or(|b| record.wpm > b) && record.wpm > 0.0;
        if let Err(e) = self.stats.append(&record) {
            self.notify(format!("could not save result: {e}"));
        }
        let daily = daily.map(|d| self.submit_daily(&d));
        self.outcome = Some(Outcome {
            metrics,
            record,
            is_pb,
            daily,
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
            Command::FontSize(None) => {
                self.open_slider(SliderTarget::FontSize);
                return;
            }
            Command::FontSize(Some(n)) => {
                self.config.set_font_size(n);
                if self.config.fullscreen {
                    self.notify(format!(
                        "font size {} · fullscreen picks the size while on",
                        self.config.font_size
                    ));
                } else {
                    self.notify(format!("font size {}", self.config.font_size));
                }
            }
            Command::WordsPerLine(None) => {
                self.open_slider(SliderTarget::WordsPerLine);
                return;
            }
            Command::WordsPerLine(Some(n)) => {
                self.config.set_words_per_line(n);
                self.notify(format!("words per line {}", self.config.words_per_line));
            }
            Command::Font(spec) => {
                let old = std::mem::replace(&mut self.config.font, spec);
                if let Err(e) = self.apply_graphics() {
                    self.config.font = old;
                    let _ = self.apply_graphics();
                    self.notify(e);
                    return;
                }
                if !self.gfx.supported() {
                    self.notify(
                        "font saved; this terminal has no kitty graphics, so it shows block glyphs",
                    );
                } else {
                    let name = if self.config.font.is_empty() {
                        "system monospace"
                    } else {
                        &self.config.font
                    };
                    self.notify(format!("font {name}"));
                }
            }
            Command::Lines(None) => {
                self.open_slider(SliderTarget::Lines);
                return;
            }
            Command::Lines(Some(n)) => {
                self.config.set_lines(n);
                self.notify(format!("lines {}", self.config.lines));
            }
            Command::Fullscreen(v) => {
                self.config.fullscreen = v.unwrap_or(!self.config.fullscreen);
                self.notify(format!("fullscreen {}", on_off(self.config.fullscreen)));
            }
            Command::Zen(v) => {
                self.config.zen = v.unwrap_or(!self.config.zen);
                self.notify(format!("zen {}", on_off(self.config.zen)));
            }
            Command::Profile(None) => {
                self.open_profiles();
                return;
            }
            Command::Profile(Some(name)) => {
                self.activate_profile(&name);
                return;
            }
            Command::Restart => self.restart(),
            Command::Stats => self.dispatch(Action::ShowStats),
            Command::Help => self.dispatch(Action::ShowHelp),
            Command::Daily(mode) => {
                self.open_daily(mode);
                return;
            }
            Command::Login => self.dispatch(Action::Login),
            Command::Logout => self.dispatch(Action::Logout),
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
                if matches!(key.as_str(), "font" | "graphics")
                    && let Err(e) = self.apply_graphics()
                {
                    self.notify(e);
                }
                changed_test = !matches!(
                    key.as_str(),
                    "theme"
                        | "zen"
                        | "font_size"
                        | "fontsize"
                        | "words_per_line"
                        | "wpl"
                        | "lines"
                        | "fullscreen"
                        | "full"
                        | "font"
                        | "graphics"
                ) && !key.starts_with("results.");
            }
        }
        if changed_test {
            self.rebuild_test();
        }
        self.save_config();
    }

    fn apply_graphics(&mut self) -> Result<(), String> {
        self.gfx.configure(&self.config, &self.paths.fonts_dir)
    }

    /// Persist the config. Active profiles whose settings were just changed
    /// by hand are deselected first, so the list never claims a profile
    /// that isn't in effect.
    fn save_config(&mut self) {
        let dropped = self.profiles.reconcile(&mut self.config);
        if !dropped.is_empty() {
            self.notify_more(format!("profile {} off", dropped.join(", ")));
        }
        if let Err(e) = self.config.save(&self.paths.config_file) {
            self.notify(format!("could not save config: {e}"));
        }
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

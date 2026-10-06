//! The `:` command language: a static registry of command specs (for the
//! palette) and a parser from a typed line to a `Command`.

pub mod palette;

pub use palette::{CommandLine, Completions, Suggestion};

use crate::app::misses::MissedRange;
use crate::config::{
    FONT_SIZE_RANGE, Keyboard, LINES_RANGE, Pace, PbEffect, ResultsConfig, WORDS_PER_LINE_RANGE,
    parse_range,
};
use crate::test::mode::Mode;

/// Parsed, validated command ready for the app to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Time(u16),
    Words(u16),
    Language(String),
    Theme(String),
    /// `None` toggles.
    Punctuation(Option<bool>),
    Numbers(Option<bool>),
    /// `None` opens the slider.
    FontSize(Option<u8>),
    /// `None` opens the slider.
    WordsPerLine(Option<u8>),
    /// `None` opens the slider.
    Lines(Option<u8>),
    Fullscreen(Option<bool>),
    /// Font family, file name or path; empty for the system monospace.
    Font(String),
    Zen(Option<bool>),
    /// The pace caret: off, pb, last or a wpm.
    Pace(Pace),
    /// The layout of the results screen's missed-keys keyboard.
    Keyboard(Keyboard),
    /// What a new personal best shows.
    PbEffect(PbEffect),
    /// The current language's modules checklist.
    Modules,
    /// Strip each language's boilerplate; `None` toggles.
    TrimSyntax(Option<bool>),
    /// `None` opens the config menu; a name switches that config on.
    ConfigProfile(Option<String>),
    /// `None` opens the install menu; a name installs that language/theme.
    Install(Option<String>),
    Uninstall(String),
    Restart,
    Stats,
    /// The missed-keys screen; `None` is the last week.
    Missed(Option<MissedRange>),
    Help,
    /// Today's online daily: `None` uses the current mode.
    Daily(Option<Mode>),
    Leaderboard,
    Login,
    Logout,
    /// `None` shows the account; `Some` makes the profile public or private.
    Account(Option<bool>),
    /// A user's profile; `None` is your own.
    User(Option<String>),
    /// Search public profiles; `None` opens an empty search to type in.
    Search(Option<String>),
    /// `None` lists who you follow; a name opens their profile if you
    /// follow them, and follows them otherwise.
    Follow(Option<String>),
    Unfollow(String),
    Quit,
    Results {
        section: String,
        value: Option<bool>,
    },
    Set {
        key: String,
        value: String,
    },
}

/// What the palette should offer for a command's argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    None,
    Themes,
    Languages,
    Fonts,
    Profiles,
    /// Catalogue entries not installed yet.
    Installable,
    /// Installed languages and themes that can be removed.
    Removable,
    TimePresets,
    WordPresets,
    OnOff,
    ResultSections,
    /// `off`, `pb`, `last` or any wpm.
    Pace,
    /// Keyboard layouts for the missed-keys heatmap.
    Keyboards,
    /// last, day, week, month, all: how far back `:missed` looks.
    MissedRanges,
    /// both, confetti, trophy, off.
    PbEffects,
    /// `time 30`, `words 25`, …: the modes dailies exist for.
    DailyModes,
    /// `public on` / `public off`.
    Account,
    /// Players you follow, most recently viewed first.
    Follows,
    /// Numeric setting in an inclusive range. Typing a number sets it
    /// directly; Enter with no number opens an interactive slider.
    Slider {
        min: u8,
        max: u8,
    },
    /// Free-form; no completion.
    Free,
}

impl ArgKind {
    /// Whether values outside the suggested candidates are valid (presets
    /// are only suggestions; any number works).
    pub fn accepts_free_text(self) -> bool {
        matches!(
            self,
            ArgKind::TimePresets
                | ArgKind::WordPresets
                | ArgKind::DailyModes
                | ArgKind::Follows
                | ArgKind::Fonts
                | ArgKind::Installable
                | ArgKind::Free
                | ArgKind::Pace
                | ArgKind::Slider { .. }
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    /// Usage shown in the palette, e.g. `<seconds>`.
    pub usage: &'static str,
    pub help: &'static str,
    pub arg: ArgKind,
    /// If true the command is meaningless without an argument, so Enter on
    /// the bare name completes it instead of running.
    pub requires_arg: bool,
}

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "time",
        aliases: &["t"],
        usage: "<seconds>",
        help: "timed test",
        arg: ArgKind::TimePresets,
        requires_arg: true,
    },
    CommandSpec {
        name: "words",
        aliases: &["w"],
        usage: "<count>",
        help: "fixed word-count test",
        arg: ArgKind::WordPresets,
        requires_arg: true,
    },
    CommandSpec {
        name: "language",
        aliases: &["lang", "l"],
        usage: "<name>",
        help: "switch word list",
        arg: ArgKind::Languages,
        requires_arg: true,
    },
    CommandSpec {
        name: "theme",
        aliases: &["th"],
        usage: "<name>",
        help: "switch color theme",
        arg: ArgKind::Themes,
        requires_arg: true,
    },
    CommandSpec {
        name: "punctuation",
        aliases: &["punc", "p"],
        usage: "[on|off]",
        help: "toggle punctuation",
        arg: ArgKind::OnOff,
        requires_arg: false,
    },
    CommandSpec {
        name: "numbers",
        aliases: &["num", "n"],
        usage: "[on|off]",
        help: "toggle numbers",
        arg: ArgKind::OnOff,
        requires_arg: false,
    },
    CommandSpec {
        name: "fontsize",
        aliases: &["fs"],
        usage: "[1-16]",
        help: "text size (enter opens a slider)",
        arg: ArgKind::Slider {
            min: FONT_SIZE_RANGE.0,
            max: FONT_SIZE_RANGE.1,
        },
        requires_arg: false,
    },
    CommandSpec {
        name: "font",
        aliases: &["ff"],
        usage: "[family|file]",
        help: "font for sizes 2+ (empty = system monospace)",
        arg: ArgKind::Fonts,
        requires_arg: false,
    },
    CommandSpec {
        name: "wordsperline",
        aliases: &["wpl", "width"],
        usage: "[4-30]",
        help: "word box width (enter opens a slider)",
        arg: ArgKind::Slider {
            min: WORDS_PER_LINE_RANGE.0,
            max: WORDS_PER_LINE_RANGE.1,
        },
        requires_arg: false,
    },
    CommandSpec {
        name: "lines",
        aliases: &["ln"],
        usage: "[1-10]",
        help: "lines of words shown (enter opens a slider)",
        arg: ArgKind::Slider {
            min: LINES_RANGE.0,
            max: LINES_RANGE.1,
        },
        requires_arg: false,
    },
    CommandSpec {
        name: "fullscreen",
        aliases: &["full"],
        usage: "[on|off]",
        help: "largest text that fits, no chrome",
        arg: ArgKind::OnOff,
        requires_arg: false,
    },
    CommandSpec {
        name: "zen",
        aliases: &[],
        usage: "[on|off]",
        help: "words only, no chrome",
        arg: ArgKind::OnOff,
        requires_arg: false,
    },
    CommandSpec {
        name: "pace",
        aliases: &["ghost"],
        usage: "<off|pb|last|wpm>",
        help: "a ghost caret racing you at your best, your last run or a speed",
        arg: ArgKind::Pace,
        requires_arg: true,
    },
    CommandSpec {
        name: "modules",
        aliases: &["module", "libs"],
        usage: "",
        help: "pick the current language's modules (numpy, pandas, …)",
        arg: ArgKind::None,
        requires_arg: false,
    },
    CommandSpec {
        name: "trimsyntax",
        aliases: &["trim", "trim_syntax"],
        usage: "[on|off]",
        help: "strip each language's boilerplate, like python's ()",
        arg: ArgKind::OnOff,
        requires_arg: false,
    },
    CommandSpec {
        name: "pbeffect",
        aliases: &["celebrate", "wineffect"],
        usage: "<both|confetti|trophy|off>",
        help: "what a new personal best shows: confetti, a trophy, both or nothing",
        arg: ArgKind::PbEffects,
        requires_arg: true,
    },
    CommandSpec {
        name: "keyboard",
        aliases: &["layout", "kb"],
        usage: "<layout>",
        help: "keyboard layout of the results screen's missed keys",
        arg: ArgKind::Keyboards,
        requires_arg: true,
    },
    CommandSpec {
        name: "config",
        aliases: &["configs", "cfg"],
        usage: "[name]",
        help: "switch saved configs (enter opens the menu)",
        arg: ArgKind::Profiles,
        requires_arg: false,
    },
    CommandSpec {
        name: "install",
        aliases: &["catalog", "get"],
        usage: "[name]",
        help: "get more languages and themes (enter opens the menu)",
        arg: ArgKind::Installable,
        requires_arg: false,
    },
    CommandSpec {
        name: "uninstall",
        aliases: &["remove"],
        usage: "<name>",
        help: "remove an installed language or theme",
        arg: ArgKind::Removable,
        requires_arg: true,
    },
    CommandSpec {
        name: "restart",
        aliases: &["r"],
        usage: "",
        help: "new test",
        arg: ArgKind::None,
        requires_arg: false,
    },
    CommandSpec {
        name: "stats",
        aliases: &["s"],
        usage: "",
        help: "show history",
        arg: ArgKind::None,
        requires_arg: false,
    },
    CommandSpec {
        name: "missed",
        aliases: &["misses"],
        usage: "[last|day|week|month|all]",
        help: "missed keys across your tests",
        arg: ArgKind::MissedRanges,
        requires_arg: false,
    },
    CommandSpec {
        name: "results",
        aliases: &["res"],
        usage: "<section> [on|off]",
        help: "toggle results sections",
        arg: ArgKind::ResultSections,
        requires_arg: true,
    },
    CommandSpec {
        name: "set",
        aliases: &[],
        usage: "<key> <value>",
        help: "set any config key",
        arg: ArgKind::Free,
        requires_arg: true,
    },
    CommandSpec {
        name: "daily",
        aliases: &["d"],
        usage: "[mode]",
        help: "today's online daily test",
        arg: ArgKind::DailyModes,
        requires_arg: false,
    },
    CommandSpec {
        name: "leaderboard",
        aliases: &["lb"],
        usage: "",
        help: "daily and all-time leaderboards",
        arg: ArgKind::None,
        requires_arg: false,
    },
    CommandSpec {
        name: "login",
        aliases: &[],
        usage: "",
        help: "log in with github (needs server)",
        arg: ArgKind::None,
        requires_arg: false,
    },
    CommandSpec {
        name: "logout",
        aliases: &[],
        usage: "",
        help: "forget the server login",
        arg: ArgKind::None,
        requires_arg: false,
    },
    CommandSpec {
        name: "account",
        aliases: &["acct"],
        usage: "[public on|off]",
        help: "your online account; make your profile public or private",
        arg: ArgKind::Account,
        requires_arg: false,
    },
    CommandSpec {
        name: "user",
        aliases: &["profile", "me", "self", "whois"],
        usage: "[login]",
        help: "a player's public profile (yours with no name)",
        arg: ArgKind::Free,
        requires_arg: false,
    },
    CommandSpec {
        name: "search",
        aliases: &["players", "find"],
        usage: "[name]",
        help: "search public profiles: bests and medals (enter or p opens one)",
        arg: ArgKind::Free,
        requires_arg: false,
    },
    CommandSpec {
        name: "follow",
        aliases: &["following", "fl"],
        usage: "[login]",
        help: "players you follow; a followed name opens their profile, a new one follows",
        arg: ArgKind::Follows,
        requires_arg: false,
    },
    CommandSpec {
        name: "unfollow",
        aliases: &[],
        usage: "<login>",
        help: "stop following a player",
        arg: ArgKind::Follows,
        requires_arg: true,
    },
    CommandSpec {
        name: "help",
        aliases: &["h", "?"],
        usage: "",
        help: "keys and commands",
        arg: ArgKind::None,
        requires_arg: false,
    },
    CommandSpec {
        name: "quit",
        aliases: &["q", "exit"],
        usage: "",
        help: "exit ttyp from the words or the landing screen; back out anywhere else",
        arg: ArgKind::None,
        requires_arg: false,
    },
];

pub fn find_spec(name: &str) -> Option<&'static CommandSpec> {
    COMMANDS
        .iter()
        .find(|c| c.name == name || c.aliases.contains(&name))
}

fn parse_on_off(s: &str) -> Result<bool, String> {
    match s {
        "on" | "true" | "yes" | "1" => Ok(true),
        "off" | "false" | "no" | "0" => Ok(false),
        _ => Err(format!("expected on/off, got `{s}`")),
    }
}

fn parse_num(s: &str, what: &str) -> Result<u16, String> {
    s.parse::<u16>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("bad {what} `{s}`"))
}

/// Parse a full command line (without the leading `:`).
pub fn parse(line: &str) -> Result<Command, String> {
    let line = line.trim();
    let mut parts = line.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("");
    let rest = parts.next().map(str::trim).unwrap_or("");
    if name.is_empty() {
        return Err(String::new());
    }
    let spec = find_spec(name).ok_or_else(|| format!("unknown command `{name}`"))?;
    let need = |usage: &str| {
        if rest.is_empty() {
            Err(format!("usage: {} {usage}", spec.name))
        } else {
            Ok(rest)
        }
    };
    let cmd = match spec.name {
        "time" => Command::Time(parse_num(need(spec.usage)?, "seconds")?),
        "words" => Command::Words(parse_num(need(spec.usage)?, "count")?),
        "language" => Command::Language(need(spec.usage)?.to_string()),
        "theme" => Command::Theme(need(spec.usage)?.to_string()),
        "punctuation" => Command::Punctuation(opt_on_off(rest)?),
        "numbers" => Command::Numbers(opt_on_off(rest)?),
        "fontsize" => Command::FontSize(opt_range(rest, FONT_SIZE_RANGE)?),
        "wordsperline" => Command::WordsPerLine(opt_range(rest, WORDS_PER_LINE_RANGE)?),
        "font" => Command::Font(rest.to_string()),
        "zen" => Command::Zen(opt_on_off(rest)?),
        "pace" => Command::Pace(Pace::parse(need(spec.usage)?)?),
        "modules" => Command::Modules,
        "trimsyntax" => Command::TrimSyntax(opt_on_off(rest)?),
        "pbeffect" => Command::PbEffect(PbEffect::parse(need(spec.usage)?)?),
        "keyboard" => Command::Keyboard(Keyboard::parse(need(spec.usage)?)?),
        "lines" => Command::Lines(opt_range(rest, LINES_RANGE)?),
        "fullscreen" => Command::Fullscreen(opt_on_off(rest)?),
        "config" => Command::ConfigProfile(Some(rest.to_string()).filter(|r| !r.is_empty())),
        "install" => Command::Install(Some(rest.to_string()).filter(|r| !r.is_empty())),
        "uninstall" => Command::Uninstall(need(spec.usage)?.to_string()),
        "restart" => Command::Restart,
        "stats" => Command::Stats,
        "missed" => Command::Missed(if rest.is_empty() {
            None
        } else {
            Some(MissedRange::parse(rest)?)
        }),
        "help" => Command::Help,
        "daily" => Command::Daily(if rest.is_empty() {
            None
        } else {
            Some(parse_mode(rest)?)
        }),
        "leaderboard" => Command::Leaderboard,
        "login" => Command::Login,
        "logout" => Command::Logout,
        "account" => Command::Account(match rest {
            "" => None,
            r => match r.split_whitespace().collect::<Vec<_>>().as_slice() {
                ["public", v] => Some(parse_on_off(v)?),
                _ => return Err(format!("usage: account {}", spec.usage)),
            },
        }),
        "user" => Command::User(Some(rest.to_string()).filter(|r| !r.is_empty())),
        "search" => Command::Search(Some(rest.to_string()).filter(|r| !r.is_empty())),
        "follow" => Command::Follow(Some(rest.to_string()).filter(|r| !r.is_empty())),
        "unfollow" => Command::Unfollow(need(spec.usage)?.to_string()),
        "quit" => Command::Quit,
        "results" => {
            let mut a = rest.split_whitespace();
            let section = a
                .next()
                .ok_or_else(|| format!("usage: results {}", spec.usage))?;
            if ResultsConfig::SECTIONS.iter().all(|s| *s != section) {
                return Err(format!(
                    "unknown section `{section}` (one of {})",
                    ResultsConfig::SECTIONS.join(", ")
                ));
            }
            Command::Results {
                section: section.to_string(),
                value: a.next().map(parse_on_off).transpose()?,
            }
        }
        "set" => {
            let mut a = rest.splitn(2, char::is_whitespace);
            let key = a.next().filter(|k| !k.is_empty());
            let value = a.next().map(str::trim).filter(|v| !v.is_empty());
            match (key, value) {
                (Some(k), Some(v)) => Command::Set {
                    key: k.to_string(),
                    value: v.to_string(),
                },
                _ => return Err(format!("usage: set {}", spec.usage)),
            }
        }
        _ => unreachable!("every spec is handled"),
    };
    Ok(cmd)
}

fn opt_range(rest: &str, range: (u8, u8)) -> Result<Option<u8>, String> {
    if rest.is_empty() {
        Ok(None)
    } else {
        parse_range(rest, range).map(Some)
    }
}

fn opt_on_off(rest: &str) -> Result<Option<bool>, String> {
    if rest.is_empty() {
        Ok(None)
    } else {
        parse_on_off(rest).map(Some)
    }
}

/// Candidate argument values for a command, given the live registries.
/// `time 30` or `words 25` (also `t30`, `w25`).
fn parse_mode(s: &str) -> Result<Mode, String> {
    let s = s.trim();
    let (kind, value) = match s.split_once(char::is_whitespace) {
        Some((k, v)) => (k, v.trim()),
        None => s.split_at(s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len())),
    };
    let n = parse_num(value, "number")?;
    match kind {
        "time" | "t" => Ok(Mode::Time(n)),
        "words" | "w" => Ok(Mode::Words(n)),
        _ => Err(format!(
            "expected `time <seconds>` or `words <count>`, got `{s}`"
        )),
    }
}

pub fn arg_candidates(kind: ArgKind, comps: &Completions) -> Vec<String> {
    match kind {
        ArgKind::None | ArgKind::Free => vec![],
        ArgKind::Themes => comps.themes.clone(),
        ArgKind::Languages => comps.languages.clone(),
        ArgKind::Fonts => comps.fonts.clone(),
        ArgKind::Profiles => comps.profiles.clone(),
        ArgKind::Installable => comps.installable.clone(),
        ArgKind::Removable => comps.removable.clone(),
        ArgKind::TimePresets => Mode::TIME_PRESETS.iter().map(u16::to_string).collect(),
        ArgKind::WordPresets => Mode::WORD_PRESETS.iter().map(u16::to_string).collect(),
        ArgKind::OnOff => vec!["on".into(), "off".into()],
        ArgKind::ResultSections => ResultsConfig::SECTIONS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ArgKind::Pace => Pace::PRESETS.iter().map(|p| p.label()).collect(),
        ArgKind::PbEffects => PbEffect::ALL.iter().map(|e| e.label().into()).collect(),
        ArgKind::Keyboards => Keyboard::ALL.iter().map(|k| k.label().into()).collect(),
        ArgKind::MissedRanges => MissedRange::ALL.iter().map(|r| r.arg().into()).collect(),
        ArgKind::Account => vec!["public on".into(), "public off".into()],
        ArgKind::Follows => comps.follows.clone(),
        ArgKind::DailyModes => ttyp_core::api::DAILY_MODES
            .iter()
            .map(Mode::label)
            .collect(),
        // The slider is the completion; the palette stays empty.
        ArgKind::Slider { .. } => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands_and_aliases() {
        assert_eq!(parse("time 30"), Ok(Command::Time(30)));
        assert_eq!(parse("t 15"), Ok(Command::Time(15)));
        assert_eq!(parse("w 25"), Ok(Command::Words(25)));
        assert_eq!(
            parse("lang english_1k"),
            Ok(Command::Language("english_1k".into()))
        );
        assert_eq!(parse("th nord"), Ok(Command::Theme("nord".into())));
        assert_eq!(parse("punc"), Ok(Command::Punctuation(None)));
        assert_eq!(parse("numbers off"), Ok(Command::Numbers(Some(false))));
        assert_eq!(parse("q"), Ok(Command::Quit));
        assert_eq!(parse("pace pb"), Ok(Command::Pace(Pace::Pb)));
        assert_eq!(parse("ghost 87"), Ok(Command::Pace(Pace::Wpm(87))));
        assert_eq!(parse("pace off"), Ok(Command::Pace(Pace::Off)));
        assert_eq!(
            parse("keyboard colemak-dh"),
            Ok(Command::Keyboard(Keyboard::ColemakDh))
        );
        assert_eq!(
            parse("layout dvorak"),
            Ok(Command::Keyboard(Keyboard::Dvorak))
        );
        assert!(parse("keyboard").is_err());
        assert_eq!(parse("missed"), Ok(Command::Missed(None)));
        assert_eq!(
            parse("misses 30d"),
            Ok(Command::Missed(Some(MissedRange::Month)))
        );
        assert!(parse("missed year").is_err());
        assert_eq!(
            parse("pbeffect trophy"),
            Ok(Command::PbEffect(PbEffect::Trophy))
        );
        assert_eq!(parse("celebrate off"), Ok(Command::PbEffect(PbEffect::Off)));
        assert_eq!(parse("modules"), Ok(Command::Modules));
        assert_eq!(parse("trim on"), Ok(Command::TrimSyntax(Some(true))));
        assert_eq!(parse("trimsyntax"), Ok(Command::TrimSyntax(None)));
        assert!(parse("pace").is_err());
        assert!(parse("pace soon").is_err());
        assert_eq!(parse("daily"), Ok(Command::Daily(None)));
        assert_eq!(parse("lb"), Ok(Command::Leaderboard));
        assert_eq!(parse("account"), Ok(Command::Account(None)));
        assert_eq!(parse("account public on"), Ok(Command::Account(Some(true))));
        assert!(parse("account public maybe").is_err());
        assert!(parse("account private").is_err());
        assert_eq!(parse("user"), Ok(Command::User(None)));
        assert_eq!(
            parse("whois octocat"),
            Ok(Command::User(Some("octocat".into())))
        );
        assert_eq!(parse("search"), Ok(Command::Search(None)));
        assert_eq!(parse("find ann"), Ok(Command::Search(Some("ann".into()))));
        assert_eq!(parse("follow"), Ok(Command::Follow(None)));
        assert_eq!(
            parse("fl octocat"),
            Ok(Command::Follow(Some("octocat".into())))
        );
        assert_eq!(
            parse("unfollow octocat"),
            Ok(Command::Unfollow("octocat".into()))
        );
        assert!(parse("unfollow").is_err());
        assert_eq!(parse("d time 30"), Ok(Command::Daily(Some(Mode::Time(30)))));
        assert_eq!(
            parse("daily w25"),
            Ok(Command::Daily(Some(Mode::Words(25))))
        );
        assert!(parse("daily fast").is_err());
        assert_eq!(parse("fontsize"), Ok(Command::FontSize(None)));
        assert_eq!(parse("fs 3"), Ok(Command::FontSize(Some(3))));
        assert_eq!(parse("fs 12"), Ok(Command::FontSize(Some(12))));
        assert!(parse("fs 17").is_err());
        assert_eq!(parse("ln 5"), Ok(Command::Lines(Some(5))));
        assert_eq!(parse("lines"), Ok(Command::Lines(None)));
        assert!(parse("lines 11").is_err());
        assert_eq!(parse("full"), Ok(Command::Fullscreen(None)));
        assert_eq!(parse("wpl"), Ok(Command::WordsPerLine(None)));
        assert_eq!(parse("width 20"), Ok(Command::WordsPerLine(Some(20))));
        assert!(parse("wpl 2").is_err());
        assert_eq!(parse("font"), Ok(Command::Font(String::new())));
        assert_eq!(
            parse("font JetBrains Mono"),
            Ok(Command::Font("JetBrains Mono".into()))
        );
        // Graphics is config-file only.
        assert!(parse("graphics off").is_err());
        assert_eq!(parse("zen"), Ok(Command::Zen(None)));
        assert_eq!(parse("config"), Ok(Command::ConfigProfile(None)));
        assert_eq!(parse("profile"), Ok(Command::User(None)));
        assert_eq!(parse("me"), Ok(Command::User(None)));
        assert_eq!(parse("self"), Ok(Command::User(None)));
        assert_eq!(parse("install"), Ok(Command::Install(None)));
        assert_eq!(parse("get nord"), Ok(Command::Install(Some("nord".into()))));
        assert_eq!(parse("remove nord"), Ok(Command::Uninstall("nord".into())));
        assert!(parse("uninstall").is_err());
        assert_eq!(
            parse("cfg sprint"),
            Ok(Command::ConfigProfile(Some("sprint".into())))
        );
        assert_eq!(
            parse("results chart off"),
            Ok(Command::Results {
                section: "chart".into(),
                value: Some(false)
            })
        );
        assert_eq!(
            parse("set results.raw on"),
            Ok(Command::Set {
                key: "results.raw".into(),
                value: "on".into()
            })
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("time").is_err());
        assert!(parse("time abc").is_err());
        assert!(parse("time 0").is_err());
        assert!(parse("nope").is_err());
        assert!(parse("results bogus").is_err());
        assert!(parse("set onlykey").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn specs_are_unique() {
        let mut names: Vec<&str> = COMMANDS
            .iter()
            .flat_map(|c| std::iter::once(c.name).chain(c.aliases.iter().copied()))
            .collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(n, names.len());
    }
}

//! The `:` command language: a static registry of command specs (for the
//! palette) and a parser from a typed line to a `Command`.

pub mod palette;

pub use palette::{CommandLine, Completions, Suggestion};

use crate::config::ResultsConfig;
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
    Zoom(ZoomArg),
    Zen(Option<bool>),
    Restart,
    Stats,
    Help,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomArg {
    In,
    Out,
    Level(u8),
}

/// What the palette should offer for a command's argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    None,
    Themes,
    Languages,
    TimePresets,
    WordPresets,
    OnOff,
    ResultSections,
    ZoomArgs,
    /// Free-form; no completion.
    Free,
}

impl ArgKind {
    /// Whether values outside the suggested candidates are valid (presets
    /// are only suggestions; any number works).
    pub fn accepts_free_text(self) -> bool {
        matches!(
            self,
            ArgKind::TimePresets | ArgKind::WordPresets | ArgKind::Free
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
        name: "zoom",
        aliases: &["z"],
        usage: "[in|out|0-4]",
        help: "scale the layout",
        arg: ArgKind::ZoomArgs,
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
        help: "exit ttyp",
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
        "zoom" => Command::Zoom(match rest {
            "" | "in" | "+" => ZoomArg::In,
            "out" | "-" => ZoomArg::Out,
            n => ZoomArg::Level(
                n.parse::<u8>()
                    .ok()
                    .filter(|l| (*l as usize) < crate::config::ZOOM_LEVELS.len())
                    .ok_or_else(|| format!("usage: zoom {}", spec.usage))?,
            ),
        }),
        "zen" => Command::Zen(opt_on_off(rest)?),
        "restart" => Command::Restart,
        "stats" => Command::Stats,
        "help" => Command::Help,
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

fn opt_on_off(rest: &str) -> Result<Option<bool>, String> {
    if rest.is_empty() {
        Ok(None)
    } else {
        parse_on_off(rest).map(Some)
    }
}

/// Candidate argument values for a command, given the live registries.
pub fn arg_candidates(kind: ArgKind, themes: &[String], languages: &[String]) -> Vec<String> {
    match kind {
        ArgKind::None | ArgKind::Free => vec![],
        ArgKind::Themes => themes.to_vec(),
        ArgKind::Languages => languages.to_vec(),
        ArgKind::TimePresets => Mode::TIME_PRESETS.iter().map(u16::to_string).collect(),
        ArgKind::WordPresets => Mode::WORD_PRESETS.iter().map(u16::to_string).collect(),
        ArgKind::OnOff => vec!["on".into(), "off".into()],
        ArgKind::ResultSections => ResultsConfig::SECTIONS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ArgKind::ZoomArgs => ["in", "out"]
            .into_iter()
            .map(String::from)
            .chain((0..crate::config::ZOOM_LEVELS.len()).map(|l| l.to_string()))
            .collect(),
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
        assert_eq!(parse("zoom"), Ok(Command::Zoom(ZoomArg::In)));
        assert_eq!(parse("z out"), Ok(Command::Zoom(ZoomArg::Out)));
        assert_eq!(parse("zoom 4"), Ok(Command::Zoom(ZoomArg::Level(4))));
        assert!(parse("zoom 9").is_err());
        assert_eq!(parse("zen"), Ok(Command::Zen(None)));
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

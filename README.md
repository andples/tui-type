# ttyp

A minimal, monkeytype-style typing test for the terminal. Rust, ratatui, no
network. Themes and word lists are plain TOML files, installed from a small
catalogue as you want them (`:install`); your history stays on your machine.

```
ttyp
```

## Install

```sh
cargo install --path .
```

Needs a terminal with true-color support.

## Keys

| key | action |
|---|---|
| `esc` | open the command line |
| `:` | open the command line (when a test isn't running) |
| `tab` | restart with new words |
| `ctrl+w` / `ctrl+backspace` | delete the current word |
| `ctrl+=` / `ctrl+-` | font size up / down |
| `ctrl+c` | quit |

Backspace moves back into the previous word only if it was left with an
error, like monkeytype.

On the results screen: `tab` next test, `s` stats, `?` help.

## Commands

Press `esc`, start typing, and the palette fuzzy-filters as you go.
`↑`/`↓` (or `ctrl+p`/`ctrl+n`) move, `tab` completes, `enter` runs.

| command | aliases | effect |
|---|---|---|
| `time <seconds>` | `t` | timed test (15 / 30 / 60 / 120 suggested, any value works) |
| `words <count>` | `w` | fixed word count (10 / 25 / 50 / 100 suggested) |
| `language <name>` | `lang`, `l` | switch word list |
| `theme <name>` | `th` | switch theme (previews live while you pick) |
| `punctuation [on\|off]` | `punc`, `p` | toggle punctuation |
| `numbers [on\|off]` | `num`, `n` | toggle numbers |
| `results <section> [on\|off]` | `res` | show/hide `chart`, `breakdown`, `consistency`, `raw` |
| `fontsize [1-16]` | `fs` | text size — 1 terminal font, 2–16 pixel font in block glyphs |
| `wordsperline [4-30]` | `wpl`, `width` | width of the word box (× 6 characters per word) |
| `lines [1-10]` | `ln` | lines of words shown at once (default 3) |
| `fullscreen [on\|off]` | `full` | largest text that fits the terminal, no chrome |
| `zen [on\|off]` | | words only — hides the brand, timer and mode line |
| `profile [name]` | `profiles`, `pf` | open the profile menu, or switch a profile on |
| `install [name]` | `catalog`, `get` | open the install menu, or install and use a language/theme |
| `uninstall <name>` | `remove` | remove an installed language or theme |
| `set <key> <value>` | | any config key, e.g. `set results.chart off` |
| `restart` | `r` | new test |
| `stats` | `s` | history |
| `daily [mode]` | `d` | today's online daily test (needs `server`, see below) |
| `leaderboard` | `lb` | the daily leaderboards |
| `login` / `logout` | | log in with GitHub for the dailies |
| `help` | `h`, `?` | keys and commands |
| `quit` | `q` | exit |

`fontsize`, `wordsperline` and `lines` take a number directly, or press `enter`
with no number to open a slider on the bottom line: `←`/`→` (or `h`/`l`)
adjust with a live preview, `enter` applies, `esc` reverts.

Every change is written to the config file immediately.

### Installing languages and themes

ttyp ships with only `english`, `english_1k` and the `default` theme. Everything
else is in the catalogue and installed when you want it. `:install` opens a menu
with a tab for languages and a tab for themes:

```
install    languages 3/8    themes 2/14

    dutch       Dutch · 196 words          available
  ● english     English · 199 words         built in
  ○ english_1k  English 1k · 999 words      built in
› ○ spanish     Spanish · 258 words        installed
```

`↑↓`/`jk` move, `tab` (or `←→`) switches tabs, `enter` uses the highlighted
item and installs it first if needed, `i` only installs it, `d` removes it
(after a `y`), `r` fetches the catalogue again and `esc` goes back. On the
themes tab the highlighted theme previews live, whether or not it's
installed. `:install nord` and `:uninstall nord` do the same from the command
line.

Installed files go in `~/.config/ttyp/languages/` and `~/.config/ttyp/themes/`,
next to any you wrote yourself (the menu lists those as `local`). If your
config names a language or theme that isn't installed, such as `gruvbox`
from before themes moved to the catalogue, ttyp installs it at startup.

The catalogue is the [`catalog/`](catalog/) directory of this repository,
fetched from GitHub. Set `catalog` in the config to use another base URL or a
local directory, or `catalog = ""` to turn it off. With `server = ""` ttyp
never installs anything on its own at startup.

### Profiles

A profile is a named set of settings — any subset of them. `:profile`
opens the menu:

```
profiles

› ● sprint    mode time 15 · punctuation on
  ○ marathon  mode time 120
  ● night     theme nord · font size 3
  + new profile

  enabling marathon replaces sprint
```

`enter`/`space` switches the highlighted profile on or off, `n` creates one,
`e` edits, `d` deletes. In the editor, `space` picks which settings the
profile controls, `←`/`→` change the value (themes preview live), `u` takes
the value currently in use, `A` takes every current value, and `enter`
saves.

Several profiles can be on at once, so you can combine, say, a timing
profile with a look profile. When a profile you switch on sets something an
active profile also sets, that one is switched off (`●` becomes `○`) — only
the settings it shares get overwritten. Changing a setting by hand likewise
switches off any profile that sets it, so a `●` always means the profile is
fully in effect. Switching a profile off leaves the settings as they are.

`:profile <name>` switches one on straight from the command line.



A terminal can't change its own font from inside a program, so sizes 2–5
draw a hand-made 4×6 pixel font with block characters. Each step is a
gentle one:

| size | glyph | rows tall |
|---|---|---|
| 1 | terminal font | 1 |
| 2 (default) | sextant blocks | 2 |
| 3 | half blocks | 3 |
| 4 | sextant blocks, 2× pixels | 4 |
| 5 | half blocks, 2× pixels | 6 |
| 6–16 | sextant blocks, 4×–14× pixels | 8, 10, … 28 |

Sizes 2 and 4 use Unicode 13 sextant characters; if your terminal font
lacks them it falls back to another installed font (Noto Sans Symbols 2
covers them). Sizes 3 and 5 only need `▀ ▄ █`.

`ctrl+=` / `ctrl+-` step the size if your terminal passes those keys through.

### Fullscreen

`fullscreen on` fills the terminal with text: it leaves one column of margin,
hides the brand, timer and mode line, and picks the largest glyph at which
`wordsperline` × `lines` words still fit on screen, then fills the height
with as many lines as fit. It re-fits when the terminal is resized and isn't
limited to the numbered sizes, so it can land between them. Lower
`wordsperline` or `lines` for bigger text; `fontsize` is ignored while it's
on.

## Online dailies

Every day at 00:00 UTC the ttyp server generates the same random tests
for everyone, scores your runs and ranks them. ttyp comes pointed at the
public server, so `:login` and `:daily` work out of the box; the only
network traffic is what those commands do. The two settings behind it,
written into your config on first run:

```toml
# ~/.config/ttyp/config.toml (top of the file, above [mode] and [results])
server = "https://api-ttyp.andrewplescan.com"
github_client_id = "Ov23li4yNGanDPbJkMSV"
```

Point `server` at your own instance to use a different one (see
`crates/ttyp-server/README.md`), or set `server = ""` for a fully offline
ttyp. `:set server …` changes it live.

- `:login` shows a code to enter at github.com/login/device; ttyp then keeps
  a server token in `~/.local/share/ttyp/token` (mode 0600). Your GitHub
  token is used once and never stored. `:logout` forgets it.
- `:daily` opens today's daily for your language and mode (`:daily time 30`,
  `:daily words 25`; the palette lists what exists). Every day at 00:00 UTC
  the server generates 14 dailies: `english` and `english_1k` × words 10,
  25, 50, 100 and time 15, 30, 60, with no punctuation or numbers.
- Finishing a daily sends your keystrokes; the server replays them through
  the same engine and ranks the run on two boards, *first try* (your first
  attempt only) and *best*. The results screen shows both ranks. If the
  server can't be reached the result is queued and sent on the next start,
  as long as the daily's UTC day hasn't ended.
- An attempt counts from your first keystroke: ttyp tells the server the
  run has started, so restarting or quitting still uses up that attempt. A
  run the server didn't see start (typed while logged out or offline) is
  still ranked on *best* but can't be a first try.
- While a daily is under way, `tab`, `:restart` and commands that would
  start a new test wait until it's finished, so you can't throw a run away
  by accident. Turn that off with `:set daily_lock off` (`daily_lock = false`
  in the config).
- `:leaderboard` shows both boards side by side (or behind `tab` on narrow
  terminals): `↑↓`/`jk` move, `g`/`G` top/bottom, `←→` mode, `l` language,
  `[` `]` day, `enter` opens that run's wpm graph, `p` opens that player's
  profile, `esc` goes back. Your own row stays visible at the bottom when it
  scrolls off.
- `:user <login>` shows a player's profile: daily streak, personal best per
  language and mode, and their latest runs (`enter` opens a run's graph).
  `:user` alone shows yours. Profiles are private until you run
  `:account public on`; `:account` shows which it is. Leaderboards list your
  GitHub login either way.
- Badges: finishing 1st, 2nd or 3rd on a daily's *first try* board earns a
  badge once that daily's UTC day is over. They're counted per language, and
  the profile shows the total and each language (`english 2·1·0` is two 1sts,
  one 2nd, no 3rds).

Daily runs also land in your local history, marked with the daily's id.
Only daily results ever leave your machine.

A build for a different server can override both defaults: set
`TTYP_DEFAULT_SERVER` and `TTYP_DEFAULT_GITHUB_CLIENT_ID` in the
environment when building (`TTYP_DEFAULT_SERVER=https://… cargo install
--path . --locked`).

## Files

| | path |
|---|---|
| config | `~/.config/ttyp/config.toml` |
| user themes | `~/.config/ttyp/themes/*.toml` |
| user languages | `~/.config/ttyp/languages/*.toml` |
| profiles | `~/.config/ttyp/profiles/*.toml` |
| history | `~/.local/share/ttyp/history.jsonl` |
| server token | `~/.local/share/ttyp/token` |
| unsent daily results | `~/.local/share/ttyp/queue/` |

Those are the Linux paths. On macOS both the config and data dirs are
`~/Library/Application Support/ttyp/` (so `config.toml`, `profiles/`,
`history.jsonl` and `token` all live there).

`--config-dir` and `--data-dir` override these; `--theme` picks a theme for
one session without saving it.

### Adding a theme

Drop a file in `~/.config/ttyp/themes/`. A file with the same `name` as a
built-in replaces it.

```toml
name = "mine"

[colors]
bg = "#1e1e2e"          # screen background
fg = "#cdd6f4"          # headings, result numbers
sub = "#6c7086"         # untyped words, hints, mode line
main = "#cba6f7"        # accent: caret, timer, selection
correct = "#cdd6f4"     # correctly typed characters
error = "#f38ba8"       # wrong characters
error_extra = "#eba0ac" # characters typed past the end of a word
```

Built in: `default`. The rest are in the catalogue (`:install`).

### Adding a language

Drop a file in `~/.config/ttyp/languages/`:

```toml
name = "english_5k"
display = "English 5k"
words = ["the", "of", "and", ...]
```

Built in: `english` (199 words), `english_1k`. The rest are in the catalogue
(`:install`).

Programming languages are in the catalogue too, as `code_<language>` (keywords,
built-ins and common idioms): `code_python`, `code_cpp`, `code_c`, `code_javascript`,
`code_typescript`, `code_ocaml`, `code_java`, `code_csharp`, `code_go`, `code_rust`,
`code_php`, `code_ruby`, `code_swift`, `code_kotlin`, `code_sql`, `code_bash`,
`code_lua`, `code_r`, `code_haskell`, `code_scala`, `code_dart`, `code_perl`,
`code_elixir` and `code_zig`. They're generated by `scripts/code-languages.py`.

### Adding to the catalogue

Put the file in `catalog/languages/` or `catalog/themes/`, named after its
`name`, then regenerate the index with `TTYP_BLESS=1 cargo test catalog_index`
(the test fails while `catalog/index.toml` is stale). Names are lowercase
letters, digits, `-` and `_`. Installs pick it up once it's on `main`.

### Credits

25 of the catalogue themes (`serika-dark`, `8008`, `iceberg-dark`, …) are
adapted from [Monkeytype](https://github.com/monkeytypegame/monkeytype)'s themes
by the Monkeytype contributors, licensed under
[GPL-3.0](https://github.com/monkeytypegame/monkeytype/blob/master/LICENSE); those
files keep that license. [`catalog/themes/MONKEYTYPE.md`](catalog/themes/MONKEYTYPE.md)
lists them and says how the colours were mapped.

### Profile files

A profile is a TOML file named after the profile, holding any subset of
`config.toml`. Only the keys present are applied:

```toml
# ~/.config/ttyp/profiles/sprint.toml
punctuation = true
mode = { time = 15 }

[results]
chart = false
```

Unknown keys or out-of-range values are reported at startup and the file
is skipped. If an active profile's file no longer matches the config (edited
or deleted), it is switched off on the next start.

## Metrics

Same definitions as monkeytype:

- **wpm** — characters of correctly typed words (including the space) ÷ 5,
  per minute
- **raw** — all typed characters ÷ 5, per minute
- **acc** — correct keystrokes ÷ all keystrokes
- **consistency** — from the variation of per-second raw speed
- **chars** — correct / incorrect / extra / missed

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run
```

The repo is a workspace: the `ttyp` binary at the root, `crates/ttyp-core`
(engine, generator, metrics, languages and the API types, shared with the
server) and `crates/ttyp-server` (the online server; see its README for
running it). Core logic (`ttyp-core`, `src/command`, `src/config`,
`src/stats`, `src/theme`, `src/online`) is terminal-free and unit-tested;
`src/ui` only renders it.

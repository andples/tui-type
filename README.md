# ttyp

A minimal, monkeytype-style typing test for the terminal. Rust, ratatui, no
network. Themes and word lists are plain TOML files; your history stays on
your machine.

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
| `fontsize [1-5]` | `fs`, `font` | text size — 1 terminal font, 2–5 pixel font in block glyphs |
| `wordsperline [4-30]` | `wpl`, `width` | width of the word box (× 6 characters per word) |
| `zen [on\|off]` | | words only — hides the brand, timer and mode line |
| `set <key> <value>` | | any config key, e.g. `set results.chart off` |
| `restart` | `r` | new test |
| `stats` | `s` | history |
| `help` | `h`, `?` | keys and commands |
| `quit` | `q` | exit |

`fontsize` and `wordsperline` take a number directly, or press `enter`
with no number to open a slider on the bottom line: `←`/`→` (or `h`/`l`)
adjust with a live preview, `enter` applies, `esc` reverts.

Every change is written to the config file immediately.

### Font sizes

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

Sizes 2 and 4 use Unicode 13 sextant characters; if your terminal font
lacks them it falls back to another installed font (Noto Sans Symbols 2
covers them). Sizes 3 and 5 only need `▀ ▄ █`.

`ctrl+=` / `ctrl+-` step the size if your terminal passes those keys through.

## Files

| | path |
|---|---|
| config | `~/.config/ttyp/config.toml` |
| user themes | `~/.config/ttyp/themes/*.toml` |
| user languages | `~/.config/ttyp/languages/*.toml` |
| history | `~/.local/share/ttyp/history.jsonl` |

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

Built in: `default`, `gruvbox`, `catppuccin-mocha`, `nord`, `rose-pine`,
`light`.

### Adding a language

Drop a file in `~/.config/ttyp/languages/`:

```toml
name = "english_5k"
display = "English 5k"
words = ["the", "of", "and", ...]
```

Built in: `english` (200 words), `english_1k`.

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
cargo test
cargo clippy --all-targets -- -D warnings
cargo run
```

Core logic (`src/test`, `src/command`, `src/config`, `src/stats`,
`src/theme`, `src/language`) is terminal-free and unit-tested; `src/ui`
only renders it. `WordGenerator` and `StatsStore` are the seams for future
learning modes and remote sync.

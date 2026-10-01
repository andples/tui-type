# Language modules and trim syntax

Two ways to make a programming language's word list fit what you actually
write, without growing the base list:

- **Modules**: optional extra word lists for common libraries. Python has
  `stdlib`, `numpy`, `pandas`, `pytorch` and `tensorflow`. The base list
  keeps only the language itself (keywords, built-ins, common methods,
  typing, exceptions, operators, idioms), and library calls live in modules.
- **Trim syntax**: boilerplate that every call repeats, such as Python's
  `()`. The `trim_syntax` setting (`:trim on`, off by default) cuts it out of
  every word: `print()` becomes `print`, while `len(xs)` stays as it is.

Python is the first language with both. This page describes how they work
and how to add them to another language.

## Using them

- `:install` → `enter` on a language that has modules opens a checklist of
  them (`space` ticks, `a` ticks all, `enter` downloads the ticked ones,
  removes unticked installed ones, and switches to the language with the
  ticked ones mixed in).
- Switching to a language that has modules installed (`:language
  code_python`) opens a checklist of the installed ones to choose which to
  mix in. `esc` keeps the previous choice.
- `:modules` opens it for the current language at any time.
- `:trim on|off` (`:trimsyntax`, or `trim_syntax = true` in the config, or a
  saved config) toggles trimming.

A run with modules is recorded as `code_python+numpy+pandas` (modules
sorted), so personal bests, the pace caret and the new-best check compare
like with like. Trim syntax doesn't change the key.

In the config:

```toml
language = "code_python"
trim_syntax = true

[modules]
code_python = ["numpy", "pandas"]
```

## Files

```
catalog/
  index.toml                       # lists each language's modules
  languages/
    code_python.toml               # name, display, trim, words
    code_python/
      numpy.toml                   # name, display, words
      pandas.toml
      …
```

Once installed, the same layout appears under `~/.config/ttyp/languages/`.
The language registry only reads the top level of that directory, so module
files are never picked up as languages.

A module file is an ordinary language file: `name` (must match the file
name), `display`, `words` and an optional `trim`. A language's `trim` applies
to its modules' words too.

```toml
name = "code_python"
display = "Python (code)"
trim = ["()"]
words = ["def", "print()", "len(xs)", …]
```

`trim` is a list of substrings removed from each word when `trim_syntax` is
on. Words that end up empty (a bare `()`) or repeated are dropped. Pick
strings that are pure boilerplate for the language. A Rust list might use
`["!()"]` for macros, but not `"()"` alone if that would mangle closures.

## Adding modules or trim to another language

1. In `scripts/code-languages.py`, give the language a `trim=[…]` if it has
   boilerplate worth cutting, and add `module("<language>", "<name>",
   "<Display>", r"""…""")` blocks for its libraries. Every entry is one token
   without spaces. Move library calls out of the base list into a module.
2. Run `scripts/code-languages.py catalog/languages`. It writes
   `code_<language>.toml` and `code_<language>/<module>.toml`.
3. Run `TTYP_BLESS=1 cargo test catalog_index` to regenerate
   `catalog/index.toml`. The test fails while it's stale.
4. Push to `main`. The catalogue is fetched from there, so the modules show
   up in `:install` without a release.

A language written by hand (not in the script) works the same way: put the
module files in `catalog/languages/<language>/` and add `trim` to its file.

## Code map

- `crates/ttyp-core/src/language/mod.rs`: `Language::trim`.
- `src/catalog/modules.rs`: `ModuleRegistry` (installed modules, by
  language), `word_pool` (base + modules, trimmed), `language_key`.
- `src/catalog/mod.rs`: `LanguageEntry::modules`, `ModuleEntry`,
  `Source::fetch_module`. `src/catalog/worker.rs`:
  `CatalogEvent::ModuleFetched`.
- `src/catalog/module_menu.rs`: the checklist's state (`Purpose::Download`
  or `Pick`). `src/ui/modules.rs` draws it. `src/app/modules.rs` handles its
  keys, downloads and choices.
- `Config::modules`, `Config::trim_syntax`. `App::build_engine` builds the
  word pool, and `App::language_key` is what runs are recorded as.

Old clients ignore `trim` and an index entry's `modules`, so the catalogue
stays readable by them.

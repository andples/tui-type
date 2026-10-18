use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use ttyp::app::App;
use ttyp::config::Paths;

/// A minimal monkeytype-style typing test for the terminal.
#[derive(Parser, Debug)]
#[command(name = "ttyp", version, about)]
struct Cli {
    /// Config directory (default: $XDG_CONFIG_HOME/ttyp).
    #[arg(long, value_name = "DIR")]
    config_dir: Option<PathBuf>,
    /// Data directory for history (default: $XDG_DATA_HOME/ttyp).
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    /// Theme to use for this session (does not change the config file).
    #[arg(long, value_name = "NAME")]
    theme: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = match (&cli.config_dir, &cli.data_dir) {
        (None, None) => Paths::discover(),
        (c, d) => {
            let default = Paths::discover();
            let config_dir = c.clone().unwrap_or_else(|| {
                default
                    .config_file
                    .parent()
                    .map(PathBuf::from)
                    .unwrap_or_default()
            });
            let data_dir = d.clone().unwrap_or_else(|| {
                default
                    .history_file
                    .parent()
                    .map(PathBuf::from)
                    .unwrap_or_default()
            });
            Paths::from_dirs(&config_dir, &data_dir)
        }
    };

    ttyp::errlog::init(paths.log_file.clone());
    let mut app = App::new(paths, cli.theme).inspect_err(|e| {
        ttyp::errlog::error("startup", format!("{e:#}"));
    })?;

    // Restore the terminal even if we panic, so a crash never leaves the
    // shell in raw mode.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ttyp::errlog::log(ttyp::errlog::Level::Error, "panic", info);
        // A background thread's panic is caught and reported by its
        // worker (`errlog::catch_panic`) while the UI carries on: leave the
        // terminal alone and keep the text off the screen.
        if std::thread::current().name() != Some("main") {
            return;
        }
        ratatui::restore();
        hide_pointer_while_typing(false);
        default_hook(info);
    }));

    let mut terminal = ratatui::init();
    hide_pointer_while_typing(true);
    let result = app.run(&mut terminal);
    if let Err(e) = &result {
        ttyp::errlog::error("run", format!("{e:#}"));
    }
    ratatui::restore();
    hide_pointer_while_typing(false);
    result
}

/// Ask the terminal to hide the mouse pointer from the first key until the
/// mouse moves (xterm's XTSMPOINTER, `CSI > 2 p`), and back to its default
/// (`CSI > 1 p`) on exit. Terminals without it ignore the sequence; ghostty,
/// kitty and alacritty have their own setting instead.
fn hide_pointer_while_typing(on: bool) {
    use std::io::Write;
    let mode = if on { 2 } else { 1 };
    let mut out = std::io::stdout();
    let _ = write!(out, "\x1b[>{mode}p");
    let _ = out.flush();
}

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

    let mut app = App::new(paths, cli.theme)?;

    // Restore the terminal even if we panic, so a crash never leaves the
    // shell in raw mode.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        default_hook(info);
    }));

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

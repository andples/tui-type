//! Live themes from utheme, macOS only (`crate::utheme_client`). With
//! `theme = "utheme"` ttyp shows utheme's current palette and recolours on
//! every switch; any other theme leaves utheme alone. `App::theme` prefers
//! the live theme; when there is none it falls back to the static
//! `themes/utheme.toml` utheme writes, then to `default`.

use std::time::Duration;

use super::App;
use crate::theme::{Colors, Hex, Theme};
use crate::utheme_client::{self, Client, Paths, Rgb, Snapshot, Update, mix};

/// The palette keys the role mapping reads.
const NEEDS: &[&str] = &["background", "foreground", "muted", "accent", "red"];

/// How often the event loop looks for a switch while following. The watcher
/// thread does the reading; this only bounds how long a recolour waits for
/// the next frame when no key comes in.
const WAKE: Duration = Duration::from_millis(200);

/// Following state: the client while `theme = "utheme"`, and the theme it
/// last delivered. Dropping it removes the heartbeat.
#[derive(Default)]
pub struct Live {
    client: Option<Client>,
    theme: Option<Theme>,
    /// Where utheme lives, when not `$UTHEME_HOME`/`$HOME` (tests).
    paths: Option<Paths>,
}

/// utheme's palette in ttyp's roles (protocol section 7, the same as the
/// static theme utheme generates).
pub fn theme_from(s: &Snapshot) -> Result<Theme, String> {
    let hex = |c: Rgb| Hex(c.0, c.1, c.2);
    let (bg, red) = (s.color("background")?, s.color("red")?);
    Ok(Theme {
        name: utheme_client::THEME.to_string(),
        colors: Colors {
            bg: hex(bg),
            fg: hex(s.color("foreground")?),
            sub: hex(s.color("muted")?),
            main: hex(s.color("accent")?),
            correct: hex(s.color("foreground")?),
            error: hex(red),
            error_extra: hex(mix(red, bg, 0.35)),
        },
    })
}

impl App {
    /// Starts or stops following to match `config.theme` (the command
    /// line, a config profile or a sync can change it), then applies
    /// whatever switch has arrived.
    pub(super) fn utheme_tick(&mut self) {
        let wanted = self.config.theme == utheme_client::THEME;
        if wanted && self.utheme.client.is_none() {
            let (app, version) = ("ttyp", env!("CARGO_PKG_VERSION"));
            let started = match &self.utheme.paths {
                Some(p) => Some(Client::start_in(p.clone(), app, version, NEEDS)),
                None => Client::start(app, version, NEEDS),
            };
            if let Some((client, first)) = started {
                self.utheme.client = Some(client);
                self.utheme_apply(first);
            }
        } else if !wanted && self.utheme.client.is_some() {
            self.utheme_stop();
            self.dirty = true;
        }
        if let Some(u) = self.utheme.client.as_mut().and_then(Client::poll) {
            self.utheme_apply(u);
        }
    }

    fn utheme_apply(&mut self, update: Update) {
        let why = match update {
            Update::Theme(s) => match theme_from(&s) {
                Ok(t) => {
                    self.utheme.theme = Some(t);
                    self.dirty = true;
                    return;
                }
                Err(e) => e,
            },
            // Keep the colours already showing.
            Update::Error(e) if self.utheme.theme.is_some() => {
                self.notify(format!("utheme: {e}"));
                return;
            }
            Update::Error(e) => e,
            Update::Gone => {
                self.utheme.theme = None;
                self.dirty = true;
                "utheme isn't set up".to_string()
            }
        };
        if self.themes.get(utheme_client::THEME).is_none() {
            self.notify(format!("{why}, using default"));
        }
    }

    /// The live theme, while following and once one has arrived.
    pub(super) fn utheme_theme(&self) -> Option<&Theme> {
        (self.config.theme == utheme_client::THEME)
            .then_some(self.utheme.theme.as_ref())
            .flatten()
    }

    /// `timeout` shortened so a switch is picked up while following, and
    /// whether it was shortened (then waking up isn't a `Tick`).
    pub(super) fn utheme_wait(&self, timeout: Option<Duration>) -> (Option<Duration>, bool) {
        match (&self.utheme.client, timeout) {
            (Some(_), Some(t)) if t <= WAKE => (Some(t), false),
            (Some(_), _) => (Some(WAKE), true),
            (None, t) => (t, false),
        }
    }

    /// Stop following and remove the heartbeat (on quit, or another theme).
    pub(super) fn utheme_stop(&mut self) {
        self.utheme.client = None;
        self.utheme.theme = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utheme_client::heartbeat;
    use crate::utheme_client::testing::{generation, switch};
    use std::collections::HashMap;
    use std::time::Instant;

    fn snapshot(colors: &[(&str, &str)]) -> Snapshot {
        Snapshot {
            generation: "g1".into(),
            id: "nord".into(),
            name: "Nord".into(),
            mode: "dark".into(),
            palette: colors
                .iter()
                .map(|(k, v)| (k.to_string(), Rgb::parse(v).unwrap()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn roles_match_the_static_theme() {
        let s = snapshot(&[
            ("background", "#2e3440"),
            ("foreground", "#d8dee9"),
            ("muted", "#4c566a"),
            ("accent", "#88c0d0"),
            ("red", "#ff0000"),
        ]);
        let t = theme_from(&s).unwrap();
        assert_eq!(t.name, "utheme");
        let c = &t.colors;
        assert_eq!(c.bg, Hex(0x2e, 0x34, 0x40));
        assert_eq!(
            (c.fg, c.correct),
            (Hex(0xd8, 0xde, 0xe9), Hex(0xd8, 0xde, 0xe9))
        );
        assert_eq!(c.sub, Hex(0x4c, 0x56, 0x6a));
        assert_eq!(c.main, Hex(0x88, 0xc0, 0xd0));
        assert_eq!(c.error, Hex(0xff, 0, 0));
        // mix(red, background, 35%), as in utheme's ttyp.toml.tpl.
        assert_eq!(c.error_extra, Hex(0xb6, 0x12, 0x16));
        assert!(theme_from(&snapshot(&[("background", "#000000")])).is_err());
    }

    #[test]
    fn follows_switches_and_lets_go_on_another_theme() {
        let paths = crate::app::tests::scratch("utheme", "server = \"\"\nsplash = false\n", 0);
        let root = paths
            .config_file
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("uth");
        let uth = Paths::under(&root);
        switch(&uth, &generation(&uth, "g1", "#111111"));
        let mut a = App::new(paths.clone(), None).unwrap();
        a.utheme.paths = Some(uth.clone());

        // Picked in the app: the live palette, at once.
        a.execute_line("theme utheme");
        a.utheme_tick();
        assert_eq!(a.theme().name, "utheme");
        assert_eq!(a.theme().colors.bg, Hex(0x11, 0x11, 0x11));
        assert_eq!(a.theme().colors.main, Hex(0x88, 0xc0, 0xd0));
        let beat = heartbeat::path(&uth, "ttyp");
        assert!(std::fs::read_to_string(&beat).unwrap().contains("\"g1\""));
        assert!(a.utheme_wait(None).1, "wakes to look for a switch");
        // From here on only utheme changes: the config must stay byte for byte.
        let config = std::fs::read(&paths.config_file).unwrap();

        switch(&uth, &generation(&uth, "g2", "#222222"));
        let start = Instant::now();
        while a.theme().colors.bg != Hex(0x22, 0x22, 0x22) {
            assert!(start.elapsed() < Duration::from_secs(3), "no recolour");
            std::thread::sleep(Duration::from_millis(20));
            a.utheme_tick();
        }
        assert!(a.dirty);
        assert!(std::fs::read_to_string(&beat).unwrap().contains("\"g2\""));
        assert_eq!(std::fs::read(&paths.config_file).unwrap(), config);

        // Another theme: no watching, no heartbeat; back again follows again.
        a.execute_line("theme default");
        a.utheme_tick();
        assert_eq!(a.theme().name, "default");
        assert!(!beat.exists());
        assert_eq!(a.utheme_wait(None), (None, false));
        a.execute_line("theme utheme");
        a.utheme_tick();
        assert_eq!(a.theme().colors.bg, Hex(0x22, 0x22, 0x22));
        let config = std::fs::read(&paths.config_file).unwrap();

        // Uninstalled: back to the fallback, heartbeat gone.
        std::fs::remove_file(&uth.current).unwrap();
        let start = Instant::now();
        while a.theme().name == "utheme" {
            assert!(start.elapsed() < Duration::from_secs(3), "no fallback");
            std::thread::sleep(Duration::from_millis(20));
            a.utheme_tick();
        }
        assert_eq!(a.theme().name, "default");
        assert!(!beat.exists());

        assert_eq!(std::fs::read(&paths.config_file).unwrap(), config);
        // No themes/utheme.toml here, and none is fetched from the catalogue.
        assert!(a.is_installed(crate::catalog::Kind::Theme, "utheme"));
        let saved = String::from_utf8(config).unwrap();
        assert!(
            saved.contains("theme = \"utheme\""),
            "the user's choice stays: {saved}"
        );
    }
}

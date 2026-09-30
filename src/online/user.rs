//! Profile screen state, no rendering: whose profile, the server's reply,
//! and a cursor over the recent runs (enter opens one's graph).

use ttyp_core::api::{Profile, ProfileRun};

use crate::ui::widgets::Selection;

#[derive(Debug)]
pub struct UserView {
    /// As asked for; the profile carries the canonical spelling.
    pub login: String,
    pub profile: Option<Profile>,
    pub loading: bool,
    pub error: Option<String>,
    pub selection: Selection,
}

impl UserView {
    pub fn new(login: String) -> Self {
        Self {
            login,
            profile: None,
            loading: true,
            error: None,
            selection: Selection::clamped(0),
        }
    }

    /// Whether a reply for `login` belongs to this view (logins ignore case).
    pub fn wants(&self, login: &str) -> bool {
        self.login.eq_ignore_ascii_case(login)
    }

    pub fn set(&mut self, profile: Profile) {
        self.loading = false;
        self.error = None;
        self.selection.set_len(profile.recent.len());
        self.profile = Some(profile);
    }

    pub fn fail(&mut self, e: String) {
        self.loading = false;
        self.error = Some(e);
    }

    /// The recent run under the cursor.
    pub fn selected(&self) -> Option<&ProfileRun> {
        self.profile.as_ref()?.recent.get(self.selection.selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttyp_core::test::Mode;

    fn run(id: i64) -> ProfileRun {
        ProfileRun {
            result_id: id,
            date: "2026-09-30".into(),
            language: "english".into(),
            mode: Mode::Time(30),
            wpm: 80.0,
            acc: 97.0,
            attempt: 1,
        }
    }

    #[test]
    fn cursor_follows_the_recent_runs() {
        let mut v = UserView::new("alice".into());
        assert!(v.wants("Alice"));
        assert!(v.selected().is_none());
        v.set(Profile {
            login: "Alice".into(),
            public: true,
            joined: "2026-09-29".into(),
            streak: 2,
            dailies: 3,
            bests: vec![run(1)],
            recent: vec![run(3), run(2)],
        });
        assert!(!v.loading);
        assert_eq!(v.selected().map(|r| r.result_id), Some(3));
        v.selection.move_by(1);
        assert_eq!(v.selected().map(|r| r.result_id), Some(2));
        v.selection.move_by(1);
        assert_eq!(v.selected().map(|r| r.result_id), Some(2), "clamped");
    }
}

//! GitHub's OAuth device flow, done by the client with the public client
//! id: ask for a code, show it, poll until the user has entered it. The
//! HTTP side is a trait so the flow is testable without GitHub.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const CODE_URL: &str = "https://github.com/login/device/code";
pub const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// A form POST returning GitHub's JSON reply.
pub trait Form {
    fn post_form(&self, url: &str, fields: &[(&str, &str)]) -> Result<serde_json::Value, String>;
}

impl Form for ureq::Agent {
    fn post_form(&self, url: &str, fields: &[(&str, &str)]) -> Result<serde_json::Value, String> {
        self.post(url)
            .header("Accept", "application/json")
            .send_form(fields.iter().copied())
            .map_err(|e| super::net::explain(&e, super::net::host_of(url)))?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub device_code: String,
    /// What the user types in at `verification_uri`.
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds between polls.
    pub interval: u64,
    pub expires_in: u64,
}

pub fn start(http: &impl Form, client_id: &str) -> Result<DeviceCode, String> {
    let v = http.post_form(
        CODE_URL,
        &[("client_id", client_id), ("scope", "read:user")],
    )?;
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        return Err(format!("github: {e}"));
    }
    let field = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("github reply missing `{k}`"))
    };
    Ok(DeviceCode {
        device_code: field("device_code")?,
        user_code: field("user_code")?,
        verification_uri: field("verification_uri")?,
        interval: v.get("interval").and_then(|x| x.as_u64()).unwrap_or(5),
        expires_in: v.get("expires_in").and_then(|x| x.as_u64()).unwrap_or(900),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Poll {
    Pending,
    /// Back off: add 5 s to the interval.
    SlowDown,
    Token(String),
    Failed(String),
}

pub fn poll(http: &impl Form, client_id: &str, device_code: &str) -> Poll {
    let v = match http.post_form(
        TOKEN_URL,
        &[
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", GRANT),
        ],
    ) {
        Ok(v) => v,
        Err(e) => return Poll::Failed(e),
    };
    if let Some(t) = v.get("access_token").and_then(|t| t.as_str()) {
        return Poll::Token(t.to_string());
    }
    match v.get("error").and_then(|e| e.as_str()) {
        Some("authorization_pending") => Poll::Pending,
        Some("slow_down") => Poll::SlowDown,
        Some("expired_token") => Poll::Failed("code expired, run :login again".into()),
        Some("access_denied") => Poll::Failed("login cancelled on github".into()),
        Some(e) => Poll::Failed(format!("github: {e}")),
        None => Poll::Failed("github reply had no token".into()),
    }
}

/// The whole flow: `on_code` gets the code to show, then we poll at
/// GitHub's pace until a token, a failure, expiry or `cancel`.
pub fn run(
    http: &impl Form,
    client_id: &str,
    mut on_code: impl FnMut(&DeviceCode),
    cancel: &AtomicBool,
    sleep: impl Fn(Duration),
) -> Result<String, String> {
    let code = start(http, client_id)?;
    on_code(&code);
    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval);
    loop {
        sleep(interval);
        if cancel.load(Ordering::Relaxed) {
            return Err("login cancelled".into());
        }
        if Instant::now() >= deadline {
            return Err("code expired, run :login again".into());
        }
        match poll(http, client_id, &code.device_code) {
            Poll::Pending => {}
            Poll::SlowDown => interval += Duration::from_secs(5),
            Poll::Token(t) => return Ok(t),
            Poll::Failed(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::json;

    use super::*;

    /// A recorded call: url and form fields.
    type Call = (String, Vec<(String, String)>);

    /// Replies in order; records every call.
    struct Script {
        replies: RefCell<Vec<serde_json::Value>>,
        calls: RefCell<Vec<Call>>,
    }

    impl Form for Script {
        fn post_form(
            &self,
            url: &str,
            fields: &[(&str, &str)],
        ) -> Result<serde_json::Value, String> {
            self.calls.borrow_mut().push((
                url.to_string(),
                fields
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ));
            let mut r = self.replies.borrow_mut();
            if r.is_empty() {
                return Err("no reply scripted".into());
            }
            Ok(r.remove(0))
        }
    }

    #[test]
    fn flow_shows_code_then_polls_at_githubs_pace() {
        let http = Script {
            replies: RefCell::new(vec![
                json!({"device_code": "dc", "user_code": "ABCD-1234",
                       "verification_uri": "https://github.com/login/device",
                       "interval": 1, "expires_in": 900}),
                json!({"error": "authorization_pending"}),
                json!({"error": "slow_down"}),
                json!({"error": "authorization_pending"}),
                json!({"access_token": "gho_x", "token_type": "bearer"}),
            ]),
            calls: RefCell::new(Vec::new()),
        };
        let shown = RefCell::new(None);
        let slept = RefCell::new(Vec::new());
        let token = run(
            &http,
            "cid",
            |c| *shown.borrow_mut() = Some(c.user_code.clone()),
            &AtomicBool::new(false),
            |d| slept.borrow_mut().push(d.as_secs()),
        )
        .unwrap();
        assert_eq!(token, "gho_x");
        assert_eq!(shown.borrow().as_deref(), Some("ABCD-1234"));
        // 1 s until the slow_down reply, then 6 s.
        assert_eq!(*slept.borrow(), vec![1, 1, 6, 6]);
        let calls = http.calls.borrow();
        assert_eq!(calls[0].0, CODE_URL);
        assert!(
            calls[0]
                .1
                .contains(&("scope".to_string(), "read:user".to_string()))
        );
        assert_eq!(calls[1].0, TOKEN_URL);
        assert!(
            calls[1]
                .1
                .contains(&("grant_type".to_string(), GRANT.to_string()))
        );
    }

    #[test]
    fn denied_cancelled_and_bad_replies() {
        let denied = Script {
            replies: RefCell::new(vec![
                json!({"device_code": "dc", "user_code": "X", "verification_uri": "u"}),
                json!({"error": "access_denied"}),
            ]),
            calls: RefCell::new(Vec::new()),
        };
        let err = run(&denied, "cid", |_| {}, &AtomicBool::new(false), |_| {}).unwrap_err();
        assert!(err.contains("cancelled on github"), "{err}");

        let cancelled = Script {
            replies: RefCell::new(vec![
                json!({"device_code": "dc", "user_code": "X", "verification_uri": "u"}),
            ]),
            calls: RefCell::new(Vec::new()),
        };
        let err = run(&cancelled, "cid", |_| {}, &AtomicBool::new(true), |_| {}).unwrap_err();
        assert_eq!(err, "login cancelled");

        let bad = Script {
            replies: RefCell::new(vec![json!({"error": "incorrect_client_credentials"})]),
            calls: RefCell::new(Vec::new()),
        };
        assert!(
            start(&bad, "cid")
                .unwrap_err()
                .contains("incorrect_client_credentials")
        );
    }
}

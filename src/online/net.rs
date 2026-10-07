//! Network failures in plain words. `explain` turns a ureq error into what
//! went wrong and what to do about it: offline, a name that doesn't
//! resolve, a server that's down or slow, a dropped connection.

use std::io::ErrorKind;
use std::net::ToSocketAddrs;

/// A well-known name to tell "no DNS at all" from "this name is gone".
const PROBE: &str = "github.com:443";

/// What went wrong reaching `host`, for the user. A failed name lookup is
/// checked against a well-known name so being offline isn't blamed on the
/// server (this may block for a moment; only call it off the UI thread).
pub fn explain(e: &ureq::Error, host: &str) -> String {
    explain_with(e, host, || PROBE.to_socket_addrs().is_ok())
}

fn explain_with(e: &ureq::Error, host: &str, online: impl FnOnce() -> bool) -> String {
    use ureq::Error as E;
    if is_lookup_failure(e) {
        return if online() {
            format!("{host} can't be found (its address doesn't resolve) · check the address")
        } else {
            format!("can't look up {host}: you seem to be offline · check your connection")
        };
    }
    let kind = match e {
        E::Io(io) => Some(io.kind()),
        _ => None,
    };
    match (e, kind) {
        (E::Timeout(_), _) | (_, Some(ErrorKind::TimedOut)) => {
            format!("{host} didn't answer in time · slow connection or server")
        }
        (E::ConnectionFailed, _) | (_, Some(ErrorKind::ConnectionRefused)) => {
            format!("{host} refused the connection · the server may be down")
        }
        (
            _,
            Some(
                ErrorKind::NetworkUnreachable
                | ErrorKind::HostUnreachable
                | ErrorKind::NetworkDown
                | ErrorKind::AddrNotAvailable,
            ),
        ) => format!("no route to {host}: you seem to be offline · check your connection"),
        (
            _,
            Some(
                ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::BrokenPipe
                | ErrorKind::UnexpectedEof,
            ),
        ) => format!("the connection to {host} dropped · try again"),
        (E::Tls(_) | E::Rustls(_) | E::Pem(_), _) => {
            format!("secure connection to {host} failed: {e}")
        }
        (E::BadUri(_), _) => format!("`{host}` isn't a valid address · check the config"),
        _ => format!("couldn't reach {host}: {e}"),
    }
}

/// The OS couldn't turn the name into an address. ureq reports most of
/// these as plain I/O errors whose text depends on the platform.
fn is_lookup_failure(e: &ureq::Error) -> bool {
    const SIGNS: &[&str] = &[
        "lookup address",
        "nodename nor servname",
        "name or service not known",
        "name resolution",
        "no address associated",
        "no such host",
    ];
    match e {
        ureq::Error::HostNotFound => true,
        ureq::Error::Io(io) => {
            let text = io.to_string().to_lowercase();
            SIGNS.iter().any(|s| text.contains(s))
        }
        _ => false,
    }
}

/// The host part of a URL (`https://host:port/path` → `host`).
pub fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    match host.rsplit_once(':') {
        Some((h, port)) if port.chars().all(|c| c.is_ascii_digit()) => h,
        _ => host,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn io_err(kind: ErrorKind, msg: &str) -> ureq::Error {
        ureq::Error::Io(io::Error::new(kind, msg))
    }

    #[test]
    fn lookup_failures_tell_offline_from_a_missing_name() {
        // The macOS text from the bug report.
        let mac = io_err(
            ErrorKind::Other,
            "failed to lookup address information: nodename nor servname provided, or not known",
        );
        let host = "api.example.com";
        let offline = explain_with(&mac, host, || false);
        assert!(offline.contains("offline"), "{offline}");
        let missing = explain_with(&mac, host, || true);
        assert!(missing.contains("can't be found"), "{missing}");
        let linux = io_err(ErrorKind::Other, "Name or service not known");
        assert!(explain_with(&linux, host, || false).contains("offline"));
        assert!(explain_with(&ureq::Error::HostNotFound, host, || false).contains("offline"));
    }

    #[test]
    fn other_failures_say_what_happened() {
        let h = "api.example.com";
        let say = |e: ureq::Error| explain_with(&e, h, || panic!("no probe needed"));
        assert!(say(io_err(ErrorKind::ConnectionRefused, "refused")).contains("may be down"));
        assert!(say(ureq::Error::ConnectionFailed).contains("may be down"));
        assert!(say(io_err(ErrorKind::TimedOut, "t")).contains("didn't answer"));
        assert!(say(io_err(ErrorKind::ConnectionReset, "r")).contains("dropped"));
        assert!(say(io_err(ErrorKind::NetworkUnreachable, "n")).contains("offline"));
        assert!(say(ureq::Error::BadUri("x".into())).contains("valid address"));
    }

    #[test]
    fn hosts_come_out_of_urls() {
        assert_eq!(host_of("https://api.example.com"), "api.example.com");
        assert_eq!(host_of("http://127.0.0.1:8099/x?y"), "127.0.0.1");
        assert_eq!(host_of("example.com/path"), "example.com");
    }
}

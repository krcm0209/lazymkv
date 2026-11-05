//! The transport the clients reach the network through (design part 8.1). The trait takes a URL and its headers and
//! returns the status and the body bytes, so the lookups, the transport that replays recorded answers, the live
//! transport of pull request 4 (`ureq`) and pull request 3's `OfflineTransport` meet at one signature. Nothing here
//! opens a socket. Used by Tasks 16, 22, 23 and 26.

/// One HTTP answer, whatever its status: the status code and the body bytes. Design part 8.1. Def: T15. Use: T16.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Why a request produced no HTTP answer at all. Design part 8.1. Def: T15. Use: T16.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// No answer came within [`TIMEOUT_S`] seconds.
    Timeout,
    /// The service could not be reached (a refused connection, a failed name lookup or a failed TLS handshake); the
    /// text says which.
    Unreachable(String),
    /// Only the recorded transport gives this: no answer is recorded for this URL.
    Unrecorded(String),
}

/// A blocking HTTP transport shared between the engine's threads (design part 8.1). Every HTTP status, a 3xx
/// included, is `Ok(Response)`; reading it is the caller's job (Ruling T4-R12: no redirect is followed). Errors never
/// hold a request header or, for `post` and `connect`, the URL (design part 8.6: a webhook URL is a secret), except
/// the recorded transport's `Unrecorded`, which the notifier maps to fixed words (Task 12). Def: T15. Use: T16, T4-6,
/// T4-7, T4-12, T4-13.
pub trait HttpTransport: Send + Sync {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, TransportError>;
    /// One POST of `body` with `headers` (design part 8.6). Def: T4-6. Use: T4-7, T4-12.
    fn post(&self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Response, TransportError>;
    /// A TCP connection to `host:port` within the timeout, then closed; no TLS handshake and no request (design part
    /// 8.6, "opens a connection to each URL's host without posting"; Ruling T4-R14). Def: T4-6. Use: T4-7, T4-13.
    fn connect(&self, host: &str, port: u16) -> Result<(), TransportError>;
}

/// Every request times out after this many seconds, as spec 12's webhook does. Design part 8.1. Def: T15. Use: T16.
pub const TIMEOUT_S: u64 = 10;

/// The `User-Agent` every request carries: `lazymkv/` and the crate's version. Design part 8.1. Def: T15. Use: T16,
/// T26.
pub fn user_agent() -> String {
    format!("lazymkv/{}", env!("CARGO_PKG_VERSION"))
}

/// Where a webhook URL connects: its scheme, host and port, nothing else (design part 8.6; Ruling T4-R14).
/// Def: T4-6. Use: T4-12, T4-13.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookTarget {
    pub https: bool,
    pub host: String,
    pub port: u16,
}

/// A webhook URL's target (design part 8.6). The scheme is `http` (port 80) or `https` (443), ASCII case ignored;
/// the host is read as the engine's `secret_host` reads it (the authority up to `/`, `?` or `#`, any `user@` dropped,
/// a bracketed IPv6 address unbracketed); an explicit port wins. None for any other scheme, an empty host, whitespace
/// anywhere in the URL, or a port that is empty, not a number or not in 1..=65535. Def: T4-6. Use: T4-12, T4-13.
pub fn webhook_target(url: &str) -> Option<WebhookTarget> {
    if url.contains(char::is_whitespace) {
        return None;
    }
    let (scheme, rest) = url.split_once("://")?;
    let https = if scheme.eq_ignore_ascii_case("https") {
        true
    } else if scheme.eq_ignore_ascii_case("http") {
        false
    } else {
        return None;
    };
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let (host, port_text) = match host_port.strip_prefix('[') {
        Some(bracketed) => {
            let (host, after) = bracketed.split_once(']')?;
            match after {
                "" => (host, None),
                _ => (host, Some(after.strip_prefix(':')?)),
            }
        }
        None => match host_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_port, None),
        },
    };
    if host.is_empty() {
        return None;
    }
    let port = match port_text {
        None => {
            if https {
                443
            } else {
                80
            }
        }
        Some(text) => match text.parse::<u16>() {
            Ok(0) | Err(_) => return None,
            Ok(port) => port,
        },
    };
    Some(WebhookTarget { https, host: host.to_string(), port })
}

/// The reason `OfflineTransport` gives (Ruling T4-R16). Def: T3-21, T4-6. Use: T4-12, T4-17.
pub const OFFLINE: &str = "the offline transport asks nothing (a test double)";

/// A test double that opens no socket: `get`, `post` and `connect` all give `Unreachable(OFFLINE)` (Ruling T4-R16;
/// the binary passes `UreqTransport` from Task 17). Def: T3-21, T4-6. Use: T4-12, T4-17.
pub struct OfflineTransport;

impl HttpTransport for OfflineTransport {
    fn get(&self, _url: &str, _headers: &[(&str, &str)]) -> Result<Response, TransportError> {
        Err(TransportError::Unreachable(OFFLINE.to_string()))
    }

    fn post(&self, _url: &str, _headers: &[(&str, &str)], _body: &[u8]) -> Result<Response, TransportError> {
        Err(TransportError::Unreachable(OFFLINE.to_string()))
    }

    fn connect(&self, _host: &str, _port: u16) -> Result<(), TransportError> {
        Err(TransportError::Unreachable(OFFLINE.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_agent_names_lazymkv_and_its_version() {
        assert_eq!(user_agent(), format!("lazymkv/{}", env!("CARGO_PKG_VERSION")));
        assert_eq!(TIMEOUT_S, 10);
    }

    #[test]
    fn webhook_target_reads_scheme_host_and_port() {
        let target = |https, host: &str, port| Some(WebhookTarget { https, host: host.to_string(), port });
        assert_eq!(webhook_target("http://ha.local:8123/api/webhook/x"), target(false, "ha.local", 8123));
        assert_eq!(webhook_target("https://h/x"), target(true, "h", 443));
        assert_eq!(webhook_target("http://h"), target(false, "h", 80));
        assert_eq!(webhook_target("HTTP://H/"), target(false, "H", 80));
        assert_eq!(webhook_target("http://[::1]:8123/"), target(false, "::1", 8123));
        assert_eq!(webhook_target("https://[fe80::1]/x"), target(true, "fe80::1", 443));
        for refused in ["ftp://h/", "http:///x", "not a url", "h:8123", "", "http://h:0/", "http://h:70000/",
                        "http://h:/", "http://h:80x/", "http://h /x"] {
            assert_eq!(webhook_target(refused), None, "{refused}");
        }
    }

    /// Review Focus 3: userinfo, a port, a path, a query and a fragment; only the host and port come out.
    #[test]
    fn a_webhook_url_with_userinfo_path_and_query_targets_only_its_host_and_port() {
        let url = "http://user-sentinel-7c1e:pass-sentinel-4b9d@ha.local:8123/api/webhook/hook-sentinel-7f3a?x=1#f";
        let target = webhook_target(url).unwrap();
        assert_eq!(target, WebhookTarget { https: false, host: "ha.local".to_string(), port: 8123 });
        let shown = format!("{target:?}");
        for secret in ["user-sentinel-7c1e", "pass-sentinel-4b9d", "hook-sentinel-7f3a", "x=1", "/api/webhook"] {
            assert!(!shown.contains(secret), "{shown}");
        }
    }

    /// Ruling T4-R16: the test double answers every operation unreachable with its one sentence.
    #[test]
    fn the_offline_transport_answers_get_post_and_connect_with_its_sentence() {
        assert_eq!(OFFLINE, "the offline transport asks nothing (a test double)");
        let offline = Err(TransportError::Unreachable(OFFLINE.to_string()));
        for url in ["https://thediscdb.com/api/dischash/ABC", "https://api.themoviedb.org/3/movie/900001", "http://x", ""] {
            assert_eq!(OfflineTransport.get(url, &[("User-Agent", "lazymkv/test")]), offline, "{url}");
            assert_eq!(OfflineTransport.post(url, &[("Content-Type", "application/json")], b"{}"), offline, "{url}");
        }
        assert_eq!(OfflineTransport.connect("ha.local", 8123), Err(TransportError::Unreachable(OFFLINE.to_string())));
    }
}

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

/// A blocking HTTP GET. Implementations are shared between the engine's threads, so they take `&self`. The headers are
/// name and value pairs. A status other than 200 is still an `Ok(Response)`: reading the status is the caller's job.
/// Design part 8.1. Def: T15. Use: T16, T22, T26.
pub trait HttpTransport: Send + Sync {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, TransportError>;
}

/// Every request times out after this many seconds, as spec 12's webhook does. Design part 8.1. Def: T15. Use: T16.
pub const TIMEOUT_S: u64 = 10;

/// The `User-Agent` every request carries: `lazymkv/` and the crate's version. Design part 8.1. Def: T15. Use: T16,
/// T26.
pub fn user_agent() -> String {
    format!("lazymkv/{}", env!("CARGO_PKG_VERSION"))
}

/// The reason `OfflineTransport` gives for every request (Ruling W26). Def: T3-21. Use: T3-21, T3-23.
pub const OFFLINE: &str = "no live transport in this build; it arrives in pull request 4";

/// The transport of pull request 3's engine binary (Ruling W26): it opens no socket and answers every URL
/// `Unreachable(OFFLINE)`, so every lookup stays pending and a job waits in AwaitingLookup until pull request 4's
/// `ureq` transport (design parts 8.1, 8.3, 12.6 item 4). Def: T3-21. Use: T3-21, T3-23.
pub struct OfflineTransport;

impl HttpTransport for OfflineTransport {
    fn get(&self, _url: &str, _headers: &[(&str, &str)]) -> Result<Response, TransportError> {
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

    /// Ruling W26: the binary's transport answers every request unreachable with one sentence, whatever its URL or
    /// headers, so a lookup reads it as pending (design part 8.3).
    #[test]
    fn the_offline_transport_answers_every_url_unreachable() {
        assert_eq!(OFFLINE, "no live transport in this build; it arrives in pull request 4");
        let urls = ["https://thediscdb.com/api/dischash/ABC", "https://api.themoviedb.org/3/movie/900001", "http://x", ""];
        for url in urls {
            let answer = OfflineTransport.get(url, &[("User-Agent", "lazymkv/test")]);
            assert_eq!(answer, Err(TransportError::Unreachable(OFFLINE.to_string())), "{url}");
        }
    }
}

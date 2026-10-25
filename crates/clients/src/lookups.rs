//! The single lookups (design parts 8.1 and 8.3): one request to TheDiscDB (by fingerprint or by disc ID) or to TMDb (a
//! search, or a movie or tv record), mapped to an [`Outcome`] by the HTTP status. The clients only move bytes: the
//! `core::lookup` parsers read every answer, and the URL stands in for the file name in their error sentences. TMDb
//! requests carry the bearer token, and a missing token asks nothing (S65). The chain that orders these lookups is
//! Task 16's. Used by Tasks 16, 23, 26 and 29.

use lazymkv_core::lookup::discdb::{parse_dischash, parse_discid, DiscDbRecord};
use lazymkv_core::lookup::tmdb::{parse_movie, parse_search, parse_tv, TmdbIdentity, TmdbKind, TmdbRecord, TmdbSearch};

use crate::transport::{user_agent, HttpTransport, TransportError};

/// TheDiscDB's API root. Design part 8.1. Def: T15. Use: T16.
pub const DISCDB: &str = "https://thediscdb.com/api";

/// TMDb's v3 API root. Design part 8.1. Def: T15. Use: T16, T26.
pub const TMDB: &str = "https://api.themoviedb.org/3";

/// TheDiscDB's lookup by content hash: `{DISCDB}/dischash/` and the fingerprint in upper case. Design part 8.1.
/// Def: T15. Use: T16, T23, T29.
pub fn dischash_url(fingerprint: &str) -> String {
    format!("{DISCDB}/dischash/{}", fingerprint.to_ascii_uppercase())
}

/// TheDiscDB's lookup by disc ID: `{DISCDB}/discid/` and the ID as given. Design part 8.1. Def: T15. Use: T16, T29.
pub fn discid_url(disc_id: &str) -> String {
    format!("{DISCDB}/discid/{disc_id}")
}

/// TMDb's `/search/multi` with the first page only and `query` as the one parameter (S20), percent-encoded: every UTF-8
/// byte except `A-Z a-z 0-9 - . _ ~` becomes `%XX` in upper-case hex (S67). Design part 8.2. Def: T15. Use: T16, T23,
/// T29.
pub fn search_url(query: &str) -> String {
    let mut encoded = String::with_capacity(query.len());
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => encoded.push(char::from(byte)),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    format!("{TMDB}/search/multi?query={encoded}")
}

/// TMDb's record of a movie or a tv show, with its external IDs appended. Design part 8.2. Def: T15. Use: T16, T23,
/// T29.
pub fn record_url(id: TmdbIdentity) -> String {
    format!("{TMDB}/{}/{}?append_to_response=external_ids", id.kind.as_str(), id.id)
}

/// The call that checks a token: it passes on HTTP 200. Design part 8.1. Def: T15. Use: T26.
pub fn authentication_url() -> String {
    format!("{TMDB}/authentication")
}

/// What was fetched: the URL, the HTTP status and the body as it arrived. Design part 8.4. Def: T15. Use: T16.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetched {
    pub url: String,
    pub status: u16,
    pub body: Vec<u8>,
}

/// Why a lookup stays pending. Design part 8.3. Def: T15. Use: T16, T24.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingReason {
    /// TMDb was not asked, because no token is set.
    NoToken,
    /// TMDb answered 401: it refused the token.
    TokenRefused,
    Timeout,
    /// No answer came; the text says why. A request with no recorded answer reads
    /// `no recorded answer for <url>` (S65).
    Unreachable(String),
    /// An HTTP status that is neither 200 nor 404 (TMDb's 401 is `TokenRefused`, TheDiscDB's is `Status(401)`).
    Status(u16),
    /// A 200 whose body the parser refused, or that is not UTF-8: the sentence names the URL.
    Unparsed(String),
}

/// The result of one lookup. `Answered` always carries a record (S46): TheDiscDB's 404, and a 200 that the parser reads
/// as no record, are both `Miss`. Design part 8.3. Def: T15. Use: T16.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome<T> {
    Answered { value: T, fetched: Fetched },
    /// The service has no such record: an HTTP 404, or a 200 read as none.
    Miss { fetched: Fetched },
    /// No usable answer yet; `fetched` holds the answer that came, when one did.
    Pending { reason: PendingReason, fetched: Option<Fetched> },
}

/// Sends the request and keeps the answer, whatever its status. A request with no recorded answer reads as unreachable,
/// because `PendingReason` has no recorded-only variant and design part 8.3 makes every missing answer pending (S65).
fn fetch(t: &dyn HttpTransport, url: &str, headers: &[(&str, &str)]) -> Result<Fetched, PendingReason> {
    match t.get(url, headers) {
        Ok(response) => Ok(Fetched { url: url.to_string(), status: response.status, body: response.body }),
        Err(TransportError::Timeout) => Err(PendingReason::Timeout),
        Err(TransportError::Unreachable(why)) => Err(PendingReason::Unreachable(why)),
        Err(TransportError::Unrecorded(url)) => {
            Err(PendingReason::Unreachable(format!("no recorded answer for {url}")))
        }
    }
}

/// One request and its mapping to an outcome: 404 is a miss; a TMDb 401 is a refused token; any other status but 200
/// is pending with that status; a 200 body is read by `parse` (given the body as text and the URL), where `Ok(None)` is
/// a miss and an error is the sentence of an unparsed answer (S65).
fn lookup<T>(
    t: &dyn HttpTransport,
    url: String,
    headers: &[(&str, &str)],
    tmdb: bool,
    parse: impl FnOnce(&str, &str) -> Result<Option<T>, String>,
) -> Outcome<T> {
    let fetched = match fetch(t, &url, headers) {
        Ok(fetched) => fetched,
        Err(reason) => return Outcome::Pending { reason, fetched: None },
    };
    match fetched.status {
        404 => Outcome::Miss { fetched },
        401 if tmdb => Outcome::Pending { reason: PendingReason::TokenRefused, fetched: Some(fetched) },
        200 => {
            let parsed = match std::str::from_utf8(&fetched.body) {
                Ok(text) => parse(text, &url),
                Err(_) => Err(format!("{url}: the body is not UTF-8")),
            };
            match parsed {
                Ok(Some(value)) => Outcome::Answered { value, fetched },
                Ok(None) => Outcome::Miss { fetched },
                Err(sentence) => Outcome::Pending { reason: PendingReason::Unparsed(sentence), fetched: Some(fetched) },
            }
        }
        other => Outcome::Pending { reason: PendingReason::Status(other), fetched: Some(fetched) },
    }
}

/// TheDiscDB needs no credential, so its requests carry the `User-Agent` alone.
fn discdb<T>(
    t: &dyn HttpTransport,
    url: String,
    parse: impl FnOnce(&str, &str) -> Result<Option<T>, String>,
) -> Outcome<T> {
    lookup(t, url, &[("User-Agent", &user_agent())], false, parse)
}

/// TMDb's requests carry the bearer token and the `User-Agent`; with no token nothing is asked (S65).
fn tmdb<T>(
    t: &dyn HttpTransport,
    token: Option<&str>,
    url: String,
    parse: impl FnOnce(&str, &str) -> Result<Option<T>, String>,
) -> Outcome<T> {
    let Some(token) = token else {
        return Outcome::Pending { reason: PendingReason::NoToken, fetched: None };
    };
    lookup(t, url, &[("Authorization", &format!("Bearer {token}")), ("User-Agent", &user_agent())], true, parse)
}

/// TheDiscDB's record for a disc fingerprint. A 404, and a 200 that `parse_dischash` reads as no record (an empty
/// body), are both `Miss` (S46). Design part 8.1. Def: T15. Use: T16.
pub fn discdb_by_fingerprint(t: &dyn HttpTransport, fingerprint: &str) -> Outcome<DiscDbRecord> {
    discdb(t, dischash_url(fingerprint), parse_dischash)
}

/// TheDiscDB's records for a disc ID. A 200 holding `[]` is `Answered` with an empty list, as `parse_discid` reads it
/// (S65). Design part 8.1. Def: T15. Use: T16.
pub fn discdb_by_disc_id(t: &dyn HttpTransport, disc_id: &str) -> Outcome<Vec<DiscDbRecord>> {
    discdb(t, discid_url(disc_id), |text, shown| parse_discid(text, shown).map(Some))
}

/// TMDb's first page of results for `query`. Design part 8.2. Def: T15. Use: T16.
pub fn tmdb_search(t: &dyn HttpTransport, token: Option<&str>, query: &str) -> Outcome<TmdbSearch> {
    tmdb(t, token, search_url(query), |text, shown| parse_search(text, shown).map(Some))
}

/// TMDb's record for `id`, read by its kind. Design part 8.2. Def: T15. Use: T16.
pub fn tmdb_record(t: &dyn HttpTransport, token: Option<&str>, id: TmdbIdentity) -> Outcome<TmdbRecord> {
    let parse = match id.kind {
        TmdbKind::Movie => parse_movie,
        TmdbKind::Tv => parse_tv,
    };
    tmdb(t, token, record_url(id), |text, shown| parse(text, shown).map(Some))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorded::RecordedTransport;
    use crate::transport::{user_agent, TransportError};
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};

    const FP: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";
    const MOVIE: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Movie, id: 900001 };
    const MOVIE_BODY: &[u8] = br#"{"id":900001,"imdb_id":"tt0900001","title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03","original_language":"en","runtime":119,"external_ids":{"imdb_id":"tt0900001"}}"#;

    #[test]
    fn a_404_dischash_is_a_miss() {
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 404, b"");
        assert_eq!(discdb_by_fingerprint(&t, FP),
            Outcome::Miss { fetched: Fetched { url: dischash_url(FP), status: 404, body: vec![] } });
    }

    #[test]
    fn a_dischash_200_with_an_empty_body_is_a_miss() {
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 200, b"");
        assert!(matches!(discdb_by_fingerprint(&t, FP), Outcome::Miss { fetched } if fetched.status == 200));
    }

    #[test]
    fn a_dischash_answer_parses() {
        let body = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/thediscdb-api/dischash-76E63EBB5B66F73B0C37BFAB59B8ABA5.json")).unwrap();
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 200, &body);
        match discdb_by_fingerprint(&t, FP) {
            Outcome::Answered { value, fetched } => {
                assert_eq!(value.tmdb_identity(), Some(TmdbIdentity { kind: TmdbKind::Movie, id: 245891 }));
                assert_eq!(fetched.body, body);
            }
            other => panic!("expected Answered, got {other:?}"),
        }
    }

    #[test]
    fn the_dischash_url_uses_the_upper_case_fingerprint() {
        assert_eq!(dischash_url("76e63ebb5b66f73b0c37bfab59b8aba5"),
            "https://thediscdb.com/api/dischash/76E63EBB5B66F73B0C37BFAB59B8ABA5");
        assert_eq!(discid_url("D1"), "https://thediscdb.com/api/discid/D1");
        assert_eq!(record_url(MOVIE), "https://api.themoviedb.org/3/movie/900001?append_to_response=external_ids");
        assert_eq!(record_url(TmdbIdentity { kind: TmdbKind::Tv, id: 5 }),
            "https://api.themoviedb.org/3/tv/5?append_to_response=external_ids");
        assert_eq!(authentication_url(), "https://api.themoviedb.org/3/authentication");
        assert_eq!(search_url("Pan's Labyrinth: \u{d1}"),
            "https://api.themoviedb.org/3/search/multi?query=Pan%27s%20Labyrinth%3A%20%C3%91");
    }

    #[test]
    fn no_token_makes_no_tmdb_request() {
        let t = RecordedTransport::new();
        assert_eq!(tmdb_search(&t, None, "SYNTHETIC MOVIE"), Outcome::Pending { reason: PendingReason::NoToken, fetched: None });
        assert_eq!(tmdb_record(&t, None, MOVIE), Outcome::Pending { reason: PendingReason::NoToken, fetched: None });
        assert!(t.requests().is_empty());
        assert!(t.unrecorded().is_empty());
    }

    #[test]
    fn a_401_is_pending_with_token_refused() {
        let t = RecordedTransport::new();
        t.answer(&record_url(MOVIE), 401, b"{\"status_code\":7}");
        assert!(matches!(tmdb_record(&t, Some("bad"), MOVIE),
            Outcome::Pending { reason: PendingReason::TokenRefused, fetched: Some(f) } if f.status == 401));
        t.answer(&dischash_url(FP), 401, b"");
        assert!(matches!(discdb_by_fingerprint(&t, FP), Outcome::Pending { reason: PendingReason::Status(401), .. }));
    }

    #[test]
    fn a_200_that_does_not_parse_is_pending_with_the_parsers_sentence() {
        let t = RecordedTransport::new();
        t.answer(&search_url("X"), 200, b"{\"results\":5}");
        match tmdb_search(&t, Some("token"), "X") {
            Outcome::Pending { reason: PendingReason::Unparsed(sentence), fetched: Some(fetched) } => {
                assert!(sentence.starts_with(&format!("{}: ", search_url("X"))), "{sentence}");
                assert_eq!(fetched.status, 200);
            }
            other => panic!("expected Pending Unparsed, got {other:?}"),
        }
        t.fail(&record_url(MOVIE), TransportError::Timeout);
        assert_eq!(tmdb_record(&t, Some("token"), MOVIE), Outcome::Pending { reason: PendingReason::Timeout, fetched: None });
    }

    #[test]
    fn a_movie_record_parses_by_its_kind() {
        let t = RecordedTransport::new();
        t.answer(&record_url(MOVIE), 200, MOVIE_BODY);
        match tmdb_record(&t, Some("token"), MOVIE) {
            Outcome::Answered { value, .. } => {
                assert_eq!(value.title, "Synthetic Movie");
                assert_eq!(value.runtime, Some(119));
                assert_eq!(value.year, Some(2001));
            }
            other => panic!("expected Answered, got {other:?}"),
        }
    }

    #[test]
    fn every_request_carries_the_user_agent_and_tmdb_the_bearer_token() {
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 404, b"");
        t.answer(&record_url(MOVIE), 200, MOVIE_BODY);
        discdb_by_fingerprint(&t, FP);
        tmdb_record(&t, Some("test-token"), MOVIE);
        let requests = t.requests();
        let ua = ("User-Agent".to_string(), user_agent());
        assert_eq!(requests[0], (dischash_url(FP), vec![ua.clone()]));
        assert_eq!(requests[1], (record_url(MOVIE), vec![("Authorization".to_string(), "Bearer test-token".to_string()), ua]));
    }
}

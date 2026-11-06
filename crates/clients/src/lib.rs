//! lazymkv-clients: TheDiscDB and TMDb (The Movie Database) behind an HTTP transport trait, with a transport that replays recorded answers (design parts 4.1, 8.1). Two test doubles, `RecordedTransport` and `OfflineTransport`, stand in for the live transport.

pub mod chain;
pub mod live;
pub mod lookups;
mod merge;
pub mod recorded;
pub mod refetch;
pub mod saved;
pub mod transport;

pub use chain::{run_round, seasons_to_fetch, status_file, Answers, ChainInputs, Lookup, LookupBook, LookupState};
pub use live::{
    UreqTransport, BAD_ANSWER, BODY_TOO_LONG, CONNECTION_FAILED, MAX_BODY_BYTES, NO_HOST, REFUSED, TLS_FAILED,
};
pub use lookups::{
    authentication_url, discdb_by_disc_id, discdb_by_fingerprint, dischash_url, discid_url, record_url, search_url,
    reason_text, season_url, tmdb_record, tmdb_search, tmdb_season, Fetched, Outcome, PendingReason, DISCDB, TMDB,
};
pub use merge::merge_pages;
pub use recorded::{Gate, RecordedTransport};
pub use refetch::{
    refetch_record, refetch_search, refetch_seasons, stale_copies, status_record, RecordRefetch, Seasons,
    SeasonsFetched, StaleCopy,
};
pub use saved::{json_file, shape_of, slim_record, slim_search, slim_tmdb, splice_season, status_text, SavedFile};
pub use transport::{
    user_agent, webhook_target, HttpTransport, OfflineTransport, Response, TransportError, WebhookTarget, OFFLINE,
    TIMEOUT_S,
};

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Each `[[package]]` of the workspace's Cargo.lock as (name, version).
    fn locked_packages() -> Vec<(String, String)> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut packages = Vec::new();
        let mut name = None;
        for line in text.lines() {
            if line == "[[package]]" {
                name = None;
            } else if let Some(value) = line.strip_prefix("name = ") {
                name = Some(value.trim_matches('"').to_string());
            } else if let Some(value) = line.strip_prefix("version = ") {
                if let Some(name) = name.take() {
                    packages.push((name, value.trim_matches('"').to_string()));
                }
            }
        }
        packages
    }

    #[test]
    fn the_lock_holds_ring_and_no_other_tls_provider() {
        let names: Vec<String> = locked_packages().into_iter().map(|(name, _)| name).collect();
        for wanted in ["ureq", "rustls", "ring", "webpki-roots"] {
            assert!(names.iter().any(|name| name == wanted), "Cargo.lock lacks {wanted}");
        }
        for refused in [
            "aws-lc-rs", "aws-lc-sys", "openssl", "openssl-sys", "native-tls", "rustls-platform-verifier", "cmake",
            "tokio", "reqwest",
        ] {
            assert!(!names.iter().any(|name| name == refused), "Cargo.lock holds {refused} (design part 8.1)");
        }
    }

    #[test]
    fn webpki_roots_is_a_1_x_release() {
        let versions: Vec<String> =
            locked_packages().into_iter().filter(|(name, _)| name == "webpki-roots").map(|(_, version)| version).collect();
        assert!(!versions.is_empty(), "Cargo.lock lacks webpki-roots");
        for version in versions {
            assert!(version.starts_with("1."), "webpki-roots {version} is not 1.x, the CDLA-Permissive-2.0 line (design part 4.7)");
        }
    }
}

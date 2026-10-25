//! lazymkv-clients: TheDiscDB and TMDb (The Movie Database) behind an HTTP transport trait, with a transport that replays recorded answers (design parts 4.1, 8.1). The live transport arrives in pull request 4.

pub mod lookups;
pub mod recorded;
pub mod saved;
pub mod transport;

pub use lookups::{
    authentication_url, discdb_by_disc_id, discdb_by_fingerprint, dischash_url, discid_url, record_url, search_url,
    tmdb_record, tmdb_search, Fetched, Outcome, PendingReason, DISCDB, TMDB,
};
pub use recorded::{Gate, RecordedTransport};
pub use saved::{slim_record, slim_search, slim_tmdb, status_text, SavedFile};
pub use transport::{user_agent, HttpTransport, Response, TransportError, TIMEOUT_S};

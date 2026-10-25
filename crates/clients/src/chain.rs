//! The movie lookup chain (design parts 8.2 and 8.3): TheDiscDB by fingerprint, TheDiscDB by disc ID, TMDb's search
//! on the disc's own name, then the TMDb record that `core::lookup::record_to_fetch` names for the answers in hand. A
//! round asks every lookup that is not answered yet, in that order, and saves each answer and `lookup-status.txt` as
//! they come; a later round asks again what stayed pending. Pull request 2 runs the movie path only (S19), with no
//! entry-title search (S47). Used by Tasks 23, 24, 25 and 26.

use std::collections::BTreeMap;

use lazymkv_core::lookup::discdb::DiscDbRecord;
use lazymkv_core::lookup::name::DiscName;
use lazymkv_core::lookup::tmdb::{TmdbRecord, TmdbSearch};
use lazymkv_core::lookup::{record_to_fetch, LookupInputs};
use serde_json::Value;

use crate::lookups::{
    discdb_by_disc_id, discdb_by_fingerprint, tmdb_record, tmdb_search, Fetched, Outcome, PendingReason,
};
use crate::saved::{slim_record, slim_search, slim_tmdb, status_text, SavedFile};
use crate::transport::HttpTransport;

/// One request of the movie chain (design part 8.2 items 1 to 4). Def: T16. Use: T23, T24, T25.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lookup {
    Dischash,
    Discid,
    Search,
    Record,
}

/// Where one lookup stands (design part 8.3). Def: T16. Use: T23, T24, T25.
#[derive(Clone, Debug, PartialEq)]
pub enum LookupState {
    /// Part of this disc's chain, but not asked yet.
    NotAsked,
    /// The request is out and no answer has come.
    InFlight,
    /// The service answered, with a record or with a miss: the HTTP status.
    Answered { status: u16 },
    /// No usable answer: the next round asks again.
    Pending { reason: PendingReason },
}

/// The answers in hand, parsed by core. Def: T16. Use: T23, T24.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answers {
    pub by_fingerprint: Option<DiscDbRecord>,
    pub by_disc_id: Vec<DiscDbRecord>,
    pub search: Option<TmdbSearch>,
    pub record: Option<TmdbRecord>,
}

/// A job's lookups. A lookup absent from `states` is not part of this disc's chain (a DVD's discid, a generic name's
/// search, a record with no identity to fetch). Def: T16. Use: T23, T24, T25.
#[derive(Clone, Debug, Default)]
pub struct LookupBook {
    pub states: BTreeMap<Lookup, LookupState>,
    pub answers: Answers,
}

/// What the chain asks with (design part 8.2). Def: T16. Use: T23.
#[derive(Clone, Debug)]
pub struct ChainInputs {
    pub fingerprint: String,
    pub disc_id: Option<String>,
    pub name: Option<DiscName>,
}

impl LookupBook {
    /// (discdb_pending, tmdb_pending) as `LookupInputs` takes them (design part 8.3). Def: T16. Use: T23, T24.
    pub fn pending(&self) -> (bool, bool) {
        let present = |lookup: Lookup| self.states.contains_key(&lookup);
        let answered = |lookup: Lookup| matches!(self.states.get(&lookup), Some(LookupState::Answered { .. }));
        let waiting = |lookup: Lookup| present(lookup) && !answered(lookup);
        // No Dischash state means no round has started, so both lookups read as pending.
        let discdb = !answered(Lookup::Dischash) || waiting(Lookup::Discid);
        let tmdb = !present(Lookup::Dischash) || waiting(Lookup::Search) || waiting(Lookup::Record);
        (discdb, tmdb)
    }

    /// The book as core's `LookupInputs`, with the flags of `pending`. Def: T16. Use: T16, T23, T24.
    pub fn lookup_inputs<'a>(&'a self, name: Option<&'a DiscName>) -> LookupInputs<'a> {
        let (discdb_pending, tmdb_pending) = self.pending();
        LookupInputs {
            name,
            // dev's rule (`crates/dev/src/cmd/plan.rs`): the first disc ID record that has titles.
            discdb_by_disc_id: self.answers.by_disc_id.iter().find(|record| !record.titles.is_empty()),
            discdb_by_fingerprint: self.answers.by_fingerprint.as_ref(),
            discdb_pending,
            search: self.answers.search.as_ref(),
            record: self.answers.record.as_ref(),
            tmdb_pending,
            ..LookupInputs::default()
        }
    }

    /// Whether a round asks `lookup`: it is part of the chain and not answered.
    fn asks(&self, lookup: Lookup) -> bool {
        !matches!(self.states.get(&lookup), None | Some(LookupState::Answered { .. }))
    }
}

/// What an outcome does to the answer in hand.
enum Settled<T> {
    /// A record came: it replaces the answer.
    Value(T),
    /// The service has no record: the answer is cleared.
    Cleared,
    /// Pending: the answer in hand stays.
    Kept,
}

impl<T> Settled<T> {
    /// Writes the change into `answer`, where `into` turns a record into the answer and a miss leaves the empty answer.
    fn write<U: Default>(self, answer: &mut U, into: impl FnOnce(T) -> U) {
        match self {
            Settled::Value(value) => *answer = into(value),
            Settled::Cleared => *answer = U::default(),
            Settled::Kept => {}
        }
    }
}

/// Sets `lookup`'s state from `outcome`: `Answered` with the HTTP status for a record or a miss, else `Pending` with
/// its reason. Returns what the outcome does to the answer, and the fetch when an answer came.
fn settle<T>(book: &mut LookupBook, lookup: Lookup, outcome: Outcome<T>) -> (Settled<T>, Option<Fetched>) {
    let (state, settled, fetched) = match outcome {
        Outcome::Answered { value, fetched } => {
            (LookupState::Answered { status: fetched.status }, Settled::Value(value), Some(fetched))
        }
        Outcome::Miss { fetched } => {
            (LookupState::Answered { status: fetched.status }, Settled::Cleared, Some(fetched))
        }
        Outcome::Pending { reason, fetched } => (LookupState::Pending { reason }, Settled::Kept, fetched),
    };
    book.states.insert(lookup, state);
    (settled, fetched)
}

/// The word `lookup-status.txt` gives a state, or None for a lookup not asked yet. dev reads every word but `200` and
/// `404` as failed, so an in-flight request and a pending lookup both read as pending there (S21, S23).
fn status_word(state: &LookupState) -> Option<String> {
    let word = match state {
        LookupState::NotAsked => return None,
        LookupState::InFlight => "000".to_string(),
        LookupState::Answered { status } => status.to_string(),
        LookupState::Pending { reason } => match reason {
            PendingReason::NoToken | PendingReason::Timeout | PendingReason::Unreachable(_) => "000".to_string(),
            PendingReason::TokenRefused => "401".to_string(),
            PendingReason::Status(code) => code.to_string(),
            PendingReason::Unparsed(_) => "unparsed".to_string(),
        },
    };
    Some(word)
}

/// `lookup-status.txt` for the book as it stands: one line per lookup asked. The record's key is `tmdb <kind>/<id>`,
/// named by the record in hand while `Record` is answered with one, else by `record_to_fetch` over the book, else by
/// the record in hand (PF-18); with no identity there is no record line. Design part 8.4.
fn status_file(book: &LookupBook, name: Option<&DiscName>) -> SavedFile {
    let mut lines = BTreeMap::new();
    for (lookup, state) in &book.states {
        let Some(word) = status_word(state) else { continue };
        let key = match lookup {
            Lookup::Dischash => "dischash".to_string(),
            Lookup::Discid => "discid".to_string(),
            Lookup::Search => "search".to_string(),
            Lookup::Record => {
                let in_hand = book.answers.record.as_ref().map(|record| record.identity);
                let identity = match (state, in_hand) {
                    (LookupState::Answered { .. }, Some(identity)) => Some(identity),
                    _ => record_to_fetch(&book.lookup_inputs(name)).or(in_hand),
                };
                let Some(identity) = identity else { continue };
                format!("tmdb {}/{}", identity.kind.as_str(), identity.id)
            }
        };
        lines.insert(key, word);
    }
    SavedFile { name: "lookup-status.txt".to_string(), bytes: status_text(&lines).into_bytes() }
}

/// Sets `lookup` in flight and saves the status file, so a reader sees `000` while the request is out (S23).
fn start(book: &mut LookupBook, lookup: Lookup, name: Option<&DiscName>, save: &mut dyn FnMut(SavedFile)) {
    book.states.insert(lookup, LookupState::InFlight);
    save(status_file(book, name));
}

/// A slim copy as saved: pretty JSON and a final newline.
fn json_file(name: &str, value: &Value) -> SavedFile {
    // Writing a `Value` into memory cannot fail: its map keys are strings and a Vec takes every byte.
    let mut bytes = serde_json::to_vec_pretty(value).unwrap_or_default();
    bytes.push(b'\n');
    SavedFile { name: name.to_string(), bytes }
}

/// The body of a TMDb request that answered with a record, as JSON; None for a miss or a pending lookup, whose bodies
/// are never kept (P29). Core's parser already read an answered body as JSON (after any byte order mark), so an answer
/// always gives Some.
fn answered_json<T>(settled: &Settled<T>, fetched: Option<&Fetched>) -> Option<Value> {
    let (Settled::Value(_), Some(fetched)) = (settled, fetched) else { return None };
    let text = std::str::from_utf8(&fetched.body).ok()?;
    serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)).ok()
}

/// One round of design part 8.3: asks every lookup of the chain that is not answered, in the order of design part 8.2,
/// saving each answer and the status file as they come. Def: T16. Use: T23, T24, T25, T26.
pub fn run_round(
    t: &dyn HttpTransport,
    token: Option<&str>,
    book: &mut LookupBook,
    inputs: &ChainInputs,
    today: &str,
    save: &mut dyn FnMut(SavedFile),
) {
    let name = inputs.name.as_ref();
    let searched = name.filter(|name| !name.generic);
    book.states.entry(Lookup::Dischash).or_insert(LookupState::NotAsked);
    if inputs.disc_id.is_some() {
        book.states.entry(Lookup::Discid).or_insert(LookupState::NotAsked);
    }
    if searched.is_some() {
        book.states.entry(Lookup::Search).or_insert(LookupState::NotAsked);
    }

    if book.asks(Lookup::Dischash) {
        start(book, Lookup::Dischash, name, save);
        let (settled, fetched) = settle(book, Lookup::Dischash, discdb_by_fingerprint(t, &inputs.fingerprint));
        settled.write(&mut book.answers.by_fingerprint, Some);
        if let Some(fetched) = fetched {
            save(SavedFile { name: "thediscdb-hash.json".to_string(), bytes: fetched.body });
        }
        save(status_file(book, name));
    }

    if let Some(disc_id) = inputs.disc_id.as_deref().filter(|_| book.asks(Lookup::Discid)) {
        start(book, Lookup::Discid, name, save);
        let (settled, fetched) = settle(book, Lookup::Discid, discdb_by_disc_id(t, disc_id));
        settled.write(&mut book.answers.by_disc_id, |records| records);
        if let Some(fetched) = fetched {
            save(SavedFile { name: "thediscdb-discid.json".to_string(), bytes: fetched.body });
        }
        save(status_file(book, name));
    }

    if let Some(searched) = searched.filter(|_| book.asks(Lookup::Search)) {
        start(book, Lookup::Search, name, save);
        let (settled, fetched) = settle(book, Lookup::Search, tmdb_search(t, token, &searched.cleaned));
        if let Some(raw) = answered_json(&settled, fetched.as_ref()) {
            save(json_file("tmdb-search.json", &slim_search(&raw, &searched.cleaned, today)));
        }
        settled.write(&mut book.answers.search, Some);
        save(status_file(book, name));
    }

    let Some(want) = record_to_fetch(&book.lookup_inputs(name)) else {
        if book.asks(Lookup::Record) {
            book.states.remove(&Lookup::Record);
        }
        return;
    };
    let answered = matches!(book.states.get(&Lookup::Record), Some(LookupState::Answered { .. }));
    if answered && book.answers.record.as_ref().map(|record| record.identity) == Some(want) {
        return;
    }
    start(book, Lookup::Record, name, save);
    let (settled, fetched) = settle(book, Lookup::Record, tmdb_record(t, token, want));
    if let Some(raw) = answered_json(&settled, fetched.as_ref()) {
        save(json_file("tmdb.json", &slim_tmdb(&raw, today)));
        save(json_file("tmdb-record.json", &slim_record(&raw, want.kind, today)));
    }
    settled.write(&mut book.answers.record, Some);
    save(status_file(book, name));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use lazymkv_core::lookup::discdb::{parse_discid, DiscDbRecord};
    use lazymkv_core::lookup::name::disc_name;
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use serde_json::{json, Value};

    use crate::{
        dischash_url, discid_url, record_url, search_url, Gate, PendingReason, RecordedTransport, SavedFile,
        TransportError,
    };

    /// John Wick's UHD disc, whose TheDiscDB answers are recorded under corpus/thediscdb-api.
    const FP: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";
    const DISC_ID: &str = "8D0355F530C31D44912831F8637FF65ACBCD6886";
    const JOHN_WICK_DISCHASH: &str = "thediscdb-api/dischash-76E63EBB5B66F73B0C37BFAB59B8ABA5.json";
    const JOHN_WICK_DISCID: &str = "thediscdb-api/discid-8D0355F530C31D44912831F8637FF65ACBCD6886.json";
    const JOHN_WICK: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Movie, id: 245891 };
    const TOKEN: &str = "test-token";
    const TODAY: &str = "2026-10-23";

    /// A file under corpus/, as bytes.
    fn corpus(relative: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(relative))
            .unwrap_or_else(|error| panic!("{relative}: {error}"))
    }

    /// A /search/multi first page of movies (id, title, release date) in TMDb's documented shape.
    fn search_body(movies: &[(i64, &str, &str)]) -> Vec<u8> {
        let results: Vec<Value> = movies
            .iter()
            .map(|(id, title, date)| {
                json!({"media_type": "movie", "id": id, "title": title, "original_title": title, "release_date": date})
            })
            .collect();
        json!({"page": 1, "results": results, "total_pages": 1, "total_results": movies.len()}).to_string().into_bytes()
    }

    /// A /movie/{id}?append_to_response=external_ids answer; the IMDb ID is made up.
    fn movie_body(id: i64, title: &str, date: &str) -> Vec<u8> {
        json!({"id": id, "title": title, "original_title": title, "release_date": date, "original_language": "en",
               "runtime": 101, "external_ids": {"imdb_id": format!("tt{id:07}")}})
        .to_string()
        .into_bytes()
    }

    fn movie(id: i64) -> TmdbIdentity {
        TmdbIdentity { kind: TmdbKind::Movie, id }
    }

    /// The chain's inputs for John Wick's fingerprint, an optional disc ID and an optional volume label.
    fn chain(disc_id: Option<&str>, label: Option<&str>) -> ChainInputs {
        ChainInputs {
            fingerprint: FP.to_string(),
            disc_id: disc_id.map(str::to_string),
            name: label.and_then(|label| disc_name(None, Some(label))),
        }
    }

    /// The URL of the disc name's search.
    fn search_of(inputs: &ChainInputs) -> String {
        search_url(&inputs.name.as_ref().unwrap().cleaned)
    }

    /// dischash and discid answer 404 with empty bodies (TheDiscDB's miss).
    fn misses(t: &RecordedTransport) {
        t.answer(&dischash_url(FP), 404, b"");
        t.answer(&discid_url(DISC_ID), 404, b"");
    }

    /// TheDiscDB misses, the name search with one plausible result (John Wick), and John Wick's record.
    fn tmdb_only(t: &RecordedTransport, inputs: &ChainInputs) {
        misses(t);
        t.answer(&search_of(inputs), 200, &search_body(&[(245891, "John Wick", "2014-10-22"), (324552, "John Wick: Chapter 2", "2017-02-08")]));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
    }

    /// One round with the token, returning the files saved, in order.
    fn round(t: &RecordedTransport, book: &mut LookupBook, inputs: &ChainInputs) -> Vec<SavedFile> {
        let mut saved = Vec::new();
        run_round(t, Some(TOKEN), book, inputs, TODAY, &mut |file| saved.push(file));
        saved
    }

    /// Every URL requested so far, in order.
    fn urls(t: &RecordedTransport) -> Vec<String> {
        t.requests().into_iter().map(|(url, _)| url).collect()
    }

    /// The sorted lines of the last lookup-status.txt in `saved`.
    fn last_status(saved: &[SavedFile]) -> Vec<String> {
        let file = saved.iter().rev().find(|file| file.name == "lookup-status.txt").expect("a status file");
        let mut lines: Vec<String> = String::from_utf8(file.bytes.clone()).unwrap().lines().map(str::to_string).collect();
        lines.sort();
        lines
    }

    fn answered(status: u16) -> LookupState {
        LookupState::Answered { status }
    }

    /// Opens its gate when dropped. A failing assertion then releases the held request while the scope's closure
    /// unwinds, so the scope joins a finished round thread instead of hanging on it (PF-19).
    struct OpenOnDrop(Gate);

    impl Drop for OpenOnDrop {
        fn drop(&mut self) {
            self.0.open();
        }
    }

    #[test]
    fn the_chain_asks_dischash_discid_search_then_record() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK"), record_url(JOHN_WICK)]);
        assert_eq!(
            book.states,
            BTreeMap::from([
                (Lookup::Dischash, answered(404)),
                (Lookup::Discid, answered(404)),
                (Lookup::Search, answered(200)),
                (Lookup::Record, answered(200)),
            ])
        );
        assert_eq!(book.pending(), (false, false));
        assert_eq!(book.answers.record.as_ref().map(|record| record.identity), Some(JOHN_WICK));
        assert_eq!(last_status(&saved), ["dischash 404", "discid 404", "search 200", "tmdb movie/245891 200"]);
    }

    #[test]
    fn each_answer_is_saved_when_it_arrives_between_two_status_files() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        let saved = round(&t, &mut LookupBook::default(), &inputs);
        let names: Vec<&str> = saved.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "lookup-status.txt", "thediscdb-hash.json", "lookup-status.txt",
                "lookup-status.txt", "thediscdb-discid.json", "lookup-status.txt",
                "lookup-status.txt", "tmdb-search.json", "lookup-status.txt",
                "lookup-status.txt", "tmdb.json", "tmdb-record.json", "lookup-status.txt",
            ]
        );
        // TheDiscDB's 404 is kept as received: an empty body.
        assert!(saved[1].bytes.is_empty());
        let search: Value = serde_json::from_slice(&saved[7].bytes).unwrap();
        assert_eq!((&search["queries"], &search["fetched_at"]), (&json!(["JOHN WICK"]), &json!(TODAY)));
        let record: Value = serde_json::from_slice(&saved[11].bytes).unwrap();
        assert_eq!((&record["id"], &record["fetched_at"]), (&json!(245891), &json!(TODAY)));
    }

    #[test]
    fn a_dvd_skips_discid() {
        let t = RecordedTransport::new();
        let inputs = chain(None, Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), search_url("JOHN WICK"), record_url(JOHN_WICK)]);
        assert!(!book.states.contains_key(&Lookup::Discid));
        assert_eq!(book.pending(), (false, false));
    }

    #[test]
    fn a_generic_name_is_never_searched() {
        for label in [Some("LOGICAL_VOLUME_ID"), None] {
            let t = RecordedTransport::new();
            misses(&t);
            let mut book = LookupBook::default();
            round(&t, &mut book, &chain(Some(DISC_ID), label));
            assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID)], "{label:?}");
            assert_eq!(book.states.keys().copied().collect::<Vec<Lookup>>(), [Lookup::Dischash, Lookup::Discid]);
            assert_eq!(book.pending(), (false, false));
        }
    }

    #[test]
    fn the_record_comes_from_discdbs_tmdb_id_when_present() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        t.answer(&dischash_url(FP), 200, &corpus(JOHN_WICK_DISCHASH));
        t.answer(&discid_url(DISC_ID), 404, b"");
        // The speculative name search is still asked, and its one plausible result is another ID.
        t.answer(&search_of(&inputs), 200, &search_body(&[(900002, "John Wick", "2099-01-01")]));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK"), record_url(JOHN_WICK)]);
        assert_eq!(book.answers.by_fingerprint.as_ref().and_then(|record| record.tmdb_identity()), Some(JOHN_WICK));
    }

    #[test]
    fn the_record_comes_from_the_one_plausible_search_result_otherwise() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        round(&t, &mut LookupBook::default(), &inputs);
        assert_eq!(urls(&t).last(), Some(&record_url(JOHN_WICK)));
        // Two plausible results: no record is asked, and nothing is left pending.
        let two = RecordedTransport::new();
        misses(&two);
        two.answer(&search_of(&inputs), 200, &search_body(&[(245891, "John Wick", "2014-10-22"), (900003, "John Wick", "1990-01-01")]));
        let mut book = LookupBook::default();
        round(&two, &mut book, &inputs);
        assert_eq!(urls(&two), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK")]);
        assert!(!book.states.contains_key(&Lookup::Record));
        assert_eq!(book.pending(), (false, false));
    }

    #[test]
    fn a_round_restarts_at_the_first_unanswered_step() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        t.fail(&dischash_url(FP), TransportError::Timeout);
        tmdb_only(&t, &inputs);
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(book.states[&Lookup::Dischash], LookupState::Pending { reason: PendingReason::Timeout });
        assert_eq!(book.pending(), (true, false));
        assert_eq!(urls(&t).len(), 4);
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(urls(&t)[4..], [dischash_url(FP)]);
        assert_eq!(book.states[&Lookup::Dischash], answered(404));
        assert_eq!(book.pending(), (false, false));
        assert_eq!(last_status(&saved), ["dischash 404", "discid 404", "search 200", "tmdb movie/245891 200"]);
    }

    #[test]
    fn a_late_thediscdb_answer_leads_to_its_record_within_the_round() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        t.fail(&dischash_url(FP), TransportError::Timeout);
        t.answer(&dischash_url(FP), 200, &corpus(JOHN_WICK_DISCHASH));
        t.answer(&discid_url(DISC_ID), 404, b"");
        t.answer(&search_of(&inputs), 200, &search_body(&[(900004, "John Wick", "2099-01-01")]));
        t.answer(&record_url(movie(900004)), 200, &movie_body(900004, "John Wick", "2099-01-01"));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(book.answers.record.as_ref().map(|record| record.identity), Some(movie(900004)));
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(urls(&t)[4..], [dischash_url(FP), record_url(JOHN_WICK)]);
        assert_eq!(book.answers.record.as_ref().map(|record| record.identity), Some(JOHN_WICK));
        assert_eq!(book.pending(), (false, false));
        assert_eq!(last_status(&saved), ["dischash 200", "discid 404", "search 200", "tmdb movie/245891 200"]);
    }

    #[test]
    fn the_status_line_is_000_while_a_request_is_in_flight() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        tmdb_only(&t, &inputs);
        let gate = t.gate(&dischash_url(FP));
        let saved: Mutex<Vec<SavedFile>> = Mutex::new(Vec::new());
        let mut book = LookupBook::default();
        std::thread::scope(|scope| {
            let _open = OpenOnDrop(gate.clone());
            let running = scope.spawn(|| {
                run_round(&t, Some(TOKEN), &mut book, &inputs, TODAY, &mut |file| saved.lock().unwrap().push(file))
            });
            let deadline = Instant::now() + Duration::from_secs(10);
            while saved.lock().unwrap().is_empty() {
                assert!(Instant::now() < deadline, "no status file was saved while dischash was held");
                std::thread::sleep(Duration::from_millis(10));
            }
            {
                let held = saved.lock().unwrap();
                assert_eq!(held.len(), 1);
                assert_eq!(held[0].name, "lookup-status.txt");
                let text = String::from_utf8(held[0].bytes.clone()).unwrap();
                assert_eq!(text.lines().collect::<Vec<&str>>(), ["dischash 000"]);
            }
            gate.open();
            running.join().unwrap();
        });
        assert_eq!(book.states[&Lookup::Dischash], answered(404));
    }

    #[test]
    fn without_a_token_the_tmdb_lookups_are_pending_and_never_requested() {
        let t = RecordedTransport::new();
        misses(&t);
        let mut book = LookupBook::default();
        let mut saved = Vec::new();
        run_round(&t, None, &mut book, &chain(Some(DISC_ID), Some("JOHN_WICK")), TODAY, &mut |file| saved.push(file));
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID)]);
        assert_eq!(book.states[&Lookup::Search], LookupState::Pending { reason: PendingReason::NoToken });
        assert!(!book.states.contains_key(&Lookup::Record));
        assert_eq!(book.pending(), (false, true));
        assert!(last_status(&saved).contains(&"search 000".to_string()));
    }

    #[test]
    fn pending_flags_follow_the_book() {
        let book = |states: &[(Lookup, LookupState)]| LookupBook {
            states: states.iter().cloned().collect(),
            answers: Answers::default(),
        };
        let timeout = LookupState::Pending { reason: PendingReason::Timeout };
        assert_eq!(LookupBook::default().pending(), (true, true));
        assert_eq!(book(&[(Lookup::Dischash, answered(404)), (Lookup::Discid, timeout.clone())]).pending(), (true, false));
        assert_eq!(book(&[(Lookup::Dischash, timeout.clone())]).pending(), (true, false));
        assert_eq!(book(&[(Lookup::Dischash, answered(200)), (Lookup::Search, LookupState::InFlight)]).pending(), (false, true));
        assert_eq!(
            book(&[(Lookup::Dischash, answered(404)), (Lookup::Search, answered(200)), (Lookup::Record, LookupState::NotAsked)]).pending(),
            (false, true)
        );
        assert_eq!(book(&[(Lookup::Dischash, answered(404))]).pending(), (false, false));
        let all = book(&[
            (Lookup::Dischash, answered(404)),
            (Lookup::Discid, answered(404)),
            (Lookup::Search, answered(200)),
            (Lookup::Record, answered(200)),
        ]);
        assert_eq!(all.pending(), (false, false));
        let waiting = book(&[(Lookup::Dischash, timeout)]);
        let inputs = waiting.lookup_inputs(None);
        assert_eq!((inputs.discdb_pending, inputs.tmdb_pending), waiting.pending());
    }

    #[test]
    fn lookup_inputs_take_the_first_disc_id_record_with_titles() {
        let empty: DiscDbRecord = serde_json::from_str(r#"{"titles": []}"#).unwrap();
        let text = String::from_utf8(corpus(JOHN_WICK_DISCID)).unwrap();
        let known = parse_discid(&text, JOHN_WICK_DISCID).unwrap().remove(0);
        let book = LookupBook {
            states: BTreeMap::from([(Lookup::Dischash, answered(404)), (Lookup::Discid, answered(200))]),
            answers: Answers { by_disc_id: vec![empty, known.clone()], ..Answers::default() },
        };
        assert_eq!(book.lookup_inputs(None).discdb_by_disc_id, Some(&known));
        assert_eq!(book.lookup_inputs(None).discdb_by_fingerprint, None);
    }
}

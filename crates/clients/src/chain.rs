//! The movie lookup chain (design parts 8.2 and 8.3): TheDiscDB by fingerprint, TheDiscDB by disc ID, TMDb's search
//! on the disc's own name, J8's second search on its tagged name when that folds to another text (its page merged into
//! the first by `merge_pages`), S47's search on the title of a TheDiscDB entry that has no TMDb ID
//! (`core::lookup::entry_search_title`; the record step then reads its page instead of the name page), then the TMDb
//! record that `core::lookup::record_to_fetch` names for the answers in hand. A round asks every lookup that is not
//! answered yet, in that order, and saves each answer and `lookup-status.txt` as they come; a later round asks again
//! what stayed pending. Pull request 2 runs the movie path only (S19). Used by Tasks 23, 24, 25 and 26.

use std::collections::BTreeMap;

use lazymkv_core::lookup::discdb::DiscDbRecord;
use lazymkv_core::lookup::name::{fold_name, DiscName};
use lazymkv_core::lookup::tmdb::{TmdbRecord, TmdbSearch};
use lazymkv_core::lookup::{entry_search_title, record_to_fetch, LookupInputs};
use serde_json::Value;

use crate::lookups::{
    discdb_by_disc_id, discdb_by_fingerprint, tmdb_record, tmdb_search, Fetched, Outcome, PendingReason,
};
use crate::merge::merge_pages;
use crate::saved::{slim_record, slim_search, slim_tmdb, status_text, SavedFile};
use crate::transport::HttpTransport;

/// One request of the chain of design part 8.2, in chain order. Def: T4-2. Use: T4-3 to T4-5, T4-9, T4-15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lookup {
    Dischash,
    Discid,
    Search,
    TaggedSearch,
    EntrySearch,
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
    /// The cleaned name's page, merged with `tagged` once both are in hand (J8). Def: T4-2. Use: T4-2, T4-3, T4-9.
    pub search: Option<TmdbSearch>,
    /// The tagged name's page (design part 8.2 item 2). Def: T4-2. Use: T4-2, T4-3.
    pub tagged: Option<TmdbSearch>,
    /// The entry-title search's page (S47; Ruling T4-R2). Def: T4-3. Use: T4-3, T4-9.
    pub entry: Option<TmdbSearch>,
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
        let tmdb = !present(Lookup::Dischash)
            || waiting(Lookup::Search)
            || waiting(Lookup::TaggedSearch)
            || waiting(Lookup::EntrySearch)
            || waiting(Lookup::Record);
        (discdb, tmdb)
    }

    /// The book as core's `LookupInputs` (Ruling T4-R2), with the flags of `pending`: `search` is `answers.entry` when
    /// `entry_search_title` over the book names a title (None while that search is pending), else `answers.search`.
    /// Def: T4-3. Use: T4-3, T4-5, T4-9.
    pub fn lookup_inputs<'a>(&'a self, name: Option<&'a DiscName>) -> LookupInputs<'a> {
        let (discdb_pending, tmdb_pending) = self.pending();
        let mut inputs = LookupInputs {
            name,
            // dev's rule (`crates/dev/src/cmd/plan.rs`): the first disc ID record that has titles.
            discdb_by_disc_id: self.answers.by_disc_id.iter().find(|record| !record.titles.is_empty()),
            discdb_by_fingerprint: self.answers.by_fingerprint.as_ref(),
            discdb_pending,
            search: self.answers.search.as_ref(),
            record: self.answers.record.as_ref(),
            tmdb_pending,
            ..LookupInputs::default()
        };
        if entry_search_title(&inputs).is_some() {
            inputs.search = self.answers.entry.as_ref();
        }
        inputs
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
/// named by the record in hand while `Record` is answered with one, else by `record_to_fetch` over the book (PF-18);
/// when that names nothing there is no record line, as step 3 of the round then drops `Record`. Design part 8.4.
fn status_file(book: &LookupBook, name: Option<&DiscName>) -> SavedFile {
    let mut lines = BTreeMap::new();
    for (lookup, state) in &book.states {
        let Some(word) = status_word(state) else { continue };
        let key = match lookup {
            Lookup::Dischash => "dischash".to_string(),
            Lookup::Discid => "discid".to_string(),
            Lookup::Search => "search".to_string(),
            Lookup::TaggedSearch => "search-tagged".to_string(),
            Lookup::EntrySearch => "search-entry".to_string(),
            Lookup::Record => {
                let identity = match (state, &book.answers.record) {
                    (LookupState::Answered { .. }, Some(record)) => Some(record.identity),
                    _ => record_to_fetch(&book.lookup_inputs(name)),
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

/// A parsed search page as JSON, for `slim_search`.
fn page_value(page: &TmdbSearch) -> Value {
    // Serializing a parsed page cannot fail: its fields are strings, numbers and options.
    serde_json::to_value(page).unwrap_or_default()
}

/// `tmdb-search.json` over the name search pages in hand: the cleaned name's page (merged once the tagged page came)
/// with its query, then the tagged name's page with its query (J8).
fn search_file(answers: &Answers, name: &DiscName, today: &str) -> SavedFile {
    let cleaned = answers.search.as_ref().map(page_value);
    let tagged = answers.tagged.as_ref().map(page_value);
    let mut pages: Vec<(&Value, &str)> = Vec::new();
    if let Some(page) = &cleaned {
        pages.push((page, &name.cleaned));
    }
    if let Some(page) = &tagged {
        pages.push((page, &name.tagged));
    }
    json_file("tmdb-search.json", &slim_search(&pages, today))
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
    if let Some(searched) = searched {
        book.states.entry(Lookup::Search).or_insert(LookupState::NotAsked);
        if fold_name(&searched.tagged) != fold_name(&searched.cleaned) {
            book.states.entry(Lookup::TaggedSearch).or_insert(LookupState::NotAsked);
        }
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

    // S47 (Ruling T4-R2): an entry search no longer named, as TheDiscDB's records now name a TMDb ID or no title,
    // leaves the book with its page. A name search answered while its page is not in hand (a restart that read the
    // saved file as the entry page) is asked again, so the name pages come back to tmdb-search.json.
    if entry_search_title(&book.lookup_inputs(name)).is_none() && book.states.remove(&Lookup::EntrySearch).is_some() {
        book.answers.entry = None;
        for (lookup, in_hand) in
            [(Lookup::Search, book.answers.search.is_some()), (Lookup::TaggedSearch, book.answers.tagged.is_some())]
        {
            if !in_hand && matches!(book.states.get(&lookup), Some(LookupState::Answered { status: 200 })) {
                book.states.insert(lookup, LookupState::NotAsked);
            }
        }
        let in_hand = book.answers.search.is_some() || book.answers.tagged.is_some();
        if let Some(searched) = searched.filter(|_| in_hand) {
            save(search_file(&book.answers, searched, today));
        }
        save(status_file(book, name));
    }

    if let Some(searched) = searched.filter(|_| book.asks(Lookup::Search)) {
        start(book, Lookup::Search, name, save);
        let (settled, _) = settle(book, Lookup::Search, tmdb_search(t, token, &searched.cleaned));
        let page = matches!(settled, Settled::Value(_));
        let answers = &mut book.answers;
        settled.write(&mut answers.search, |search| match &answers.tagged {
            Some(tagged) => Some(merge_pages(&search, tagged)),
            None => Some(search),
        });
        if page && book.answers.entry.is_none() {
            save(search_file(&book.answers, searched, today));
        }
        save(status_file(book, name));
    }

    if let Some(searched) = searched.filter(|_| book.asks(Lookup::TaggedSearch)) {
        start(book, Lookup::TaggedSearch, name, save);
        let (settled, _) = settle(book, Lookup::TaggedSearch, tmdb_search(t, token, &searched.tagged));
        let page = matches!(settled, Settled::Value(_));
        settled.write(&mut book.answers.tagged, Some);
        if let (Some(search), Some(tagged)) = (&book.answers.search, &book.answers.tagged) {
            book.answers.search = Some(merge_pages(search, tagged));
        }
        if page && book.answers.entry.is_none() {
            save(search_file(&book.answers, searched, today));
        }
        save(status_file(book, name));
    }

    // S47 (Rulings T4-R2, T4-R5): the entry's title is searched once; an answered entry search is not asked again for
    // another title, as the fingerprint's and the disc ID's records of one disc name one work. Its page replaces the
    // name pages in tmdb-search.json; until it comes the name pages stay there.
    if let Some(title) = entry_search_title(&book.lookup_inputs(name)) {
        book.states.entry(Lookup::EntrySearch).or_insert(LookupState::NotAsked);
        if book.asks(Lookup::EntrySearch) {
            start(book, Lookup::EntrySearch, name, save);
            let (settled, _) = settle(book, Lookup::EntrySearch, tmdb_search(t, token, &title));
            let answered = matches!(settled, Settled::Value(_));
            settled.write(&mut book.answers.entry, Some);
            if let Some(page) = book.answers.entry.as_ref().filter(|_| answered).map(page_value) {
                save(json_file("tmdb-search.json", &slim_search(&[(&page, &title)], today)));
            }
            save(status_file(book, name));
        }
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
    use lazymkv_core::lookup::tmdb::{parse_movie, parse_search, TmdbIdentity, TmdbKind};
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
    fn a_pending_record_with_no_identity_left_to_fetch_has_no_status_line_and_leaves_the_book() {
        // The record of an earlier identity is in hand, and the last fetch of a record timed out.
        let earlier = String::from_utf8(movie_body(900004, "John Wick", "2099-01-01")).unwrap();
        let mut book = LookupBook {
            states: BTreeMap::from([
                (Lookup::Dischash, LookupState::Pending { reason: PendingReason::Timeout }),
                (Lookup::Record, LookupState::Pending { reason: PendingReason::Timeout }),
            ]),
            answers: Answers { record: Some(parse_movie(&earlier, "movie.json").unwrap()), ..Answers::default() },
        };
        // TheDiscDB misses and the name search leaves two plausible results: no record is to be fetched.
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        misses(&t);
        t.answer(&search_of(&inputs), 200, &search_body(&[(245891, "John Wick", "2014-10-22"), (900003, "John Wick", "1990-01-01")]));
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(record_to_fetch(&book.lookup_inputs(inputs.name.as_ref())), None);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK")]);
        for file in saved.iter().filter(|file| file.name == "lookup-status.txt") {
            let text = String::from_utf8(file.bytes.clone()).unwrap();
            assert!(!text.lines().any(|line| line.starts_with("tmdb ")), "{text}");
        }
        assert_eq!(last_status(&saved), ["dischash 404", "discid 404", "search 200"]);
        assert!(!book.states.contains_key(&Lookup::Record));
        assert_eq!(book.pending(), (false, false));
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

    /// The name search pages of `PIRANHA_3D` (cleaned `PIRANHA`, tagged `PIRANHA 3D`; decision-core H1). IDs made up.
    const PIRANHA_CLEANED: [(i64, &str, &str); 2] = [(900101, "Piranha", "1978-08-03"), (900102, "Piranha", "2010-08-20")];
    const PIRANHA_TAGGED: [(i64, &str, &str); 2] = [(900102, "Piranha", "2010-08-20"), (900103, "Piranha 3D", "2010-08-20")];

    /// The tmdb-search.json files in `saved`, parsed, in order.
    fn search_files(saved: &[SavedFile]) -> Vec<Value> {
        saved.iter().filter(|f| f.name == "tmdb-search.json").map(|f| serde_json::from_slice(&f.bytes).unwrap()).collect()
    }

    #[test]
    fn a_name_whose_tagged_form_differs_is_searched_twice_and_merged() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("PIRANHA_3D"));
        misses(&t);
        t.answer(&search_url("PIRANHA"), 200, &search_body(&PIRANHA_CLEANED));
        t.answer(&search_url("PIRANHA 3D"), 200, &search_body(&PIRANHA_TAGGED));
        t.answer(&record_url(movie(900103)), 200, &movie_body(900103, "Piranha 3D", "2010-08-20"));
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &inputs);
        let searched = [search_url("PIRANHA"), search_url("PIRANHA 3D")];
        assert_eq!(urls(&t)[2..4], searched);
        assert_eq!(urls(&t).last(), Some(&record_url(movie(900103))));
        let merged: Vec<i64> = book.answers.search.as_ref().unwrap().results.iter().map(|r| r.id).collect();
        assert_eq!(merged, [900101, 900102, 900103]);
        assert_eq!(book.pending(), (false, false));
        assert_eq!(last_status(&saved), ["dischash 404", "discid 404", "search 200", "search-tagged 200", "tmdb movie/900103 200"]);
        let files = search_files(&saved);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["queries"], json!(["PIRANHA"]));
        assert_eq!(files[1]["queries"], json!(["PIRANHA", "PIRANHA 3D"]));
        assert_eq!(files[1]["results"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn a_name_whose_forms_fold_equal_is_searched_once() {
        let mut inputs = chain(Some(DISC_ID), None);
        inputs.name = disc_name(Some("Synthetic Movie"), Some("SYNTHETIC_MOVIE"));
        let cleaned = inputs.name.as_ref().unwrap().cleaned.clone();
        let t = RecordedTransport::new();
        misses(&t);
        t.answer(&search_url(&cleaned), 200, &search_body(&[(900001, "Synthetic Movie", "2001-02-03")]));
        t.answer(&record_url(movie(900001)), 200, &movie_body(900001, "Synthetic Movie", "2001-02-03"));
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url(&cleaned), record_url(movie(900001))]);
        assert!(!book.states.contains_key(&Lookup::TaggedSearch));
        assert!(!last_status(&saved).iter().any(|line| line.starts_with("search-tagged")));
    }

    #[test]
    fn tmdb_stays_pending_until_the_tagged_search_answers() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("PIRANHA_3D"));
        misses(&t);
        t.answer(&search_url("PIRANHA"), 200, &search_body(&PIRANHA_CLEANED));
        t.answer(&search_url("PIRANHA 3D"), 503, b"");
        t.answer(&search_url("PIRANHA 3D"), 200, &search_body(&PIRANHA_TAGGED));
        t.answer(&record_url(movie(900103)), 200, &movie_body(900103, "Piranha 3D", "2010-08-20"));
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(book.states[&Lookup::TaggedSearch], LookupState::Pending { reason: PendingReason::Status(503) });
        assert!(book.pending().1);
        assert!(last_status(&saved).contains(&"search-tagged 503".to_string()));
        let files = search_files(&saved);
        assert_eq!((files.len(), &files[0]["queries"]), (1, &json!(["PIRANHA"])));
        // Two plausible "Piranha" results: no record yet.
        assert!(!urls(&t).iter().any(|url| url.contains("/movie/")));
        let saved = round(&t, &mut book, &inputs);
        assert_eq!(book.pending(), (false, false));
        assert_eq!(search_files(&saved)[0]["queries"], json!(["PIRANHA", "PIRANHA 3D"]));
        assert_eq!(urls(&t).last(), Some(&record_url(movie(900103))));
    }

    /// A dischash hit whose media has a title and a year but no TMDb ID (S47's case), with one title.
    fn entry_body(title: &str, year: i32) -> Vec<u8> {
        json!({"media": {"title": title, "full_title": title, "year": year, "type": "Movie", "external_ids": {}},
               "titles": [{"index": 0, "source_file": "00001.mpls", "duration": "2:01:14"}]}).to_string().into_bytes()
    }
    /// Knowing (TMDb 13811, TheDiscDB's) and a made-up namesake.
    const KNOWING_PAGE: [(i64, &str, &str); 2] = [(13811, "Knowing", "2009-03-19"), (900201, "Knowing", "1990-01-01")];

    /// TheDiscDB knows the fingerprint by title alone; the name and entry searches and Knowing's record answer.
    fn knowing(t: &RecordedTransport, name_query: &str) {
        t.answer(&dischash_url(FP), 200, &entry_body("Knowing", 2009));
        t.answer(&discid_url(DISC_ID), 404, b"");
        t.answer(&search_url(name_query), 200, &search_body(&KNOWING_PAGE));
        t.answer(&search_url("Knowing"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&record_url(movie(13811)), 200, &movie_body(13811, "Knowing", "2009-03-19"));
    }

    #[test]
    fn an_entry_without_a_tmdb_id_is_searched_by_its_title_alone() {
        let t = RecordedTransport::new();
        knowing(&t, "KNOWING");
        let mut book = LookupBook::default();
        let saved = round(&t, &mut book, &chain(Some(DISC_ID), Some("KNOWING")));
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("KNOWING"), search_url("Knowing"), record_url(movie(13811))]);
        assert_eq!(urls(&t)[3], "https://api.themoviedb.org/3/search/multi?query=Knowing");
        assert_eq!(last_status(&saved), ["dischash 200", "discid 404", "search 200", "search-entry 200", "tmdb movie/13811 200"]);
        assert_eq!(book.pending(), (false, false));
    }

    #[test]
    fn the_plan_reads_the_entry_page_not_the_name_page() {
        let record: DiscDbRecord = serde_json::from_slice(&entry_body("Knowing", 2009)).unwrap();
        let name_page = parse_search(&String::from_utf8(search_body(&PIRANHA_CLEANED)).unwrap(), "name").unwrap();
        let entry_page = parse_search(&String::from_utf8(search_body(&KNOWING_PAGE)).unwrap(), "entry").unwrap();
        let mut book = LookupBook {
            states: BTreeMap::from([(Lookup::Dischash, answered(200)), (Lookup::Search, answered(200)), (Lookup::EntrySearch, answered(200))]),
            answers: Answers { by_fingerprint: Some(record), search: Some(name_page), entry: Some(entry_page.clone()), ..Answers::default() },
            ..LookupBook::default() // Task 4 adds record_file
        };
        assert_eq!(book.lookup_inputs(None).search, Some(&entry_page));
        book.states.insert(Lookup::EntrySearch, LookupState::Pending { reason: PendingReason::Status(503) });
        book.answers.entry = None;
        assert_eq!(book.lookup_inputs(None).search, None);
        assert!(book.pending().1);
    }

    #[test]
    fn the_entry_page_replaces_the_name_page_in_tmdb_search_json() {
        let t = RecordedTransport::new();
        t.fail(&dischash_url(FP), TransportError::Timeout);
        knowing(&t, "KNOWING");
        let inputs = chain(Some(DISC_ID), Some("KNOWING"));
        let mut book = LookupBook::default();
        let first = round(&t, &mut book, &inputs);
        assert_eq!(search_files(&first).last().unwrap()["queries"], json!(["KNOWING"]));
        let second = round(&t, &mut book, &inputs);
        assert_eq!(search_files(&second).last().unwrap()["queries"], json!(["Knowing"]));
    }

    #[test]
    fn an_entry_with_neither_id_nor_title_falls_back_to_the_name_search() {
        let t = RecordedTransport::new();
        let inputs = chain(Some(DISC_ID), Some("JOHN_WICK"));
        t.answer(&dischash_url(FP), 200, br#"{"media": {"type": "Movie", "external_ids": {}}, "titles": [{"index": 0}]}"#);
        tmdb_only(&t, &inputs);
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert_eq!(urls(&t), [dischash_url(FP), discid_url(DISC_ID), search_url("JOHN WICK"), record_url(JOHN_WICK)]);
        assert!(!book.states.contains_key(&Lookup::EntrySearch));
        assert_eq!(book.lookup_inputs(inputs.name.as_ref()).search, book.answers.search.as_ref());
    }

    #[test]
    fn a_late_name_answer_never_overwrites_the_entry_page() {
        let t = RecordedTransport::new();
        t.answer(&search_url("KNOWING 3D"), 503, b"");
        t.answer(&search_url("KNOWING 3D"), 200, &search_body(&[(900202, "Knowing 3D", "2011-01-01")]));
        knowing(&t, "KNOWING");
        let inputs = chain(Some(DISC_ID), Some("KNOWING_3D"));
        let mut book = LookupBook::default();
        let first = round(&t, &mut book, &inputs);
        assert_eq!(
            search_files(&first).iter().map(|f| f["queries"].clone()).collect::<Vec<Value>>(),
            [json!(["KNOWING"]), json!(["Knowing"])]
        );
        assert!(book.pending().1);
        let second = round(&t, &mut book, &inputs);
        assert!(search_files(&second).is_empty());
        let merged: Vec<i64> = book.answers.search.as_ref().unwrap().results.iter().map(|r| r.id).collect();
        assert_eq!(merged, [13811, 900201, 900202]);
        assert!(last_status(&second).contains(&"search-tagged 200".to_string()));
        assert_eq!(book.pending(), (false, false));
    }

    #[test]
    fn an_entry_search_no_longer_named_leaves_the_book_and_the_name_page_returns() {
        let t = RecordedTransport::new();
        t.fail(&dischash_url(FP), TransportError::Timeout);
        t.answer(&dischash_url(FP), 200, &corpus(JOHN_WICK_DISCHASH));
        let disc_id_body = format!("[{}]", String::from_utf8(entry_body("Knowing", 2009)).unwrap());
        t.answer(&discid_url(DISC_ID), 200, disc_id_body.as_bytes());
        t.answer(&search_url("KNOWING"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&search_url("Knowing"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&record_url(movie(13811)), 200, &movie_body(13811, "Knowing", "2009-03-19"));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
        let inputs = chain(Some(DISC_ID), Some("KNOWING"));
        let mut book = LookupBook::default();
        round(&t, &mut book, &inputs);
        assert!(book.answers.entry.is_some());
        let saved = round(&t, &mut book, &inputs);
        assert!(!book.states.contains_key(&Lookup::EntrySearch));
        assert!(book.answers.entry.is_none());
        assert_eq!(search_files(&saved).last().unwrap()["queries"], json!(["KNOWING"]));
        assert_eq!(urls(&t).last(), Some(&record_url(JOHN_WICK)));
    }

    #[test]
    fn a_one_page_search_file_keeps_pull_request_2s_bytes() {
        let mut inputs = chain(Some(DISC_ID), None);
        inputs.name = disc_name(Some("Synthetic Movie"), Some("SYNTHETIC_MOVIE"));
        let cleaned = inputs.name.as_ref().unwrap().cleaned.clone();
        let body = search_body(&[(900001, "Synthetic Movie", "2001-02-03")]);
        let t = RecordedTransport::new();
        misses(&t);
        t.answer(&search_url(&cleaned), 200, &body);
        t.answer(&record_url(movie(900001)), 200, &movie_body(900001, "Synthetic Movie", "2001-02-03"));
        let saved = round(&t, &mut LookupBook::default(), &inputs);
        let raw: Value = serde_json::from_slice(&body).unwrap();
        let mut expected = serde_json::to_vec_pretty(&slim_search(&[(&raw, &cleaned)], TODAY)).unwrap();
        expected.push(b'\n');
        let files: Vec<&SavedFile> = saved.iter().filter(|f| f.name == "tmdb-search.json").collect();
        assert_eq!(files.len(), 1);
        assert_eq!(String::from_utf8(files[0].bytes.clone()).unwrap(), String::from_utf8(expected).unwrap());
    }

    #[test]
    fn a_restart_that_read_the_entry_page_fetches_the_name_page_again() {
        // The book a restart leaves after "dischash 000", "discid 200", "search 200" and "search-entry 200": the saved
        // tmdb-search.json held the entry page, so the name page is not in hand.
        let record: DiscDbRecord = serde_json::from_slice(&entry_body("Knowing", 2009)).unwrap();
        let entry_page = parse_search(&String::from_utf8(search_body(&KNOWING_PAGE)).unwrap(), "entry").unwrap();
        let mut book = LookupBook {
            states: BTreeMap::from([
                (Lookup::Dischash, LookupState::Pending { reason: PendingReason::Timeout }),
                (Lookup::Discid, answered(200)),
                (Lookup::Search, answered(200)),
                (Lookup::EntrySearch, answered(200)),
            ]),
            answers: Answers { by_disc_id: vec![record], entry: Some(entry_page), ..Answers::default() },
        };
        // The fingerprint's record now names John Wick's TMDb ID, so no entry title is named.
        let t = RecordedTransport::new();
        t.answer(&dischash_url(FP), 200, &corpus(JOHN_WICK_DISCHASH));
        t.answer(&search_url("KNOWING"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&record_url(JOHN_WICK), 200, &movie_body(245891, "John Wick", "2014-10-22"));
        let saved = round(&t, &mut book, &chain(Some(DISC_ID), Some("KNOWING")));
        assert!(urls(&t).contains(&search_url("KNOWING")));
        assert!(book.answers.search.is_some());
        assert!(book.answers.entry.is_none());
        assert!(!book.states.contains_key(&Lookup::EntrySearch));
        assert_eq!(search_files(&saved).last().unwrap()["queries"], json!(["KNOWING"]));
    }

    #[test]
    fn a_record_from_thediscdb_drops_the_entry_line_from_the_status_file() {
        // TheDiscDB's disc ID record names Knowing by title alone; the fingerprint, timed out at first, names TMDb 13811.
        let t = RecordedTransport::new();
        t.fail(&dischash_url(FP), TransportError::Timeout);
        let mut named: Value = serde_json::from_slice(&entry_body("Knowing", 2009)).unwrap();
        named["media"]["external_ids"]["tmdb"] = json!("13811");
        t.answer(&dischash_url(FP), 200, named.to_string().as_bytes());
        let disc_id_body = format!("[{}]", String::from_utf8(entry_body("Knowing", 2009)).unwrap());
        t.answer(&discid_url(DISC_ID), 200, disc_id_body.as_bytes());
        t.answer(&search_url("KNOWING"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&search_url("Knowing"), 200, &search_body(&KNOWING_PAGE));
        t.answer(&record_url(movie(13811)), 200, &movie_body(13811, "Knowing", "2009-03-19"));
        let inputs = chain(Some(DISC_ID), Some("KNOWING"));
        let mut book = LookupBook::default();
        let first = round(&t, &mut book, &inputs);
        assert!(last_status(&first).contains(&"search-entry 200".to_string()));
        assert_eq!(urls(&t).last(), Some(&record_url(movie(13811))));
        let asked = t.requests().len();
        let second = round(&t, &mut book, &inputs);
        assert!(!last_status(&second).iter().any(|line| line.starts_with("search-entry")));
        assert_eq!(urls(&t)[asked..], [dischash_url(FP)]);
        assert!(!book.states.contains_key(&Lookup::EntrySearch));
    }
}

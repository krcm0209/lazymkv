//! The local refetch of a stale TMDb copy (design part 8.4, R-B4, K15; Rulings T4-R7, T4-R10). Before `plan`,
//! `decide` and `retune` read a recording folder, each TMDb copy in it that is more than six months old is fetched
//! again with the token of TMDB_TOKEN and replaced in place, or the command exits 3 naming it. A copy without a
//! `fetched_at` is read as it is, and so is an engine job folder (one holding `state.json`): the engine fetches its
//! own copies again. No text here holds the token.

use std::fs;
use std::io::Write;
use std::path::Path;

use lazymkv_clients::{
    reason_text, refetch_record, refetch_search, stale_copies, status_record, HttpTransport, OfflineTransport,
    SavedFile, StaleCopy,
};
use lazymkv_core::lookup::stale::TMDB_COPIES;
use lazymkv_core::recording::Tree;
use serde_json::Value;

use crate::evidence::{Ctx, Recording, Refused};

/// The variable a local run reads the TMDb token from (design part 8.4, part 15 item 8). Def: T4-10. Use: T4-10.
pub const TMDB_TOKEN_VAR: &str = "TMDB_TOKEN";
/// The file that marks an engine job folder (design part 7.1), which dev reads as it is. Def: T4-10. Use: T4-10.
pub const JOB_STATE_FILE: &str = "state.json";
/// The lookups' status file of a recording, whose `tmdb` line names the record.
const STATUS_FILE: &str = "lookup-status.txt";
/// The copy of TMDb's search, fetched again from its own `queries`.
const SEARCH_COPY: &str = "tmdb-search.json";
/// What the temporary file of a write is named after: `<file>.lazymkv-tmp`, beside the copy.
const TEMP_SUFFIX: &str = ".lazymkv-tmp";
/// Why a stale record copy cannot be fetched again when the status file names no record.
const NO_RECORD: &str = "no tmdb line of lookup-status.txt names the record";

/// Fetches again each TMDb copy of `folder` that is stale (design part 8.4, R-B4, K15; Rulings T4-R7, T4-R10) and
/// replaces it, giving the names it replaced in `TMDB_COPIES` order; Ok(vec![]) for a folder holding
/// `JOB_STATE_FILE` and for one with nothing stale, asking nothing. Err is `Refused { code: 3, line }` naming
/// `<shown>/<file>`; no text holds the token. Def: T4-10. Use: T4-10.
pub fn refresh_copies(ctx: &Ctx, folder: &Path, shown: &str) -> Result<Vec<String>, Refused> {
    if folder.join(JOB_STATE_FILE).is_file() {
        return Ok(vec![]);
    }
    let status = fs::read_to_string(folder.join(STATUS_FILE)).unwrap_or_default();
    let status = status.strip_prefix('\u{feff}').unwrap_or(&status);
    // No TMDb copy, nothing stale: the clock (a PowerShell start on Windows) is read only once a copy is there.
    if !TMDB_COPIES.iter().any(|file| folder.join(file).is_file()) {
        return Ok(vec![]);
    }
    let today = ctx.today();
    let copies = stale_copies(status, &|name| fs::read(folder.join(name)).ok(), false, &today);
    let Some(first) = copies.first() else {
        return Ok(vec![]);
    };
    let Some(token) = ctx.env.get(TMDB_TOKEN_VAR).filter(|token| !token.is_empty()) else {
        return Err(refused(no_token_line(shown, first)));
    };
    let offline = OfflineTransport;
    let t: &dyn HttpTransport = match &ctx.http {
        Some(http) => http.as_ref(),
        None => &offline,
    };
    let mut replaced = Vec::new();
    let records: Vec<&StaleCopy> = copies.iter().filter(|copy| copy.file != SEARCH_COPY).collect();
    if let Some(first_record) = records.first() {
        let id = status_record(status).ok_or_else(|| refused(failed_line(shown, first_record, NO_RECORD)))?;
        // Dev has no parsed season, so a TV record is fetched with every season it lists (Ruling T4-R4).
        let (_, files) = refetch_record(t, Some(token), id, None, &today)
            .map_err(|reason| refused(failed_line(shown, first_record, &reason_text(&reason))))?;
        for copy in &records {
            // Only a stale copy is written, so a phase 0 folder never gains a tmdb-record.json.
            if let Some(file) = files.iter().find(|file| file.name == copy.file) {
                write_copy(folder, file).map_err(|error| refused(failed_line(shown, copy, &error.to_string())))?;
                replaced.push(file.name.clone());
            }
        }
    }
    if let Some(search) = copies.iter().find(|copy| copy.file == SEARCH_COPY) {
        let queries = search_queries(&folder.join(SEARCH_COPY));
        let (_, file) = refetch_search(t, Some(token), &queries, &today)
            .map_err(|reason| refused(failed_line(shown, search, &reason_text(&reason))))?;
        write_copy(folder, &file).map_err(|error| refused(failed_line(shown, search, &error.to_string())))?;
        replaced.push(file.name);
    }
    Ok(replaced)
}

/// `<shown>/<file>: the TMDb copy is more than six months old (fetched_at <date>) and TMDB_TOKEN is not set to fetch
/// it again`; `(no fetched_at)` replaces `(fetched_at <date>)` when the copy has none. Def: T4-10. Use: T4-10.
pub fn no_token_line(shown: &str, copy: &StaleCopy) -> String {
    format!(
        "{}: the TMDb copy is more than six months old ({}) and {TMDB_TOKEN_VAR} is not set to fetch it again",
        named(shown, copy),
        dated(copy)
    )
}

/// `<shown>/<file>: the TMDb copy is more than six months old (fetched_at <date>) and could not be fetched again:
/// <why>`, `<why>` being `reason_text` of the fetch's reason or a write's error. Def: T4-10. Use: T4-10.
pub fn failed_line(shown: &str, copy: &StaleCopy, why: &str) -> String {
    format!(
        "{}: the TMDb copy is more than six months old ({}) and could not be fetched again: {why}",
        named(shown, copy),
        dated(copy)
    )
}

/// The stderr line of a copy fetched again: `lazymkv-dev <command>: <shown>/<file> was more than six months old and
/// was fetched again`. Def: T4-10. Use: T4-10.
pub fn fetched_line(command: &str, shown: &str, file: &str) -> String {
    format!("lazymkv-dev {command}: {shown}/{file} was more than six months old and was fetched again")
}

/// The gate of `plan` and `decide` over a recording (design part 8.4, K15): `refresh_copies` over its folder, each
/// copy fetched again printed as `fetched_line`; Err holds the exit code after `lazymkv-dev <command>: error: <line>`.
pub(crate) fn gate_recording(ctx: &Ctx, rec: &Recording, command: &str, err: &mut dyn Write) -> Result<(), i32> {
    let shown = rec.tree.display(&rec.dir);
    match refresh_copies(ctx, &rec.tree.path(&rec.dir), &shown) {
        Ok(names) => {
            for name in names {
                let _ = writeln!(err, "{}", fetched_line(command, &shown, &name));
            }
            Ok(())
        }
        Err(refused) => {
            let _ = writeln!(err, "lazymkv-dev {command}: error: {}", refused.line);
            Err(refused.code)
        }
    }
}

/// A `Refused` with exit code 3.
fn refused(line: String) -> Refused {
    Refused { code: 3, line }
}

/// `<shown>/<file>`.
fn named(shown: &str, copy: &StaleCopy) -> String {
    format!("{shown}/{}", copy.file)
}

/// `fetched_at <date>`, or `no fetched_at` for a copy without one.
fn dated(copy: &StaleCopy) -> String {
    match &copy.fetched_at {
        Some(date) => format!("fetched_at {date}"),
        None => "no fetched_at".to_string(),
    }
}

/// The search copy's top-level `queries`, its strings in order; none when the file does not read or has no such array.
fn search_queries(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let value: Value = serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text)).unwrap_or_default();
    value
        .get("queries")
        .and_then(Value::as_array)
        .map(|queries| queries.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Writes `file` over its copy in `folder`: the bytes to `<name>.lazymkv-tmp` beside it, then a rename over the copy.
/// A failed write or rename removes the temporary file.
fn write_copy(folder: &Path, file: &SavedFile) -> std::io::Result<()> {
    let temp = folder.join(format!("{}{TEMP_SUFFIX}", file.name));
    let result = fs::write(&temp, &file.bytes).and_then(|()| fs::rename(&temp, folder.join(&file.name)));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;

    use lazymkv_clients::{record_url, HttpTransport, RecordedTransport, TransportError};
    use lazymkv_core::lookup::stale::fetched_at;
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use serde_json::{json, Value};

    use crate::evidence::Ctx;
    use crate::testutil::{Scratch, ScratchExt};

    const TOKEN: &str = "token-sentinel-5d2e";
    /// Ctx::for_test's day is 2026-10-13: 2026-01-01 plus six months is 2026-07-01, before it; 2026-10-05 is fresh.
    const STALE: &str = "2026-01-01";
    const FRESH: &str = "2026-10-05";
    const STATUS: &str = "\u{feff}dischash 404\r\ndiscid 404\r\ntmdb movie/6537 200\r\n";
    const RECORD_ANSWER: &str = r#"{"id": 6537, "title": "Made Up Film", "original_title": "Pelicula Inventada",
 "release_date": "2007-05-20", "original_language": "es", "runtime": 100, "external_ids": {"imdb_id": "tt0006537"}}"#;
    const SPEC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/superpowers/specs/2026-10-04-lazymkv-design.md");
    /// lookup-status.txt of a disc TheDiscDB knows, as `crates/dev/src/cmd/plan.rs`'s tests with a TheDiscDB hit write it.
    const KNOWN_STATUS: &str = "\u{feff}dischash 200\r\ndiscid 404\r\ntmdb movie/6537 200\r\n";
    /// TheDiscDB's `dischash` answer naming movie 6537, the minimal form of plan.rs's `DISCHASH`: a Movie whose
    /// `media.external_ids.tmdb` gives the plan its TMDb identity (core `lookup/mod.rs:836`), and one MainMovie title.
    const DISCHASH: &str = r#"{"content_hash": "0123456789ABCDEF0123456789ABCDEF", "format": "Blu-ray",
 "media": {"title": "Made Up Film", "year": 2007, "type": "Movie", "external_ids": {"tmdb": "6537"}},
 "titles": [{"index": 0, "item": {"title": "Made Up Film", "type": "MainMovie"}}]}"#;

    fn copy(fetched: &str) -> String {
        format!(r#"{{"id": 6537, "title": "Made Up Film", "original_language": "es", "fetched_at": "{fetched}"}}"#)
    }
    fn movie() -> String { record_url(TmdbIdentity { kind: TmdbKind::Movie, id: 6537 }) }
    fn line(file: &str, rest: &str) -> String {
        format!("recs/film/{file}: the TMDb copy is more than six months old (fetched_at 2026-01-01) and {rest}")
    }

    /// recs/film: the scan and the eng-only label files exactly as `recording` in crates/ctl/src/tests.rs:24-46 writes
    /// recs/disc-a, plus meta.json `{"volume_label": "FILM", "format": "bluray", "fingerprint":
    /// "0123456789ABCDEF0123456789ABCDEF", "disc_id": null, "file_count": 3, "total_bytes": 1000, "captured_unix":
    /// 1790000000}`, lookup-status.txt `STATUS` and tmdb.json `tmdb`.
    fn film(s: &Scratch, tmdb: &str) {
        s.write("recs/film/scan.txt", concat!(
            "TINFO:0,8,0,\"2\"\nTINFO:0,9,0,\"0:10:00\"\n",
            "SINFO:0,0,1,6201,\"Video\"\nSINFO:0,0,6,0,\"Mpeg4\"\n",
            "SINFO:0,1,1,6202,\"Audio\"\nSINFO:0,1,3,0,\"eng\"\nSINFO:0,1,6,0,\"DTS-HD MA\"\nSINFO:0,1,14,0,\"6\"\n",
            "SINFO:0,2,1,6202,\"Audio\"\nSINFO:0,2,3,0,\"eng\"\nSINFO:0,2,6,0,\"DD\"\nSINFO:0,2,14,0,\"2\"\n",
            "SINFO:0,3,1,6202,\"Audio\"\nSINFO:0,3,3,0,\"spa\"\nSINFO:0,3,6,0,\"DD\"\nSINFO:0,3,14,0,\"2\"\n",
            "SINFO:0,4,1,6203,\"Subtitles\"\nSINFO:0,4,3,0,\"eng\"\nSINFO:0,4,6,0,\"PGS\"\n"));
        s.write("recs/film/eng-only/rip-summary.json",
                json!({"ok": true, "title": 0, "profile": "C:\\made-up\\eng-only.mmcp.xml"}).to_string());
        s.write("recs/film/eng-only/mkvmerge.json", json!({
            "tracks": [
                {"id": 0, "type": "video", "codec": "AVC", "properties": {"language": "und"}},
                {"id": 1, "type": "audio", "codec": "DTS-HD MA", "properties": {"language": "eng", "audio_channels": 6}},
                {"id": 2, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 2}},
                {"id": 3, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}}
            ],
            "chapters": [{"num_entries": 2}],
            "container": {"properties": {"duration": 600_000_000_000_i64}}
        }).to_string());
        s.write("recs/film/eng-only/rip.txt", "MSG:5011,0,0,\"made-up line\",\"made-up line\"\n");
        s.write("recs/film/meta.json", json!({"volume_label": "FILM", "format": "bluray", "fingerprint":
            "0123456789ABCDEF0123456789ABCDEF", "disc_id": null, "file_count": 3, "total_bytes": 1000, "captured_unix":
            1790000000}).to_string());
        s.write("recs/film/lookup-status.txt", STATUS);
        s.write("recs/film/tmdb.json", tmdb);
    }

    /// `film`, then thediscdb-hash.json `DISCHASH` and lookup-status.txt `KNOWN_STATUS`, so the plan has TMDb identity
    /// movie 6537 and reads tmdb.json's record for it.
    fn known_film(s: &Scratch, tmdb: &str) {
        film(s, tmdb);
        s.write("recs/film/thediscdb-hash.json", DISCHASH);
        s.write("recs/film/lookup-status.txt", KNOWN_STATUS);
    }

    /// (exit code, stdout, stderr) of lazymkv-dev's dispatcher.
    fn run(ctx: &Ctx, argv: &[&str]) -> (i32, String, String) {
        let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = crate::dispatch(ctx, &argv, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    fn ctx_with(s: &Scratch, http: &Arc<RecordedTransport>, token: bool) -> Ctx {
        let mut ctx = Ctx::for_test(&s.0);
        ctx.http = Some(Arc::clone(http) as Arc<dyn HttpTransport>);
        if token { ctx.env.insert(TMDB_TOKEN_VAR.to_string(), TOKEN.to_string()); }
        ctx
    }

    #[test]
    fn plan_over_a_folder_with_a_stale_copy_and_no_token_exits_3_naming_it() {
        let s = Scratch::new("stale-plan-no-token");
        film(&s, &copy(STALE));
        let http = Arc::new(RecordedTransport::new());
        let (code, out, err) = run(&ctx_with(&s, &http, false), &["plan", "recs/film"]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert_eq!(err, format!("lazymkv-dev plan: error: {}\n", line("tmdb.json", "TMDB_TOKEN is not set to fetch it again")));
        assert!(http.requests().is_empty());
        assert_eq!(fs::read_to_string(s.0.join("recs/film/tmdb.json")).unwrap(), copy(STALE));
    }

    #[test]
    fn plan_with_tmdb_token_fetches_the_stale_copy_again_and_plans() {
        let s = Scratch::new("stale-plan-token");
        known_film(&s, &copy(STALE));
        let http = Arc::new(RecordedTransport::new());
        http.answer(&movie(), 200, RECORD_ANSWER.as_bytes());
        let (code, out, err) = run(&ctx_with(&s, &http, true), &["plan", "recs/film"]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(err, "lazymkv-dev plan: recs/film/tmdb.json was more than six months old and was fetched again\n");
        let requests = http.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, movie());
        assert!(requests[0].1.contains(&("Authorization".to_string(), format!("Bearer {TOKEN}"))));
        let text = fs::read_to_string(s.0.join("recs/film/tmdb.json")).unwrap();
        assert_eq!(fetched_at(&text).as_deref(), Some("2026-10-13"));
        assert!(text.ends_with('\n'));
        assert!(!s.0.join("recs/film/tmdb-record.json").exists(), "only the stale copy is replaced");
        let doc: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(doc["plan"]["identity"]["original_language"], json!("es"));
        assert!(!out.contains(TOKEN) && !err.contains(TOKEN));
        assert_eq!(fs::read_dir(s.0.join("recs/film")).unwrap().count(), 6, "no temporary file is left");
    }

    #[test]
    fn decide_and_retune_apply_the_same_gate() {
        let s = Scratch::new("stale-gate");
        film(&s, &copy(STALE));
        let http = Arc::new(RecordedTransport::new());
        let ctx = ctx_with(&s, &http, false);
        let (code, _, err) = run(&ctx, &["decide", "--rec", "recs/film/eng-only"]);
        assert_eq!((code, err), (3, format!("lazymkv-dev decide: error: {}\n",
                                            line("tmdb.json", "TMDB_TOKEN is not set to fetch it again"))));
        let shown = "corpus/owner/film/tmdb.json: the TMDb copy is more than six months old (fetched_at 2026-01-01) and \
                     TMDB_TOKEN is not set to fetch it again\n";
        for argv in [&["retune", "--evidence", "recs", "--spec", SPEC][..],
                     &["retune", "--subtitles", "--evidence", "recs", "--spec", SPEC][..]] {
            assert_eq!(run(&ctx, argv), (3, String::new(), shown.to_string()), "{argv:?}");
        }
        assert!(http.requests().is_empty());
    }

    #[test]
    fn a_fresh_folder_asks_nothing() {
        let s = Scratch::new("stale-fresh");
        film(&s, &copy(FRESH));
        let http = Arc::new(RecordedTransport::new());
        let gated = run(&ctx_with(&s, &http, true), &["plan", "recs/film"]);
        assert_eq!(gated.0, 0);
        assert_eq!(gated, run(&Ctx::for_test(&s.0), &["plan", "recs/film"]), "byte-identical to a run with no transport");
        assert!(http.requests().is_empty());
    }

    #[test]
    fn a_failed_refetch_exits_3_with_the_reason_and_never_the_token() {
        for (fault, why) in [(None, "TMDb refused the token (HTTP 401)"), (Some(TransportError::Timeout), "no answer within 10 s")] {
            let s = Scratch::new("stale-failed");
            film(&s, &copy(STALE));
            let http = Arc::new(RecordedTransport::new());
            match fault { None => http.answer(&movie(), 401, b"{}"), Some(error) => http.fail(&movie(), error) }
            let (code, out, err) = run(&ctx_with(&s, &http, true), &["plan", "recs/film"]);
            assert_eq!((code, out.as_str()), (3, ""));
            assert_eq!(err, format!("lazymkv-dev plan: error: {}\n",
                                    line("tmdb.json", &format!("could not be fetched again: {why}"))));
            assert!(!err.contains(TOKEN));
            assert_eq!(fs::read_to_string(s.0.join("recs/film/tmdb.json")).unwrap(), copy(STALE));
        }
    }

    #[test]
    fn an_engine_job_folder_is_read_without_the_gate() {
        let s = Scratch::new("stale-job-folder");
        film(&s, &copy(STALE));
        s.write("recs/film/state.json", "{}");
        let http = Arc::new(RecordedTransport::new());
        let ctx = ctx_with(&s, &http, false);
        assert_eq!(run(&ctx, &["plan", "recs/film"]).0, 0);
        assert_eq!(run(&ctx, &["decide", "--rec", "recs/film/eng-only"]).0, 0);
        assert!(http.requests().is_empty());
        assert_eq!(fs::read_to_string(s.0.join("recs/film/tmdb.json")).unwrap(), copy(STALE));
        assert_eq!(refresh_copies(&ctx, &s.0.join("recs/film"), "recs/film"), Ok(vec![]));
    }

    #[test]
    fn a_copy_without_fetched_at_outside_a_job_folder_is_read_as_it_is() {
        let s = Scratch::new("stale-no-fetched-at");
        film(&s, r#"{"id": 6537, "title": "Made Up Film", "original_language": "es"}"#); // parse_recorded's id and title, no fetched_at
        let http = Arc::new(RecordedTransport::new());
        let ctx = ctx_with(&s, &http, false);
        assert_eq!(run(&ctx, &["plan", "recs/film"]).0, 0);
        // --allow-unmeasured: the synthetic evidence has no level folder, whose own exit 3 would hide the gate's.
        let (code, _, err) = run(&ctx, &["retune", "--evidence", "recs", "--spec", SPEC, "--allow-unmeasured"]);
        assert_ne!(code, 3, "{err}");
        assert!(!err.contains("TMDb copy"), "{err}");
        assert!(http.requests().is_empty());
    }
}

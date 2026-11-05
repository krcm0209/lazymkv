//! `lazymkv-dev census [--evidence DIR]`: one line per recording of the evidence clone, with its slug, disc type, kind,
//! TheDiscDB answer, English subtitle count, object-based audio and title (decisions.md A9, Ruling T4-R22). A report
//! only: it never fetches and never refuses a stale copy (Owner decision 24). Exit 0; 2 a usage error or a file that
//! could not be read; 3 no evidence clone.

use std::io::Write;

use lazymkv_core::codec::ObjectAudio;
use lazymkv_core::disc::{Disc, StreamKind, Title};
use lazymkv_core::lookup::tmdb::parse_recorded;
use lazymkv_core::lookup::Kind;
use lazymkv_core::recording::files::Meta;
use lazymkv_core::recording::{read_json, read_text, text_mode, Encoding, Errors, PathKey, Tree};
use lazymkv_core::robot::parse;
use lazymkv_core::settings::Settings;
use lazymkv_core::subtitle::labels::row14::{media, recorded_discdb, Media};

use crate::cmd::hit_rate::{statuses, FAILED, HIT, MISS, NOT_MADE};
use crate::cmd::plan::discdb_answer;
use crate::evidence::{os_error_text, resolve_evidence, Ctx, DiskTree, Refused, EVIDENCE_SHOWN};
use crate::{usage_error, Args};

/// The synopsis after "usage: lazymkv-dev " (decisions.md A9, Ruling T4-R22). Def: T4-16. Use: T4-16.
pub const USAGE: &str = "census [--evidence DIR]";

/// A recording's disc type: meta.json's `format` with the main title's video height (decisions.md A9, Ruling
/// T4-R22). Def: T4-16. Use: T4-16.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscType {
    Uhd,
    BluRay,
    Dvd,
}

impl DiscType {
    /// "UHD", "Blu-ray" or "DVD", as the census line prints it (decisions.md A9, Ruling T4-R22). Def: T4-16. Use: T4-16.
    pub fn as_str(self) -> &'static str {
        match self {
            DiscType::Uhd => "UHD",
            DiscType::BluRay => "Blu-ray",
            DiscType::Dvd => "DVD",
        }
    }
}

/// One recording's census columns (decisions.md A9, Ruling T4-R22). `slug` is the folder's name as hit-rate prints
/// it; `title` and `year` come from tmdb.json, else TheDiscDB's media; `kind` is `row14::media`'s (None when it is
/// unknown); `discdb` is "hit", "miss", "failed" or "n/a"; `english_subtitles` is None when the recording has no
/// scan.txt; `object_audio` holds Atmos before DTS:X, each once. Def: T4-16. Use: T4-16.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CensusRow {
    pub slug: String,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub kind: Option<Kind>,
    pub discdb: &'static str,
    pub english_subtitles: Option<usize>,
    pub object_audio: Vec<ObjectAudio>,
    pub disc_type: Option<DiscType>,
}

/// The disc's main title: the longest by seconds, the lower number on a tie; None when the disc has no title
/// (decisions.md A9, Ruling T4-R22). Def: T4-16. Use: T4-16.
pub fn main_title(disc: &Disc) -> Option<&Title> {
    let mut longest: Option<&Title> = None;
    // The titles come in number order, so a later title replaces the one kept only when it is strictly longer.
    for title in disc.titles.values() {
        if longest.map_or(true, |kept| title.seconds > kept.seconds) {
            longest = Some(title);
        }
    }
    longest
}

/// The census row of the recording `disc` (decisions.md A9, Ruling T4-R22): its meta.json, lookup-status.txt,
/// TheDiscDB answers (`plan::discdb_answer`), tmdb.json (`parse_recorded`, read when its `tmdb` status is 200 or
/// absent) and scan.txt. Err, naming the file as the tree shows it, when meta.json does not parse, lookup-status.txt is
/// not UTF-8, scan.txt cannot be read or `disc` refuses it, or a read tmdb.json does not parse. Def: T4-16.
/// Use: T4-16.
pub fn census_row(tree: &dyn Tree, disc: &PathKey) -> Result<CensusRow, String> {
    let meta_key = disc.join("meta.json");
    let meta: Meta = read_json(tree, &meta_key).map_err(|error| format!("{}: {}", tree.display(&meta_key), error))?;
    let status_key = disc.join("lookup-status.txt");
    let status = statuses(tree, disc).map_err(|error| format!("{}: {}", tree.display(&status_key), error))?;
    let status_of = |lookup: &str| status.get(lookup).map(String::as_str);
    let by_hash = discdb_answer(tree, &disc.join("thediscdb-hash.json"), status_of("dischash"), false);
    let by_id = discdb_answer(tree, &disc.join("thediscdb-discid.json"), status_of("discid"), true);
    let discdb = if by_hash == HIT || by_id == HIT {
        HIT
    } else if by_hash == MISS && (by_id == MISS || by_id == NOT_MADE) {
        MISS
    } else if by_hash == FAILED || by_id == FAILED {
        FAILED
    } else {
        NOT_MADE
    };
    let tmdb_key = disc.join("tmdb.json");
    let recorded = if tree.is_file(&tmdb_key) && matches!(status_of("tmdb"), None | Some("200")) {
        let shown = tree.display(&tmdb_key);
        let text = read_text(tree, &tmdb_key, Encoding::Utf8Sig, Errors::Strict).map_err(|error| format!("{shown}: {error}"))?;
        Some(parse_recorded(&text, &shown)?)
    } else {
        None
    };
    let (title, year) = match recorded {
        Some(record) => (Some(record.title), record.year),
        None => match recorded_discdb(tree, disc).and_then(|record| record.title_and_year()) {
            Some((title, year)) => (Some(title), year),
            None => (None, None),
        },
    };
    let scan_key = disc.join("scan.txt");
    let typed = if tree.is_file(&scan_key) {
        let shown = tree.display(&scan_key);
        let text = read_text(tree, &scan_key, Encoding::Utf8, Errors::Replace).map_err(|error| format!("{shown}: {error}"))?;
        Some(lazymkv_core::disc::disc(&parse(&text_mode(&text))).map_err(|sentence| format!("{shown}: {sentence}"))?)
    } else {
        None
    };
    let kind = match media(tree, disc, typed.as_ref(), &Settings::default()).0 {
        Media::Film => Some(Kind::Movie),
        Media::Tv => Some(Kind::Tv),
        Media::Unknown => None,
    };
    let main = typed.as_ref().and_then(main_title);
    let english_subtitles = typed.as_ref().map(|_| {
        main.map_or(0, |title| {
            title
                .streams_of(StreamKind::Subtitles)
                .into_iter()
                .filter(|stream| stream.language.as_deref() == Some("eng") && !stream.flags.derived_forced_only())
                .count()
        })
    });
    let objects: Vec<ObjectAudio> =
        main.map_or_else(Vec::new, |title| title.streams_of(StreamKind::Audio).into_iter().filter_map(|stream| stream.codec.object).collect());
    let object_audio = [ObjectAudio::Atmos, ObjectAudio::DtsX].into_iter().filter(|object| objects.contains(object)).collect();
    let disc_type = match meta.format.as_deref() {
        Some("dvd") => Some(DiscType::Dvd),
        Some("bluray") if typed.is_some() => {
            let height = main.and_then(Title::video_height);
            Some(if height.map_or(false, |height| height >= 2000) { DiscType::Uhd } else { DiscType::BluRay })
        }
        _ => None,
    };
    Ok(CensusRow {
        slug: disc.name().to_string(),
        title,
        year,
        kind,
        discdb,
        english_subtitles,
        object_audio,
        disc_type,
    })
}

/// The census line of one row, without its newline: the slug, disc type, kind, TheDiscDB word, English subtitle
/// count, object-based audio (`?` without a scan, `-` when none) and `<title> (<year>)`, each unknown column `?`
/// (decisions.md A9, Ruling T4-R22). Def: T4-16. Use: T4-16.
pub fn line(row: &CensusRow) -> String {
    let disc_type = row.disc_type.map_or("?", DiscType::as_str);
    let kind = match row.kind {
        Some(Kind::Movie) => "movie",
        Some(Kind::Tv) => "tv",
        None => "?",
    };
    let subs = row.english_subtitles.map_or_else(|| "?".to_string(), |count| count.to_string());
    // The object column prints `?` when `english_subtitles` is None because both come from the scan.
    let object = if row.english_subtitles.is_none() {
        "?".to_string()
    } else if row.object_audio.is_empty() {
        "-".to_string()
    } else {
        row.object_audio.iter().map(|object| object.as_str()).collect::<Vec<_>>().join("+")
    };
    let title = match (&row.title, row.year) {
        (Some(title), Some(year)) => format!("{title} ({year})"),
        (Some(title), None) => title.clone(),
        (None, _) => "?".to_string(),
    };
    format!(
        "{:<40} {:<7} {:<5} discdb={:<6} eng-subs={:<2} object={:<11} {}",
        row.slug, disc_type, kind, row.discdb, subs, object, title
    )
}

/// Writes one census line per recording directly in `root` (each entry holding meta.json, in `PathKey` order, as
/// hit-rate lists them) as it goes, and returns 0; Err after the lines written so far when a row cannot be read. No
/// summary line follows the rows (decisions.md A9, Ruling T4-R22). Def: T4-16. Use: T4-16.
pub fn report(tree: &dyn Tree, root: &PathKey, out: &mut dyn Write) -> Result<i32, String> {
    let recordings = tree.list(root);
    // `Tree::list` already gives PathKey order, so no sort follows.
    for disc in recordings {
        if !tree.exists(&disc.join("meta.json")) {
            continue;
        }
        let row = census_row(tree, &disc)?;
        let _ = writeln!(out, "{}", line(&row));
    }
    Ok(0)
}

/// `lazymkv-dev census [--evidence DIR]` (decisions.md A9, Ruling T4-R22): the evidence from --evidence, else
/// LAZYMKV_EVIDENCE, else corpus/owner; its refusal is printed with its code (3 without the clone). A row that cannot
/// be read prints its text and exits 2 after the lines already written. Def: T4-16. Use: T4-16, T4-20.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(args, &["--evidence"]);
    let flag = match parsed.flag("--evidence") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "census", USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, "census", USAGE, &message),
    };
    if !words.is_empty() {
        return usage_error(err, "census", USAGE, &format!("unrecognized arguments: {}", words.join(" ")));
    }
    let dir = match resolve_evidence(ctx, flag.as_deref()) {
        Ok(dir) => dir,
        Err(Refused { code, line }) => {
            let _ = writeln!(err, "{line}");
            return code;
        }
    };
    let tree = match DiskTree::open(&dir, EVIDENCE_SHOWN) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = writeln!(err, "{}", os_error_text(&error, &dir.display().to_string()));
            return 2;
        }
    };
    match report(&tree, &PathKey::root(), out) {
        Ok(code) => code,
        Err(text) => {
            let _ = writeln!(err, "{text}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{census_row, main_title, run};
    use crate::evidence::{Ctx, EVIDENCE_URL};
    use crate::testutil::{call, printed, Scratch, ScratchExt};
    use lazymkv_core::disc::disc;
    use lazymkv_core::recording::{text_mode, PathKey};
    use lazymkv_core::robot::parse;
    use std::path::Path;

    const SMILE_SCAN: &str = "corpus/thediscdb/movie/Smile (2022)/2022-4k/disc01.txt";
    const KNOWING_SCAN: &str = "corpus/thediscdb/movie/Knowing (2009)/2010-dvd/disc01.txt";

    /// A public file of the repository as text (RP21's public list, crates/dev/tests/clone_values.rs).
    fn public(relative: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative)).unwrap()
    }

    /// The five recordings of Ruling T4-R22, built from public files.
    fn public_recordings(tmp: &Scratch) {
        tmp.write("disc-a/meta.json", r#"{"format": "bluray"}"#);
        tmp.write("disc-a/lookup-status.txt", "dischash 404\n");
        tmp.write("disc-a/thediscdb-hash.json", "");
        tmp.write("disc-a/scan.txt", &public("corpus/scenarios/disc-a-scan/scan.txt"));
        tmp.write("smile/meta.json", r#"{"format": "bluray"}"#);
        tmp.write("smile/lookup-status.txt", "dischash 200\n");
        tmp.write("smile/thediscdb-hash.json", &public("corpus/thediscdb-api/dischash-E826F0C64B04079EA83094AC046E8709.json"));
        tmp.write("smile/scan.txt", &public(SMILE_SCAN));
        tmp.write("knowing/meta.json", r#"{"format": "dvd"}"#);
        tmp.write("knowing/lookup-status.txt", "dischash 200\n");
        tmp.write("knowing/thediscdb-hash.json", &public("corpus/thediscdb-api/dischash-631BD77872C157D7C436E6F48F06A478.json"));
        tmp.write("knowing/scan.txt", &public(KNOWING_SCAN));
        tmp.write("synthetic-movie/meta.json", r#"{"format": "bluray"}"#);
        tmp.write("synthetic-movie/lookup-status.txt", "dischash 404\ndiscid 404\ntmdb movie/900001 200\n");
        tmp.write("synthetic-movie/tmdb.json", r#"{"id": 900001, "title": "Synthetic Movie", "original_title": "Synthetic Movie", "release_date": "2001-02-03", "original_language": "en", "external_ids": {"imdb_id": "tt0900001"}, "fetched_at": "2026-10-01"}"#);
        tmp.write("synthetic-movie/scan.txt", &public("corpus/scenarios/movie-direct/scan.txt"));
        tmp.write("not-a-disc/meta.json", r#"{"format": "dvd"}"#);
        tmp.write("not-a-disc/lookup-status.txt", "dischash 200\n");
        tmp.write("not-a-disc/thediscdb-hash.json", r#"{"errors": "x"}"#);
        tmp.write("no-meta/scan.txt", &public("corpus/scenarios/movie-direct/scan.txt"));
        tmp.write("a-file.txt", "not a recording");
    }

    fn census(dir: &Path) -> (i32, String, String) {
        call(run, &Ctx::for_test(dir), &["--evidence", &dir.to_string_lossy()])
    }

    #[test]
    fn census_lists_each_public_recording_with_its_columns() {
        let tmp = Scratch::new("census-public");
        public_recordings(&tmp);
        let (code, out, err) = census(&tmp.0);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, printed(&[
            "disc-a                                   UHD     movie discdb=miss   eng-subs=1  object=-           ?",
            "knowing                                  DVD     movie discdb=hit    eng-subs=2  object=-           Knowing (2009)",
            "not-a-disc                               DVD     ?     discdb=miss   eng-subs=?  object=?           ?",
            "smile                                    UHD     movie discdb=hit    eng-subs=3  object=Atmos       Smile (2022)",
            "synthetic-movie                          Blu-ray movie discdb=miss   eng-subs=0  object=-           Synthetic Movie (2001)",
        ]));
    }

    #[test]
    fn census_reads_a_dischash_200_that_is_no_disc_as_a_miss() {
        let tmp = Scratch::new("census-s46");
        for (name, body) in [("empty", ""), ("errors", r#"{"errors": "x"}"#), ("no-titles", r#"{"titles": []}"#)] {
            tmp.write(&format!("{name}/meta.json"), r#"{"format": "dvd"}"#);
            tmp.write(&format!("{name}/lookup-status.txt"), "dischash 200\n");
            tmp.write(&format!("{name}/thediscdb-hash.json"), body);
        }
        tmp.write("broken/meta.json", r#"{"format": "dvd"}"#);
        tmp.write("broken/lookup-status.txt", "dischash 200\n");
        tmp.write("broken/thediscdb-hash.json", "not json");
        let tree = crate::evidence::DiskTree::open(&tmp.0, "evidence").unwrap();
        let word = |name: &str| census_row(&tree, &PathKey::root().join(name)).unwrap().discdb;
        assert_eq!([word("empty"), word("errors"), word("no-titles"), word("broken")], ["miss", "miss", "miss", "failed"]);
    }

    #[test]
    fn census_takes_the_longest_title_and_the_lower_number_on_a_tie() {
        let smile = disc(&parse(&text_mode(&public(SMILE_SCAN)))).unwrap();
        assert_eq!(main_title(&smile).map(|title| title.number), Some(14)); // 14 and 122 both run 1:55:37
        let knowing = disc(&parse(&text_mode(&public(KNOWING_SCAN)))).unwrap();
        assert_eq!(main_title(&knowing).map(|title| title.number), Some(0));
    }

    #[test]
    fn census_counts_english_subtitles_without_derived_forced_only_streams() {
        let tmp = Scratch::new("census-subs");
        public_recordings(&tmp);
        let tree = crate::evidence::DiskTree::open(&tmp.0, "evidence").unwrap();
        let subs = |name: &str| census_row(&tree, &PathKey::root().join(name)).unwrap().english_subtitles;
        assert_eq!(subs("disc-a"), Some(1)); // eng flags 0 counted, eng flags 6144 (4096 and 2048) left out
        assert_eq!(subs("knowing"), Some(2)); // eng flags 0 and eng flags 1 (a commentary flag) both counted
    }

    #[test]
    fn census_lists_a_recording_without_scan_txt_with_question_marks_and_exits_0() {
        let tmp = Scratch::new("census-noscan");
        tmp.write("not-a-disc/meta.json", r#"{"format": "bluray"}"#);
        let (code, out, err) = census(&tmp.0);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, printed(&[
            "not-a-disc                               ?       ?     discdb=n/a    eng-subs=?  object=?           ?",
        ]));
    }

    #[test]
    fn census_exits_2_naming_a_meta_json_that_does_not_parse() {
        let tmp = Scratch::new("census-broken");
        public_recordings(&tmp);
        tmp.write("zz-broken/meta.json", "{");
        let (code, out, err) = census(&tmp.0);
        assert_eq!(code, 2);
        assert_eq!(out.lines().count(), 5, "{out}"); // the rows before the broken one are kept
        assert!(err.contains("zz-broken/meta.json"), "{err}");
    }

    #[test]
    fn census_without_the_evidence_clone_exits_3() {
        let tmp = Scratch::new("census-absent");
        let absent = tmp.0.join("absent").to_string_lossy().into_owned();
        let (code, out, err) = call(run, &Ctx::for_test(&tmp.0), &["--evidence", &absent]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert_eq!(err, printed(&[&format!(
            "{absent}: no such folder; the evidence repository lazymkv-evidence is not cloned there (git clone {EVIDENCE_URL} {absent})"
        )]));
    }

    #[test]
    fn census_is_reached_through_its_row_of_the_command_table() {
        let row = crate::COMMANDS.iter().find(|(name, _, _)| *name == "census").map(|(_, _, synopsis)| *synopsis);
        assert_eq!(row, Some("[--evidence DIR]"));
        assert_eq!(crate::COMMANDS.last().map(|(name, _, _)| *name), Some("census"));
        let tmp = Scratch::new("census-dispatch");
        public_recordings(&tmp);
        let argv: Vec<String> = ["census", "--evidence", &tmp.0.to_string_lossy()].iter().map(|w| w.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(crate::dispatch(&Ctx::for_test(&tmp.0), &argv, &mut out, &mut err), 0);
        assert_eq!(String::from_utf8(out).unwrap(), census(&tmp.0).1);
    }
}

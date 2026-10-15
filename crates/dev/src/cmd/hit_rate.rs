//! `lazymkv-dev hit-rate <dir>`: experiment 10, how many recorded discs does TheDiscDB know?
//! Py: hit_rate, its lines verbatim. Exit 0 when there is a recording and every lookup answered; 1 otherwise; 2
//! where the Python crashed (no argument; the folder or a meta.json unreadable; a lookup-status.txt not UTF-8).

use crate::evidence::{Ctx, DiskTree};
use crate::usage_error;
use lazymkv_core::recording::files::{lookup_statuses, thediscdb_answer, DiscDbAnswer, Meta};
use lazymkv_core::recording::{decode_text, read_json, Encoding, Errors, Ordered, PathKey, Tree};
use std::io::Write;

/// The command's name as typed.
const NAME: &str = "hit-rate";
/// Its usage after "usage: lazymkv-dev ". The Python read sys.argv[1] and ignored any further word; so does this.
const USAGE: &str = "hit-rate <dir>";
/// A missing folder argument (the Python crashed with an IndexError).
const REQUIRED: &str = "the following arguments are required: dir";

/// How a lookup ended. Py: lookup's strings.
const HIT: &str = "hit";
const MISS: &str = "miss";
const FAILED: &str = "failed";
const NOT_MADE: &str = "n/a";

/// HTTP status per lookup, from the recorder's lookup-status.txt ({"dischash": "200"}); empty when the file cannot be
/// read (the Python's OSError); Err when it is not UTF-8 (the Python crashed). Py: hit_rate.statuses.
pub fn statuses(tree: &dyn Tree, disc: &PathKey) -> Result<Ordered<String>, String> {
    let bytes = match tree.read(&disc.join("lookup-status.txt")) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(Ordered::new()),
    };
    let text = decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict)?;
    Ok(lookup_statuses(&text))
}

/// "hit", "miss", "failed" or "n/a" for the answer file `path` given the lookup's HTTP status. Only an answer from
/// TheDiscDB counts as a miss. Py: hit_rate.lookup.
pub fn lookup(tree: &dyn Tree, path: &PathKey, status: Option<&str>) -> &'static str {
    match status {
        // the lookup was not made, for example the disc has no disc ID
        None => NOT_MADE,
        Some("404") => MISS,
        // network error, rate limit or server error
        Some(status) if status != "200" => FAILED,
        Some(_) => match read_json::<serde_json::Value>(tree, path) {
            Err(_) => FAILED,
            Ok(value) => match thediscdb_answer(&value) {
                DiscDbAnswer::Hit => HIT,
                DiscDbAnswer::Miss => MISS,
                // a 200 answer that is not a disc record
                DiscDbAnswer::NotADisc => FAILED,
            },
        },
    }
}

/// Writes hit_rate.main's lines for the recordings directly in `root` as it goes, and returns the exit code; Err
/// where the Python crashed, after the lines written so far. Py: hit_rate.main.
pub fn report(tree: &dyn Tree, root: &PathKey, out: &mut dyn Write) -> Result<i32, String> {
    let mut rows: Vec<(&'static str, &'static str)> = Vec::new();
    for disc in tree.list(root) {
        if !tree.exists(&disc.join("meta.json")) {
            continue;
        }
        let meta: Meta = read_json(tree, &disc.join("meta.json"))?;
        let status = statuses(tree, &disc)?;
        let by_hash = lookup(tree, &disc.join("thediscdb-hash.json"), status.get("dischash").map(String::as_str));
        let by_id = lookup(tree, &disc.join("thediscdb-discid.json"), status.get("discid").map(String::as_str));
        rows.push((by_hash, by_id));
        // str(meta.get('format')): the format, or "None" without one.
        let format = meta.format.as_deref().unwrap_or("None");
        let _ = writeln!(out, "{:<40} {:<7} hash={:<6} discid={}", disc.name(), format, by_hash, by_id);
    }
    let answered: Vec<&str> = rows.iter().map(|(by_hash, _)| *by_hash).filter(|by_hash| *by_hash == HIT || *by_hash == MISS).collect();
    let hits = answered.iter().filter(|by_hash| **by_hash == HIT).count();
    let unanswered = rows.len() - answered.len();
    let id_failed = rows.iter().filter(|(_, by_id)| *by_id == FAILED).count();
    let either = rows.iter().filter(|(by_hash, by_id)| *by_hash == HIT || *by_id == HIT).count();
    let _ = writeln!(out);
    let _ = writeln!(out, "TheDiscDB knows {hits} of {} discs by fingerprint", answered.len());
    let _ = writeln!(out, "by fingerprint or disc ID: {either} of {} discs", rows.len());
    if unanswered > 0 {
        let _ = writeln!(out, "{unanswered} fingerprint lookups failed or were not made; they are not counted as misses");
    }
    if id_failed > 0 {
        let _ = writeln!(out, "{id_failed} disc ID lookups failed; the combined figure may be too low");
    }
    Ok(if !rows.is_empty() && unanswered == 0 && id_failed == 0 { 0 } else { 1 })
}

/// `lazymkv-dev hit-rate <dir>`. Py: hit_rate's `sys.exit(main(sys.argv[1]))`.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(dir) = args.first() else {
        return usage_error(err, NAME, USAGE, REQUIRED);
    };
    if dir == "-h" || dir == "--help" {
        let _ = writeln!(out, "usage: lazymkv-dev {USAGE}");
        return 0;
    }
    let tree = match DiskTree::open(&ctx.abs(dir), dir) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = writeln!(err, "lazymkv-dev {NAME}: error: {dir}: {error}");
            return 2;
        }
    };
    match report(&tree, &PathKey::root(), out) {
        Ok(code) => code,
        Err(message) => {
            let _ = writeln!(err, "lazymkv-dev {NAME}: error: {message}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::evidence::Ctx;
    use crate::testutil::{printed, Scratch as TempDir, ScratchExt};
    use std::path::Path;

    /// A recording with meta.json, lookup-status.txt and the answers given. Py: HitRate.disc.
    fn disc(tmp: &TempDir, name: &str, status: &str, hash_body: Option<&str>, id_body: Option<&str>) {
        tmp.write(&format!("{name}/meta.json"), r#"{"format": "UHD"}"#);
        tmp.write(&format!("{name}/lookup-status.txt"), status);
        if let Some(body) = hash_body {
            tmp.write(&format!("{name}/thediscdb-hash.json"), body);
        }
        if let Some(body) = id_body {
            tmp.write(&format!("{name}/thediscdb-discid.json"), body);
        }
    }

    /// The command over `dir`: (exit code, stdout, stderr). Py: quiet(hit_rate.main, dir).
    fn quiet(dir: &Path) -> (i32, String, String) {
        let ctx = Ctx::for_test(dir);
        let args = vec![dir.to_string_lossy().into_owned()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&ctx, &args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// Two recordings whose every lookup answered: a hit by both, a miss by both.
    fn answered(tmp: &TempDir) {
        disc(tmp, "a", "dischash 200\ndiscid 200\n", Some(r#"{"titles": [{"index": 0}]}"#), Some(r#"[{"titles": []}]"#));
        disc(tmp, "b", "dischash 404\ndiscid 200\n", Some(r#"{"title": "Not Found"}"#), Some("[]"));
    }

    #[test]
    fn hit_rate_hits_and_misses_are_counted_when_every_lookup_answered() {
        let tmp = TempDir::new("answered");
        answered(&tmp);
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("TheDiscDB knows 1 of 2 discs by fingerprint"), "{out}");
        assert!(out.contains("by fingerprint or disc ID: 1 of 2 discs"), "{out}");
    }

    #[test]
    fn hit_rate_a_failed_lookup_is_not_a_miss() {
        let tmp = TempDir::new("failed");
        disc(&tmp, "a", "dischash 500\n", Some(r#"{"errors": "x"}"#), None);
        disc(&tmp, "b", "dischash 200\n", Some("not json"), None);
        disc(&tmp, "c", "", None, None);
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(!out.contains("hash=miss"), "{out}");
        assert!(out.contains("TheDiscDB knows 0 of 0 discs by fingerprint"), "{out}");
        assert!(out.contains("3 fingerprint lookups failed or were not made"), "{out}");
    }

    #[test]
    fn hit_rate_a_failed_disc_id_lookup_is_reported() {
        let tmp = TempDir::new("disc-id");
        disc(&tmp, "a", "dischash 404\ndiscid 429\n", Some(r#"{"title": "Not Found"}"#), Some("rate limited"));
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("hash=miss"), "{out}");
        assert!(out.contains("discid=failed"), "{out}");
        assert!(out.contains("1 disc ID lookups failed"), "{out}");
    }

    #[test]
    fn hit_rate_a_200_answer_that_is_not_a_disc_record_is_a_failure() {
        let tmp = TempDir::new("not-a-disc");
        disc(&tmp, "a", "dischash 200\n", Some(r#"{"errors": "x"}"#), None);
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("hash=failed"), "{out}");
    }

    #[test]
    fn hit_rate_a_recording_without_a_status_file_is_not_a_miss() {
        let tmp = TempDir::new("no-status");
        tmp.write("a/meta.json", r#"{"format": "DVD"}"#);
        tmp.write("a/thediscdb-hash.json", r#"{"titles": [{"index": 0}]}"#);
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("hash=n/a"), "{out}");
        assert!(out.contains("TheDiscDB knows 0 of 0 discs by fingerprint"), "{out}");
    }

    #[test]
    fn hit_rate_prints_the_pythons_lines() {
        let tmp = TempDir::new("lines");
        answered(&tmp);
        let (code, out, err) = quiet(&tmp.0);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            printed(&[
                "a                                        UHD     hash=hit    discid=hit",
                "b                                        UHD     hash=miss   discid=miss",
                "",
                "TheDiscDB knows 1 of 2 discs by fingerprint",
                "by fingerprint or disc ID: 1 of 2 discs",
            ])
        );
    }

    #[test]
    fn hit_rate_prints_none_for_a_meta_without_a_format() {
        let tmp = TempDir::new("no-format");
        tmp.write("a/meta.json", r#"{"volume_label": "X"}"#);
        tmp.write("a/thediscdb-hash.json", r#"{"titles": [{"index": 0}]}"#);
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert_eq!(
            out,
            printed(&[
                "a                                        None    hash=n/a    discid=n/a",
                "",
                "TheDiscDB knows 0 of 0 discs by fingerprint",
                "by fingerprint or disc ID: 0 of 1 discs",
                "1 fingerprint lookups failed or were not made; they are not counted as misses",
            ])
        );
    }

    #[test]
    fn hit_rate_takes_recordings_in_python_path_order() {
        // sorted(Path objects): code point order, so "C" before "a" and "a" before "a-2"; a file and a folder
        // without meta.json are not recordings.
        let tmp = TempDir::new("order");
        for name in ["b", "a-2", "C", "a"] {
            disc(&tmp, name, "dischash 404\n", None, None);
        }
        tmp.write("notes.md", "x");
        std::fs::create_dir_all(tmp.0.join("plain")).unwrap();
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 0, "{out}");
        assert_eq!(
            out,
            printed(&[
                "C                                        UHD     hash=miss   discid=n/a",
                "a                                        UHD     hash=miss   discid=n/a",
                "a-2                                      UHD     hash=miss   discid=n/a",
                "b                                        UHD     hash=miss   discid=n/a",
                "",
                "TheDiscDB knows 0 of 4 discs by fingerprint",
                "by fingerprint or disc ID: 0 of 4 discs",
            ])
        );
    }

    #[test]
    fn hit_rate_without_a_readable_folder_exits_2() {
        let ctx = Ctx::for_test(Path::new("."));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(run(&ctx, &[], &mut out, &mut err), 2);
        assert!(out.is_empty());
        assert_eq!(
            String::from_utf8(err).unwrap(),
            printed(&[
                "usage: lazymkv-dev hit-rate <dir>",
                "lazymkv-dev hit-rate: error: the following arguments are required: dir",
            ])
        );
        let tmp = TempDir::new("broken");
        tmp.write("a/meta.json", "not json");
        let (code, out, err) = quiet(&tmp.0);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("lazymkv-dev hit-rate: error: "), "{err}");
        assert_eq!(quiet(&tmp.0.join("missing")).0, 2);
    }
}

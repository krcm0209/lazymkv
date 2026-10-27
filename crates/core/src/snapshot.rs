//! The pure part of a disc snapshot (R-B2, design part 4.1): which files are navigation files, which file is the AACS
//! unit key, and meta.json's values from a listing. No I/O (RP19); tools::snapshot reads and writes the files
//! (Task 12).

use crate::fingerprint::{aacs_disc_id, content_hash, disc_format, DiscFile, DiscFormat};

/// meta.json as phase 0's capture writes it (spec 15 "Recordings", R-B2), keys in this order. This is the writer's type;
/// the reader stays `recording::files::Meta`, because merging them would print `captured_unix` as a float (S9).
/// Def: T3. Use: T12, T22.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DiscMeta {
    /// The volume's label, as the caller read it.
    pub volume_label: String,
    /// "bluray" or "dvd" from `disc_format`, None when it gives neither.
    pub format: Option<String>,
    /// TheDiscDB's content hash, `content_hash` of the listing (spec 5).
    pub fingerprint: Option<String>,
    /// The AACS disc ID, `aacs_disc_id` of the unit key file when the caller read one.
    pub disc_id: Option<String>,
    /// How many files the listing holds.
    pub file_count: usize,
    /// The listing's files' sizes, summed.
    pub total_bytes: u64,
    /// When the snapshot was taken, in Unix seconds.
    pub captured_unix: u64,
}

/// True for the small unencrypted navigation files a recording keeps under `nav/`: `BDMV/PLAYLIST/*.mpls`,
/// `BDMV/index.bdmv`, `BDMV/MovieObject.bdmv`, `BDMV/META/DL/*.xml` and `VIDEO_TS/*.IFO`, ignoring ASCII case
/// (R-B2, from capture's disc.rs:57). Def: T3. Use: T12.
pub fn is_nav_file(relative_path: &str) -> bool {
    let path = relative_path.to_ascii_uppercase();
    (path.starts_with("BDMV/PLAYLIST/") && path.ends_with(".MPLS"))
        || path == "BDMV/INDEX.BDMV"
        || path == "BDMV/MOVIEOBJECT.BDMV"
        || (path.starts_with("BDMV/META/DL/") && path.ends_with(".XML"))
        || (path.starts_with("VIDEO_TS/") && path.ends_with(".IFO"))
}

/// The AACS unit key file of a listing: `AACS/Unit_Key_RO.inf`, else `AACS/DUPLICATE/Unit_Key_RO.inf`, matched
/// ignoring ASCII case (R-B2, from capture's disc.rs:66). Def: T3. Use: T12, T22.
pub fn unit_key_file(files: &[DiscFile]) -> Option<&DiscFile> {
    ["AACS/UNIT_KEY_RO.INF", "AACS/DUPLICATE/UNIT_KEY_RO.INF"]
        .iter()
        .find_map(|wanted| files.iter().find(|file| file.path.to_ascii_uppercase() == *wanted))
}

/// meta.json's values for a listing (spec 5, R-B2):
/// - `format` is "bluray" or "dvd" from `disc_format`;
/// - `fingerprint` is `content_hash`;
/// - `disc_id` is `aacs_disc_id(unit_key)` when the caller read the unit key, else None;
/// - `file_count` and `total_bytes` count `files`;
/// - `captured_unix` is passed in, because core reads no clock (RP19).
///
/// Def: T3. Use: T12, T22.
pub fn disc_meta(files: &[DiscFile], volume_label: &str, unit_key: Option<&[u8]>, captured_unix: u64) -> DiscMeta {
    DiscMeta {
        volume_label: volume_label.to_string(),
        format: disc_format(files).map(|format| match format {
            DiscFormat::BluRay => "bluray".to_string(),
            DiscFormat::Dvd => "dvd".to_string(),
        }),
        fingerprint: content_hash(files),
        disc_id: unit_key.map(aacs_disc_id),
        file_count: files.len(),
        total_bytes: files.iter().map(|file| file.size).sum(),
        captured_unix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::DiscFile;

    fn file(path: &str, size: u64) -> DiscFile {
        DiscFile { path: path.into(), size }
    }

    #[test]
    fn disc_meta_gives_phase_0s_seven_keys_in_order() {
        let files = [file("AACS/Unit_Key_RO.inf", 3), file("BDMV/STREAM/00000.m2ts", 100), file("BDMV/STREAM/00001.m2ts", 300)];
        let meta = disc_meta(&files, "MY_DISC", Some(&b"abc"[..]), 7);
        assert_eq!(
            serde_json::to_string(&meta).unwrap(),
            concat!(
                r#"{"volume_label":"MY_DISC","format":"bluray","fingerprint":"881150E67DD260B285FD74856BFEEBE2","#,
                r#""disc_id":"A9993E364706816ABA3E25717850C26C9CD0D89D","file_count":3,"total_bytes":403,"captured_unix":7}"#
            )
        );
    }

    #[test]
    fn a_dvd_has_format_dvd_and_no_disc_id_without_a_unit_key() {
        let files = [file("VIDEO_TS/VIDEO_TS.IFO", 10), file("VIDEO_TS/VTS_01_1.VOB", 2000)];
        assert_eq!(
            disc_meta(&files, "DVD_DISC", None, 0),
            DiscMeta {
                volume_label: "DVD_DISC".into(),
                format: Some("dvd".into()),
                fingerprint: Some("106D7957633694DC87F528C84A4C73FA".into()),
                disc_id: None,
                file_count: 2,
                total_bytes: 2010,
                captured_unix: 0,
            }
        );
        let empty = disc_meta(&[], "", None, 0);
        assert_eq!((empty.format, empty.fingerprint, empty.file_count, empty.total_bytes), (None, None, 0, 0));
    }

    #[test]
    fn the_unit_key_is_found_in_either_aacs_folder() {
        let main = file("AACS/Unit_Key_RO.inf", 3);
        let duplicate = file("AACS/DUPLICATE/Unit_Key_RO.inf", 4);
        let found = |files: &[DiscFile]| unit_key_file(files).map(|file| file.path.clone());
        assert_eq!(found(&[duplicate.clone(), main.clone()]), Some(main.path.clone()), "the main folder wins");
        assert_eq!(found(&[duplicate.clone()]), Some(duplicate.path.clone()));
        assert_eq!(found(&[file("aacs/unit_key_ro.inf", 3)]), Some("aacs/unit_key_ro.inf".to_string()));
        assert_eq!(found(&[file("AACS/Unit_Key_RW.inf", 3)]), None);
        assert_eq!(found(&[]), None);
    }

    #[test]
    fn navigation_files_are_recognised() {
        assert!(is_nav_file("BDMV/PLAYLIST/00800.mpls"));
        assert!(is_nav_file("BDMV/index.bdmv"));
        assert!(is_nav_file("BDMV/MovieObject.bdmv"));
        assert!(is_nav_file("BDMV/META/DL/bdmt_eng.xml"));
        assert!(is_nav_file("VIDEO_TS/VTS_01_0.IFO"));
        assert!(!is_nav_file("BDMV/STREAM/00001.m2ts"));
        assert!(!is_nav_file("VIDEO_TS/VTS_01_1.VOB"));
        assert!(!is_nav_file("AACS/Unit_Key_RO.inf"));
    }
}

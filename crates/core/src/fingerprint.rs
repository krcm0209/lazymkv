//! Disc fingerprints computed from a file listing and file bytes handed in.

use md5::{Digest, Md5};
use sha1::Sha1;

/// One file on a disc: its path relative to the disc root, and its size in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscFile {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscFormat {
    BluRay,
    Dvd,
}

fn upper_parts(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_uppercase())
        .collect()
}

/// Blu-ray and UHD discs have files in `BDMV/STREAM`; DVDs have `VIDEO_TS`.
pub fn disc_format(files: &[DiscFile]) -> Option<DiscFormat> {
    let mut dvd = false;
    for file in files {
        let parts = upper_parts(&file.path);
        if parts.len() == 3 && parts[0] == "BDMV" && parts[1] == "STREAM" {
            return Some(DiscFormat::BluRay);
        }
        if parts.len() == 2 && parts[0] == "VIDEO_TS" {
            dvd = true;
        }
    }
    dvd.then_some(DiscFormat::Dvd)
}

/// TheDiscDB's content hash: the MD5 of the file sizes, each as a little-endian
/// 64-bit integer, of `BDMV/STREAM/*.m2ts` (Blu-ray, UHD) or of every file in
/// `VIDEO_TS` (DVD), sorted by file name. Returned as uppercase hex.
pub fn content_hash(files: &[DiscFile]) -> Option<String> {
    let format = disc_format(files)?;
    let mut hashed: Vec<(String, u64)> = files
        .iter()
        .filter_map(|file| {
            let parts = upper_parts(&file.path);
            match format {
                DiscFormat::BluRay
                    if parts.len() == 3
                        && parts[0] == "BDMV"
                        && parts[1] == "STREAM"
                        && parts[2].ends_with(".M2TS") =>
                {
                    Some((parts[2].clone(), file.size))
                }
                DiscFormat::Dvd if parts.len() == 2 && parts[0] == "VIDEO_TS" => {
                    Some((parts[1].clone(), file.size))
                }
                _ => None,
            }
        })
        .collect();
    if hashed.is_empty() {
        return None;
    }
    hashed.sort();
    let mut md5 = Md5::new();
    for (_, size) in &hashed {
        md5.update((*size as i64).to_le_bytes());
    }
    Some(hex_upper(md5.finalize().as_slice()))
}

/// The AACS disc ID: the SHA-1 of the bytes of `AACS/Unit_Key_RO.inf`, as uppercase hex.
pub fn aacs_disc_id(unit_key_ro: &[u8]) -> String {
    hex_upper(Sha1::digest(unit_key_ro).as_slice())
}

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bd(name: &str, size: u64) -> DiscFile {
        DiscFile { path: format!("BDMV/STREAM/{name}"), size }
    }

    // File sizes from TheDiscDB's public log for John Wick (2014), 4K disc.
    // Its recorded ContentHash is the expected value.
    fn john_wick() -> Vec<DiscFile> {
        vec![
            bd("00009.m2ts", 2_979_840),
            bd("00007.m2ts", 1_420_118_016),
            bd("00006.m2ts", 1_522_556_928),
            bd("00005.m2ts", 1_256_460_288),
            bd("00004.m2ts", 1_498_521_600),
            bd("00002.m2ts", 3_618_170_880),
            bd("00003.m2ts", 2_834_472_960),
            bd("00001.m2ts", 67_757_930_496),
            bd("00000.m2ts", 38_842_368),
            bd("00011.m2ts", 135_168),
            bd("00010.m2ts", 2_961_408),
            bd("00008.m2ts", 269_912_064),
        ]
    }

    #[test]
    fn content_hash_matches_thediscdb_for_a_uhd() {
        assert_eq!(
            content_hash(&john_wick()).as_deref(),
            Some("76E63EBB5B66F73B0C37BFAB59B8ABA5")
        );
    }

    #[test]
    fn content_hash_ignores_files_outside_the_stream_folder_and_listing_order() {
        let mut files = john_wick();
        files.reverse();
        files.push(DiscFile { path: "BDMV/PLAYLIST/00001.mpls".into(), size: 1234 });
        files.push(DiscFile { path: "BDMV\\STREAM\\SSIF\\00001.ssif".into(), size: 99 });
        assert_eq!(
            content_hash(&files).as_deref(),
            Some("76E63EBB5B66F73B0C37BFAB59B8ABA5")
        );
    }

    #[test]
    fn format_is_detected_from_the_folder_layout() {
        assert_eq!(disc_format(&john_wick()), Some(DiscFormat::BluRay));
        let dvd = vec![
            DiscFile { path: "VIDEO_TS/VIDEO_TS.IFO".into(), size: 10 },
            DiscFile { path: "VIDEO_TS/VTS_01_1.VOB".into(), size: 20 },
        ];
        assert_eq!(disc_format(&dvd), Some(DiscFormat::Dvd));
        assert_eq!(disc_format(&[]), None);
        assert_eq!(content_hash(&[]), None);
    }

    #[test]
    fn dvd_hash_covers_every_file_in_video_ts() {
        let a = vec![
            DiscFile { path: "VIDEO_TS/VIDEO_TS.IFO".into(), size: 10 },
            DiscFile { path: "VIDEO_TS/VTS_01_1.VOB".into(), size: 20 },
        ];
        let mut b = a.clone();
        b.push(DiscFile { path: "VIDEO_TS/VIDEO_TS.BUP".into(), size: 10 });
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn aacs_disc_id_is_the_uppercase_sha1_of_the_file() {
        assert_eq!(aacs_disc_id(b"abc"), "A9993E364706816ABA3E25717850C26C9CD0D89D");
    }
}

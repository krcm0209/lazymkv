//! Disc fingerprints computed from a file listing and file bytes handed in (spec 5), and the listing a TheDiscDB scan
//! log carries in its `HSH:` lines (`hashed_files`), so a pinned log is fingerprinted the way a disc is.

use md5::Md5;
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
    use md5::Digest as _;
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
    use sha1::Digest as _;
    hex_upper(Sha1::digest(unit_key_ro).as_slice())
}

/// The files of a TheDiscDB scan log's `HSH:index,name,time,size` lines, in log order: a name ending in `.m2ts` (any
/// case) under `BDMV/STREAM`, any other under `VIDEO_TS`, so `content_hash` of them is the log's fingerprint. A line
/// with fewer than four fields, an empty name or a size that is not a number is skipped; CRLF line ends and a leading
/// byte-order mark are accepted. Spec 5.
pub fn hashed_files(log: &str) -> Vec<DiscFile> {
    log.lines()
        .filter_map(|line| line.trim_start_matches('\u{feff}').strip_prefix("HSH:"))
        .filter_map(|rest| {
            let fields: Vec<&str> = rest.split(',').collect();
            if fields.len() < 4 {
                return None;
            }
            let name = fields[1].trim();
            if name.is_empty() {
                return None;
            }
            let size: u64 = fields[fields.len() - 1].trim().parse().ok()?;
            let folder = if name.to_ascii_lowercase().ends_with(".m2ts") { "BDMV/STREAM" } else { "VIDEO_TS" };
            Some(DiscFile { path: format!("{folder}/{name}"), size })
        })
        .collect()
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

    /// TheDiscDB's John Wick UHD log and Knowing DVD log (`corpus/thediscdb`), compiled into the test binary.
    const JOHN_WICK_LOG: &str =
        include_str!("../../../corpus/thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt");
    const KNOWING_LOG: &str = include_str!("../../../corpus/thediscdb/movie/Knowing (2009)/2010-dvd/disc01.txt");

    #[test]
    fn hashed_files_reads_every_hsh_line_of_a_pinned_uhd_log_in_log_order() {
        let files = hashed_files(JOHN_WICK_LOG);
        assert_eq!(files, john_wick());
        assert_eq!(content_hash(&files).as_deref(), Some("76E63EBB5B66F73B0C37BFAB59B8ABA5"));
    }

    #[test]
    fn hashed_files_puts_a_dvds_files_under_video_ts() {
        let files = hashed_files(KNOWING_LOG);
        assert_eq!(files.len(), 33);
        assert_eq!(files[0], DiscFile { path: "VIDEO_TS/VIDEO_TS.BUP".to_string(), size: 18_432 });
        assert_eq!(files[32], DiscFile { path: "VIDEO_TS/VTS_06_1.VOB".to_string(), size: 13_287_424 });
        assert_eq!(disc_format(&files), Some(DiscFormat::Dvd));
        assert_eq!(content_hash(&files).as_deref(), Some("631BD77872C157D7C436E6F48F06A478"));
    }

    #[test]
    fn hashed_files_skips_lines_it_cannot_read_and_reads_crlf_logs() {
        let log = concat!(
            "\u{feff}MSG:1005,0,1,\"MakeMKV v1.18.4 win(x64-release) started\",\"%1 started\",\"MakeMKV v1.18.4 win(x64-release)\"\r\n",
            "HSH:0,00001.M2TS,1/7/2026 9:49:51 AM,1000\r\n",
            "HSH:1,00002.m2ts,1/7/2026 9:49:51 AM,many\r\n",
            "HSH:2,00003.m2ts\r\n",
            "HSH:3,,1/7/2026 9:49:51 AM,7\r\n",
            "HSH:4,VIDEO_TS.IFO,4/27/2009 11:11:53 PM, 2048 \r\n",
            "TINFO:0,9,0,\"0:10:00\"\r\n",
        );
        assert_eq!(
            hashed_files(log),
            [
                DiscFile { path: "BDMV/STREAM/00001.M2TS".to_string(), size: 1000 },
                DiscFile { path: "VIDEO_TS/VIDEO_TS.IFO".to_string(), size: 2048 },
            ]
        );
        assert!(hashed_files("").is_empty());
        assert_eq!(hashed_files("\u{feff}HSH:0,00009.m2ts,1/7/2026 1:51:43 PM,2979840\n").len(), 1);
    }
}

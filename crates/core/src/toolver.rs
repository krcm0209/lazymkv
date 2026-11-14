//! The minimum versions of MKVToolNix and FFmpeg that the self-test judges a found tool against (design part 11.1,
//! spec 13; Rulings K10, K11, T6-R4; call 9 of pull request 6's notes), and the reader of the release number a tool
//! prints. Pure: no I/O (input and output). Def: T6-2. Use: T6-2.

use crate::job::Tool;
use crate::settings::version_part;

/// The minimum MKVToolNix for a copy the user supplies (design part 11.1, Ruling K10): 65.0, whose spellings
/// `--default-track-flag` and `--forced-display-flag` step 13 uses. A constant, not a setting (call 9).
/// Def: T6-2. Use: T6-2.
pub const MKVTOOLNIX_MIN: &str = "65.0";

/// The minimum FFmpeg for a copy the user supplies (design part 11.1, Ruling K11): 7.0, whose
/// `-/filter_complex graph.txt` step 10a's level pass uses. A constant, not a setting (call 9).
/// Def: T6-2. Use: T6-2.
pub const FFMPEG_MIN: &str = "7.0";

/// The largest first number a release number may have; a larger one is a dated git build (Ruling T6-R4).
const LAST_RELEASE: u32 = 999;

/// The minimum version `tool` is judged against in the self-test (design part 11.1, call 9): `MKVTOOLNIX_MIN` for
/// mkvmerge, `FFMPEG_MIN` for ffprobe and ffmpeg, None for makemkvcon (its own check) and java (not judged).
/// Def: T6-2. Use: T6-2.
pub fn minimum(tool: Tool) -> Option<&'static str> {
    match tool {
        Tool::Mkvmerge => Some(MKVTOOLNIX_MIN),
        Tool::Ffprobe | Tool::Ffmpeg => Some(FFMPEG_MIN),
        Tool::Makemkvcon | Tool::Java => None,
    }
}

/// The release number of a version word as a comparable triple (design part 9.2, Ruling T6-R4, decision E1): an
/// optional leading `n`, then one to three runs of ASCII (American Standard Code for Information Interchange) digits
/// joined by `.`, ending at the end of the word or at `-`, `+` or `~`; missing parts are 0. None for anything else: a
/// first number above 999 (a dated git build), a fourth part, an empty part, a part above `u32::MAX`, or any other
/// character. Each part is read by `settings::version_part`. `settings::version_triple` is a different grammar (exactly
/// three parts, no prefix and no suffix), and the two readers are not to be merged (the review of pull request #21
/// (X11)). Def: T6-2. Use: T6-2.
pub fn release_number(version: &str) -> Option<(u32, u32, u32)> {
    let rest = version.strip_prefix('n').unwrap_or(version);
    let end = rest.find(|c: char| matches!(c, '-' | '+' | '~')).unwrap_or(rest.len());
    let mut parts = [0u32; 3];
    let mut count = 0;
    for part in rest[..end].split('.') {
        if count == parts.len() {
            return None;
        }
        parts[count] = version_part(part)?;
        count += 1;
    }
    (parts[0] <= LAST_RELEASE).then_some((parts[0], parts[1], parts[2]))
}

/// How a version stands against a minimum (design part 11.1, call 9, decision E1). Def: T6-2. Use: T6-2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// The version's release number is at or above the minimum's.
    AtLeast,
    /// The version's release number is below the minimum's: the check is not ok.
    Below,
    /// The version has no release number to compare (E1), such as an FFmpeg git build: the check stays ok.
    Unreadable,
}

/// How `version` stands against `minimum`, both read with `release_number` (design part 11.1, call 9, Ruling T6-R4,
/// decision E1). A minimum that does not read is a bug and gives `Unreadable`. Def: T6-2. Use: T6-2.
pub fn standing(version: &str, minimum: &str) -> Standing {
    match (release_number(version), release_number(minimum)) {
        (Some(have), Some(need)) if have < need => Standing::Below,
        (Some(_), Some(_)) => Standing::AtLeast,
        _ => Standing::Unreadable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_numbers_are_read_as_tools_print_them() {
        for (text, want) in [
            ("100.0", Some((100, 0, 0))),
            ("9.0.2", Some((9, 0, 2))),
            ("9.0.2-full_build-www.gyan.dev", Some((9, 0, 2))),
            ("n7.1.5-12-g1fdbca85aa-linux64-gpl-7.1", Some((7, 1, 5))),
            ("6.1.1-3ubuntu5", Some((6, 1, 1))),
            ("7.1", Some((7, 1, 0))),
            ("7", Some((7, 0, 0))),
            ("8.0+git", Some((8, 0, 0))),
            ("7.0~rc1", Some((7, 0, 0))),
            ("999.0", Some((999, 0, 0))),
            ("N-121000-g5d8a9d5e3b", None),
            ("2026-10-01-git-0a1b2c3d4e", None),
            ("1000.0", None),
            ("7.1.1.2", None),
            ("7.x", None),
            ("7.", None),
            ("v7.1", None),
            ("n", None),
            ("", None),
            ("4294967296.0", None),
        ] {
            assert_eq!(release_number(text), want, "{text}");
        }
    }

    #[test]
    fn a_version_stands_at_least_below_or_unreadable_against_its_minimum() {
        assert_eq!(standing("65.0", MKVTOOLNIX_MIN), Standing::AtLeast);
        assert_eq!(standing("64.0", MKVTOOLNIX_MIN), Standing::Below);
        assert_eq!(standing("7.0", FFMPEG_MIN), Standing::AtLeast);
        assert_eq!(standing("6.1.1-3ubuntu5", FFMPEG_MIN), Standing::Below);
        assert_eq!(standing("N-121000-g5d8a9d5e3b", FFMPEG_MIN), Standing::Unreadable);
    }

    #[test]
    fn only_mkvmerge_ffprobe_and_ffmpeg_have_a_minimum() {
        assert_eq!(minimum(Tool::Mkvmerge), Some("65.0"));
        assert_eq!((minimum(Tool::Ffprobe), minimum(Tool::Ffmpeg)), (Some("7.0"), Some("7.0")));
        assert_eq!((minimum(Tool::Java), minimum(Tool::Makemkvcon)), (None, None));
    }
}

//! The disc phase's planning (spec 6.1 step 5): the disc kind and its groups (8.2, `kind`), the movie titles and
//! their editions (8.3 rules 1 to 4 and 8.4, `movie`), and the mode the title confidence sets (6.2, 6.3, 8.8,
//! `Mode`). Task 6 adds `tv` (8.5) and Task 7 the planner itself (`plan`), which Tasks 13, 15 and 16 call.

pub mod kind;
pub mod movie;

pub use crate::lookup::Kind;
pub use kind::{Group, KindSource};

use crate::settings::Settings;

/// 6.2 or 6.3: Direct rips the planned titles; Copy backs up the whole disc and decides after review. Spec 6.2, 6.3, 8.8.
/// Def: T5. Use: T7, T14, T15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Direct,
    Copy,
}

impl Mode {
    /// Both modes, Direct first.
    pub const ALL: [Mode; 2] = [Mode::Direct, Mode::Copy];

    /// The serde spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Direct => "direct",
            Mode::Copy => "copy",
        }
    }
}

impl std::str::FromStr for Mode {
    type Err = String;

    /// Only the `as_str` spellings.
    fn from_str(text: &str) -> Result<Mode, String> {
        Mode::ALL.into_iter().find(|mode| mode.as_str() == text).ok_or_else(|| format!("'{text}' is not a mode"))
    }
}

/// Direct when `title_confidence >= settings.auto_threshold`, else Copy (6.2, 6.3, 8.8). Reads `settings.auto_threshold`
/// only. Def: T5. Use: T7.
pub fn mode(title_confidence: f64, settings: &Settings) -> Mode {
    if title_confidence >= settings.auto_threshold {
        Mode::Direct
    } else {
        Mode::Copy
    }
}

/// The inputs the tests of `plan::kind`, `plan::movie` and this file share: the nine pinned TheDiscDB logs, their
/// live answers, and the streams synthetic scans are built from. Test builds only; nothing here reads the evidence
/// clone.
#[cfg(test)]
pub(crate) mod fixtures {
    use std::fs;
    use std::path::Path;

    use crate::disc::{disc, Disc};
    use crate::lookup::discdb::{parse_dischash, DiscDbRecord};
    use crate::robot::parse;

    pub(crate) const JOHN_WICK_UHD: &str = "thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt";
    pub(crate) const JOHN_WICK_BLU_RAY: &str = "thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc02.txt";
    pub(crate) const KNOWING: &str = "thediscdb/movie/Knowing (2009)/2010-dvd/disc01.txt";
    pub(crate) const SMILE: &str = "thediscdb/movie/Smile (2022)/2022-4k/disc01.txt";
    pub(crate) const EXPENDABLES: &str =
        "thediscdb/movie/The Expendables 3 (2014)/the-expendables-3-film-collection/disc01.txt";
    pub(crate) const CHERNOBYL_1: &str = "thediscdb/series/Chernobyl (2019)/2020-4k-US/disc01.txt";
    pub(crate) const CHERNOBYL_1_PINNED_JSON: &str = "thediscdb/series/Chernobyl (2019)/2020-4k-US/disc01.json";
    pub(crate) const CHERNOBYL_2: &str = "thediscdb/series/Chernobyl (2019)/2020-4k-US/disc02.txt";
    pub(crate) const FIREFLY_1: &str = "thediscdb/series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt";
    pub(crate) const FIREFLY_2: &str = "thediscdb/series/Firefly (2002)/2003-dvd-the-complete-series/disc02.txt";

    /// The `dischash` answers of `corpus/thediscdb-api` for the logs above (their `ContentHash`).
    pub(crate) const JOHN_WICK_UHD_API: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";
    pub(crate) const EXPENDABLES_API: &str = "545411B47060B18FD7F8D94599F24708";
    pub(crate) const SMILE_API: &str = "E826F0C64B04079EA83094AC046E8709";
    pub(crate) const KNOWING_API: &str = "631BD77872C157D7C436E6F48F06A478";
    pub(crate) const CHERNOBYL_1_API: &str = "D11592C9B2D252937C22EDF01DBD1E05";
    pub(crate) const CHERNOBYL_2_API: &str = "59796E40C7ABE16ACD9B259E732D392B";
    pub(crate) const FIREFLY_1_API: &str = "5A961BC67EB34171F24852D3A7688020";

    /// CINFO 1 of a Blu-ray and of a DVD.
    pub(crate) const BLURAY: &[(u32, &str)] = &[(1, "Blu-ray disc")];
    pub(crate) const DVD: &[(u32, &str)] = &[(1, "DVD disc")];
    /// Streams as MakeMKV reports them (attributes 1, 3, 5, 6, 7, 14), for synthetic titles.
    pub(crate) const VIDEO: &[(u32, &str)] =
        &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")];
    pub(crate) const DVD_VIDEO: &[(u32, &str)] = &[(1, "Video"), (5, "V_MPEG2"), (6, "Mpeg2"), (7, "Mpeg2")];
    pub(crate) const ENG_AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];
    pub(crate) const SPA_AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "spa"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];
    pub(crate) const FRA_AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "fra"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];
    pub(crate) const JPN_AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "jpn"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")];
    pub(crate) const ENG_PGS: &[(u32, &str)] =
        &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];

    /// A file under `corpus/`, read whole (the logs are UTF-8; any stray byte is replaced, as the parser's callers do).
    pub(crate) fn corpus_text(relative: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(relative);
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// The typed disc of a robot log's text.
    pub(crate) fn disc_of(text: &str) -> Disc {
        disc(&parse(text)).expect("the scan builds a typed disc")
    }

    /// The typed disc of one pinned log.
    pub(crate) fn pinned(relative: &str) -> Disc {
        disc_of(&corpus_text(relative))
    }

    /// The `dischash` answer `corpus/thediscdb-api/dischash-<hash>.json`, which must be a hit.
    pub(crate) fn api_record(hash: &str) -> DiscDbRecord {
        let name = format!("thediscdb-api/dischash-{hash}.json");
        parse_dischash(&corpus_text(&name), &name).expect("the answer parses").expect("the answer is a hit")
    }

    /// A pinned `discNN.json` (the data repository's form, without `media`).
    pub(crate) fn pinned_record(relative: &str) -> DiscDbRecord {
        serde_json::from_str(&corpus_text(relative)).expect("the pinned record parses")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disc::Disc;

    #[test]
    fn direct_mode_needs_the_auto_threshold() {
        let settings = Settings::default();
        assert_eq!(mode(0.98, &settings), Mode::Direct);
        assert_eq!(mode(0.85, &settings), Mode::Direct);
        assert_eq!(mode(0.84, &settings), Mode::Copy);
        assert_eq!(mode(0.60, &settings), Mode::Copy);
    }

    #[test]
    fn a_higher_auto_threshold_sends_a_0_85_plan_to_copy_mode() {
        let settings = Settings { auto_threshold: 0.90, ..Settings::default() };
        assert_eq!(mode(0.85, &settings), Mode::Copy);
        assert_eq!(mode(0.90, &settings), Mode::Direct);
        assert_eq!(mode(0.98, &settings), Mode::Direct);
    }

    #[test]
    fn modes_spell_themselves_as_the_spec_does() {
        assert_eq!(Mode::ALL.map(Mode::as_str), ["direct", "copy"]);
        for mode in Mode::ALL {
            assert_eq!(serde_json::to_string(&mode).unwrap(), format!("\"{}\"", mode.as_str()));
            assert_eq!(mode.as_str().parse::<Mode>(), Ok(mode));
        }
        assert!("Direct".parse::<Mode>().is_err());
    }

    #[test]
    fn the_plan_module_reexports_the_kind_types() {
        // Chiefly a compile check: `plan::Group`, `plan::KindSource` and `plan::Kind` must be the kind module's items (the
        // function pointer type-checks only when `plan::Group` is `kind::Group`); the serde spellings are then read through
        // the re-exported paths.
        let _: fn(&Disc, &[u32]) -> Vec<Group> = kind::groups;
        assert_eq!(serde_json::to_string(&KindSource::PlayAll).unwrap(), "\"play_all\"");
        assert_eq!(serde_json::to_string(&Kind::Tv).unwrap(), "\"tv\"");
    }
}

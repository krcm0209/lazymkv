//! The codec table of design spec 8.6 "Codecs": MakeMKV's codec names (scan attributes 5, 6 and 7, spec 9.4) mapped
//! to a codec family, the fidelity tier of 8.6 step 3.2, the object-based extension of step 3.3 and the display name
//! 8.10 uses; the channel layout names of 8.6 "Codecs"; attribute 13's bit rate text (9.4); and the codec names
//! `mkvmerge -J` writes for each family (6.2 step 7). The rows come from every scan recorded or pinned in phase 0
//! (the four recordings' `scan.txt` and the nine `corpus/thediscdb` logs); the rows the spec names but no recording
//! shows carry MakeMKV's and mkvmerge's documented spellings and are marked "spelling not yet seen in a recording".
//! Written from the spec (RP20); `Codec.lossless` is the product's copy beside `audio::reference::is_lossless`
//! (Global Constraints). Used by Task 2 (`Stream.codec`) and Tasks 6, 8, 9, 11 and 12.

/// A codec family the rules name (spec 8.6 "Codecs", 8.6 step 3's tiers, 6.2 step 7's mkvmerge names). The serde and
/// `as_str` spelling is the variant's name in kebab-case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    /// HEVC video (attribute 5 `V_MPEGH/ISO/HEVC`), Dolby Vision included.
    Hevc,
    /// AVC video (`V_MPEG4/ISO/AVC`).
    Avc,
    /// MPEG-2 video (`V_MPEG2`).
    Mpeg2,
    /// VC-1 video (`V_MS/VFW/WVC1`, ruling RP26; matched on attribute 5 alone, its attributes 6 and 7 not yet seen in a
    /// recording).
    Vc1,
    /// Dolby TrueHD (`A_TRUEHD`), with or without Atmos.
    TrueHd,
    /// DTS-HD Master Audio (`A_DTS`, attribute 6 `DTS-HD MA`), and DTS:X carried on it.
    DtsHdMa,
    /// DTS-HD High Resolution Audio (`A_DTS`, attribute 6 `DTS-HD HR`; spelling not yet seen in a recording).
    DtsHdHr,
    /// DTS (`A_DTS`, attribute 6 `DTS`), DTS-ES and DTS 96/24 included.
    Dts,
    /// DTS Express (`A_DTS`, attribute 6 or 7 `DTS-HD LBR` or `DTS Express`; spelling not yet seen in a recording), the
    /// low bit rate format of picture-in-picture commentaries.
    DtsExpress,
    /// Dolby Digital, AC-3 (`A_AC3`).
    Ac3,
    /// Dolby Digital Plus, E-AC-3 (`A_EAC3`; spelling not yet seen in a recording).
    Eac3,
    /// Linear PCM (`A_LPCM`, or a codec ID starting `A_PCM`; spelling not yet seen in a recording).
    Lpcm,
    /// FLAC (`A_FLAC`; spelling not yet seen in a recording).
    Flac,
    /// MPEG audio (`A_MPEG/L2` and the other layers; spelling not yet seen in a recording).
    MpegAudio,
    /// AAC (`A_AAC`, with or without a profile suffix; spelling not yet seen in a recording).
    Aac,
    /// Blu-ray PGS subtitles (`S_HDMV/PGS`).
    Pgs,
    /// DVD subtitles (`S_VOBSUB`).
    VobSub,
    /// A DVD's closed captions (`S_CC608/DVD`), which MakeMKV converts to text (8.7 "Set aside first").
    ClosedCaptions,
    /// Any other codec: a codec the table does not know (6.2 step 7 then compares no codec name for it).
    Unknown,
}

impl Family {
    /// Every family, in the order of the enum. Spec 8.6 "Codecs".
    pub const ALL: [Family; 19] = [
        Family::Hevc,
        Family::Avc,
        Family::Mpeg2,
        Family::Vc1,
        Family::TrueHd,
        Family::DtsHdMa,
        Family::DtsHdHr,
        Family::Dts,
        Family::DtsExpress,
        Family::Ac3,
        Family::Eac3,
        Family::Lpcm,
        Family::Flac,
        Family::MpegAudio,
        Family::Aac,
        Family::Pgs,
        Family::VobSub,
        Family::ClosedCaptions,
        Family::Unknown,
    ];

    /// The kebab-case spelling, the same string serde writes. Spec 8.6 "Codecs".
    pub fn as_str(self) -> &'static str {
        match self {
            Family::Hevc => "hevc",
            Family::Avc => "avc",
            Family::Mpeg2 => "mpeg2",
            Family::Vc1 => "vc1",
            Family::TrueHd => "true-hd",
            Family::DtsHdMa => "dts-hd-ma",
            Family::DtsHdHr => "dts-hd-hr",
            Family::Dts => "dts",
            Family::DtsExpress => "dts-express",
            Family::Ac3 => "ac3",
            Family::Eac3 => "eac3",
            Family::Lpcm => "lpcm",
            Family::Flac => "flac",
            Family::MpegAudio => "mpeg-audio",
            Family::Aac => "aac",
            Family::Pgs => "pgs",
            Family::VobSub => "vob-sub",
            Family::ClosedCaptions => "closed-captions",
            Family::Unknown => "unknown",
        }
    }
}

/// Spec 8.6 step 3.3's object-based extension. The serde and `as_str` spelling is the format's own name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ObjectAudio {
    /// Dolby Atmos (seen on TrueHD: attribute 7 `TrueHD Atmos`).
    #[serde(rename = "Atmos")]
    Atmos,
    /// DTS:X (on DTS-HD MA; spelling not yet seen in a recording).
    #[serde(rename = "DTS:X")]
    DtsX,
}

impl ObjectAudio {
    /// "Atmos" or "DTS:X", the same string serde writes. Spec 8.6 step 3.3.
    pub fn as_str(self) -> &'static str {
        match self {
            ObjectAudio::Atmos => "Atmos",
            ObjectAudio::DtsX => "DTS:X",
        }
    }
}

/// What the table says about one stream's codec (spec 8.6 "Codecs").
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Codec {
    /// The codec family.
    pub family: Family,
    /// The display name 8.10 uses ("TrueHD", "DTS-HD MA", "DTS", "DTS Express", "Dolby Digital", "Dolby Digital Plus",
    /// "DTS-HD HR", "LPCM", "FLAC", "MPEG Audio", "AAC", "HEVC", "AVC", "MPEG-2", "VC-1", "PGS", "VobSub", "Closed
    /// Captions");
    /// attribute 6, else 7, else 5, for Unknown.
    pub display: String,
    /// From attribute 7 at scan time: "TrueHD Atmos" gives Atmos, a 7 containing "DTS:X" gives DtsX (spelling not yet
    /// seen).
    pub object: Option<ObjectAudio>,
    /// True for TrueHd, DtsHdMa, Lpcm, Flac (8.6 step 3.2 (a)); false for every other family. The product's copy
    /// beside audio::reference::is_lossless (Global Constraints, one-copy rule).
    pub lossless: bool,
}

/// The table row for MakeMKV's attributes 5 (codec ID), 6 (short name) and 7 (long name); never fails: an unknown
/// trio is `Family::Unknown`, named by attribute 6, else 7, else 5. Attribute 5 decides the family (exactly, or by
/// the prefixes `A_PCM`, `A_MPEG/` and `A_AAC/`); `A_DTS` is told apart by attribute 6, then 7. The object-based
/// extension is read from attribute 7, then 6, for an audio family only. Rows from the recordings: V_MPEGH/ISO/HEVC,
/// V_MPEG4/ISO/AVC, V_MPEG2, A_TRUEHD, A_DTS with short "DTS-HD MA" or "DTS", A_AC3, S_HDMV/PGS, S_VOBSUB,
/// S_CC608/DVD; rows the spec names but no recording shows, with MakeMKV's documented spellings: A_DTS "DTS-HD HR",
/// "DTS:X" and DTS Express ("DTS-HD LBR" or "DTS Express"), A_EAC3 "E-AC3", A_PCM or A_LPCM "LPCM", A_FLAC "FLAC", A_MPEG/L2 "Mpeg", A_AAC "AAC"; and
/// V_MS/VFW/WVC1, VC-1 (ruling RP26), matched on attribute 5 alone. Spec 8.6 "Codecs".
pub fn codec(id: &str, short: &str, long: &str) -> Codec {
    let family = family_of(id, short, long);
    let display = match display_name(family) {
        Some(name) => name.to_string(),
        None => [short, long, id].into_iter().find(|text| !text.is_empty()).unwrap_or("").to_string(),
    };
    let object = if is_audio(family) { object_in(long).or_else(|| object_in(short)) } else { None };
    Codec { family, display, object, lossless: is_lossless(family) }
}

/// Attribute 41's short names and the family each names. Attribute 41 is MakeMKV's output codec short name (apdefs.h
/// `ap_iaOutputCodecShort`), written in the spelling of attribute 6: "Text" is the one recorded (a DVD's closed
/// captions converted to text, 6.2 step 7); the others are the attribute 6 spellings of the recordings and of the rows
/// above ("DD" and "AC3" both for Dolby Digital), not yet seen in attribute 41.
const OUTPUT_SHORT_NAMES: [(&str, Family); 18] = [
    ("Text", Family::ClosedCaptions),
    ("MpegH", Family::Hevc),
    ("Mpeg4", Family::Avc),
    ("Mpeg2", Family::Mpeg2),
    ("TrueHD", Family::TrueHd),
    ("DTS-HD MA", Family::DtsHdMa),
    ("DTS-HD HR", Family::DtsHdHr),
    ("DTS", Family::Dts),
    ("DTS-HD LBR", Family::DtsExpress),
    ("DD", Family::Ac3),
    ("AC3", Family::Ac3),
    ("E-AC3", Family::Eac3),
    ("LPCM", Family::Lpcm),
    ("FLAC", Family::Flac),
    ("Mpeg", Family::MpegAudio),
    ("AAC", Family::Aac),
    ("PGS", Family::Pgs),
    ("CC", Family::ClosedCaptions),
];

/// The table row for attribute 41, MakeMKV's output codec short name (apdefs.h `ap_iaOutputCodecShort`): the codec the
/// output file holds when MakeMKV converts a stream (an LPCM track written as FLAC by MakeMKV's default profile, a DVD's
/// closed captions written as text). A short name of `OUTPUT_SHORT_NAMES` gives its family, with the family's display
/// name and no object-based extension; any other value is read as a codec ID through `codec(short, "", "")`, so an
/// attribute 5 spelling still resolves, and a value neither knows is `Family::Unknown` named by itself. Never fails.
/// Spec 6.2 step 7 ("compares attribute 41 where present"), 8.6 "Codecs", 9.4.
pub fn output_codec(short: &str) -> Codec {
    match OUTPUT_SHORT_NAMES.iter().find(|(name, _)| *name == short) {
        Some(&(_, family)) => Codec {
            family,
            display: display_name(family).unwrap_or(short).to_string(),
            object: None,
            lossless: is_lossless(family),
        },
        None => codec(short, "", ""),
    }
}

/// The family attribute 5 names, with `A_DTS` told apart by attributes 6 and 7.
fn family_of(id: &str, short: &str, long: &str) -> Family {
    match id {
        "V_MPEGH/ISO/HEVC" => Family::Hevc,
        "V_MPEG4/ISO/AVC" => Family::Avc,
        "V_MPEG2" => Family::Mpeg2,
        "V_MS/VFW/WVC1" => Family::Vc1,
        "A_TRUEHD" => Family::TrueHd,
        "A_DTS" => dts_family(short, long),
        "A_AC3" => Family::Ac3,
        "A_EAC3" => Family::Eac3,
        "A_LPCM" => Family::Lpcm,
        "A_FLAC" => Family::Flac,
        "A_AAC" => Family::Aac,
        "S_HDMV/PGS" => Family::Pgs,
        "S_VOBSUB" => Family::VobSub,
        "S_CC608/DVD" => Family::ClosedCaptions,
        _ if id.starts_with("A_PCM") => Family::Lpcm,
        _ if id.starts_with("A_MPEG/") => Family::MpegAudio,
        _ if id.starts_with("A_AAC/") => Family::Aac,
        _ => Family::Unknown,
    }
}

/// An `A_DTS` stream's family (spec 8.6 "Codecs", SR5): DTS:X (in attribute 6 or 7) rides on DTS-HD MA; else the family
/// attribute 6 names (`dts_named`), and only when attribute 6 names none, the family attribute 7 names, so a pair that
/// disagrees is read by attribute 6; else Unknown.
fn dts_family(short: &str, long: &str) -> Family {
    if short.contains("DTS:X") || long.contains("DTS:X") {
        return Family::DtsHdMa;
    }
    dts_named(short).or_else(|| dts_named(long)).unwrap_or(Family::Unknown)
}

/// The DTS family one attribute's text names exactly: "DTS-HD MA" or "DTS-HD Master Audio" is DTS-HD MA, "DTS-HD HR" or
/// "DTS-HD High Resolution Audio" is DTS-HD HR, "DTS-HD LBR" or "DTS Express" is DTS Express (spelling not yet seen in a
/// recording), "DTS" is DTS; None for any other text, the empty text included. SR5.
fn dts_named(text: &str) -> Option<Family> {
    match text {
        "DTS-HD MA" | "DTS-HD Master Audio" => Some(Family::DtsHdMa),
        "DTS-HD HR" | "DTS-HD High Resolution Audio" => Some(Family::DtsHdHr),
        "DTS-HD LBR" | "DTS Express" => Some(Family::DtsExpress),
        "DTS" => Some(Family::Dts),
        _ => None,
    }
}

/// 8.10's display name of a known family; None for Unknown.
fn display_name(family: Family) -> Option<&'static str> {
    let name = match family {
        Family::Hevc => "HEVC",
        Family::Avc => "AVC",
        Family::Mpeg2 => "MPEG-2",
        Family::Vc1 => "VC-1",
        Family::TrueHd => "TrueHD",
        Family::DtsHdMa => "DTS-HD MA",
        Family::DtsHdHr => "DTS-HD HR",
        Family::Dts => "DTS",
        Family::DtsExpress => "DTS Express",
        Family::Ac3 => "Dolby Digital",
        Family::Eac3 => "Dolby Digital Plus",
        Family::Lpcm => "LPCM",
        Family::Flac => "FLAC",
        Family::MpegAudio => "MPEG Audio",
        Family::Aac => "AAC",
        Family::Pgs => "PGS",
        Family::VobSub => "VobSub",
        Family::ClosedCaptions => "Closed Captions",
        Family::Unknown => return None,
    };
    Some(name)
}

/// The audio families.
fn is_audio(family: Family) -> bool {
    matches!(
        family,
        Family::TrueHd
            | Family::DtsHdMa
            | Family::DtsHdHr
            | Family::Dts
            | Family::DtsExpress
            | Family::Ac3
            | Family::Eac3
            | Family::Lpcm
            | Family::Flac
            | Family::MpegAudio
            | Family::Aac
    )
}

/// 8.6 step 3.2 (a): TrueHD, DTS-HD MA, LPCM, FLAC.
fn is_lossless(family: Family) -> bool {
    matches!(family, Family::TrueHd | Family::DtsHdMa | Family::Lpcm | Family::Flac)
}

/// The extension a text names: "Atmos" anywhere gives Atmos, else "DTS:X" anywhere gives DtsX.
fn object_in(text: &str) -> Option<ObjectAudio> {
    if text.contains("Atmos") {
        Some(ObjectAudio::Atmos)
    } else if text.contains("DTS:X") {
        Some(ObjectAudio::DtsX)
    } else {
        None
    }
}

/// Attribute 13 text ("1.5 Mb/s", "192 Kb/s", "9.8 Mb/s") as kilobits per second, rounded to the nearest whole
/// number; None for anything else. The grammar is the one every recorded and pinned value follows: ASCII digits with
/// at most one '.', one space, then "Kb/s" or "Mb/s" exactly; a value above u32::MAX kilobits is None. Spec 9.4
/// "Attribute 13 is text with units".
pub fn bitrate_kbps(text: &str) -> Option<u32> {
    let (number, unit) = text.split_once(' ')?;
    let factor = match unit {
        "Kb/s" => 1.0,
        "Mb/s" => 1000.0,
        _ => return None,
    };
    let digits = number.bytes().filter(|byte| byte.is_ascii_digit()).count();
    let dots = number.bytes().filter(|byte| *byte == b'.').count();
    if digits == 0 || dots > 1 || digits + dots != number.len() {
        return None;
    }
    let kbps = (number.parse::<f64>().ok()? * factor).round();
    if kbps > f64::from(u32::MAX) {
        return None;
    }
    Some(kbps as u32)
}

/// Spec 8.6 step 3.2's fidelity tiers, in rank order: the derived order sorts the better tier first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// (a) Lossless: TrueHD, DTS-HD MA, LPCM, FLAC.
    Lossless,
    /// (b) High-rate lossy: DTS-HD High Resolution, E-AC-3 at 768 kb/s or more, DTS at 1.5 Mb/s or more.
    HighRateLossy,
    /// (c) Other lossy: AC-3, lower-rate E-AC-3 and DTS, and DTS Express.
    OtherLossy,
    /// (d) MPEG audio and AAC.
    Low,
}

impl Tier {
    /// The snake_case spelling, the same string serde writes. Spec 8.6 step 3.2.
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Lossless => "lossless",
            Tier::HighRateLossy => "high_rate_lossy",
            Tier::OtherLossy => "other_lossy",
            Tier::Low => "low",
        }
    }
}

/// Spec 8.6 step 3.2's E-AC-3 line: at this rate or more an E-AC-3 track is high-rate lossy (a fixed value of spec 14).
pub const EAC3_HIGH_RATE_KBPS: u32 = 768;

/// Spec 8.6 step 3.2's DTS line, 1.5 Mb/s: at this rate or more a DTS track is high-rate lossy (a fixed value of
/// spec 14).
pub const DTS_HIGH_RATE_KBPS: u32 = 1500;

/// Spec 8.6 step 3.2: (a) Lossless for the lossless families (`is_lossless`'s set, not repeated here); (b)
/// HighRateLossy for DtsHdHr, Eac3 at EAC3_HIGH_RATE_KBPS or more, Dts at DTS_HIGH_RATE_KBPS or more; (c) OtherLossy
/// for Ac3, lower-rate or unknown-rate Eac3 and Dts, and DtsExpress at any rate (other lossy DTS); (d) Low for MpegAudio and Aac; None for Unknown and for video and
/// subtitle families.
pub fn tier(family: Family, bitrate_kbps: Option<u32>) -> Option<Tier> {
    let at_least = |line: u32| bitrate_kbps.is_some_and(|rate| rate >= line);
    match family {
        family if is_lossless(family) => Some(Tier::Lossless),
        Family::DtsHdHr => Some(Tier::HighRateLossy),
        Family::Eac3 if at_least(EAC3_HIGH_RATE_KBPS) => Some(Tier::HighRateLossy),
        Family::Dts if at_least(DTS_HIGH_RATE_KBPS) => Some(Tier::HighRateLossy),
        Family::Ac3 | Family::Eac3 | Family::Dts | Family::DtsExpress => Some(Tier::OtherLossy),
        Family::MpegAudio | Family::Aac => Some(Tier::Low),
        // Video, subtitle and Unknown families have no tier. The guard arm above took the lossless families (the one
        // copy of the set is `is_lossless`); a guard does not count toward exhaustiveness, so a wildcard ends it.
        _ => None,
    }
}

/// Spec 8.6 "Codecs": 1 "1.0", 2 "2.0", 6 "5.1", 7 "6.1", 8 "7.1"; any other count "<n>.0" (3, 4, 5 give "3.0",
/// "4.0", "5.0"; 0 gives "0.0"; Open point 10).
pub fn channel_layout(channels: u32) -> String {
    match channels {
        6 => "5.1".to_string(),
        7 => "6.1".to_string(),
        8 => "7.1".to_string(),
        other => format!("{other}.0"),
    }
}

/// ffprobe's profile string to the object-based extension (6.4 step 9; EXP-06 not run, so provisional): a profile
/// containing "Atmos" gives Atmos, one containing "DTS:X" gives DtsX, else None (Open point 8).
pub fn object_audio(ffprobe_profile: Option<&str>) -> Option<ObjectAudio> {
    ffprobe_profile.and_then(object_in)
}

/// The codec names `mkvmerge -J` may write for a stream of this codec, as 6.2 step 7 records them; `verify_rip`
/// accepts any of them. Hevc ["HEVC/H.265/MPEG-H"], Avc ["AVC/H.264/MPEG-4p10"], Mpeg2 ["MPEG-1/2"], TrueHd
/// ["TrueHD", "TrueHD Atmos"] (the second provisional: an Atmos stream's spelling is not yet seen), DtsHdMa
/// ["DTS-HD Master Audio"], DtsHdHr ["DTS-HD High Resolution Audio"], Dts ["DTS", "DTS-ES", "DTS 96/24"] (a DTS-ES or
/// DTS 96/24 stream is a DTS stream to MakeMKV and has its own name in mkvmerge), DtsExpress ["DTS Express"], Ac3
/// ["AC-3"], Eac3 ["E-AC-3"], Lpcm
/// ["PCM"], Flac ["FLAC"], MpegAudio ["MP2"], Aac ["AAC"], Pgs ["HDMV PGS"], VobSub ["VobSub"], ClosedCaptions
/// ["SubRip/SRT"] (converted through attribute 41 "Text"), Vc1 ["VC-1"] (ruling RP26), Unknown [] (`verify` then
/// compares no codec name for the track and says so in its report). The names seen in
/// the recordings are HEVC, DTS-HD Master Audio, DTS, AC-3, HDMV PGS, MPEG-1/2, VobSub, SubRip/SRT and AVC; the
/// others are mkvmerge's documented spellings, not yet seen. Spec 6.2 step 7.
pub fn mkvmerge_names(codec: &Codec) -> &'static [&'static str] {
    match codec.family {
        Family::Hevc => &["HEVC/H.265/MPEG-H"],
        Family::Avc => &["AVC/H.264/MPEG-4p10"],
        Family::Mpeg2 => &["MPEG-1/2"],
        Family::Vc1 => &["VC-1"],
        Family::TrueHd => &["TrueHD", "TrueHD Atmos"],
        Family::DtsHdMa => &["DTS-HD Master Audio"],
        Family::DtsHdHr => &["DTS-HD High Resolution Audio"],
        Family::Dts => &["DTS", "DTS-ES", "DTS 96/24"],
        Family::DtsExpress => &["DTS Express"],
        Family::Ac3 => &["AC-3"],
        Family::Eac3 => &["E-AC-3"],
        Family::Lpcm => &["PCM"],
        Family::Flac => &["FLAC"],
        Family::MpegAudio => &["MP2"],
        Family::Aac => &["AAC"],
        Family::Pgs => &["HDMV PGS"],
        Family::VobSub => &["VobSub"],
        Family::ClosedCaptions => &["SubRip/SRT"],
        Family::Unknown => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use crate::robot::{parse, Attrs};

    fn known(family: Family, display: &str, object: Option<ObjectAudio>, lossless: bool) -> Codec {
        Codec { family, display: display.to_string(), object, lossless }
    }

    /// The nine pinned TheDiscDB scan logs (`discNN.txt`, not the summaries), sorted by path.
    fn pinned_logs() -> Vec<PathBuf> {
        fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if path.is_dir() {
                    walk(&path, found);
                } else if name.starts_with("disc") && name.ends_with(".txt") && !name.ends_with("-summary.txt") {
                    found.push(path);
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb");
        let mut found = Vec::new();
        walk(&root, &mut found);
        found.sort();
        assert_eq!(found.len(), 9, "expected the nine pinned logs under {}", root.display());
        found
    }

    /// Every stream of the nine pinned logs, as robot::parse reads it.
    fn pinned_streams() -> Vec<Attrs> {
        pinned_logs()
            .iter()
            .flat_map(|path| {
                let text = String::from_utf8_lossy(&fs::read(path).unwrap()).into_owned();
                parse(&text).streams.into_values()
            })
            .collect()
    }

    /// Attribute `id` of a stream, "" when absent.
    fn attr(attrs: &Attrs, id: u32) -> &str {
        attrs.get(&id).map(String::as_str).unwrap_or("")
    }

    #[test]
    fn hevc_reads_as_hevc_with_or_without_dolby_vision() {
        let expected = known(Family::Hevc, "HEVC", None, false);
        assert_eq!(codec("V_MPEGH/ISO/HEVC", "MpegH", "MpegH HEVC Main10@L5.1"), expected);
        assert_eq!(codec("V_MPEGH/ISO/HEVC", "MpegH", "MpegH HEVC Main10@L5.1 (dvhe.07.06 BL+MEL+RPU)"), expected);
        assert_eq!(codec("V_MPEGH/ISO/HEVC", "MpegH", "MpegH HEVC Main10@L5.1 (dvhe.07.06 BL+FEL+RPU)"), expected);
    }

    #[test]
    fn avc_reads_as_avc() {
        assert_eq!(codec("V_MPEG4/ISO/AVC", "Mpeg4", "Mpeg4 AVC High@L4.1"), known(Family::Avc, "AVC", None, false));
    }

    #[test]
    fn mpeg_2_video_reads_as_mpeg_2() {
        assert_eq!(codec("V_MPEG2", "Mpeg2", "Mpeg2"), known(Family::Mpeg2, "MPEG-2", None, false));
    }

    #[test]
    fn vc_1_video_reads_as_vc_1_by_attribute_5_alone() {
        // Ruling RP26. Not seen in a recording: attributes 6 and 7 here are made up, and the row ignores them.
        let expected = known(Family::Vc1, "VC-1", None, false);
        assert_eq!(codec("V_MS/VFW/WVC1", "VC-1", "VC-1 Advanced@L3"), expected);
        assert_eq!(codec("V_MS/VFW/WVC1", "", ""), expected);
        assert_eq!(codec("V_MS/VFW/WVC1", "WVC1", "Microsoft VC-1"), expected);
    }

    #[test]
    fn truehd_reads_as_lossless_truehd_without_an_extension() {
        assert_eq!(codec("A_TRUEHD", "TrueHD", "TrueHD"), known(Family::TrueHd, "TrueHD", None, true));
    }

    #[test]
    fn truehd_atmos_reads_as_truehd_with_the_atmos_extension() {
        assert_eq!(
            codec("A_TRUEHD", "TrueHD", "TrueHD Atmos"),
            known(Family::TrueHd, "TrueHD", Some(ObjectAudio::Atmos), true)
        );
    }

    #[test]
    fn dts_hd_master_audio_reads_as_lossless_dts_hd_ma() {
        assert_eq!(
            codec("A_DTS", "DTS-HD MA", "DTS-HD Master Audio"),
            known(Family::DtsHdMa, "DTS-HD MA", None, true)
        );
    }

    #[test]
    fn plain_dts_reads_as_lossy_dts() {
        assert_eq!(codec("A_DTS", "DTS", "DTS"), known(Family::Dts, "DTS", None, false));
    }

    #[test]
    fn a_dts_stream_named_by_one_attribute_only_reads_by_that_attribute() {
        assert_eq!(codec("A_DTS", "", "DTS-HD Master Audio"), known(Family::DtsHdMa, "DTS-HD MA", None, true));
        assert_eq!(codec("A_DTS", "", "DTS"), known(Family::Dts, "DTS", None, false));
        assert_eq!(codec("A_DTS", "DTS-HD MA", ""), known(Family::DtsHdMa, "DTS-HD MA", None, true));
        assert_eq!(codec("A_DTS", "DTS-HD HR", ""), known(Family::DtsHdHr, "DTS-HD HR", None, false));
        assert_eq!(codec("A_DTS", "DTS", ""), known(Family::Dts, "DTS", None, false));
    }

    #[test]
    fn a_conflicting_dts_pair_is_read_by_attribute_6_then_7() {
        assert_eq!(codec("A_DTS", "DTS", "DTS-HD Master Audio").family, Family::Dts);
        assert_eq!(codec("A_DTS", "DTS-HD HR", "DTS-HD Master Audio").family, Family::DtsHdHr);
        assert_eq!(codec("A_DTS", "DTS-HD LBR", "DTS-HD Master Audio").family, Family::DtsExpress);
        assert_eq!(codec("A_DTS", "DTS-HD MA", "DTS").family, Family::DtsHdMa);
        assert_eq!(codec("A_DTS", "DTS", "DTS:X").family, Family::DtsHdMa, "DTS:X in either attribute wins");
    }

    #[test]
    fn dolby_digital_reads_as_ac_3_named_dolby_digital() {
        assert_eq!(codec("A_AC3", "DD", "Dolby Digital"), known(Family::Ac3, "Dolby Digital", None, false));
    }

    #[test]
    fn pgs_subtitles_read_as_pgs() {
        assert_eq!(codec("S_HDMV/PGS", "PGS", "HDMV PGS Subtitles"), known(Family::Pgs, "PGS", None, false));
    }

    #[test]
    fn dvd_subtitles_read_as_vobsub_although_attribute_6_is_empty() {
        assert_eq!(codec("S_VOBSUB", "", "Dvd Subtitles"), known(Family::VobSub, "VobSub", None, false));
    }

    #[test]
    fn dvd_closed_captions_read_as_closed_captions() {
        assert_eq!(
            codec("S_CC608/DVD", "CC", "Closed Captions"),
            known(Family::ClosedCaptions, "Closed Captions", None, false)
        );
    }

    #[test]
    fn dts_hd_high_resolution_reads_as_lossy_dts_hd_hr() {
        let expected = known(Family::DtsHdHr, "DTS-HD HR", None, false);
        assert_eq!(codec("A_DTS", "DTS-HD HR", "DTS-HD High Resolution Audio"), expected);
        assert_eq!(codec("A_DTS", "", "DTS-HD High Resolution Audio"), expected);
    }

    #[test]
    fn dts_x_reads_as_dts_hd_ma_with_the_dts_x_extension() {
        let expected = known(Family::DtsHdMa, "DTS-HD MA", Some(ObjectAudio::DtsX), true);
        assert_eq!(codec("A_DTS", "DTS:X", "DTS:X"), expected);
        assert_eq!(codec("A_DTS", "DTS-HD MA", "DTS-HD Master Audio DTS:X"), expected);
        assert_eq!(codec("A_DTS", "DTS:X", "DTS-HD Master Audio"), expected);
    }

    #[test]
    fn e_ac_3_reads_as_dolby_digital_plus() {
        assert_eq!(codec("A_EAC3", "E-AC3", "Dolby Digital Plus"), known(Family::Eac3, "Dolby Digital Plus", None, false));
    }

    #[test]
    fn lpcm_reads_as_lossless_lpcm_under_either_codec_id() {
        let expected = known(Family::Lpcm, "LPCM", None, true);
        assert_eq!(codec("A_LPCM", "LPCM", "LPCM"), expected);
        assert_eq!(codec("A_PCM", "LPCM", "LPCM"), expected);
        assert_eq!(codec("A_PCM/INT/LIT", "LPCM", "LPCM"), expected);
    }

    #[test]
    fn flac_reads_as_lossless_flac() {
        assert_eq!(codec("A_FLAC", "FLAC", "FLAC"), known(Family::Flac, "FLAC", None, true));
    }

    #[test]
    fn mpeg_audio_reads_as_mpeg_audio_for_every_layer() {
        let expected = known(Family::MpegAudio, "MPEG Audio", None, false);
        assert_eq!(codec("A_MPEG/L2", "Mpeg", "Mpeg"), expected);
        assert_eq!(codec("A_MPEG/L3", "Mpeg", "Mpeg"), expected);
    }

    #[test]
    fn aac_reads_as_aac_with_or_without_a_profile_suffix() {
        let expected = known(Family::Aac, "AAC", None, false);
        assert_eq!(codec("A_AAC", "AAC", "AAC"), expected);
        assert_eq!(codec("A_AAC/MPEG4/LC", "AAC", "AAC"), expected);
    }

    #[test]
    fn an_unknown_codec_is_named_by_attribute_6_then_7_then_5() {
        assert_eq!(codec("X_NEW", "New", "New Codec"), known(Family::Unknown, "New", None, false));
        assert_eq!(codec("X_NEW", "", "New Codec"), known(Family::Unknown, "New Codec", None, false));
        assert_eq!(codec("X_NEW", "", ""), known(Family::Unknown, "X_NEW", None, false));
        assert_eq!(codec("", "", ""), known(Family::Unknown, "", None, false));
    }

    #[test]
    fn dts_express_reads_as_lossy_dts_express_by_attribute_6_or_7() {
        // Not seen in a recording: MakeMKV's documented spellings of DTS Express (a picture-in-picture commentary).
        let expected = known(Family::DtsExpress, "DTS Express", None, false);
        assert_eq!(codec("A_DTS", "DTS-HD LBR", "DTS Express"), expected);
        assert_eq!(codec("A_DTS", "DTS-HD LBR", ""), expected);
        assert_eq!(codec("A_DTS", "", "DTS Express"), expected);
        assert_eq!(codec("A_DTS", "DTS Express", ""), expected);
        assert_eq!(tier(Family::DtsExpress, None), Some(Tier::OtherLossy));
        assert_eq!(tier(Family::DtsExpress, Some(1500)), Some(Tier::OtherLossy));
        assert_eq!(mkvmerge_names(&expected), ["DTS Express"]);
        assert_eq!(output_codec("DTS-HD LBR").family, Family::DtsExpress);
    }

    #[test]
    fn a_dts_stream_of_an_unrecognised_kind_is_unknown() {
        assert_eq!(codec("A_DTS", "DTS-XYZ", "Made Up DTS"), known(Family::Unknown, "DTS-XYZ", None, false));
    }

    #[test]
    fn only_an_audio_codec_carries_an_object_extension() {
        assert_eq!(codec("V_MPEGH/ISO/HEVC", "MpegH", "Atmos").object, None);
        assert_eq!(codec("S_HDMV/PGS", "PGS", "DTS:X").object, None);
        assert_eq!(codec("A_NEW", "", "Atmos").object, None);
        assert_eq!(codec("A_EAC3", "E-AC3", "Dolby Digital Plus Atmos").object, Some(ObjectAudio::Atmos));
    }

    #[test]
    fn every_family_is_spelled_in_kebab_case() {
        let spellings: Vec<&str> = Family::ALL.iter().map(|family| family.as_str()).collect();
        assert_eq!(
            spellings,
            [
                "hevc",
                "avc",
                "mpeg2",
                "vc1",
                "true-hd",
                "dts-hd-ma",
                "dts-hd-hr",
                "dts",
                "dts-express",
                "ac3",
                "eac3",
                "lpcm",
                "flac",
                "mpeg-audio",
                "aac",
                "pgs",
                "vob-sub",
                "closed-captions",
                "unknown",
            ]
        );
        for family in Family::ALL {
            assert_eq!(serde_json::to_value(family).unwrap(), Value::String(family.as_str().to_string()));
            assert_eq!(serde_json::from_value::<Family>(Value::String(family.as_str().to_string())).unwrap(), family);
        }
        assert!(Family::ALL.windows(2).all(|pair| pair[0] < pair[1]), "ALL is in the enum's order");
    }

    #[test]
    fn every_stream_of_the_nine_pinned_logs_has_a_known_codec() {
        let streams = pinned_streams();
        assert_eq!(streams.len(), 1634);
        let mut families: BTreeMap<Family, usize> = BTreeMap::new();
        let mut atmos = 0;
        for attrs in &streams {
            let found = codec(attr(attrs, 5), attr(attrs, 6), attr(attrs, 7));
            *families.entry(found.family).or_default() += 1;
            if found.object == Some(ObjectAudio::Atmos) {
                atmos += 1;
            }
        }
        assert_eq!(
            families,
            BTreeMap::from([
                (Family::Hevc, 282),
                (Family::Avc, 85),
                (Family::Mpeg2, 34),
                (Family::TrueHd, 44),
                (Family::DtsHdMa, 14),
                (Family::Dts, 59),
                (Family::Ac3, 221),
                (Family::Pgs, 852),
                (Family::VobSub, 36),
                (Family::ClosedCaptions, 7),
            ])
        );
        assert_eq!(atmos, 42);
    }

    #[test]
    fn a_bitrate_in_kb_s_or_mb_s_reads_as_whole_kilobits_per_second() {
        assert_eq!(bitrate_kbps("192 Kb/s"), Some(192));
        assert_eq!(bitrate_kbps("640 Kb/s"), Some(640));
        assert_eq!(bitrate_kbps("768 Kb/s"), Some(768));
        assert_eq!(bitrate_kbps("1.5 Mb/s"), Some(1500));
        assert_eq!(bitrate_kbps("9.8 Mb/s"), Some(9800));
        assert_eq!(bitrate_kbps("34.9 Mb/s"), Some(34900));
        assert_eq!(bitrate_kbps("40 Mb/s"), Some(40000));
    }

    #[test]
    fn a_bitrate_is_rounded_to_the_nearest_kilobit() {
        assert_eq!(bitrate_kbps("1.2346 Mb/s"), Some(1235));
        assert_eq!(bitrate_kbps("1.2344 Mb/s"), Some(1234));
        assert_eq!(bitrate_kbps("0.4 Kb/s"), Some(0));
    }

    #[test]
    fn text_that_is_not_a_number_a_space_and_a_unit_has_no_bitrate() {
        for text in [
            "",
            "1.5",
            "Mb/s",
            "1.5Mb/s",
            "1.5 MB/s",
            "1.5 mb/s",
            "1.5 Gb/s",
            "192 kbps",
            "fast",
            "-1 Kb/s",
            "+1 Kb/s",
            "1.5.2 Mb/s",
            "1,5 Mb/s",
            ". Kb/s",
            "1e3 Kb/s",
            " 192 Kb/s",
            "192  Kb/s",
            "5000000 Mb/s",
        ] {
            assert_eq!(bitrate_kbps(text), None, "{text:?}");
        }
    }

    #[test]
    fn every_bitrate_of_the_nine_pinned_logs_parses() {
        let texts: Vec<String> = pinned_streams().iter().filter_map(|attrs| attrs.get(&13).cloned()).collect();
        assert_eq!(texts.len(), 365);
        for text in &texts {
            assert!(bitrate_kbps(text).is_some(), "{text}");
        }
    }

    #[test]
    fn the_lossless_families_are_tier_a_at_any_bitrate() {
        for family in [Family::TrueHd, Family::DtsHdMa, Family::Lpcm, Family::Flac] {
            assert_eq!(tier(family, None), Some(Tier::Lossless), "{family:?}");
            assert_eq!(tier(family, Some(128)), Some(Tier::Lossless), "{family:?}");
        }
    }

    #[test]
    fn dts_hd_high_resolution_is_tier_b_at_any_bitrate() {
        assert_eq!(tier(Family::DtsHdHr, None), Some(Tier::HighRateLossy));
        assert_eq!(tier(Family::DtsHdHr, Some(1000)), Some(Tier::HighRateLossy));
    }

    #[test]
    fn e_ac_3_is_tier_b_from_768_kbps_and_tier_c_below_or_without_a_rate() {
        assert_eq!(tier(Family::Eac3, Some(768)), Some(Tier::HighRateLossy));
        assert_eq!(tier(Family::Eac3, Some(1536)), Some(Tier::HighRateLossy));
        assert_eq!(tier(Family::Eac3, Some(767)), Some(Tier::OtherLossy));
        assert_eq!(tier(Family::Eac3, Some(640)), Some(Tier::OtherLossy));
        assert_eq!(tier(Family::Eac3, None), Some(Tier::OtherLossy));
    }

    #[test]
    fn dts_is_tier_b_from_1_5_mbps_and_tier_c_below_or_without_a_rate() {
        assert_eq!(tier(Family::Dts, bitrate_kbps("1.5 Mb/s")), Some(Tier::HighRateLossy));
        assert_eq!(tier(Family::Dts, Some(1509)), Some(Tier::HighRateLossy));
        assert_eq!(tier(Family::Dts, Some(1499)), Some(Tier::OtherLossy));
        assert_eq!(tier(Family::Dts, bitrate_kbps("768 Kb/s")), Some(Tier::OtherLossy));
        assert_eq!(tier(Family::Dts, None), Some(Tier::OtherLossy));
    }

    #[test]
    fn ac_3_is_tier_c_at_any_bitrate() {
        assert_eq!(tier(Family::Ac3, Some(640)), Some(Tier::OtherLossy));
        assert_eq!(tier(Family::Ac3, Some(448)), Some(Tier::OtherLossy));
        assert_eq!(tier(Family::Ac3, None), Some(Tier::OtherLossy));
    }

    #[test]
    fn mpeg_audio_and_aac_are_tier_d() {
        for family in [Family::MpegAudio, Family::Aac] {
            assert_eq!(tier(family, None), Some(Tier::Low), "{family:?}");
            assert_eq!(tier(family, Some(320)), Some(Tier::Low), "{family:?}");
        }
    }

    #[test]
    fn video_subtitle_and_unknown_families_have_no_tier() {
        for family in [
            Family::Hevc,
            Family::Avc,
            Family::Mpeg2,
            Family::Vc1,
            Family::Pgs,
            Family::VobSub,
            Family::ClosedCaptions,
            Family::Unknown,
        ] {
            assert_eq!(tier(family, None), None, "{family:?}");
            assert_eq!(tier(family, Some(9800)), None, "{family:?}");
        }
    }

    #[test]
    fn tiers_sort_better_first_and_are_spelled_in_snake_case() {
        assert!(Tier::Lossless < Tier::HighRateLossy);
        assert!(Tier::HighRateLossy < Tier::OtherLossy);
        assert!(Tier::OtherLossy < Tier::Low);
        for (value, text) in [
            (Tier::Lossless, "lossless"),
            (Tier::HighRateLossy, "high_rate_lossy"),
            (Tier::OtherLossy, "other_lossy"),
            (Tier::Low, "low"),
        ] {
            assert_eq!(value.as_str(), text);
            assert_eq!(serde_json::to_value(value).unwrap(), Value::String(text.to_string()));
            assert_eq!(serde_json::from_value::<Tier>(Value::String(text.to_string())).unwrap(), value);
        }
        assert_eq!(EAC3_HIGH_RATE_KBPS, 768);
        assert_eq!(DTS_HIGH_RATE_KBPS, 1500);
    }

    #[test]
    fn channel_counts_name_their_layouts_as_8_6_lists_them() {
        assert_eq!(channel_layout(1), "1.0");
        assert_eq!(channel_layout(2), "2.0");
        assert_eq!(channel_layout(6), "5.1");
        assert_eq!(channel_layout(7), "6.1");
        assert_eq!(channel_layout(8), "7.1");
    }

    #[test]
    fn other_channel_counts_are_named_n_point_0() {
        assert_eq!(channel_layout(0), "0.0");
        assert_eq!(channel_layout(3), "3.0");
        assert_eq!(channel_layout(4), "4.0");
        assert_eq!(channel_layout(5), "5.0");
        assert_eq!(channel_layout(16), "16.0");
    }

    #[test]
    fn every_channel_count_of_the_nine_pinned_logs_has_its_layout_name() {
        let mut layouts: BTreeMap<String, usize> = BTreeMap::new();
        for attrs in pinned_streams() {
            if let Some(count) = attrs.get(&14) {
                *layouts.entry(channel_layout(count.parse().unwrap())).or_default() += 1;
            }
        }
        assert_eq!(
            layouts,
            BTreeMap::from([("2.0".to_string(), 151), ("5.1".to_string(), 143), ("7.1".to_string(), 44)])
        );
    }

    #[test]
    fn ffprobe_profiles_naming_atmos_or_dts_x_give_the_extension() {
        assert_eq!(object_audio(Some("Dolby TrueHD + Dolby Atmos")), Some(ObjectAudio::Atmos));
        assert_eq!(object_audio(Some("DTS-HD MA + DTS:X")), Some(ObjectAudio::DtsX));
        assert_eq!(object_audio(Some("DTS:X")), Some(ObjectAudio::DtsX));
        assert_eq!(object_audio(Some("DTS-HD MA")), None);
        assert_eq!(object_audio(Some("")), None);
        assert_eq!(object_audio(None), None);
    }

    #[test]
    fn the_object_extensions_are_spelled_atmos_and_dts_x() {
        for (value, text) in [(ObjectAudio::Atmos, "Atmos"), (ObjectAudio::DtsX, "DTS:X")] {
            assert_eq!(value.as_str(), text);
            assert_eq!(serde_json::to_value(value).unwrap(), Value::String(text.to_string()));
            assert_eq!(serde_json::from_value::<ObjectAudio>(Value::String(text.to_string())).unwrap(), value);
        }
    }

    #[test]
    fn mkvmerge_names_are_the_ones_6_2_step_7_records_for_every_family() {
        let names = |family: Family| mkvmerge_names(&Codec { family, display: String::new(), object: None, lossless: false });
        assert_eq!(names(Family::Hevc), ["HEVC/H.265/MPEG-H"]);
        assert_eq!(names(Family::Avc), ["AVC/H.264/MPEG-4p10"]);
        assert_eq!(names(Family::Mpeg2), ["MPEG-1/2"]);
        assert_eq!(names(Family::Vc1), ["VC-1"]);
        assert_eq!(names(Family::TrueHd), ["TrueHD", "TrueHD Atmos"]);
        assert_eq!(names(Family::DtsHdMa), ["DTS-HD Master Audio"]);
        assert_eq!(names(Family::DtsHdHr), ["DTS-HD High Resolution Audio"]);
        assert_eq!(names(Family::Dts), ["DTS", "DTS-ES", "DTS 96/24"]);
        assert_eq!(names(Family::DtsExpress), ["DTS Express"]);
        assert_eq!(names(Family::Ac3), ["AC-3"]);
        assert_eq!(names(Family::Eac3), ["E-AC-3"]);
        assert_eq!(names(Family::Lpcm), ["PCM"]);
        assert_eq!(names(Family::Flac), ["FLAC"]);
        assert_eq!(names(Family::MpegAudio), ["MP2"]);
        assert_eq!(names(Family::Aac), ["AAC"]);
        assert_eq!(names(Family::Pgs), ["HDMV PGS"]);
        assert_eq!(names(Family::VobSub), ["VobSub"]);
        assert_eq!(names(Family::ClosedCaptions), ["SubRip/SRT"]);
        assert!(names(Family::Unknown).is_empty());
    }

    #[test]
    fn attribute_41_short_names_give_the_output_codecs_family() {
        assert_eq!(output_codec("FLAC").family, Family::Flac);
        assert_eq!(output_codec("LPCM").family, Family::Lpcm);
        assert_eq!(output_codec("DD").family, Family::Ac3);
        assert_eq!(output_codec("E-AC3").family, Family::Eac3);
        assert_eq!(output_codec("Text").family, Family::ClosedCaptions);
        assert_eq!(mkvmerge_names(&output_codec("Text")), ["SubRip/SRT"]);
        assert_eq!(output_codec("FLAC"), known(Family::Flac, "FLAC", None, true));
        // An attribute 5 spelling still resolves; a value neither knows is Unknown, named by itself.
        assert_eq!(output_codec("A_FLAC").family, Family::Flac);
        assert_eq!(output_codec("Made Up"), known(Family::Unknown, "Made Up", None, false));
    }

    #[test]
    fn the_recorded_codecs_give_the_names_mkvmerge_wrote_for_them() {
        for (id, short, long, written) in [
            ("V_MPEGH/ISO/HEVC", "MpegH", "MpegH HEVC Main10@L5.1", "HEVC/H.265/MPEG-H"),
            ("V_MPEG4/ISO/AVC", "Mpeg4", "Mpeg4 AVC High@L4.1", "AVC/H.264/MPEG-4p10"),
            ("V_MPEG2", "Mpeg2", "Mpeg2", "MPEG-1/2"),
            ("A_DTS", "DTS-HD MA", "DTS-HD Master Audio", "DTS-HD Master Audio"),
            ("A_DTS", "DTS", "DTS", "DTS"),
            ("A_AC3", "DD", "Dolby Digital", "AC-3"),
            ("S_HDMV/PGS", "PGS", "HDMV PGS Subtitles", "HDMV PGS"),
            ("S_VOBSUB", "", "Dvd Subtitles", "VobSub"),
            ("S_CC608/DVD", "CC", "Closed Captions", "SubRip/SRT"),
        ] {
            assert!(mkvmerge_names(&codec(id, short, long)).contains(&written), "{id} {short} {long}");
        }
    }
}

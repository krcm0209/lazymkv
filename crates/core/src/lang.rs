//! The language code tables of the phase 0 analysers, as the Python has them (port spec section 3.1): the
//! reconciler's bibliographic-to-terminology mapping (compare_tracks.B_TO_T) and the audio rule's tables
//! (audio_similarity.ISO639, NO_LANGUAGE, TMDB_NO_LANGUAGE; spec 8.6 "Languages").
//!
//! Phase 1 adds the wanted languages of R4 and spec 8.6 "Languages" over the same tables (`Wanted`,
//! `wanted_languages`, `is_english`, `is_no_language`), written from the spec (RP20); Tasks 5, 7 to 11 and 13 use
//! them.

/// ISO 639-2 bibliographic code -> terminology code: the 20 pairs of the Python, in its order.
/// Py: compare_tracks.B_TO_T.
pub const B_TO_T: [(&str, &str); 20] = [
    ("alb", "sqi"), ("arm", "hye"), ("baq", "eus"), ("bur", "mya"), ("chi", "zho"), ("cze", "ces"),
    ("dut", "nld"), ("fre", "fra"), ("geo", "kat"), ("ger", "deu"), ("gre", "ell"), ("ice", "isl"),
    ("mac", "mkd"), ("mao", "mri"), ("may", "msa"), ("per", "fas"), ("rum", "ron"), ("slo", "slk"),
    ("tib", "bod"), ("wel", "cym"),
];

/// `(code or "und").lower()`, then mapped through `B_TO_T` (None and "" are "und"). Py: compare_tracks.norm_lang.
pub fn norm_lang(code: Option<&str>) -> String {
    let code = match code {
        Some(text) if !text.is_empty() => text.to_lowercase(),
        _ => "und".to_string(),
    };
    match B_TO_T.iter().find(|(bibliographic, _)| *bibliographic == code) {
        Some((_, terminology)) => terminology.to_string(),
        None => code,
    }
}

/// The codes that name no language (spec 8.6). Py: audio_similarity.NO_LANGUAGE.
pub const NO_LANGUAGE: [&str; 6] = ["", "-", "und", "zxx", "mul", "mis"];

/// TMDb's original_language of a work without speech: English is then the reference language.
/// Py: audio_similarity.TMDB_NO_LANGUAGE.
pub const TMDB_NO_LANGUAGE: &str = "xx";

/// TMDb's two-letter original_language -> the three-letter codes a stream may carry: every entry of the Python's
/// table in its order (191 entries, 223 codes). Py: audio_similarity.ISO639.
pub const ISO639: &[(&str, &[&str])] = &[
    ("aa", &["aar"]), ("ab", &["abk"]), ("ae", &["ave"]), ("af", &["afr"]), ("ak", &["aka"]), ("am", &["amh"]),
    ("an", &["arg"]), ("ar", &["ara"]), ("as", &["asm"]), ("av", &["ava"]), ("ay", &["aym"]), ("az", &["aze"]),
    ("ba", &["bak"]), ("be", &["bel"]), ("bg", &["bul"]), ("bh", &["bih"]), ("bi", &["bis"]), ("bm", &["bam"]),
    ("bn", &["ben"]), ("bo", &["bod", "tib"]), ("br", &["bre"]), ("bs", &["bos"]), ("ca", &["cat"]),
    ("ce", &["che"]), ("ch", &["cha"]), ("cn", &["zho", "chi", "yue"]), ("co", &["cos"]), ("cr", &["cre"]),
    ("cs", &["ces", "cze"]), ("cu", &["chu"]), ("cv", &["chv"]), ("cy", &["cym", "wel"]), ("da", &["dan"]),
    ("de", &["deu", "ger"]), ("dv", &["div"]), ("dz", &["dzo"]), ("ee", &["ewe"]), ("el", &["ell", "gre"]),
    ("en", &["eng"]), ("eo", &["epo"]), ("es", &["spa"]), ("et", &["est"]), ("eu", &["eus", "baq"]),
    ("fa", &["fas", "per"]), ("ff", &["ful"]), ("fi", &["fin"]), ("fj", &["fij"]), ("fo", &["fao"]),
    ("fr", &["fra", "fre"]), ("fy", &["fry"]), ("ga", &["gle"]), ("gd", &["gla"]), ("gl", &["glg"]),
    ("gn", &["grn"]), ("gu", &["guj"]), ("gv", &["glv"]), ("ha", &["hau"]), ("he", &["heb"]), ("hi", &["hin"]),
    ("ho", &["hmo"]), ("hr", &["hrv"]), ("ht", &["hat"]), ("hu", &["hun"]), ("hy", &["hye", "arm"]),
    ("hz", &["her"]), ("ia", &["ina"]), ("id", &["ind"]), ("ie", &["ile"]), ("ig", &["ibo"]), ("ii", &["iii"]),
    ("ik", &["ipk"]), ("in", &["ind"]), ("io", &["ido"]), ("is", &["isl", "ice"]), ("it", &["ita"]),
    ("iu", &["iku"]), ("iw", &["heb"]), ("ja", &["jpn"]), ("ji", &["yid"]), ("jv", &["jav"]), ("jw", &["jav"]),
    ("ka", &["kat", "geo"]), ("kg", &["kon"]), ("ki", &["kik"]), ("kj", &["kua"]), ("kk", &["kaz"]),
    ("kl", &["kal"]), ("km", &["khm"]), ("kn", &["kan"]), ("ko", &["kor"]), ("kr", &["kau"]), ("ks", &["kas"]),
    ("ku", &["kur"]), ("kv", &["kom"]), ("kw", &["cor"]), ("ky", &["kir"]), ("la", &["lat"]), ("lb", &["ltz"]),
    ("lg", &["lug"]), ("li", &["lim"]), ("ln", &["lin"]), ("lo", &["lao"]), ("lt", &["lit"]), ("lu", &["lub"]),
    ("lv", &["lav"]), ("mg", &["mlg"]), ("mh", &["mah"]), ("mi", &["mri", "mao"]), ("mk", &["mkd", "mac"]),
    ("ml", &["mal"]), ("mn", &["mon"]), ("mo", &["ron", "rum", "mol"]), ("mr", &["mar"]), ("ms", &["msa", "may"]),
    ("mt", &["mlt"]), ("my", &["mya", "bur"]), ("na", &["nau"]), ("nb", &["nob", "nor"]), ("nd", &["nde"]),
    ("ne", &["nep"]), ("ng", &["ndo"]), ("nl", &["nld", "dut"]), ("nn", &["nno", "nor"]),
    ("no", &["nor", "nob", "nno"]), ("nr", &["nbl"]), ("nv", &["nav"]), ("ny", &["nya"]), ("oc", &["oci"]),
    ("oj", &["oji"]), ("om", &["orm"]), ("or", &["ori"]), ("os", &["oss"]), ("pa", &["pan"]), ("pi", &["pli"]),
    ("pl", &["pol"]), ("ps", &["pus"]), ("pt", &["por"]), ("qu", &["que"]), ("rm", &["roh"]), ("rn", &["run"]),
    ("ro", &["ron", "rum"]), ("ru", &["rus"]), ("rw", &["kin"]), ("sa", &["san"]), ("sc", &["srd"]),
    ("sd", &["snd"]), ("se", &["sme"]), ("sg", &["sag"]), ("sh", &["hbs", "srp", "hrv", "bos"]), ("si", &["sin"]),
    ("sk", &["slk", "slo"]), ("sl", &["slv"]), ("sm", &["smo"]), ("sn", &["sna"]), ("so", &["som"]),
    ("sq", &["sqi", "alb"]), ("sr", &["srp"]), ("ss", &["ssw"]), ("st", &["sot"]), ("su", &["sun"]),
    ("sv", &["swe"]), ("sw", &["swa"]), ("ta", &["tam"]), ("te", &["tel"]), ("tg", &["tgk"]), ("th", &["tha"]),
    ("ti", &["tir"]), ("tk", &["tuk"]), ("tl", &["tgl", "fil"]), ("tn", &["tsn"]), ("to", &["ton"]),
    ("tr", &["tur"]), ("ts", &["tso"]), ("tt", &["tat"]), ("tw", &["twi"]), ("ty", &["tah"]), ("ug", &["uig"]),
    ("uk", &["ukr"]), ("ur", &["urd"]), ("uz", &["uzb"]), ("ve", &["ven"]), ("vi", &["vie"]), ("vo", &["vol"]),
    ("wa", &["wln"]), ("wo", &["wol"]), ("xh", &["xho"]), ("yi", &["yid"]), ("yo", &["yor"]), ("za", &["zha"]),
    ("zh", &["zho", "chi"]), ("zu", &["zul"]),
];

/// The three-letter codes of one two-letter code, or None when the table lacks it. Py: `ISO639.get(code)`.
pub fn iso639(code: &str) -> Option<&'static [&'static str]> {
    ISO639.iter().find(|(two, _)| *two == code).map(|(_, three)| *three)
}

/// The wanted languages of one work (R4, spec 8.6 "Languages"): English always, plus the original language when TMDb
/// gives one that is not English. Built by `wanted_languages`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Wanted {
    /// TMDb's two-letter code as given (None when TMDb is unknown).
    pub original_two_letter: Option<String>,
    /// The original language's three-letter codes (empty for English, "xx", None, or a code the table lacks).
    pub original: Vec<String>,
    /// ["eng"] always.
    pub english: Vec<String>,
    /// True when the original language is English, unknown, none, or absent from the table (8.6 "Content": English is
    /// then the reference language).
    pub original_is_english: bool,
}

/// The wanted languages (R4, spec 8.6 "Languages"): English always, plus the original language's three-letter codes
/// through `iso639` when TMDb's code is Some and not "en" and not `TMDB_NO_LANGUAGE`. The code is looked up
/// lower-cased (TMDb writes it lower-case, so "ES" reads as "es"); `original_two_letter` keeps it as given.
pub fn wanted_languages(original_language: Option<&str>) -> Wanted {
    let original: Vec<String> = match original_language.map(str::to_ascii_lowercase) {
        Some(code) if code != "en" && code != TMDB_NO_LANGUAGE => match iso639(&code) {
            Some(three) => three.iter().map(|code| code.to_string()).collect(),
            None => Vec::new(),
        },
        _ => Vec::new(),
    };
    let original_is_english = original.is_empty();
    Wanted {
        original_two_letter: original_language.map(str::to_string),
        original,
        english: vec!["eng".to_string()],
        original_is_english,
    }
}

impl Wanted {
    /// A stream's attribute 3 is a wanted code: English (`is_english`) or one of the original language's codes,
    /// ignoring ASCII case. A code that names no language is never wanted: such a stream is kept as belonging to no
    /// language (spec 8.6 "Languages").
    pub fn contains(&self, code: &str) -> bool {
        is_english(code) || self.is_original(code)
    }

    /// A stream's code is one of the original language's codes, ignoring ASCII case (never, for an English work).
    /// Spec 8.6 "Languages".
    pub fn is_original(&self, code: &str) -> bool {
        self.original.iter().any(|original| original.eq_ignore_ascii_case(code))
    }
}

/// "eng" or "en", ignoring ASCII case: an English stream (spec 8.6, 8.7).
pub fn is_english(code: &str) -> bool {
    code.eq_ignore_ascii_case("eng") || code.eq_ignore_ascii_case("en")
}

/// A stream that belongs to no language (spec 8.6 "Languages": no code, or "und"): `NO_LANGUAGE.contains(code)` after
/// lower-casing; None counts as no language.
pub fn is_no_language(code: Option<&str>) -> bool {
    match code {
        None => true,
        Some(code) => NO_LANGUAGE.contains(&code.to_lowercase().as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b_to_t_has_the_twenty_python_pairs_in_order() {
        assert_eq!(B_TO_T.len(), 20);
        assert_eq!(B_TO_T[0], ("alb", "sqi"));
        assert_eq!(B_TO_T[7], ("fre", "fra"));
        assert_eq!(B_TO_T[19], ("wel", "cym"));
    }

    #[test]
    fn norm_lang_lowers_maps_and_reads_nothing_as_und() {
        assert_eq!(norm_lang(None), "und");
        assert_eq!(norm_lang(Some("")), "und");
        assert_eq!(norm_lang(Some("FRE")), "fra");
        assert_eq!(norm_lang(Some("Ger")), "deu");
        assert_eq!(norm_lang(Some("eng")), "eng");
        assert_eq!(norm_lang(Some("ZXX")), "zxx");
        assert_eq!(norm_lang(Some("fra")), "fra");
    }

    #[test]
    fn iso639_is_the_python_table() {
        assert_eq!(ISO639.len(), 191);
        assert_eq!(ISO639.iter().map(|(_, codes)| codes.len()).sum::<usize>(), 223);
        assert_eq!(ISO639[0], ("aa", &["aar"][..]));
        assert_eq!(ISO639[190], ("zu", &["zul"][..]));
        assert_eq!(iso639("cs"), Some(&["ces", "cze"][..]));
        assert_eq!(iso639("sh"), Some(&["hbs", "srp", "hrv", "bos"][..]));
        assert_eq!(iso639("cn"), Some(&["zho", "chi", "yue"][..]));
        assert_eq!(iso639("no"), Some(&["nor", "nob", "nno"][..]));
        assert_eq!(iso639("xx"), None);
        assert_eq!(iso639("EN"), None);
    }

    #[test]
    fn no_language_codes_are_the_python_tuple() {
        assert_eq!(NO_LANGUAGE, ["", "-", "und", "zxx", "mul", "mis"]);
        assert_eq!(TMDB_NO_LANGUAGE, "xx");
    }

    fn codes(list: &[&str]) -> Vec<String> {
        list.iter().map(|code| code.to_string()).collect()
    }

    #[test]
    fn an_english_work_wants_english_only() {
        let wanted = wanted_languages(Some("en"));
        assert_eq!(
            wanted,
            Wanted {
                original_two_letter: Some("en".to_string()),
                original: Vec::new(),
                english: codes(&["eng"]),
                original_is_english: true,
            }
        );
        assert!(wanted.contains("eng"));
        assert!(!wanted.contains("spa"));
        assert!(!wanted.is_original("eng"));
    }

    #[test]
    fn a_spanish_work_wants_english_and_spanish() {
        let wanted = wanted_languages(Some("es"));
        assert_eq!(
            wanted,
            Wanted {
                original_two_letter: Some("es".to_string()),
                original: codes(&["spa"]),
                english: codes(&["eng"]),
                original_is_english: false,
            }
        );
        assert!(wanted.contains("spa"));
        assert!(wanted.contains("SPA"));
        assert!(wanted.contains("eng"));
        assert!(!wanted.contains("fra"));
        assert!(wanted.is_original("spa"));
        assert!(!wanted.is_original("eng"));
    }

    #[test]
    fn a_chinese_work_wants_every_code_of_cn() {
        let wanted = wanted_languages(Some("cn"));
        assert_eq!(wanted.original, codes(&["zho", "chi", "yue"]));
        assert!(!wanted.original_is_english);
        assert!(wanted.contains("zho") && wanted.contains("chi") && wanted.contains("yue"));
        assert!(!wanted.contains("jpn"));
    }

    #[test]
    fn a_french_work_wants_both_codes_of_the_bibliographic_pair() {
        let wanted = wanted_languages(Some("fr"));
        assert_eq!(wanted.original, codes(&["fra", "fre"]));
        assert!(wanted.contains("fre"));
        assert!(wanted.contains("fra"));
        assert!(wanted.is_original("fre"));
    }

    #[test]
    fn a_work_without_speech_wants_english_only() {
        assert_eq!(
            wanted_languages(Some("xx")),
            Wanted {
                original_two_letter: Some("xx".to_string()),
                original: Vec::new(),
                english: codes(&["eng"]),
                original_is_english: true,
            }
        );
    }

    #[test]
    fn an_unknown_original_language_wants_english_only() {
        assert_eq!(
            wanted_languages(None),
            Wanted { original_two_letter: None, original: Vec::new(), english: codes(&["eng"]), original_is_english: true }
        );
    }

    #[test]
    fn a_code_the_table_lacks_wants_english_only() {
        let wanted = wanted_languages(Some("qq"));
        assert_eq!(wanted.original_two_letter.as_deref(), Some("qq"));
        assert_eq!(wanted.original, Vec::<String>::new());
        assert!(wanted.original_is_english);
        assert!(!wanted.contains("qqq"));
    }

    #[test]
    fn a_tmdb_code_in_capitals_is_looked_up_lower_case() {
        let spanish = wanted_languages(Some("ES"));
        assert_eq!(spanish.original_two_letter.as_deref(), Some("ES"));
        assert_eq!(spanish.original, codes(&["spa"]));
        assert!(!spanish.original_is_english);
        let english = wanted_languages(Some("EN"));
        assert_eq!(english.original, Vec::<String>::new());
        assert!(english.original_is_english);
    }

    #[test]
    fn a_stream_without_a_language_is_never_a_wanted_language() {
        let wanted = wanted_languages(Some("es"));
        for code in ["und", "", "-", "zxx", "mul", "mis"] {
            assert!(!wanted.contains(code), "{code:?}");
        }
    }

    #[test]
    fn is_english_accepts_eng_and_en_in_any_case_only() {
        for code in ["eng", "en", "ENG", "En"] {
            assert!(is_english(code), "{code:?}");
        }
        for code in ["enm", "spa", "", "und", "e"] {
            assert!(!is_english(code), "{code:?}");
        }
    }

    #[test]
    fn is_no_language_reads_none_and_the_six_no_language_codes() {
        assert!(is_no_language(None));
        for code in ["", "-", "und", "UND", "zxx", "mul", "mis"] {
            assert!(is_no_language(Some(code)), "{code:?}");
        }
        for code in ["eng", "spa", "en", "xx"] {
            assert!(!is_no_language(Some(code)), "{code:?}");
        }
    }
}

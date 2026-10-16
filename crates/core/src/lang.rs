//! The language code tables of the phase 0 analysers, as the Python has them (port spec section 3.1): the
//! reconciler's bibliographic-to-terminology mapping (compare_tracks.B_TO_T) and the audio rule's tables
//! (audio_similarity.ISO639, NO_LANGUAGE, TMDB_NO_LANGUAGE; spec 8.6 "Languages").

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
}

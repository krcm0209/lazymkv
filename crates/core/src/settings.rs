//! Settings (design spec section 14) with the spec's defaults, the checks spec 7 and 14 make on the roots and on the
//! values the rules divide by or compare against (Open point 21), and the subtitle text rule built from them
//! (RP15). A settings file is a JSON object naming only the keys it changes: absent keys take `Settings::default()`,
//! an unknown key is refused. Every pure rule of phase 1 takes `&Settings` and reads only the keys its doc names;
//! the fixed values of spec 14 ("Fixed values, not settings") are constants of the rules that use them, never
//! fields here. `path_inside` is lexical (Open point 20): it resolves no link and no mapped drive. Written from the
//! spec (RP20); used by every rule, by Task 15 (`--settings FILE`) and by Tasks 16 and 18 (`text_rule`).

/// The edition layout of spec 8.9's table (setting `edition_style`, spec 14). The serde and `as_str` spelling is the
/// table's lower-case name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditionStyle {
    /// The default: one folder, files named `- <Res> <Ed> {edition-<Ed>}` (a Plex edition and a Jellyfin version
    /// label; not yet verified, EXP-09 not run).
    Combined,
    /// One folder per edition, named with `{edition-<Ed>}` (the layout Plex documents).
    Plex,
    /// One folder, files named `- <Res> <Ed>` (Jellyfin's version picker).
    Jellyfin,
}

impl EditionStyle {
    /// Every style, in spec 8.9's table order.
    pub const ALL: [EditionStyle; 3] = [EditionStyle::Combined, EditionStyle::Plex, EditionStyle::Jellyfin];

    /// "combined", "plex" or "jellyfin", the same string serde writes. Spec 8.9.
    pub fn as_str(self) -> &'static str {
        match self {
            EditionStyle::Combined => "combined",
            EditionStyle::Plex => "plex",
            EditionStyle::Jellyfin => "jellyfin",
        }
    }
}

impl std::str::FromStr for EditionStyle {
    type Err = String;

    /// Only the exact spellings of `as_str` are accepted.
    fn from_str(text: &str) -> Result<EditionStyle, String> {
        EditionStyle::ALL
            .into_iter()
            .find(|style| style.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not an edition style (combined, plex or jellyfin; spec 8.9)"))
    }
}

/// Spec 14 "tool paths": overrides for 4.4's programs (MakeMKV, MKVToolNix, ffprobe, the Java runtime) plus ffmpeg
/// (6.4 step 10a); None means auto. A settings file may name only the programs it overrides; an unknown program is
/// refused.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToolPaths {
    /// MakeMKV's `makemkvcon` (4.4).
    pub makemkvcon: Option<String>,
    /// MKVToolNix's `mkvmerge` (4.4).
    pub mkvmerge: Option<String>,
    /// FFmpeg's `ffprobe` (4.4).
    pub ffprobe: Option<String>,
    /// `ffmpeg`, which 6.4 step 10a runs.
    pub ffmpeg: Option<String>,
    /// The Java runtime MakeMKV uses for Blu-ray menus (4.4; 8.3 rule 2 needs it).
    pub java: Option<String>,
}

/// Every key of spec 14 with its default (`Settings::default()` is the table's Default column), in the table's
/// order. A settings file may name only the keys it changes: absent keys take the default (`#[serde(default)]`); an
/// unknown key is refused (`deny_unknown_fields`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Spec 7: where rips and disc copies are staged, a local or UNC path; None until set. Required by the engine,
    /// not by any rule here.
    pub staging_root: Option<String>,
    /// Spec 7 and 8.9: the movie library's root, an absolute local or UNC path (`validate` refuses a blank or relative
    /// one); None until set.
    pub movies_root: Option<String>,
    /// Spec 7 and 8.9: the TV library's root, an absolute local or UNC path; None until set.
    pub tv_root: Option<String>,
    /// 120: MakeMKV's minimum title length in seconds (`--minlength`, 9.2; 8.2's candidate filter reads it).
    pub minlength_s: u32,
    /// 1024: MakeMKV's read cache in MB (`--cache`, 9.2; not yet verified, EXP-04 not run).
    pub cache_mb: u32,
    /// 0.85: the confidence needed to file automatically (8.8; 6.2 and 6.3 choose the mode by it).
    pub auto_threshold: f64,
    /// 45: the shortest title considered a feature, in minutes (8.3 rule 3.2).
    pub feature_min_minutes: u32,
    /// true: D9, a TMDb-only identification may file automatically (8.1 caps it at 0.84 when false).
    pub tmdb_only_autofile: bool,
    /// true: D9, channel count is compared before fidelity tier when ordering audio (8.6 step 3).
    pub prefer_most_channels: bool,
    /// true: D9, original-language audio first for non-English works (8.6 step 5).
    pub original_language_first: bool,
    /// 300: the start-time tolerance of subtitle alignment, in milliseconds (8.7 "Alignment").
    pub subs_tolerance_ms: u32,
    /// 0.70: two full tracks aligned at least this much are in the same group (8.7; provisional, EXP-06 not run).
    pub subs_same_min: f64,
    /// 0.30: two full tracks aligned at most this much are in different groups (8.7; provisional, EXP-06 not run).
    pub subs_diff_max: f64,
    /// 64: image-subtitle packets this small clear the screen and are not events (8.7 "Events").
    pub sub_clear_max_bytes: i64,
    /// Combined: the edition layout (8.9; not yet verified, EXP-09 not run).
    pub edition_style: EditionStyle,
    /// false: add the square-bracket ID tag for Jellyfin 10.x (8.9).
    pub jellyfin10_compat: bool,
    /// 7: days the raw data of filed jobs is kept (7 "Retention").
    pub retention_days: u32,
    /// true: D16, keep the tuning evidence of every ripped title (6.4 step 10a, 7 "Tuning evidence").
    pub keep_tuning_evidence: bool,
    /// None: where the tuning evidence is kept; required while keep_tuning_evidence is on; not inside staging_root or
    /// a library root (7, 14).
    pub tuning_root: Option<String>,
    /// 1: simultaneous file-phase jobs (6).
    pub file_phase_concurrency: u32,
    /// 10: seconds to wait after a disc arrives before touching the drive (6.1 step 1).
    pub drive_settle_s: u32,
    /// 120: seconds to wait for the volume to mount (6.1 step 1).
    pub drive_ready_timeout_s: u32,
    /// 30: the longest allowed scan, in minutes (10.4).
    pub scan_timeout_min: u32,
    /// 300: the watchdog's stall time, in seconds (10.4).
    pub stall_timeout_s: u32,
    /// 20: read errors per minute that mark a drive as struggling (10.4).
    pub read_error_rate: u32,
    /// 30: minutes to wait for a stuck makemkvcon to exit (10.4).
    pub exit_wait_min: u32,
    /// "2.0.0": the lowest supported MakeMKV (9.1); `version_triple` reads it.
    pub makemkv_min_version: String,
    /// Auto: overrides for the external programs (spec 14 "tool paths").
    pub tool_paths: ToolPaths,
}

impl Default for Settings {
    /// Spec 14's Default column.
    fn default() -> Settings {
        Settings {
            staging_root: None,
            movies_root: None,
            tv_root: None,
            minlength_s: 120,
            cache_mb: 1024,
            auto_threshold: 0.85,
            feature_min_minutes: 45,
            tmdb_only_autofile: true,
            prefer_most_channels: true,
            original_language_first: true,
            subs_tolerance_ms: 300,
            subs_same_min: 0.70,
            subs_diff_max: 0.30,
            sub_clear_max_bytes: 64,
            edition_style: EditionStyle::Combined,
            jellyfin10_compat: false,
            retention_days: 7,
            keep_tuning_evidence: true,
            tuning_root: None,
            file_phase_concurrency: 1,
            drive_settle_s: 10,
            drive_ready_timeout_s: 120,
            scan_timeout_min: 30,
            stall_timeout_s: 300,
            read_error_rate: 20,
            exit_wait_min: 30,
            makemkv_min_version: "2.0.0".to_string(),
            tool_paths: ToolPaths::default(),
        }
    }
}

/// True when `inner` is `outer` or a path below it: both split on '/' and '\\', empty and trailing components
/// dropped, compared component by component ignoring ASCII case (Windows and SMB shares are case-insensitive); a UNC
/// prefix `\\server\share` is two components. A path with no component (empty, or separators only) holds nothing and
/// is inside nothing: a blank root is a missing setting. Lexical only: no link or mapped drive is resolved (Open
/// point 20). Spec 7 "The staging root may not be inside a library root".
pub fn path_inside(inner: &str, outer: &str) -> bool {
    let inner = components(inner);
    let outer = components(outer);
    !outer.is_empty()
        && inner.len() >= outer.len()
        && outer.iter().zip(&inner).all(|(outer, inner)| outer.eq_ignore_ascii_case(inner))
}

/// True when `a` and `b` name the same file: each is inside the other under `path_inside`, so the comparison is
/// lexical, ignores ASCII case, and treats '/' and '\\', repeated separators and a trailing separator alike. A path with
/// no component is the same as nothing, not even as another blank path. The one equality rule of a final path: the
/// owner's collision answer and the same-job review both read it. Spec 7 ("Path collisions add `path_collision`"), 8.9
/// ("A second copy at the same resolution and edition is a `path_collision`"). Use: `decision::same_path_reviews`,
/// `remembered::Overrides::collision_for`.
pub fn same_path(a: &str, b: &str) -> bool {
    path_inside(a, b) && path_inside(b, a)
}

/// A path's components: split on '/' and '\\', empty ones dropped.
fn components(path: &str) -> Vec<&str> {
    path.split(|c: char| c == '/' || c == '\\').filter(|component| !component.is_empty()).collect()
}

/// A root setting as a path: None when it is absent or blank (no path component once surrounding whitespace is set
/// aside, such as "", "  " or "/"), which is a missing setting (spec 14), so no final path is ever built under it. The
/// one test of a blank root: `validate` and `decision::decide`'s naming both read it. Spec 7, 14. Def: PR #3 review
/// (F14). Use: T13.
pub fn configured_root(root: Option<&str>) -> Option<&str> {
    root.filter(|text| !components(text.trim()).is_empty())
}

/// Whether a library root is an absolute path: a UNC path (`\\server\share`, or with forward slashes), a
/// drive-letter path (`D:\Movies`, `D:/Movies`) or a path from the root of a POSIX file system (`/srv/media/Movies`).
/// A relative path (`Movies`), a drive-relative one (`D:Movies`) or a path with leading whitespace is not. Spec 7, 14.
/// Def: PR #3 review (F14). Use: T1.
pub fn is_absolute_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/');
    path.starts_with("\\\\") || path.starts_with('/') || drive
}

/// The version string `N.N.N` (three runs of ASCII digits joined by '.') as a comparable triple; None otherwise (a
/// sign, a prefix, a suffix, a missing or fourth part, or a part above u32::MAX). Spec 9.1.
pub fn version_triple(text: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = text.split('.').collect();
    let [major, minor, patch] = parts.as_slice() else {
        return None;
    };
    Some((version_part(major)?, version_part(minor)?, version_part(patch)?))
}

/// One part of a version: ASCII digits only, at least one, within u32.
fn version_part(part: &str) -> Option<u32> {
    if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

/// One refused setting (spec 7, 14): the key it concerns, as spec 14 spells it, and one plain sentence naming the value
/// and the rule it breaks.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SettingsProblem {
    /// The settings key.
    pub key: String,
    /// The sentence.
    pub problem: String,
}

/// A SettingsProblem for `key`.
fn problem(key: &str, text: impl Into<String>) -> SettingsProblem {
    SettingsProblem { key: key.to_string(), problem: text.into() }
}

impl Settings {
    /// Every problem, in key order (spec 14's table order, which is the struct's): staging_root inside movies_root or
    /// tv_root, or equal to one (one problem per root, by `path_inside`); movies_root and tv_root, when set, blank
    /// (`configured_root`) or not absolute (`is_absolute_root`): a final path is never built under either (8.9);
    /// minlength_s 0; auto_threshold outside
    /// 0.0..=1.0 (not-a-number included); feature_min_minutes 0; subs_diff_max above subs_same_min; tuning_root
    /// absent or blank (no path component, so it is inside nothing) while keep_tuning_evidence is on; a set tuning_root
    /// inside staging_root or a library root, whatever keep_tuning_evidence says (one problem per root);
    /// file_phase_concurrency 0; makemkv_min_version not `N.N.N` (`version_triple`). Empty when valid.
    /// `Settings::default().validate()` is exactly one problem, keyed `tuning_root` (spec 14's defaults turn
    /// keep_tuning_evidence on and leave tuning_root unset); `lazymkv-dev plan` prints the problems and plans with
    /// them. Spec 7, 14, Open point 21.
    pub fn validate(&self) -> Vec<SettingsProblem> {
        let mut problems = Vec::new();
        if let Some(staging) = &self.staging_root {
            for (key, library) in [("movies_root", &self.movies_root), ("tv_root", &self.tv_root)] {
                if let Some(library) = library {
                    if path_inside(staging, library) {
                        problems.push(problem(
                            "staging_root",
                            format!(
                                "staging_root {staging} is inside {key} {library}: the staging root may not be inside a library root (spec 7)"
                            ),
                        ));
                    }
                }
            }
        }
        for (key, library) in [("movies_root", &self.movies_root), ("tv_root", &self.tv_root)] {
            let Some(text) = library.as_deref() else {
                continue;
            };
            if configured_root(Some(text)).is_none() {
                problems.push(problem(key, format!("{key} is blank: a library root must be an absolute path (spec 7, 14)")));
            } else if !is_absolute_root(text) {
                problems.push(problem(
                    key,
                    format!(
                        "{key} {text} is not an absolute path: a library root must be a UNC path, a drive-letter path or a path from / (spec 7, 14)"
                    ),
                ));
            }
        }
        if self.minlength_s == 0 {
            problems.push(problem(
                "minlength_s",
                "minlength_s is 0: MakeMKV's minimum title length must be at least 1 second (spec 14)",
            ));
        }
        if !(0.0..=1.0).contains(&self.auto_threshold) {
            problems.push(problem(
                "auto_threshold",
                format!("auto_threshold {} is outside 0 to 1: it is a confidence (spec 8.8)", self.auto_threshold),
            ));
        }
        if self.feature_min_minutes == 0 {
            problems.push(problem(
                "feature_min_minutes",
                "feature_min_minutes is 0: the shortest feature must be at least 1 minute (spec 8.3)",
            ));
        }
        if self.subs_diff_max > self.subs_same_min {
            problems.push(problem(
                "subs_diff_max",
                format!(
                    "subs_diff_max {} is above subs_same_min {}: the two alignment bands may not overlap (spec 8.7)",
                    self.subs_diff_max, self.subs_same_min
                ),
            ));
        }
        let tuning = configured_root(self.tuning_root.as_deref());
        match tuning {
            None => {
                if self.keep_tuning_evidence {
                    problems.push(problem(
                        "tuning_root",
                        "tuning_root is not set: it is required while keep_tuning_evidence is on (spec 7, 14)",
                    ));
                }
            }
            Some(tuning) => {
                let roots = [
                    ("staging_root", &self.staging_root),
                    ("movies_root", &self.movies_root),
                    ("tv_root", &self.tv_root),
                ];
                for (key, other) in roots {
                    if let Some(other) = other {
                        if path_inside(tuning, other) {
                            problems.push(problem(
                                "tuning_root",
                                format!(
                                    "tuning_root {tuning} is inside {key} {other}: the tuning root may not be inside the staging root or a library root (spec 7)"
                                ),
                            ));
                        }
                    }
                }
            }
        }
        if self.file_phase_concurrency == 0 {
            problems.push(problem(
                "file_phase_concurrency",
                "file_phase_concurrency is 0: at least one file-phase job must be able to run (spec 6, 14)",
            ));
        }
        if version_triple(&self.makemkv_min_version).is_none() {
            problems.push(problem(
                "makemkv_min_version",
                format!(
                    "makemkv_min_version {} is not a version of the form N.N.N (spec 9.1)",
                    self.makemkv_min_version
                ),
            ));
        }
        problems
    }
}

impl Settings {
    /// A JSON settings text, read and checked (ruling RP22): keys absent from the text take Settings::default()'s
    /// value. Err, one sentence, only when the text cannot be read: text that is not JSON (serde_json's message),
    /// text that is not one JSON object, or a key the settings lack or whose value cannot be read into its type
    /// (`<key>: <serde's message>`, the first such key in the text's order: each key is read on its own as a Settings
    /// with only that key, so serde's message is tied to it). Ok holds the settings with validate()'s problems,
    /// possibly none: a caller that needs a valid file checks the vector, and `lazymkv-dev plan` (Task 15) prints
    /// them and plans anyway. Spec 14.
    pub fn from_json(text: &str) -> Result<(Settings, Vec<SettingsProblem>), String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
        let serde_json::Value::Object(map) = value else {
            return Err("a settings file is one JSON object of spec 14's keys".to_string());
        };
        for (key, value) in &map {
            let mut one = serde_json::Map::new();
            one.insert(key.clone(), value.clone());
            if let Err(err) = serde_json::from_value::<Settings>(serde_json::Value::Object(one)) {
                return Err(format!("{key}: {err}"));
            }
        }
        let settings: Settings = serde_json::from_value(serde_json::Value::Object(map)).map_err(|err| err.to_string())?;
        let problems = settings.validate();
        Ok((settings, problems))
    }
}

impl Settings {
    /// Milliseconds as seconds for the alignment rule (`subs_tolerance_ms as f64 / 1000.0`; 300 gives 0.3). Spec 8.7.
    pub fn subs_tolerance_s(&self) -> f64 {
        f64::from(self.subs_tolerance_ms) / 1000.0
    }

    /// The text rule with this file's subtitle settings: CLEAR_MAX_BYTES = sub_clear_max_bytes, TOL_S =
    /// subs_tolerance_s(), SAME_MIN = subs_same_min, DIFF_MAX = subs_diff_max, every other field
    /// `subtitle::text::Rule::default()`'s; pinned by `the_subtitle_defaults_equal_the_text_rules_defaults`
    /// (`Settings::default().text_rule() == text::Rule::default()`). Spec 8.7, 14, RP15.
    pub fn text_rule(&self) -> crate::subtitle::text::Rule {
        crate::subtitle::text::Rule {
            CLEAR_MAX_BYTES: self.sub_clear_max_bytes,
            TOL_S: self.subs_tolerance_s(),
            SAME_MIN: self.subs_same_min,
            DIFF_MAX: self.subs_diff_max,
            ..crate::subtitle::text::Rule::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_is_the_default_column_of_spec_14() {
        let settings = Settings::default();
        assert_eq!(settings.staging_root, None);
        assert_eq!(settings.movies_root, None);
        assert_eq!(settings.tv_root, None);
        assert_eq!(settings.minlength_s, 120);
        assert_eq!(settings.cache_mb, 1024);
        assert_eq!(settings.auto_threshold, 0.85);
        assert_eq!(settings.feature_min_minutes, 45);
        assert!(settings.tmdb_only_autofile);
        assert!(settings.prefer_most_channels);
        assert!(settings.original_language_first);
        assert_eq!(settings.subs_tolerance_ms, 300);
        assert_eq!(settings.subs_same_min, 0.70);
        assert_eq!(settings.subs_diff_max, 0.30);
        assert_eq!(settings.sub_clear_max_bytes, 64);
        assert_eq!(settings.edition_style, EditionStyle::Combined);
        assert!(!settings.jellyfin10_compat);
        assert_eq!(settings.retention_days, 7);
        assert!(settings.keep_tuning_evidence);
        assert_eq!(settings.tuning_root, None);
        assert_eq!(settings.file_phase_concurrency, 1);
        assert_eq!(settings.drive_settle_s, 10);
        assert_eq!(settings.drive_ready_timeout_s, 120);
        assert_eq!(settings.scan_timeout_min, 30);
        assert_eq!(settings.stall_timeout_s, 300);
        assert_eq!(settings.read_error_rate, 20);
        assert_eq!(settings.exit_wait_min, 30);
        assert_eq!(settings.makemkv_min_version, "2.0.0");
        assert_eq!(
            settings.tool_paths,
            ToolPaths { makemkvcon: None, mkvmerge: None, ffprobe: None, ffmpeg: None, java: None }
        );
    }

    #[test]
    fn the_defaults_serialise_as_spec_14s_keys_in_its_order() {
        let text = serde_json::to_string(&Settings::default()).unwrap();
        assert_eq!(
            text,
            concat!(
                r#"{"staging_root":null,"movies_root":null,"tv_root":null,"minlength_s":120,"cache_mb":1024,"#,
                r#""auto_threshold":0.85,"feature_min_minutes":45,"tmdb_only_autofile":true,"#,
                r#""prefer_most_channels":true,"original_language_first":true,"subs_tolerance_ms":300,"#,
                r#""subs_same_min":0.7,"subs_diff_max":0.3,"sub_clear_max_bytes":64,"edition_style":"combined","#,
                r#""jellyfin10_compat":false,"retention_days":7,"keep_tuning_evidence":true,"tuning_root":null,"#,
                r#""file_phase_concurrency":1,"drive_settle_s":10,"drive_ready_timeout_s":120,"scan_timeout_min":30,"#,
                r#""stall_timeout_s":300,"read_error_rate":20,"exit_wait_min":30,"makemkv_min_version":"2.0.0","#,
                r#""tool_paths":{"makemkvcon":null,"mkvmerge":null,"ffprobe":null,"ffmpeg":null,"java":null}}"#,
            )
        );
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), Settings::default());
    }

    #[test]
    fn edition_styles_are_spelled_as_spec_8_9_writes_them() {
        let spellings: Vec<&str> = EditionStyle::ALL.iter().map(|style| style.as_str()).collect();
        assert_eq!(spellings, ["combined", "plex", "jellyfin"]);
        for style in EditionStyle::ALL {
            let text = serde_json::Value::String(style.as_str().to_string());
            assert_eq!(serde_json::to_value(style).unwrap(), text);
            assert_eq!(serde_json::from_value::<EditionStyle>(text).unwrap(), style);
            assert_eq!(style.as_str().parse::<EditionStyle>(), Ok(style));
        }
    }

    #[test]
    fn an_edition_style_outside_the_three_is_refused() {
        assert_eq!(
            "Plex".parse::<EditionStyle>(),
            Err("\"Plex\" is not an edition style (combined, plex or jellyfin; spec 8.9)".to_string())
        );
        assert_eq!(
            "".parse::<EditionStyle>(),
            Err("\"\" is not an edition style (combined, plex or jellyfin; spec 8.9)".to_string())
        );
        assert!(serde_json::from_str::<EditionStyle>(r#""versions""#).is_err());
    }

    #[test]
    fn tool_paths_default_to_auto_and_name_each_program() {
        assert_eq!(
            ToolPaths::default(),
            ToolPaths { makemkvcon: None, mkvmerge: None, ffprobe: None, ffmpeg: None, java: None }
        );
        let paths: ToolPaths = serde_json::from_str(
            r#"{"makemkvcon": "C:\\Program Files (x86)\\MakeMKV\\makemkvcon64.exe", "java": "C:\\Java\\bin\\java.exe"}"#,
        )
        .unwrap();
        assert_eq!(
            paths,
            ToolPaths {
                makemkvcon: Some(r"C:\Program Files (x86)\MakeMKV\makemkvcon64.exe".to_string()),
                mkvmerge: None,
                ffprobe: None,
                ffmpeg: None,
                java: Some(r"C:\Java\bin\java.exe".to_string()),
            }
        );
        assert!(serde_json::from_str::<ToolPaths>(r#"{"vlc": "vlc.exe"}"#).is_err());
    }

    #[test]
    fn a_path_is_inside_itself_and_below_itself_but_not_above() {
        assert!(path_inside("/srv/media/Movies", "/srv/media/Movies"));
        assert!(path_inside("/srv/media/Movies/lazymkv-staging", "/srv/media/Movies"));
        assert!(!path_inside("/srv/media", "/srv/media/Movies"));
        assert!(!path_inside("/srv/media/Shows", "/srv/media/Movies"));
    }

    #[test]
    fn a_sibling_that_shares_a_prefix_is_not_inside() {
        assert!(!path_inside(r"\\unas-pro\media-staging", r"\\unas-pro\media"));
        assert!(!path_inside(r"\\unas-pro\media\Movies2", r"\\unas-pro\media\Movies"));
    }

    #[test]
    fn a_root_inside_another_is_refused_whatever_the_case_or_trailing_separator() {
        assert!(path_inside(r"\\UNAS-PRO\Media\movies\lazymkv-staging\", r"\\unas-pro\media\Movies"));
        assert!(path_inside(r"\\unas-pro\media\Movies\", r"\\UNAS-PRO\MEDIA\MOVIES\\"));
        assert!(path_inside("D:/Media/Movies/", r"d:\media\movies"));
        assert!(!path_inside(r"\\UNAS-PRO\media\lazymkv-staging\", r"\\unas-pro\media\Movies"));
    }

    #[test]
    fn forward_slashes_back_slashes_and_repeated_separators_are_one_separator() {
        assert!(path_inside(r"\\unas-pro/media//Movies\\x", r"\\unas-pro\media\Movies"));
        assert!(path_inside("/srv//media///Movies", "/srv/media/Movies/"));
    }

    #[test]
    fn the_same_path_ignores_case_separators_and_a_trailing_separator_but_not_a_different_name() {
        assert!(same_path(
            r"\\UNAS-PRO\Media\Movies\Dune (2021)\Dune (2021).mkv",
            "//unas-pro/media//movies/dune (2021)/dune (2021).mkv/"
        ));
        assert!(same_path("/srv/media/Movies", "/srv/media/Movies"));
        assert!(!same_path("/srv/media/Movies/Dune (2021)", "/srv/media/Movies/Dune (2022)"));
        assert!(!same_path("/srv/media/Movies", "/srv/media/Movies/Dune"));
        assert!(!same_path("", ""));
    }

    #[test]
    fn a_unc_prefix_is_two_components() {
        assert!(path_inside(r"\\unas-pro\media", r"\\unas-pro"));
        assert!(path_inside(r"\\unas-pro\media\Movies", r"\\unas-pro\media"));
        assert!(!path_inside(r"\\other\media", r"\\unas-pro\media"));
        assert!(!path_inside(r"\\unas-pro\other", r"\\unas-pro\media"));
    }

    #[test]
    fn an_empty_path_holds_nothing_and_is_inside_nothing() {
        assert!(!path_inside("", ""));
        assert!(!path_inside("/srv/media", ""));
        assert!(!path_inside("", "/srv/media"));
        assert!(!path_inside("/srv/media", "/"));
        assert!(!path_inside(r"\\", r"\\"));
    }

    #[test]
    fn version_triple_reads_three_numbers_joined_by_dots() {
        assert_eq!(version_triple("2.0.0"), Some((2, 0, 0)));
        assert_eq!(version_triple("1.18.3"), Some((1, 18, 3)));
        assert_eq!(version_triple("10.20.300"), Some((10, 20, 300)));
        assert!(version_triple("1.18.3") < version_triple("2.0.0"));
        assert!(version_triple("2.0.0") < version_triple("2.0.1"));
    }

    #[test]
    fn version_triple_refuses_anything_else() {
        for text in [
            "",
            "2",
            "2.0",
            "2.0.0.1",
            "v2.0.0",
            "2.0.0-beta",
            "2..0",
            " 2.0.0",
            "2.0.x",
            "+2.0.0",
            "2.-1.0",
            "4294967296.0.0",
        ] {
            assert_eq!(version_triple(text), None, "{text:?}");
        }
    }

    /// The defaults with a tuning root beside the owner's libraries: no problem.
    fn valid() -> Settings {
        Settings { tuning_root: Some(r"\\unas-pro\media\lazymkv-tuning".to_string()), ..Settings::default() }
    }

    /// A root setting.
    fn root(text: &str) -> Option<String> {
        Some(text.to_string())
    }

    /// The keys of a list of problems, in its order.
    fn keys(problems: &[SettingsProblem]) -> Vec<&str> {
        problems.iter().map(|problem| problem.key.as_str()).collect()
    }

    fn problem_of(key: &str, text: &str) -> SettingsProblem {
        SettingsProblem { key: key.to_string(), problem: text.to_string() }
    }

    #[test]
    fn the_defaults_raise_exactly_the_tuning_root_problem() {
        assert_eq!(
            Settings::default().validate(),
            vec![problem_of(
                "tuning_root",
                "tuning_root is not set: it is required while keep_tuning_evidence is on (spec 7, 14)"
            )]
        );
    }

    #[test]
    fn a_blank_tuning_root_is_not_set() {
        for text in ["", "/", r"\\"] {
            assert_eq!(
                Settings { tuning_root: root(text), ..Settings::default() }.validate(),
                vec![problem_of(
                    "tuning_root",
                    "tuning_root is not set: it is required while keep_tuning_evidence is on (spec 7, 14)"
                )],
                "{text:?}"
            );
        }
        assert_eq!(
            Settings { tuning_root: root(""), keep_tuning_evidence: false, ..Settings::default() }.validate(),
            Vec::new()
        );
    }

    #[test]
    fn a_tuning_root_or_keep_tuning_evidence_off_leaves_the_defaults_valid() {
        assert_eq!(valid().validate(), Vec::new());
        assert_eq!(Settings { keep_tuning_evidence: false, ..Settings::default() }.validate(), Vec::new());
    }

    #[test]
    fn the_owners_roots_beside_each_other_on_one_share_are_valid() {
        let settings = Settings {
            staging_root: root(r"\\unas-pro\media\lazymkv-staging"),
            movies_root: root(r"\\unas-pro\media\Movies"),
            tv_root: root(r"\\unas-pro\media\Shows"),
            ..valid()
        };
        assert_eq!(settings.validate(), Vec::new());
    }

    #[test]
    fn a_blank_or_relative_library_root_is_refused_and_an_absolute_one_of_any_form_is_not() {
        // F14: a blank root is a missing setting, and a relative one would file relative to the engine's working folder.
        let with = |movies: &str, tv: &str| Settings { movies_root: root(movies), tv_root: root(tv), ..valid() };
        assert_eq!(
            with("", "Shows").validate(),
            vec![
                problem("movies_root", "movies_root is blank: a library root must be an absolute path (spec 7, 14)"),
                problem(
                    "tv_root",
                    "tv_root Shows is not an absolute path: a library root must be a UNC path, a drive-letter path or a path from / (spec 7, 14)"
                ),
            ]
        );
        for blank in ["  ", "/", r"\\"] {
            assert_eq!(
                with(blank, "/srv/media/Shows").validate(),
                vec![problem("movies_root", "movies_root is blank: a library root must be an absolute path (spec 7, 14)")],
                "{blank:?}"
            );
        }
        for relative in ["Movies", r"D:Movies", r" \\unas-pro\media\Movies", "./Movies"] {
            assert_eq!(with(relative, "/srv/media/Shows").validate().len(), 1, "{relative:?}");
        }
        for absolute in [r"\\unas-pro\media\Movies", "//unas-pro/media/Movies", r"D:\Movies", "d:/Movies", "/srv/media/Movies"] {
            assert_eq!(with(absolute, absolute).validate(), Vec::new(), "{absolute:?}");
            assert!(is_absolute_root(absolute), "{absolute:?}");
        }
        assert_eq!(configured_root(Some("  ")), None);
        assert_eq!(configured_root(Some("/")), None);
        assert_eq!(configured_root(None), None);
        assert_eq!(configured_root(Some("/srv/media/Movies")), Some("/srv/media/Movies"));
        // An unset root is no problem here: the decision builds no path under it and holds the job (8.8).
        assert_eq!(Settings { movies_root: None, tv_root: None, ..valid() }.validate(), Vec::new());
    }

    #[test]
    fn a_staging_root_inside_the_movies_root_is_refused() {
        let settings = Settings {
            staging_root: root(r"\\unas-pro\media\Movies\lazymkv-staging"),
            movies_root: root(r"\\unas-pro\media\Movies"),
            tv_root: root(r"\\unas-pro\media\Shows"),
            ..valid()
        };
        assert_eq!(
            settings.validate(),
            vec![problem_of(
                "staging_root",
                r"staging_root \\unas-pro\media\Movies\lazymkv-staging is inside movies_root \\unas-pro\media\Movies: the staging root may not be inside a library root (spec 7)"
            )]
        );
    }

    #[test]
    fn a_staging_root_equal_to_the_tv_root_is_refused() {
        let settings = Settings {
            staging_root: root(r"\\unas-pro\media\Shows"),
            movies_root: root(r"\\unas-pro\media\Movies"),
            tv_root: root(r"\\unas-pro\media\Shows"),
            ..valid()
        };
        assert_eq!(
            settings.validate(),
            vec![problem_of(
                "staging_root",
                r"staging_root \\unas-pro\media\Shows is inside tv_root \\unas-pro\media\Shows: the staging root may not be inside a library root (spec 7)"
            )]
        );
    }

    #[test]
    fn a_staging_root_inside_both_library_roots_is_refused_once_for_each() {
        let settings = Settings {
            staging_root: root(r"\\unas-pro\media\Shows\lazymkv-staging"),
            movies_root: root(r"\\unas-pro\media"),
            tv_root: root(r"\\unas-pro\media\Shows"),
            tuning_root: root(r"D:\lazymkv-tuning"),
            ..valid()
        };
        assert_eq!(
            settings.validate(),
            vec![
                problem_of(
                    "staging_root",
                    r"staging_root \\unas-pro\media\Shows\lazymkv-staging is inside movies_root \\unas-pro\media: the staging root may not be inside a library root (spec 7)"
                ),
                problem_of(
                    "staging_root",
                    r"staging_root \\unas-pro\media\Shows\lazymkv-staging is inside tv_root \\unas-pro\media\Shows: the staging root may not be inside a library root (spec 7)"
                ),
            ]
        );
    }

    #[test]
    fn a_staging_root_typed_in_another_case_with_a_trailing_separator_is_still_refused() {
        let settings = Settings {
            staging_root: root(r"\\UNAS-PRO\Media\movies\lazymkv-staging\"),
            movies_root: root(r"\\unas-pro\media\Movies"),
            ..valid()
        };
        assert_eq!(keys(&settings.validate()), ["staging_root"]);
    }

    #[test]
    fn a_tuning_root_inside_the_staging_root_is_refused_whatever_keep_tuning_evidence_says() {
        let settings = Settings {
            staging_root: root(r"\\unas-pro\media\lazymkv-staging"),
            tuning_root: root(r"\\unas-pro\media\lazymkv-staging\tuning"),
            ..Settings::default()
        };
        let expected = vec![problem_of(
            "tuning_root",
            r"tuning_root \\unas-pro\media\lazymkv-staging\tuning is inside staging_root \\unas-pro\media\lazymkv-staging: the tuning root may not be inside the staging root or a library root (spec 7)"
        )];
        assert_eq!(settings.validate(), expected);
        assert_eq!(
            Settings { keep_tuning_evidence: false, ..settings }.validate(),
            expected,
            "a set tuning_root is checked whatever keep_tuning_evidence says"
        );
    }

    #[test]
    fn a_tuning_root_inside_a_library_root_is_refused() {
        let in_tv = Settings {
            movies_root: root(r"\\unas-pro\media\Movies"),
            tv_root: root(r"\\unas-pro\media\Shows"),
            tuning_root: root(r"\\unas-pro\media\Shows\lazymkv-tuning"),
            ..Settings::default()
        };
        assert_eq!(
            in_tv.validate(),
            vec![problem_of(
                "tuning_root",
                r"tuning_root \\unas-pro\media\Shows\lazymkv-tuning is inside tv_root \\unas-pro\media\Shows: the tuning root may not be inside the staging root or a library root (spec 7)"
            )]
        );
        let in_movies = Settings { tuning_root: root(r"\\unas-pro\media\Movies"), ..in_tv };
        assert_eq!(
            in_movies.validate(),
            vec![problem_of(
                "tuning_root",
                r"tuning_root \\unas-pro\media\Movies is inside movies_root \\unas-pro\media\Movies: the tuning root may not be inside the staging root or a library root (spec 7)"
            )]
        );
    }

    #[test]
    fn an_auto_threshold_outside_0_to_1_is_refused() {
        for (value, shown) in [(1.5, "1.5"), (-0.1, "-0.1"), (f64::NAN, "NaN")] {
            assert_eq!(
                Settings { auto_threshold: value, ..valid() }.validate(),
                vec![problem_of(
                    "auto_threshold",
                    &format!("auto_threshold {shown} is outside 0 to 1: it is a confidence (spec 8.8)")
                )]
            );
        }
        for value in [0.0, 0.9, 1.0] {
            assert_eq!(Settings { auto_threshold: value, ..valid() }.validate(), Vec::new(), "{value}");
        }
    }

    #[test]
    fn a_subs_diff_max_above_subs_same_min_is_refused() {
        assert_eq!(
            Settings { subs_diff_max: 0.75, ..valid() }.validate(),
            vec![problem_of(
                "subs_diff_max",
                "subs_diff_max 0.75 is above subs_same_min 0.7: the two alignment bands may not overlap (spec 8.7)"
            )]
        );
        assert_eq!(Settings { subs_same_min: 0.5, subs_diff_max: 0.5, ..valid() }.validate(), Vec::new());
        assert_eq!(Settings { subs_same_min: 0.8, subs_diff_max: 0.2, ..valid() }.validate(), Vec::new());
    }

    #[test]
    fn a_zero_minimum_title_length_is_refused() {
        assert_eq!(
            Settings { minlength_s: 0, ..valid() }.validate(),
            vec![problem_of(
                "minlength_s",
                "minlength_s is 0: MakeMKV's minimum title length must be at least 1 second (spec 14)"
            )]
        );
        assert_eq!(Settings { minlength_s: 300, ..valid() }.validate(), Vec::new());
    }

    #[test]
    fn a_zero_feature_length_is_refused() {
        assert_eq!(
            Settings { feature_min_minutes: 0, ..valid() }.validate(),
            vec![problem_of(
                "feature_min_minutes",
                "feature_min_minutes is 0: the shortest feature must be at least 1 minute (spec 8.3)"
            )]
        );
        assert_eq!(Settings { feature_min_minutes: 60, ..valid() }.validate(), Vec::new());
    }

    #[test]
    fn a_zero_file_phase_concurrency_is_refused() {
        assert_eq!(
            Settings { file_phase_concurrency: 0, ..valid() }.validate(),
            vec![problem_of(
                "file_phase_concurrency",
                "file_phase_concurrency is 0: at least one file-phase job must be able to run (spec 6, 14)"
            )]
        );
        assert_eq!(Settings { file_phase_concurrency: 2, ..valid() }.validate(), Vec::new());
    }

    #[test]
    fn a_makemkv_min_version_that_is_not_n_n_n_is_refused() {
        assert_eq!(
            Settings { makemkv_min_version: "2.0".to_string(), ..valid() }.validate(),
            vec![problem_of("makemkv_min_version", "makemkv_min_version 2.0 is not a version of the form N.N.N (spec 9.1)")]
        );
        assert_eq!(
            Settings { makemkv_min_version: "v2.0.0".to_string(), ..valid() }.validate(),
            vec![problem_of("makemkv_min_version", "makemkv_min_version v2.0.0 is not a version of the form N.N.N (spec 9.1)")]
        );
        assert_eq!(Settings { makemkv_min_version: "2.1.0".to_string(), ..valid() }.validate(), Vec::new());
    }

    #[test]
    fn every_problem_is_listed_in_spec_14s_key_order() {
        let settings = Settings {
            staging_root: root(r"\\unas-pro\media\Movies\staging"),
            movies_root: root(r"\\unas-pro\media\Movies"),
            minlength_s: 0,
            auto_threshold: 2.0,
            feature_min_minutes: 0,
            subs_diff_max: 0.9,
            file_phase_concurrency: 0,
            makemkv_min_version: "x".to_string(),
            ..Settings::default()
        };
        let problems = settings.validate();
        assert_eq!(
            keys(&problems),
            [
                "staging_root",
                "minlength_s",
                "auto_threshold",
                "feature_min_minutes",
                "subs_diff_max",
                "tuning_root",
                "file_phase_concurrency",
                "makemkv_min_version",
            ]
        );
        assert_eq!(problems.len(), 8);
    }

    #[test]
    fn a_settings_file_naming_one_key_keeps_every_other_default() {
        assert_eq!(
            Settings::from_json(r#"{"keep_tuning_evidence": false}"#),
            Ok((Settings { keep_tuning_evidence: false, ..Settings::default() }, vec![]))
        );
    }

    #[test]
    fn a_settings_file_changing_several_keys_reads_each_of_them() {
        let text = r#"{
            "staging_root": "\\\\unas-pro\\media\\lazymkv-staging",
            "movies_root": "\\\\unas-pro\\media\\Movies",
            "tv_root": "\\\\unas-pro\\media\\Shows",
            "tuning_root": "\\\\unas-pro\\media\\lazymkv-tuning",
            "auto_threshold": 0.9,
            "edition_style": "plex",
            "jellyfin10_compat": true,
            "tool_paths": {"mkvmerge": "C:\\Tools\\mkvmerge.exe"}
        }"#;
        assert_eq!(
            Settings::from_json(text),
            Ok((
                Settings {
                    staging_root: root(r"\\unas-pro\media\lazymkv-staging"),
                    movies_root: root(r"\\unas-pro\media\Movies"),
                    tv_root: root(r"\\unas-pro\media\Shows"),
                    tuning_root: root(r"\\unas-pro\media\lazymkv-tuning"),
                    auto_threshold: 0.9,
                    edition_style: EditionStyle::Plex,
                    jellyfin10_compat: true,
                    tool_paths: ToolPaths { mkvmerge: Some(r"C:\Tools\mkvmerge.exe".to_string()), ..ToolPaths::default() },
                    ..Settings::default()
                },
                vec![]
            ))
        );
    }

    #[test]
    fn an_empty_settings_file_is_the_defaults_with_their_one_problem() {
        let (settings, problems) = Settings::from_json("{}").unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(problems, Settings::default().validate());
        assert_eq!(keys(&problems), ["tuning_root"]);
    }

    #[test]
    fn an_unknown_key_is_refused_naming_that_key() {
        let refused = Settings::from_json(r#"{"keep_tuning_evidence": false, "minlength": 300}"#).unwrap_err();
        assert!(
            refused.starts_with("minlength: unknown field `minlength`, expected one of `staging_root`"),
            "{refused}"
        );
    }

    #[test]
    fn an_unknown_tool_is_refused_naming_tool_paths() {
        let refused =
            Settings::from_json(r#"{"keep_tuning_evidence": false, "tool_paths": {"vlc": "C:\\VLC\\vlc.exe"}}"#)
                .unwrap_err();
        assert!(refused.starts_with("tool_paths: unknown field `vlc`, expected one of `makemkvcon`"), "{refused}");
    }

    #[test]
    fn a_value_of_the_wrong_type_is_refused_naming_its_key() {
        assert_eq!(
            Settings::from_json(r#"{"keep_tuning_evidence": "off"}"#),
            Err("keep_tuning_evidence: invalid type: string \"off\", expected a boolean".to_string())
        );
        let negative = Settings::from_json(r#"{"keep_tuning_evidence": false, "minlength_s": -5}"#).unwrap_err();
        assert!(negative.starts_with("minlength_s: invalid value: integer `-5`"), "{negative}");
        let style = Settings::from_json(r#"{"keep_tuning_evidence": false, "edition_style": "Plex"}"#).unwrap_err();
        assert!(
            style.starts_with("edition_style: unknown variant `Plex`, expected one of `combined`, `plex`, `jellyfin`"),
            "{style}"
        );
    }

    #[test]
    fn the_first_bad_key_in_file_order_is_the_one_named() {
        let refused =
            Settings::from_json(r#"{"minlength_s": "120", "keep_tuning_evidence": false, "cache": 512}"#).unwrap_err();
        assert!(refused.starts_with("minlength_s: invalid type: string \"120\""), "{refused}");
        assert!(!refused.contains("cache"), "{refused}");
    }

    #[test]
    fn a_file_that_is_not_json_is_refused_with_serde_jsons_message() {
        let refused = Settings::from_json("keep_tuning_evidence = false").unwrap_err();
        assert!(refused.starts_with("expected value at line 1 column 1"), "{refused}");
    }

    #[test]
    fn a_file_that_is_not_an_object_is_refused() {
        for text in ["[]", "null", "120"] {
            assert_eq!(
                Settings::from_json(text),
                Err("a settings file is one JSON object of spec 14's keys".to_string()),
                "{text}"
            );
        }
    }

    #[test]
    fn a_well_formed_file_comes_back_with_the_problems_of_validate() {
        assert_eq!(
            Settings::from_json(r#"{"keep_tuning_evidence": false, "auto_threshold": 1.5}"#),
            Ok((
                Settings { keep_tuning_evidence: false, auto_threshold: 1.5, ..Settings::default() },
                vec![problem_of("auto_threshold", "auto_threshold 1.5 is outside 0 to 1: it is a confidence (spec 8.8)")]
            ))
        );
    }

    #[test]
    fn a_settings_file_with_only_validate_problems_still_reads() {
        let (settings, problems) = Settings::from_json(r#"{"auto_threshold": 1.5}"#).unwrap();
        assert_eq!(settings, Settings { auto_threshold: 1.5, ..Settings::default() });
        assert_eq!(keys(&problems), ["auto_threshold", "tuning_root"]);
    }

    #[test]
    fn valid_settings_written_out_read_back_unchanged() {
        let text = serde_json::to_string(&valid()).unwrap();
        assert_eq!(Settings::from_json(&text), Ok((valid(), vec![])));
    }

    #[test]
    fn subs_tolerance_s_is_the_milliseconds_in_seconds() {
        assert_eq!(Settings::default().subs_tolerance_s(), 0.3);
        assert_eq!(Settings { subs_tolerance_ms: 250, ..Settings::default() }.subs_tolerance_s(), 0.25);
        assert_eq!(Settings { subs_tolerance_ms: 0, ..Settings::default() }.subs_tolerance_s(), 0.0);
    }

    #[test]
    fn the_subtitle_defaults_equal_the_text_rules_defaults() {
        assert_eq!(Settings::default().text_rule(), crate::subtitle::text::Rule::default());
    }

    #[test]
    fn the_text_rule_takes_the_four_subtitle_settings_and_keeps_every_other_threshold() {
        use crate::subtitle::text::Rule;
        let settings = Settings {
            sub_clear_max_bytes: 100,
            subs_tolerance_ms: 250,
            subs_same_min: 0.8,
            subs_diff_max: 0.2,
            ..Settings::default()
        };
        let rule = settings.text_rule();
        assert_eq!(rule.CLEAR_MAX_BYTES, 100);
        assert_eq!(rule.TOL_S, 0.25);
        assert_eq!(rule.SAME_MIN, 0.8);
        assert_eq!(rule.DIFF_MAX, 0.2);
        assert_eq!(rule.FULL_SHARE, 0.10);
        assert_eq!(rule.FORCED_MAX_EVENTS, 150);
        assert_eq!(Rule { CLEAR_MAX_BYTES: 64, TOL_S: 0.300, SAME_MIN: 0.70, DIFF_MAX: 0.30, ..rule }, Rule::default());
    }
}

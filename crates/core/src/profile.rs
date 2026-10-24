//! Builds a per-job MakeMKV conversion profile from MakeMKV's default profile.

/// Selects every stream except 3D video, with equal weights so the output
/// track order follows the scan order (spec 9.3).
pub const ALL_TRACKS_SELECTION: &str = "+sel:all,-sel:mvcvideo,=100:all";

/// The 1009 fallback profile's selection: every stream but 3D video (design part 5.4, K6, L4). Def: T4. Use: T23.
pub const FALLBACK_SELECTION: &str = "+sel:all,-sel:mvcvideo";

/// The job profile's file name in the job folder (design part 7.1). Def: T4. Use: T13, T23.
pub const JOB_PROFILE_FILE: &str = "job.mmcp.xml";

/// The fallback profile's file name in the job folder (design part 7.1, K6). Def: T4. Use: T13, T23.
pub const FALLBACK_PROFILE_FILE: &str = "fallback.mmcp.xml";

#[derive(Debug, PartialEq, Eq)]
pub enum ProfileError {
    NoNameElement,
    NoDefaultSelection,
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Returns `default_profile_xml` with its profile name set to `name` and every
/// `defaultSelection` attribute set to the literal `selection`.
pub fn make_job_profile(
    default_profile_xml: &str,
    name: &str,
    selection: &str,
) -> Result<String, ProfileError> {
    const NAME_END: &str = "</name>";
    const KEY: &str = "defaultSelection=\"";

    let name_start = default_profile_xml
        .find("<name")
        .ok_or(ProfileError::NoNameElement)?;
    let name_end = default_profile_xml[name_start..]
        .find(NAME_END)
        .ok_or(ProfileError::NoNameElement)?
        + name_start
        + NAME_END.len();

    let mut out = String::with_capacity(default_profile_xml.len() + 256);
    out.push_str(&default_profile_xml[..name_start]);
    out.push_str(&format!("<name lang=\"eng\">{}</name>", xml_escape(name)));

    let mut rest = &default_profile_xml[name_end..];
    let mut replaced = 0;
    while let Some(position) = rest.find(KEY) {
        let value_start = position + KEY.len();
        let value_len = rest[value_start..]
            .find('"')
            .ok_or(ProfileError::NoDefaultSelection)?;
        out.push_str(&rest[..value_start]);
        out.push_str(&xml_escape(selection));
        rest = &rest[value_start + value_len..];
        replaced += 1;
    }
    out.push_str(rest);

    if replaced == 0 {
        return Err(ProfileError::NoDefaultSelection);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::default_profile_xml;

    #[test]
    fn replaces_the_name_and_every_selection() {
        let out = make_job_profile(&default_profile_xml(), "lazymkv all tracks", ALL_TRACKS_SELECTION).unwrap();
        assert!(out.contains(r#"<name lang="eng">lazymkv all tracks</name>"#));
        assert!(!out.contains(":5086"));
        assert!(!out.contains("$app_DefaultSelectionString"));
        assert_eq!(
            out.matches(r#"defaultSelection="+sel:all,-sel:mvcvideo,=100:all""#).count(),
            2
        );
        assert!(out.contains(r#"<mkvSettings insertFirstChapter00IfMissing="true" />"#));
        assert!(out.contains(r#"<outputSettings name="copy" outputFormat="directCopy">"#));
    }

    #[test]
    fn escapes_xml_special_characters() {
        let out = make_job_profile(&default_profile_xml(), "a & b", "+sel:(eng|\"x\")&<y>").unwrap();
        assert!(out.contains("<name lang=\"eng\">a &amp; b</name>"));
        assert!(out.contains("defaultSelection=\"+sel:(eng|&quot;x&quot;)&amp;&lt;y&gt;\""));
    }

    #[test]
    fn reports_a_profile_it_cannot_edit() {
        assert_eq!(
            make_job_profile("<profile></profile>", "n", "s"),
            Err(ProfileError::NoNameElement)
        );
        assert_eq!(
            make_job_profile("<profile><name>x</name></profile>", "n", "s"),
            Err(ProfileError::NoDefaultSelection)
        );
    }

    #[test]
    fn the_fallback_profile_selects_every_stream_but_mvc_video() {
        let out = make_job_profile(&default_profile_xml(), "j", FALLBACK_SELECTION).unwrap();
        assert_eq!(out.matches("defaultSelection=\"+sel:all,-sel:mvcvideo\"").count(), 2);
        assert_eq!(FALLBACK_SELECTION, "+sel:all,-sel:mvcvideo");
        assert_eq!((JOB_PROFILE_FILE, FALLBACK_PROFILE_FILE), ("job.mmcp.xml", "fallback.mmcp.xml"));
    }
}

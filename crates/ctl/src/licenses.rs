//! `lazymkvctl licenses`: the licence texts lazymkvctl carries (design parts 9.6, 11.4; E1, E2, call 15). A
//! `cargo install` has no `licenses\` folder, so the program carries the texts itself (ARCH-17).

use lazymkv_ocr_models::{ATTRIBUTION, LICENCE_TEXT};
use std::borrow::Cow;
use std::fs;
use std::path::Path;

/// The repository's `LICENSE`, the MIT License lazymkv is under (design part 11.4; E1). Def: T5-23. Use: T5-23, T5-26.
pub const MIT_TEXT: &str = include_str!("../../../LICENSE");

/// TMDB's required sentence, printed verbatim (design part 11.4; E2); the logo waits for the UI (user interface).
/// Def: T5-23. Use: T5-23, T5-26.
pub const TMDB_NOTICE: &str = "This product uses the TMDB API but is not endorsed or certified by TMDB.";

/// TMDB's address, printed under its sentence (design part 11.4; T5-R33). Def: T5-23. Use: T5-23, T5-26.
pub const TMDB_ADDRESS: &str = "https://www.themoviedb.org";

/// The committed `cargo-about` file of the Rust crates compiled into lazymkv, refreshed only by committing CI's
/// `crate-notices` artifact (design part 11.4; T5-R43, call 26). Def: T5-23. Use: T5-23, T5-26.
pub const CRATE_NOTICES: &str = include_str!("../crate-notices.txt");

/// One part of the output: `heading`, a blank line, `body`, ended by a newline when it has none.
fn part(heading: &str, body: &str) -> String {
    let mut text = format!("{heading}\n\n{body}");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// `text` with a final newline, added when it has none: the form T6-R9 compares two texts in.
fn ended(text: &str) -> Cow<'_, str> {
    if text.ends_with('\n') {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(format!("{text}\n"))
    }
}

/// The bundled tools' licence parts: only when `exe`'s folder is named `bin` (ignoring ASCII case) and a `licenses`
/// folder sits beside that folder; one part per regular file in it, sorted by file name. A file is left out when its
/// text, with a final newline added when it has none, equals one of `printed` under the same rule, so a release tree
/// prints each text once (T6-R9, notes call 8); the text is the file's bytes read as UTF-8 with any invalid sequence
/// replaced, as the part prints it.
fn bundled(exe: Option<&Path>, printed: &[&str]) -> Result<Vec<String>, String> {
    let Some(bin) = exe.and_then(Path::parent) else { return Ok(Vec::new()) };
    let named_bin = bin.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("bin"));
    let Some(dir) = bin.parent().map(|root| root.join("licenses")).filter(|dir| named_bin && dir.is_dir()) else {
        return Ok(Vec::new());
    };
    let failed = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|error| failed(&dir, error))? {
        let entry = entry.map_err(|error| failed(&dir, error))?;
        if entry.file_type().map_err(|error| failed(&entry.path(), error))?.is_file() {
            files.push(entry.file_name());
        }
    }
    files.sort();
    let mut parts = Vec::new();
    for name in files {
        let path = dir.join(&name);
        let bytes = fs::read(&path).map_err(|error| failed(&path, error))?;
        let text = String::from_utf8_lossy(&bytes);
        if printed.iter().any(|body| ended(body) == ended(&text)) {
            continue;
        }
        let heading = format!("Bundled tool licence: {}", name.to_string_lossy());
        parts.push(part(&heading, &text));
    }
    Ok(parts)
}

/// The text `lazymkvctl licenses` prints: lazymkv's MIT License, the ocrs models' CC-BY-SA-4.0 attribution and text,
/// TMDB's sentence and address, the crate notices, then the bundled tools' licences; the parts are joined by one blank
/// line. `exe` is lazymkvctl's own path; the bundled tools' licences are added only when its folder is named `bin` and
/// a `licenses` folder sits beside that folder, and a file there whose text equals the MIT part's, the models' part's
/// or the crate notices' body is not printed again (T6-R9, notes call 8). Err is `<path>: <io error>` for a file or
/// the folder that cannot be read. Design parts 9.6, 11.4; E1, E2, call 15. Def: T5-23. Use: T5-23, T5-26, T6-4.
pub fn licenses_text(exe: Option<&Path>) -> Result<String, String> {
    let models = format!("{ATTRIBUTION}\n{LICENCE_TEXT}");
    let mut parts = vec![
        part("lazymkv: the MIT License", MIT_TEXT),
        part("The ocrs models: CC-BY-SA-4.0", &models),
        part("The Movie Database (TMDB)", &format!("{TMDB_NOTICE}\n{TMDB_ADDRESS}\n")),
    ];
    let mut notices = CRATE_NOTICES.to_string();
    if !notices.ends_with('\n') {
        notices.push('\n');
    }
    parts.push(notices);
    parts.extend(bundled(exe, &[MIT_TEXT, models.as_str(), CRATE_NOTICES])?);
    Ok(parts.join("\n"))
}

//! Readers of the evidence clone that more than one dev integration test needs (`plan.rs`, `decide.rs`): values a test
//! compares against are read from the clone when the test runs, never written in a test (ruling RP21). Not a test
//! crate of its own: each test file that uses it declares `mod clone;`.

use std::collections::BTreeMap;
use std::path::Path;

/// A JSON file of the recording folder `dir`, a leading byte-order mark dropped.
pub fn clone_json(dir: &Path, file: &str) -> serde_json::Value {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The episode names of the recording folder `dir`'s TheDiscDB answer (thediscdb-hash.json): each Episode item's
/// title, by the item's source file.
pub fn episode_names(dir: &Path) -> BTreeMap<String, String> {
    let answer = clone_json(dir, "thediscdb-hash.json");
    answer["titles"]
        .as_array()
        .expect("the answer lists titles")
        .iter()
        .filter(|title| title["item"]["type"] == "Episode")
        .map(|title| {
            let source = title["source_file"].as_str().expect("an Episode item has a source file");
            let name = title["item"]["title"].as_str().expect("an Episode item has a title");
            (source.to_string(), name.to_string())
        })
        .collect()
}

//! J8's merged search page (design part 8.2 item 2): the cleaned name's `/search/multi` first page, then the tagged
//! name's results it does not already hold.

use lazymkv_core::lookup::tmdb::TmdbSearch;

/// J8's merge of two `/search/multi` first pages (design part 8.2 item 2, designer's call; decision-core J8): every
/// result of `cleaned` in its order, then each result of `tagged`, in its order, whose `(media_type, id)` (its
/// `identity()` for every result `parse_search` keeps) is not among the results kept so far. Pure. Def: T4-2.
/// Use: T4-2 (chain), T4-8 (`refetch_search`).
pub fn merge_pages(cleaned: &TmdbSearch, tagged: &TmdbSearch) -> TmdbSearch {
    let mut kept: Vec<(&str, i64)> =
        cleaned.results.iter().map(|result| (result.media_type.as_str(), result.id)).collect();
    let mut merged = cleaned.clone();
    for result in &tagged.results {
        let key = (result.media_type.as_str(), result.id);
        if !kept.contains(&key) {
            kept.push(key);
            merged.results.push(result.clone());
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saved::slim_search;
    use lazymkv_core::lookup::tmdb::{parse_search, TmdbSearchResult};
    use serde_json::{json, Value};

    const TODAY: &str = "2026-10-23";

    fn result(media_type: &str, id: i64) -> TmdbSearchResult {
        TmdbSearchResult { media_type: media_type.to_string(), id, title: Some(format!("T{id}")), name: None,
            original_title: None, original_name: None, release_date: None, first_air_date: None }
    }

    fn page(results: &[(&str, i64)]) -> TmdbSearch {
        TmdbSearch { results: results.iter().map(|(kind, id)| result(kind, *id)).collect() }
    }

    fn ids(search: &TmdbSearch) -> Vec<(String, i64)> {
        search.results.iter().map(|r| (r.media_type.clone(), r.id)).collect()
    }

    #[test]
    fn merge_keeps_the_cleaned_results_first_then_the_new_tagged_ones() {
        let merged = merge_pages(&page(&[("movie", 1), ("movie", 2)]), &page(&[("movie", 2), ("movie", 3)]));
        assert_eq!(ids(&merged), ids(&page(&[("movie", 1), ("movie", 2), ("movie", 3)])));
    }

    #[test]
    fn merge_compares_by_kind_and_id() {
        let merged = merge_pages(&page(&[("movie", 1)]), &page(&[("tv", 1), ("movie", 1)]));
        assert_eq!(ids(&merged), ids(&page(&[("movie", 1), ("tv", 1)])));
    }

    #[test]
    fn merge_of_an_empty_tagged_page_is_the_cleaned_page() {
        let cleaned = page(&[("movie", 1), ("tv", 2)]);
        assert_eq!(merge_pages(&cleaned, &TmdbSearch::default()), cleaned);
    }

    #[test]
    fn the_slim_merged_page_parses_to_the_merged_pages() {
        let movie = |id: i64, title: &str| json!({"media_type": "movie", "id": id, "title": title, "original_title": title, "release_date": "2010-08-20"});
        let c = json!({"page": 1, "results": [movie(900101, "Piranha"), movie(900102, "Piranha")]});
        let t = json!({"page": 1, "results": [movie(900102, "Piranha"), {"media_type": "person", "id": 7, "name": "Someone"},
            {"media_type": "tv", "id": 900103, "name": "Piranha 3D", "original_name": "Piranha 3D", "first_air_date": "2010-01-01"}]});
        let parse = |value: &Value| parse_search(&value.to_string(), "page").unwrap();
        let slim = slim_search(&[(&c, "PIRANHA"), (&t, "PIRANHA 3D")], TODAY);
        let text = serde_json::to_string(&slim).unwrap();
        assert_eq!(parse_search(&text, "t").unwrap(), merge_pages(&parse(&c), &parse(&t)));
        assert_eq!(slim["queries"], json!(["PIRANHA", "PIRANHA 3D"]));
        assert_eq!(slim.as_object().unwrap().keys().last().map(String::as_str), Some("fetched_at"));
    }
}

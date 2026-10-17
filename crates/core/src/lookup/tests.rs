//! Tests of `lookup/mod.rs`: `Kind` (Task 3). Task 4 appends the identification's tests (spec 8.1).

use super::tmdb::TmdbKind;
use super::Kind;

#[test]
fn kind_spells_itself_as_movie_and_tv() {
    assert_eq!(Kind::ALL, [Kind::Movie, Kind::Tv]);
    assert_eq!(Kind::ALL.map(Kind::as_str), ["movie", "tv"]);
    assert_eq!(serde_json::to_string(&Kind::Tv).unwrap(), "\"tv\"");
    assert_eq!(serde_json::from_str::<Kind>("\"movie\"").unwrap(), Kind::Movie);
    assert!(serde_json::from_str::<Kind>("\"Movie\"").is_err());
    assert_eq!("tv".parse::<Kind>(), Ok(Kind::Tv));
    assert_eq!("series".parse::<Kind>(), Err("\"series\" is not a disc kind (movie or tv)".to_string()));
}

#[test]
fn kind_and_tmdb_kind_convert_both_ways() {
    assert_eq!(Kind::from(TmdbKind::Movie), Kind::Movie);
    assert_eq!(Kind::from(TmdbKind::Tv), Kind::Tv);
    assert_eq!(TmdbKind::from(Kind::Movie), TmdbKind::Movie);
    assert_eq!(TmdbKind::from(Kind::Tv), TmdbKind::Tv);
    for kind in Kind::ALL {
        assert_eq!(Kind::from(TmdbKind::from(kind)), kind);
        assert_eq!(TmdbKind::from(kind).as_str(), kind.as_str());
    }
}

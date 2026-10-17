//! `lazymkv-dev plan` (ruling RP14 of the phase 1 plan; port spec 3.4) over the nine pinned TheDiscDB scan logs of
//! corpus/thediscdb, which always run (CI included), and over the four recordings of the evidence clone, which print
//! "skipped: <reason>" and pass without it. The expectations are the "Expected plans" table of the phase 1 plan's
//! interfaces: identity, kind, titles in plan order with their keys, title confidence, mode and review reasons (design
//! spec 6.1 step 5, 8.1 to 8.5, 8.8), and the inputs each source gives (Open point 26 for a pinned log; spec 15
//! "Recordings" for a recording). Inputs: the repository's corpus/thediscdb and corpus/thediscdb-api, and the evidence
//! clone (LAZYMKV_EVIDENCE, else corpus/owner under the repository root). A value only the private clone holds (a
//! fingerprint, a disc ID, a title's duration and size, an episode name, a volume label) is read from the clone when
//! the test runs, never written here (ruling RP21 of the phase 1 plan).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lazymkv_core::audio::labels::is_slug;
use lazymkv_core::disc::Disc;
use lazymkv_core::lang::wanted_languages;
use lazymkv_core::lookup::name::NameSource;
use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
use lazymkv_core::lookup::{IdentityStep, Kind};
use lazymkv_core::plan::movie::TitleRule;
use lazymkv_core::plan::tv::{AssignedEpisode, Episode, SeasonSession};
use lazymkv_core::plan::{KindSource, Mode, Plan, PlannedRole, PlannedTitle};
use lazymkv_core::recording::text_mode;
use lazymkv_core::review::{Note, ReviewReason};
use lazymkv_core::robot::parse;
use lazymkv_core::settings::Settings;
use lazymkv_dev::cmd::plan::{plan_doc, LookupFile, PlanDoc, PlanInputsSummary, PlanOptions, PlanSource};
use lazymkv_dev::evidence::{resolve_evidence, resolve_recording, skip, Ctx};

/// The nine pinned logs, below corpus/thediscdb.
const JOHN_WICK_UHD: &str = "movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt";
const JOHN_WICK_BLU_RAY: &str = "movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc02.txt";
const EXPENDABLES: &str = "movie/The Expendables 3 (2014)/the-expendables-3-film-collection/disc01.txt";
const SMILE: &str = "movie/Smile (2022)/2022-4k/disc01.txt";
const KNOWING: &str = "movie/Knowing (2009)/2010-dvd/disc01.txt";
const CHERNOBYL_1: &str = "series/Chernobyl (2019)/2020-4k-US/disc01.txt";
const CHERNOBYL_2: &str = "series/Chernobyl (2019)/2020-4k-US/disc02.txt";
const FIREFLY_1: &str = "series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt";
const FIREFLY_2: &str = "series/Firefly (2002)/2003-dvd-the-complete-series/disc02.txt";

/// The recordings of the evidence clone, one test each. A recording added later is listed by
/// recordings_without_a_test_are_listed_as_not_covered until it gets its test here.
const RECORDINGS: [&str; 4] =
    ["pans-labyrinth-2006-uhd", "short-circuit-1986-dvd", "short-circuit-2-1988-dvd", "the-office-2005-s3d3-bluray"];

/// The context of the pinned-log tests: crates/dev, whose repository root holds corpus/thediscdb-api.
fn pinned_ctx() -> Ctx {
    Ctx::for_test(Path::new(env!("CARGO_MANIFEST_DIR")))
}

/// The context of the recording tests: the process's cwd (crates/dev under cargo test), no environment but
/// LAZYMKV_EVIDENCE.
fn context() -> Ctx {
    let mut ctx = Ctx::for_test(&std::env::current_dir().expect("a current directory"));
    if let Ok(value) = std::env::var("LAZYMKV_EVIDENCE") {
        ctx.env.insert("LAZYMKV_EVIDENCE".to_string(), value);
    }
    ctx
}

fn not_made() -> LookupFile {
    LookupFile { path: None, answer: "n/a".to_string() }
}

fn found(path: &str, answer: &str) -> LookupFile {
    LookupFile { path: Some(path.to_string()), answer: answer.to_string() }
}

/// The plan of one pinned log (below corpus/thediscdb, named from crates/dev), after checking the inputs Open point 26
/// gives it: the fingerprint of its HSH lines, which names its corpus/thediscdb-api answer, and nothing else.
fn pinned(log: &str, hash: &str) -> Plan {
    let path = format!("../../corpus/thediscdb/{log}");
    let doc = plan_doc(&pinned_ctx(), &PlanSource::ScanFile(Path::new(&path)), &PlanOptions::default())
        .unwrap_or_else(|refused| panic!("{}", refused.line));
    assert_eq!(doc.slug, path);
    assert_eq!(
        doc.inputs,
        PlanInputsSummary {
            scan: path.clone(),
            fingerprint: Some(hash.to_string()),
            disc_id: None,
            bdmt: None,
            meta: false,
            discdb_hash: found(&format!("corpus/thediscdb-api/dischash-{hash}.json"), "hit"),
            discdb_discid: not_made(),
            tmdb: not_made(),
            tmdb_search: None,
            tmdb_record: None,
            lookup_status: BTreeMap::new(),
            remembered: None,
        }
    );
    assert_eq!(doc.settings, Settings::default());
    assert_eq!(doc.settings_problems, Settings::default().validate());
    doc.plan
}

fn movie(id: i64) -> TmdbIdentity {
    TmdbIdentity { kind: TmdbKind::Movie, id }
}

fn show(id: i64) -> TmdbIdentity {
    TmdbIdentity { kind: TmdbKind::Tv, id }
}

/// 8.1 step 3, which every pinned log and two recordings reach: TheDiscDB by fingerprint at 0.95.
fn assert_discdb_one(plan: &Plan, tmdb: TmdbIdentity, title: &str, year: i32) {
    assert_eq!(
        (plan.identity.step, plan.identity.confidence, plan.identity_confidence),
        (IdentityStep::DiscdbOne, 0.95, 0.95)
    );
    assert_eq!(plan.identity.tmdb, Some(tmdb));
    assert_eq!((plan.identity.title.as_deref(), plan.identity.year), (Some(title), Some(year)));
    assert_eq!((plan.identity.review, plan.identity.lookup_pending), (None, false));
}

/// 0.98 (8.3 rule 1 or 8.5's map), Direct (0.98 >= 0.85), no review reason and no note.
fn assert_filed_automatically(plan: &Plan) {
    assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
    assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
    assert_eq!(plan.notes.iter().map(|noted| noted.note).collect::<Vec<Note>>(), Vec::<Note>::new());
}

/// A planned title as the planner writes it from the scan, neither suggested nor remembered.
fn planned(key: &str, title: u32, role: PlannedRole, seconds: u32, size: u64, source_file: Option<&str>) -> PlannedTitle {
    PlannedTitle {
        key: key.to_string(),
        title,
        role,
        seconds: Some(seconds),
        size: Some(size),
        source_file: source_file.map(str::to_string),
        suggested: false,
        remembered: false,
    }
}

fn feature(edition: Option<&str>) -> PlannedRole {
    PlannedRole::Feature { edition: edition.map(str::to_string) }
}

fn episode(season: u32, episodes: &[u32], name: &str) -> PlannedRole {
    PlannedRole::Episode(Episode { season: Some(season), episodes: episodes.to_vec(), name: Some(name.to_string()) })
}

/// The season session a mapped disc joins (8.5): one disc, its (episode, title) pairs as AssignedEpisode entries
/// in the order given (the callers pass them sorted by episode, as the session keeps them), no episode count (no
/// TMDb season record), open.
fn session(show_id: i64, season: u32, fingerprint: &str, pairs: &[(u32, u32)]) -> SeasonSession {
    SeasonSession {
        show_id,
        season,
        discs: vec![fingerprint.to_string()],
        assigned: pairs.iter().map(|&(episode, title)| AssignedEpisode { episode, fingerprint: fingerprint.to_string(), title }).collect(),
        episode_count: None,
        closed: false,
    }
}

#[test]
fn john_wick_uhd_disc01_plans_title_0_from_thediscdb() {
    let plan = pinned(JOHN_WICK_UHD, "76E63EBB5B66F73B0C37BFAB59B8ABA5");
    assert_discdb_one(&plan, movie(245891), "John Wick", 2014);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
    // Title 8 has the same segment map (the playlist's clip); TheDiscDB's MainMovie is title 0.
    assert_eq!(plan.titles, vec![planned("t00-00001", 0, feature(None), 6073, 67_757_930_496, Some("00001.mpls"))]);
    assert_eq!(plan.evidence.title_rule, Some(TitleRule::Database));
    assert_eq!(plan.wanted, wanted_languages(None));
    assert_eq!(plan.session, None);
    assert_filed_automatically(&plan);
}

#[test]
fn john_wick_blu_ray_disc02_plans_title_0_from_thediscdb() {
    let plan = pinned(JOHN_WICK_BLU_RAY, "BB9E6C963A4E2EAAB46C748CD4C00B69");
    assert_discdb_one(&plan, movie(245891), "John Wick", 2014);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
    assert_eq!(plan.titles, vec![planned("t00-00001", 0, feature(None), 6073, 25_234_495_488, Some("00001.mpls"))]);
    assert_eq!(plan.evidence.title_rule, Some(TitleRule::Database));
    assert_filed_automatically(&plan);
}

#[test]
fn expendables_3_plans_title_41_as_the_unrated_cut() {
    let plan = pinned(EXPENDABLES, "545411B47060B18FD7F8D94599F24708");
    assert_discdb_one(&plan, movie(138103), "The Expendables 3", 2014);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
    // Open point 5: TheDiscDB's item title names no edition, so the label table names it (FPL_MainFeature_UR).
    assert_eq!(
        plan.titles,
        vec![planned("t00-00756", 41, feature(Some("Unrated Cut")), 7891, 28_831_358_976, Some("00756.mpls"))]
    );
    assert_eq!(plan.evidence.title_rule, Some(TitleRule::Database));
    assert_filed_automatically(&plan);
}

#[test]
fn smile_plans_title_14() {
    let plan = pinned(SMILE, "E826F0C64B04079EA83094AC046E8709");
    assert_discdb_one(&plan, movie(882598), "Smile", 2022);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
    assert_eq!(plan.titles, vec![planned("t00-00040", 14, feature(None), 6937, 79_673_339_904, Some("00040.mpls"))]);
    assert_filed_automatically(&plan);
}

#[test]
fn knowing_dvd_plans_title_0_keyed_by_attribute_24() {
    let plan = pinned(KNOWING, "631BD77872C157D7C436E6F48F06A478");
    assert_discdb_one(&plan, movie(13811), "Knowing", 2009);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
    assert_eq!(plan.titles, vec![planned("t00-title01", 0, feature(None), 7274, 7_098_400_768, None)]);
    assert_filed_automatically(&plan);
}

#[test]
fn chernobyl_disc_1_plans_three_episodes_in_episode_order() {
    let hash = "D11592C9B2D252937C22EDF01DBD1E05";
    let plan = pinned(CHERNOBYL_1, hash);
    assert_discdb_one(&plan, show(87108), "Chernobyl", 2019);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
    // TheDiscDB lists E01, E03, E02 in index order; the plan files them in episode order.
    assert_eq!(
        plan.titles,
        vec![
            planned("t00-00801", 1, episode(1, &[1], "1.23.45"), 3534, 28_652_482_560, Some("00801.mpls")),
            planned("t01-00802", 8, episode(1, &[2], "Please Remain Calm"), 3896, 31_796_779_008, Some("00802.mpls")),
            planned("t02-00803", 7, episode(1, &[3], "Open Wide O Earth"), 3707, 30_230_642_688, Some("00803.mpls")),
        ]
    );
    assert_eq!(plan.session, Some(session(87108, 1, hash, &[(1, 1), (2, 8), (3, 7)])));
    let name = plan.evidence.name.as_ref().expect("the volume label names the disc");
    assert_eq!((name.raw.as_str(), name.disc, name.season), ("CHERNOBYL_D1", Some(1), None));
    assert_filed_automatically(&plan);
}

#[test]
fn chernobyl_disc_2_plans_episodes_4_and_5() {
    let hash = "59796E40C7ABE16ACD9B259E732D392B";
    let plan = pinned(CHERNOBYL_2, hash);
    assert_discdb_one(&plan, show(87108), "Chernobyl", 2019);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
    assert_eq!(
        plan.titles,
        vec![
            planned(
                "t00-00801",
                6,
                episode(1, &[4], "The Happiness of All Mankind"),
                3899,
                28_075_038_720,
                Some("00801.mpls")
            ),
            planned("t01-00802", 8, episode(1, &[5], "Vichnaya Pamyat"), 4293, 31_053_785_088, Some("00802.mpls")),
        ]
    );
    assert_eq!(plan.session, Some(session(87108, 1, hash, &[(4, 6), (5, 8)])));
    assert_filed_automatically(&plan);
}

#[test]
fn firefly_disc_1_plans_episodes_1_to_3_by_dvd_title_number() {
    let hash = "5A961BC67EB34171F24852D3A7688020";
    let plan = pinned(FIREFLY_1, hash);
    assert_discdb_one(&plan, show(1437), "Firefly", 2002);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
    assert_eq!(
        plan.titles,
        vec![
            planned("t00-title01", 0, episode(1, &[1], "Serenity"), 5186, 3_971_414_016, None),
            planned("t01-title02", 1, episode(1, &[2], "The Train Job"), 2554, 1_956_683_776, None),
            planned("t02-title03", 2, episode(1, &[3], "Bushwhacked"), 2627, 1_925_113_856, None),
        ]
    );
    assert_eq!(plan.session, Some(session(1437, 1, hash, &[(1, 0), (2, 1), (3, 2)])));
    assert_filed_automatically(&plan);
}

#[test]
fn firefly_disc_2_plans_episodes_4_to_7_with_dvd_title_11() {
    let hash = "294A3E64C457A1BC9A0833335C090568";
    let plan = pinned(FIREFLY_2, hash);
    assert_discdb_one(&plan, show(1437), "Firefly", 2002);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
    // Scan title 5 is DVD title 11 (Review Focus 1): its key names attribute 24.
    assert_eq!(
        plan.titles,
        vec![
            planned("t00-title01", 0, episode(1, &[4], "Shindig"), 2631, 1_987_276_800, None),
            planned("t01-title02", 1, episode(1, &[5], "Safe"), 2559, 1_906_104_320, None),
            planned("t02-title03", 2, episode(1, &[6], "Our Mrs. Reynolds"), 2629, 2_013_861_888, None),
            planned("t03-title11", 5, episode(1, &[7], "Jaynestown"), 2628, 1_966_837_760, None),
        ]
    );
    assert_eq!(plan.session, Some(session(1437, 1, hash, &[(4, 0), (5, 1), (6, 2), (7, 5)])));
    assert_filed_automatically(&plan);
}

/// plan_doc of one recording of the evidence clone, by slug, with the recording's folder on disk; None (after the skip
/// line) without the clone.
fn recording(slug: &str) -> Option<(PlanDoc, PathBuf)> {
    let ctx = context();
    let evidence = match resolve_evidence(&ctx, None) {
        Ok(dir) => dir,
        Err(refused) => {
            skip(&refused.line);
            return None;
        }
    };
    let evidence_arg = evidence.to_string_lossy().into_owned();
    let rec = match resolve_recording(&ctx, Some(evidence_arg.as_str()), slug) {
        Ok(rec) => rec,
        Err(refused) => {
            skip(&refused.line);
            return None;
        }
    };
    let doc = plan_doc(&ctx, &PlanSource::Recording(&rec), &PlanOptions::default())
        .unwrap_or_else(|refused| panic!("{}", refused.line));
    assert_eq!(doc.slug, slug);
    assert_eq!(doc.settings, Settings::default());
    assert_eq!(doc.settings_problems, Settings::default().validate());
    Some((doc, evidence.join(slug)))
}

/// A recording's file as every report prints it (port spec 3.4).
fn shown(slug: &str, file: &str) -> String {
    format!("corpus/owner/{slug}/{file}")
}

fn status(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(name, value)| (name.to_string(), value.to_string())).collect()
}

/// What only the evidence clone holds of a recording, read from its files when the test runs (ruling RP21):
/// meta.json's fingerprint and disc ID, and scan.txt typed by `disc::disc` as `plan_doc` reads it (UTF-8 with
/// replacement, through text mode).
struct CloneFacts {
    fingerprint: String,
    disc_id: Option<String>,
    disc: Disc,
}

/// A JSON file of the recording folder `dir`, a leading byte-order mark dropped.
fn clone_json(dir: &Path, file: &str) -> serde_json::Value {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The facts of the recording folder `dir`.
fn clone_facts(dir: &Path) -> CloneFacts {
    let meta = clone_json(dir, "meta.json");
    let scan = dir.join("scan.txt");
    let bytes = std::fs::read(&scan).unwrap_or_else(|error| panic!("{}: {error}", scan.display()));
    let disc = lazymkv_core::disc::disc(&parse(&text_mode(&String::from_utf8_lossy(&bytes))))
        .unwrap_or_else(|sentence| panic!("{}: {sentence}", scan.display()));
    CloneFacts {
        fingerprint: meta["fingerprint"].as_str().expect("meta.json states the fingerprint").to_string(),
        disc_id: meta["disc_id"].as_str().map(str::to_string),
        disc,
    }
}

/// A planned title of a recording whose seconds and size are the clone's scan's (attributes 9 and 11), neither
/// suggested nor remembered.
fn scanned(facts: &CloneFacts, key: &str, title: u32, role: PlannedRole, source_file: Option<&str>) -> PlannedTitle {
    let scan = facts.disc.title(title).unwrap_or_else(|sentence| panic!("{sentence}"));
    planned(
        key,
        title,
        role,
        scan.seconds.expect("the scan states the title's duration"),
        scan.size.expect("the scan states the title's size"),
        source_file,
    )
}

/// The episode names of the recording folder `dir`'s TheDiscDB answer (thediscdb-hash.json): each Episode item's
/// title, by the item's source file.
fn episode_names(dir: &Path) -> BTreeMap<String, String> {
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

#[test]
fn pans_labyrinth_2006_uhd() {
    let slug = "pans-labyrinth-2006-uhd";
    let Some((doc, _)) = recording(slug) else { return };
    assert_eq!(
        doc.inputs,
        PlanInputsSummary {
            scan: shown(slug, "scan.txt"),
            fingerprint: Some("6F39192C77BB1773D14982AD87B27DF6".to_string()),
            disc_id: Some("C337194670AF8C954C320875D671653F7EF2C256".to_string()),
            bdmt: Some(shown(slug, "nav/BDMV/META/DL/bdmt_eng.xml")),
            meta: true,
            discdb_hash: found(&shown(slug, "thediscdb-hash.json"), "hit"),
            discdb_discid: found(&shown(slug, "thediscdb-discid.json"), "miss"),
            tmdb: found(&shown(slug, "tmdb.json"), "hit"),
            tmdb_search: None,
            tmdb_record: None,
            lookup_status: status(&[("dischash", "200"), ("discid", "404"), ("tmdb", "200")]),
            remembered: None,
        }
    );
    let plan = doc.plan;
    // The disc ID answered 404, so 8.1 step 3 by fingerprint; tmdb.json is the same TMDb record, original es.
    assert_discdb_one(&plan, movie(1417), "Pan's Labyrinth", 2006);
    assert_eq!(plan.identity.original_language.as_deref(), Some("es"));
    assert_eq!(plan.wanted, wanted_languages(Some("es")));
    assert_eq!(plan.wanted.original, vec!["spa".to_string()]);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
    assert_eq!(plan.titles, vec![planned("t00-00800", 0, feature(None), 7150, 55_613_952_000, Some("00800.mpls"))]);
    assert_eq!(plan.evidence.title_rule, Some(TitleRule::Database));
    let name = plan.evidence.name.as_ref().expect("the bdmt file names the disc");
    assert_eq!((name.raw.as_str(), name.source), ("Pan's Labyrinth", NameSource::Bdmt));
    assert_eq!(plan.session, None);
    assert_filed_automatically(&plan);
}

#[test]
fn the_office_2005_s3d3_bluray() {
    let slug = "the-office-2005-s3d3-bluray";
    let Some((doc, dir)) = recording(slug) else { return };
    let facts = clone_facts(&dir);
    assert!(facts.disc_id.is_some(), "the Office's meta.json states a disc ID");
    assert_eq!(
        doc.inputs,
        PlanInputsSummary {
            scan: shown(slug, "scan.txt"),
            fingerprint: Some(facts.fingerprint.clone()),
            disc_id: facts.disc_id.clone(),
            bdmt: None,
            meta: true,
            discdb_hash: found(&shown(slug, "thediscdb-hash.json"), "hit"),
            discdb_discid: found(&shown(slug, "thediscdb-discid.json"), "miss"),
            tmdb: found(&shown(slug, "tmdb.json"), "hit"),
            tmdb_search: None,
            tmdb_record: None,
            lookup_status: status(&[("dischash", "200"), ("discid", "404"), ("tmdb", "200")]),
            remembered: None,
        }
    );
    let plan = doc.plan;
    assert_discdb_one(&plan, show(2316), "The Office", 2005);
    assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
    // 8.5: TheDiscDB maps the six titles of the Expected plans row, ordered by episode. Each title's playlist,
    // duration and size are the clone's scan's, its episode name the clone's TheDiscDB answer's (RP21); its key is
    // its plan position and its playlist's stem.
    let names = episode_names(&dir);
    let mapped = |position: usize, title: u32, episodes: &[u32]| {
        let playlist = facts
            .disc
            .title(title)
            .unwrap_or_else(|sentence| panic!("{sentence}"))
            .source_file
            .clone()
            .expect("a Blu-ray title has attribute 16");
        let key = format!("t{position:02}-{}", playlist.trim_end_matches(".mpls"));
        let name = names.get(&playlist).unwrap_or_else(|| panic!("TheDiscDB's answer names no episode for {playlist}"));
        scanned(&facts, &key, title, episode(3, episodes, name), Some(playlist.as_str()))
    };
    assert_eq!(
        plan.titles,
        vec![
            mapped(0, 3, &[12, 13]),
            mapped(1, 2, &[14]),
            mapped(2, 1, &[15]),
            mapped(3, 0, &[16]),
            mapped(4, 6, &[17]),
            mapped(5, 5, &[18]),
        ]
    );
    assert_eq!(
        plan.session,
        Some(session(2316, 3, &facts.fingerprint, &[(12, 3), (13, 3), (14, 2), (15, 1), (16, 0), (17, 6), (18, 5)]))
    );
    let name = plan.evidence.name.as_ref().expect("the volume label names the disc");
    assert_eq!(
        (name.raw.as_str(), name.source, name.season, name.disc),
        ("THE OFFICE S3D3", NameSource::VolumeLabel, Some(3), Some(3))
    );
    assert_eq!(plan.wanted, wanted_languages(Some("en")));
    assert_filed_automatically(&plan);
}

/// A Short Circuit recording: no answer in hand (TheDiscDB answered 404; no disc ID; no TMDb search is recorded), so
/// 8.1 step 6 and 8.3 rule 3.5's one feature-length title at 0.85. The fingerprint, the volume label and title 0's
/// duration and size are the clone's (RP21).
fn assert_short_circuit(doc: &PlanDoc, slug: &str, facts: &CloneFacts, generic: bool) {
    assert_eq!(
        doc.inputs,
        PlanInputsSummary {
            scan: shown(slug, "scan.txt"),
            fingerprint: Some(facts.fingerprint.clone()),
            disc_id: None,
            bdmt: None,
            meta: true,
            discdb_hash: found(&shown(slug, "thediscdb-hash.json"), "miss"),
            discdb_discid: not_made(),
            tmdb: not_made(),
            tmdb_search: None,
            tmdb_record: None,
            lookup_status: status(&[("dischash", "404")]),
            remembered: None,
        }
    );
    let plan = &doc.plan;
    assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Nothing, 0.0));
    assert_eq!((plan.identity.tmdb, plan.identity.lookup_pending), (None, false));
    assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Default));
    assert_eq!(plan.titles, vec![scanned(facts, "t00-title01", 0, feature(None), None)]);
    assert_eq!(plan.evidence.title_rule, Some(TitleRule::Structure));
    assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
    assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown]);
    // A DVD has no Java menus: the label rule never applies, so no note records its absence.
    assert_eq!(plan.notes.iter().map(|noted| noted.note).collect::<Vec<Note>>(), Vec::<Note>::new());
    assert_eq!(plan.wanted, wanted_languages(None));
    let volume_label = facts.disc.volume_label.as_deref().expect("the scan states the volume label (CINFO 32)");
    let found_name = plan.evidence.name.as_ref().expect("the volume label names the disc");
    assert_eq!(
        (found_name.raw.as_str(), found_name.source, found_name.generic),
        (volume_label, NameSource::VolumeLabel, generic)
    );
}

#[test]
fn short_circuit_1986_dvd() {
    let slug = "short-circuit-1986-dvd";
    let Some((doc, dir)) = recording(slug) else { return };
    // The disc's own volume label, not a generic one (8.1).
    assert_short_circuit(&doc, slug, &clone_facts(&dir), false);
}

#[test]
fn short_circuit_2_1988_dvd() {
    let slug = "short-circuit-2-1988-dvd";
    let Some((doc, dir)) = recording(slug) else { return };
    let facts = clone_facts(&dir);
    // DVD_VIDEO is a generic name (8.1): never searched.
    assert_eq!(facts.disc.volume_label.as_deref(), Some("DVD_VIDEO"));
    assert_short_circuit(&doc, slug, &facts, true);
}

#[test]
fn recordings_without_a_test_are_listed_as_not_covered() {
    let ctx = context();
    let evidence = match resolve_evidence(&ctx, None) {
        Ok(dir) => dir,
        Err(refused) => return skip(&refused.line),
    };
    let mut slugs: Vec<String> = std::fs::read_dir(&evidence)
        .expect("the evidence clone is readable")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_slug(name) && name != "golden")
        .collect();
    slugs.sort();
    for slug in slugs {
        if !RECORDINGS.contains(&slug.as_str()) {
            skip(&format!("not covered: {} (crates/dev/tests/plan.rs has no test for it)", slug));
        }
    }
}

/// The command itself over one pinned log: exit 0 and plan_doc's document, printed by pyjson::dumps with indent 1.
#[test]
fn the_command_prints_a_pinned_logs_plan_and_exits_0() {
    let ctx = pinned_ctx();
    let path = format!("../../corpus/thediscdb/{SMILE}");
    let argv = vec!["plan".to_string(), path.clone()];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = lazymkv_dev::dispatch(&ctx, &argv, &mut out, &mut err);
    assert_eq!((code, String::from_utf8(err).unwrap()), (0, String::new()));
    let doc = plan_doc(&ctx, &PlanSource::ScanFile(Path::new(&path)), &PlanOptions::default())
        .unwrap_or_else(|refused| panic!("{}", refused.line));
    let expected = lazymkv_dev::pyjson::dumps(
        &serde_json::to_value(&doc).expect("the document serialises"),
        lazymkv_dev::pyjson::Style::INDENT1_ASCII,
    );
    let printed = String::from_utf8(out).unwrap();
    assert_eq!(printed, format!("{expected}\n"));
    assert_eq!(serde_json::from_str::<PlanDoc>(&printed).expect("the document reads back"), doc);
}

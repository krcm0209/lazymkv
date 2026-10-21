//! Spec 5's remembered decisions (`<fingerprint>.json`: the TMDb identity and kind; for each title its source file,
//! segment map, duration, role, and its audio and subtitle overrides keyed by the stream's signature) and spec 11's
//! review actions (Set identity, Assign titles, Override tracks, Resolve collision, Remember), RP16: the types, the title
//! match (source file, segment map and duration within 2 s, never the title key) and the override lookups by signature
//! within the matched title. A
//! remembered or review decision sets the confidence it covers to 1.00 (`lookup::REMEMBERED_CONFIDENCE`, spec 5 and
//! 8.8). Persistence is phase 2. Used by the planner (Task 7), the audio and subtitle rules (Tasks 8 and 10), the
//! decision (Task 13) and the job reducer (Task 14).

use std::collections::BTreeMap;
use std::str::FromStr;

use crate::codec::Family;
use crate::disc::{same_source, segment_maps_agree, title_source, Disc, Stream, StreamKind, Title, DURATION_MATCH_S};
use crate::lookup::tmdb::TmdbIdentity;
use crate::settings::same_path;
use crate::tracks::SubtitleRole;

/// A title's role in a remembered or review decision (spec 5; 11 "Assign titles", with an editable edition name). Def: T6. Use: T7, T13.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "role")]
pub enum TitleRole {
    /// The film's feature (its only or main cut).
    Feature,
    /// A named cut (8.4), its edition name as the owner left it; naming::edition_label sanitises and caps it.
    Cut { edition: String },
    /// One or more episodes of a season (8.5; season 0 is the specials season).
    Episodes { season: u32, episodes: Vec<u32> },
    /// Not filed.
    Ignored,
}

/// One title of a remembered decision (spec 5: "for each title its source file, segment map, duration, role, and its
/// audio and subtitle overrides keyed by the stream's signature"); `dvd_title` is attribute 24, the DVD's identity of a
/// title (Review Focus 1). The overrides are the title's own: a signature names a stream within this title, so two
/// titles of one disc keep their own decisions for streams of one signature. Def: T6. Use: T6, T13.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RememberedTitle {
    /// Attribute 16 (a Blu-ray's playlist, "00800.mpls"); None on a DVD.
    pub source_file: Option<String>,
    /// Attribute 24 as MakeMKV wrote it ("01"); None on a Blu-ray.
    pub dvd_title: Option<String>,
    /// Attribute 26 as text (it may end in "...").
    pub segment_map: String,
    /// Attribute 9 in seconds.
    pub seconds: u32,
    pub role: TitleRole,
    /// The title's audio overrides by signature, in the order Remember wrote them (`apply_audio` reads them for the scan
    /// title `matched_titles` matched to this one).
    pub audio: Vec<AudioOverrideEntry>,
    /// The title's subtitle overrides by signature (`apply_subtitles`).
    pub subtitles: Vec<SubtitleOverrideEntry>,
}

/// A stream's signature (spec 5): kind, language, codec family, channels, position among identical streams (0-based). Def: T6. Use: T8, T10, T13.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct StreamSignature {
    /// Attribute 1.
    pub kind: StreamKind,
    /// Attribute 3 lower-cased (`Stream.language`); None when absent.
    pub language: Option<String>,
    /// The codec family of attributes 5, 6 and 7 (`Stream.codec.family`).
    pub codec: Family,
    /// Attribute 14.
    pub channels: Option<u32>,
    /// How many earlier streams of the title share the four fields above.
    pub position: u32,
}

/// An audio override (spec 11 "Override tracks": keep with order and default, drop, or mark as commentary, which drops it). Def: T6. Use: T8, T13, T14.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum AudioOverride {
    /// Not kept.
    Drop,
    /// Kept at `order` among the kept audio tracks (0 first); the default track when `default`.
    Keep { order: u32, default: bool },
    /// Marked as commentary: dropped (spec 11).
    Commentary,
}

/// A subtitle override (spec 11: keep with a role, or drop). Def: T6. Use: T10, T13, T14.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum SubtitleOverride {
    /// Not kept.
    Drop,
    /// Kept as the forced, regular or SDH (subtitles for the deaf and hard of hearing) track.
    Keep { role: SubtitleRole },
}

/// One remembered audio override: the stream's signature and the action, flattened into one JSON object
/// (`{"signature": {...}, "action": "keep", "order": 1, "default": true}`). A list, not a map: serde_json writes no map
/// keyed by a struct. Spec 5. Def: T6. Use: T8, T13.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioOverrideEntry {
    pub signature: StreamSignature,
    #[serde(flatten)]
    pub action: AudioOverride,
}

/// The same for subtitles. Spec 5. Def: T6. Use: T10, T13.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SubtitleOverrideEntry {
    pub signature: StreamSignature,
    #[serde(flatten)]
    pub action: SubtitleOverride,
}

/// The remembered-decisions file (spec 5): the TMDb identity (its `kind` is the kind; no second field) and the titles,
/// each with its own track overrides. Round-trips through serde_json (`a_remembered_file_round_trips_through_json`).
/// Def: T6. Use: T7, T13, T15.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Remembered {
    /// The disc's fingerprint (spec 5), the file's name.
    pub fingerprint: String,
    pub tmdb: TmdbIdentity,
    pub titles: Vec<RememberedTitle>,
}

/// Spec 11 "Resolve collision": the action. `CollisionAnswer` ties it to the path it answers. Def: T6. Use: T13.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Collision {
    /// Keep both: the new file gets a distinguishing label such as `- 2160p 2`.
    KeepBoth,
    /// Replace the existing file after an explicit confirmation (it is moved into the job's staging folder and kept).
    Replace,
    /// Discard the job.
    Discard,
}

impl Collision {
    /// The spec spelling, as serde writes it: "keep_both", "replace", "discard". Spec 11.
    pub fn as_str(self) -> &'static str {
        match self {
            Collision::KeepBoth => "keep_both",
            Collision::Replace => "replace",
            Collision::Discard => "discard",
        }
    }

    /// The three actions in spec 11's order.
    pub const ALL: [Collision; 3] = [Collision::KeepBoth, Collision::Replace, Collision::Discard];
}

impl FromStr for Collision {
    type Err = String;

    /// Only the three spec spellings. Spec 11.
    fn from_str(text: &str) -> Result<Collision, String> {
        Collision::ALL
            .into_iter()
            .find(|collision| collision.as_str() == text)
            .ok_or_else(|| format!("'{text}' is not a collision action (keep_both, replace or discard)"))
    }
}

/// One answer of spec 11's Resolve collision: the final path the owner was shown colliding with a library file (the
/// decision's plain final path, `NamedPath.path`, without the keep-both label) and the action confirmed for it. Keep
/// both and Replace resolve the collision of that path only: a decision that builds another path (after Set identity, a
/// new edition or season, or a library change) raises `path_collision` again (spec 7: a file the job did not create is
/// never overwritten without its own confirmation). Discard discards the job whatever the path. Spec 7, 11. Def: PR #3
/// review (F5). Use: T13, T14.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CollisionAnswer {
    /// The colliding final path as shown, compared whole, case-insensitively and separators aside (spec 7's Windows
    /// shares).
    pub path: String,
    pub action: Collision,
}

/// The signature of `streams[index]` (position counts earlier streams with the same kind, language, codec and channels).
/// `index` must be below `streams.len()`: callers pass a title's own stream list and an index into it. Spec 5. Def: T6.
/// Use: T6, T8, T10, T13.
pub fn signature(streams: &[Stream], index: usize) -> StreamSignature {
    let stream = &streams[index];
    let identical = |other: &&Stream| {
        other.kind == stream.kind
            && other.language == stream.language
            && other.codec.family == stream.codec.family
            && other.channels == stream.channels
    };
    StreamSignature {
        kind: stream.kind,
        language: stream.language.clone(),
        codec: stream.codec.family,
        channels: stream.channels,
        position: streams[..index].iter().filter(identical).count() as u32,
    }
}

/// Stream number to action for every stream of `kind` among `streams` whose signature has an entry (the first entry with
/// that signature); streams of other kinds are never looked up, so an entry whose signature names another kind is
/// never applied.
fn by_signature<'a, A>(entries: &[(&'a StreamSignature, &'a A)], streams: &[Stream], kind: StreamKind) -> BTreeMap<u32, &'a A> {
    let mut out = BTreeMap::new();
    for (index, stream) in streams.iter().enumerate() {
        if stream.kind != kind {
            continue;
        }
        let wanted = signature(streams, index);
        if let Some(entry) = entries.iter().find(|entry| *entry.0 == wanted) {
            out.insert(stream.number, entry.1);
        }
    }
    out
}

/// Stream number to override for every stream whose signature has an entry. `overrides` is one title's list: the
/// `RememberedTitle.audio` of the remembered title `matched_titles` matched to the scan title whose streams these are.
/// Only the audio streams of `streams` are looked up. Spec 5, 11. Def: T6. Use: T8.
pub fn apply_audio<'a>(overrides: &'a [AudioOverrideEntry], streams: &[Stream]) -> BTreeMap<u32, &'a AudioOverride> {
    let entries: Vec<(&StreamSignature, &AudioOverride)> =
        overrides.iter().map(|entry| (&entry.signature, &entry.action)).collect();
    by_signature(&entries, streams, StreamKind::Audio)
}

/// The same for subtitles, over the matched title's `RememberedTitle.subtitles`: only the subtitle streams of `streams`
/// are looked up. Spec 5, 11. Def: T6. Use: T10.
pub fn apply_subtitles<'a>(overrides: &'a [SubtitleOverrideEntry], streams: &[Stream]) -> BTreeMap<u32, &'a SubtitleOverride> {
    let entries: Vec<(&StreamSignature, &SubtitleOverride)> =
        overrides.iter().map(|entry| (&entry.signature, &entry.action)).collect();
    by_signature(&entries, streams, StreamKind::Subtitles)
}

/// The remembered title is this scan title: `disc::same_source(&title_source(rem.source_file, rem.dvd_title), &title.source())`
/// (source file on a Blu-ray; on a DVD, both source files None, attribute 24 equal as numbers), `disc::segment_maps_agree`,
/// and duration within `disc::DURATION_MATCH_S`; never by title key. A scan title without a duration, or a remembered
/// title with neither a source file nor attribute 24, never matches. Def: T6. Use: T6, T7.
pub fn match_title(remembered: &RememberedTitle, title: &Title) -> bool {
    let source = title_source(remembered.source_file.as_deref(), remembered.dvd_title.as_deref());
    same_source(&source, &title.source())
        && segment_maps_agree(&remembered.segment_map, &title.segment_map)
        && title.seconds.map_or(false, |seconds| seconds.abs_diff(remembered.seconds) <= DURATION_MATCH_S)
}

/// Every scan title matched to a remembered title, title number to that remembered title (a remembered title is used
/// once). The scan's titles are taken in number order, each given the first remembered title not yet used that matches
/// it. The matched title's `audio` and `subtitles` are the overrides `apply_audio` and `apply_subtitles` apply to that
/// scan title. Spec 5, 6.1 step 2 ("A new job applies any remembered decisions"). Def: PR #3 review (F10). Use: T7,
/// T13, phase 2.
pub fn matched_titles<'a>(remembered: &'a Remembered, disc: &Disc) -> BTreeMap<u32, &'a RememberedTitle> {
    let mut used = vec![false; remembered.titles.len()];
    let mut out = BTreeMap::new();
    for (&number, title) in &disc.titles {
        let found = (0..remembered.titles.len()).find(|&at| !used[at] && match_title(&remembered.titles[at], title));
        if let Some(at) = found {
            used[at] = true;
            out.insert(number, &remembered.titles[at]);
        }
    }
    out
}

/// Every scan title matched to a remembered title, title number to role (`matched_titles`' roles). Spec 5, 6.1 step 2.
/// Def: T6. Use: T7.
pub fn apply_titles<'a>(remembered: &'a Remembered, disc: &Disc) -> BTreeMap<u32, &'a TitleRole> {
    matched_titles(remembered, disc).into_iter().map(|(number, title)| (number, &title.role)).collect()
}

/// One title of the owner's Assign titles: the scan title number and its role, flattened into one JSON object
/// (`{"title": 3, "role": "episodes", "season": 3, "episodes": [12, 13]}`). Spec 11. Def: T6. Use: T7, T13, T14.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TitleOverrideEntry {
    /// The scan title number (the key).
    pub title: u32,
    #[serde(flatten)]
    pub role: TitleRole,
}

/// One track of the owner's Override tracks: the scan title and stream numbers and the action, flattened into one JSON
/// object (`{"title": 0, "stream": 1, "action": "keep", "order": 0, "default": true}`). Spec 11. Def: T6. Use: T13, T14.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioTrackOverride {
    /// The scan title number (the first key).
    pub title: u32,
    /// The scan stream number within that title (the second key; `Stream.number` restarts in every title).
    pub stream: u32,
    #[serde(flatten)]
    pub action: AudioOverride,
}

/// The same for subtitles. Spec 11. Def: T6. Use: T13.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SubtitleTrackOverride {
    /// The scan title number (the first key).
    pub title: u32,
    /// The scan stream number within that title (the second key).
    pub stream: u32,
    #[serde(flatten)]
    pub action: SubtitleOverride,
}

/// The owner's review decisions on one job (spec 11 Set identity, Assign titles, Override tracks, Resolve collision); each
/// sets the confidence it covers to 1.00 (8.8). Assign titles and Override tracks are lists of entries keyed by the scan's
/// title number (and stream number) of this job, sorted by those keys, one entry per key: lists, not integer-keyed maps,
/// because `Event::ReviewApproved` carries them inside an internally tagged enum, whose buffered content cannot give an
/// integer map key (the interfaces' "Keyed collections"). Def: T6. Use: T7, T13, T14.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Overrides {
    /// Set identity.
    pub identity: Option<TmdbIdentity>,
    /// The season review asked for (8.5) when the plan has none.
    pub season: Option<u32>,
    /// Assign titles: one entry per title, sorted by title number.
    pub titles: Vec<TitleOverrideEntry>,
    /// Override tracks: one entry per scan title and stream, sorted by title, then stream.
    pub audio: Vec<AudioTrackOverride>,
    pub subtitles: Vec<SubtitleTrackOverride>,
    /// Resolve collision: one answer per colliding final path the owner was shown, sorted by path (a list of entries,
    /// as the other keyed collections here).
    pub collision: Vec<CollisionAnswer>,
}

impl Overrides {
    /// The role the owner's Assign titles gives scan title `title`, if it names it. Spec 11. Def: T6. Use: T13.
    pub fn title_role(&self, title: u32) -> Option<&TitleRole> {
        self.titles.iter().find(|entry| entry.title == title).map(|entry| &entry.role)
    }

    /// The owner's audio overrides of scan title `title` by scan stream number (the form `AudioFacts.reviewed` takes);
    /// None when no entry names the title. Spec 11. Def: T6. Use: T13.
    pub fn audio_of(&self, title: u32) -> Option<BTreeMap<u32, AudioOverride>> {
        let streams: BTreeMap<u32, AudioOverride> = self
            .audio
            .iter()
            .filter(|entry| entry.title == title)
            .map(|entry| (entry.stream, entry.action.clone()))
            .collect();
        (!streams.is_empty()).then_some(streams)
    }

    /// Whether the owner's Resolve collision discarded the job (any answer is Discard). Spec 11. Def: PR #3 review (F5).
    /// Use: T13.
    pub fn discards(&self) -> bool {
        self.collision.iter().any(|answer| answer.action == Collision::Discard)
    }

    /// The action the owner confirmed for the colliding final path `path` (compared whole, case-insensitively,
    /// separators and a trailing separator aside, `settings::same_path`); None when no answer names it.
    /// Spec 7, 11. Def: PR #3 review (F5). Use: T13.
    pub fn collision_for(&self, path: &str) -> Option<Collision> {
        self.collision.iter().find(|answer| same_path(&answer.path, path)).map(|answer| answer.action)
    }

    /// The same for subtitles (`SubtitleFacts.reviewed`). Spec 11. Def: T6. Use: T13.
    pub fn subtitles_of(&self, title: u32) -> Option<BTreeMap<u32, SubtitleOverride>> {
        let streams: BTreeMap<u32, SubtitleOverride> = self
            .subtitles
            .iter()
            .filter(|entry| entry.title == title)
            .map(|entry| (entry.stream, entry.action.clone()))
            .collect();
        (!streams.is_empty()).then_some(streams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::disc::disc;
    use crate::lookup::tmdb::TmdbKind;
    use crate::plan::fixtures::{pinned, CHERNOBYL_1, ENG_AC3, ENG_AC3_2CH, ENG_PGS, FIREFLY_1_API, FIREFLY_2, VIDEO};
    use crate::robot::parse;
    use crate::testsupport::scan_text;

    fn json_of<T: serde::Serialize>(value: &T) -> String {
        serde_json::to_string(value).unwrap()
    }

    /// Writes `value` as JSON and reads it back: the same value.
    fn round_trips<T>(value: &T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        assert_eq!(&serde_json::from_str::<T>(&json_of(value)).unwrap(), value);
    }

    /// The signature of an English stereo Dolby Digital stream at `position` among identical ones.
    fn eng_ac3_2(position: u32) -> StreamSignature {
        StreamSignature { kind: StreamKind::Audio, language: Some("eng".to_string()), codec: Family::Ac3, channels: Some(2), position }
    }

    /// The signature of an English PGS subtitle stream at `position` among identical ones.
    fn eng_pgs(position: u32) -> StreamSignature {
        StreamSignature { kind: StreamKind::Subtitles, language: Some("eng".to_string()), codec: Family::Pgs, channels: None, position }
    }

    fn remembered_title(
        source_file: Option<&str>,
        dvd_title: Option<&str>,
        segment_map: &str,
        seconds: u32,
        role: TitleRole,
    ) -> RememberedTitle {
        RememberedTitle {
            source_file: source_file.map(str::to_string),
            dvd_title: dvd_title.map(str::to_string),
            segment_map: segment_map.to_string(),
            seconds,
            role,
            audio: Vec::new(),
            subtitles: Vec::new(),
        }
    }

    fn episodes(season: u32, numbers: &[u32]) -> TitleRole {
        TitleRole::Episodes { season, episodes: numbers.to_vec() }
    }

    /// What Remember (spec 11) stores for Firefly's first DVD (TMDb show 1437): its three episode titles by attribute 24,
    /// each with its track overrides by signature (the pilot's second stereo track marked commentary, the next
    /// episode's kept), and its two short titles ignored.
    fn firefly_remembered() -> Remembered {
        let vobsub =
            StreamSignature { kind: StreamKind::Subtitles, language: Some("eng".to_string()), codec: Family::VobSub, channels: None, position: 0 };
        let regular = SubtitleOverrideEntry { signature: vobsub, action: SubtitleOverride::Keep { role: SubtitleRole::Regular } };
        let mut pilot = remembered_title(None, Some("01"), "1-2,3,4-6,7-10,11-15,16-20,21-24,25", 5186, episodes(1, &[1]));
        pilot.audio = vec![
            AudioOverrideEntry { signature: eng_ac3_2(0), action: AudioOverride::Keep { order: 0, default: true } },
            AudioOverrideEntry { signature: eng_ac3_2(1), action: AudioOverride::Commentary },
        ];
        pilot.subtitles = vec![regular.clone()];
        let mut second = remembered_title(None, Some("02"), "1,2-6,7-11,12-14,15", 2554, episodes(1, &[2]));
        second.audio = vec![
            AudioOverrideEntry { signature: eng_ac3_2(0), action: AudioOverride::Keep { order: 0, default: true } },
            AudioOverrideEntry { signature: eng_ac3_2(1), action: AudioOverride::Keep { order: 1, default: false } },
        ];
        second.subtitles = vec![regular];
        Remembered {
            fingerprint: FIREFLY_1_API.to_string(),
            tmdb: TmdbIdentity { kind: TmdbKind::Tv, id: 1437 },
            titles: vec![
                pilot,
                second,
                remembered_title(None, Some("03"), "1-4,5-10,11-14,15", 2627, episodes(1, &[3])),
                remembered_title(None, Some("05"), "1,2", 7, TitleRole::Ignored),
                remembered_title(None, Some("06"), "1,2", 11, TitleRole::Ignored),
            ],
        }
    }

    #[test]
    fn title_roles_are_written_with_the_spec_spellings() {
        let roles = [
            (TitleRole::Feature, r#"{"role":"feature"}"#),
            (TitleRole::Cut { edition: "Unrated Cut".to_string() }, r#"{"role":"cut","edition":"Unrated Cut"}"#),
            (episodes(3, &[12, 13]), r#"{"role":"episodes","season":3,"episodes":[12,13]}"#),
            (TitleRole::Ignored, r#"{"role":"ignored"}"#),
        ];
        for (role, text) in roles {
            assert_eq!(json_of(&role), text);
            round_trips(&role);
        }
    }

    #[test]
    fn a_stream_signature_is_written_kind_language_codec_channels_position() {
        assert_eq!(
            json_of(&eng_ac3_2(1)),
            format!(
                r#"{{"kind":{},"language":"eng","codec":{},"channels":2,"position":1}}"#,
                json_of(&StreamKind::Audio),
                json_of(&Family::Ac3)
            )
        );
        let video = StreamSignature { kind: StreamKind::Video, language: None, codec: Family::Hevc, channels: None, position: 0 };
        assert_eq!(
            json_of(&video),
            format!(
                r#"{{"kind":{},"language":null,"codec":{},"channels":null,"position":0}}"#,
                json_of(&StreamKind::Video),
                json_of(&Family::Hevc)
            )
        );
        round_trips(&video);
    }

    #[test]
    fn override_entries_write_the_action_beside_the_signature() {
        let audio = json_of(&eng_ac3_2(0));
        let cases = [
            (AudioOverride::Keep { order: 1, default: true }, format!(r#"{{"signature":{audio},"action":"keep","order":1,"default":true}}"#)),
            (AudioOverride::Drop, format!(r#"{{"signature":{audio},"action":"drop"}}"#)),
            (AudioOverride::Commentary, format!(r#"{{"signature":{audio},"action":"commentary"}}"#)),
        ];
        for (action, text) in cases {
            let entry = AudioOverrideEntry { signature: eng_ac3_2(0), action };
            assert_eq!(json_of(&entry), text);
            round_trips(&entry);
        }
        let subtitles = json_of(&eng_pgs(0));
        let sdh = SubtitleOverrideEntry { signature: eng_pgs(0), action: SubtitleOverride::Keep { role: SubtitleRole::Sdh } };
        assert_eq!(
            json_of(&sdh),
            format!(r#"{{"signature":{subtitles},"action":"keep","role":{}}}"#, json_of(&SubtitleRole::Sdh))
        );
        round_trips(&sdh);
        let dropped = SubtitleOverrideEntry { signature: eng_pgs(0), action: SubtitleOverride::Drop };
        assert_eq!(json_of(&dropped), format!(r#"{{"signature":{subtitles},"action":"drop"}}"#));
        round_trips(&dropped);
    }

    #[test]
    fn collision_actions_use_the_spec_spellings() {
        assert_eq!(Collision::ALL.map(Collision::as_str), ["keep_both", "replace", "discard"]);
        for collision in Collision::ALL {
            assert_eq!(json_of(&collision), format!("\"{}\"", collision.as_str()));
            assert_eq!(collision.as_str().parse::<Collision>(), Ok(collision));
            round_trips(&collision);
        }
        assert_eq!(
            "keep-both".parse::<Collision>(),
            Err("'keep-both' is not a collision action (keep_both, replace or discard)".to_string())
        );
    }

    #[test]
    fn a_remembered_file_round_trips_through_json() {
        let remembered = firefly_remembered();
        let text = serde_json::to_string_pretty(&remembered).unwrap();
        assert_eq!(serde_json::from_str::<Remembered>(&text).unwrap(), remembered);
    }

    #[test]
    fn a_remembered_file_is_written_with_its_fields_in_order() {
        let remembered = Remembered {
            fingerprint: FIREFLY_1_API.to_string(),
            tmdb: TmdbIdentity { kind: TmdbKind::Tv, id: 1437 },
            titles: vec![remembered_title(None, Some("05"), "1,2", 7, TitleRole::Ignored)],
        };
        assert_eq!(
            json_of(&remembered),
            format!(
                concat!(
                    r#"{{"fingerprint":"5A961BC67EB34171F24852D3A7688020","tmdb":{},"titles":[{{"source_file":null,"#,
                    r#""dvd_title":"05","segment_map":"1,2","seconds":7,"role":{{"role":"ignored"}},"audio":[],"#,
                    r#""subtitles":[]}}]}}"#
                ),
                json_of(&remembered.tmdb)
            )
        );
    }

    #[test]
    fn empty_overrides_are_written_with_every_key() {
        assert_eq!(
            json_of(&Overrides::default()),
            r#"{"identity":null,"season":null,"titles":[],"audio":[],"subtitles":[],"collision":[]}"#
        );
    }

    #[test]
    fn overrides_round_trip_through_json_as_sorted_entries_and_answer_their_lookups() {
        let identity = TmdbIdentity { kind: TmdbKind::Tv, id: 2316 };
        let keep = AudioOverride::Keep { order: 0, default: true };
        let forced = SubtitleOverride::Keep { role: SubtitleRole::Forced };
        let overrides = Overrides {
            identity: Some(identity),
            season: Some(3),
            titles: vec![
                TitleOverrideEntry { title: 3, role: episodes(3, &[12, 13]) },
                TitleOverrideEntry { title: 4, role: TitleRole::Ignored },
            ],
            audio: vec![
                AudioTrackOverride { title: 0, stream: 1, action: keep.clone() },
                AudioTrackOverride { title: 0, stream: 3, action: AudioOverride::Commentary },
            ],
            subtitles: vec![
                SubtitleTrackOverride { title: 0, stream: 5, action: forced.clone() },
                SubtitleTrackOverride { title: 0, stream: 6, action: SubtitleOverride::Drop },
            ],
            collision: vec![CollisionAnswer { path: r"\\nas\media\Movies\Film\Film.mkv".to_string(), action: Collision::KeepBoth }],
        };
        let text = json_of(&overrides);
        assert_eq!(
            text,
            format!(
                concat!(
                    r#"{{"identity":{},"season":3,"titles":[{{"title":3,"role":"episodes","season":3,"episodes":[12,13]}},"#,
                    r#"{{"title":4,"role":"ignored"}}],"audio":[{{"title":0,"stream":1,"action":"keep","order":0,"#,
                    r#""default":true}},{{"title":0,"stream":3,"action":"commentary"}}],"subtitles":[{{"title":0,"#,
                    r#""stream":5,"action":"keep","role":{}}},{{"title":0,"stream":6,"action":"drop"}}],"#,
                    r#""collision":[{{"path":"\\\\nas\\media\\Movies\\Film\\Film.mkv","action":"keep_both"}}]}}"#
                ),
                json_of(&identity),
                json_of(&SubtitleRole::Forced)
            )
        );
        assert_eq!(serde_json::from_str::<Overrides>(&text).unwrap(), overrides);

        assert_eq!(overrides.title_role(3), Some(&episodes(3, &[12, 13])));
        assert_eq!(overrides.title_role(4), Some(&TitleRole::Ignored));
        assert_eq!(overrides.title_role(0), None);
        assert_eq!(overrides.audio_of(0), Some(BTreeMap::from([(1, keep), (3, AudioOverride::Commentary)])));
        assert_eq!(overrides.audio_of(3), None);
        assert_eq!(overrides.subtitles_of(0), Some(BTreeMap::from([(5, forced), (6, SubtitleOverride::Drop)])));
        assert_eq!(overrides.subtitles_of(1), None);
        // Resolve collision answers its own path only, whatever its case or separators; nothing here discards the job.
        assert_eq!(overrides.collision_for(r"\\nas\media\Movies\Film\Film.mkv"), Some(Collision::KeepBoth));
        assert_eq!(overrides.collision_for("//NAS/media/movies/film/film.mkv"), Some(Collision::KeepBoth));
        assert_eq!(overrides.collision_for(r"\\nas\media\Movies\Other\Other.mkv"), None);
        assert!(!overrides.discards());
        let discarded = Overrides {
            collision: vec![CollisionAnswer { path: "/srv/Film.mkv".to_string(), action: Collision::Discard }],
            ..Overrides::default()
        };
        assert!(discarded.discards());
    }

    // VIDEO, ENG_AC3 (six channels), ENG_AC3_2CH and ENG_PGS are the `plan::fixtures` streams (AVC video; English PGS subtitles).
    const SPA_AC3_2CH: &[(u32, &str)] = &[(1, "Audio"), (3, "spa"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")];
    const ENG_DTS_HD_MA_6CH: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_DTS"), (6, "DTS-HD MA"), (7, "DTS-HD Master Audio"), (14, "6")];
    const SPA_PGS: &[(u32, &str)] = &[(1, "Subtitles"), (3, "spa"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];

    /// Title 0 of a synthetic Blu-ray scan whose stream n has the attributes `streams[n]`.
    fn title_with(streams: &[&[(u32, &str)]]) -> Title {
        let numbered: Vec<(u32, &[(u32, &str)])> =
            streams.iter().enumerate().map(|(number, attrs)| (number as u32, *attrs)).collect();
        let attrs: &[(u32, &str)] = &[(9, "1:00:00"), (16, "00800.mpls"), (26, "1")];
        let text = scan_text(&[(1, "Blu-ray disc")], &[(0, attrs, numbered.as_slice())]);
        let mut typed = disc(&parse(&text)).unwrap();
        typed.titles.remove(&0).unwrap()
    }

    #[test]
    fn a_stream_signature_counts_its_position_among_identical_streams() {
        let title =
            title_with(&[VIDEO, ENG_AC3_2CH, ENG_DTS_HD_MA_6CH, ENG_AC3_2CH, SPA_AC3_2CH, ENG_PGS, ENG_PGS, ENG_AC3]);
        let streams = &title.streams;
        assert_eq!(
            signature(streams, 0),
            StreamSignature { kind: StreamKind::Video, language: None, codec: Family::Avc, channels: None, position: 0 }
        );
        assert_eq!(signature(streams, 1), eng_ac3_2(0));
        assert_eq!(
            signature(streams, 2),
            StreamSignature { kind: StreamKind::Audio, language: Some("eng".to_string()), codec: Family::DtsHdMa, channels: Some(6), position: 0 }
        );
        assert_eq!(signature(streams, 3), eng_ac3_2(1));
        assert_eq!(
            signature(streams, 4),
            StreamSignature { kind: StreamKind::Audio, language: Some("spa".to_string()), codec: Family::Ac3, channels: Some(2), position: 0 }
        );
        assert_eq!(signature(streams, 5), eng_pgs(0));
        assert_eq!(signature(streams, 6), eng_pgs(1));
        // Six channels make it a different stream from the stereo ones, though language and codec agree.
        assert_eq!(
            signature(streams, 7),
            StreamSignature { kind: StreamKind::Audio, language: Some("eng".to_string()), codec: Family::Ac3, channels: Some(6), position: 0 }
        );
    }

    #[test]
    fn audio_overrides_follow_the_stream_signature_not_the_stream_number() {
        let dts = StreamSignature { kind: StreamKind::Audio, language: Some("eng".to_string()), codec: Family::DtsHdMa, channels: Some(6), position: 0 };
        let spanish = StreamSignature { kind: StreamKind::Audio, language: Some("spa".to_string()), codec: Family::Ac3, channels: Some(2), position: 0 };
        // Remembered from a scan listing video, English stereo, English DTS-HD MA, English stereo and Spanish stereo.
        let overrides = vec![
            AudioOverrideEntry { signature: dts, action: AudioOverride::Keep { order: 0, default: true } },
            AudioOverrideEntry { signature: eng_ac3_2(1), action: AudioOverride::Commentary },
            AudioOverrideEntry { signature: spanish, action: AudioOverride::Drop },
            // No stream of the title has this signature.
            AudioOverrideEntry { signature: eng_ac3_2(2), action: AudioOverride::Drop },
            // A subtitle signature in an audio entry never lands on the subtitle stream.
            AudioOverrideEntry { signature: eng_pgs(0), action: AudioOverride::Drop },
        ];
        // This scan lists the same streams in another order.
        let title = title_with(&[VIDEO, ENG_DTS_HD_MA_6CH, ENG_AC3_2CH, SPA_AC3_2CH, ENG_AC3_2CH, ENG_PGS]);
        assert_eq!(
            apply_audio(&overrides, &title.streams),
            BTreeMap::from([(1, &overrides[0].action), (3, &overrides[2].action), (4, &overrides[1].action)])
        );
        assert_eq!(apply_audio(&[], &title.streams), BTreeMap::new());
    }

    #[test]
    fn subtitle_overrides_follow_the_stream_signature_and_ignore_other_kinds() {
        let overrides = vec![
            SubtitleOverrideEntry { signature: eng_pgs(0), action: SubtitleOverride::Keep { role: SubtitleRole::Regular } },
            SubtitleOverrideEntry { signature: eng_pgs(1), action: SubtitleOverride::Keep { role: SubtitleRole::Sdh } },
            SubtitleOverrideEntry { signature: eng_pgs(2), action: SubtitleOverride::Drop },
            // An audio signature in a subtitle entry never lands on the audio stream.
            SubtitleOverrideEntry { signature: eng_ac3_2(0), action: SubtitleOverride::Keep { role: SubtitleRole::Forced } },
        ];
        let title = title_with(&[VIDEO, ENG_AC3_2CH, ENG_PGS, SPA_PGS, ENG_PGS, ENG_PGS]);
        assert_eq!(
            apply_subtitles(&overrides, &title.streams),
            BTreeMap::from([(2, &overrides[0].action), (4, &overrides[1].action), (5, &overrides[2].action)])
        );
    }

    // The pinned logs CHERNOBYL_1 and FIREFLY_2 are read with Task 5's `plan::fixtures::pinned`.

    /// A synthetic Blu-ray scan: each title given by its attributes, with one English stereo audio stream.
    fn blu_ray(titles: &[(u32, &[(u32, &str)])]) -> Disc {
        let streams: &[(u32, &[(u32, &str)])] = &[(0, ENG_AC3_2CH)];
        let rows: Vec<(u32, &[(u32, &str)], &[(u32, &[(u32, &str)])])> =
            titles.iter().map(|&(number, attrs)| (number, attrs, streams)).collect();
        disc(&parse(&scan_text(&[(1, "Blu-ray disc")], &rows))).unwrap()
    }

    fn remembered_with(titles: Vec<RememberedTitle>) -> Remembered {
        Remembered { fingerprint: FIREFLY_1_API.to_string(), tmdb: TmdbIdentity { kind: TmdbKind::Tv, id: 1437 }, titles }
    }

    #[test]
    fn a_remembered_title_matches_by_source_file_segment_map_and_duration_within_2_s() {
        let two_longer: &[(u32, &str)] = &[(9, "0:58:56"), (16, "00801.mpls"), (26, "20")];
        let three_longer: &[(u32, &str)] = &[(9, "0:58:57"), (16, "00801.mpls"), (26, "20")];
        let other_playlist: &[(u32, &str)] = &[(9, "0:58:54"), (16, "00802.mpls"), (26, "20")];
        let other_map: &[(u32, &str)] = &[(9, "0:58:54"), (16, "00801.mpls"), (26, "21")];
        let two_shorter: &[(u32, &str)] = &[(9, "0:58:52"), (16, "00801.mpls"), (26, "20")];
        let scanned = blu_ray(&[(0, two_longer), (1, three_longer), (2, other_playlist), (3, other_map), (4, two_shorter)]);
        let remembered = remembered_title(Some("00801.mpls"), None, "20", 3534, episodes(1, &[1]));
        let matched: Vec<u32> =
            scanned.titles.values().filter(|title| match_title(&remembered, title)).map(|title| title.number).collect();
        assert_eq!(matched, vec![0, 4]);
    }

    #[test]
    fn a_remembered_title_never_matches_another_playlist_with_the_same_map_and_duration() {
        // Chernobyl disc 1 lists episode 1 twice: playlist 00801.mpls (title 1) and its clip 00020.m2ts (title 13), with
        // the same segment map and duration.
        let scanned = pinned(CHERNOBYL_1);
        let (playlist, clip) = (scanned.title(1).unwrap(), scanned.title(13).unwrap());
        assert_eq!((playlist.segment_map.as_str(), playlist.seconds), ("20", Some(3534)));
        assert_eq!((clip.segment_map.as_str(), clip.seconds), ("20", Some(3534)));
        let episode = remembered_title(Some("00801.mpls"), None, "20", 3534, episodes(1, &[1]));
        assert!(match_title(&episode, playlist));
        assert!(!match_title(&episode, clip));
        let remembered =
            remembered_with(vec![episode, remembered_title(Some("00020.m2ts"), None, "20", 3534, TitleRole::Ignored)]);
        assert_eq!(
            apply_titles(&remembered, &scanned),
            BTreeMap::from([(1, &remembered.titles[0].role), (13, &remembered.titles[1].role)])
        );
    }

    #[test]
    fn on_a_dvd_a_remembered_title_matches_by_dvd_title_number_not_segment_map() {
        // Firefly disc 2's titles 0 and 2 share the segment map 1-5,6-9,10-14,15 and last 0:43:51 and 0:43:49, within
        // 2 s of each other: only attribute 24 ("01", "03") tells them apart.
        let scanned = pinned(FIREFLY_2);
        let (shindig, reynolds) = (scanned.title(0).unwrap(), scanned.title(2).unwrap());
        assert_eq!(shindig.segment_map, reynolds.segment_map);
        assert_eq!((shindig.seconds, reynolds.seconds), (Some(2631), Some(2629)));
        let remembered = remembered_title(None, Some("03"), "1-5,6-9,10-14,15", 2629, episodes(1, &[6]));
        assert!(match_title(&remembered, reynolds));
        assert!(!match_title(&remembered, shindig));
        // Attribute 24 compares as a number: "3" is title "03".
        let as_number = remembered_title(None, Some("3"), "1-5,6-9,10-14,15", 2629, episodes(1, &[6]));
        assert!(match_title(&as_number, reynolds));
    }

    #[test]
    fn a_truncated_segment_map_matches_its_untruncated_prefix() {
        let full: &[(u32, &str)] = &[(9, "1:30:00"), (16, "00800.mpls"), (26, "101,102,103,104,105")];
        let other: &[(u32, &str)] = &[(9, "1:30:00"), (16, "00800.mpls"), (26, "101,102,104,105,106")];
        let truncated: &[(u32, &str)] = &[(9, "1:30:00"), (16, "00800.mpls"), (26, "101,102,103,10...")];
        let scanned = blu_ray(&[(0, full), (1, other), (2, truncated)]);
        let remembered_truncated = remembered_title(Some("00800.mpls"), None, "101,102,103,10...", 5400, TitleRole::Feature);
        assert!(match_title(&remembered_truncated, scanned.title(0).unwrap()));
        assert!(!match_title(&remembered_truncated, scanned.title(1).unwrap()));
        let remembered_full = remembered_title(Some("00800.mpls"), None, "101,102,103,104,105", 5400, TitleRole::Feature);
        assert!(match_title(&remembered_full, scanned.title(2).unwrap()));
    }

    #[test]
    fn a_title_without_a_duration_or_a_remembered_title_without_a_source_never_matches() {
        let no_duration: &[(u32, &str)] = &[(16, "00800.mpls"), (26, "1,2,3")];
        let timed: &[(u32, &str)] = &[(9, "1:00:00"), (16, "00800.mpls"), (26, "1,2,3")];
        let scanned = blu_ray(&[(0, no_duration), (1, timed)]);
        assert_eq!(scanned.title(0).unwrap().seconds, None);
        assert!(!match_title(&remembered_title(Some("00800.mpls"), None, "1,2,3", 0, TitleRole::Feature), scanned.title(0).unwrap()));
        assert!(match_title(&remembered_title(Some("00800.mpls"), None, "1,2,3", 3600, TitleRole::Feature), scanned.title(1).unwrap()));
        assert!(!match_title(&remembered_title(None, None, "1,2,3", 3600, TitleRole::Feature), scanned.title(1).unwrap()));
    }

    #[test]
    fn apply_titles_maps_every_matched_scan_title_to_its_role() {
        let scanned = pinned(FIREFLY_2);
        // In another order than the scan's: the match never uses title numbers or the order of the file.
        let remembered = remembered_with(vec![
            remembered_title(None, Some("11"), "1-4,5-9,10-14,15", 2628, episodes(1, &[7])),
            remembered_title(None, Some("01"), "1-5,6-9,10-14,15", 2631, episodes(1, &[4])),
            remembered_title(None, Some("02"), "1-5,6-10,11-14,15", 2559, episodes(1, &[5])),
            remembered_title(None, Some("03"), "1-5,6-9,10-14,15", 2629, episodes(1, &[6])),
            remembered_title(None, Some("05"), "1,2", 7, TitleRole::Ignored),
        ]);
        assert_eq!(
            apply_titles(&remembered, &scanned),
            BTreeMap::from([
                (0, &remembered.titles[1].role),
                (1, &remembered.titles[2].role),
                (2, &remembered.titles[3].role),
                (3, &remembered.titles[4].role),
                (5, &remembered.titles[0].role),
            ])
        );
    }

    #[test]
    fn a_remembered_title_is_used_once() {
        // Two scan titles that are the same playlist with the same map and duration.
        let same: &[(u32, &str)] = &[(9, "1:40:00"), (16, "00800.mpls"), (26, "1,2,3")];
        let scanned = blu_ray(&[(0, same), (1, same)]);
        let one = remembered_with(vec![remembered_title(Some("00800.mpls"), None, "1,2,3", 6000, TitleRole::Feature)]);
        assert_eq!(apply_titles(&one, &scanned), BTreeMap::from([(0, &one.titles[0].role)]));
        let two = remembered_with(vec![
            remembered_title(Some("00800.mpls"), None, "1,2,3", 6000, TitleRole::Feature),
            remembered_title(Some("00800.mpls"), None, "1,2,3", 6000, TitleRole::Cut { edition: "Extended Cut".to_string() }),
        ]);
        assert_eq!(apply_titles(&two, &scanned), BTreeMap::from([(0, &two.titles[0].role), (1, &two.titles[1].role)]));
    }
}

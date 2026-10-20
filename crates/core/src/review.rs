//! The closed sets of review reasons and notes: design spec 8.8's table of review reasons with 8.6 "Content"'s
//! `commentary_detected`, and the notes of 8.6 step 6,
//! 8.7 "Text" and "Other tracks", 8.9, 7, 6.3 step 6a, 10.2 (ToolMissing), D9 and RP7. Every rule of phase 1 returns
//! these enums, never strings: a result's `reviews` field holds `ReviewReason`s sorted by the table's order and
//! unique (`sorted_unique`), its `notes` field holds `Noted` entries in the order the rule raised them. The serde and
//! `as_str` spellings are the spec's snake_case strings; four notes the spec describes only in words take this plan's
//! spelling (`possible_audio_description`, `lossy_before_lossless`, `direct_mode_fallback`, `label_rule_unavailable`),
//! which the owner confirmed (ruling OR8, 2026-10-17); the rulings plan adds a thirteenth, `unlisted_label_cut` (OR16).
//! Written from the spec (RP20); used by Tasks 4 to 18.

/// A reason that holds a job for review: spec 8.8's table in its order, with 8.6 "Content"'s commentary_detected placed
/// after audio_reference_unclear (the derived order, which `sorted_unique` sorts by). The serde and `as_str` spelling is
/// the spec's snake_case string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReason {
    /// Spec 8.1 step 6: nothing identified the work (identity confidence 0).
    IdentityUnknown,
    /// Spec 8.1 step 5: several plausible TMDb results or a failed runtime check (0.50), or a TMDb-only identification
    /// with `tmdb_only_autofile` off (capped at 0.84).
    IdentityAmbiguous,
    /// Spec 8.3 rules 3.4, 3.6 and 4: the feature cannot be told from its rivals, or its runtime is far from TMDb's.
    TitleAmbiguous,
    /// Spec 8.3 rule 3.7: the rival titles are largely different content.
    MultipleFeatures,
    /// Spec 8.3 and 8.4: a cut without an edition name.
    UnnamedCut,
    /// Spec 8.5: a TV disc no database maps to episodes.
    TvUnmapped,
    /// Spec 8.6 step 6: the original language is wanted but no stream carries it.
    OriginalLanguageMissing,
    /// Spec 8.6 step 6: no stream in a wanted language and none without a language.
    NoWantedAudio,
    /// Spec 8.6 "Content" and D21: the content rule's veto (the programme reference is not the first track of its
    /// language, and that first track is lossless or has more than two channels).
    AudioReferenceUnclear,
    /// Spec 8.6 "Content", RP7 and section 17 row 13's fallback: the content rule found the commentary pattern.
    CommentaryDetected,
    /// Spec 8.7: an alignment in the in-between band, no dialogue group, more than two dialogue tracks, or text that
    /// disagrees with the counts and timing.
    SubtitleAmbiguous,
    /// Spec 7 and 8.9: the final path is taken by a file this job did not create.
    PathCollision,
    /// Spec 6.3 step 6: the whole-disc copy completed with hash failures (MSG:5079).
    CopyHashErrors,
    /// Spec 6.2 step 7: a title failed verification a second time.
    VerifyFailed,
    /// Spec 6.2 step 7: a saved title logged read errors (MSG:2003).
    ReadErrors,
}

impl ReviewReason {
    /// Every review reason: spec 8.8's table in its order, with commentary_detected (8.6 "Content") after
    /// audio_reference_unclear. Spec 8.8.
    pub const ALL: [ReviewReason; 15] = [
        ReviewReason::IdentityUnknown,
        ReviewReason::IdentityAmbiguous,
        ReviewReason::TitleAmbiguous,
        ReviewReason::MultipleFeatures,
        ReviewReason::UnnamedCut,
        ReviewReason::TvUnmapped,
        ReviewReason::OriginalLanguageMissing,
        ReviewReason::NoWantedAudio,
        ReviewReason::AudioReferenceUnclear,
        ReviewReason::CommentaryDetected,
        ReviewReason::SubtitleAmbiguous,
        ReviewReason::PathCollision,
        ReviewReason::CopyHashErrors,
        ReviewReason::VerifyFailed,
        ReviewReason::ReadErrors,
    ];

    /// The spec's snake_case spelling, the same string serde writes. Spec 8.8.
    pub fn as_str(self) -> &'static str {
        match self {
            ReviewReason::IdentityUnknown => "identity_unknown",
            ReviewReason::IdentityAmbiguous => "identity_ambiguous",
            ReviewReason::TitleAmbiguous => "title_ambiguous",
            ReviewReason::MultipleFeatures => "multiple_features",
            ReviewReason::UnnamedCut => "unnamed_cut",
            ReviewReason::TvUnmapped => "tv_unmapped",
            ReviewReason::OriginalLanguageMissing => "original_language_missing",
            ReviewReason::NoWantedAudio => "no_wanted_audio",
            ReviewReason::AudioReferenceUnclear => "audio_reference_unclear",
            ReviewReason::CommentaryDetected => "commentary_detected",
            ReviewReason::SubtitleAmbiguous => "subtitle_ambiguous",
            ReviewReason::PathCollision => "path_collision",
            ReviewReason::CopyHashErrors => "copy_hash_errors",
            ReviewReason::VerifyFailed => "verify_failed",
            ReviewReason::ReadErrors => "read_errors",
        }
    }
}

impl std::str::FromStr for ReviewReason {
    type Err = String;

    /// Only the exact spellings of `as_str` are accepted.
    fn from_str(text: &str) -> Result<ReviewReason, String> {
        ReviewReason::ALL
            .into_iter()
            .find(|reason| reason.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not a review reason (spec 8.8)"))
    }
}

/// A note on a job: recorded with the decision, never holding the job (spec 8.6 step 6, 8.7, 8.9, 7, 6.3 step 6a,
/// 10.2, D9, RP7, and 8.3 rule 1 under OR5). The serde and `as_str` spelling is the spec's snake_case string, or this
/// plan's for the four the spec describes in words and for `unlisted_label_cut` (OR16).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Note {
    /// Spec 8.6 step 6: a kept track that is lossy, has two channels or fewer and is not the first of its language;
    /// names the track.
    PossibleCommentary,
    /// RP7: the content rule's ad-pattern and note-ad verdicts (a possible audio description).
    PossibleAudioDescription,
    /// Spec 8.6 step 6: the first track is lossy while a lossless track of the same language with fewer channels exists.
    LosslessOriginalAvailable,
    /// Spec 8.6 step 6: no English stream, but the original language is present.
    NoEnglishAudio,
    /// D9's first default: a lossy track with more channels was placed before a lossless one; names the lossy track.
    LossyBeforeLossless,
    /// Spec 8.7 "Other tracks": a track that is not full and has 150 events or more was dropped; names the track.
    PartialSubtitleDropped,
    /// Spec 8.7 "Text": the text's verdict differs from the counts and timing; names the track.
    SubtitleTextDisagrees,
    /// Spec 8.9: the Jellyfin 10 compatibility tag was left out (a year-like TMDb ID without an IMDb or TVDB ID).
    CompatTagOmitted,
    /// Spec 7 "Overwriting": a recorded file that no longer matches its record was left alone.
    OldFileKept,
    /// Spec 7 "Tuning evidence": tuning_root was unreachable or short of space; the evidence is written later.
    TuningEvidencePending,
    /// Spec 6.3 step 6a and its second read failure: the job switched to Direct mode ("a note records the switch").
    DirectModeFallback,
    /// Spec 8.3 rule 2 and 10.2 ToolMissing: MakeMKV did not report Java, so the label rule does not apply ("a note
    /// records why").
    LabelRuleUnavailable,
    /// Spec 8.3 rule 1 under ruling OR5 (2026-10-17): a Java label names a cut TheDiscDB does not list as a main movie.
    /// The detail states only that fact, naming the title and its label as the scan wrote it ("title 54's label
    /// FPL_MainFeature_R names a cut TheDiscDB does not list as a main movie", ruling H3).
    UnlistedLabelCut,
}

impl Note {
    /// Every note, in the order of the enum. Spec 8.6, 8.7, 8.9, 7, 8.3 rule 1 (OR5, OR16).
    pub const ALL: [Note; 13] = [
        Note::PossibleCommentary,
        Note::PossibleAudioDescription,
        Note::LosslessOriginalAvailable,
        Note::NoEnglishAudio,
        Note::LossyBeforeLossless,
        Note::PartialSubtitleDropped,
        Note::SubtitleTextDisagrees,
        Note::CompatTagOmitted,
        Note::OldFileKept,
        Note::TuningEvidencePending,
        Note::DirectModeFallback,
        Note::LabelRuleUnavailable,
        Note::UnlistedLabelCut,
    ];

    /// The snake_case spelling, the same string serde writes. Spec 8.6, 8.7, 8.9, 7.
    pub fn as_str(self) -> &'static str {
        match self {
            Note::PossibleCommentary => "possible_commentary",
            Note::PossibleAudioDescription => "possible_audio_description",
            Note::LosslessOriginalAvailable => "lossless_original_available",
            Note::NoEnglishAudio => "no_english_audio",
            Note::LossyBeforeLossless => "lossy_before_lossless",
            Note::PartialSubtitleDropped => "partial_subtitle_dropped",
            Note::SubtitleTextDisagrees => "subtitle_text_disagrees",
            Note::CompatTagOmitted => "compat_tag_omitted",
            Note::OldFileKept => "old_file_kept",
            Note::TuningEvidencePending => "tuning_evidence_pending",
            Note::DirectModeFallback => "direct_mode_fallback",
            Note::LabelRuleUnavailable => "label_rule_unavailable",
            Note::UnlistedLabelCut => "unlisted_label_cut",
        }
    }
}

impl std::str::FromStr for Note {
    type Err = String;

    /// Only the exact spellings of `as_str` are accepted.
    fn from_str(text: &str) -> Result<Note, String> {
        Note::ALL
            .into_iter()
            .find(|note| note.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not a note (spec 8.6, 8.7, 8.9, 7)"))
    }
}

/// A note as a rule raised it: which note, the scan stream it names (PossibleCommentary, PartialSubtitleDropped,
/// LossyBeforeLossless, SubtitleTextDisagrees; None for a note on the title or the job) and one plain sentence of
/// detail. Spec 8.6 step 6, 8.7, 8.9, 7.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Noted {
    /// Which note.
    pub note: Note,
    /// The scan's stream number the note names, or None.
    pub stream: Option<u32>,
    /// One plain sentence of detail.
    pub detail: String,
}

/// `reasons` sorted by the enum's order (spec 8.8's table, with commentary_detected after audio_reference_unclear) and
/// without repeats: the form of every `reviews` field.
pub fn sorted_unique(reasons: Vec<ReviewReason>) -> Vec<ReviewReason> {
    let mut reasons = reasons;
    reasons.sort();
    reasons.dedup();
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// One review reason: its spec spelling through as_str, serde both ways and FromStr, and its place in ALL.
    fn reason_round_trips(reason: ReviewReason, text: &str) {
        assert_eq!(reason.as_str(), text);
        assert_eq!(serde_json::to_value(reason).unwrap(), Value::String(text.to_string()));
        assert_eq!(serde_json::from_value::<ReviewReason>(Value::String(text.to_string())).unwrap(), reason);
        assert_eq!(text.parse::<ReviewReason>(), Ok(reason));
        assert!(ReviewReason::ALL.contains(&reason), "{text} is missing from ALL");
    }

    /// One note, checked the same four ways.
    fn note_round_trips(note: Note, text: &str) {
        assert_eq!(note.as_str(), text);
        assert_eq!(serde_json::to_value(note).unwrap(), Value::String(text.to_string()));
        assert_eq!(serde_json::from_value::<Note>(Value::String(text.to_string())).unwrap(), note);
        assert_eq!(text.parse::<Note>(), Ok(note));
        assert!(Note::ALL.contains(&note), "{text} is missing from ALL");
    }

    /// One #[test] function per row: `name: value => "spelling",`, each calling `$check(value, "spelling")`.
    macro_rules! round_trips {
        ($check:ident { $($name:ident: $value:expr => $text:literal,)* }) => {
            $(
                #[test]
                fn $name() {
                    $check($value, $text);
                }
            )*
        };
    }

    round_trips!(reason_round_trips {
        review_reason_identity_unknown_round_trips: ReviewReason::IdentityUnknown => "identity_unknown",
        review_reason_identity_ambiguous_round_trips: ReviewReason::IdentityAmbiguous => "identity_ambiguous",
        review_reason_title_ambiguous_round_trips: ReviewReason::TitleAmbiguous => "title_ambiguous",
        review_reason_multiple_features_round_trips: ReviewReason::MultipleFeatures => "multiple_features",
        review_reason_unnamed_cut_round_trips: ReviewReason::UnnamedCut => "unnamed_cut",
        review_reason_tv_unmapped_round_trips: ReviewReason::TvUnmapped => "tv_unmapped",
        review_reason_original_language_missing_round_trips: ReviewReason::OriginalLanguageMissing => "original_language_missing",
        review_reason_no_wanted_audio_round_trips: ReviewReason::NoWantedAudio => "no_wanted_audio",
        review_reason_audio_reference_unclear_round_trips: ReviewReason::AudioReferenceUnclear => "audio_reference_unclear",
        review_reason_commentary_detected_round_trips: ReviewReason::CommentaryDetected => "commentary_detected",
        review_reason_subtitle_ambiguous_round_trips: ReviewReason::SubtitleAmbiguous => "subtitle_ambiguous",
        review_reason_path_collision_round_trips: ReviewReason::PathCollision => "path_collision",
        review_reason_copy_hash_errors_round_trips: ReviewReason::CopyHashErrors => "copy_hash_errors",
        review_reason_verify_failed_round_trips: ReviewReason::VerifyFailed => "verify_failed",
        review_reason_read_errors_round_trips: ReviewReason::ReadErrors => "read_errors",
    });

    round_trips!(note_round_trips {
        note_possible_commentary_round_trips: Note::PossibleCommentary => "possible_commentary",
        note_possible_audio_description_round_trips: Note::PossibleAudioDescription => "possible_audio_description",
        note_lossless_original_available_round_trips: Note::LosslessOriginalAvailable => "lossless_original_available",
        note_no_english_audio_round_trips: Note::NoEnglishAudio => "no_english_audio",
        note_lossy_before_lossless_round_trips: Note::LossyBeforeLossless => "lossy_before_lossless",
        note_partial_subtitle_dropped_round_trips: Note::PartialSubtitleDropped => "partial_subtitle_dropped",
        note_subtitle_text_disagrees_round_trips: Note::SubtitleTextDisagrees => "subtitle_text_disagrees",
        note_compat_tag_omitted_round_trips: Note::CompatTagOmitted => "compat_tag_omitted",
        note_old_file_kept_round_trips: Note::OldFileKept => "old_file_kept",
        note_tuning_evidence_pending_round_trips: Note::TuningEvidencePending => "tuning_evidence_pending",
        note_direct_mode_fallback_round_trips: Note::DirectModeFallback => "direct_mode_fallback",
        note_label_rule_unavailable_round_trips: Note::LabelRuleUnavailable => "label_rule_unavailable",
        note_unlisted_label_cut_round_trips: Note::UnlistedLabelCut => "unlisted_label_cut",
    });

    #[test]
    fn all_review_reasons_are_8_8s_fourteen_and_8_6s_commentary_detected_in_order() {
        let spellings: Vec<&str> = ReviewReason::ALL.iter().map(|reason| reason.as_str()).collect();
        assert_eq!(
            spellings,
            [
                "identity_unknown",
                "identity_ambiguous",
                "title_ambiguous",
                "multiple_features",
                "unnamed_cut",
                "tv_unmapped",
                "original_language_missing",
                "no_wanted_audio",
                "audio_reference_unclear",
                "commentary_detected",
                "subtitle_ambiguous",
                "path_collision",
                "copy_hash_errors",
                "verify_failed",
                "read_errors",
            ]
        );
        assert!(ReviewReason::ALL.windows(2).all(|pair| pair[0] < pair[1]), "ALL is in the enum's order");
    }

    #[test]
    fn a_string_outside_the_review_reasons_is_refused() {
        assert_eq!(
            "Identity_Unknown".parse::<ReviewReason>(),
            Err("\"Identity_Unknown\" is not a review reason (spec 8.8)".to_string())
        );
        assert_eq!("".parse::<ReviewReason>(), Err("\"\" is not a review reason (spec 8.8)".to_string()));
        assert!(serde_json::from_value::<ReviewReason>(json!("possible_commentary")).is_err());
        assert!(serde_json::from_value::<ReviewReason>(json!("identity unknown")).is_err());
    }

    #[test]
    fn all_notes_are_the_thirteen_in_spec_order() {
        let spellings: Vec<&str> = Note::ALL.iter().map(|note| note.as_str()).collect();
        assert_eq!(
            spellings,
            [
                "possible_commentary",
                "possible_audio_description",
                "lossless_original_available",
                "no_english_audio",
                "lossy_before_lossless",
                "partial_subtitle_dropped",
                "subtitle_text_disagrees",
                "compat_tag_omitted",
                "old_file_kept",
                "tuning_evidence_pending",
                "direct_mode_fallback",
                "label_rule_unavailable",
                "unlisted_label_cut",
            ]
        );
        assert!(Note::ALL.windows(2).all(|pair| pair[0] < pair[1]), "ALL is in the enum's order");
    }

    #[test]
    fn a_string_outside_the_notes_is_refused() {
        assert_eq!(
            "commentary_detected".parse::<Note>(),
            Err("\"commentary_detected\" is not a note (spec 8.6, 8.7, 8.9, 7)".to_string())
        );
        assert_eq!("Possible_Commentary".parse::<Note>(), Err("\"Possible_Commentary\" is not a note (spec 8.6, 8.7, 8.9, 7)".to_string()));
        assert!(serde_json::from_value::<Note>(json!("read_errors")).is_err());
    }

    #[test]
    fn a_noted_note_writes_its_note_stream_and_detail_in_that_order() {
        let named = Noted {
            note: Note::PossibleCommentary,
            stream: Some(3),
            detail: "stream 3 is lossy with 2 channels and not the first eng track".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&named).unwrap(),
            r#"{"note":"possible_commentary","stream":3,"detail":"stream 3 is lossy with 2 channels and not the first eng track"}"#
        );
        let unnamed = Noted {
            note: Note::NoEnglishAudio,
            stream: None,
            detail: "no English stream; the original language spa is present".to_string(),
        };
        let text = serde_json::to_string(&unnamed).unwrap();
        assert_eq!(text, r#"{"note":"no_english_audio","stream":null,"detail":"no English stream; the original language spa is present"}"#);
        assert_eq!(serde_json::from_str::<Noted>(&text).unwrap(), unnamed);
    }

    #[test]
    fn sorted_unique_sorts_by_the_table_order_and_drops_repeats() {
        assert_eq!(
            sorted_unique(vec![
                ReviewReason::ReadErrors,
                ReviewReason::IdentityUnknown,
                ReviewReason::SubtitleAmbiguous,
                ReviewReason::IdentityUnknown,
                ReviewReason::NoWantedAudio,
                ReviewReason::ReadErrors,
            ]),
            vec![
                ReviewReason::IdentityUnknown,
                ReviewReason::NoWantedAudio,
                ReviewReason::SubtitleAmbiguous,
                ReviewReason::ReadErrors,
            ]
        );
        assert_eq!(sorted_unique(Vec::new()), Vec::<ReviewReason>::new());
        assert_eq!(sorted_unique(vec![ReviewReason::PathCollision]), vec![ReviewReason::PathCollision]);
    }
}

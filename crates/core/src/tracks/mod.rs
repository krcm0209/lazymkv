//! The track rules of design spec 8.6 (audio), 8.7 (English subtitles) and 8.10 (track metadata in the final file).
//! Task 1 creates the module with `SubtitleRole`, the roles a kept English subtitle track holds, which Task 6's
//! review overrides need before the subtitle rule exists; Tasks 8, 9 and 11 add `pub mod audio;`,
//! `pub mod subtitle;` and `pub mod layout;`. Written from the spec (RP20); `SubtitleRole` is used by Tasks 6, 10,
//! 11, 13 and 18.

pub mod audio;

/// The role of a kept English subtitle track (spec 8.7 "Regular and SDH" and "Forced"). The derived order is 8.10's
/// output order: forced, regular, then SDH (subtitles for the deaf and hard of hearing).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleRole {
    /// The forced track, flagged forced (8.7 "Forced").
    Forced,
    /// The regular dialogue track (8.7 "Regular and SDH").
    Regular,
    /// The hearing-impaired dialogue track, flagged hearing-impaired (8.7 "Regular and SDH").
    Sdh,
}

impl SubtitleRole {
    /// Every role, in 8.10's output order.
    pub const ALL: [SubtitleRole; 3] = [SubtitleRole::Forced, SubtitleRole::Regular, SubtitleRole::Sdh];

    /// The snake_case spelling, the same string serde writes. Spec 8.7, 8.10.
    pub fn as_str(self) -> &'static str {
        match self {
            SubtitleRole::Forced => "forced",
            SubtitleRole::Regular => "regular",
            SubtitleRole::Sdh => "sdh",
        }
    }

    /// 8.7's track names: "English (Forced)", "English", "English (SDH)".
    pub fn name(self) -> &'static str {
        match self {
            SubtitleRole::Forced => "English (Forced)",
            SubtitleRole::Regular => "English",
            SubtitleRole::Sdh => "English (SDH)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// One role: its serde and as_str spelling, its 8.7 track name, and its place in ALL.
    fn role_is(role: SubtitleRole, text: &str, name: &str) {
        assert_eq!(role.as_str(), text);
        assert_eq!(role.name(), name);
        assert_eq!(serde_json::to_value(role).unwrap(), Value::String(text.to_string()));
        assert_eq!(serde_json::from_value::<SubtitleRole>(Value::String(text.to_string())).unwrap(), role);
        assert!(SubtitleRole::ALL.contains(&role), "{text} is missing from ALL");
    }

    #[test]
    fn the_forced_role_is_spelled_forced_and_named_english_forced() {
        role_is(SubtitleRole::Forced, "forced", "English (Forced)");
    }

    #[test]
    fn the_regular_role_is_spelled_regular_and_named_english() {
        role_is(SubtitleRole::Regular, "regular", "English");
    }

    #[test]
    fn the_sdh_role_is_spelled_sdh_and_named_english_sdh() {
        role_is(SubtitleRole::Sdh, "sdh", "English (SDH)");
    }

    #[test]
    fn the_roles_sort_in_8_10s_output_order_forced_regular_sdh() {
        assert_eq!(SubtitleRole::ALL, [SubtitleRole::Forced, SubtitleRole::Regular, SubtitleRole::Sdh]);
        let mut roles = vec![SubtitleRole::Sdh, SubtitleRole::Forced, SubtitleRole::Regular];
        roles.sort();
        assert_eq!(roles, SubtitleRole::ALL);
        assert!(serde_json::from_value::<SubtitleRole>(Value::String("SDH".to_string())).is_err());
    }
}

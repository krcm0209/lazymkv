//! The OCR time cap of design part 10.8 (P26): three times P26's bar of about 676 ms per event, so 2.03 s per event with a 60 s floor for start-up. A fixed value of spec 14, never a setting; the only short caps are the test seams of T5-R10 and T5-R58.

/// Seconds of cap per event: three times P26's bar of about 676 ms (design part 10.8, spec 14). A fixed value, no
/// setting. Def: T5-6. Use: T5-10, T5-15.
pub const CAP_PER_EVENT_S: f64 = 2.03;

/// The cap's floor in seconds, for start-up (design part 10.8, spec 14). A fixed value, no setting. Def: T5-6.
/// Use: T5-10, T5-15.
pub const CAP_FLOOR_S: f64 = 60.0;

/// The time cap in seconds, `max(60, 2.03 x events)` (design part 10.8, spec 14); the engine and
/// `lazymkv-dev ocr-trial read` always compute the cap with this function (T5-R10). Def: T5-6. Use: T5-10, T5-15.
pub fn time_cap_s(events: usize) -> f64 {
    CAP_FLOOR_S.max(CAP_PER_EVENT_S * events as f64)
}

/// The reason text of a run stopped by the cap (design part 10.8, spec 14; T5-R9): `read` is the whole records of
/// the killed run (`evidence::whole_records`, T5-R9). Def: T5-6. Use: T5-10, T5-15.
pub fn cap_reason(cap_s: f64, read: usize, events: usize) -> String {
    format!("time cap of {cap_s:.1} s reached after {read} of {events} events")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_is_60_s_up_to_29_events_and_1800_61_s_for_887() {
        assert_eq!((CAP_PER_EVENT_S, CAP_FLOOR_S), (2.03, 60.0));
        for events in [0, 1, 29] {
            assert_eq!(time_cap_s(events), 60.0, "{events}");
        }
        assert!((time_cap_s(30) - 60.9).abs() < 1e-9);
        assert!((time_cap_s(887) - 1800.61).abs() < 1e-9);
    }

    #[test]
    fn cap_reason_prints_one_decimal() {
        assert_eq!(cap_reason(time_cap_s(887), 412, 887), "time cap of 1800.6 s reached after 412 of 887 events");
        assert_eq!(cap_reason(time_cap_s(20), 12, 20), "time cap of 60.0 s reached after 12 of 20 events");
        assert_eq!(cap_reason(2.0, 3, 10), "time cap of 2.0 s reached after 3 of 10 events");
        assert_eq!(cap_reason(8.0, 3, 10), "time cap of 8.0 s reached after 3 of 10 events", "2 s times ci_factor() 4 (T5-R58)");
    }

    #[test]
    fn the_cap_is_a_fixed_value_and_no_setting() {
        let settings = include_str!("../../settings.rs");
        for word in ["time_cap", "ocr_cap", "cap_per_event", "cap_floor"] {
            assert!(!settings.contains(word), "settings.rs names {word} (design 10.8, spec 14, R3-worker-21)");
        }
    }
}

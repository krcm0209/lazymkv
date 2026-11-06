//! TMDb's six-month limit on a kept copy (design part 8.4, R-B4; Ruling T4-R7): the calendar-month arithmetic over
//! `yyyy-MM-dd` dates, the rule that says a copy is stale, and the read of a copy's top-level `fetched_at`. Pure: the
//! callers read the files and pass today's local date. No date-time crate (design part 4.6).

use serde_json::Value;

/// The months a TMDb copy may be kept before it is fetched again (design part 8.4, R-B4). Def: T4-8. Use: T4-9,
/// T4-10, T4-11.
pub const STALE_MONTHS: u32 = 6;

/// The job-folder files that hold TMDb copies, in the order they are checked (design part 8.4). Def: T4-8.
/// Use: T4-9, T4-10, T4-11.
pub const TMDB_COPIES: [&str; 3] = ["tmdb.json", "tmdb-record.json", "tmdb-search.json"];

/// Whether `year` is a Gregorian leap year.
fn leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// The last day of `month` (1 to 12) in `year`.
fn last_day(year: u64, month: u64) -> u64 {
    match month {
        2 if leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The digits of `text` as a number, when every byte is an ASCII digit.
fn digits(text: &[u8]) -> Option<u64> {
    text.iter().try_fold(0u64, |number, byte| byte.is_ascii_digit().then(|| number * 10 + u64::from(byte - b'0')))
}

/// A strict `yyyy-MM-dd` read: ten characters, digits and two dashes, a month of 1 to 12, a day of 1 to the month's
/// last day.
fn read_date(date: &str) -> Option<(u64, u64, u64)> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let (year, month, day) = (digits(&bytes[0..4])?, digits(&bytes[5..7])?, digits(&bytes[8..10])?);
    if !(1..=12).contains(&month) || day < 1 || day > last_day(year, month) {
        return None;
    }
    Some((year, month, day))
}

/// `date` moved `months` calendar months later, the day clamped to the new month's last day (`2026-08-31` and six
/// months give `2027-02-28`), formatted `yyyy-MM-dd`. None when `date` is not a real `yyyy-MM-dd` date. Design part
/// 8.4 (R-B4). Pure. Def: T4-8. Use: T4-9, T4-10, T4-11.
pub fn add_months(date: &str, months: u32) -> Option<String> {
    let (year, month, day) = read_date(date)?;
    let index = year * 12 + month - 1 + u64::from(months);
    let (year, month) = (index / 12, index % 12 + 1);
    let day = day.min(last_day(year, month));
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// The proleptic Gregorian (year, month, day) of a count of seconds since 1970-01-01 (UTC), by the days-to-civil
/// algorithm. Pure. Def: T4-19. Use: T4-19, dev's `civil_time`.
pub fn civil_date(unix_secs: u64) -> (i64, u32, u32) {
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month as u32, day as u32)
}

/// Whether a copy fetched on `fetched_at` is past TMDb's six-month limit on `today` (design part 8.4, R-B4; Ruling
/// T4-R7): never when `today` is not a real `yyyy-MM-dd` date; always when `fetched_at` is None or not a real date;
/// else when `today` is after `fetched_at` plus `STALE_MONTHS` months. A date after today is fresh. Pure. Def: T4-8.
/// Use: T4-9, T4-10, T4-11.
pub fn is_stale(fetched_at: Option<&str>, today: &str) -> bool {
    if add_months(today, 0).is_none() {
        return false;
    }
    match fetched_at.and_then(|date| add_months(date, STALE_MONTHS)) {
        Some(limit) => today > limit.as_str(),
        None => true,
    }
}

/// The top-level `fetched_at` of a saved copy's text (design part 8.4), after any leading byte order mark, when the
/// text is a JSON object whose `fetched_at` is a string; None otherwise. Pure. Def: T4-8. Use: T4-9, T4-10, T4-11.
pub fn fetched_at(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)).ok()?;
    value.as_object()?.get("fetched_at")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_stale_reads_calendar_months_clamps_the_day_and_reads_a_future_date_as_fresh() {
        assert!(!is_stale(Some("2026-05-04"), "2026-11-04"));
        assert!(is_stale(Some("2026-05-03"), "2026-11-04"));
        assert!(is_stale(Some("2026-08-31"), "2027-03-01"));
        assert!(!is_stale(Some("2026-08-31"), "2027-02-28"));
        assert!(!is_stale(Some("2027-01-01"), "2026-11-04"), "a date after today is fresh");
        assert!(is_stale(None, "2026-11-04"));
        assert!(is_stale(Some("2026-13-01"), "2026-11-04"));
        for today in ["2026-11-4", "", "2026-02-30", "yesterday"] {
            assert!(!is_stale(None, today), "{today}");
            assert!(!is_stale(Some("2020-01-01"), today), "{today}");
        }
    }

    #[test]
    fn add_months_clamps_to_the_last_day_and_leap_years() {
        assert_eq!(add_months("2027-08-31", 6).as_deref(), Some("2028-02-29"));
        assert_eq!(add_months("2026-08-31", 6).as_deref(), Some("2027-02-28"));
        assert_eq!(add_months("2099-08-31", 6).as_deref(), Some("2100-02-28"));
        assert_eq!(add_months("2026-11-04", 2).as_deref(), Some("2027-01-04"));
        assert_eq!(add_months("2026-01-31", 0).as_deref(), Some("2026-01-31"));
        for bad in ["2026-02-29", "2026-04-31", "2026-00-10", "2026-1-01", "26-01-01", "2026/01/01", "2026-01-01x"] {
            assert_eq!(add_months(bad, 6), None, "{bad}");
        }
        assert_eq!((STALE_MONTHS, TMDB_COPIES), (6, ["tmdb.json", "tmdb-record.json", "tmdb-search.json"]));
    }

    #[test]
    fn fetched_at_reads_the_top_level_date_only() {
        assert_eq!(fetched_at(r#"{"id": 1, "fetched_at": "2026-01-01"}"#).as_deref(), Some("2026-01-01"));
        assert_eq!(fetched_at("\u{feff}{\"fetched_at\": \"2026-01-02\"}").as_deref(), Some("2026-01-02"));
        for text in [r#"{"results": [{"fetched_at": "2026-01-01"}]}"#, r#"{"fetched_at": 20260101}"#, "[]", "not json", ""] {
            assert_eq!(fetched_at(text), None, "{text}");
        }
    }
}

//! Phase 0's local-time text (design part 4.6, R23.7), from Unix milliseconds and an offset from Coordinated
//! Universal Time (UTC), with no date-time crate.

/// Milliseconds in a day.
const DAY_MS: i128 = 86_400_000;

/// `unix_ms` as `yyyy-MM-dd HH:mm:ss.fff` at `offset_minutes` ahead of UTC (negative: behind), in the proleptic
/// Gregorian calendar (design part 4.6, R23.7). Def: T3-7. Use: T3-7.
pub fn format_local_time(unix_ms: u64, offset_minutes: i32) -> String {
    // i128 holds every u64 instant moved by every i32 offset, so nothing here overflows.
    let local_ms = i128::from(unix_ms) + i128::from(offset_minutes) * 60_000;
    let (days, of_day) = (local_ms.div_euclid(DAY_MS), local_ms.rem_euclid(DAY_MS));
    let (year, month, day) = civil_from_days(days);
    let (hour, minute) = (of_day / 3_600_000, of_day / 60_000 % 60);
    let (second, milli) = (of_day / 1_000 % 60, of_day % 1_000);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{milli:03}")
}

/// The proleptic Gregorian (year, month, day) of `days` days after 1970-01-01, by Howard Hinnant's civil-from-days
/// over 400-year eras of 146,097 days each, counted from 0000-03-01 so that a leap day ends its year.
fn civil_from_days(days: i128) -> (i128, i128, i128) {
    let z = days + 719_468; // days from 0000-03-01
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097); // 0 to 146,096
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // 0 to 399
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // 0 to 365, from 1 March
    let shifted_month = (5 * day_of_year + 2) / 153; // 0 is March, 11 is February
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    let year = year_of_era + era * 400 + i128::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::format_local_time;

    #[test]
    fn the_epoch_formats_with_three_digit_milliseconds() {
        assert_eq!(format_local_time(0, 0), "1970-01-01 00:00:00.000");
    }

    #[test]
    fn a_leap_day_and_a_negative_offset_roll_back_a_day() {
        // 2024-03-01 00:59:59.999 UTC, read one hour behind UTC.
        assert_eq!(format_local_time(1_709_254_799_999, -60), "2024-02-29 23:59:59.999");
    }

    #[test]
    fn milliseconds_are_padded() {
        assert_eq!(format_local_time(7, 0), "1970-01-01 00:00:00.007");
    }

    #[test]
    fn the_century_rules_make_2000_a_leap_year_and_2100_not() {
        assert_eq!(format_local_time(951_782_400_000, 0), "2000-02-29 00:00:00.000");
        assert_eq!(format_local_time(4_107_542_399_999, 0), "2100-02-28 23:59:59.999");
        assert_eq!(format_local_time(4_107_542_400_000, 0), "2100-03-01 00:00:00.000");
    }

    #[test]
    fn a_negative_offset_at_the_epoch_gives_1969() {
        assert_eq!(format_local_time(0, -60), "1969-12-31 23:00:00.000");
    }
}

//! The parts of the `Ejector` that need no Windows call (design parts 4.2 and 5.6 step 8, where the disc phase ends by
//! ejecting the tray): the letter of a drive id and phase 0's eject report. `win::eject` makes the Windows calls and
//! runs the four steps whose results the report names. Compiled and tested on every system.

/// The letter of a drive id in MakeMKV's `D:` form, the form the `DriveWatcher` reports and the job record keeps: an
/// ASCII upper-case letter and a colon, nothing else (design part 5.4; Ruling W8). Err `not a drive id: <drive>` for
/// any other text. Def: T3-15. Use: T3-15.
pub fn drive_letter(drive: &str) -> Result<char, String> {
    let mut characters = drive.chars();
    match (characters.next(), characters.next(), characters.next()) {
        (Some(letter), Some(':'), None) if letter.is_ascii_uppercase() => Ok(letter),
        _ => Err(format!("not a drive id: {drive}")),
    }
}

/// Phase 0's eject report (`crates/capture/src/eject.rs`, experiment 7): each step's name and `ok` or `error <code>`,
/// joined by ", ", in the order given (design part 5.6 step 8). Def: T3-15. Use: T3-15.
pub fn eject_report(steps: &[(&str, Result<(), u32>)]) -> String {
    steps
        .iter()
        .map(|(name, result)| match result {
            Ok(()) => format!("{name}=ok"),
            Err(code) => format!("{name}=error {code}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_upper_case_letter_and_a_colon_is_a_drive_id() {
        assert_eq!(drive_letter("D:"), Ok('D'));
        assert_eq!(drive_letter("Z:"), Ok('Z'));
        for drive in ["d:", r"D:\", "DD:", "D", "", "1:", ":D", "tok0", "É:"] {
            assert_eq!(drive_letter(drive), Err(format!("not a drive id: {drive}")), "{drive}");
        }
    }

    #[test]
    fn the_report_names_each_step_in_order_with_its_error_code() {
        let steps = [("lock", Ok(())), ("dismount", Err(5)), ("allow-removal", Ok(())), ("eject", Ok(()))];
        assert_eq!(eject_report(&steps), "lock=ok, dismount=error 5, allow-removal=ok, eject=ok");
        assert_eq!(eject_report(&[("lock", Err(21)), ("eject", Err(1))]), "lock=error 21, eject=error 1");
    }
}

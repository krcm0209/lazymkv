//! The pure security rules of design parts 3.2 to 3.5 and P6: the security descriptors of the mutex, the pipe and the
//! data folder, the verdict on the token at the other end of the pipe, the production names and the tests' own names.
//! Compiled and tested on every system; `platform::win` applies them.

use std::sync::atomic::{AtomicU32, Ordering};

/// The SID (security identifier) of the LocalSystem account (design part 3.5). Def: T3-2. Use: T3-2, T3-11.
pub const SYSTEM_SID: &str = "S-1-5-18";
/// The SID of the built-in Administrators group (design parts 3.3, 3.5). Def: T3-2. Use: T3-2, T3-3.
pub const ADMINISTRATORS_SID: &str = "S-1-5-32-544";
/// The SID of the high mandatory integrity level, an elevated token's level (design part 3.4). Def: T3-2.
/// Use: T3-2, T3-11.
pub const HIGH_INTEGRITY_SID: &str = "S-1-16-12288";
/// The engine mutex's security descriptor in SDDL (Security Descriptor Definition Language): owner Administrators,
/// full access for SYSTEM and Administrators only (design part 3.3, Ruling W5). Def: T3-2. Use: T3-2.
pub const MUTEX_SDDL: &str = "O:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)";

/// The pipe's descriptor: read and write for `owner_sid` and SYSTEM only, under a high mandatory label that refuses
/// reads and writes from a lower level (design part 3.4, P6). Def: T3-2. Use: T3-11.
pub fn pipe_sddl(owner_sid: &str) -> String {
    format!("D:P(A;;GRGW;;;{owner_sid})(A;;GRGW;;;SY)S:(ML;;NWNR;;;HI)")
}

/// The data folder's access list: protected from the parent's, inherited by everything below, full control for
/// SYSTEM and Administrators, read for `reader_sid` (design part 3.5, Ruling W6). Def: T3-2. Use: T3-3, T3-24.
pub fn folder_sddl(reader_sid: &str) -> String {
    format!("D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;{reader_sid})")
}

/// Whether `owner_sid` may own the data folder: Administrators or SYSTEM, compared exactly (design part 3.5).
/// Def: T3-2. Use: T3-3.
pub fn owner_allowed(owner_sid: &str) -> bool {
    owner_sid == ADMINISTRATORS_SID || owner_sid == SYSTEM_SID
}

/// A token's elevation type (design part 3.2): `Default` has no linked token (UAC, User Account Control, off, or a
/// service or Secure Shell (SSH) logon), `Full` is the elevated half of a split token, `Limited` the filtered half.
/// Def: T3-2. Use: T3-2, T3-7.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ElevationType {
    /// A token with no linked token, elevated or not.
    Default,
    /// The elevated half of a split token, whose linked token is the filtered half.
    Full,
    /// The filtered half of a split token, whose linked token is the elevated half.
    Limited,
}

/// What lazymkv reads from a token (design parts 3.2, 3.4): the user's SID string, whether the token is elevated, its
/// elevation type, its integrity level's SID string, and whether it has a linked token (Ruling T3-2-c).
/// Def: T3-2. Use: T3-2, T3-7, T3-11, T3-12, T3-23.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenFacts {
    /// The SID of the token's user, in its string form `S-1-5-...`.
    pub user_sid: String,
    /// Whether the token is elevated.
    pub elevated: bool,
    /// The token's elevation type.
    pub elevation_type: ElevationType,
    /// The SID of the token's mandatory integrity level, in its string form `S-1-16-...`.
    pub integrity_sid: String,
    /// Whether the token has a linked token (Ruling T3-2-c).
    pub linked: bool,
}

/// The verdict on the process at the other end of the pipe (design parts 3.3, 3.4; P6): another user is refused
/// first, then a token that is not elevated. Only `user_sid` and `elevated` are read. Def: T3-2. Use: T3-11, T3-12.
pub fn token_verdict(peer: &TokenFacts, own_user_sid: &str) -> Result<(), &'static str> {
    if peer.user_sid != own_user_sid {
        Err("its user differs from this process's user")
    } else if !peer.elevated {
        Err("its token is not elevated")
    } else {
        Ok(())
    }
}

/// The engine's pipe (design part 3.3, ARCH-16). Only the binaries use it. Def: T3-2. Use: T3-12, T3-23.
pub const PIPE_NAME: &str = r"\\.\pipe\lazymkv";
/// The engine's mutex, in the namespace every logon session shares (design part 3.3, ARCH-16). Only the binaries use
/// it. Def: T3-2. Use: T3-23.
pub const MUTEX_NAME: &str = r"Global\lazymkv-engine";

/// The count `test_name` appends, process-wide, from 0.
static NEXT_TEST_NAME: AtomicU32 = AtomicU32::new(0);

/// A name for one test's pipe, mutex or credential prefix that no other test and no running engine uses (Global
/// Constraints): `lazymkv-test-<tag>-<process id>-<n>`, `n` from a process-wide `AtomicU32` that starts at 0. `tag`
/// is a short lower-case word with no backslash. Def: T3-2. Use: T3-2, T3-4, T3-11 to T3-14, T3-23.
pub fn test_name(tag: &str) -> String {
    let n = NEXT_TEST_NAME.fetch_add(1, Ordering::Relaxed);
    format!("lazymkv-test-{tag}-{}-{n}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "S-1-5-21-1000-2000-3000-1001";
    const OTHER: &str = "S-1-5-21-1000-2000-3000-1002";
    const OTHER_USER: Result<(), &str> = Err("its user differs from this process's user");

    fn peer(user_sid: &str, elevated: bool, elevation_type: ElevationType) -> TokenFacts {
        TokenFacts {
            user_sid: user_sid.to_string(),
            elevated,
            elevation_type,
            // S-1-16-8192 is the medium level a non-elevated process runs at.
            integrity_sid: if elevated { HIGH_INTEGRITY_SID } else { "S-1-16-8192" }.to_string(),
            linked: true,
        }
    }

    #[test]
    fn the_pipe_descriptor_grants_the_owner_and_system_read_and_write_with_a_high_label() {
        assert_eq!(pipe_sddl(OWN), "D:P(A;;GRGW;;;S-1-5-21-1000-2000-3000-1001)(A;;GRGW;;;SY)S:(ML;;NWNR;;;HI)");
    }

    #[test]
    fn the_folder_descriptor_is_protected_and_inherited() {
        assert_eq!(
            folder_sddl(OWN),
            "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FR;;;S-1-5-21-1000-2000-3000-1001)"
        );
    }

    #[test]
    fn only_administrators_and_system_may_own_the_data_folder() {
        assert!(owner_allowed("S-1-5-32-544"));
        assert!(owner_allowed("S-1-5-18"));
        assert!(!owner_allowed(OWN));
        assert!(!owner_allowed("S-1-5-32-545")); // the Users group
        assert!(!owner_allowed(""));
    }

    #[test]
    fn a_peer_of_another_user_is_refused_before_its_elevation_is_read() {
        assert_eq!(token_verdict(&peer(OTHER, false, ElevationType::Limited), OWN), OTHER_USER);
        assert_eq!(token_verdict(&peer(OTHER, true, ElevationType::Full), OWN), OTHER_USER);
        assert_eq!(token_verdict(&peer(SYSTEM_SID, true, ElevationType::Default), OWN), OTHER_USER);
    }

    #[test]
    fn a_peer_of_the_same_user_without_elevation_is_refused() {
        assert_eq!(token_verdict(&peer(OWN, false, ElevationType::Limited), OWN), Err("its token is not elevated"));
    }

    #[test]
    fn an_elevated_peer_of_the_same_user_passes() {
        assert_eq!(token_verdict(&peer(OWN, true, ElevationType::Full), OWN), Ok(()));
        // An SSH session's elevated token has no linked token.
        assert_eq!(token_verdict(&peer(OWN, true, ElevationType::Default), OWN), Ok(()));
    }

    #[test]
    fn test_names_differ_within_a_process_and_carry_the_tag_and_pid() {
        let (first, second) = (test_name("pipe"), test_name("pipe"));
        let prefix = format!("lazymkv-test-pipe-{}-", std::process::id());
        assert_ne!(first, second);
        assert!(first.starts_with(&prefix), "{first}");
        assert!(second.starts_with(&prefix), "{second}");
        assert!(first[prefix.len()..].parse::<u32>().is_ok(), "{first}");
    }

    #[test]
    fn the_production_names_are_the_designs() {
        assert_eq!(PIPE_NAME, r"\\.\pipe\lazymkv");
        assert_eq!(MUTEX_NAME, r"Global\lazymkv-engine");
        assert_eq!(MUTEX_SDDL, "O:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)");
        assert_eq!((SYSTEM_SID, ADMINISTRATORS_SID, HIGH_INTEGRITY_SID), ("S-1-5-18", "S-1-5-32-544", "S-1-16-12288"));
    }
}

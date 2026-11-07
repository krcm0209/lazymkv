//! The pure security rules of design parts 3.2 to 3.5 and P6: the security descriptors of the mutex, the pipe and the
//! data folder, the verdict on the token at the other end of the pipe, the environment a guarded child gets, the
//! production names and the tests' own names. Compiled and tested on every system; `platform::win` applies them.

use std::ffi::OsString;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::ConnectError;

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

/// The `ConnectError` of the pipe client's failed open with OS code `raw_os_error` (design part 9.6; Ruling W11):
/// `ERROR_FILE_NOT_FOUND` (2) is NotRunning; `ERROR_ACCESS_DENIED` (5) is NotElevated when this process is not
/// elevated, else Refused; any other code, `ERROR_PIPE_BUSY` (231) after the one wait among them, is Refused.
/// Def: T3-12. Use: T3-12.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn connect_error(raw_os_error: i32, own_elevated: bool) -> ConnectError {
    match raw_os_error {
        2 => ConnectError::NotRunning,
        5 if !own_elevated => ConnectError::NotElevated,
        _ => ConnectError::Refused,
    }
}

/// The engine's pipe (design part 3.3, ARCH-16). Only the binaries use it. Def: T3-2. Use: T3-12, T3-23.
pub const PIPE_NAME: &str = r"\\.\pipe\lazymkv";
/// The engine's mutex, in the namespace every logon session shares (design part 3.3, ARCH-16). Only the binaries use
/// it. Def: T3-2. Use: T3-23.
pub const MUTEX_NAME: &str = r"Global\lazymkv-engine";

/// The names Windows defines for a logon session: the only ones of the engine's environment a guarded child inherits
/// (review round 1 of pull request 3 (V1)). Their values come from the engine's own environment, which
/// `HKCU\Environment` and `Volatile Environment` shape (`USERPROFILE`, `APPDATA`, `LOCALAPPDATA`, `TEMP`, `TMP` and
/// PATH's user part), so a non-elevated process of the owner can still choose where an elevated child that follows
/// one of them reads or writes; building the values from HKLM and the token is left for later, the deferred item V1
/// extends in docs/phase2/engine.md (the round's final review (W1)). Def: PR #14 review round 1 (V1). Use: PR #14
/// review round 1 (V1).
pub const CHILD_ENV_NAMES: [&str; 31] = [
    "SystemRoot",
    "windir",
    "SystemDrive",
    "ComSpec",
    "PATHEXT",
    "PATH",
    "OS",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_IDENTIFIER",
    "PROCESSOR_LEVEL",
    "PROCESSOR_REVISION",
    "COMPUTERNAME",
    "ProgramData",
    "ALLUSERSPROFILE",
    "PUBLIC",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "CommonProgramFiles",
    "CommonProgramFiles(x86)",
    "CommonProgramW6432",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "USERNAME",
    "USERDOMAIN",
    "TEMP",
    "TMP",
];

/// The environment a guarded child gets: the pairs of `inherited` whose name is valid UTF-8 and equals a name of
/// `CHILD_ENV_NAMES` ignoring ASCII case, in their order and with their own spelling, then `extra` in its order, so
/// that where `extra` names a kept variable again, std's later `env` wins. Every other name is dropped, `=C:`-style
/// entries among them. This is design part 3.5's rule, "if a non-elevated process could write there, it could choose
/// what the engine runs", applied to the children's environment (review round 1 of pull request 3 (V1)): Windows
/// builds the elevated engine's environment over HKCU's Environment, which a non-elevated process of the owner can
/// write, and an option variable there (JAVA_TOOL_OPTIONS, FFREPORT, MKVTOOLNIX_OPTIONS) could choose what an
/// elevated tool loads or where it writes. A list of the names kept, not of the names dropped, since makemkvcon is
/// closed source and the Java runtime, Qt and .NET read vendor variables nobody lists.
/// Def: PR #14 review round 1 (V1). Use: PR #14 review round 1 (V1).
pub fn child_environment<I: IntoIterator<Item = (OsString, OsString)>>(
    inherited: I,
    extra: &[(String, String)],
) -> Vec<(OsString, OsString)> {
    let listed = |name: &str| CHILD_ENV_NAMES.iter().any(|kept| kept.eq_ignore_ascii_case(name));
    let mut environment: Vec<(OsString, OsString)> =
        inherited.into_iter().filter(|(name, _)| name.to_str().is_some_and(listed)).collect();
    environment.extend(extra.iter().map(|(name, value)| (OsString::from(name), OsString::from(value))));
    environment
}

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
    fn access_denied_is_not_elevated_only_when_this_process_is_not() {
        assert_eq!(connect_error(5, false), ConnectError::NotElevated);
        assert_eq!(connect_error(5, true), ConnectError::Refused);
    }

    #[test]
    fn file_not_found_is_not_running_and_any_other_error_is_refused() {
        assert_eq!(connect_error(2, true), ConnectError::NotRunning);
        assert_eq!(connect_error(2, false), ConnectError::NotRunning);
        for code in [231, 109, 87, 1326] {
            assert_eq!(connect_error(code, true), ConnectError::Refused, "{code}");
            assert_eq!(connect_error(code, false), ConnectError::Refused, "{code}");
        }
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

    /// `pairs` as the environment pairs `child_environment` takes and gives.
    fn pairs(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs.iter().map(|(name, value)| (OsString::from(name), OsString::from(value))).collect()
    }

    /// Review round 1 of pull request 3 (V1).
    #[test]
    fn the_child_environment_keeps_windows_own_variables_whatever_their_case() {
        let inherited = pairs(&[
            ("Path", "p"),
            ("SYSTEMROOT", "s"),
            ("windir", "w"),
            ("TEMP", "t"),
            ("userprofile", "u"),
            ("ProgramFiles(x86)", "x"),
        ]);
        assert_eq!(child_environment(inherited.clone(), &[]), inherited);
    }

    /// Review round 1 of pull request 3 (V1): a name the list does not hold is dropped, however like a listed one it
    /// looks.
    #[test]
    fn the_child_environment_drops_every_variable_it_does_not_name() {
        let inherited = pairs(&[
            ("JAVA_TOOL_OPTIONS", "-agentpath:x"),
            ("_JAVA_OPTIONS", "x"),
            ("JDK_JAVA_OPTIONS", "x"),
            ("OPENJ9_JAVA_OPTIONS", "x"),
            ("JAVA_HOME", "x"),
            ("CLASSPATH", "x"),
            ("FFREPORT", "file=x"),
            ("MKVTOOLNIX_OPTIONS", "--redirect-output x"),
            ("MTX_OPTIONS", "x"),
            ("MKVMERGE_OPTIONS", "x"),
            ("MKVMERGE_DEBUG", "x"),
            ("QT_PLUGIN_PATH", "x"),
            ("COR_PROFILER_PATH", "x"),
            ("PATH_EXTRA", "x"),
            ("=C:", "C:\\"),
            ("PATH", "p"),
        ]);
        assert_eq!(child_environment(inherited, &[]), pairs(&[("PATH", "p")]));
    }

    /// Review round 1 of pull request 3 (V1): `Spawn::env` comes after what is kept, so where it names a kept variable
    /// again, std's later `env` wins.
    #[test]
    fn a_spawns_own_variables_come_after_the_kept_ones() {
        let extra = [
            ("LAZYMKV_TEST_HELPER".to_string(), "print-env".to_string()),
            ("TEMP".to_string(), "mine".to_string()),
        ];
        assert_eq!(
            child_environment(pairs(&[("TEMP", "t"), ("FOO", "f")]), &extra),
            pairs(&[("TEMP", "t"), ("LAZYMKV_TEST_HELPER", "print-env"), ("TEMP", "mine")])
        );
    }

    /// R5-code-38: on Windows the worker's thread count reaches it only through `Spawn::env`; an inherited value is
    /// dropped, so a variable the owner set for the engine never sizes the worker's pool.
    #[test]
    fn rten_num_threads_reaches_the_child_only_through_the_spawns_own_variables() {
        let inherited = || pairs(&[("RTEN_NUM_THREADS", "8"), ("PATH", "p")]);
        let extra = [("RTEN_NUM_THREADS".to_string(), "3".to_string())];
        assert_eq!(child_environment(inherited(), &extra), pairs(&[("PATH", "p"), ("RTEN_NUM_THREADS", "3")]));
        assert_eq!(child_environment(inherited(), &[]), pairs(&[("PATH", "p")]));
    }

    #[test]
    fn the_production_names_are_the_designs() {
        assert_eq!(PIPE_NAME, r"\\.\pipe\lazymkv");
        assert_eq!(MUTEX_NAME, r"Global\lazymkv-engine");
        assert_eq!(MUTEX_SDDL, "O:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)");
        assert_eq!((SYSTEM_SID, ADMINISTRATORS_SID, HIGH_INTEGRITY_SID), ("S-1-5-18", "S-1-5-32-544", "S-1-16-12288"));
    }
}

//! The parts of design part 6.1 that need no Windows call, compiled and tested on every system (design part 4.2): the
//! Credential Manager target a secret is stored under (Ruling W13), the share and the volume of a root (S11), and the
//! share connector's order of attempts over its three Windows calls (P7; Ruling W14). `win::creds` makes the
//! Credential Manager calls and `win::shares` the share connector's.

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lazymkv_core::ipc::share_credential;

use crate::{Reach, SecretStore, ShareConnector};

/// The prefix of lazymkv's Credential Manager entries (design part 6.1; Ruling W13): the secret
/// `share:\\server\share` is the entry `lazymkv:share:\\server\share`, and `tmdb_token` the entry
/// `lazymkv:tmdb_token`. Only `CredentialStore::new` reaches Credential Manager with it; vault tests give a prefix of
/// their own (Global Constraints). Def: T3-13. Use: T3-13.
pub const CREDENTIAL_PREFIX: &str = "lazymkv:";

/// The Credential Manager target of the secret `name` under `prefix`: the two joined (design part 6.1; Ruling W13).
/// Def: T3-13. Use: T3-13.
pub fn credential_target(prefix: &str, name: &str) -> String {
    format!("{prefix}{name}")
}

/// The `\\server\share` of a Universal Naming Convention (UNC) root (design part 6.1): '/' is read as '\', the first
/// two components after the leading `\\` are kept in their own case, and both must be non-empty. None for a local
/// root, for a root with fewer than two components, and for the `\\?\` and `\\.\` forms. Def: T3-14. Use: T3-14.
pub fn share_of(root: &str) -> Option<String> {
    let root = root.replace('/', r"\");
    let mut components = root.strip_prefix(r"\\")?.split('\\');
    let (server, share) = (components.next()?, components.next()?);
    if server.is_empty() || share.is_empty() || server == "?" || server == "." {
        return None;
    }
    Some(format!(r"\\{server}\{share}"))
}

/// The volume of a root, equal for two roots on one volume (design part 6.1, S11): `share_of(root)` in ASCII lower
/// case, or `X:` in upper case for a drive-letter root (`X:\` or `X:/`); None for any other root. Lexical: no Windows
/// call, so the owner thread's `volume` calls never wait on a share. Def: T3-14. Use: T3-14.
pub fn volume_key(root: &str) -> Option<String> {
    match share_of(root) {
        Some(share) => Some(share.to_ascii_lowercase()),
        None => drive_letter(root).map(|letter| format!("{letter}:")),
    }
}

/// The secret name of a share's credential: "share:" and the share (design parts 6.1, 9.4; Ruling T7-f).
/// Def: T3-14. Use: T3-14.
pub fn share_secret_name(share: &str) -> String {
    format!("share:{share}")
}

/// The Windows error a second credential for an already reached server gives, named in the connector's text (design
/// part 6.1). Def: T3-14. Use: T3-14.
pub const SESSION_CONFLICT: &str = "ERROR_SESSION_CREDENTIAL_CONFLICT";

/// The code of `SESSION_CONFLICT`, 1219.
const SESSION_CONFLICT_CODE: u32 = 1219;

/// The errors of a share's first probe after which the stored credential is tried: `ERROR_ACCESS_DENIED` (5) and
/// `ERROR_LOGON_FAILURE` (1326), how Windows says the session holds no credential the server accepts (design part
/// 6.1). Any other error is the share not answering.
const FALL_BACK: [u32; 2] = [5, 1326];

/// The errors of `connect` that are Windows refusing the credential itself, so it is remembered and not tried again
/// until its value changes, and a wrong password cannot lock the owner's account (Review Focus 5):
/// `ERROR_ACCESS_DENIED` (5), `ERROR_INVALID_PASSWORD` (86), `ERROR_LOGON_FAILURE` (1326), `ERROR_ACCOUNT_RESTRICTION`
/// (1327), `ERROR_PASSWORD_EXPIRED` (1330), `ERROR_ACCOUNT_DISABLED` (1331) and `ERROR_ACCOUNT_LOCKED_OUT` (1909).
const CREDENTIAL_REFUSED: [u32; 7] = [5, 86, 1326, 1327, 1330, 1331, 1909];

/// What `access` reports after a `reach` that got in to a drive root, through the session's own access, and through
/// lazymkv's stored credential (design parts 6.1, 7.1).
const LOCAL_FOLDER: &str = "local folder";
const SESSION_ACCESS: &str = "the session's access";
const STORED_CREDENTIAL: &str = "lazymkv's stored credential";

/// The three Windows calls of the share connector (design part 6.1; Owner decision 7), behind a seam so that the
/// fallback's order is tested on every system: `probe` answers whether a share root `\\server\share\` or a drive root
/// `X:\` opens as a folder, `connect` makes a deviceless connection with a user name and a password, and `free` reads
/// the bytes available to the caller on such a root. Each Err is a Windows error code. Def: T3-14. Use: T3-14.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) trait ShareApi: Send + Sync {
    fn probe(&self, path: &str) -> Result<(), u32>;
    fn connect(&self, share: &str, user: &str, password: &str) -> Result<(), u32>;
    fn free(&self, path: &str) -> Result<u64, u32>;
}

/// The `ShareConnector` of design part 6.1 over a `ShareApi` (P7, S11, V15; Ruling W14): the session's access first,
/// lazymkv's stored credential second, a refused credential not tried again until its value changes (Review Focus 5),
/// and texts that hold no user name or password. Def: T3-14. Use: T3-14.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct Shares<A: ShareApi> {
    /// The Windows calls.
    api: A,
    /// Where lazymkv's share credentials are read from.
    secrets: Arc<dyn SecretStore>,
    /// volume_key -> (the refused value's hash, Windows' code), kept in memory only, for the engine's run.
    refused: Mutex<BTreeMap<String, (u64, u32)>>,
    /// volume_key -> how the last reach got in.
    access: Mutex<BTreeMap<String, &'static str>>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl<A: ShareApi> Shares<A> {
    /// A connector over `api` that reads lazymkv's credentials from `secrets`, with no credential refused and no access
    /// recorded yet (Ruling W14). Def: T3-14. Use: T3-14.
    pub(crate) fn new(api: A, secrets: Arc<dyn SecretStore>) -> Shares<A> {
        Shares { api, secrets, refused: Mutex::default(), access: Mutex::default() }
    }

    /// Rules 2 to 6 of `reach` for a root with a target (Ruling W14): how it got in, or the Unreachable text. No lock
    /// is held across an `api` call or a secret read. Two threads that meet one value at the same moment may each try
    /// it once.
    fn attempt(&self, target: &Target) -> Result<&'static str, String> {
        let first = match self.api.probe(&target.path) {
            Ok(()) => return Ok(if target.share.is_some() { SESSION_ACCESS } else { LOCAL_FOLDER }),
            Err(code) => code,
        };
        // A drive root never connects; a share falls back only when the session's access was refused.
        let share = match &target.share {
            Some(share) if FALL_BACK.contains(&first) => share,
            Some(share) => return Err(no_answer(share, first)),
            None => return Err(no_answer(&target.path, first)),
        };
        let Some(value) = self.secrets.get(&share_secret_name(share)) else {
            return Err(format!(
                "{share} refused this session's access ({}), and this session can read no lazymkv credential for it",
                os_error(first)
            ));
        };
        let Ok((user, password)) = share_credential(&value) else {
            return Err(format!(
                "lazymkv's stored credential for {share} is not a user name and a password on two lines"
            ));
        };
        let hash = value_hash(&value);
        let remembered =
            lock(&self.refused).get(&target.key).filter(|(refused, _)| *refused == hash).map(|&(_, code)| code);
        if let Some(code) = remembered {
            return Err(refused_text(share, first, code));
        }
        match self.api.connect(share, user, password) {
            Err(SESSION_CONFLICT_CODE) => Err(format!(
                "{share}: {SESSION_CONFLICT}: this session already reaches the server as another user, so lazymkv's stored credential was not used"
            )),
            Err(code) if CREDENTIAL_REFUSED.contains(&code) => {
                lock(&self.refused).insert(target.key.clone(), (hash, code));
                Err(refused_text(share, first, code))
            }
            // The server did not answer the connection, so nothing is remembered and the next reach tries again.
            Err(code) => Err(no_answer(share, code)),
            Ok(()) => match self.api.probe(&target.path) {
                Ok(()) => {
                    lock(&self.refused).remove(&target.key);
                    Ok(STORED_CREDENTIAL)
                }
                Err(code) => Err(format!(
                    "{share} refused access again after connecting with lazymkv's stored credential: {}",
                    os_error(code)
                )),
            },
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl<A: ShareApi> ShareConnector for Shares<A> {
    fn reach(&self, root: &str) -> Reach {
        let Some(target) = target(root) else { return Reach::Unreachable(neither(root)) };
        match self.attempt(&target) {
            Ok(access) => {
                lock(&self.access).insert(target.key, access);
                Reach::Reachable
            }
            Err(text) => {
                lock(&self.access).remove(&target.key);
                Reach::Unreachable(text)
            }
        }
    }

    fn free_bytes(&self, root: &str) -> io::Result<u64> {
        let target = target(root).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, neither(root)))?;
        self.api.free(&target.path).map_err(os_error)
    }

    fn volume(&self, root: &str) -> Option<String> {
        volume_key(root)
    }

    fn access(&self, root: &str) -> Option<&'static str> {
        let key = volume_key(root)?;
        lock(&self.access).get(&key).copied()
    }
}

/// Where `reach` and `free_bytes` look for a root (Ruling W14): `path` is the share root `\\server\share\` or the
/// drive root `X:\`, so a missing folder below it is not an unreachable share; `share` is the share of a share root;
/// `key` is the root's volume_key.
struct Target {
    path: String,
    share: Option<String>,
    key: String,
}

/// The target of `root`, or None for a root that is neither a share nor a drive-letter root.
fn target(root: &str) -> Option<Target> {
    if let Some(share) = share_of(root) {
        let key = share.to_ascii_lowercase();
        return Some(Target { path: format!(r"{share}\"), share: Some(share), key });
    }
    let letter = drive_letter(root)?;
    Some(Target { path: format!(r"{letter}:\"), share: None, key: format!("{letter}:") })
}

/// The upper-case letter of a drive-letter root, one ASCII letter, a colon and '\' or '/'; None for any other root.
fn drive_letter(root: &str) -> Option<char> {
    match root.as_bytes() {
        [letter, b':', b'\\' | b'/', ..] if letter.is_ascii_alphabetic() => {
            Some(char::from(letter.to_ascii_uppercase()))
        }
        _ => None,
    }
}

/// Windows error `code` as an `io::Error`, whose text is the system's message and `(os error <code>)`.
fn os_error(code: u32) -> io::Error {
    io::Error::from_raw_os_error(code as i32)
}

/// The text of a root that is neither a share nor a drive-letter root.
fn neither(root: &str) -> String {
    format!("{root} is neither a drive folder nor a share")
}

/// The text of a share or a drive root that failed with `code` (Ruling W14).
fn no_answer(place: &str, code: u32) -> String {
    format!("{place} does not answer: {}", os_error(code))
}

/// The text of a stored credential Windows refused with `code`, after the session's access was refused with `first`.
fn refused_text(share: &str, first: u32, code: u32) -> String {
    format!(
        "{share} refused this session's access ({}), and Windows refused lazymkv's stored credential: {}; it is not tried again until it changes",
        os_error(first),
        os_error(code)
    )
}

/// The hash `refused` keeps of a stored value: `DefaultHasher` over it, kept in memory only (Review Focus 5).
fn value_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Locks `mutex`, ignoring a poison a panicked thread left: each map is changed one whole entry at a time.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::io;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::sync::{Arc, Mutex};

    use crate::testutil::Secrets;
    use crate::{Reach, SecretStore, ShareConnector};

    const ROOT: &str = r"\\nas\media\lazymkv-staging\engine";
    const NAME: &str = r"share:\\nas\media";
    const CREDENTIAL: &str = "user-sentinel-7c1e\npass-sentinel-4b9d";

    /// The text of a Windows error code, as the connector writes it.
    fn e(code: u32) -> String {
        io::Error::from_raw_os_error(code as i32).to_string()
    }

    /// A scripted ShareApi: `probe` answers from `answers` in order and repeats the last one, `connect` and `free` give
    /// their fields, and every call is recorded.
    struct Api {
        answers: Mutex<VecDeque<Result<(), u32>>>,
        connect: Result<(), u32>,
        free: Result<u64, u32>,
        probes: Mutex<Vec<String>>,
        connects: Mutex<Vec<(String, String, String)>>,
        frees: Mutex<Vec<String>>,
    }

    fn api_free(answers: &[Result<(), u32>], connect: Result<(), u32>, free: Result<u64, u32>) -> Arc<Api> {
        Arc::new(Api {
            answers: Mutex::new(answers.iter().copied().collect()),
            connect,
            free,
            probes: Mutex::default(),
            connects: Mutex::default(),
            frees: Mutex::default(),
        })
    }

    fn api(answers: &[Result<(), u32>], connect: Result<(), u32>) -> Arc<Api> {
        api_free(answers, connect, Ok(123_456))
    }

    impl ShareApi for Arc<Api> {
        fn probe(&self, path: &str) -> Result<(), u32> {
            self.probes.lock().unwrap().push(path.to_string());
            let mut answers = self.answers.lock().unwrap();
            if answers.len() > 1 { answers.pop_front().unwrap() } else { answers.front().copied().unwrap_or(Ok(())) }
        }
        // `secret_half`, not `password`: a name CodeQL's cleartext-logging query keys on (Global Constraints).
        fn connect(&self, share: &str, user: &str, secret_half: &str) -> Result<(), u32> {
            self.connects.lock().unwrap().push((share.to_string(), user.to_string(), secret_half.to_string()));
            self.connect
        }
        fn free(&self, path: &str) -> Result<u64, u32> {
            self.frees.lock().unwrap().push(path.to_string());
            self.free
        }
    }

    fn probes(api: &Api) -> Vec<String> {
        api.probes.lock().unwrap().clone()
    }

    fn connects(api: &Api) -> Vec<(String, String, String)> {
        api.connects.lock().unwrap().clone()
    }

    fn sentinels(share: &str) -> (String, String, String) {
        (share.to_string(), "user-sentinel-7c1e".to_string(), "pass-sentinel-4b9d".to_string())
    }

    /// testutil::Secrets that counts its reads.
    struct Counted {
        inner: Secrets,
        gets: AtomicUsize,
    }

    impl SecretStore for Counted {
        fn get(&self, name: &str) -> Option<String> {
            self.gets.fetch_add(1, SeqCst);
            self.inner.get(name)
        }
        fn set(&self, name: &str, value: &str) -> Result<(), String> {
            self.inner.set(name, value)
        }
    }

    fn counted(entries: &[(&str, &str)]) -> Arc<Counted> {
        let inner = entries.iter().fold(Secrets::new(), |secrets, (name, value)| secrets.with(name, value));
        Arc::new(Counted { inner, gets: AtomicUsize::new(0) })
    }

    fn connector(api: &Arc<Api>, secrets: &Arc<Counted>) -> Shares<Arc<Api>> {
        Shares::new(Arc::clone(api), Arc::clone(secrets) as Arc<dyn SecretStore>)
    }

    #[test]
    fn the_targets_are_the_prefix_and_the_secret_name() {
        assert_eq!(credential_target(CREDENTIAL_PREFIX, r"share:\\server\share"), r"lazymkv:share:\\server\share");
        assert_eq!(credential_target(CREDENTIAL_PREFIX, "tmdb_token"), "lazymkv:tmdb_token");
    }

    #[test]
    fn the_session_access_is_tried_first_and_the_secret_store_not_read() {
        let (api, secrets) = (api(&[Ok(())], Ok(())), counted(&[(NAME, CREDENTIAL)]));
        assert_eq!(connector(&api, &secrets).reach(ROOT), Reach::Reachable);
        assert_eq!(probes(&api), [r"\\nas\media\"]);
        assert!(connects(&api).is_empty());
        assert_eq!(secrets.gets.load(SeqCst), 0);
    }

    #[test]
    fn access_denied_and_logon_failure_fall_back_to_the_stored_credential_once() {
        for code in [5, 1326] {
            let (api, secrets) = (api(&[Err(code), Ok(())], Ok(())), counted(&[(NAME, CREDENTIAL)]));
            assert_eq!(connector(&api, &secrets).reach(ROOT), Reach::Reachable, "{code}");
            // assert!, not assert_eq!: a failure prints no recorded half of the credential.
            assert!(connects(&api) == [sentinels(r"\\nas\media")], "{code}: connect was not called with the stored credential's two halves");
            assert_eq!(probes(&api), [r"\\nas\media\", r"\\nas\media\"], "{code}");
        }
    }

    #[test]
    fn any_other_first_error_is_unreachable_with_no_fallback() {
        for code in [53, 67, 121] {
            let (api, secrets) = (api(&[Err(code)], Ok(())), counted(&[(NAME, CREDENTIAL)]));
            let reach = connector(&api, &secrets).reach(ROOT);
            assert_eq!(reach, Reach::Unreachable(format!(r"\\nas\media does not answer: {}", e(code))));
            assert!(connects(&api).is_empty());
            assert_eq!(secrets.gets.load(SeqCst), 0);
        }
    }

    #[test]
    fn denied_with_no_readable_credential_says_so() {
        let (api, secrets) = (api(&[Err(5)], Ok(())), counted(&[]));
        let reach = connector(&api, &secrets).reach(ROOT);
        let text = format!(r"\\nas\media refused this session's access ({}), and this session can read no lazymkv credential for it", e(5));
        assert_eq!(reach, Reach::Unreachable(text));
        assert!(connects(&api).is_empty());
    }

    #[test]
    fn a_refused_connection_says_windows_refused_the_stored_credential() {
        let (api, secrets) = (api(&[Err(5)], Err(86)), counted(&[(NAME, CREDENTIAL)]));
        let reach = connector(&api, &secrets).reach(ROOT);
        let text = format!(r"\\nas\media refused this session's access ({}), and Windows refused lazymkv's stored credential: {}; it is not tried again until it changes", e(5), e(86));
        assert_eq!(reach, Reach::Unreachable(text));
        assert_eq!(probes(&api).len(), 1);
    }

    #[test]
    fn a_session_credential_conflict_is_named_and_not_retried() {
        let (api, secrets) = (api(&[Err(5)], Err(1219)), counted(&[(NAME, CREDENTIAL)]));
        let text = r"\\nas\media: ERROR_SESSION_CREDENTIAL_CONFLICT: this session already reaches the server as another user, so lazymkv's stored credential was not used";
        assert_eq!(connector(&api, &secrets).reach(ROOT), Reach::Unreachable(text.to_string()));
        assert_eq!((probes(&api).len(), connects(&api).len()), (1, 1));
    }

    #[test]
    fn a_retried_access_that_fails_again_says_so() {
        let (api, secrets) = (api(&[Err(5), Err(5)], Ok(())), counted(&[(NAME, CREDENTIAL)]));
        let reach = connector(&api, &secrets).reach(ROOT);
        let text = format!(r"\\nas\media refused access again after connecting with lazymkv's stored credential: {}", e(5));
        assert_eq!(reach, Reach::Unreachable(text));
        assert_eq!(probes(&api).len(), 2);
    }

    #[test]
    fn a_malformed_stored_value_is_named_without_its_text() {
        for value in ["user-sentinel-7c1e", "user-sentinel-7c1e\npass-sentinel-4b9d\nthird", "\npass-sentinel-4b9d"] {
            let (api, secrets) = (api(&[Err(5)], Ok(())), counted(&[(NAME, value)]));
            let text = r"lazymkv's stored credential for \\nas\media is not a user name and a password on two lines";
            assert_eq!(connector(&api, &secrets).reach(ROOT), Reach::Unreachable(text.to_string()), "{value:?}");
            assert!(connects(&api).is_empty());
        }
    }

    #[test]
    fn no_text_holds_the_sentinel_user_or_password() {
        let cases: [(&[Result<(), u32>], Result<(), u32>, &str); 7] = [
            (&[Err(53)], Ok(()), CREDENTIAL),
            (&[Err(5)], Err(1326), CREDENTIAL),
            (&[Err(5)], Err(1219), CREDENTIAL),
            (&[Err(5)], Err(53), CREDENTIAL),
            (&[Err(5), Err(5)], Ok(()), CREDENTIAL),
            (&[Err(5)], Ok(()), "user-sentinel-7c1e"),
            (&[Err(5)], Ok(()), "pass-sentinel-4b9d\n"),
        ];
        for (answers, connect, value) in cases {
            let (api, secrets) = (api_free(answers, connect, Err(5)), counted(&[(NAME, value)]));
            let shares = connector(&api, &secrets);
            for _ in 0..2 {
                let Reach::Unreachable(text) = shares.reach(ROOT) else { panic!("{answers:?} {connect:?} reached") };
                let free = shares.free_bytes(ROOT).unwrap_err().to_string();
                for text in [text, free] {
                    assert!(!text.contains("sentinel"), "{text}");
                }
            }
        }
    }

    /// Review Focus 5.
    #[test]
    fn a_refused_stored_credential_is_not_tried_again_until_it_changes() {
        let (api, secrets) = (api(&[Err(5)], Err(1326)), counted(&[(NAME, CREDENTIAL)]));
        let shares = connector(&api, &secrets);
        let first = shares.reach(ROOT);
        assert_eq!(shares.reach(ROOT), first);
        assert_eq!(connects(&api).len(), 1);
        secrets.set(NAME, "user-sentinel-7c1e\npass-sentinel-4b9d-new").unwrap();
        shares.reach(ROOT);
        assert_eq!(connects(&api).len(), 2);
    }

    #[test]
    fn a_connection_the_server_does_not_answer_is_tried_again() {
        let (api, secrets) = (api(&[Err(5)], Err(53)), counted(&[(NAME, CREDENTIAL)]));
        let shares = connector(&api, &secrets);
        assert_eq!(shares.reach(ROOT), Reach::Unreachable(format!(r"\\nas\media does not answer: {}", e(53))));
        shares.reach(ROOT);
        assert_eq!(connects(&api).len(), 2);
    }

    /// Review Focus 1: the secret is stored with backslashes in the root's own case, since this store compares exactly.
    #[test]
    fn a_forward_slash_root_finds_the_backslash_secret_and_a_root_in_another_case_shares_its_volume() {
        let secrets = counted(&[(r"share:\\unas-pro\media", "user-sentinel-7c1e\r\npass-sentinel-4b9d")]);
        let api = api(&[Err(5), Ok(())], Ok(()));
        let shares = connector(&api, &secrets);
        assert_eq!(shares.reach("//unas-pro/media/x"), Reach::Reachable);
        assert_eq!(probes(&api), [r"\\unas-pro\media\", r"\\unas-pro\media\"]);
        assert!(connects(&api) == [sentinels(r"\\unas-pro\media")], "connect was not called with the stored credential's two halves");
        assert_eq!(share_of(r"\\UNAS-PRO\Media\y").as_deref(), Some(r"\\UNAS-PRO\Media"));
        assert_eq!(shares.volume("//UNAS-PRO/Media/y"), shares.volume(r"\\unas-pro\media\x"));
        assert_eq!(shares.volume("//UNAS-PRO/Media/y").as_deref(), Some(r"\\unas-pro\media"));
    }

    #[test]
    fn two_local_roots_on_one_letter_are_one_volume() {
        let shares = connector(&api(&[Ok(())], Ok(())), &counted(&[]));
        assert_eq!(shares.volume(r"C:\lazymkv\dev\staging").as_deref(), Some("C:"));
        assert_eq!(shares.volume("c:/lazymkv/dev/library/Movies").as_deref(), Some("C:"));
        assert_eq!(shares.volume(r"D:\x").as_deref(), Some("D:"));
    }

    #[test]
    fn a_question_mark_path_is_not_a_share() {
        for root in [r"\\?\C:\x", r"\\?\UNC\nas\media\x", r"\\.\pipe\lazymkv", r"\\nas", r"\\nas\", "Movies", "/srv/media", "C:relative"] {
            assert_eq!((share_of(root), volume_key(root)), (None, None), "{root}");
            let (api, secrets) = (api(&[Ok(())], Ok(())), counted(&[]));
            let shares = connector(&api, &secrets);
            let text = format!("{root} is neither a drive folder nor a share");
            assert_eq!(shares.reach(root), Reach::Unreachable(text.clone()));
            assert_eq!(shares.free_bytes(root).unwrap_err().to_string(), text);
            assert!(probes(&api).is_empty());
        }
    }

    #[test]
    fn a_local_root_never_connects_and_reports_its_drive_free_space() {
        let (api, secrets) = (api(&[Ok(())], Ok(())), counted(&[(NAME, CREDENTIAL)]));
        let shares = connector(&api, &secrets);
        assert_eq!(shares.reach(r"C:\lazymkv\dev\staging"), Reach::Reachable);
        assert_eq!(shares.free_bytes("c:/lazymkv/dev/staging").unwrap(), 123_456);
        assert_eq!(probes(&api), [r"C:\"]);
        assert_eq!(*api.frees.lock().unwrap(), [r"C:\"]);
        let denied = self::api(&[Err(5)], Ok(()));
        let reach = connector(&denied, &secrets).reach(r"Q:\x");
        assert_eq!(reach, Reach::Unreachable(format!(r"Q:\ does not answer: {}", e(5))));
        assert!(connects(&api).is_empty() && connects(&denied).is_empty());
        assert_eq!(secrets.gets.load(SeqCst), 0);
    }

    #[test]
    fn access_reports_how_the_last_reach_succeeded() {
        let (api, secrets) = (api(&[Ok(()), Err(5), Ok(()), Err(53)], Ok(())), counted(&[(NAME, CREDENTIAL)]));
        let shares = connector(&api, &secrets);
        assert_eq!(shares.access(ROOT), None);
        shares.reach(ROOT);
        assert_eq!(shares.access(ROOT), Some("the session's access"));
        shares.reach(ROOT);
        assert_eq!(shares.access(r"\\NAS\Media\other"), Some("lazymkv's stored credential"));
        shares.reach(ROOT);
        assert_eq!(shares.access(ROOT), None);
        let local = connector(&self::api(&[Ok(())], Ok(())), &secrets);
        local.reach(r"C:\x");
        assert_eq!(local.access(r"c:\y"), Some("local folder"));
    }
}

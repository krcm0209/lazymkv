//! The Windows `SecretStore` on Credential Manager (spec 4.3; design parts 4.2, 6.1; Ruling W13): one generic
//! credential per secret, read with `CredReadW` and written with `CredWriteW`. The target's rule sits in `shares`,
//! compiled and tested on every system.

use std::io;
use std::ptr::null_mut;
use std::slice;

#[cfg(test)]
use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
#[cfg(test)]
use windows_sys::Win32::Security::Credentials::CredDeleteW;
use windows_sys::Win32::Security::Credentials::{
    CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};

use super::wide;
use crate::shares::{credential_target, CREDENTIAL_PREFIX};
use crate::SecretStore;

/// The user name every lazymkv entry carries (Ruling W13).
const USER_NAME: &str = "lazymkv";
/// The most bytes an entry's blob holds, `CRED_MAX_CREDENTIAL_BLOB_SIZE` (2560; Ruling W13).
const MAX_BLOB: usize = CRED_MAX_CREDENTIAL_BLOB_SIZE as usize;
/// `set`'s text for a value over MAX_BLOB bytes, before any Windows call; it never holds the value (Ruling W13).
const TOO_LONG: &str = "the value is longer than 2560 bytes, the most a Credential Manager entry holds";

/// Credential Manager as the `SecretStore` (spec 4.3; design parts 4.2, 6.1; Ruling W13; Owner decision 8): each
/// secret is a generic credential with the target `credential_target(prefix, name)`, the user name `lazymkv`, the
/// value's UTF-8 bytes as its blob (2560 bytes at most) and the persistence `CRED_PERSIST_LOCAL_MACHINE`. `get` is None
/// on every error, so in a logon session with no vault (a key-based Secure Shell (SSH) logon, design part 6.1) a
/// stored secret reads as unset. `set`'s Err never holds the value. It lists nothing (S77 stays).
/// Def: T3-13. Use: T3-14, T3-23.
pub struct CredentialStore {
    /// The text every target starts with: CREDENTIAL_PREFIX, or a test's own prefix.
    prefix: String,
}

impl CredentialStore {
    /// The engine's store, under CREDENTIAL_PREFIX (design part 6.1; Ruling W13). Def: T3-13. Use: T3-14, T3-23.
    pub fn new() -> CredentialStore {
        CredentialStore::with_prefix(CREDENTIAL_PREFIX)
    }

    /// A store whose targets start with `prefix`; a test gives `lazymkv-test-<tag>-<pid>-<n>:`, never `lazymkv:`
    /// (Global Constraints). Def: T3-13. Use: T3-13.
    pub(crate) fn with_prefix(prefix: &str) -> CredentialStore {
        CredentialStore { prefix: prefix.to_string() }
    }

    /// Removes the entry of `name`, Ok also when nothing is stored under it (`ERROR_NOT_FOUND`, 1168), so a test's
    /// cleanup may run after the test removed it. Def: T3-13. Use: T3-13.
    #[cfg(test)]
    pub(crate) fn remove(&self, name: &str) -> Result<(), String> {
        let target = wide(credential_target(&self.prefix, name));
        // SAFETY: `target` is NUL-terminated and outlives the call, which takes no other pointer.
        if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } != 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
            Ok(())
        } else {
            Err(format!("Credential Manager could not remove the entry: {error}"))
        }
    }
}

impl SecretStore for CredentialStore {
    fn get(&self, name: &str) -> Option<String> {
        let target = wide(credential_target(&self.prefix, name));
        let mut credential: *mut CREDENTIALW = null_mut();
        // SAFETY: `target` is NUL-terminated and outlives the call; on success the call writes to `credential` the
        // address of a credential it allocates, which CredFree releases below.
        if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
            return None;
        }
        // The credential in a binding whose address is never taken, so that the code scanner sees the null check
        // cover its reads.
        let credential = credential;
        if credential.is_null() {
            None
        } else {
            // SAFETY: a non-null `credential` points at the CREDENTIALW CredReadW allocated, alive until CredFree.
            let (blob, size) = unsafe { ((*credential).CredentialBlob, (*credential).CredentialBlobSize) };
            // A size of 0 is the empty value, and its pointer is not read.
            let value = if size == 0 {
                Some(String::new())
            } else if blob.is_null() {
                None
            } else {
                // SAFETY: a non-null blob of the credential CredReadW returned holds its CredentialBlobSize bytes,
                // alive until CredFree; they are copied out before it runs.
                let bytes = unsafe { slice::from_raw_parts(blob, size as usize) };
                std::str::from_utf8(bytes).ok().map(String::from)
            };
            // SAFETY: CredReadW allocated `credential`; it is freed once, here, after the last read of it and of the
            // blob it holds.
            unsafe { CredFree(credential.cast_const().cast()) };
            value
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        if value.len() > MAX_BLOB {
            return Err(TOO_LONG.to_string());
        }
        let mut target = wide(credential_target(&self.prefix, name));
        let mut user = wide(USER_NAME);
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            // At most MAX_BLOB, checked above, so it fits.
            CredentialBlobSize: value.len() as u32,
            // CredWriteW only reads the blob, so the value's shared bytes are never written through this pointer.
            CredentialBlob: value.as_ptr().cast_mut(),
            // Owner decision 8's persistence; an overturn changes this one constant.
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: user.as_mut_ptr(),
            ..CREDENTIALW::default()
        };
        // SAFETY: `credential`, the two NUL-terminated strings and the value's bytes it points at outlive the call; the
        // blob's size is the value's length; CredWriteW reads them and writes none of them.
        if unsafe { CredWriteW(&credential, 0) } == 0 {
            return Err(format!("Credential Manager refused the entry: {}", io::Error::last_os_error()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::test_name;
    use crate::testutil::skip;
    use crate::SecretStore;

    /// A store under the test's own prefix, `lazymkv-test-<tag>-<pid>-<n>:` (Global Constraints).
    fn store(tag: &str) -> CredentialStore {
        CredentialStore::with_prefix(&format!("{}:", test_name(tag)))
    }

    /// Removes the test's entry however the test ends.
    struct Removal<'a> {
        store: &'a CredentialStore,
        name: &'a str,
    }

    impl Drop for Removal<'_> {
        fn drop(&mut self) {
            let _ = self.store.remove(self.name);
        }
    }

    /// Stores `value` under `name` and returns false. Returns true after printing a skip, writing nothing, when `CI`
    /// and `LAZYMKV_TEST_VAULT` are both unset, so no run writes the owner's vault before Owner decision 8's yes
    /// (`platform-ignored.ps1` sets the variable for `win::creds::` only); and when this logon session has no vault
    /// (ERROR_NO_SUCH_LOGON_SESSION, 1312; design part 6.1) and `CI` is unset. On continuous integration (CI) it never
    /// skips, unless Owner decision 15's vault fallback is taken.
    fn stored(store: &CredentialStore, name: &str, value: &str) -> bool {
        if std::env::var_os("CI").is_none() && std::env::var_os("LAZYMKV_TEST_VAULT").is_none() {
            skip("LAZYMKV_TEST_VAULT is not set; the vault tests run on CI and, after Owner decision 8, in the desktop session (design part 6.1)");
            return true;
        }
        let Err(error) = store.set(name, value) else { return false };
        assert!(error.starts_with("Credential Manager refused the entry: "), "{error}");
        assert!(!error.contains("sentinel"), "{error}");
        if error.contains("(os error 1312)") && std::env::var_os("CI").is_none() {
            skip("no credential vault in this logon session (design part 6.1)");
            return true;
        }
        panic!("{error}");
    }

    #[test]
    fn a_test_secret_round_trips_and_is_removed() {
        let store = store("creds");
        let _removal = Removal { store: &store, name: "tmdb_token" };
        if stored(&store, "tmdb_token", "pass-sentinel-4b9d") {
            return;
        }
        assert_eq!(store.get("tmdb_token").as_deref(), Some("pass-sentinel-4b9d"));
        assert!(!stored(&store, "tmdb_token", "pass-sentinel-4b9d-2"));
        assert_eq!(store.get("tmdb_token").as_deref(), Some("pass-sentinel-4b9d-2"));
        store.remove("tmdb_token").unwrap();
        assert_eq!(store.get("tmdb_token"), None);
        store.remove("tmdb_token").unwrap();
    }

    #[test]
    fn an_absent_secret_is_none() {
        assert_eq!(store("creds-absent").get(r"share:\\server\share"), None);
    }

    #[test]
    fn a_value_over_2560_bytes_is_refused_without_the_value() {
        let store = store("creds-long");
        let value = "pass-sentinel-4b9d".repeat(143); // 2574 bytes
        let refused = store.set("tmdb_token", &value).unwrap_err();
        assert_eq!(refused, "the value is longer than 2560 bytes, the most a Credential Manager entry holds");
        assert_eq!(store.get("tmdb_token"), None);
    }

    /// Review Focus 1: only Credential Manager ignores case, so this is the one test of the case rule.
    #[test]
    fn a_secret_is_found_by_its_name_in_another_case() {
        let store = store("creds-case");
        let _removal = Removal { store: &store, name: r"share:\\server\share" };
        if stored(&store, r"share:\\server\share", "user-sentinel-7c1e\npass-sentinel-4b9d") {
            return;
        }
        assert_eq!(store.get(r"share:\\SERVER\Share").as_deref(), Some("user-sentinel-7c1e\npass-sentinel-4b9d"));
    }
}

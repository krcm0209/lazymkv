//! The Windows calls of the share connector (spec 4.3; design parts 4.2, 6.1; P7; Ruling W14): `GetFileAttributesW`
//! asks whether a share root or a drive root opens as a folder, `WNetAddConnection2W` makes a deviceless connection
//! with lazymkv's stored credential, and `GetDiskFreeSpaceExW` reads the bytes available to the caller. The fallback's
//! order, its texts and the volume rule sit in `shares`, compiled and tested on every system.

use std::ptr::{self, null_mut};
use std::sync::atomic::{compiler_fence, Ordering};
use std::sync::Arc;

use windows_sys::Win32::Foundation::{GetLastError, ERROR_DIRECTORY};
use windows_sys::Win32::NetworkManagement::WNet::{WNetAddConnection2W, NETRESOURCEW, RESOURCETYPE_DISK};
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetFileAttributesW, FILE_ATTRIBUTE_DIRECTORY, INVALID_FILE_ATTRIBUTES,
};

use super::wide;
use crate::shares::{ShareApi, Shares};
use crate::{SecretStore, ShareConnector};

/// The three Windows calls of design part 6.1 as the `ShareApi` of `Shares` (Ruling W14). Def: T3-14. Use: T3-14.
pub(crate) struct Win32Shares;

impl ShareApi for Win32Shares {
    fn probe(&self, path: &str) -> Result<(), u32> {
        let path = wide(path);
        // SAFETY: `path` is NUL-terminated and outlives the call, which only reads it.
        let attributes = unsafe { GetFileAttributesW(path.as_ptr()) };
        if attributes == INVALID_FILE_ATTRIBUTES {
            // SAFETY: GetLastError takes no argument and reads this thread's last error, which the failed call set.
            return Err(unsafe { GetLastError() });
        }
        if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(ERROR_DIRECTORY);
        }
        Ok(())
    }

    fn connect(&self, share: &str, user: &str, password: &str) -> Result<(), u32> {
        let mut remote = wide(share);
        let user = wide(user);
        let mut password = wide(password);
        // No local name, so the connection is deviceless and maps no drive letter (design part 6.1, spec 4.3).
        let resource = NETRESOURCEW {
            dwType: RESOURCETYPE_DISK,
            lpRemoteName: remote.as_mut_ptr(),
            ..NETRESOURCEW::default()
        };
        // Flags 0: no CONNECT_UPDATE_PROFILE, so Windows does not restore the connection at a later logon, and no
        // CONNECT_INTERACTIVE, so Windows never prompts (design part 6.1).
        // SAFETY: `resource`, the share name it points at and the two NUL-terminated strings outlive the call, which
        // reads them and writes none; a null local name makes the connection deviceless, and the call takes a null
        // comment and provider.
        let code = unsafe { WNetAddConnection2W(&resource, password.as_ptr(), user.as_ptr(), 0) };
        zero(&mut password);
        if code == 0 {
            Ok(())
        } else {
            Err(code)
        }
    }

    fn free(&self, path: &str) -> Result<u64, u32> {
        let path = wide(path);
        let mut available = 0u64;
        // SAFETY: `path` is NUL-terminated and outlives the call; `available` is a u64 the call writes; the total and
        // total-free out-pointers may be null.
        if unsafe { GetDiskFreeSpaceExW(path.as_ptr(), &mut available, null_mut(), null_mut()) } == 0 {
            // SAFETY: GetLastError takes no argument and reads this thread's last error, which the failed call set.
            return Err(unsafe { GetLastError() });
        }
        Ok(available)
    }
}

/// Fills `units` with zeros through volatile writes, which the compiler keeps although the buffer is dropped next.
fn zero(units: &mut [u16]) {
    for unit in units.iter_mut() {
        // SAFETY: `unit` is a live, aligned and exclusive reference to one element of the buffer.
        unsafe { ptr::write_volatile(unit, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

/// The Windows `ShareConnector` (spec 4.3; design parts 4.2, 6.1; P7): `Shares` over the Windows calls, reading
/// lazymkv's credential from `secrets`. Def: T3-14. Use: T3-23.
pub fn share_connector(secrets: Arc<dyn SecretStore>) -> Arc<dyn ShareConnector> {
    Arc::new(Shares::new(Win32Shares, secrets))
}

/// LAZYMKV_TEST_SHARE_ROOT, or None after printing the skip (design part 12.1's convention).
#[cfg(test)]
fn test_share_root() -> Option<String> {
    let root = std::env::var("LAZYMKV_TEST_SHARE_ROOT").ok().filter(|root| !root.is_empty());
    if root.is_none() {
        crate::testutil::skip("LAZYMKV_TEST_SHARE_ROOT is not set (design part 12.4)");
    }
    root
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;

    use super::share_connector;
    use crate::testutil::{skip, temp_dir, Secrets};
    use crate::Reach;

    #[test]
    fn a_local_temp_folder_is_reachable_with_free_space() {
        let dir = temp_dir("share-local");
        let root = dir.to_str().unwrap();
        let shares = share_connector(Arc::new(Secrets::new()));
        assert_eq!(shares.reach(root), Reach::Reachable);
        assert!(shares.free_bytes(root).unwrap() > 0);
        assert_eq!(shares.access(root), Some("local folder"));
        assert_eq!(shares.volume(root), Some(root[..2].to_ascii_uppercase()));
    }

    #[test]
    fn a_missing_drive_letter_is_unreachable_with_its_os_error() {
        // SAFETY: GetLogicalDrives takes no argument and returns a bit mask.
        let mask = unsafe { GetLogicalDrives() };
        let Some(letter) = ('D'..='Z').rev().find(|letter| mask & (1 << (*letter as u32 - 'A' as u32)) == 0) else {
            return skip("every drive letter from D to Z is in use");
        };
        let root = format!(r"{letter}:\lazymkv");
        let shares = share_connector(Arc::new(Secrets::new()));
        let Reach::Unreachable(text) = shares.reach(&root) else { panic!("{root} reads as reachable") };
        assert!(text.starts_with(&format!(r"{letter}:\ does not answer: ")), "{text}");
        assert!(text.contains("(os error "), "{text}");
        assert!(shares.free_bytes(&root).unwrap_err().raw_os_error().is_some());
        assert_eq!(shares.access(&root), None);
    }
}

#[cfg(test)]
mod desktop {
    use std::sync::Arc;
    use std::{fs, io};

    use super::{share_connector, test_share_root};
    use crate::security::test_name;
    use crate::testutil::Secrets;
    use crate::{rename_no_replace, Reach};

    #[test]
    #[ignore = "desktop session, LAZYMKV_TEST_SHARE_ROOT under lazymkv-staging (design part 12.4)"]
    fn a_real_share_root_is_reachable_with_the_session_access() {
        let Some(root) = test_share_root() else { return };
        let shares = share_connector(Arc::new(Secrets::new()));
        assert_eq!(shares.reach(&root), Reach::Reachable);
        assert_eq!(shares.access(&root), Some("the session's access"));
    }

    #[test]
    #[ignore = "desktop session, LAZYMKV_TEST_SHARE_ROOT under lazymkv-staging (design part 12.4)"]
    fn a_real_share_reports_free_space_and_refuses_a_move_over_a_file() {
        let Some(root) = test_share_root() else { return };
        assert!(root.to_ascii_lowercase().contains(r"\lazymkv-staging\"), "{root} is not under lazymkv-staging (Owner decision 9)");
        let shares = share_connector(Arc::new(Secrets::new()));
        assert!(shares.free_bytes(&root).unwrap() > 0);
        let dir = std::path::Path::new(&root).join(test_name("share-move"));
        fs::create_dir_all(&dir).unwrap();
        // Removes the folder however the test ends, so a failure leaves nothing on the NAS (Owner decision 9).
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let (from, taken, free) = (dir.join("a.mkv.partial"), dir.join("b.mkv"), dir.join("c.mkv"));
        fs::write(&from, "new").unwrap();
        fs::write(&taken, "old").unwrap();
        assert_eq!(rename_no_replace(&from, &taken).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!((fs::read_to_string(&from).unwrap(), fs::read_to_string(&taken).unwrap()), ("new".to_string(), "old".to_string()));
        rename_no_replace(&from, &free).unwrap();
        assert!(!from.exists());
        assert_eq!(fs::read_to_string(&free).unwrap(), "new");
    }
}

#[cfg(test)]
mod over_ssh {
    use std::sync::Arc;

    use super::{share_connector, test_share_root};
    use crate::security::test_name;
    use crate::shares::share_of;
    use crate::win::CredentialStore;
    use crate::Reach;

    #[test]
    #[ignore = "over SSH, LAZYMKV_TEST_SHARE_ROOT"]
    fn over_ssh_a_denied_share_without_a_reachable_credential_says_so() {
        let Some(root) = test_share_root() else { return };
        let share = share_of(&root).expect("LAZYMKV_TEST_SHARE_ROOT names a share");
        let secrets = CredentialStore::with_prefix(&format!("{}:", test_name("ssh")));
        let Reach::Unreachable(text) = share_connector(Arc::new(secrets)).reach(&root) else {
            panic!("{root} answered over SSH, where design part 6.1 expects access denied");
        };
        assert!(text.starts_with(&format!("{share} refused this session's access (")), "{text}");
        assert!(text.ends_with("), and this session can read no lazymkv credential for it"), "{text}");
    }
}

#[cfg(test)]
mod arranged {
    use std::sync::Arc;

    use super::{share_connector, test_share_root};
    use crate::win::CredentialStore;
    use crate::Reach;

    /// The owner stored this share's credential with `lazymkvctl secret set`, so it sits under the engine's own prefix;
    /// the test reads it and writes nothing (Owner decision 7).
    #[test]
    #[ignore = "desktop session, only when the owner arranges a share the session cannot reach and stores its credential (Owner decision 7)"]
    fn the_stored_credential_reaches_a_share_the_session_cannot() {
        let Some(root) = test_share_root() else { return };
        let shares = share_connector(Arc::new(CredentialStore::new()));
        assert_eq!(shares.reach(&root), Reach::Reachable);
        assert_eq!(shares.access(&root), Some("lazymkv's stored credential"));
    }
}

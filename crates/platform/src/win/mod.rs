//! The Windows implementations of lazymkv-platform's traits and helpers (design parts 4.2, 4.3). Every `windows-sys`
//! call of the workspace sits in this module, and each `unsafe` block carries a `// SAFETY:` line saying why the call
//! is sound. The rules these implementations apply (descriptor strings, the token verdict, the share and volume rules,
//! the drive diff, the local-time text, the eject report) sit in modules that compile and are tested on every system.
//! Compiled only on Windows.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Diagnostics::Debug::{GetErrorMode, SetErrorMode, SEM_FAILCRITICALERRORS};
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY, REG_ROUTINE_FLAGS};

mod awake;
mod client;
mod console;
mod creds;
mod drives;
mod eject;
mod files;
mod folder;
mod host;
mod job;
mod mutex;
mod pipe;
mod shares;
mod token;

pub use awake::WinKeepAwake;
pub use client::PipeConnector;
pub use console::on_console_stop;
pub use creds::CredentialStore;
pub use drives::WinDrives;
pub use eject::WinEjector;
pub(crate) use files::{move_no_replace, move_replacing};
pub use folder::{program_data, protect_data_folder};
pub(crate) use folder::profile_folder;
pub use host::WinHost;
pub use job::JobProcessGuard;
pub use mutex::{EngineMutex, MutexRefusal};
pub use pipe::PipeServer;
pub use shares::share_connector;
pub use token::own_token;
pub(crate) use token::{elevation_type, linked_token, sid_string, token_of_process, Descriptor};
#[cfg(test)]
pub(crate) use token::{read_descriptor, read_handle, read_path, AclEntry};

/// winnt.h's `MAXIMUM_ALLOWED`: an access check returns every right granted (design part 3.6). Def: T3-7. Use: T3-7.
pub(crate) const MAXIMUM_ALLOWED: u32 = 0x0200_0000;
/// winnt.h's `SE_GROUP_INTEGRITY`, marking a token's integrity level (design part 3.6). Def: T3-7. Use: T3-7, T3-11.
pub(crate) const SE_GROUP_INTEGRITY: u32 = 0x20;

/// Turns Windows' critical-error box off for the whole process (design part 5.4; review round 1 of pull request 3,
/// V6): with `SEM_FAILCRITICALERRORS` set, a disc removed or swapped mid-read gives the reading thread an error instead
/// of a "No Disk" box that holds it until someone answers. The mode's other flags are kept, every thread of the
/// process reads it, and every child started afterwards inherits it. Def: PR #14 review round 1 (V6). Use: T3-23.
pub fn fail_critical_errors() {
    // SAFETY: both calls take and return flags only.
    unsafe { SetErrorMode(GetErrorMode() | SEM_FAILCRITICALERRORS) };
}

/// `text` as UTF-16 code units with a terminating NUL, for the wide-string arguments of Win32 calls (design part
/// 4.3). Def: T3-2. Use: T3-2 to T3-4, T3-7, T3-11 to T3-16.
pub(crate) fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// `units` up to its first NUL, or all of it when it holds none (design part 4.3). Def: T3-7. Use: T3-7, PR #14
/// review (F1, F2).
pub(crate) fn until_nul(units: &[u16]) -> &[u16] {
    let end = units.iter().position(|&unit| unit == 0).unwrap_or(units.len());
    &units[..end]
}

/// The string value `value` of the key `key` below the predefined root `root`, as UTF-16 units up to its first NUL,
/// read with `RegGetValueW` under `flags`: the `RRF_RT_*` types it may have, with `RRF_NOEXPAND` where a
/// `REG_EXPAND_SZ` value must come back unexpanded. None when it is absent, of another type, or cannot be read. The
/// call opens and closes the key itself, so no key handle is left open on any path (design parts 3.5, 3.6, 7.2;
/// whole-branch review F1, F2). Def: PR #14 review (F1). Use: T3-7 (`app_key`), PR #14 review (F1, F2).
pub(crate) fn registry_text(root: HKEY, key: &str, value: &str, flags: REG_ROUTINE_FLAGS) -> Option<Vec<u16>> {
    let (key, value) = (wide(key), wide(value));
    let mut size = 0u32;
    // SAFETY: both names are NUL-terminated and outlive the call; with no type or data pointer, the call writes only
    // the value's size in bytes to `size`.
    let status = unsafe { RegGetValueW(root, key.as_ptr(), value.as_ptr(), flags, null_mut(), null_mut(), &mut size) };
    if status != ERROR_SUCCESS {
        return None;
    }
    let mut data = vec![0u16; (size as usize).div_ceil(2).max(1)];
    let mut written = u32::try_from(data.len() * 2).ok()?;
    // SAFETY: as above; `data` holds `written` bytes, the size passed, and the call writes at most that many to it and
    // the count it wrote to `written`.
    let status = unsafe {
        RegGetValueW(root, key.as_ptr(), value.as_ptr(), flags, null_mut(), data.as_mut_ptr().cast(), &mut written)
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    // The count in a binding whose address is never taken, checked non-zero and bounded by the buffer before it bounds
    // the read.
    let units = written as usize / 2;
    if units == 0 {
        return None;
    }
    Some(until_nul(&data[..units.min(data.len())]).to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test first takes the flag off, since a process inherits its parent's mode and the SSH session's already
    /// holds it, so the call under test is what sets it.
    #[test]
    fn fail_critical_errors_adds_only_its_flag() {
        // SAFETY: GetErrorMode takes no argument and returns the process's flags.
        let inherited = unsafe { GetErrorMode() };
        println!("inherited error mode: {inherited:#x}");
        // SAFETY: SetErrorMode takes and returns flags only.
        unsafe { SetErrorMode(inherited & !SEM_FAILCRITICALERRORS) };
        // SAFETY: as GetErrorMode above.
        let before = unsafe { GetErrorMode() };
        assert_eq!(before & SEM_FAILCRITICALERRORS, 0, "the flag could not be taken off");
        fail_critical_errors();
        // SAFETY: as above.
        let after = unsafe { GetErrorMode() };
        assert_eq!(after, before | SEM_FAILCRITICALERRORS, "the mode before was {before:#x}");
        fail_critical_errors();
        // SAFETY: as above.
        assert_eq!(unsafe { GetErrorMode() }, after, "a second call changed the mode");
    }
}

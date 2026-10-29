//! The Windows implementations of lazymkv-platform's traits and helpers (design parts 4.2, 4.3). Every `windows-sys`
//! call of the workspace sits in this module, and each `unsafe` block carries a `// SAFETY:` line saying why the call
//! is sound. The rules these implementations apply (descriptor strings, the token verdict, the share and volume rules,
//! the drive diff, the local-time text, the eject report) sit in modules that compile and are tested on every system.
//! Compiled only on Windows.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
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
pub use host::WinHost;
pub use job::JobProcessGuard;
pub use mutex::{EngineMutex, MutexRefusal};
pub use pipe::PipeServer;
pub use shares::share_connector;
pub use token::own_token;
pub(crate) use token::{sid_string, token_facts, token_of_process, Descriptor};
#[cfg(test)]
pub(crate) use token::{read_descriptor, read_handle, read_path, AclEntry};

/// winnt.h's `MAXIMUM_ALLOWED`: an access check returns every right granted (design part 3.6). Def: T3-7. Use: T3-7.
pub(crate) const MAXIMUM_ALLOWED: u32 = 0x0200_0000;
/// winnt.h's `SE_GROUP_INTEGRITY`, marking a token's integrity level (design part 3.6). Def: T3-7. Use: T3-7, T3-11.
pub(crate) const SE_GROUP_INTEGRITY: u32 = 0x20;

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

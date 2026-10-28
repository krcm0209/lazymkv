//! The Windows implementations of lazymkv-platform's traits and helpers (design parts 4.2, 4.3). Every `windows-sys`
//! call of the workspace sits in this module, and each `unsafe` block carries a `// SAFETY:` line saying why the call
//! is sound. The rules these implementations apply (descriptor strings, the token verdict, the share and volume rules,
//! the drive diff, the local-time text, the eject report) sit in modules that compile and are tested on every system.
//! Compiled only on Windows.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

mod awake;
mod client;
mod creds;
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
pub use creds::CredentialStore;
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

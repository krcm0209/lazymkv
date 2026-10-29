//! The engine's named mutex (design parts 3.3, 3.8; Rulings W4, W5, T3-2-a, T3-2-b): created with a descriptor that
//! grants only SYSTEM and Administrators, never waited on, and, when the name is taken, read for who holds it.

use std::io;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER,
    ERROR_INVALID_HANDLE, ERROR_SUCCESS, HANDLE,
};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT};
use windows_sys::Win32::Security::{LookupAccountSidW, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID};
use windows_sys::Win32::Storage::FileSystem::READ_CONTROL;
use windows_sys::Win32::System::Threading::{CreateMutexW, OpenMutexW};

use super::{sid_string, wide, Descriptor};
use crate::security::{ADMINISTRATORS_SID, MUTEX_SDDL};

/// The engine's named mutex, held for the engine's life (design parts 3.3, 3.8). Its existence is the lock (Ruling
/// T3-2-a); `Drop` closes the handle, which releases the name once no process holds one. Def: T3-2. Use: T3-23.
pub struct EngineMutex {
    handle: HANDLE,
}

/// Why the mutex was not taken (design part 3.3; Rulings W4, W5, T3-2-b). Def: T3-2. Use: T3-23.
#[derive(Debug)]
pub enum MutexRefusal {
    /// The mutex exists and Administrators own it: another elevated engine made it.
    EngineRunning,
    /// Something else holds the name; `account` names its owner, for standard error only (Ruling W4).
    Squatter {
        /// The owner's account as `<domain>\<name>`, its SID string, or `UNREADABLE_ACCOUNT` (Ruling T3-2-b).
        account: String,
    },
    /// Windows refused the creation for another reason.
    Os(io::Error),
}

/// A squatter's account when its owner cannot be read (Ruling W5). Def: T3-2. Use: T3-2.
pub(crate) const UNREADABLE_ACCOUNT: &str = "an account lazymkv cannot read";

impl EngineMutex {
    /// Creates the mutex `name` from MUTEX_SDDL, or says who holds it (design part 3.3; Rulings W5, T3-2-b).
    /// Def: T3-2. Use: T3-23.
    pub fn take(name: &str) -> Result<EngineMutex, MutexRefusal> {
        let descriptor = Descriptor::from_sddl(MUTEX_SDDL).map_err(MutexRefusal::Os)?;
        let attributes = descriptor.attributes();
        let name = wide(name);
        // SAFETY: the attributes point at `descriptor`, which outlives the call; the name is NUL-terminated and
        // outlives it too. The mutex is not owned on creation (Ruling T3-2-a).
        let handle = unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) };
        // SAFETY: GetLastError takes nothing; it is read at once, before any other call.
        let code = unsafe { GetLastError() };
        if !handle.is_null() && code != ERROR_ALREADY_EXISTS {
            return Ok(EngineMutex { handle });
        }
        let existing = if !handle.is_null() {
            handle
        } else if code == ERROR_ACCESS_DENIED || code == ERROR_INVALID_HANDLE {
            // SAFETY: the name is NUL-terminated and outlives the call; a null result is checked below.
            let opened = unsafe { OpenMutexW(READ_CONTROL, 0, name.as_ptr()) };
            if opened.is_null() {
                return Err(MutexRefusal::Squatter { account: UNREADABLE_ACCOUNT.to_string() });
            }
            opened
        } else {
            return Err(MutexRefusal::Os(io::Error::from_raw_os_error(code as i32)));
        };
        let refusal = refusal_by_owner(existing);
        // SAFETY: `existing` came from CreateMutexW or OpenMutexW above and is closed once, here.
        unsafe { CloseHandle(existing) };
        Err(refusal)
    }
}

impl Drop for EngineMutex {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateMutexW in `take` and is closed once, here. The mutex was never owned
        // (Ruling T3-2-a), so there is nothing to release first.
        unsafe { CloseHandle(self.handle) };
    }
}

/// What the owner of the existing mutex `handle`, open with `READ_CONTROL`, decides (Ruling T3-2-b): Administrators
/// is `EngineRunning`, another owner a `Squatter` named by account, and an owner that cannot be read a `Squatter` with
/// `UNREADABLE_ACCOUNT`.
fn refusal_by_owner(handle: HANDLE) -> MutexRefusal {
    let unreadable = || MutexRefusal::Squatter { account: UNREADABLE_ACCOUNT.to_string() };
    let mut owner: PSID = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `handle` is an open mutex handle; the group and list pointers may be null; the call writes the owner,
    // which points into the descriptor it allocates with LocalAlloc.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return unreadable();
    }
    let sid = if owner.is_null() { None } else { sid_string(owner).ok() };
    let refusal = match sid {
        Some(sid) if sid == ADMINISTRATORS_SID => MutexRefusal::EngineRunning,
        Some(sid) => MutexRefusal::Squatter { account: account_name(owner).unwrap_or(sid) },
        None => unreadable(),
    };
    // SAFETY: GetSecurityInfo allocated the descriptor with LocalAlloc; it is freed once, here, after the last read of
    // `owner`, which points into it.
    unsafe { LocalFree(descriptor) };
    refusal
}

/// The account `sid` names, as `<domain>\<name>` or `<name>` when the domain is empty, or None when the SID has no
/// name or the lookup fails (Ruling T3-2-b).
fn account_name(sid: PSID) -> Option<String> {
    let (mut name_length, mut domain_length, mut kind) = (0u32, 0u32, 0);
    // SAFETY: `sid` points at a valid SID; null buffers with lengths 0 ask only for the lengths, which the call writes.
    let sized = unsafe {
        LookupAccountSidW(null(), sid, null_mut(), &mut name_length, null_mut(), &mut domain_length, &mut kind)
    };
    // SAFETY: GetLastError takes nothing; it is read at once, before any other call.
    if sized != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return None;
    }
    let mut name = vec![0u16; name_length.max(1) as usize];
    let mut domain = vec![0u16; domain_length.max(1) as usize];
    (name_length, domain_length) = (name.len() as u32, domain.len() as u32);
    // SAFETY: each buffer holds the length passed with it, in UTF-16 code units.
    let found = unsafe {
        LookupAccountSidW(
            null(),
            sid,
            name.as_mut_ptr(),
            &mut name_length,
            domain.as_mut_ptr(),
            &mut domain_length,
            &mut kind,
        )
    };
    if found == 0 {
        return None;
    }
    let name = String::from_utf16_lossy(&name[..name_length as usize]);
    let domain = String::from_utf16_lossy(&domain[..domain_length as usize]);
    Some(if domain.is_empty() { name } else { format!(r"{domain}\{name}") })
}

#[cfg(test)]
mod tests {
    use std::io;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::Authorization::SE_KERNEL_OBJECT;
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW};

    use super::*;
    use crate::security::{test_name, ADMINISTRATORS_SID, SYSTEM_SID};
    use crate::win::{own_token, read_handle, wide, Descriptor};

    const MUTEX_ALL_ACCESS: u32 = 0x001F_0001;
    const GENERIC_ALL: u32 = 0x1000_0000;

    /// A handle another process would hold on a name, closed on drop.
    struct Held(HANDLE);

    impl Drop for Held {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateMutexW or CreateEventW in this test and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Creates the mutex `name` with `sddl`, as a process other than the engine would.
    fn hold_mutex(name: &str, sddl: &str) -> Held {
        let descriptor = Descriptor::from_sddl(sddl).unwrap();
        let attributes = descriptor.attributes();
        let name = wide(name);
        // SAFETY: the attributes point at `descriptor`, which outlives the call; the name is NUL-terminated.
        let handle = unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) };
        assert!(!handle.is_null(), "CreateMutexW: {}", io::Error::last_os_error());
        Held(handle)
    }

    /// The result's variant, for messages that never print a squatter's account (Ruling W4).
    fn kind(result: &Result<EngineMutex, MutexRefusal>) -> &'static str {
        match result {
            Ok(_) => "Ok",
            Err(MutexRefusal::EngineRunning) => "EngineRunning",
            Err(MutexRefusal::Squatter { .. }) => "Squatter",
            Err(MutexRefusal::Os(_)) => "Os",
        }
    }

    /// The mask as the kernel holds it once GA is mapped to the mutex's own rights.
    fn mapped(mask: u32) -> u32 {
        if mask == GENERIC_ALL { MUTEX_ALL_ACCESS } else { mask }
    }

    #[test]
    fn a_second_take_of_a_held_mutex_says_an_engine_is_running() {
        let name = format!(r"Global\{}", test_name("mutex"));
        let first = EngineMutex::take(&name);
        assert_eq!(kind(&first), "Ok");
        assert_eq!(kind(&EngineMutex::take(&name)), "EngineRunning");
    }

    #[test]
    fn the_mutex_is_free_again_once_dropped() {
        let name = format!(r"Global\{}", test_name("mutex-free"));
        let first = EngineMutex::take(&name);
        assert_eq!(kind(&first), "Ok");
        drop(first);
        assert_eq!(kind(&EngineMutex::take(&name)), "Ok");
    }

    #[test]
    fn a_mutex_owned_by_the_user_is_a_squatter_named_by_account() {
        let user = own_token().unwrap().user_sid;
        let name = format!(r"Global\{}", test_name("mutex-user"));
        let _held = hold_mutex(&name, &format!("O:{user}D:P(A;;GA;;;{user})(A;;GA;;;SY)(A;;GA;;;BA)"));
        match EngineMutex::take(&name) {
            Err(MutexRefusal::Squatter { account }) => {
                assert!(!account.is_empty() && account != UNREADABLE_ACCOUNT, "the squatter's account is not named")
            }
            other => panic!("expected Squatter, got {}", kind(&other)),
        }
    }

    #[test]
    fn a_mutex_whose_owner_cannot_be_read_is_a_squatter_lazymkv_cannot_read() {
        // OW is OWNER RIGHTS: denying it takes the owner's implicit right to read the descriptor away too.
        let user = own_token().unwrap().user_sid;
        let name = format!(r"Global\{}", test_name("mutex-dark"));
        let _held = hold_mutex(&name, &format!("O:{user}D:P(D;;GA;;;OW)"));
        match EngineMutex::take(&name) {
            Err(MutexRefusal::Squatter { account }) => {
                assert!(account == UNREADABLE_ACCOUNT, "the account is not UNREADABLE_ACCOUNT")
            }
            other => panic!("expected Squatter, got {}", kind(&other)),
        }
    }

    #[test]
    fn a_name_held_by_another_kind_of_object_is_a_squatter_lazymkv_cannot_read() {
        let name = format!(r"Global\{}", test_name("mutex-event"));
        let wide_name = wide(&name);
        // SAFETY: default security, a manual-reset event not signalled; the name is NUL-terminated.
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide_name.as_ptr()) };
        assert!(!event.is_null(), "CreateEventW: {}", io::Error::last_os_error());
        let _held = Held(event);
        match EngineMutex::take(&name) {
            Err(MutexRefusal::Squatter { account }) => {
                assert!(account == UNREADABLE_ACCOUNT, "the account is not UNREADABLE_ACCOUNT")
            }
            other => panic!("expected Squatter, got {}", kind(&other)),
        }
    }

    #[test]
    fn the_created_mutex_is_owned_by_administrators_and_grants_only_system_and_administrators() {
        let name = format!(r"Global\{}", test_name("mutex-list"));
        let taken = EngineMutex::take(&name);
        assert_eq!(kind(&taken), "Ok");
        let mutex = taken.ok().unwrap();
        let read = read_handle(mutex.handle, SE_KERNEL_OBJECT).unwrap();
        assert_eq!(read.owner, ADMINISTRATORS_SID);
        // Type 0 is an allow entry; the flags are 0, since a mutex has nothing below it to inherit.
        let entries: Vec<(u8, u8, u32, &str)> =
            read.entries.iter().map(|e| (e.kind, e.flags, mapped(e.mask), e.sid.as_str())).collect();
        assert_eq!(entries, vec![(0, 0, MUTEX_ALL_ACCESS, SYSTEM_SID), (0, 0, MUTEX_ALL_ACCESS, ADMINISTRATORS_SID)]);
    }
}

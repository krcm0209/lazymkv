//! A process's token and the security descriptors lazymkv builds (design parts 3.2 to 3.5): the facts of a token, a
//! SID's string form, and a descriptor built from SDDL (Security Descriptor Definition Language). For the tests, it
//! also reads an object's owner and access list back as strings.

use std::io;
#[cfg(test)]
use std::path::Path;
use std::ptr::null_mut;

use windows_sys::core::PWSTR;
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_INSUFFICIENT_BUFFER, HANDLE};
#[cfg(test)]
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
#[cfg(test)]
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, GetSecurityInfo, SE_FILE_OBJECT, SE_OBJECT_TYPE,
};
use windows_sys::Win32::Security::{
    GetSecurityDescriptorDacl, GetTokenInformation, TokenElevation, TokenElevationType, TokenIntegrityLevel,
    TokenLinkedToken, TokenUser, ACL, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_ELEVATION,
    TOKEN_ELEVATION_TYPE, TOKEN_INFORMATION_CLASS, TOKEN_LINKED_TOKEN, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TOKEN_USER,
};
#[cfg(test)]
use windows_sys::Win32::Security::{
    GetAce, GetSecurityDescriptorControl, GetSecurityDescriptorOwner, ACCESS_ALLOWED_ACE, ACE_HEADER,
    DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, SE_DACL_PROTECTED,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::wide;
use crate::security::{ElevationType, TokenFacts};

/// The facts of this process's token (design part 3.2): the engine's start check, and the confirmation that the mini
/// PC's SSH session is elevated. Def: T3-2. Use: T3-3, T3-11, T3-12, T3-23.
pub fn own_token() -> io::Result<TokenFacts> {
    // SAFETY: GetCurrentProcess takes nothing and returns a pseudo handle that needs no closing.
    facts_of_process(unsafe { GetCurrentProcess() })
}

/// The facts of `token`, opened with `TOKEN_QUERY` (design parts 3.2, 3.4). Def: T3-2. Use: T3-2, T3-7.
pub(crate) fn token_facts(token: HANDLE) -> io::Result<TokenFacts> {
    let user = variable(token, TokenUser)?;
    // SAFETY: a TokenUser read starts with a TOKEN_USER, whose SID points into the same buffer, alive here.
    let user_sid = sid_string(unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid })?;
    let elevation: TOKEN_ELEVATION = fixed(token, TokenElevation)?;
    // 1, 2 and 3 are TokenElevationTypeDefault, TokenElevationTypeFull and TokenElevationTypeLimited.
    let elevation_type = match fixed::<TOKEN_ELEVATION_TYPE>(token, TokenElevationType)? {
        1 => ElevationType::Default,
        2 => ElevationType::Full,
        3 => ElevationType::Limited,
        other => {
            let text = format!("the token's elevation type is {other}, not 1, 2 or 3");
            return Err(io::Error::new(io::ErrorKind::InvalidData, text));
        }
    };
    let label = variable(token, TokenIntegrityLevel)?;
    // SAFETY: a TokenIntegrityLevel read starts with a TOKEN_MANDATORY_LABEL, whose SID points into the same buffer.
    let integrity_sid = sid_string(unsafe { (*label.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()).Label.Sid })?;
    // Ruling T3-2-c: linked when the call returns a token, which is closed at once.
    let linked = match fixed::<TOKEN_LINKED_TOKEN>(token, TokenLinkedToken) {
        Ok(link) if !link.LinkedToken.is_null() => {
            // SAFETY: the call opened the linked token for this process; it is closed once, here.
            unsafe { CloseHandle(link.LinkedToken) };
            true
        }
        _ => false,
    };
    Ok(TokenFacts { user_sid, elevated: elevation.TokenIsElevated != 0, elevation_type, integrity_sid, linked })
}

/// The facts of process `pid`'s token: the pipe's client (T3-11) and its server (T3-12) (design parts 3.3, 3.4).
/// Def: T3-2. Use: T3-11, T3-12.
pub(crate) fn token_of_process(pid: u32) -> io::Result<TokenFacts> {
    // SAFETY: OpenProcess takes no pointer; a null result is checked below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let facts = facts_of_process(process);
    // SAFETY: `process` came from OpenProcess above and is closed once, here.
    unsafe { CloseHandle(process) };
    facts
}

/// The facts of the token of `process`, a handle open with at least `PROCESS_QUERY_LIMITED_INFORMATION`; the token is
/// opened with `TOKEN_QUERY` and closed on every path.
fn facts_of_process(process: HANDLE) -> io::Result<TokenFacts> {
    let mut token: HANDLE = null_mut();
    // SAFETY: `process` is an open process handle, and the call writes the token's handle to `token`.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let facts = token_facts(token);
    // SAFETY: `token` came from OpenProcessToken above and is closed once, here.
    unsafe { CloseHandle(token) };
    facts
}

/// The variable-size information `class` of `token`, read with two calls, the first for the size, into a buffer of
/// `u64`s so that the structure at its start is aligned.
fn variable(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> io::Result<Vec<u64>> {
    let mut size = 0u32;
    // SAFETY: a null buffer of length 0 asks only for the size, which the call writes to `size`.
    if unsafe { GetTokenInformation(token, class, null_mut(), 0, &mut size) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
            return Err(error);
        }
    }
    let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: the buffer holds at least `size` bytes, the length passed.
    if unsafe { GetTokenInformation(token, class, buffer.as_mut_ptr().cast(), size, &mut size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buffer)
}

/// The fixed-size information `class` of `token`, read with one call into the `T` that class writes.
fn fixed<T: Default>(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> io::Result<T> {
    let mut value = T::default();
    let (pointer, length, mut size) = ((&mut value as *mut T).cast(), size_of::<T>() as u32, 0u32);
    // SAFETY: every caller names the `T` that `class` writes, and `length` is the size of `value`, which `pointer`
    // points at.
    if unsafe { GetTokenInformation(token, class, pointer, length, &mut size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(value)
}

/// `sid` in its string form, `S-1-5-...` (design parts 3.3, 3.5). Def: T3-2. Use: T3-2, T3-3.
pub(crate) fn sid_string(sid: PSID) -> io::Result<String> {
    let mut text: PWSTR = null_mut();
    // SAFETY: `sid` points at a valid SID, as every caller passes; the call writes the address of the string it
    // allocates with LocalAlloc to `text`.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `text` is the NUL-terminated string the call allocated; it is read up to its NUL, copied, and freed once.
    let string = unsafe {
        let length = (0..).take_while(|&index| *text.add(index) != 0).count();
        let string = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
        LocalFree(text.cast());
        string
    };
    Ok(string)
}

/// A security descriptor built from an SDDL string, freed with `LocalFree` on drop (design parts 3.3 to 3.5).
/// Def: T3-2. Use: T3-2, T3-3, T3-11.
pub(crate) struct Descriptor(PSECURITY_DESCRIPTOR);

impl Descriptor {
    /// The descriptor `sddl` describes, or Windows' refusal of the string (design parts 3.3 to 3.5).
    /// Def: T3-2. Use: T3-2, T3-3, T3-11.
    pub(crate) fn from_sddl(sddl: &str) -> io::Result<Descriptor> {
        let text = wide(sddl);
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: `text` is NUL-terminated and outlives the call; the call writes the descriptor it allocates with
        // LocalAlloc to `descriptor`, and the size pointer may be null.
        let made = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if made == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Descriptor(descriptor))
    }

    /// The descriptor itself, for a test's call that reads it (design part 3.5). Def: T3-2. Use: T3-3.
    #[cfg(test)]
    pub(crate) fn as_ptr(&self) -> PSECURITY_DESCRIPTOR {
        self.0
    }

    /// Security attributes that point at this descriptor, with a handle no child inherits; the descriptor must
    /// outlive the call they are passed to (design parts 3.3 to 3.5). Def: T3-2. Use: T3-2, T3-3, T3-11.
    pub(crate) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        }
    }

    /// The descriptor's access list, or null when it has none (design part 3.5). Def: T3-2. Use: T3-3.
    pub(crate) fn dacl(&self) -> *mut ACL {
        dacl_of(self.0)
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: the descriptor came from ConvertStringSecurityDescriptorToSecurityDescriptorW, which allocated it
        // with LocalAlloc, and is freed once, here.
        unsafe { LocalFree(self.0) };
    }
}

// SAFETY: the descriptor is written only by the conversion that made it and is read-only after; LocalFree runs once,
// in Drop. PipeServer (an IpcTransport: Send + Sync) holds one (T3-11).
unsafe impl Send for Descriptor {}
// SAFETY: as for Send: no call through `&Descriptor` writes the descriptor.
unsafe impl Sync for Descriptor {}

/// The access list of `descriptor`, or null when it reports none.
fn dacl_of(descriptor: PSECURITY_DESCRIPTOR) -> *mut ACL {
    let (mut present, mut dacl, mut defaulted) = (0, null_mut(), 0);
    // SAFETY: `descriptor` is a valid descriptor, as every caller passes; the call writes the three values.
    let read = unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) };
    if read == 0 || present == 0 {
        null_mut()
    } else {
        dacl
    }
}

/// One entry of an access list as a test reads it back (design parts 3.3 to 3.5). Def: T3-2.
/// Use: T3-2, T3-3, T3-11.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AclEntry {
    /// The entry's type: 0 allows, 1 denies.
    pub(crate) kind: u8,
    /// The entry's inheritance flags.
    pub(crate) flags: u8,
    /// The rights the entry allows or denies.
    pub(crate) mask: u32,
    /// The SID the entry names, in its string form.
    pub(crate) sid: String,
}

/// An object's owner and access list as a test reads them back (design parts 3.3 to 3.5). Def: T3-2.
/// Use: T3-2, T3-3, T3-11.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReadBack {
    /// The owner's SID string, or "" when the descriptor has no owner.
    pub(crate) owner: String,
    /// Whether the access list is protected from the parent's.
    pub(crate) protected: bool,
    /// The access list's entries, in order.
    pub(crate) entries: Vec<AclEntry>,
}

/// The owner, the protection and the entries of `descriptor`; an absent owner reads "" (design parts 3.3 to 3.5).
/// It panics on an entry that neither allows nor denies, since no lazymkv list holds one. Def: T3-2. Use: T3-2, T3-3.
#[cfg(test)]
pub(crate) fn read_descriptor(descriptor: PSECURITY_DESCRIPTOR) -> ReadBack {
    // ACCESS_ALLOWED_ACE_TYPE and ACCESS_DENIED_ACE_TYPE, whose entries share ACCESS_ALLOWED_ACE's layout.
    const ALLOWED: u8 = 0;
    const DENIED: u8 = 1;
    let (mut owner_sid, mut defaulted): (PSID, _) = (null_mut(), 0);
    // SAFETY: `descriptor` is a valid descriptor, as every caller passes; the call writes the two values.
    let read = unsafe { GetSecurityDescriptorOwner(descriptor, &mut owner_sid, &mut defaulted) };
    assert_ne!(read, 0, "GetSecurityDescriptorOwner: {}", io::Error::last_os_error());
    let owner = if owner_sid.is_null() { String::new() } else { sid_string(owner_sid).expect("the owner's SID") };
    let (mut control, mut revision) = (0u16, 0u32);
    // SAFETY: as above; the call writes the control bits and the revision.
    let read = unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) };
    assert_ne!(read, 0, "GetSecurityDescriptorControl: {}", io::Error::last_os_error());
    let dacl = dacl_of(descriptor);
    let mut entries = Vec::new();
    // SAFETY: a non-null `dacl` points at the descriptor's access list, alive as long as the descriptor.
    let count = if dacl.is_null() { 0 } else { unsafe { (*dacl).AceCount } };
    for index in 0..u32::from(count) {
        let mut ace = null_mut();
        // SAFETY: `index` is below the list's AceCount; the call writes the entry's address to `ace`.
        assert_ne!(unsafe { GetAce(dacl, index, &mut ace) }, 0, "GetAce: {}", io::Error::last_os_error());
        // The entry as its header, in a binding whose address is never taken, so that the code scanner sees the
        // null check below cover both reads in its branch.
        let ace = ace.cast::<ACE_HEADER>();
        let (header, mask) = if ace.is_null() {
            panic!("GetAce wrote no entry")
        } else {
            // SAFETY: a non-null `ace` points at the entry GetAce found, and every entry starts with an ACE_HEADER.
            let header = unsafe { *ace };
            let kind = header.AceType;
            assert!(matches!(kind, ALLOWED | DENIED), "entry type {kind} is neither allow nor deny");
            // SAFETY: as above, and an allow or deny entry is an ACCESS_ALLOWED_ACE.
            (header, unsafe { (*ace.cast::<ACCESS_ALLOWED_ACE>()).Mask })
        };
        // An ACCESS_ALLOWED_ACE's SID starts at SidStart, inside the list.
        let sid = ace.cast::<u8>().wrapping_add(std::mem::offset_of!(ACCESS_ALLOWED_ACE, SidStart)).cast();
        let sid = sid_string(sid).expect("the entry's SID");
        entries.push(AclEntry { kind: header.AceType, flags: header.AceFlags, mask, sid });
    }
    ReadBack { owner, protected: control & SE_DACL_PROTECTED != 0, entries }
}

/// The owner and access list of the object `handle`, open with `READ_CONTROL`, of type `object` (design parts 3.3,
/// 3.4). Def: T3-2. Use: T3-2, T3-11.
#[cfg(test)]
pub(crate) fn read_handle(handle: HANDLE, object: SE_OBJECT_TYPE) -> io::Result<ReadBack> {
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `handle` is open with READ_CONTROL, as every caller passes; the owner, group and list pointers may be
    // null, since the descriptor holds them; the call writes the descriptor it allocates with LocalAlloc.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            object,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    read_back(status, descriptor)
}

/// The owner and access list of the file or folder `path` (design part 3.5). Def: T3-2. Use: T3-3.
#[cfg(test)]
pub(crate) fn read_path(path: &Path) -> io::Result<ReadBack> {
    let name = wide(path);
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `name` is NUL-terminated and outlives the call; the rest is as in `read_handle`.
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    read_back(status, descriptor)
}

/// The read-back of the descriptor that a security-information call returned with `status`, freed once it is read.
#[cfg(test)]
fn read_back(status: WIN32_ERROR, descriptor: PSECURITY_DESCRIPTOR) -> io::Result<ReadBack> {
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let read = read_descriptor(descriptor);
    // SAFETY: the call allocated the descriptor with LocalAlloc; it is freed once, here.
    unsafe { LocalFree(descriptor) };
    Ok(read)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn the_process_token_is_read_and_printed() {
        let facts = own_token().unwrap();
        let line = format!(
            "token facts: elevated={} type={:?} integrity={} linked={}\n",
            facts.elevated, facts.elevation_type, facts.integrity_sid, facts.linked
        );
        std::io::stderr().write_all(line.as_bytes()).unwrap(); // Ruling T3-2-d; the user SID is not printed
        assert!(facts.user_sid.starts_with("S-1-5-"), "the user SID is not an account SID");
        assert!(facts.integrity_sid.starts_with("S-1-16-"), "{}", facts.integrity_sid);
        if std::env::var_os("CI").is_some() || std::env::var_os("SSH_CONNECTION").is_some() {
            assert!(facts.elevated, "design part 3.2: the runner and the mini PC's SSH session run elevated");
        }
    }

    #[test]
    fn the_token_of_this_process_read_by_its_id_equals_the_process_token() {
        assert!(token_of_process(std::process::id()).unwrap() == own_token().unwrap());
    }
}

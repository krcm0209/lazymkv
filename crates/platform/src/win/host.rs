//! The engine's `Host` on Windows (design part 4.2's helpers outside the traits; parts 3.6, 4.6, 7.2; Rulings W17,
//! W18): the clock and the local time, MakeMKV's registration key and debug log, a Toolhelp process snapshot, the
//! volume label, the Program Files install paths from the registry, and the writable test with the user's filtered
//! token.

use std::ffi::{c_void, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::time::{SystemTime, UNIX_EPOCH};

use lazymkv_core::job::Tool;
use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS, FILETIME, HANDLE, INVALID_HANDLE_VALUE, SYSTEMTIME};
use windows_sys::Win32::Security::Authorization::{ConvertStringSidToSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    AccessCheck, CreateRestrictedToken, DuplicateTokenEx, GetTokenInformation, SecurityIdentification,
    SetTokenInformation, TokenImpersonation, TokenIntegrityLevel, TokenLinkedToken, DACL_SECURITY_INFORMATION,
    GENERIC_MAPPING, GROUP_SECURITY_INFORMATION, LABEL_SECURITY_INFORMATION, LUA_TOKEN, OWNER_SECURITY_INFORMATION,
    PRIVILEGE_SET, PSECURITY_DESCRIPTOR, PSID, SID_AND_ATTRIBUTES, TOKEN_ADJUST_DEFAULT, TOKEN_DUPLICATE,
    TOKEN_IMPERSONATE, TOKEN_LINKED_TOKEN, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetVolumeInformationW, DELETE, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY, FILE_ALL_ACCESS, FILE_APPEND_DATA,
    FILE_DELETE_CHILD, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_WRITE_DATA, WRITE_DAC,
    WRITE_OWNER,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_SZ};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime};

use super::{registry_text, token_facts, until_nul, wide, MAXIMUM_ALLOWED, SE_GROUP_INTEGRITY};
use crate::localtime::format_local_time;
use crate::security::ElevationType;
use crate::Host;

/// Milliseconds from 1601-01-01, where `FILETIME` counts from, to the Unix epoch.
const FILETIME_EPOCH_MS: u64 = 11_644_473_600_000;
/// `FILETIME` ticks, of 100 nanoseconds each, in a millisecond.
const TICKS_PER_MS: u64 = 10_000;
/// `FILETIME` ticks in a minute.
const TICKS_PER_MINUTE: i128 = 600_000_000;
/// The SID (security identifier) of the medium mandatory integrity level, a non-elevated process's level.
const MEDIUM_INTEGRITY_SID: &str = "S-1-16-8192";
/// The rights that make a folder writable (Ruling W17): add a file or a folder, delete a child, delete it, or change
/// its access list or owner.
const FOLDER_WRITE_RIGHTS: u32 =
    FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD | DELETE | WRITE_DAC | WRITE_OWNER;
/// The rights that make a file writable (Ruling W17): write, append, delete it, or change its access list or owner.
const FILE_WRITE_RIGHTS: u32 = FILE_WRITE_DATA | FILE_APPEND_DATA | DELETE | WRITE_DAC | WRITE_OWNER;
/// The `u32`s of the privilege-set buffer `AccessCheck` writes: 256 bytes, as `u32`s so that the `PRIVILEGE_SET` at
/// its start is aligned.
const PRIVILEGE_UNITS: usize = 64;
/// The units of a volume-name buffer: `MAX_PATH` and its NUL.
const LABEL_UNITS: usize = 261;
/// The key below `HKEY_LOCAL_MACHINE` whose values `ProgramFilesDir (x86)` and `ProgramFilesDir` name the Program Files
/// folders (design part 3.6; Ruling W18, amended after the whole-branch review (F1)).
const CURRENT_VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion";

/// The engine's `Host` on Windows (design part 4.2's helpers outside the traits; parts 3.6, 4.6, 7.2; Rulings W17,
/// W18): the system clock, the local time from the time-zone rules, MakeMKV's key from the registry, a Toolhelp
/// process snapshot, the volume label, the Program Files install paths from the registry, never from the environment,
/// and the writable test with the user's filtered token. It holds no handle between calls. Def: T3-7. Use: T3-23.
pub struct WinHost;

impl WinHost {
    /// A host over this machine (design part 4.2). Def: T3-7. Use: T3-23.
    pub fn new() -> WinHost {
        WinHost
    }
}

impl Host for WinHost {
    fn now_unix(&self) -> u64 {
        self.now_ms() / 1000
    }

    /// The system clock; a clock set before 1970 reads 0, since the trait has no error.
    fn now_ms(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
    }

    fn local_date(&self) -> String {
        let mut text = self.local_time(self.now_ms());
        text.truncate(10);
        text
    }

    /// One formatter serves every host: Windows gives only the offset at `unix_ms`, 0 when it cannot.
    fn local_time(&self, unix_ms: u64) -> String {
        format_local_time(unix_ms, offset_minutes(unix_ms).unwrap_or(0))
    }

    fn makemkv_app_key(&self) -> Option<String> {
        app_key()
    }

    /// `%USERPROFILE%\MakeMKV_log.txt`, where MakeMKV 2.0.0 writes its debug log (design part 7.2).
    fn makemkv_debug_log(&self) -> Option<PathBuf> {
        std::env::var_os("USERPROFILE").map(|profile| PathBuf::from(profile).join("MakeMKV_log.txt"))
    }

    /// An empty list when the snapshot fails, since the trait has no error (S55's check then finds no MakeMKV
    /// process).
    fn running_processes(&self) -> Vec<String> {
        process_names()
    }

    fn volume_label(&self, root: &Path) -> Option<String> {
        label_of(root)
    }

    /// Design part 3.6; Ruling W18, amended after the whole-branch review (F1): the Program Files folder is the value
    /// `ProgramFilesDir (x86)` (MakeMKV) or `ProgramFilesDir` (MKVToolNix) under `HKEY_LOCAL_MACHINE`'s
    /// `CURRENT_VERSION_KEY`, read as `REG_SZ` with `RRF_NOEXPAND`, and never the `ProgramFiles(x86)` or
    /// `ProgramFiles` variable, which a non-elevated process of the user can set for the elevated engine. None for a
    /// value that cannot be read, is empty, still holds a `%` or is not absolute, since the folder must not come back
    /// from the environment through an expansion.
    fn default_install_path(&self, tool: Tool) -> Option<PathBuf> {
        let (value, below) = match tool {
            Tool::Makemkvcon => ("ProgramFilesDir (x86)", r"MakeMKV\makemkvcon64.exe"),
            Tool::Mkvmerge => ("ProgramFilesDir", r"MKVToolNix\mkvmerge.exe"),
            Tool::Ffprobe | Tool::Ffmpeg | Tool::Java => return None,
        };
        let folder = registry_text(HKEY_LOCAL_MACHINE, CURRENT_VERSION_KEY, value, RRF_RT_REG_SZ | RRF_NOEXPAND)?;
        if folder.contains(&u16::from(b'%')) {
            return None;
        }
        let folder = PathBuf::from(OsString::from_wide(&folder));
        // An empty value gives an empty path, which is not absolute either.
        folder.is_absolute().then(|| folder.join(below))
    }

    /// Ruling W17: true at the first call that fails, so a lookup never trusts what it could not judge.
    fn user_writable(&self, path: &Path) -> bool {
        writable(path).unwrap_or(true)
    }
}

/// The local time's offset from UTC (Coordinated Universal Time) in minutes at `unix_ms`, under the time zone's rule
/// for that instant (design part 4.6); None when the instant is beyond `FILETIME` or a call fails.
fn offset_minutes(unix_ms: u64) -> Option<i32> {
    let ticks = unix_ms.checked_add(FILETIME_EPOCH_MS)?.checked_mul(TICKS_PER_MS)?;
    let utc_file = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    let (mut utc, mut local, mut local_file) = (SYSTEMTIME::default(), SYSTEMTIME::default(), FILETIME::default());
    // SAFETY: each call reads and writes live locals of the types it declares; the null zone pointer selects the
    // current time zone.
    let converted = unsafe {
        FileTimeToSystemTime(&utc_file, &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(null(), &utc, &mut local) != 0
            && SystemTimeToFileTime(&local, &mut local_file) != 0
    };
    if !converted {
        return None;
    }
    let local_ticks = (u64::from(local_file.dwHighDateTime) << 32) | u64::from(local_file.dwLowDateTime);
    i32::try_from((i128::from(local_ticks) - i128::from(ticks)) / TICKS_PER_MINUTE).ok()
}

/// MakeMKV's registration key, `app_Key` under `HKCU\Software\MakeMKV` (design part 7.2), or None when it is unset,
/// empty or cannot be read. The value is never formatted into a log line, an error or a message (S61).
fn app_key() -> Option<String> {
    let text = registry_text(HKEY_CURRENT_USER, r"Software\MakeMKV", "app_Key", RRF_RT_REG_SZ)?;
    (!text.is_empty()).then(|| String::from_utf16_lossy(&text))
}

/// The image names of the running processes from a Toolhelp snapshot (design part 5.3), or an empty list when the
/// snapshot cannot be taken.
fn process_names() -> Vec<String> {
    // SAFETY: CreateToolhelp32Snapshot takes no pointer; an INVALID_HANDLE_VALUE result is checked below.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Vec::new();
    }
    // SAFETY: raw is an open snapshot handle that nothing else owns.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..PROCESSENTRY32W::default() };
    let mut names = Vec::new();
    // SAFETY: the snapshot is open, and `entry` is a PROCESSENTRY32W whose dwSize says its size; it is read only after
    // a call that returned non-zero filled it.
    let mut filled = unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) };
    while filled != 0 {
        names.push(String::from_utf16_lossy(until_nul(&entry.szExeFile)));
        // SAFETY: as for Process32FirstW.
        filled = unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) };
    }
    names
}

/// The volume label of the volume whose root is `root`, with a `\` appended when it has none; Some(the label,
/// possibly empty) when Windows answers, None when it does not. No drive serial is read.
fn label_of(root: &Path) -> Option<String> {
    let mut text = root.as_os_str().to_os_string();
    if text.encode_wide().last() != Some(u16::from(b'\\')) {
        text.push(r"\");
    }
    let name = wide(text);
    let mut label = [0u16; LABEL_UNITS];
    // SAFETY: `name` is NUL-terminated and outlives the call; `label` holds the LABEL_UNITS units passed; the serial,
    // component-length and flag pointers are null and the file-system name has no buffer and a size of 0, so the call
    // writes the label only.
    let read = unsafe {
        GetVolumeInformationW(
            name.as_ptr(),
            label.as_mut_ptr(),
            LABEL_UNITS as u32,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
        )
    };
    (read != 0).then(|| String::from_utf16_lossy(until_nul(&label)))
}

/// Memory a Windows call allocated with LocalAlloc, freed with `LocalFree` on drop.
struct LocalMemory(*mut c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // SAFETY: every `LocalMemory` holds a non-null pointer that a call allocated with LocalAlloc; it is freed once,
        // here.
        unsafe { LocalFree(self.0) };
    }
}

/// `raw`, a handle a call just wrote, owned so that it closes on drop; an error when the call wrote none.
fn owned(raw: HANDLE) -> io::Result<OwnedHandle> {
    if raw.is_null() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Windows returned no handle"));
    }
    // SAFETY: `raw` is an open handle that nothing else owns.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

/// Ruling W17's test (design part 3.6): whether the user's filtered token is granted any write right on `path`; Err
/// at the first call that fails.
fn writable(path: &Path) -> io::Result<bool> {
    let descriptor = descriptor_of(path)?;
    let token = filtered_token()?;
    let granted = granted_access(&descriptor, &token)?;
    let wanted = if path.is_dir() { FOLDER_WRITE_RIGHTS } else { FILE_WRITE_RIGHTS };
    Ok(granted & wanted != 0)
}

/// The owner, group, access list and mandatory label of `path`: `AccessCheck` needs the owner and the group, and the
/// label makes it apply the no-write-up rule.
fn descriptor_of(path: &Path) -> io::Result<LocalMemory> {
    let name = wide(path);
    let parts = OWNER_SECURITY_INFORMATION
        | GROUP_SECURITY_INFORMATION
        | DACL_SECURITY_INFORMATION
        | LABEL_SECURITY_INFORMATION;
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `name` is NUL-terminated and outlives the call; the owner, group and list pointers may be null, since the
    // descriptor holds them; the call writes the descriptor it allocates with LocalAlloc to `descriptor`.
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            parts,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    // The descriptor in a binding whose address is never taken, checked before it is owned.
    let descriptor = descriptor;
    if descriptor.is_null() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Windows returned no security descriptor"));
    }
    Ok(LocalMemory(descriptor))
}

/// The user's filtered token, the token a non-elevated process of the user gets (design part 3.6, Ruling W17), as an
/// identification-level impersonation token at medium integrity: the linked token when this process's token is the
/// elevated half of a split token, else a limited copy of this process's token.
fn filtered_token() -> io::Result<OwnedHandle> {
    let mut raw: HANDLE = null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo handle that needs no closing; the call writes the token's handle to
    // `raw`.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut raw) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let own = owned(raw)?;
    if token_facts(own.as_raw_handle())?.elevation_type == ElevationType::Full {
        linked_token(&own)
    } else {
        limited_token(&own)
    }
}

/// The linked token of `token`, the elevated half of a split token: the filtered half, which Windows returns as an
/// identification-level impersonation token at medium integrity.
fn linked_token(token: &OwnedHandle) -> io::Result<OwnedHandle> {
    let mut link = TOKEN_LINKED_TOKEN::default();
    let (length, mut written) = (size_of::<TOKEN_LINKED_TOKEN>() as u32, 0u32);
    // SAFETY: the token is open with TOKEN_QUERY; `link` is the TOKEN_LINKED_TOKEN that TokenLinkedToken writes, and
    // `length` is its size; the call opens the linked token for this process and writes its handle into `link`.
    let read = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenLinkedToken,
            (&mut link as *mut TOKEN_LINKED_TOKEN).cast(),
            length,
            &mut written,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    owned(link.LinkedToken)
}

/// A limited copy of `token`, open with `TOKEN_DUPLICATE`: `CreateRestrictedToken(LUA_TOKEN)` (LUA: limited user
/// account) makes the Administrators group deny-only, `DuplicateTokenEx` makes it an identification-level impersonation
/// token, which `AccessCheck` takes, and its integrity level is set to medium.
fn limited_token(token: &OwnedHandle) -> io::Result<OwnedHandle> {
    let mut raw: HANDLE = null_mut();
    // SAFETY: the token is open with TOKEN_DUPLICATE; no SID or privilege list is passed, so each count is 0 and each
    // pointer null; the call writes the new token's handle to `raw`.
    let made = unsafe {
        CreateRestrictedToken(token.as_raw_handle(), LUA_TOKEN, 0, null(), 0, null(), 0, null(), &mut raw)
    };
    if made == 0 {
        return Err(io::Error::last_os_error());
    }
    let restricted = owned(raw)?;
    let mut raw: HANDLE = null_mut();
    let access = TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_IMPERSONATE;
    // SAFETY: the restricted token has the access of the token it came from, TOKEN_DUPLICATE among it; the attributes
    // pointer may be null, for a default descriptor and a handle no child inherits; the call writes the copy's handle
    // to `raw`.
    let copied = unsafe {
        DuplicateTokenEx(
            restricted.as_raw_handle(),
            access,
            null(),
            SecurityIdentification,
            TokenImpersonation,
            &mut raw,
        )
    };
    if copied == 0 {
        return Err(io::Error::last_os_error());
    }
    let limited = owned(raw)?;
    set_medium_integrity(&limited)?;
    Ok(limited)
}

/// Sets `token`'s integrity level, open with `TOKEN_ADJUST_DEFAULT`, to medium.
fn set_medium_integrity(token: &OwnedHandle) -> io::Result<()> {
    let text = wide(MEDIUM_INTEGRITY_SID);
    let mut sid: PSID = null_mut();
    // SAFETY: `text` is NUL-terminated and outlives the call; the call writes the SID it allocates with LocalAlloc to
    // `sid`.
    if unsafe { ConvertStringSidToSidW(text.as_ptr(), &mut sid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // The SID in a binding whose address is never taken, checked before it is owned.
    let sid = sid;
    if sid.is_null() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Windows returned no SID"));
    }
    let sid = LocalMemory(sid);
    let label = TOKEN_MANDATORY_LABEL { Label: SID_AND_ATTRIBUTES { Sid: sid.0, Attributes: SE_GROUP_INTEGRITY } };
    // SAFETY: the token is open with TOKEN_ADJUST_DEFAULT; `label` is the TOKEN_MANDATORY_LABEL that
    // TokenIntegrityLevel reads, the length passed is its size, and the SID it points at lives in `sid` until after
    // the call.
    let set = unsafe {
        SetTokenInformation(
            token.as_raw_handle(),
            TokenIntegrityLevel,
            (&label as *const TOKEN_MANDATORY_LABEL).cast(),
            size_of::<TOKEN_MANDATORY_LABEL>() as u32,
        )
    };
    if set == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// The rights `token` is granted on the object `descriptor` describes; 0 when the check denies every right.
/// `MAXIMUM_ALLOWED` returns every right granted, so the caller's answer is "any write right", not "every write
/// right".
fn granted_access(descriptor: &LocalMemory, token: &OwnedHandle) -> io::Result<u32> {
    let mapping = GENERIC_MAPPING {
        GenericRead: FILE_GENERIC_READ,
        GenericWrite: FILE_GENERIC_WRITE,
        GenericExecute: FILE_GENERIC_EXECUTE,
        GenericAll: FILE_ALL_ACCESS,
    };
    let mut privileges = [0u32; PRIVILEGE_UNITS];
    let mut length = size_of_val(&privileges) as u32;
    let (mut granted, mut status) = (0u32, 0);
    // SAFETY: the descriptor is the one GetNamedSecurityInfoW allocated, alive until `descriptor` drops (a descriptor
    // that lacks a part AccessCheck needs fails the call, it is not read out of bounds); the token handle is open while
    // `token` lives; `mapping` outlives the call; `privileges` is `length` bytes of `u32`s, aligned for the
    // PRIVILEGE_SET the call writes to it; the call writes the rights and the status to `granted` and `status`.
    let checked = unsafe {
        AccessCheck(
            descriptor.0,
            token.as_raw_handle(),
            MAXIMUM_ALLOWED,
            &mapping,
            privileges.as_mut_ptr().cast::<PRIVILEGE_SET>(),
            &mut length,
            &mut granted,
            &mut status,
        )
    };
    if checked == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(if status != 0 { granted } else { 0 })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Command;

    use lazymkv_core::job::Tool;
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    use windows_sys::Win32::System::Time::SystemTimeToFileTime;

    use super::WinHost;
    use crate::testutil::{reg_query, temp_dir};
    use crate::win::wide;
    use crate::Host;

    /// The key `reg query` reads the Program Files folders from.
    const CURRENT_VERSION: &str = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion";
    /// The LAZYMKV_TEST_HELPER value that makes `install_paths_child` write its install paths.
    const INSTALL_PATHS_CHILD: &str = "print-install-paths";

    /// `local_time`'s text back into its fields.
    fn fields(text: &str) -> SYSTEMTIME {
        let at = |from: usize, to: usize| text[from..to].parse::<u16>().unwrap();
        SYSTEMTIME { wYear: at(0, 4), wMonth: at(5, 7), wDayOfWeek: 0, wDay: at(8, 10), wHour: at(11, 13),
            wMinute: at(14, 16), wSecond: at(17, 19), wMilliseconds: at(20, 23) }
    }

    /// A local SYSTEMTIME as 100-nanosecond ticks, so two local readings can be subtracted.
    fn ticks(time: &SYSTEMTIME) -> i64 {
        let mut file = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        // SAFETY: both pointers are to live locals of the declared types.
        assert_ne!(unsafe { SystemTimeToFileTime(time, &mut file) }, 0);
        (i64::from(file.dwHighDateTime) << 32) | i64::from(file.dwLowDateTime)
    }

    fn local_now() -> SYSTEMTIME {
        let mut time = fields("1601-01-01 00:00:00.000");
        // SAFETY: GetLocalTime writes the struct it is given.
        unsafe { GetLocalTime(&mut time) };
        time
    }

    /// The byte count of `HKCU\Software\MakeMKV\app_Key`, read without its data.
    fn app_key_bytes() -> Option<u32> {
        let (key, value) = (wide(r"Software\MakeMKV"), wide("app_Key"));
        let mut bytes = 0u32;
        // SAFETY: a size query: no type or data pointer; `bytes` receives the count.
        let status = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(),
                std::ptr::null_mut(), &mut bytes)
        };
        (status == 0).then_some(bytes)
    }

    #[test]
    fn local_time_matches_get_local_time_within_a_second() {
        let host = WinHost::new();
        let ours = host.local_time(host.now_ms());
        let windows = local_now();
        assert_eq!(ours.len(), 23, "{ours}");
        // GetLocalTime moves at the timer tick and now_ms at the precise clock, so they are compared by distance.
        let apart_ms = (ticks(&fields(&ours)) - ticks(&windows)).abs() / 10_000;
        assert!(apart_ms < 1_000, "{ours} is {apart_ms} ms from GetLocalTime");
    }

    #[test]
    fn local_date_is_the_first_ten_characters_of_local_time() {
        let host = WinHost::new();
        let (first, time, last) = (host.local_date(), host.local_time(host.now_ms()), host.local_date());
        assert_eq!(first.len(), 10, "{first}");
        // Equal dates around the read mean no midnight passed between them.
        if first == last {
            assert_eq!(&time[..10], first);
        }
    }

    #[test]
    fn the_process_list_holds_this_test_binary() {
        let exe = std::env::current_exe().unwrap();
        let own = exe.file_stem().unwrap().to_string_lossy().to_lowercase();
        let names: Vec<String> = WinHost::new()
            .running_processes()
            .iter()
            .map(|name| name.to_lowercase().trim_end_matches(".exe").to_string())
            .collect();
        // Only the count is printed: the list is the machine's.
        assert!(names.contains(&own), "{own} is not among {} names", names.len());
    }

    #[test]
    fn the_system_drive_has_a_label_or_none_without_error() {
        let host = WinHost::new();
        let drive = std::env::var("SystemDrive").unwrap();
        // Some: the volume answered (its label may be empty). None: the path is not a volume root.
        assert!(host.volume_label(&PathBuf::from(format!(r"{drive}\"))).is_some());
        assert!(host.volume_label(&PathBuf::from(&drive)).is_some());
        assert_eq!(host.volume_label(&temp_dir("host-label").join("absent")), None);
    }

    #[test]
    fn the_debug_log_is_makemkv_log_txt_in_the_profile() {
        let profile = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
        // assert!, not assert_eq!: a failure would print the profile path, which holds the account name.
        assert!(
            WinHost::new().makemkv_debug_log() == Some(profile.join("MakeMKV_log.txt")),
            "the debug log is not MakeMKV_log.txt in the profile"
        );
    }

    #[test]
    fn the_app_key_is_read_without_being_printed() {
        // More than the terminating NUL means a key is set. Only the two booleans meet the assertion, so a failure
        // prints no key (S61, design part 7.2).
        let set = app_key_bytes().is_some_and(|bytes| bytes > 2);
        assert_eq!(WinHost::new().makemkv_app_key().is_some(), set);
    }

    #[test]
    fn default_install_paths_are_the_registrys_program_files_folders_and_none_for_ffprobe() {
        // Whole-branch review F1: the folders are the HKLM values, read here by `reg query` in a child process.
        let folder = |value: &str| PathBuf::from(reg_query(CURRENT_VERSION, value).expect("a Program Files value"));
        let host = WinHost::new();
        let makemkvcon = folder("ProgramFilesDir (x86)").join(r"MakeMKV\makemkvcon64.exe");
        assert_eq!(host.default_install_path(Tool::Makemkvcon), Some(makemkvcon));
        let mkvmerge = folder("ProgramFilesDir").join(r"MKVToolNix\mkvmerge.exe");
        assert_eq!(host.default_install_path(Tool::Mkvmerge), Some(mkvmerge));
        for tool in [Tool::Ffprobe, Tool::Ffmpeg, Tool::Java] {
            assert_eq!(host.default_install_path(tool), None, "{tool:?}");
        }
    }

    /// Whole-branch review F1: a child whose `ProgramFiles` and `ProgramFiles(x86)` name a temporary folder, as a user
    /// variable a non-elevated process set would, finds the same install paths as this process. A child process, since
    /// `set_var` here would race the other tests.
    #[test]
    fn the_program_files_variables_do_not_move_the_install_paths() {
        let planted = temp_dir("host-program-files");
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "win::host::tests::install_paths_child", "--ignored", "--nocapture"])
            .env("LAZYMKV_TEST_HELPER", INSTALL_PATHS_CHILD)
            .env("ProgramFiles", &planted)
            .env("ProgramFiles(x86)", &planted)
            .output()
            .unwrap();
        let _ = fs::remove_dir_all(&planted);
        assert!(output.status.success(), "the child failed: {}", String::from_utf8_lossy(&output.stderr));
        let text = String::from_utf8_lossy(&output.stdout);
        let host = WinHost::new();
        for (tool, name) in [(Tool::Makemkvcon, "makemkvcon"), (Tool::Mkvmerge, "mkvmerge")] {
            let ours = host.default_install_path(tool);
            assert!(ours.is_some(), "this process has no {name} install path");
            let expected = install_path_line(name, ours.as_deref());
            let theirs = text.lines().find(|line| line.starts_with(&format!("install-path {name} ")));
            // Only booleans reach the message: a moved path names the temporary folder, which holds the account name.
            assert!(
                theirs == Some(expected.as_str()),
                "the child's {name} path differs from this process's; it lies under the planted folder: {}",
                theirs.is_some_and(|line| line.contains(&*planted.to_string_lossy()))
            );
        }
    }

    /// The line `install_paths_child` writes for one tool's install path.
    fn install_path_line(name: &str, path: Option<&std::path::Path>) -> String {
        format!("install-path {name} {}", path.map_or("none".to_string(), |path| path.display().to_string()))
    }

    /// The child `the_program_files_variables_do_not_move_the_install_paths` starts: this test binary running only this
    /// test, which writes one `install_path_line` per tool when LAZYMKV_TEST_HELPER names `INSTALL_PATHS_CHILD`, and
    /// does nothing otherwise.
    #[test]
    #[ignore]
    fn install_paths_child() {
        if std::env::var("LAZYMKV_TEST_HELPER").as_deref() != Ok(INSTALL_PATHS_CHILD) {
            return;
        }
        let host = WinHost::new();
        let mut out = std::io::stdout();
        for (tool, name) in [(Tool::Makemkvcon, "makemkvcon"), (Tool::Mkvmerge, "mkvmerge")] {
            // A line of its own, apart from the test harness's progress text.
            let line = install_path_line(name, host.default_install_path(tool).as_deref());
            out.write_all(format!("\n{line}\n").as_bytes()).unwrap();
        }
        out.flush().unwrap();
    }

    #[test]
    fn a_fresh_temp_folder_is_user_writable() {
        let dir = temp_dir("host-writable");
        let file = dir.join("tool.exe");
        fs::write(&file, b"x").unwrap();
        let host = WinHost::new();
        assert!(host.user_writable(&dir));
        assert!(host.user_writable(&file));
    }

    #[test]
    fn the_windows_folder_is_not_user_writable() {
        let system32 = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
        let host = WinHost::new();
        assert!(!host.user_writable(&system32));
        assert!(!host.user_writable(&system32.join("cmd.exe")));
    }

    #[test]
    fn a_path_that_cannot_be_read_counts_as_writable() {
        // Ruling W17: any failure answers true, so a lookup never trusts what it could not judge.
        assert!(WinHost::new().user_writable(&temp_dir("host-absent").join("absent")));
    }
}

//! The engine's data folder (design part 3.5, MISS-ARCH-18; Rulings W6, T3-3-a to T3-3-c): created, when absent,
//! owned by Administrators with the protected access list from its first moment; then, created or found, refused if it
//! is a link, a junction, a file or a folder of another owner, and otherwise given the protected access list. Its
//! default place comes from the registry, not the environment (Ruling W1, amended after the whole-branch review (F2)).

use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{LocalFree, ERROR_ALREADY_EXISTS, ERROR_SUCCESS};
use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    PSID,
};
use windows_sys::Win32::Storage::FileSystem::{CreateDirectoryW, FILE_ATTRIBUTE_REPARSE_POINT};
use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ};
use windows_sys::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;

use super::{registry_text, sid_string, wide, Descriptor};
use crate::security::{folder_sddl, owner_allowed};

/// The key below `HKEY_LOCAL_MACHINE` whose value `ProgramData` names the machine's ProgramData folder, as
/// `%SystemDrive%\ProgramData` (whole-branch review F2).
const PROFILE_LIST_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList";
/// The variable the registry's `ProgramData` value starts with; it is replaced by the Windows folder's drive, never
/// read from the environment.
const SYSTEM_DRIVE: &str = "%SystemDrive%";

/// Makes `path` the engine's data folder (design part 3.5, MISS-ARCH-18; Rulings W6, T3-3-a to T3-3-c). An absent
/// folder is created owned by Administrators. A link, a junction, a file, or a folder owned by any account but
/// Administrators or SYSTEM is refused. Then the protected access list `folder_sddl(reader_sid)` is set, and everything
/// below inherits it. `reader_sid` is the owner's account, the engine's own token user. Err is a sentence for standard
/// error. Def: T3-3. Use: T3-23.
pub fn protect_data_folder(path: &Path, reader_sid: &str) -> Result<(), String> {
    let unreadable = |error: io::Error| format!("{} could not be read: {error}", path.display());
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // Ruling T3-3-a. The path is read again whether it was made here or, error 183, by another process
            // meanwhile, and goes on as an existing folder: a volume that keeps no access lists ignores the creation's
            // descriptor without an error, so only the owner check and the list set below confirm the protection.
            if let Err(error) = create(path, reader_sid) {
                if error.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32) {
                    return Err(format!("{} could not be created: {error}", path.display()));
                }
            }
            fs::symlink_metadata(path).map_err(unreadable)?
        }
        Err(error) => return Err(unreadable(error)),
    };
    // Ruling T3-3-b: SetNamedSecurityInfoW follows a link, so a link, a junction or a file is refused before any
    // change.
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 || !metadata.is_dir() {
        return Err(format!(
            "{} is a link, a junction or a file, not a folder; the engine will not use it (design part 3.5)",
            path.display()
        ));
    }
    // Ruling T3-3-c: the owner is checked before the list is set, so a refused folder keeps its list.
    let owner = owner_of(path).map_err(|error| format!("{}'s owner could not be read: {error}", path.display()))?;
    if !owner_allowed(&owner) {
        return Err(format!(
            "{} is owned by {owner}, neither Administrators nor SYSTEM; the engine will not use it (design part 3.5)",
            path.display()
        ));
    }
    set_list(path, reader_sid).map_err(|error| format!("{}'s access list could not be set: {error}", path.display()))
}

/// The engine's default data folder (design part 3.1; Ruling W1, amended after the whole-branch review (F2)): the value
/// `ProgramData` under `HKEY_LOCAL_MACHINE`'s `PROFILE_LIST_KEY`, read with `RRF_NOEXPAND`, its leading
/// `%SystemDrive%` replaced by the drive of the Windows folder (`GetSystemWindowsDirectoryW`), joined with `lazymkv`.
/// No environment variable is read, since a non-elevated process of the user can set `ProgramData` and `SystemDrive`
/// for the elevated engine. None when the value cannot be read, still holds a `%` after the replacement, or is not an
/// absolute path. Def: T3-3. Use: T3-23.
pub fn program_data() -> Option<PathBuf> {
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
    let value = registry_text(HKEY_LOCAL_MACHINE, PROFILE_LIST_KEY, "ProgramData", flags)?;
    let variable: Vec<u16> = SYSTEM_DRIVE.encode_utf16().collect();
    let units = match value.get(..variable.len()) {
        Some(head) if String::from_utf16_lossy(head).eq_ignore_ascii_case(SYSTEM_DRIVE) => {
            let mut units = system_drive()?;
            units.extend_from_slice(&value[variable.len()..]);
            units
        }
        _ => value,
    };
    if units.contains(&u16::from(b'%')) {
        return None;
    }
    let folder = PathBuf::from(OsString::from_wide(&units));
    // An empty value gives an empty path, which is not absolute either.
    folder.is_absolute().then(|| folder.join("lazymkv"))
}

/// The drive of the Windows folder, such as `C:`, as UTF-16 units; None when the folder cannot be read or does not
/// start with a drive letter and a colon.
fn system_drive() -> Option<Vec<u16>> {
    // SAFETY: with no buffer and a size of 0, the call writes nothing and returns the size it needs, NUL included.
    let needed = unsafe { GetSystemWindowsDirectoryW(null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut folder = vec![0u16; needed as usize];
    // SAFETY: `folder` holds the `needed` units passed, and the call writes at most that many to it.
    let length = unsafe { GetSystemWindowsDirectoryW(folder.as_mut_ptr(), needed) };
    // The length in a binding whose address is never taken, checked non-zero and below the buffer's size, which a
    // successful call returns, before it bounds the read.
    let length = length as usize;
    if length == 0 || length >= folder.len() {
        return None;
    }
    let is_letter = |unit: u16| u8::try_from(unit).is_ok_and(|byte| byte.is_ascii_alphabetic());
    match folder[..length] {
        [letter, colon, ..] if is_letter(letter) && colon == u16::from(b':') => Some(vec![letter, colon]),
        _ => None,
    }
}

/// Creates the last component of `path` with a descriptor that makes it owned by Administrators and protected from its
/// first moment on a volume that keeps access lists (Ruling T3-3-a); a missing parent is not created (Ruling W15).
fn create(path: &Path, reader_sid: &str) -> io::Result<()> {
    let descriptor = Descriptor::from_sddl(&format!("O:BA{}", folder_sddl(reader_sid)))?;
    let attributes = descriptor.attributes();
    let name = wide(path);
    // SAFETY: `name` is NUL-terminated and outlives the call; the attributes point at `descriptor`, which outlives it
    // too.
    if unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// The security identifier (SID) of the owner of the file or folder `path`, in its string form.
fn owner_of(path: &Path) -> io::Result<String> {
    let name = wide(path);
    let (mut owner, mut descriptor): (PSID, PSECURITY_DESCRIPTOR) = (null_mut(), null_mut());
    // SAFETY: `name` is NUL-terminated and outlives the call; the group and list pointers may be null; the call writes
    // the owner, which points into the descriptor it allocates with LocalAlloc.
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    // The owner in a binding whose address is never taken, so that the code scanner sees the null check cover its use.
    let owner = owner;
    let sid = if owner.is_null() {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Windows returned no owner"))
    } else {
        sid_string(owner)
    };
    // SAFETY: GetNamedSecurityInfoW allocated the descriptor with LocalAlloc; it is freed once, here, after the last
    // read of `owner`, which points into it.
    unsafe { LocalFree(descriptor) };
    sid
}

/// Sets `folder_sddl(reader_sid)` on `path` as a protected list, which Windows propagates to everything below.
fn set_list(path: &Path, reader_sid: &str) -> io::Result<()> {
    let descriptor = Descriptor::from_sddl(&folder_sddl(reader_sid))?;
    let dacl = descriptor.dacl();
    // A null list would grant everyone full control. A descriptor from folder_sddl always holds one, so this guards
    // against a failed read of it rather than choosing a branch.
    assert!(!dacl.is_null(), "the descriptor of folder_sddl holds no access list");
    let name = wide(path);
    // SAFETY: `name` is NUL-terminated and outlives the call; `dacl` is the non-null list of `descriptor`, which
    // outlives the call; the owner, group and system list are not set, so their pointers are null.
    let status = unsafe {
        SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            dacl,
            null(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{ConvertStringSidToSidW, SetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSID, UNPROTECTED_DACL_SECURITY_INFORMATION,
    };

    use super::*;
    use crate::security::{folder_sddl, ADMINISTRATORS_SID};
    use crate::testutil::{reg_query, temp_dir};
    use crate::win::{own_token, read_descriptor, read_path, wide, AclEntry, Descriptor};

    /// Removes the test's folder when the test ends, failed or not (Global Constraints).
    struct Cleanup(PathBuf);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The entries `folder_sddl(sid)` holds, read with the reader the folder is read with.
    fn folder_entries(sid: &str) -> Vec<AclEntry> {
        read_descriptor(Descriptor::from_sddl(&folder_sddl(sid)).unwrap().as_ptr()).entries
    }

    /// The entries a file below gets from the folder's: the same rights, flagged only INHERITED_ACE (0x10).
    fn inherited(folder: &[AclEntry]) -> Vec<AclEntry> {
        folder.iter().map(|entry| AclEntry { flags: 0x10, ..entry.clone() }).collect()
    }

    /// Sets `path`'s owner to `sid`, as another process or an earlier run would have left it.
    fn set_owner(path: &Path, sid: &str) {
        let (sid_text, name) = (wide(sid), wide(path));
        let mut owner: PSID = std::ptr::null_mut();
        // SAFETY: the SID text is NUL-terminated; `owner` receives a SID that LocalFree frees below.
        assert_ne!(unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut owner) }, 0, "ConvertStringSidToSidW");
        // SAFETY: the name is NUL-terminated, `owner` is a valid SID, and no other part is passed.
        let code = unsafe {
            SetNamedSecurityInfoW(name.as_ptr(), SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION, owner,
                std::ptr::null_mut(), std::ptr::null(), std::ptr::null())
        };
        // SAFETY: `owner` came from ConvertStringSidToSidW and is freed once.
        unsafe { LocalFree(owner) };
        assert_eq!(code, 0, "SetNamedSecurityInfoW(owner): {}", std::io::Error::from_raw_os_error(code as i32));
    }

    /// Replaces `path`'s list with `sddl`'s, not protected, as an earlier tool might have left it.
    fn set_open_list(path: &Path, sddl: &str) {
        let (descriptor, name) = (Descriptor::from_sddl(sddl).unwrap(), wide(path));
        // SAFETY: the name is NUL-terminated and the list belongs to `descriptor`, which outlives the call.
        let code = unsafe {
            SetNamedSecurityInfoW(name.as_ptr(), SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | UNPROTECTED_DACL_SECURITY_INFORMATION, std::ptr::null_mut(),
                std::ptr::null_mut(), descriptor.dacl(), std::ptr::null())
        };
        assert_eq!(code, 0, "SetNamedSecurityInfoW(list): {}", std::io::Error::from_raw_os_error(code as i32));
    }

    #[test]
    fn an_absent_folder_is_created_owned_by_administrators_with_the_protected_list() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-new"));
        let path = dir.0.join("data");
        protect_data_folder(&path, &own.user_sid).unwrap();
        let read = read_path(&path).unwrap();
        assert_eq!(read.owner, ADMINISTRATORS_SID);
        assert!(read.protected, "the folder still inherits from its parent");
        assert_eq!(read.entries, folder_entries(&own.user_sid));
    }

    #[test]
    fn an_existing_folder_owned_by_administrators_gets_the_list_again() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-again"));
        let path = dir.0.join("data");
        fs::create_dir(&path).unwrap();
        set_owner(&path, ADMINISTRATORS_SID);
        set_open_list(&path, "D:AI(A;OICI;FA;;;WD)"); // WD is Everyone
        fs::write(path.join("settings.json"), "{}").unwrap();
        protect_data_folder(&path, &own.user_sid).unwrap();
        let read = read_path(&path).unwrap();
        assert_eq!(read.owner, ADMINISTRATORS_SID);
        assert!(read.protected);
        assert_eq!(read.entries, folder_entries(&own.user_sid));
        let file = read_path(&path.join("settings.json")).unwrap();
        assert_eq!(file.entries, inherited(&folder_entries(&own.user_sid)), "the new list reached the file below");
    }

    #[test]
    fn a_folder_owned_by_the_user_is_refused_by_sid() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-user"));
        let path = dir.0.join("data");
        fs::create_dir(&path).unwrap();
        set_owner(&path, &own.user_sid);
        let before = read_path(&path).unwrap();
        assert_eq!(
            protect_data_folder(&path, &own.user_sid),
            Err(format!(
                "{} is owned by {}, neither Administrators nor SYSTEM; the engine will not use it (design part 3.5)",
                path.display(),
                own.user_sid
            ))
        );
        assert!(read_path(&path).unwrap() == before, "a refused folder keeps its owner and list");
    }

    #[test]
    fn a_file_created_below_inherits_the_list() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-below"));
        let path = dir.0.join("data");
        protect_data_folder(&path, &own.user_sid).unwrap();
        fs::write(path.join("engine.log"), "").unwrap();
        let file = read_path(&path.join("engine.log")).unwrap();
        assert!(!file.protected);
        assert_eq!(file.entries, inherited(&folder_entries(&own.user_sid)));
    }

    #[test]
    fn a_junction_or_a_file_in_place_of_the_folder_is_refused() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-link"));
        let target = dir.0.join("target");
        fs::create_dir(&target).unwrap();
        set_owner(&target, ADMINISTRATORS_SID); // so only Ruling T3-3-b can refuse it
        let before = read_path(&target).unwrap();
        let link = dir.0.join("data");
        let made = std::process::Command::new("cmd").arg("/c").arg("mklink").arg("/J").arg(&link).arg(&target)
            .output().unwrap();
        assert!(made.status.success(), "mklink /J failed");
        let refusal = |path: &Path| {
            format!("{} is a link, a junction or a file, not a folder; the engine will not use it (design part 3.5)",
                path.display())
        };
        assert_eq!(protect_data_folder(&link, &own.user_sid), Err(refusal(&link)));
        assert!(read_path(&target).unwrap() == before, "the junction's target kept its list");
        let file = dir.0.join("file");
        fs::write(&file, "").unwrap();
        assert_eq!(protect_data_folder(&file, &own.user_sid), Err(refusal(&file)));
    }

    #[test]
    fn a_missing_parent_is_not_created() {
        let own = own_token().unwrap();
        let dir = Cleanup(temp_dir("folder-parent"));
        let path = dir.0.join("absent").join("data");
        let error = protect_data_folder(&path, &own.user_sid).unwrap_err();
        assert!(error.starts_with(&format!("{} could not be created: ", path.display())), "{error}");
        assert!(!dir.0.join("absent").exists());
    }

    /// Whole-branch review F2: the folder is the HKLM value, read here by `reg query` in a child process, with its
    /// `%SystemDrive%` replaced by the system drive, joined with `lazymkv`. The test's own `SystemDrive` stands in for
    /// the drive, since nothing here sets it.
    #[test]
    fn program_data_is_derived_from_the_registry() {
        let key = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList";
        let value = reg_query(key, "ProgramData").expect("the registry names ProgramData");
        let drive = std::env::var("SystemDrive").expect("Windows sets SystemDrive");
        let expected = PathBuf::from(value.replacen("%SystemDrive%", &drive, 1)).join("lazymkv");
        assert!(expected.is_absolute(), "{}", expected.display());
        assert_eq!(program_data(), Some(expected));
    }
}

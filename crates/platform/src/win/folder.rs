//! The engine's data folder (design part 3.5, MISS-ARCH-18; Rulings W6, T3-3-a to T3-3-c): created owned by
//! Administrators with the protected access list from its first moment, or, when it exists, refused if it is a link,
//! a junction, a file or a folder of another owner, and otherwise given the protected access list again.

use std::fs;
use std::io;
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

use super::{sid_string, wide, Descriptor};
use crate::security::{folder_sddl, owner_allowed};

/// Makes `path` the engine's data folder (design part 3.5, MISS-ARCH-18; Rulings W6, T3-3-a to T3-3-c). An absent
/// folder is created owned by Administrators. A link, a junction, a file, or a folder owned by any account but
/// Administrators or SYSTEM is refused. Then the protected access list `folder_sddl(reader_sid)` is set, and everything
/// below inherits it. `reader_sid` is the owner's account, the engine's own token user. Err is a sentence for standard
/// error. Def: T3-3. Use: T3-23.
pub fn protect_data_folder(path: &Path, reader_sid: &str) -> Result<(), String> {
    let unreadable = |error: io::Error| format!("{} could not be read: {error}", path.display());
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => match create(path, reader_sid) {
            // Ruling T3-3-a: the folder was made with its owner and its protected list, and nothing is below it yet.
            Ok(()) => return Ok(()),
            // Another process made it meanwhile: it is read again and goes on as an existing folder.
            Err(error) if error.raw_os_error() == Some(ERROR_ALREADY_EXISTS as i32) => {
                fs::symlink_metadata(path).map_err(unreadable)?
            }
            Err(error) => return Err(format!("{} could not be created: {error}", path.display())),
        },
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

/// `%ProgramData%` from the environment (design part 3.1, Ruling W1), or None when it is unset, empty or not an
/// absolute path. The engine's default data folder is this joined with `lazymkv`. Def: T3-3. Use: T3-23.
pub fn program_data() -> Option<PathBuf> {
    let folder = PathBuf::from(std::env::var_os("ProgramData")?);
    // An empty value gives an empty path, which is not absolute either.
    folder.is_absolute().then_some(folder)
}

/// Creates the last component of `path`, owned by Administrators and with the protected list from its first moment
/// (Ruling T3-3-a); a missing parent is not created (Ruling W15).
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
    use crate::testutil::temp_dir;
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

    #[test]
    fn program_data_is_read_from_the_environment() {
        let set = PathBuf::from(std::env::var_os("ProgramData").expect("Windows sets ProgramData"));
        assert!(set.is_absolute());
        assert_eq!(program_data(), Some(set));
    }
}

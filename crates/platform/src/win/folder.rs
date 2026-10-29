//! The engine's data folder (design part 3.5, MISS-ARCH-18; Rulings W6, T3-3-a to T3-3-c): created, when absent,
//! owned by Administrators with the protected access list from its first moment; then, created or found, refused if it
//! is a link, a junction, a file or a folder of another owner, or if any entry below it, at any depth and without
//! following links, is a link or a junction or is owned by any account but Administrators or SYSTEM (review round 1 of
//! pull request 3 (V3)); and otherwise given the protected access list. Setting the list replaces only the inherited
//! entries below: each entry keeps its owner, its explicit entries and any protected list as they were, which is why
//! every entry is checked before any change. Its default place comes from the registry, not the environment (Ruling
//! W1, amended after the whole-branch review (F2)), as does an account's profile folder, where MakeMKV writes its debug
//! log (review round 1 of pull request 3 (V14)).

use std::ffi::OsString;
use std::fs::{self, Metadata};
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
/// `%SystemDrive%\ProgramData` (whole-branch review F2), and below which the key named by an account's SID holds the
/// account's profile folder as `ProfileImagePath` (review round 1 of pull request 3 (V14)).
const PROFILE_LIST_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList";
/// The variable a registry folder value may start with; it is replaced by the Windows folder's drive, never read from
/// the environment.
const SYSTEM_DRIVE: &str = "%SystemDrive%";

/// Makes `path` the engine's data folder (design part 3.5, MISS-ARCH-18; Rulings W6, T3-3-a to T3-3-c). An absent
/// folder is created owned by Administrators. A link, a junction, a file, or a folder owned by any account but
/// Administrators or SYSTEM is refused. So is a folder holding, at any depth below it, an entry that is a link or a
/// junction or that is owned by any account but Administrators or SYSTEM, found without following links and named in
/// the refusal (review round 1 of pull request 3 (V3)). Every refusal comes before any change. Then the protected
/// access list `folder_sddl(reader_sid)` is set, and the entries below inherit it: the set replaces only their
/// inherited entries and leaves each entry's owner, its explicit entries and any protected list as they were.
/// `reader_sid` is the owner's account, the engine's own token user. Err is a sentence for standard error, with SIDs,
/// never account names. Def: T3-3. Use: T3-23.
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
    if is_link(&metadata) || !metadata.is_dir() {
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
    // Rulings T3-3-b and T3-3-c, widened to every entry below (review round 1 of pull request 3 (V3)): the set would
    // follow a link below, and an entry another account owns could change its own list after the set, so either is
    // refused here, before any change.
    check_below(path)?;
    set_list(path, reader_sid).map_err(|error| format!("{}'s access list could not be set: {error}", path.display()))
}

/// Whether `metadata`, read without following a link, is a reparse point: a symbolic link, a junction or any other.
fn is_link(metadata: &Metadata) -> bool {
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Refuses the data folder `folder` when any entry below it, at any depth, is a link or a junction, or is owned by any
/// account but Administrators or SYSTEM (design part 3.5; review round 1 of pull request 3 (V3)). The walk reads each
/// entry with `symlink_metadata`, so it never follows a link and never descends into one. An entry or a folder that
/// cannot be read, or whose owner cannot be read, refuses the folder too, so an entry whose list excludes
/// Administrators is refused. Err is a sentence for standard error naming the entry, with SIDs only.
fn check_below(folder: &Path) -> Result<(), String> {
    let unreadable = |path: &Path, error: io::Error| format!("{} could not be read: {error}", path.display());
    let mut folders = vec![folder.to_path_buf()];
    while let Some(listed) = folders.pop() {
        for entry in fs::read_dir(&listed).map_err(|error| unreadable(&listed, error))? {
            let entry = entry.map_err(|error| unreadable(&listed, error))?.path();
            let metadata = fs::symlink_metadata(&entry).map_err(|error| unreadable(&entry, error))?;
            if is_link(&metadata) {
                return Err(format!(
                    "{} is a link or a junction; the engine will not use the data folder {} (design part 3.5)",
                    entry.display(),
                    folder.display()
                ));
            }
            let owner =
                owner_of(&entry).map_err(|error| format!("{}'s owner could not be read: {error}", entry.display()))?;
            if !owner_allowed(&owner) {
                return Err(format!(
                    "{} is owned by {owner}, neither Administrators nor SYSTEM; the engine will not use the data \
                     folder {} (design part 3.5)",
                    entry.display(),
                    folder.display()
                ));
            }
            if metadata.is_dir() {
                folders.push(entry);
            }
        }
    }
    Ok(())
}

/// The engine's default data folder (design part 3.1; Ruling W1, amended after the whole-branch review (F2)): the value
/// `ProgramData` under `HKEY_LOCAL_MACHINE`'s `PROFILE_LIST_KEY`, read as `registry_folder` reads it, joined with
/// `lazymkv`. No environment variable is read, since a non-elevated process of the user can set `ProgramData` and
/// `SystemDrive` for the elevated engine. None when `registry_folder` gives none. Def: T3-3. Use: T3-23.
pub fn program_data() -> Option<PathBuf> {
    registry_folder(PROFILE_LIST_KEY, "ProgramData").map(|folder| folder.join("lazymkv"))
}

/// The profile folder of the account whose SID is `sid`: the value `ProfileImagePath` under `HKEY_LOCAL_MACHINE`'s
/// `PROFILE_LIST_KEY\<sid>`, read as `registry_folder` reads it (design part 7.2), never `USERPROFILE`, which a
/// non-elevated process of the user can set for the elevated engine. None when `registry_folder` gives none.
/// Def: PR #14 review round 1 (V14). Use: T3-7.
pub(crate) fn profile_folder(sid: &str) -> Option<PathBuf> {
    registry_folder(&format!(r"{PROFILE_LIST_KEY}\{sid}"), "ProfileImagePath")
}

/// The folder the value `value` of the key `key` below `HKEY_LOCAL_MACHINE` names (design parts 3.1, 7.2; whole-branch
/// review F2): read as `REG_SZ` or `REG_EXPAND_SZ` with `RRF_NOEXPAND`, its leading `%SystemDrive%` replaced by the
/// drive of the Windows folder (`GetSystemWindowsDirectoryW`), and no environment variable read. None when the value
/// cannot be read, still holds a `%` after the replacement, or is not an absolute path.
/// Def: PR #14 review round 1 (V14), from F2's `program_data`. Use: T3-3, T3-7.
pub(crate) fn registry_folder(key: &str, value: &str) -> Option<PathBuf> {
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
    let value = registry_text(HKEY_LOCAL_MACHINE, key, value, flags)?;
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
    folder.is_absolute().then_some(folder)
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

/// Sets `folder_sddl(reader_sid)` on `path` as a protected list, which Windows propagates to everything below. The
/// propagation replaces only each entry's inherited entries and leaves its owner, its explicit entries and any
/// protected list as they were, so `check_below` must first have found every entry below neither a link nor a junction
/// and owned by Administrators or SYSTEM (review round 1 of pull request 3 (V3)).
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

    use lazymkv_core::testsupport::Scratch;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{ConvertStringSidToSidW, SetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSID, UNPROTECTED_DACL_SECURITY_INFORMATION,
    };

    use super::*;
    use crate::security::{folder_sddl, ADMINISTRATORS_SID};
    use crate::testutil::reg_query;
    use crate::win::{own_token, read_descriptor, read_path, wide, AclEntry, Descriptor};

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
        let dir = Scratch::new("folder-new");
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
        let dir = Scratch::new("folder-again");
        let path = dir.0.join("data");
        fs::create_dir(&path).unwrap();
        set_owner(&path, ADMINISTRATORS_SID);
        set_open_list(&path, "D:AI(A;OICI;FA;;;WD)"); // WD is Everyone
        fs::write(path.join("settings.json"), "{}").unwrap();
        // An administrator's file, as the folder's: review round 1 of pull request 3 (V3) refuses any other owner.
        set_owner(&path.join("settings.json"), ADMINISTRATORS_SID);
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
        let dir = Scratch::new("folder-user");
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
        let dir = Scratch::new("folder-below");
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
        let dir = Scratch::new("folder-link");
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

    /// The refusal of `entry`, below the data folder `folder`, owned by `owner` (review round 1 of pull request 3
    /// (V3)).
    fn owner_refusal(entry: &Path, owner: &str, folder: &Path) -> String {
        format!(
            "{} is owned by {owner}, neither Administrators nor SYSTEM; the engine will not use the data folder {} \
             (design part 3.5)",
            entry.display(),
            folder.display()
        )
    }

    /// Review round 1 of pull request 3 (V3): a settings.json the user owns keeps its owner's implicit right to change
    /// its list after the protected list propagates, so the folder is refused, naming the file, before any change.
    #[test]
    fn a_file_below_owned_by_another_account_is_refused_and_the_folder_keeps_its_list() {
        let own = own_token().unwrap();
        let dir = Scratch::new("folder-below-owner");
        let path = dir.0.join("data");
        fs::create_dir(&path).unwrap();
        set_owner(&path, ADMINISTRATORS_SID);
        set_open_list(&path, "D:AI(A;OICI;FA;;;WD)"); // WD is Everyone
        let settings = path.join("settings.json");
        fs::write(&settings, "{}").unwrap();
        set_owner(&settings, &own.user_sid);
        let (folder_before, file_before) = (read_path(&path).unwrap(), read_path(&settings).unwrap());
        assert_eq!(protect_data_folder(&path, &own.user_sid), Err(owner_refusal(&settings, &own.user_sid, &path)));
        assert!(read_path(&path).unwrap() == folder_before, "a refused folder keeps its owner and list");
        assert!(read_path(&settings).unwrap() == file_before, "the refused file keeps its owner and list");
    }

    /// Review round 1 of pull request 3 (V3): the walk goes to any depth.
    #[test]
    fn an_entry_owned_by_another_account_deeper_down_is_refused() {
        let own = own_token().unwrap();
        let dir = Scratch::new("folder-deep-owner");
        let path = dir.0.join("data");
        let job = path.join("jobs").join("abc-1");
        fs::create_dir_all(&job).unwrap();
        for folder in [&path, &path.join("jobs"), &job] {
            set_owner(folder, ADMINISTRATORS_SID);
        }
        let state = job.join("state.json");
        fs::write(&state, "{}").unwrap();
        set_owner(&state, &own.user_sid);
        assert_eq!(protect_data_folder(&path, &own.user_sid), Err(owner_refusal(&state, &own.user_sid, &path)));
    }

    /// Review round 1 of pull request 3 (V3): a junction below the folder is refused before the list is set, so the
    /// propagation never reaches its target (Ruling T3-3-b, widened).
    #[test]
    fn a_junction_below_the_folder_is_refused_and_its_target_keeps_its_list() {
        let own = own_token().unwrap();
        let dir = Scratch::new("folder-below-link");
        let (path, target) = (dir.0.join("data"), dir.0.join("target"));
        for folder in [&path, &target] {
            fs::create_dir(folder).unwrap();
            set_owner(folder, ADMINISTRATORS_SID);
            set_open_list(folder, "D:AI(A;OICI;FA;;;WD)"); // WD is Everyone
        }
        let link = path.join("inner");
        let made = std::process::Command::new("cmd").arg("/c").arg("mklink").arg("/J").arg(&link).arg(&target)
            .output().unwrap();
        assert!(made.status.success(), "mklink /J failed");
        let (folder_before, target_before) = (read_path(&path).unwrap(), read_path(&target).unwrap());
        assert_eq!(
            protect_data_folder(&path, &own.user_sid),
            Err(format!(
                "{} is a link or a junction; the engine will not use the data folder {} (design part 3.5)",
                link.display(),
                path.display()
            ))
        );
        assert!(read_path(&target).unwrap() == target_before, "the junction's target kept its list");
        assert!(read_path(&path).unwrap() == folder_before, "a refused folder keeps its owner and list");
    }

    /// Review round 1 of pull request 3 (V3): the canary that this elevated token's default owner is Administrators.
    /// The engine's own files below, written between two starts, must not refuse the second start. A failure naming an
    /// `S-1-5-21-` owner means the token's default owner is its user, which needs a ruling of its own, not a weaker
    /// walk.
    #[test]
    fn a_restart_accepts_the_entries_its_own_token_wrote_below() {
        let own = own_token().unwrap();
        let dir = Scratch::new("folder-restart");
        let path = dir.0.join("data");
        protect_data_folder(&path, &own.user_sid).unwrap();
        fs::write(path.join("engine.log"), "").unwrap();
        let job = path.join("jobs").join("abc-1");
        fs::create_dir_all(&job).unwrap();
        fs::write(job.join("state.json"), "{}").unwrap();
        assert_eq!(protect_data_folder(&path, &own.user_sid), Ok(()));
    }

    #[test]
    fn a_missing_parent_is_not_created() {
        let own = own_token().unwrap();
        let dir = Scratch::new("folder-parent");
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

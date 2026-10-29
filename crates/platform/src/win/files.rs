//! The file moves on `MoveFileExW` (S13; design parts 6.5, 7.4) that `write_atomic` and `rename_no_replace` make on
//! Windows.

use std::io;
use std::path::Path;
use std::ptr::null_mut;
use std::time::Duration;

use windows_sys::Win32::Storage::FileSystem::{
    GetFullPathNameW, MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

use super::wide;

/// How many times `move_replacing` tries a replace that meets a held target, and the pause between two tries: nine
/// pauses, at most 90 ms in all (Ruling T3-4-a).
const HELD_TRIES: u32 = 10;
const HELD_PAUSE: Duration = Duration::from_millis(10);

/// The wide length, NUL included, from which `wide_path` makes a path verbatim: std's `LEGACY_MAX_PATH`, the limit
/// `CreateDirectoryW` keeps below MAX_PATH (Ruling T3-4-c).
const LEGACY_MAX_PATH: usize = 248;
const SEP: u16 = b'\\' as u16;
const QUERY: u16 = b'?' as u16;
/// `\\?\`, the verbatim prefix.
const VERBATIM_PREFIX: &[u16] = &[SEP, SEP, QUERY, SEP];
/// `\??\`, the prefix of a path already in the object manager's form.
const NT_PREFIX: &[u16] = &[SEP, QUERY, QUERY, SEP];
/// `\\?\UNC\`, the verbatim prefix of a `\\server\share` path.
const UNC_PREFIX: &[u16] = &[SEP, SEP, QUERY, SEP, b'U' as u16, b'N' as u16, b'C' as u16, SEP];

/// `path` as the NUL-terminated wide (UTF-16, 16-bit Unicode) string Windows takes. Err `InvalidInput` with the text
/// `the path holds a NUL character` when the path holds one, since Windows would read the name only up to it. A path
/// whose wide form, NUL included, reaches 248 units and does not already start with `\\?\` or `\??\` is made absolute
/// with `GetFullPathNameW` and given the verbatim prefix, as std's `get_long_path` does, so a name past MAX_PATH (260
/// units) reaches `MoveFileExW` whatever the machine's LongPathsEnabled setting (Ruling T3-4-c); a shorter path is
/// passed as given. Named apart from `win::wide` (T3-2), which takes any text and checks nothing.
fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let name = wide(path);
    if name[..name.len() - 1].contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "the path holds a NUL character"));
    }
    if name.len() < LEGACY_MAX_PATH || name.starts_with(VERBATIM_PREFIX) || name.starts_with(NT_PREFIX) {
        return Ok(name);
    }
    Ok(verbatim(&full_path(&name)?))
}

/// `name`, a NUL-terminated wide path, made absolute and normalized by `GetFullPathNameW`, without its NUL. The first
/// call asks the length, NUL included, and the next fills a buffer of that length; a length that no longer fits, when
/// the current folder changed between the two, is asked for again.
fn full_path(name: &[u16]) -> io::Result<Vec<u16>> {
    // SAFETY: `name` is NUL-terminated and outlives the call; a null buffer of length 0 asks only for the length, and
    // the file-part pointer may be null.
    let mut needed = unsafe { GetFullPathNameW(name.as_ptr(), 0, null_mut(), null_mut()) };
    loop {
        if needed == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0u16; needed as usize];
        // SAFETY: `buffer` holds `needed` units, the length passed with it, and outlives the call; `name` is
        // NUL-terminated and outlives it; the file-part pointer may be null.
        let written = unsafe { GetFullPathNameW(name.as_ptr(), needed, buffer.as_mut_ptr(), null_mut()) };
        if written != 0 && written < needed {
            buffer.truncate(written as usize);
            return Ok(buffer);
        }
        needed = written;
    }
}

/// `full`, an absolute path from `GetFullPathNameW`, with the prefix std's `get_long_path` gives it, and a NUL: `\\?\`
/// before a drive path, `\\?\UNC\` in place of the leading `\\` of a `\\server\share` path, `\\?\` in place of a
/// `\\.\` device prefix, and nothing before a path already verbatim or of another form (Ruling T3-4-c).
fn verbatim(full: &[u16]) -> Vec<u16> {
    const COLON: u16 = b':' as u16;
    const DOT: u16 = b'.' as u16;
    let (prefix, rest): (&[u16], &[u16]) = match full {
        [_, COLON, SEP, ..] => (VERBATIM_PREFIX, full),
        [SEP, SEP, DOT, SEP, rest @ ..] => (VERBATIM_PREFIX, rest),
        [SEP, SEP, QUERY, SEP, ..] | [SEP, QUERY, QUERY, SEP, ..] => (&[], full),
        [SEP, SEP, rest @ ..] => (UNC_PREFIX, rest),
        _ => (&[], full),
    };
    prefix.iter().chain(rest).copied().chain(std::iter::once(0)).collect()
}

/// Puts `from` in place of `to` with `MoveFileExW(from, to, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`
/// (design part 7.4, S13). A try that fails with os error 5, 32 or 33 is made again, up to `HELD_TRIES` tries
/// `HELD_PAUSE` apart (Ruling T3-4-a); any other error, and the last try's, is `io::Error::last_os_error()` read at once
/// after the failed call, so its raw code is the call's (RR1-2). Def: T3-4. Use: T3-4 (`write_atomic`).
pub(crate) fn move_replacing(from: &Path, to: &Path) -> io::Result<()> {
    let (from, to) = (wide_path(from)?, wide_path(to)?);
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
    let mut tries = 1;
    loop {
        // SAFETY: both names are NUL-terminated and outlive the call, which only reads them.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), flags) } != 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        // Access denied, a sharing violation or a lock violation: another handle holds the target for now.
        let held = matches!(error.raw_os_error(), Some(5 | 32 | 33));
        if !held || tries >= HELD_TRIES {
            return Err(error);
        }
        tries += 1;
        std::thread::sleep(HELD_PAUSE);
    }
}

/// Moves `from` to `to` with one `MoveFileExW(from, to, 0)` (design part 6.5, LIB-8, S13); a failure is
/// `io::Error::last_os_error()` read at once after the call, so an existing `to` is `ErrorKind::AlreadyExists`
/// (80 or 183) and another volume is os error 17. Def: T3-4. Use: T3-4 (`rename_no_replace`).
pub(crate) fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    let (from, to) = (wide_path(from)?, wide_path(to)?);
    // SAFETY: both names are NUL-terminated and outlive the call, which only reads them.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::{Component, Path, PathBuf, Prefix};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    use super::{move_no_replace, move_replacing};
    use crate::security::test_name;
    use crate::testutil::{skip, temp_dir};
    use crate::{rename_no_replace, write_atomic};

    /// Removes the test's folder when the test ends, failed or not (Global Constraints).
    struct Cleanup(PathBuf);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The upper-case drive letter `path` starts with, or None for a path without one.
    fn drive_of(path: &Path) -> Option<u8> {
        match path.components().next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Some(letter.to_ascii_uppercase()),
                _ => None,
            },
            _ => None,
        }
    }

    #[test]
    fn the_no_replace_move_refuses_an_existing_target_and_keeps_both_files() {
        let dir = temp_dir("files-taken");
        let _cleanup = Cleanup(dir.clone());
        let (from, to) = (dir.join("a.mkv.partial"), dir.join("a.mkv"));
        fs::write(&from, "new").unwrap();
        fs::write(&to, "old").unwrap();
        let error = move_no_replace(&from, &to).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(matches!(error.raw_os_error(), Some(80 | 183)), "{error:?}");
        assert_eq!(fs::read_to_string(&from).unwrap(), "new");
        assert_eq!(fs::read_to_string(&to).unwrap(), "old");
    }

    /// Design part 6.5: without `MOVEFILE_COPY_ALLOWED` a move to another volume is refused, never done as a copy. The
    /// other volume is the one this test binary sits on, when it is not the temporary folder's (on `windows-latest`
    /// the target folder is on D: and the temporary folder on C:; on the mini PC both are on C:, so it skips there).
    #[test]
    fn the_no_replace_move_across_volumes_fails_rather_than_copies() {
        let dir = temp_dir("files-xvol");
        let _cleanup = Cleanup(dir.clone());
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        match (drive_of(&dir), drive_of(&exe_dir)) {
            (Some(temp), Some(exe)) if temp != exe => {}
            _ => {
                skip("the temporary folder and this test binary are on one volume, so no move across volumes can be tried");
                return;
            }
        }
        let other = exe_dir.join(test_name("xvol"));
        fs::create_dir(&other).unwrap();
        let from = dir.join("a.mkv.partial");
        fs::write(&from, "placeholder").unwrap();
        let error = move_no_replace(&from, &other.join("a.mkv")).unwrap_err();
        let kept = (from.exists(), other.join("a.mkv").exists());
        fs::remove_dir_all(&other).unwrap();
        assert_eq!(error.raw_os_error(), Some(17), "{error:?}");
        assert_eq!(kept, (true, false));
    }

    #[test]
    fn a_path_holding_nul_is_invalid_input_and_nothing_moves() {
        let dir = temp_dir("files-nul");
        let _cleanup = Cleanup(dir.clone());
        fs::write(dir.join("a"), "kept").unwrap();
        let error = move_no_replace(&dir.join("a\0b"), &dir.join("c")).unwrap_err();
        assert_eq!((error.kind(), error.to_string()), (io::ErrorKind::InvalidInput, "the path holds a NUL character".to_string()));
        let error = move_replacing(&dir.join("a"), &dir.join("c\0d")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(dir.join("a").exists() && !dir.join("c").exists());
    }

    #[test]
    fn a_name_outside_ascii_survives_both_moves() {
        let dir = temp_dir("files-names");
        let _cleanup = Cleanup(dir.clone());
        write_atomic(&dir.join("Café Ōsaka 千 (2001).json"), b"{}").unwrap();
        let partial = dir.join("Café Ōsaka 千 (2001).mkv.partial");
        fs::write(&partial, "placeholder").unwrap();
        rename_no_replace(&partial, &dir.join("Café Ōsaka 千 (2001).mkv")).unwrap();
        let mut names: Vec<String> =
            fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["Café Ōsaka 千 (2001).json", "Café Ōsaka 千 (2001).mkv"]);
    }

    /// RR1-2: a target held open for the whole call fails the replace with a code the store's retry knows, the old
    /// bytes stay, and the next write, once the holder is gone, replaces it and removes the temporary file.
    #[test]
    fn a_replace_over_a_target_held_open_returns_the_raw_os_error_the_store_retries() {
        for share in [FILE_SHARE_READ | FILE_SHARE_WRITE, 0] {
            let dir = temp_dir("files-held");
            let _cleanup = Cleanup(dir.clone());
            let path = dir.join("state.json");
            fs::write(&path, b"old").unwrap();
            let held = fs::OpenOptions::new().read(true).share_mode(share).open(&path).unwrap();
            let error = write_atomic(&path, b"new").unwrap_err();
            println!("share {share:#x}: {error}");
            assert!(matches!(error.raw_os_error(), Some(5 | 32 | 33)), "share {share:#x}: {error:?}");
            drop(held);
            assert_eq!(fs::read(&path).unwrap(), b"old", "share {share:#x}");
            write_atomic(&path, b"new").unwrap();
            assert_eq!(fs::read(&path).unwrap(), b"new", "share {share:#x}");
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "share {share:#x}: a temporary file was left");
        }
    }

    /// Ruling T3-4-a: a reader that opens the target again and again, as the harness's polls do, never fails a write.
    #[test]
    fn a_target_another_thread_keeps_opening_is_replaced_every_time() {
        let dir = temp_dir("files-polled");
        let _cleanup = Cleanup(dir.clone());
        let path = dir.join("drives.json");
        write_atomic(&path, b"0").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let poller = {
            let (path, stop) = (path.clone(), Arc::clone(&stop));
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    drop(fs::File::open(&path));
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
        };
        for n in 1..=200u32 {
            write_atomic(&path, n.to_string().as_bytes()).unwrap_or_else(|error| panic!("write {n}: {error}"));
        }
        stop.store(true, Ordering::SeqCst);
        poller.join().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "200");
    }

    /// Ruling T3-4-c: names past MAX_PATH (260 units), as a long title under a deep library folder gives, reach both
    /// moves through the verbatim prefix, whatever the machine's LongPathsEnabled setting.
    #[test]
    fn a_path_longer_than_max_path_is_moved_through_the_verbatim_prefix() {
        let dir = temp_dir("files-long");
        let _cleanup = Cleanup(dir.clone());
        let folder = (1..=5).fold(dir.clone(), |path, n| path.join(format!("{n} Synthetic Folder Of A Deep Library")));
        fs::create_dir_all(&folder).unwrap();
        let movie = "Synthetic Movie With A Long Title (2001) {tmdb-900001} - 1080p";
        let (partial, filed, other) = (
            folder.join(format!("{movie}.mkv.partial")),
            folder.join(format!("{movie}.mkv")),
            folder.join(format!("{movie} {{edition-Extended}}.mkv")),
        );
        for path in [&partial, &filed, &other] {
            let units = path.as_os_str().encode_wide().count();
            assert!(units > 260, "{units} units: {}", path.display());
        }
        fs::write(&partial, "placeholder").unwrap();
        move_no_replace(&partial, &filed).unwrap();
        assert!(filed.exists() && !partial.exists());
        fs::write(&other, "old").unwrap();
        move_replacing(&filed, &other).unwrap();
        assert_eq!(fs::read(&other).unwrap(), b"placeholder");
        assert!(!filed.exists());
    }
}

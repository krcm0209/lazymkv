//! The file moves on `MoveFileExW` (S13; design parts 6.5, 7.4) that `write_atomic` and `rename_no_replace` make on
//! Windows.

use std::io;
use std::path::Path;
use std::time::Duration;

use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};

use super::wide;

/// How many times `move_replacing` tries a replace that meets a held target, and the pause between two tries: nine
/// pauses, at most 90 ms in all (Ruling T3-4-a).
const HELD_TRIES: u32 = 10;
const HELD_PAUSE: Duration = Duration::from_millis(10);

/// `path` as the NUL-terminated wide (UTF-16, 16-bit Unicode) string Windows takes. Err `InvalidInput` with the text
/// `the path holds a NUL character` when the path holds one, since Windows would read the name only up to it. Named
/// apart from `win::wide` (T3-2), which takes any text and checks nothing.
fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let name = wide(path);
    if name[..name.len() - 1].contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "the path holds a NUL character"));
    }
    Ok(name)
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
}

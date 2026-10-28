//! The Windows `Ejector` (spec 4.3; design parts 4.2 and 5.6 step 8; PIPE-16): phase 0's four steps on `windows-sys`.
//! The drive id's letter and the report's text sit in `eject`, compiled and tested on every system.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, GetDriveTypeW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use super::wide;
use crate::eject::{drive_letter, eject_report};
use crate::Ejector;

// The control codes and the drive type of `crates/capture/src/eject.rs:39-43`, which experiment 7 ran on both drives.
const DRIVE_CDROM: u32 = 5;
const FSCTL_LOCK_VOLUME: u32 = 0x0009_0018;
const FSCTL_DISMOUNT_VOLUME: u32 = 0x0009_0020;
const IOCTL_STORAGE_MEDIA_REMOVAL: u32 = 0x002D_4804;
const IOCTL_STORAGE_EJECT_MEDIA: u32 = 0x002D_4808;

/// The four steps in order: the report's name, the control code and its input (a one-byte 0 for allow removal).
const STEPS: [(&str, u32, &[u8]); 4] = [
    ("lock", FSCTL_LOCK_VOLUME, &[]),
    ("dismount", FSCTL_DISMOUNT_VOLUME, &[]),
    ("allow-removal", IOCTL_STORAGE_MEDIA_REMOVAL, &[0]),
    ("eject", IOCTL_STORAGE_EJECT_MEDIA, &[]),
];

/// The `Ejector` on Windows (spec 4.3; design parts 4.2 and 5.6 step 8; PIPE-16): phase 0's four steps, lock, dismount,
/// allow removal and eject, rewritten on `windows-sys` from `capture`'s eject, which stays as it is. Every step runs
/// even when an earlier one failed; Ok when the eject step succeeded, else Err with phase 0's report. An id that is
/// not of the `D:` form and a drive that is not optical are refused before anything is opened, and nothing is written
/// to standard output. Def: T3-15. Use: T3-23.
pub struct WinEjector;

impl Ejector for WinEjector {
    fn eject(&self, drive: &str) -> Result<(), String> {
        let letter = drive_letter(drive)?;
        let root = wide(format!(r"{letter}:\"));
        // SAFETY: `root` is NUL-terminated and outlives the call, which only reads it.
        if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_CDROM {
            return Err(format!("{drive} is not an optical drive"));
        }
        let device = wide(format!(r"\\.\{letter}:"));
        // SAFETY: `device` is NUL-terminated and outlives the call, which only reads it; the security attributes and
        // the template file are null, which `CreateFileW` accepts.
        let handle = unsafe {
            CreateFileW(
                device.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            // SAFETY: GetLastError takes no argument and reads this thread's last error, which the failed call set.
            return Err(format!("cannot open drive {drive} (error {})", unsafe { GetLastError() }));
        }
        let drive_handle = OpenDrive(handle);
        run_steps(|code, input| control(&drive_handle, code, input))
    }
}

/// An open drive handle, closed when it goes out of scope, so every path of `eject` closes it once.
struct OpenDrive(HANDLE);

impl Drop for OpenDrive {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful `CreateFileW` in `WinEjector::eject` and is closed once, here.
        unsafe { CloseHandle(self.0) };
    }
}

/// One `DeviceIoControl` call on `drive` with `code` and `input` as its input buffer (null when empty) and no output
/// buffer. Err holds the thread's last error when the call returned zero.
fn control(drive: &OpenDrive, code: u32, input: &[u8]) -> Result<(), u32> {
    let in_buffer = if input.is_empty() { null() } else { input.as_ptr().cast() };
    let mut returned = 0u32;
    // SAFETY: the handle is open for the guard's life; `in_buffer` is null or points at `input.len()` bytes of
    // `input`, which outlives the call, and the call only reads them; the output buffer and the overlapped structure
    // are null, so the call is synchronous and writes only to `returned`, a local that outlives it.
    let done = unsafe {
        DeviceIoControl(drive.0, code, in_buffer, input.len() as u32, null_mut(), 0, &mut returned, null_mut())
    };
    if done != 0 {
        Ok(())
    } else {
        // SAFETY: GetLastError takes no argument and reads this thread's last error, which the failed call set.
        Err(unsafe { GetLastError() })
    }
}

/// Runs every step of STEPS through `control`, each even when an earlier one failed; Ok when the eject step
/// succeeded, else Err(eject_report(..)) (design part 5.6 step 8).
fn run_steps(mut control: impl FnMut(u32, &[u8]) -> Result<(), u32>) -> Result<(), String> {
    let results = STEPS.map(|(name, code, input)| (name, control(code, input)));
    let [.., (_, ejected)] = &results;
    if ejected.is_ok() {
        Ok(())
    } else {
        Err(eject_report(&results))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::testutil::{ci_factor, skip};
    use crate::Ejector;

    #[test]
    fn a_fixed_drive_is_refused_as_not_optical() {
        let system = std::env::var("SystemDrive").expect("SystemDrive is set on Windows").to_ascii_uppercase();
        assert_eq!(WinEjector.eject(&system), Err(format!("{system} is not an optical drive")));
    }

    #[test]
    fn every_step_runs_after_an_earlier_one_fails() {
        let mut seen = Vec::new();
        let result = run_steps(|code, input| {
            seen.push((code, input.to_vec()));
            match code {
                0x0009_0018 => Err(5),
                0x0009_0020 => Err(6),
                _ => Ok(()),
            }
        });
        assert_eq!(result, Ok(()));
        assert_eq!(seen, [(0x0009_0018, vec![]), (0x0009_0020, vec![]), (0x002D_4804, vec![0]), (0x002D_4808, vec![])]);
        let failed = run_steps(|code, _| if code == 0x002D_4808 { Err(1) } else { Ok(()) });
        assert_eq!(failed, Err("lock=ok, dismount=ok, allow-removal=ok, eject=error 1".to_string()));
    }

    #[test]
    #[ignore = "needs a disc in LAZYMKV_TEST_DRIVE; over SSH, as experiment 7 (design part 12.4)"]
    fn the_tray_opens_on_eject() {
        let Ok(drive) = std::env::var("LAZYMKV_TEST_DRIVE") else {
            return skip("LAZYMKV_TEST_DRIVE is not set (design part 12.4)");
        };
        let root = format!(r"{drive}\");
        std::fs::read_dir(&root)
            .map(drop)
            .unwrap_or_else(|error| panic!("{root} must hold a readable disc before the eject: {error}"));
        assert_eq!(WinEjector.eject(&drive), Ok(()));
        let (start, wait) = (Instant::now(), Duration::from_secs(10) * ci_factor());
        while std::fs::read_dir(&root).is_ok() {
            assert!(start.elapsed() < wait, "{root} still reads {wait:?} after the eject");
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

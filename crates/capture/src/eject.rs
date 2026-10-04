//! Ejects an optical drive's tray (spec 4.3, Ejector). Exercised on real drives in Task 14.

#[cfg(windows)]
pub fn eject(drive_letter: char) -> Result<(), String> {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn DeviceIoControl(
            handle: *mut c_void,
            code: u32,
            in_buffer: *const c_void,
            in_size: u32,
            out_buffer: *mut c_void,
            out_size: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const FSCTL_LOCK_VOLUME: u32 = 0x0009_0018;
    const FSCTL_DISMOUNT_VOLUME: u32 = 0x0009_0020;
    const IOCTL_STORAGE_MEDIA_REMOVAL: u32 = 0x002D_4804;
    const IOCTL_STORAGE_EJECT_MEDIA: u32 = 0x002D_4808;

    let device: Vec<u16> = format!(r"\\.\{drive_letter}:")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = CreateFileW(
            device.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null_mut(),
            OPEN_EXISTING,
            0,
            null_mut(),
        );
        if handle as isize == -1 {
            return Err(format!("cannot open drive {drive_letter}: (error {})", GetLastError()));
        }

        let mut steps: Vec<String> = Vec::new();
        let mut control = |name: &str, code: u32, in_buffer: *const c_void, in_size: u32| -> bool {
            let mut returned = 0u32;
            let ok = DeviceIoControl(handle, code, in_buffer, in_size, null_mut(), 0, &mut returned, null_mut()) != 0;
            steps.push(if ok {
                format!("{name}=ok")
            } else {
                format!("{name}=error {}", GetLastError())
            });
            ok
        };

        control("lock", FSCTL_LOCK_VOLUME, null(), 0);
        control("dismount", FSCTL_DISMOUNT_VOLUME, null(), 0);
        let allow_removal: u8 = 0;
        control(
            "allow-removal",
            IOCTL_STORAGE_MEDIA_REMOVAL,
            &allow_removal as *const u8 as *const c_void,
            1,
        );
        let ejected = control("eject", IOCTL_STORAGE_EJECT_MEDIA, null(), 0);
        CloseHandle(handle);

        let report = steps.join(", ");
        if ejected {
            println!("{report}");
            Ok(())
        } else {
            Err(report)
        }
    }
}

#[cfg(not(windows))]
pub fn eject(_drive_letter: char) -> Result<(), String> {
    Err("eject is only implemented on Windows".to_string())
}

//! Drive type: is a folder on a spinning hard disk (HDD)?
//!
//! There, one file after the other is fastest, because otherwise the read head jumps between
//! the files. Measured on 2026-10-18 on an 8 TB HDD, read buffered:
//! 1 thread 222–234 MB/s, 2–8 threads 180–209 MB/s, 16 threads 164–178 MB/s.

use std::path::Path;

/// How many files are hashed at once: `wanted` from the toolbar (*Threads*), on an
/// HDD (`seeks`) one.
pub(crate) fn hash_threads(wanted: usize, seeks: Option<bool>) -> usize {
    if seeks == Some(true) {
        1
    } else {
        wanted.max(1)
    }
}

/// `Some(true)` on an HDD, `Some(false)` on an SSD and the like. `None`
/// if Windows does not say, for example on network drives or volumes spanning several disks.
#[cfg(windows)]
#[allow(unsafe_code)] // Windows API; every block has a // SAFETY: comment
pub(crate) fn seek_penalty(path: &Path) -> Option<bool> {
    use std::ffi::c_void;
    use std::mem::{offset_of, size_of};
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetVolumePathNameW(file: *const u16, mount: *mut u16, len: u32) -> i32;
        fn GetVolumeNameForVolumeMountPointW(mount: *const u16, name: *mut u16, len: u32) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn DeviceIoControl(
            device: *mut c_void,
            code: u32,
            input: *const c_void,
            input_len: u32,
            output: *mut c_void,
            output_len: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;
    const OPEN_EXISTING: u32 = 3;
    const INVALID_HANDLE_VALUE: isize = -1;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
    const STORAGE_DEVICE_SEEK_PENALTY_PROPERTY: u32 = 7;
    const PROPERTY_STANDARD_QUERY: u32 = 0;

    /// `STORAGE_PROPERTY_QUERY`
    #[repr(C)]
    struct Query {
        property_id: u32,
        query_type: u32,
        additional: [u8; 4],
    }

    /// `DEVICE_SEEK_PENALTY_DESCRIPTOR`
    #[repr(C)]
    struct SeekPenalty {
        version: u32,
        size: u32,
        incurs_seek_penalty: u8,
    }

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut mount = [0u16; 1024];
    // SAFETY: `wide` is null-terminated; the length passed for `mount` is its real length.
    if unsafe { GetVolumePathNameW(wide.as_ptr(), mount.as_mut_ptr(), mount.len() as u32) } == 0 {
        return None;
    }
    let mut volume = [0u16; 64];
    // SAFETY: `mount` is the null-terminated mount point ("D:\"); the length passed for
    // `volume` is its real length.
    let named = unsafe {
        GetVolumeNameForVolumeMountPointW(mount.as_ptr(), volume.as_mut_ptr(), volume.len() as u32)
    };
    if named == 0 {
        return None;
    }
    // Without the final "\", `\\?\Volume{…}` opens the volume instead of its root folder.
    let end = volume.iter().position(|&unit| unit == 0)?;
    if end > 0 && volume[end - 1] == u16::from(b'\\') {
        volume[end - 1] = 0;
    }
    // SAFETY: `volume` is null-terminated. Access 0 suffices for the query and needs no
    // administrator rights.
    let handle = unsafe {
        CreateFileW(
            volume.as_ptr(),
            0,
            FILE_SHARE_READ_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle.is_null() || handle as isize == INVALID_HANDLE_VALUE {
        return None;
    }
    let query = Query {
        property_id: STORAGE_DEVICE_SEEK_PENALTY_PROPERTY,
        query_type: PROPERTY_STANDARD_QUERY,
        additional: [0; 4],
    };
    let mut answer = SeekPenalty {
        version: 0,
        size: 0,
        incurs_seek_penalty: 0,
    };
    let mut returned = 0u32;
    // SAFETY: `handle` is open; input and output are `repr(C)` and the sizes passed are
    // their real sizes.
    let asked = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const Query).cast(),
            size_of::<Query>() as u32,
            (&mut answer as *mut SeekPenalty).cast(),
            size_of::<SeekPenalty>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    // SAFETY: `handle` comes from `CreateFileW` and is closed exactly once.
    unsafe { CloseHandle(handle) };
    let filled = returned as usize > offset_of!(SeekPenalty, incurs_seek_penalty);
    (asked != 0 && filled).then_some(answer.incurs_seek_penalty != 0)
}

/// `Some(true)` on an HDD (`queue/rotational` is 1), `None` if
/// there is no block device for it (network drive, tmpfs, btrfs subvolume).
#[cfg(target_os = "linux")]
pub(crate) fn seek_penalty(path: &Path) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    let dev = std::fs::metadata(path).ok()?.dev();
    // Like `major()` and `minor()` from glibc.
    let major = ((dev >> 32) & 0xffff_f000) | ((dev >> 8) & 0x0000_0fff);
    let minor = ((dev >> 12) & 0xffff_ff00) | (dev & 0x0000_00ff);
    let device = std::fs::canonicalize(format!("/sys/dev/block/{major}:{minor}")).ok()?;
    // A partition has no `queue` of its own; then the disk above it applies.
    for dir in [Some(device.as_path()), device.parent()]
        .into_iter()
        .flatten()
    {
        if let Ok(text) = std::fs::read_to_string(dir.join("queue/rotational")) {
            return Some(text.trim() == "1");
        }
    }
    None
}

#[cfg(not(any(windows, target_os = "linux")))]
pub(crate) fn seek_penalty(_path: &Path) -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinning_disks_hash_one_file_at_a_time() {
        assert_eq!(hash_threads(16, Some(true)), 1);
        assert_eq!(hash_threads(16, Some(false)), 16);
        assert_eq!(hash_threads(16, None), 16);
        assert_eq!(hash_threads(0, None), 1);
    }

    #[test]
    fn asking_the_drive_does_not_fail() {
        // The answer depends on the machine; the query itself must not fail.
        let _ = seek_penalty(&std::env::temp_dir());
        #[cfg(target_os = "linux")]
        assert_eq!(seek_penalty(Path::new("/gibt/es/nicht/veritas")), None);
    }
}

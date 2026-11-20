//! What Veritas asks of the system: recycle bin and file manager.

use std::path::Path;

use crate::i18n::{plain_io, Say};

pub(crate) fn move_to_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|err| err.to_string())
}

/// Windows: Explorer with the file selected. `raw_arg`, because Explorer expects `/select,"…"`
/// exactly like this and does not understand the usual quoting of `Command`.
#[cfg(windows)]
pub(crate) fn show_in_folder(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("explorer")
        // Explorer understands `/select,` only with backslashes.
        .raw_arg(format!(
            "/select,\"{}\"",
            path.display().to_string().replace('/', "\\")
        ))
        .spawn()
        .map(|_| ())
        .map_err(|err| plain_io(&err))
}

/// Otherwise the file's folder in the file manager the system provides.
#[cfg(not(windows))]
pub(crate) fn show_in_folder(path: &Path) -> Result<(), String> {
    let folder = path
        .parent()
        .ok_or_else(|| crate::i18n::t().sys_not_found.to_string())?;
    open::that_detached(folder).map_err(|err| plain_io(&err))
}

/// On Windows, network and removable drives have no recycle bin; deleting there would be
/// permanent. Then the text says why Veritas does not delete. On Linux, `trash` creates a
/// recycle bin on every drive or reports an error.
#[cfg(windows)]
#[allow(unsafe_code)] // Windows API; every block has a // SAFETY: comment
pub(crate) fn no_trash(path: &Path) -> Option<Say> {
    use std::path::{Component, Prefix};
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDriveTypeW(root: *const u16) -> u32;
    }
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_REMOTE: u32 = 4;
    let letter = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return Some(Say::of(|t| t.no_trash_unc)),
        },
        _ => return None,
    };
    let root: Vec<u16> = format!("{}:\\", char::from(letter))
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: `root` is a null-terminated UTF-16 path like "D:\", which outlives the call.
    match unsafe { GetDriveTypeW(root.as_ptr()) } {
        DRIVE_REMOTE => Some(Say::of(|t| t.no_trash_network)),
        DRIVE_REMOVABLE => Some(Say::of(|t| t.no_trash_removable)),
        _ => None,
    }
}

#[cfg(not(windows))]
pub(crate) fn no_trash(_path: &Path) -> Option<Say> {
    None
}

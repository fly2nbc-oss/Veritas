//! The preview when a folder is opened: files and entries of the checksum file, without hashing.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::checksum;
use crate::drive;
use crate::hashing::{Digests, HashMode};
use crate::history::Timeline;
use crate::i18n::Msg;
use crate::report::{path_key, FOLD_CASE};
use crate::scanning::{plain_file_meta, rel_to_path, FileRow, RowKind};

/// Everything that opening a folder reads, without hashing. Built in a thread of its own (`load`),
/// so that a large folder does not stall the UI.
pub(crate) struct Preview {
    /// The checksum file that applies to the folder (`md5_choice`).
    pub(crate) md5_name: String,
    pub(crate) md5_exists: bool,
    pub(crate) rows: Vec<FileRow>,
    /// Note on the checksum file (encoding, not readable).
    pub(crate) note: Option<String>,
    pub(crate) timeline: Result<Timeline, String>,
    /// Classic hard disk (HDD)? (`drive::seek_penalty`)
    pub(crate) seeks: Option<bool>,
    /// See `Listing::recorded`.
    pub(crate) recorded: HashSet<String>,
    /// Set when an SHA file applies and `{folder name}.md5` already lies next to it (only if the
    /// SHA file was opened explicitly): its paths, like `recorded`. "Create MD5" then replaces that
    /// `.md5`, and the confirmation names its entries.
    pub(crate) md5_recorded: Option<HashSet<String>>,
}

/// The rows of the preview (`preview_files`).
pub(crate) struct Listing {
    pub(crate) rows: Vec<FileRow>,
    /// Note on the checksum file (encoding, not readable).
    pub(crate) note: Option<String>,
    /// Every path of the checksum file as a `path_key`, including those in subfolders when these
    /// are off. Confirmations ("Replace?", delete) do not read the file again that way.
    pub(crate) recorded: HashSet<String>,
}

/// Reads the checksum file, files, history and drive type of a folder. `pinned` is an explicitly
/// opened checksum file that applies instead of the rule as long as it exists. `stop`: nobody waits
/// for the result any more (another folder is chosen); reading then ends early and the result is
/// incomplete.
pub(crate) fn load(
    root: &Path,
    pinned: Option<&str>,
    recursive: bool,
    stop: &AtomicBool,
) -> Preview {
    let (md5_name, md5_exists) = md5_choice(root, pinned);
    let listing = preview_files(root, &md5_name, recursive, stop);
    let stopped = stop.load(Ordering::Relaxed);
    let timeline = if stopped {
        Err(NO_FOLDER.into())
    } else {
        history_of(root, &md5_name)
    };
    let seeks = if stopped {
        None
    } else {
        drive::seek_penalty(root)
    };
    let md5_recorded = if stopped {
        None
    } else {
        md5_beside(root, &md5_name)
    };
    Preview {
        md5_name,
        md5_exists,
        rows: listing.rows,
        note: listing.note,
        timeline,
        seeks,
        recorded: listing.recorded,
        md5_recorded,
    }
}

/// The paths (`path_key`) of the `.md5` that "Create MD5" would replace next to the SHA file
/// `chosen`; `None` for a `.md5` or if there is none.
fn md5_beside(root: &Path, chosen: &str) -> Option<HashSet<String>> {
    if !checksum::is_sha_filename(chosen) {
        return None;
    }
    let target = checksum::names_for(root, chosen).md5;
    if !root.join(&target).is_file() {
        return None;
    }
    let (entries, _) = read_recorded_md5(root, &target);
    Some(
        entries
            .keys()
            .map(|path| path_key(path, FOLD_CASE))
            .collect(),
    )
}

/// The folder's checksum file and whether it exists: the explicitly opened one, otherwise the rule
/// (`checksum::choose_md5_file`).
pub(crate) fn md5_choice(root: &Path, pinned: Option<&str>) -> (String, bool) {
    let name = match pinned {
        Some(name) if root.join(name).is_file() => name.to_string(),
        _ => checksum::choose_md5_file(root),
    };
    let exists = root.join(&name).is_file();
    (name, exists)
}

/// The history next to the checksum file `md5_name`; otherwise the reason why there is none.
pub(crate) fn history_of(root: &Path, md5_name: &str) -> Result<Timeline, String> {
    if md5_name.is_empty() {
        return Err(NO_FOLDER.into());
    }
    let name = checksum::names_for(root, md5_name).json;
    Timeline::load(&root.join(name))
}

/// Reason while no folder is chosen.
pub(crate) const NO_FOLDER: &str = Msg::NoFolder.en();

/// Rows when a folder is opened: the files in the folder and every entry of the checksum file,
/// without hashing. Entries in subfolders count as open if the file exists, otherwise as deleted.
/// Without subfolders these entries are left out. `stop` as in `load`.
pub(crate) fn preview_files(
    root: &Path,
    chosen: &str,
    recursive: bool,
    stop: &AtomicBool,
) -> Listing {
    let names = checksum::names_for(root, chosen);
    let skip = checksum::excluded_filenames(root, &names);
    let algo = checksum::algorithm_of_name(chosen).unwrap_or(HashMode::Md5);
    let (recorded, note) = read_recorded_md5(root, chosen);
    let same = |name: &str, other: &str| path_key(name, FOLD_CASE) == path_key(other, FOLD_CASE);
    let by_key: HashMap<String, &String> = recorded
        .iter()
        .map(|(path, hash)| (path_key(path, FOLD_CASE), hash))
        .collect();
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    if let Ok(read) = std::fs::read_dir(root) {
        for entry in read.flatten() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let own = if same(&name, chosen) {
                Some(RowKind::Checksum)
            } else if same(&name, &names.json) {
                Some(RowKind::Report)
            } else {
                None
            };
            if own.is_none() && checksum::is_excluded_name(&skip, &name) {
                continue;
            }
            let key = path_key(&name, FOLD_CASE);
            // Foreign SHA files as in the run (`drop_foreign_checksums`): only if the checksum file
            // lists them.
            if own.is_none() && checksum::is_sha_filename(&name) && !by_key.contains_key(&key) {
                continue;
            }
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            if let Some(kind) = own {
                rows.push(own_row(kind, name, path, &meta));
                continue;
            }
            let hash = by_key
                .get(&key)
                .map(|hash| Digests::of(algo, (*hash).clone()));
            seen.insert(key);
            let kind = if hash.is_some() {
                RowKind::Recorded
            } else {
                RowKind::Added
            };
            rows.push(FileRow {
                old_hash: hash.unwrap_or_default(),
                ..FileRow::bare(name, Some(path), kind).with_meta(&meta)
            });
        }
    }
    // Checked folders below `root`: no link, no second read per entry.
    let mut checked = HashSet::new();
    for (name, hash) in &recorded {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if seen.contains(&path_key(name, FOLD_CASE)) {
            continue;
        }
        if !recursive && name.contains('/') {
            continue;
        }
        // Like the run: only regular files inside the folder, never follow a link.
        let absolute = root.join(rel_to_path(name));
        let row = match plain_file_meta(root, &absolute, &mut checked) {
            Ok(meta) => {
                FileRow::bare(name.clone(), Some(absolute), RowKind::Recorded).with_meta(&meta)
            }
            Err(_) => FileRow::bare(name.clone(), None, RowKind::Removed),
        };
        rows.push(FileRow {
            old_hash: Digests::of(algo, hash.clone()),
            ..row
        });
    }
    rows.sort_by(|left, right| left.path.cmp(&right.path));
    Listing {
        rows,
        note,
        recorded: by_key.into_keys().collect(),
    }
}

/// The entries of the chosen file, read like in the run (`read_checksum_file`); only those with the
/// hash length of their algorithm (`.md5` 32, `SHA256SUMS` 64 …). The second field says when the
/// file was not read as UTF-8 or was not readable.
pub(crate) fn read_recorded_md5(
    root: &Path,
    chosen: &str,
) -> (BTreeMap<String, String>, Option<String>) {
    if chosen.is_empty() {
        return (BTreeMap::new(), None);
    }
    let path = root.join(chosen);
    let file = match checksum::read_checksum_file(&path) {
        Ok(read) => read,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return (BTreeMap::new(), None),
        Err(err) if err.kind() == std::io::ErrorKind::FileTooLarge => {
            return (BTreeMap::new(), Some(err.to_string()))
        }
        Err(err) => {
            return (
                BTreeMap::new(),
                Some(Msg::Unreadable.fill(&[chosen, &err.to_string()])),
            )
        }
    };
    let length = checksum::algorithm_of_name(chosen)
        .unwrap_or(HashMode::Md5)
        .hex_len();
    let map = file
        .entries
        .into_iter()
        .filter(|(_, hash)| Some(hash.len()) == length)
        .collect();
    let note = file
        .encoding
        .map(|encoding| Msg::PreviewNotUtf8.fill(&[chosen, encoding]));
    (map, note)
}

/// Row for a file that Veritas manages itself: the checksum file or the history.
fn own_row(kind: RowKind, name: String, path: PathBuf, meta: &std::fs::Metadata) -> FileRow {
    FileRow::bare(name, Some(path), kind).with_meta(meta)
}

/// After a run: put the checksum file and the history back into the list.
pub(crate) fn include_own_rows(root: &Path, md5_name: &str, rows: &mut Vec<FileRow>) {
    if md5_name.is_empty() {
        return;
    }
    let history = checksum::names_for(root, md5_name).json;
    for (kind, name) in [(RowKind::Checksum, md5_name), (RowKind::Report, &history)] {
        if rows.iter().any(|row| row.path == name) {
            continue;
        }
        let path = root.join(name);
        if let Some(meta) = path.metadata().ok().filter(|meta| meta.is_file()) {
            rows.push(own_row(kind, name.to_string(), path, &meta));
        }
    }
    rows.sort_by(|left, right| left.path.cmp(&right.path));
}

/// File name if `path` is a checksum file (`*.md5`, `*.sha256`, `SHA256SUMS` …,
/// `checksum::algorithm_of_name`).
pub(crate) fn md5_file_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| checksum::algorithm_of_name(name).is_some())
        .map(str::to_string)
}

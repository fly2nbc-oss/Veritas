use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::hashing::Digests;
use crate::i18n::Msg;
use crate::integrity::IntegrityStatus;

/// File that was read successfully in the current run.
#[derive(Debug, Clone)]
pub struct CurrentFile {
    pub path: String,
    pub absolute: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub hash: Digests,
}

#[derive(Debug, Clone)]
pub struct ModifiedPair {
    pub file: CurrentFile,
    pub old_hash: Digests,
}

#[derive(Debug, Clone)]
pub struct RemovedFile {
    pub path: String,
    pub hash: Digests,
}

#[derive(Debug, Clone, Default)]
pub struct Classified {
    pub unchanged: Vec<CurrentFile>,
    pub modified: Vec<ModifiedPair>,
    pub added: Vec<CurrentFile>,
    pub removed: Vec<RemovedFile>,
}

/// Windows does not distinguish upper and lower case in file names.
pub(crate) const FOLD_CASE: bool = cfg!(windows);

/// Comparison key of a relative path. Lowercased with `fold_case`.
pub(crate) fn path_key(path: &str, fold_case: bool) -> String {
    if fold_case {
        path.to_lowercase()
    } else {
        path.to_owned()
    }
}

/// The verdict on a file that was read: added, unchanged or modified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Added,
    Unchanged,
    Modified,
}

/// The one rule for a single file, shared by `classify` and the intermediate states of a run:
/// without an entry added; if every comparable checksum matches, unchanged; otherwise modified
/// (also when no checksum is comparable).
pub(crate) fn verdict(old: Option<&Digests>, new: &Digests) -> Verdict {
    match old.map(|old| old.comparable_equal(new)) {
        None => Verdict::Added,
        Some(Some(true)) => Verdict::Unchanged,
        Some(Some(false) | None) => Verdict::Modified,
    }
}

/// Compares the previous checksum with the current state.
/// Read errors do not count as missing.
pub fn classify(
    previous: &BTreeMap<String, Digests>,
    current: Vec<CurrentFile>,
    error_paths: &HashSet<String>,
) -> Classified {
    classify_with(previous, current, error_paths, FOLD_CASE)
}

fn classify_with(
    previous: &BTreeMap<String, Digests>,
    current: Vec<CurrentFile>,
    error_paths: &HashSet<String>,
    fold_case: bool,
) -> Classified {
    let by_key: HashMap<String, &Digests> = previous
        .iter()
        .map(|(path, hash)| (path_key(path, fold_case), hash))
        .collect();
    let errors: HashSet<String> = error_paths
        .iter()
        .map(|path| path_key(path, fold_case))
        .collect();
    let mut seen = HashSet::new();
    let mut unchanged = Vec::new();
    let mut modified = Vec::new();
    let mut added = Vec::new();
    for file in current {
        let key = path_key(&file.path, fold_case);
        let old = by_key.get(&key).copied();
        seen.insert(key);
        match (verdict(old, &file.hash), old) {
            (Verdict::Unchanged, _) => unchanged.push(file),
            (Verdict::Modified, Some(old)) => modified.push(ModifiedPair {
                old_hash: old.clone(),
                file,
            }),
            (Verdict::Added | Verdict::Modified, _) => added.push(file),
        }
    }
    let mut removed = Vec::new();
    for (path, hash) in previous {
        let key = path_key(path, fold_case);
        if !seen.contains(&key) && !errors.contains(&key) {
            removed.push(RemovedFile {
                path: path.clone(),
                hash: hash.clone(),
            });
        }
    }
    unchanged.sort_by(|left, right| left.path.cmp(&right.path));
    modified.sort_by(|left, right| left.file.path.cmp(&right.file.path));
    added.sort_by(|left, right| left.path.cmp(&right.path));
    removed.sort_by(|left, right| left.path.cmp(&right.path));
    Classified {
        unchanged,
        modified,
        added,
        removed,
    }
}

/// What a run did. English is written (since the language choice); older histories contain the
/// German values, which are still read as an alias.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Action {
    /// "MD5 erzeugen" (create MD5).
    #[serde(rename = "created", alias = "erzeugt")]
    Created,
    /// "Prüfen" (verify).
    #[serde(rename = "verified", alias = "geprüft")]
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeReport {
    pub timestamp: String,
    pub base_directory: String,
    pub hash_mode: String,
    pub action: Action,
    /// True if there was no previous checksum file and only a baseline was created.
    pub baseline: bool,
    pub unchanged_count: u64,
    pub file_count: u64,
    pub modified: Vec<ModifiedEntry>,
    pub added: Vec<PathEntry>,
    pub removed: Vec<PathEntry>,
    pub errors: Vec<ErrorEntry>,
    pub skipped_symlinks: u64,
    pub skipped_non_regular: u64,
    pub cancelled: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModifiedEntry {
    pub path: String,
    pub old_hash: Digests,
    pub new_hash: Digests,
    pub integrity_status: IntegrityStatus,
    pub integrity_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathEntry {
    pub path: String,
    pub hash: Digests,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorEntry {
    pub path: String,
    pub message: String,
    /// Previous hash from the checksum file, if the file was listed there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_hash: Option<Digests>,
}

/// Veritas writes single runs only in the history now; the tests check the old format.
#[cfg(test)]
impl ChangeReport {
    pub fn to_pretty_json(&self) -> Result<Vec<u8>, Error> {
        pretty_json(self)
    }
}

/// Identifier of the history format in `{folder name}.json`.
pub const HISTORY_FORMAT: &str = "veritas-verlauf-1";
/// All runs of a folder, oldest first. Every create and every verify appends a run;
/// nothing drops out. Instead of a `.md5.bak`, the history records what a replacement
/// changed: together with the new `.md5` it yields the old one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct History {
    pub format: String,
    pub runs: Vec<ChangeReport>,
}

impl History {
    pub fn new(runs: Vec<ChangeReport>) -> Self {
        Self {
            format: HISTORY_FORMAT.to_string(),
            runs,
        }
    }

    /// Reads an existing history. A report in the earlier single format becomes the first
    /// run. If the text is neither, a new history begins and the second field
    /// is `false`.
    pub fn from_text(text: &str) -> (Self, bool) {
        if let Ok(history) = serde_json::from_str::<History>(text) {
            if history.format == HISTORY_FORMAT {
                return (history, true);
            }
        }
        if let Ok(single) = serde_json::from_str::<ChangeReport>(text) {
            return (Self::new(vec![single]), true);
        }
        (Self::new(Vec::new()), false)
    }

    /// The format identifier if the text is a Veritas history in a different (newer)
    /// format, for example after a downgrade. This version must not replace it.
    pub fn other_format(text: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let format = value.get("format")?.as_str()?;
        (format.starts_with("veritas-verlauf-") && format != HISTORY_FORMAT)
            .then(|| format.to_string())
    }

    pub fn push(&mut self, run: ChangeReport) {
        self.runs.push(run);
    }

    pub fn to_pretty_json(&self) -> Result<Vec<u8>, Error> {
        pretty_json(self)
    }
}

fn pretty_json<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|err| Error::Message(Msg::JsonFailed.fill(&[&err.to_string()])))?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrity::IntegrityStatus;

    fn md5(value: &str) -> Digests {
        crate::hashing::Digests::of(crate::hashing::HashMode::Md5, value.to_string())
    }

    fn file(path: &str, hash: &str) -> CurrentFile {
        CurrentFile {
            path: path.to_string(),
            absolute: PathBuf::from(path),
            size: 1,
            modified: None,
            hash: md5(hash),
        }
    }

    #[test]
    fn classifies_added_modified_removed_and_skips_read_errors() {
        let mut previous = BTreeMap::new();
        previous.insert("a.txt".into(), md5("AAA"));
        previous.insert("b.txt".into(), md5("BBB"));
        previous.insert("c.txt".into(), md5("CCC"));
        previous.insert("locked.txt".into(), md5("LLL"));
        let current = vec![
            file("b.txt", "XXX"),
            file("a.txt", "AAA"),
            file("d.txt", "DDD"),
        ];
        let mut errors = HashSet::new();
        errors.insert("locked.txt".to_string());
        let diff = classify(&previous, current, &errors);
        assert_eq!(diff.unchanged.len(), 1);
        assert_eq!(diff.unchanged[0].path, "a.txt");
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.modified[0].file.path, "b.txt");
        assert_eq!(diff.modified[0].old_hash.md5.as_deref(), Some("BBB"));
        assert_eq!(diff.modified[0].file.hash.md5.as_deref(), Some("XXX"));
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].path, "d.txt");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].path, "c.txt");
    }

    #[test]
    fn case_folding_matches_names_that_differ_only_in_case() {
        let mut previous = BTreeMap::new();
        previous.insert("Fotos/IMG_1.JPG".into(), md5("AAA"));
        let current = vec![file("fotos/img_1.jpg", "AAA")];
        let folded = classify_with(&previous, current.clone(), &HashSet::new(), true);
        assert_eq!(folded.unchanged.len(), 1);
        assert!(folded.added.is_empty());
        assert!(folded.removed.is_empty());
        let exact = classify_with(&previous, current, &HashSet::new(), false);
        assert_eq!(exact.added.len(), 1);
        assert_eq!(exact.removed.len(), 1);
    }

    #[test]
    fn json_report_has_required_shape() {
        let report = ChangeReport {
            timestamp: "2026-10-12T19:02:00+00:00".into(),
            base_directory: "/medien/fotos".into(),
            hash_mode: "md5".into(),
            action: Action::Verified,
            baseline: false,
            unchanged_count: 4,
            file_count: 6,
            modified: vec![ModifiedEntry {
                path: "bilder/a.jpg".into(),
                old_hash: md5("AAA"),
                new_hash: md5("BBB"),
                integrity_status: IntegrityStatus::PossiblyDamaged,
                integrity_reason: "JPEG: EOI (FF D9) fehlt am Dateiende".into(),
                size: Some(12),
            }],
            added: vec![PathEntry {
                path: "neu.txt".into(),
                hash: md5("CCC"),
                size: Some(3),
            }],
            removed: vec![PathEntry {
                path: "alt.txt".into(),
                hash: md5("DDD"),
                size: None,
            }],
            errors: vec![ErrorEntry {
                path: "gesperrt.txt".into(),
                message: "Keine Leseberechtigung".into(),
                old_hash: None,
            }],
            skipped_symlinks: 1,
            skipped_non_regular: 0,
            cancelled: false,
            warnings: vec![],
        };
        let bytes = report.to_pretty_json().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["base_directory"], "/medien/fotos");
        assert_eq!(value["unchanged_count"], 4);
        assert_eq!(value["modified"][0]["path"], "bilder/a.jpg");
        assert_eq!(value["modified"][0]["old_hash"]["md5"], "AAA");
        assert_eq!(value["modified"][0]["new_hash"]["md5"], "BBB");
        assert!(value["modified"][0]["old_hash"].get("sha256").is_none());
        assert_eq!(value["modified"][0]["integrity_status"], "possibly damaged");
        assert!(value["modified"][0]["integrity_reason"]
            .as_str()
            .unwrap()
            .contains("EOI"));
        assert_eq!(value["added"][0]["path"], "neu.txt");
        assert_eq!(value["removed"][0]["path"], "alt.txt");
        assert_eq!(value["errors"][0]["message"], "Keine Leseberechtigung");
        let parsed: ChangeReport = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed, report);
        assert!(parsed.timestamp.parse::<jiff::Timestamp>().is_ok());
    }
}

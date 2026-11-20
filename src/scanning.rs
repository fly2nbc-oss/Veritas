use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::checksum::{
    excluded_filenames, format_checksum, is_excluded_name, is_sha_filename, names_for,
    output_names, read_checksum_file, read_limited, NameStyle, OutputNames, MAX_HISTORY_BYTES,
};
use crate::drive;
use crate::error::{file_problem, Error};
use crate::format::plural;
use crate::hashing::{hash_file, Digests, HashMode};
use crate::i18n::Msg;
use crate::integrity::{check_path, IntegrityFinding, IntegrityStatus};
use crate::progress::{LiveRow, Progress};
use crate::report::{
    classify, path_key, verdict, Action, ChangeReport, Classified, CurrentFile, ErrorEntry,
    History, ModifiedEntry, ModifiedPair, PathEntry, Verdict, FOLD_CASE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Generate,
    Verify,
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub root: PathBuf,
    pub recursive: bool,
    /// At most this many files at once; one on a classic hard disk (HDD).
    pub threads: usize,
    /// HDD? Already known when the UI opened the folder; `None` makes `run_scan` ask itself
    /// (`drive::seek_penalty`).
    pub seeks: Option<bool>,
    pub hash_mode: HashMode,
    pub name_style: NameStyle,
    /// A specific checksum file in the folder (`.md5` or SHA file, `checksum::names_for`). `None`
    /// follows `name_style`.
    pub md5_file: Option<String>,
    pub kind: JobKind,
    /// Checksums of an earlier run in the same folder. Only Create uses them, and only if they fit
    /// these settings (`Known::usable_for`).
    pub known: Option<Arc<Known>>,
}

/// Margin for the modification time: FAT and exFAT store it only to 2 s accuracy. If it was closer
/// than that to the start of reading, the file could have changed once more afterwards without size
/// or time showing it – then the result is not carried over.
const MTIME_SLACK: Duration = Duration::from_secs(2);

/// Size and modification time of a regular file as `fs::symlink_metadata` reports them. The same
/// source when reading and when carrying over – not the metadata from the walk, which on Windows
/// come from the directory entry and can be stale there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: SystemTime,
}

impl Stamp {
    fn of(path: &Path) -> Option<Self> {
        let meta = fs::symlink_metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        Some(Self {
            size: meta.len(),
            modified: meta.modified().ok()?,
        })
    }

    /// Was the modification time at least `MTIME_SLACK` before `started`? A time in the future
    /// (other clock, network drive) does not count.
    fn settled_before(&self, started: SystemTime) -> bool {
        self.modified
            .checked_add(MTIME_SLACK)
            .is_some_and(|limit| limit <= started)
    }
}

/// A file that was read and whose checksum a later Create may carry over.
#[derive(Debug, Clone)]
struct KnownFile {
    /// Size and modification time before and after reading (equal, otherwise the entry would not
    /// exist).
    stamp: Stamp,
    hash: Digests,
    /// The structure check, if the file mismatched there.
    integrity: Option<IntegrityFinding>,
}

/// What a finished run knows about the files it read. "Create MD5" takes from it the checksum of
/// every file whose size and modification time have stayed the same, and reads only new, changed
/// and previously unreadable files.
#[derive(Debug, Clone)]
pub struct Known {
    /// The run the checksums come from.
    pub source: JobKind,
    /// When this run finished.
    pub at: jiff::Timestamp,
    root: PathBuf,
    recursive: bool,
    hash_mode: HashMode,
    /// The checksum files of the run with size and modification time, as it read or wrote them. If
    /// one changes, nothing applies any more.
    checksums: Vec<(PathBuf, Option<Stamp>)>,
    /// Per `path_key` of the relative path.
    files: HashMap<String, KnownFile>,
}

impl Known {
    /// How many checksums are known.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Fits a Create run with these settings: the same folder, the same *Subfolders* setting, the
    /// same algorithms, and the checksum files are unchanged since the run. Asks the file system
    /// (once per checksum file).
    pub fn usable_for(&self, opts: &ScanOptions) -> bool {
        opts.kind == JobKind::Generate
            && self.root == opts.root
            && self.recursive == opts.recursive
            && self.hash_mode == opts.hash_mode
            && checksum_stamps(&opts.root, opts) == self.checksums
    }
}

/// Size and modification time of this run's checksum files; `None` where one is missing.
fn checksum_stamps(root: &Path, opts: &ScanOptions) -> Vec<(PathBuf, Option<Stamp>)> {
    checksum_targets(root, opts.hash_mode, &output_names_for(root, opts))
        .into_iter()
        .map(|(_, path)| {
            let stamp = Stamp::of(&path);
            (path, stamp)
        })
        .collect()
}

/// The names of checksum file and history: the chosen checksum file or the rule. If it is an SHA
/// file, Verify reads it and Create writes `{folder name}.md5`.
fn output_names_for(root: &Path, opts: &ScanOptions) -> OutputNames {
    match opts.md5_file.as_deref() {
        Some(file) if !file.is_empty() => names_for(root, file),
        _ => output_names(root, opts.name_style),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Recorded,
    Unchanged,
    Modified,
    Added,
    Removed,
    Error,
    Partial,
    /// The chosen checksum file. Neutral in the list, not "ok".
    Checksum,
    /// The history `{folder name}.json` next to the checksum file.
    Report,
}

#[derive(Debug, Clone)]
pub struct FileRow {
    pub path: String,
    pub absolute: Option<PathBuf>,
    pub kind: RowKind,
    pub size: Option<u64>,
    /// Modification time, captured once when the folder is read.
    pub modified: Option<SystemTime>,
    pub old_hash: Digests,
    pub new_hash: Digests,
    pub integrity: Option<IntegrityFinding>,
    pub message: Option<String>,
}

/// Status of a row as the list and chips show it. Grey means open or new, green is correct, yellow
/// mismatches or is missing, red is damaged or unreadable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowMark {
    Open,
    Ok,
    Wrong,
    Damaged,
    Deleted,
    New,
    Error,
    /// The chosen checksum file itself.
    Source,
    /// The history `{folder name}.json`.
    Report,
}

impl FileRow {
    /// A row without size, date, checksums, finding and message.
    pub(crate) fn bare(path: String, absolute: Option<PathBuf>, kind: RowKind) -> Self {
        Self {
            path,
            absolute,
            kind,
            size: None,
            modified: None,
            old_hash: Digests::default(),
            new_hash: Digests::default(),
            integrity: None,
            message: None,
        }
    }

    /// A file that was read, with its new checksum; `old_hash` comes from the checksum file.
    fn hashed(file: &CurrentFile, kind: RowKind, old_hash: Digests) -> Self {
        Self {
            size: Some(file.size),
            modified: file.modified,
            old_hash,
            new_hash: file.hash.clone(),
            ..Self::bare(file.path.clone(), Some(file.absolute.clone()), kind)
        }
    }

    /// Size and modification time from the metadata.
    pub(crate) fn with_meta(mut self, meta: &fs::Metadata) -> Self {
        self.size = Some(meta.len());
        self.modified = meta.modified().ok();
        self
    }

    /// The status of the row; cancelled interim states have none.
    pub fn mark(&self) -> Option<RowMark> {
        match self.kind {
            RowKind::Recorded => Some(RowMark::Open),
            RowKind::Unchanged => Some(RowMark::Ok),
            RowKind::Modified if self.damaged() => Some(RowMark::Damaged),
            RowKind::Modified => Some(RowMark::Wrong),
            RowKind::Removed => Some(RowMark::Deleted),
            RowKind::Added => Some(RowMark::New),
            RowKind::Error => Some(RowMark::Error),
            RowKind::Checksum => Some(RowMark::Source),
            RowKind::Report => Some(RowMark::Report),
            RowKind::Partial => None,
        }
    }

    /// The structure check considers the file possibly damaged.
    pub fn damaged(&self) -> bool {
        self.integrity.as_ref().map(|item| item.status) == Some(IntegrityStatus::PossiblyDamaged)
    }
}

/// How many rows per status – the single count for summary, chips and message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarkCounts([usize; 9]);

impl MarkCounts {
    pub fn of(rows: &[FileRow]) -> Self {
        let mut counts = Self::default();
        for mark in rows.iter().filter_map(FileRow::mark) {
            counts.0[mark as usize] += 1;
        }
        counts
    }

    pub fn get(&self, mark: RowMark) -> usize {
        self.0[mark as usize]
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub ok: u64,
    pub modified: u64,
    pub added: u64,
    pub removed: u64,
    pub damaged: u64,
    pub errors: u64,
    pub skipped_symlinks: u64,
    pub skipped_non_regular: u64,
}

impl Summary {
    pub fn from_rows(rows: &[FileRow], skipped_symlinks: u64, skipped_non_regular: u64) -> Self {
        let counts = MarkCounts::of(rows);
        let count = |marks: &[RowMark]| marks.iter().map(|&mark| counts.get(mark) as u64).sum();
        Summary {
            ok: count(&[RowMark::Open, RowMark::Ok]),
            modified: count(&[RowMark::Wrong, RowMark::Damaged]),
            added: count(&[RowMark::New]),
            removed: count(&[RowMark::Deleted]),
            damaged: count(&[RowMark::Damaged]),
            errors: count(&[RowMark::Error]),
            skipped_symlinks,
            skipped_non_regular,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanOutcome {
    pub kind: JobKind,
    pub baseline: bool,
    pub cancelled: bool,
    pub report: ChangeReport,
    pub rows: Vec<FileRow>,
    /// The history as this run wrote it. `None` if nothing was written (cancel, read-only medium) –
    /// then what is on disk applies.
    pub history: Option<History>,
    pub summary: Summary,
    /// Create: the paths that are now in the checksum file. The UI does not read the file again for
    /// that. `None` for Verify (the file stays) and for a cancel.
    pub recorded: Option<Vec<String>>,
    /// What the run knows about the files it read, for the next Create. `None` on a cancel.
    pub known: Option<Known>,
    /// This many files the run read (hashed or tried to).
    pub read: u64,
    /// This many checksums it carried over from `ScanOptions::known` without reading.
    pub reused: u64,
}

struct Found {
    absolute: PathBuf,
    relative: String,
    size: u64,
    modified: Option<SystemTime>,
}

struct Walked {
    files: Vec<Found>,
    issues: Vec<FileIssue>,
    skipped_symlinks: u64,
    skipped_non_regular: u64,
    cancelled: bool,
}

struct FileIssue {
    path: String,
    message: String,
}

struct ParsedFile {
    map: BTreeMap<String, String>,
    warnings: Vec<String>,
}

enum HashItem {
    /// Read; plus size and modification time if they were stable while reading.
    Ok(CurrentFile, Option<Stamp>),
    Fail(FileIssue),
    Stop,
}

/// Scans `opts.root`. The call blocks; the GUI starts it in a thread.
///
/// Phases: load the checksum file, walk the folder, hash, evaluate (comparison, structure check,
/// report, rows), write. A cancel writes nothing.
pub fn run_scan(
    opts: &ScanOptions,
    progress: &Progress,
    cancel: &AtomicBool,
) -> Result<ScanOutcome, Error> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(cancelled_outcome(opts, &opts.root, Vec::new(), Vec::new()));
    }
    let root = normalize_root(&opts.root)?;
    let names = output_names_for(&root, opts);
    // One file after another on an HDD (`drive`).
    let seeks = opts.seeks.or_else(|| drive::seek_penalty(&root));
    let threads = drive::hash_threads(opts.threads.clamp(1, 256), seeks);

    // Known checksums only when creating, and only if they still fit. The state of the checksum
    // files is captured before reading: if one changes during the run, it no longer fits
    // afterwards.
    let known = opts
        .known
        .as_deref()
        .filter(|_| opts.kind == JobKind::Generate);
    let usable = known.filter(|known| known.usable_for(opts));
    let read_stamps = checksum_stamps(&root, opts);

    progress.set_phase(Msg::PhaseReadMd5.en());
    let mut loaded = load_phase(&root, opts, &names)?;
    if known.is_some() && usable.is_none() {
        loaded.warnings.push(Msg::KnownStale.en().into());
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(cancelled_outcome(opts, &root, loaded.warnings, Vec::new()));
    }

    progress.set_phase(Msg::PhaseWalk.en());
    let mut walked = walk_tree(&root, opts.recursive, &names, progress, cancel)?;
    drop_foreign_checksums(&mut walked.files, &loaded.previous);
    loaded.warnings.extend(walk_notes(
        walked.skipped_symlinks,
        walked.skipped_non_regular,
    ));
    if walked.cancelled {
        return Ok(cancelled_outcome(
            opts,
            &root,
            loaded.warnings,
            walked.issues,
        ));
    }

    // Create after a run: do not read unchanged files again.
    let found = std::mem::take(&mut walked.files);
    let split = match usable {
        Some(known) => {
            // The pill only counts once it is settled what will be read – not all files.
            progress.set_phase(Msg::PhaseMatch.en());
            progress.set_files_total(0);
            progress.set_bytes_total(0);
            split_known(known, found, threads, cancel)?
        }
        None => Split {
            reused: Vec::new(),
            to_read: found,
        },
    };
    if cancel.load(Ordering::Relaxed) {
        return Ok(cancelled_outcome(
            opts,
            &root,
            loaded.warnings,
            walked.issues,
        ));
    }
    if let Some(known) = usable {
        loaded.warnings.push(reuse_note(
            known.source,
            split.reused.len(),
            split.to_read.len(),
        ));
    }

    // Interim state for the list: carried-over checksums immediately, read files one by one.
    let judge = LiveJudge::new(opts.kind, &loaded.previous);
    for item in &split.reused {
        progress.push_live(judge.live(&item.file.path, &item.file.hash));
    }
    progress.set_phase(Msg::PhaseHash.en());
    progress.reset_clock();
    progress.set_reused(split.reused.len() as u64);
    progress.set_files_total(split.to_read.len() as u64);
    progress.set_bytes_total(split.to_read.iter().map(|file| file.size).sum());
    let mut hashed = hash_all(
        &split.to_read,
        opts.hash_mode,
        threads,
        progress,
        cancel,
        &judge,
    )?;
    if hashed.cancelled || cancel.load(Ordering::Relaxed) {
        let rows = partial_rows(&hashed.files);
        let mut report = base_report(opts, &root, true, 0, rows.len() as u64);
        report.warnings = loaded.warnings;
        report.errors = issues_to_entries(&hashed.issues, None);
        report.skipped_symlinks = walked.skipped_symlinks;
        report.skipped_non_regular = walked.skipped_non_regular;
        return Ok(outcome(opts, false, true, report, rows, None));
    }

    let read = split.to_read.len() as u64;
    let reused = split.reused.len() as u64;
    // What the next Create may carry over: the carried-over entries with their state from back then
    // and those just read that did not change while being read.
    let mut next_files = HashMap::new();
    for file in &hashed.files {
        if let Some(stamp) = hashed.settled.get(&file.path) {
            next_files.insert(
                path_key(&file.path, FOLD_CASE),
                KnownFile {
                    stamp: *stamp,
                    hash: file.hash.clone(),
                    integrity: None,
                },
            );
        }
    }
    for item in split.reused {
        let key = path_key(&item.file.path, FOLD_CASE);
        if let Some(finding) = &item.known.integrity {
            hashed.findings.insert(key.clone(), finding.clone());
        }
        next_files.insert(key, item.known);
        hashed.files.push(item.file);
    }
    hashed
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));

    let evaluated = evaluate(opts, &root, loaded, walked, hashed, progress, cancel);
    // The last point for a cancel: after it the run writes and finishes.
    if evaluated.aborted || !progress.begin_writing(cancel) {
        return Ok(outcome(
            opts,
            evaluated.baseline,
            true,
            evaluated.report,
            evaluated.rows,
            None,
        ));
    }

    progress.set_phase(Msg::PhaseWrite.en());
    let Evaluated {
        baseline,
        mut report,
        rows,
        files,
        kept,
        ..
    } = evaluated;
    let (history, notes) = write_outputs(&root, opts, &names, &files, &kept, &report)?;
    report.warnings.extend(notes);
    // The structure check of a mismatching file belongs to its state; the next Create carries it
    // over together with the checksum.
    for entry in &report.modified {
        if let Some(known) = next_files.get_mut(&path_key(&entry.path, FOLD_CASE)) {
            known.integrity = Some(IntegrityFinding {
                status: entry.integrity_status,
                reason: entry.integrity_reason.clone(),
            });
        }
    }
    let checksums = match opts.kind {
        // Create has just written them.
        JobKind::Generate => checksum_stamps(&root, opts),
        JobKind::Verify => read_stamps,
    };
    progress.set_phase(Msg::PhaseDone.en());
    let mut done = outcome(opts, baseline, false, report, rows, history);
    if opts.kind == JobKind::Generate && opts.hash_mode.includes_md5() {
        done.recorded = Some(hash_map(&files, &kept, HashMode::Md5).into_keys().collect());
    }
    done.read = read;
    done.reused = reused;
    done.known = Some(Known {
        source: opts.kind,
        at: jiff::Timestamp::now(),
        root: opts.root.clone(),
        recursive: opts.recursive,
        hash_mode: opts.hash_mode,
        checksums,
        files: next_files,
    });
    Ok(done)
}

/// Classifies a read file for the interim state of the list – by the same rule as the result at the
/// end: Verify by `report::verdict` like `classify`, Create takes in every file (the list then
/// shows each as open).
struct LiveJudge<'a> {
    /// Verify: the entries of the checksum file per `path_key`. `None` when creating.
    previous: Option<HashMap<String, &'a Digests>>,
}

impl<'a> LiveJudge<'a> {
    fn new(kind: JobKind, previous: &'a BTreeMap<String, Digests>) -> Self {
        let previous = (kind == JobKind::Verify).then(|| {
            previous
                .iter()
                .map(|(path, hash)| (path_key(path, FOLD_CASE), hash))
                .collect()
        });
        Self { previous }
    }

    /// A read file with its new checksum.
    fn live(&self, path: &str, hash: &Digests) -> LiveRow {
        let kind = match &self.previous {
            None => RowKind::Recorded,
            Some(previous) => {
                match verdict(previous.get(&path_key(path, FOLD_CASE)).copied(), hash) {
                    Verdict::Added => RowKind::Added,
                    Verdict::Unchanged => RowKind::Unchanged,
                    Verdict::Modified => RowKind::Modified,
                }
            }
        };
        LiveRow {
            path: path.to_string(),
            kind,
            new_hash: hash.clone(),
            message: None,
        }
    }

    /// A file that could not be read.
    fn failed(issue: &FileIssue) -> LiveRow {
        LiveRow {
            path: issue.path.clone(),
            kind: RowKind::Error,
            new_hash: Digests::default(),
            message: Some(issue.message.clone()),
        }
    }
}

/// A file whose known checksum Create carries over.
struct Reused {
    file: CurrentFile,
    known: KnownFile,
}

/// The found files, split into carried-over and to-be-read ones.
struct Split {
    reused: Vec<Reused>,
    to_read: Vec<Found>,
}

/// Splits the found files: if a file is known and its size and modification time are the same now
/// as when it was read back then, its checksum still applies; everything else is read.
fn split_known(
    known: &Known,
    found: Vec<Found>,
    threads: usize,
    cancel: &AtomicBool,
) -> Result<Split, Error> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|err| Error::ThreadPool(err.to_string()))?;
    let checked: Vec<(Found, Option<KnownFile>)> = pool.install(|| {
        found
            .into_par_iter()
            .map(|file| {
                if cancel.load(Ordering::Relaxed) {
                    return (file, None);
                }
                let hit = known
                    .files
                    .get(&path_key(&file.relative, FOLD_CASE))
                    .filter(|entry| Stamp::of(&file.absolute) == Some(entry.stamp))
                    .cloned();
                (file, hit)
            })
            .collect()
    });
    let mut split = Split {
        reused: Vec::new(),
        to_read: Vec::new(),
    };
    for (file, hit) in checked {
        match hit {
            Some(entry) => split.reused.push(Reused {
                file: CurrentFile {
                    path: file.relative,
                    absolute: file.absolute,
                    size: file.size,
                    modified: file.modified,
                    hash: entry.hash.clone(),
                },
                known: entry,
            }),
            None => split.to_read.push(file),
        }
    }
    Ok(split)
}

/// Note in the report: how many checksums were carried over, how many files read.
fn reuse_note(source: JobKind, reused: usize, read: usize) -> String {
    let template = match source {
        JobKind::Verify => Msg::ReuseFromVerify,
        JobKind::Generate => Msg::ReuseFromGenerate,
    };
    template.fill(&[
        &plural(reused, "checksum", "checksums"),
        &plural(read, "file", "files"),
    ])
}

/// What the checksum file yields before the run.
struct Loaded {
    /// The entries this run compares.
    previous: BTreeMap<String, Digests>,
    /// Entries this run does not touch (subfolders off): Create carries them over.
    nested: BTreeMap<String, Digests>,
    /// A checksum file already existed; otherwise the first one is created (baseline).
    existed: bool,
    warnings: Vec<String>,
}

/// Phase 1: read the checksum file. Without subfolders its entries there are left out: not reported
/// as deleted, carried over unchanged when creating.
fn load_phase(root: &Path, opts: &ScanOptions, names: &OutputNames) -> Result<Loaded, Error> {
    let loaded = load_previous(root, opts, names)?;
    let mut previous = loaded.map;
    let mut warnings = loaded.warnings;
    let nested = if opts.recursive {
        BTreeMap::new()
    } else {
        split_nested(&mut previous)
    };
    if !nested.is_empty() {
        let template = match opts.kind {
            JobKind::Generate => Msg::NestedKept,
            JobKind::Verify => Msg::NestedSkipped,
        };
        warnings.push(template.fill(&[&nested.len().to_string()]));
    }
    Ok(Loaded {
        previous,
        nested,
        existed: loaded.existed,
        warnings,
    })
}

/// Result of the evaluation, before anything is written.
struct Evaluated {
    baseline: bool,
    report: ChangeReport,
    rows: Vec<FileRow>,
    /// The hashed files, for the new checksum file.
    files: Vec<CurrentFile>,
    /// Entries that Create carries over unchanged (subfolders off, unreadable folders).
    kept: BTreeMap<String, Digests>,
    /// Cancelled during the structure check.
    aborted: bool,
}

/// Phase 4: compare, structurally check mismatching files, build report and rows.
fn evaluate(
    opts: &ScanOptions,
    root: &Path,
    loaded: Loaded,
    walked: Walked,
    hashed: Hashed,
    progress: &Progress,
    cancel: &AtomicBool,
) -> Evaluated {
    let Hashed {
        files,
        issues: hash_issues,
        findings: reused_findings,
        ..
    } = hashed;
    let Loaded {
        previous,
        mut nested,
        existed,
        mut warnings,
    } = loaded;
    // An unreadable folder: its entries count as read errors, not as deleted, and Create carries
    // them over unchanged.
    let failed: HashSet<&str> = hash_issues
        .iter()
        .map(|issue| issue.path.as_str())
        .collect();
    let below: Vec<FileIssue> = entries_below_issues(&previous, &walked.issues, &files, FOLD_CASE)
        .into_iter()
        .filter(|issue| !failed.contains(issue.path.as_str()))
        .collect();
    if !below.is_empty() {
        let template = if opts.kind == JobKind::Generate {
            Msg::BelowUnreadableKept
        } else {
            Msg::BelowUnreadable
        };
        warnings.push(template.fill(&[&below.len().to_string()]));
    }
    for issue in &below {
        if let Some(hash) = previous.get(&issue.path) {
            nested.insert(issue.path.clone(), hash.clone());
        }
    }
    let mut issues = walked.issues;
    issues.extend(below);
    issues.extend(hash_issues);
    let baseline = opts.kind == JobKind::Generate && !existed;
    let error_paths: HashSet<String> = issues.iter().map(|issue| issue.path.clone()).collect();
    let classified = if baseline {
        Classified::default()
    } else {
        classify(&previous, files.clone(), &error_paths)
    };

    let mut aborted = false;
    let mut findings = Vec::with_capacity(classified.modified.len());
    if !baseline && !classified.modified.is_empty() {
        progress.set_phase(Msg::PhaseIntegrity.en());
        for (index, pair) in classified.modified.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                aborted = true;
                for rest in &classified.modified[index..] {
                    findings.push((
                        rest,
                        IntegrityFinding {
                            status: IntegrityStatus::NotChecked,
                            reason: Msg::RunCancelledStructure.en().into(),
                        },
                    ));
                }
                break;
            }
            // A carried-over file is unchanged; its structure check still applies.
            let finding = reused_findings
                .get(&path_key(&pair.file.path, FOLD_CASE))
                .cloned()
                .unwrap_or_else(|| check_path(&pair.file.absolute));
            findings.push((pair, finding));
        }
    }
    if aborted {
        warnings.push(Msg::AbortedIntegrity.en().into());
    }

    let report = assemble_report(&ReportInput {
        opts,
        root,
        baseline,
        current: &files,
        classified: &classified,
        findings: &findings,
        issues: &issues,
        previous: &previous,
        warnings: &warnings,
        skipped_symlinks: walked.skipped_symlinks,
        skipped_non_regular: walked.skipped_non_regular,
        cancelled: aborted,
    });
    let rows = build_rows(root, baseline, &files, &classified, &findings, &issues);
    Evaluated {
        baseline,
        report,
        rows,
        files,
        kept: nested,
        aborted,
    }
}

/// Takes the entries in subfolders (paths with `/`) out of `previous`.
fn split_nested(previous: &mut BTreeMap<String, Digests>) -> BTreeMap<String, Digests> {
    let nested: Vec<String> = previous
        .keys()
        .filter(|path| path.contains('/'))
        .cloned()
        .collect();
    let mut out = BTreeMap::new();
    for path in nested {
        if let Some(hash) = previous.remove(&path) {
            out.insert(path, hash);
        }
    }
    out
}

fn normalize_root(root: &Path) -> Result<PathBuf, Error> {
    if !root.exists() || !root.is_dir() {
        return Err(Error::NotADirectory(root.to_path_buf()));
    }
    Ok(root.to_path_buf())
}

struct LoadedPrevious {
    map: BTreeMap<String, Digests>,
    existed: bool,
    warnings: Vec<String>,
}

fn load_previous(
    root: &Path,
    opts: &ScanOptions,
    names: &OutputNames,
) -> Result<LoadedPrevious, Error> {
    let targets = checksum_targets(root, opts.hash_mode, names);
    let mut map = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut existed = false;
    let mut saw_file = false;
    for (algo, path) in &targets {
        match read_previous(path, *algo) {
            Ok(None) => {
                if opts.hash_mode == HashMode::Both {
                    warnings.push(Msg::AlgoMissing.fill(&[&path.display().to_string()]));
                }
            }
            Ok(Some(parsed)) => {
                saw_file = true;
                existed = true;
                warnings.extend(parsed.warnings);
                merge_hashes(&mut map, parsed.map, *algo);
            }
            // Only a file without a valid line is replaced. If it is unreadable or too large,
            // Create aborts instead of faking a baseline.
            Err(err @ Error::ChecksumInvalid { .. }) if opts.kind == JobKind::Generate => {
                warnings.push(Msg::ReplacingInvalid.fill(&[&err.to_string()]));
            }
            Err(err) => return Err(err),
        }
    }
    if opts.kind == JobKind::Verify && !saw_file {
        let missing = targets
            .first()
            .map(|(_, path)| path.clone())
            .unwrap_or_else(|| root.join(&names.md5));
        return Err(Error::ChecksumMissing { path: missing });
    }
    if opts.kind == JobKind::Generate && !saw_file {
        existed = false;
        let missing = Msg::AlgoMissing.fill(&[""]);
        warnings.retain(|warning| !warning.ends_with(&missing));
    }
    Ok(LoadedPrevious {
        map,
        existed,
        warnings,
    })
}

/// Per algorithm of the mode, its checksum file: `Both` reads and writes `.md5` and `.sha256`,
/// otherwise the one file (for an opened SHA file that one, `names_for`).
fn checksum_targets(root: &Path, mode: HashMode, names: &OutputNames) -> Vec<(HashMode, PathBuf)> {
    mode.parts()
        .iter()
        .map(|&algo| (algo, root.join(names.file(algo))))
        .collect()
}

fn read_previous(path: &Path, algo: HashMode) -> Result<Option<ParsedFile>, Error> {
    if !path.is_file() {
        return Ok(None);
    }
    let file = match read_checksum_file(path) {
        Ok(read) => read,
        Err(source) if source.kind() == io::ErrorKind::FileTooLarge => {
            return Err(Error::Message(source.to_string()));
        }
        Err(source) => {
            return Err(Error::Io {
                path: Some(path.to_path_buf()),
                source,
            });
        }
    };
    let mut kept = BTreeMap::new();
    let mut warnings = Vec::new();
    if let Some(encoding) = file.encoding {
        warnings.push(Msg::NotUtf8Read.fill(&[&path.display().to_string(), encoding]));
    }
    for warning in file.line_warnings {
        warnings.push(format!("{}: {warning}", path.display()));
    }
    for (entry_path, hash) in file.entries {
        if Some(hash.len()) == algo.hex_len() {
            kept.insert(entry_path, hash);
        } else {
            warnings.push(Msg::HashLenMismatch.fill(&[
                &path.display().to_string(),
                algo.label(),
                &entry_path,
            ]));
        }
    }
    if kept.is_empty() && file.nonempty && !warnings.is_empty() {
        let detail = warnings.into_iter().take(3).collect::<Vec<_>>().join(" ");
        return Err(Error::ChecksumInvalid {
            path: path.to_path_buf(),
            detail,
        });
    }
    Ok(Some(ParsedFile {
        map: kept,
        warnings,
    }))
}

fn merge_hashes(
    into: &mut BTreeMap<String, Digests>,
    map: BTreeMap<String, String>,
    algo: HashMode,
) {
    for (path, hash) in map {
        into.entry(path).or_default().set(algo, hash);
    }
}

/// Foreign SHA checksum files in the chosen folder (`SHA256SUMS`, `x.sha1` …) are not part of the
/// data: they are not hashed and never appear as "new". If one is already in the checksum file that
/// applies, it stays – otherwise it would count as deleted when verifying.
fn drop_foreign_checksums(files: &mut Vec<Found>, previous: &BTreeMap<String, Digests>) {
    let listed: HashSet<String> = previous
        .keys()
        .map(|path| path_key(path, FOLD_CASE))
        .collect();
    files.retain(|file| {
        file.relative.contains('/')
            || !is_sha_filename(&file.relative)
            || listed.contains(&path_key(&file.relative, FOLD_CASE))
    });
}

fn walk_tree(
    root: &Path,
    recursive: bool,
    names: &OutputNames,
    progress: &Progress,
    cancel: &AtomicBool,
) -> Result<Walked, Error> {
    let excluded = excluded_filenames(root, names);
    let depth = if recursive { usize::MAX } else { 1 };
    let walker = WalkDir::new(root)
        .follow_links(false)
        .follow_root_links(true)
        .max_depth(depth);
    let mut files = Vec::new();
    let mut bytes = 0u64;
    let mut issues = Vec::new();
    let mut skipped_symlinks = 0u64;
    let mut skipped_non_regular = 0u64;

    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Walked {
                files,
                issues,
                skipped_symlinks,
                skipped_non_regular,
                cancelled: true,
            });
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                let path = err
                    .path()
                    .map(|path| display_rel(root, path))
                    .unwrap_or_else(|| UNKNOWN_PLACE.into());
                let message = if err.loop_ancestor().is_some() {
                    Msg::SymlinkLoop.fill(&[&path])
                } else if err
                    .io_error()
                    .is_some_and(|io_err| io_err.kind() == io::ErrorKind::PermissionDenied)
                {
                    Msg::DirDenied.fill(&[&path])
                } else {
                    Msg::DirReadError.fill(&[&path])
                };
                issues.push(FileIssue { path, message });
                continue;
            }
        };
        let path = entry.path();
        if path == root {
            continue;
        }
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            skipped_symlinks += 1;
            continue;
        }
        if file_type.is_dir() {
            continue;
        }
        if !file_type.is_file() {
            skipped_non_regular += 1;
            continue;
        }
        if is_excluded(root, path, &excluded) {
            continue;
        }
        let Some(relative) = relative_unix(root, path) else {
            issues.push(FileIssue {
                path: path.to_string_lossy().into_owned(),
                message: Msg::BadName.en().into(),
            });
            continue;
        };
        let meta = entry.metadata().ok();
        let size = meta.as_ref().map(|meta| meta.len()).unwrap_or(0);
        let modified = meta.and_then(|meta| meta.modified().ok());
        bytes += size;
        files.push(Found {
            absolute: path.to_path_buf(),
            relative,
            size,
            modified,
        });
        if files.len() % 64 == 0 {
            progress.set_files_total(files.len() as u64);
            progress.set_bytes_total(bytes);
        }
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    progress.set_files_total(files.len() as u64);
    progress.set_bytes_total(bytes);
    Ok(Walked {
        files,
        issues,
        skipped_symlinks,
        skipped_non_regular,
        cancelled: false,
    })
}

fn is_excluded(root: &Path, path: &Path, excluded: &HashSet<String>) -> bool {
    if path.parent() != Some(root) {
        return false;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| is_excluded_name(excluded, name))
}

/// Path of an error in the walk that names no place – for example when reading a folder breaks off
/// midway (disk, disconnected network drive).
const UNKNOWN_PLACE: &str = Msg::UnknownPlace.en();

/// For every folder the walk could not read (`walk`), the entries from `previous` below it – as
/// read errors. `""` stands for the chosen folder itself; an error without a place
/// (`UNKNOWN_PLACE`) can affect every entry that was not hashed. Hashed files (`hashed`) are never
/// affected.
fn entries_below_issues(
    previous: &BTreeMap<String, Digests>,
    walk: &[FileIssue],
    hashed: &[CurrentFile],
    fold_case: bool,
) -> Vec<FileIssue> {
    let dirs: Vec<(String, &FileIssue)> = walk
        .iter()
        .map(|issue| {
            let place = if issue.path == UNKNOWN_PLACE {
                String::new()
            } else {
                path_key(&issue.path, fold_case)
            };
            (place, issue)
        })
        .collect();
    let done: HashSet<String> = hashed
        .iter()
        .map(|file| path_key(&file.path, fold_case))
        .collect();
    let mut out = Vec::new();
    for path in previous.keys() {
        let key = path_key(path, fold_case);
        if done.contains(&key) {
            continue;
        }
        let hit = dirs.iter().find(|(dir, _)| {
            dir.is_empty()
                || key
                    .strip_prefix(dir.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        });
        if let Some((_, issue)) = hit {
            out.push(FileIssue {
                path: path.clone(),
                message: Msg::NotChecked.fill(&[&issue.message]),
            });
        }
    }
    out
}

fn relative_unix(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_str()?;
                if text.is_empty() || text == "." || text == ".." {
                    return None;
                }
                parts.push(text.to_string());
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn display_rel(root: &Path, path: &Path) -> String {
    relative_unix(root, path).unwrap_or_else(|| {
        path.strip_prefix(root)
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().into_owned())
    })
}

fn walk_notes(symlinks: u64, special: u64) -> Vec<String> {
    let mut notes = Vec::new();
    if symlinks > 0 {
        notes.push(Msg::SymlinksSkipped.fill(&[&symlinks.to_string()]));
    }
    if special > 0 {
        notes.push(Msg::SpecialSkipped.fill(&[&special.to_string()]));
    }
    notes
}

struct Hashed {
    files: Vec<CurrentFile>,
    issues: Vec<FileIssue>,
    /// Per relative path, size and modification time of the files that did not change while being
    /// read (see `hash_all`) – only their checksum may be carried over by a later Create.
    settled: HashMap<String, Stamp>,
    /// Per `path_key`: the structure check of files carried over from the earlier run.
    findings: HashMap<String, IntegrityFinding>,
    cancelled: bool,
}

fn hash_all(
    files: &[Found],
    mode: HashMode,
    threads: usize,
    progress: &Progress,
    cancel: &AtomicBool,
    judge: &LiveJudge<'_>,
) -> Result<Hashed, Error> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|err| Error::ThreadPool(err.to_string()))?;
    let items: Vec<HashItem> = pool.install(|| {
        files
            .par_iter()
            .map(|file| {
                if cancel.load(Ordering::Relaxed) {
                    return HashItem::Stop;
                }
                progress.set_current(&file.relative);
                // Size and modification time before opening and after reading: only if both are
                // equal and the last change was long enough before the read does the checksum
                // surely belong to this state (`Stamp::settled_before`).
                let started = SystemTime::now();
                let before = Stamp::of(&file.absolute);
                match hash_file(&file.absolute, mode, Some(progress), Some(cancel)) {
                    Ok(hash) => {
                        progress.push_live(judge.live(&file.relative, &hash));
                        progress.inc_files();
                        let after = Stamp::of(&file.absolute);
                        let settled = before
                            .filter(|stamp| after == Some(*stamp) && stamp.settled_before(started));
                        HashItem::Ok(
                            CurrentFile {
                                path: file.relative.clone(),
                                absolute: file.absolute.clone(),
                                size: file.size,
                                modified: file.modified,
                                hash,
                            },
                            settled,
                        )
                    }
                    Err(Error::Cancelled) => HashItem::Stop,
                    Err(err) => {
                        let issue = FileIssue {
                            path: file.relative.clone(),
                            message: file_problem(&err),
                        };
                        progress.push_live(LiveJudge::failed(&issue));
                        progress.inc_files();
                        HashItem::Fail(issue)
                    }
                }
            })
            .collect()
    });
    let mut hashed = Hashed {
        files: Vec::new(),
        issues: Vec::new(),
        settled: HashMap::new(),
        findings: HashMap::new(),
        cancelled: cancel.load(Ordering::Relaxed),
    };
    for item in items {
        match item {
            HashItem::Ok(file, settled) => {
                if let Some(stamp) = settled {
                    hashed.settled.insert(file.path.clone(), stamp);
                }
                hashed.files.push(file);
            }
            HashItem::Fail(issue) => hashed.issues.push(issue),
            HashItem::Stop => hashed.cancelled = true,
        }
    }
    hashed
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    hashed
        .issues
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(hashed)
}

struct ReportInput<'a> {
    opts: &'a ScanOptions,
    root: &'a Path,
    baseline: bool,
    current: &'a [CurrentFile],
    classified: &'a Classified,
    findings: &'a [(&'a ModifiedPair, IntegrityFinding)],
    issues: &'a [FileIssue],
    previous: &'a BTreeMap<String, Digests>,
    warnings: &'a [String],
    skipped_symlinks: u64,
    skipped_non_regular: u64,
    cancelled: bool,
}

fn assemble_report(input: &ReportInput<'_>) -> ChangeReport {
    let mut report = base_report(
        input.opts,
        input.root,
        input.baseline,
        if input.baseline {
            0
        } else {
            input.classified.unchanged.len() as u64
        },
        input.current.len() as u64,
    );
    report.cancelled = input.cancelled;
    report.warnings = input.warnings.to_vec();
    report.skipped_symlinks = input.skipped_symlinks;
    report.skipped_non_regular = input.skipped_non_regular;
    report.errors = issues_to_entries(input.issues, Some(input.previous));
    if !input.baseline {
        report.modified = input
            .findings
            .iter()
            .map(|(pair, finding)| ModifiedEntry {
                path: pair.file.path.clone(),
                old_hash: pair.old_hash.clone(),
                new_hash: pair.file.hash.clone(),
                integrity_status: finding.status,
                integrity_reason: finding.reason.clone(),
                size: Some(pair.file.size),
            })
            .collect();
        report.added = input
            .classified
            .added
            .iter()
            .map(|file| PathEntry {
                path: file.path.clone(),
                hash: file.hash.clone(),
                size: Some(file.size),
            })
            .collect();
        report.removed = input
            .classified
            .removed
            .iter()
            .map(|file| PathEntry {
                path: file.path.clone(),
                hash: file.hash.clone(),
                size: None,
            })
            .collect();
    }
    report
}

fn base_report(
    opts: &ScanOptions,
    root: &Path,
    baseline: bool,
    unchanged_count: u64,
    file_count: u64,
) -> ChangeReport {
    ChangeReport {
        timestamp: utc_now(),
        base_directory: display_base(root),
        hash_mode: opts.hash_mode.id().to_string(),
        action: if opts.kind == JobKind::Generate {
            Action::Created
        } else {
            Action::Verified
        },
        baseline,
        unchanged_count,
        file_count,
        modified: Vec::new(),
        added: Vec::new(),
        removed: Vec::new(),
        errors: Vec::new(),
        skipped_symlinks: 0,
        skipped_non_regular: 0,
        cancelled: false,
        warnings: Vec::new(),
    }
}

fn display_base(root: &Path) -> String {
    if root.is_absolute() {
        root.display().to_string()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(root))
            .unwrap_or_else(|_| root.to_path_buf())
            .display()
            .to_string()
    }
}

/// Errors with the hash the checksum file had for this file so far. That way the history also
/// records the old state of a file that was unreadable when replacing.
fn issues_to_entries(
    issues: &[FileIssue],
    previous: Option<&BTreeMap<String, Digests>>,
) -> Vec<ErrorEntry> {
    let by_key: HashMap<String, &Digests> = previous
        .into_iter()
        .flatten()
        .map(|(path, hash)| (path_key(path, FOLD_CASE), hash))
        .collect();
    let mut errors: Vec<ErrorEntry> = issues
        .iter()
        .map(|issue| ErrorEntry {
            path: issue.path.clone(),
            message: issue.message.clone(),
            old_hash: by_key
                .get(&path_key(&issue.path, FOLD_CASE))
                .map(|hash| (*hash).clone()),
        })
        .collect();
    errors.sort_by(|left, right| left.path.cmp(&right.path));
    errors
}

fn build_rows(
    root: &Path,
    baseline: bool,
    current: &[CurrentFile],
    classified: &Classified,
    findings: &[(&ModifiedPair, IntegrityFinding)],
    issues: &[FileIssue],
) -> Vec<FileRow> {
    let mut rows = Vec::new();
    if baseline {
        for file in current {
            rows.push(FileRow::hashed(file, RowKind::Recorded, Digests::default()));
        }
    } else {
        for file in &classified.unchanged {
            rows.push(FileRow::hashed(file, RowKind::Unchanged, file.hash.clone()));
        }
        for (pair, finding) in findings {
            rows.push(FileRow {
                integrity: Some(finding.clone()),
                ..FileRow::hashed(&pair.file, RowKind::Modified, pair.old_hash.clone())
            });
        }
        for file in &classified.added {
            rows.push(FileRow::hashed(file, RowKind::Added, Digests::default()));
        }
        for file in &classified.removed {
            let absolute = Some(root.join(rel_to_path(&file.path)));
            rows.push(FileRow {
                old_hash: file.hash.clone(),
                ..FileRow::bare(file.path.clone(), absolute, RowKind::Removed)
            });
        }
    }
    let known: HashSet<String> = rows.iter().map(|row| row.path.clone()).collect();
    for issue in issues {
        if known.contains(&issue.path) {
            continue;
        }
        let absolute = Some(root.join(rel_to_path(&issue.path)));
        rows.push(FileRow {
            message: Some(issue.message.clone()),
            ..FileRow::bare(issue.path.clone(), absolute, RowKind::Error)
        });
    }
    rows.sort_by(|left, right| left.path.cmp(&right.path));
    rows
}

fn partial_rows(files: &[CurrentFile]) -> Vec<FileRow> {
    files
        .iter()
        .map(|file| FileRow {
            message: Some(Msg::RunCancelledRow.en().into()),
            ..FileRow::hashed(file, RowKind::Partial, Digests::default())
        })
        .collect()
}

pub(crate) fn rel_to_path(relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

/// Metadata of `absolute` if it is a regular file under `root` and no section below `root` is a
/// link (or on Windows a junction) – Veritas does not follow links, as in the run. `root` itself
/// may be a link. `checked` remembers checked folders, so that the preview does not read every
/// folder again for each entry.
pub(crate) fn plain_file_meta(
    root: &Path,
    absolute: &Path,
    checked: &mut HashSet<PathBuf>,
) -> Result<fs::Metadata, String> {
    let relative = absolute
        .strip_prefix(root)
        .map_err(|_| Msg::NotInFolder.en().to_string())?;
    let parts: Vec<Component<'_>> = relative.components().collect();
    let mut current = root.to_path_buf();
    for (index, part) in parts.iter().enumerate() {
        let Component::Normal(name) = part else {
            return Err(Msg::NotInFolder.en().into());
        };
        current.push(name);
        let last = index + 1 == parts.len();
        if !last && checked.contains(&current) {
            continue;
        }
        let meta = fs::symlink_metadata(&current).map_err(|err| err.to_string())?;
        if meta.file_type().is_symlink() {
            return Err(Msg::IsLink.en().into());
        }
        if last {
            return if meta.is_file() {
                Ok(meta)
            } else {
                Err(Msg::NotRegularFile.en().into())
            };
        }
        if !meta.is_dir() {
            return Err(Msg::NotInFolder.en().into());
        }
        checked.insert(current.clone());
    }
    Err(Msg::NotAFileInFolder.en().into())
}

/// The check before opening and deleting: `absolute` is a regular file under `root`
/// (`plain_file_meta`), and its canonical path lies under the canonical `root`. Returns the
/// canonical path – for checks like `no_trash`, not for acting on (on Windows `\\?\…`, which some
/// shell calls do not understand).
pub(crate) fn contained_file(root: &Path, absolute: &Path) -> Result<PathBuf, String> {
    plain_file_meta(root, absolute, &mut HashSet::new())?;
    let canon_root = root.canonicalize().map_err(|err| err.to_string())?;
    let canon = absolute.canonicalize().map_err(|err| err.to_string())?;
    if canon.starts_with(&canon_root) {
        Ok(canon)
    } else {
        Err(Msg::NotInFolder.en().into())
    }
}

fn write_outputs(
    root: &Path,
    opts: &ScanOptions,
    names: &OutputNames,
    files: &[CurrentFile],
    kept: &BTreeMap<String, Digests>,
    report: &ChangeReport,
) -> Result<(Option<History>, Vec<String>), Error> {
    let json_path = root.join(&names.json);
    let mut run = report.clone();
    // Verify delivers its result even if the history cannot be written (read-only medium); then it
    // appears as a warning. Create aborts before anything is replaced.
    let refuse = |reason: String| -> Result<(Option<History>, Vec<String>), Error> {
        match opts.kind {
            JobKind::Generate => Err(Error::Message(Msg::NothingWritten.fill(&[&reason]))),
            JobKind::Verify => Ok((None, vec![Msg::NotInHistory.fill(&[&reason])])),
        }
    };
    let mut history = match history_for_write(&json_path, &names.json) {
        Ok((history, note)) => {
            run.warnings.extend(note);
            history
        }
        Err(reason) => return refuse(reason),
    };
    history.push(run);
    let json = history.to_pretty_json()?;

    // First all staging files, then the history, last the checksum files: if anything fails, the
    // old `.md5` stays – and the history names no run that did not happen.
    let mut outputs = Vec::new();
    if opts.kind == JobKind::Generate {
        for (algo, path) in checksum_targets(root, opts.hash_mode, names) {
            let text = format_checksum(&hash_map(files, kept, algo));
            outputs.push((path, text.into_bytes()));
        }
    }
    outputs.insert(0, (json_path, json));
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (path, data) in &outputs {
        match stage(path, data) {
            Ok(tmp) => staged.push((tmp, path.clone())),
            Err(err) => {
                discard(&staged);
                return refuse(err.to_string());
            }
        }
    }
    for (index, (tmp, path)) in staged.iter().enumerate() {
        if let Err(source) = fs::rename(tmp, path) {
            discard(&staged[index..]);
            let err = Error::Write {
                path: path.clone(),
                source,
            };
            if index == 0 {
                return refuse(err.to_string());
            }
            return Err(Error::Message(Msg::RenameFailed.fill(&[&err.to_string()])));
        }
    }
    // The history is on disk like this now; the UI does not have to read it again.
    Ok((Some(history), Vec::new()))
}

/// The existing history the new run is appended to. If it is missing, a new one begins; if it is a
/// foreign file, it is replaced (second field: a note about that). If it is unreadable or a history
/// in another format (downgrade), the reason is returned – then it is not touched, rather than
/// replaced by an empty one.
fn history_for_write(path: &Path, name: &str) -> Result<(History, Option<String>), String> {
    let bytes = match read_limited(path, MAX_HISTORY_BYTES) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok((History::new(Vec::new()), None))
        }
        Err(err) if err.kind() == io::ErrorKind::FileTooLarge => return Err(err.to_string()),
        Err(err) => return Err(Msg::HistoryUnreadable.fill(&[name, &err.to_string()])),
    };
    let text = String::from_utf8_lossy(&bytes);
    let (history, known) = History::from_text(&text);
    if known {
        return Ok((history, None));
    }
    if let Some(format) = History::other_format(&text) {
        return Err(Msg::HistoryOtherFormat.fill(&[name, &format]));
    }
    Ok((history, Some(Msg::HistoryReplaced.fill(&[name]))))
}

/// Lines of the new checksum file: the hashed files and the carried-over entries.
fn hash_map(
    files: &[CurrentFile],
    kept: &BTreeMap<String, Digests>,
    algo: HashMode,
) -> BTreeMap<String, String> {
    let pick = |digests: &Digests| digests.get(algo).cloned();
    let mut map = BTreeMap::new();
    for file in files {
        if let Some(hash) = pick(&file.hash) {
            map.insert(file.path.clone(), hash);
        }
    }
    for (path, digests) in kept {
        if let Some(hash) = pick(digests) {
            map.entry(path.clone()).or_insert(hash);
        }
    }
    map
}

/// Writes `data` completely to `{path}.partial` and returns that path; renaming happens later
/// (`write_outputs`). A leftover `.partial` is removed first – a link there disappears itself, its
/// target stays. `create_new` creates the file anew and follows no link (O_EXCL), not even one that
/// shows up in between.
fn stage(path: &Path, data: &[u8]) -> Result<PathBuf, Error> {
    let tmp = partial_path(path);
    let _ = fs::remove_file(&tmp);
    let fail = |source| Error::Write {
        path: path.to_path_buf(),
        source,
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(fail)?;
    let written = file.write_all(data).and_then(|()| file.sync_all());
    drop(file);
    if let Err(source) = written {
        let _ = fs::remove_file(&tmp);
        return Err(fail(source));
    }
    Ok(tmp)
}

/// Removes staging files that will not be renamed any more.
fn discard(staged: &[(PathBuf, PathBuf)]) {
    for (tmp, _) in staged {
        let _ = fs::remove_file(tmp);
    }
}

fn partial_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("veritas"))
        .to_os_string();
    name.push(".partial");
    path.with_file_name(name)
}

/// Now in UTC as RFC 3339 with `+00:00`, like the histories up to 0.7 (`chrono::to_rfc3339`).
fn utc_now() -> String {
    jiff::Timestamp::now()
        .to_zoned_datetime(jiff::tz::TimeZone::UTC)
        .strftime("%Y-%m-%dT%H:%M:%S%.f%:z")
        .to_string()
}

fn cancelled_outcome(
    opts: &ScanOptions,
    root: &Path,
    warnings: Vec<String>,
    issues: Vec<FileIssue>,
) -> ScanOutcome {
    let mut report = base_report(opts, root, false, 0, 0);
    report.cancelled = true;
    report.warnings = warnings;
    report.errors = issues_to_entries(&issues, None);
    let rows = issues
        .into_iter()
        .map(|issue| FileRow {
            message: Some(issue.message),
            ..FileRow::bare(issue.path, None, RowKind::Error)
        })
        .collect();
    outcome(opts, false, true, report, rows, None)
}

/// The result of a run; the summary counts the rows, the skipped entries are in the report.
fn outcome(
    opts: &ScanOptions,
    baseline: bool,
    cancelled: bool,
    report: ChangeReport,
    rows: Vec<FileRow>,
    history: Option<History>,
) -> ScanOutcome {
    let summary = Summary::from_rows(&rows, report.skipped_symlinks, report.skipped_non_regular);
    ScanOutcome {
        kind: opts.kind,
        baseline,
        cancelled,
        report,
        rows,
        history,
        summary,
        recorded: None,
        known: None,
        read: 0,
        reused: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::NameStyle;
    use crate::hashing::hash_slice;
    use std::sync::atomic::AtomicU64;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("veritas-scan-{}-{n}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn options(root: &Path, kind: JobKind) -> ScanOptions {
        ScanOptions {
            root: root.to_path_buf(),
            recursive: true,
            threads: 2,
            seeks: None,
            hash_mode: HashMode::Md5,
            name_style: NameStyle::Checksums,
            md5_file: None,
            kind,
            known: None,
        }
    }

    /// The history in `checksums.json`.
    fn history(dir: &Path) -> serde_json::Value {
        serde_json::from_slice(&fs::read(dir.join("checksums.json")).unwrap()).unwrap()
    }

    /// The latest run in the history.
    fn last_run(dir: &Path) -> serde_json::Value {
        history(dir)["runs"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone()
    }

    fn go(opts: ScanOptions) -> Result<ScanOutcome, Error> {
        let progress = Progress::new();
        let cancel = AtomicBool::new(false);
        run_scan(&opts, &progress, &cancel)
    }

    #[test]
    fn generate_writes_relative_md5_and_empty_change_lists() {
        let dir = TempDir::new();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("meine datei.txt"), b"abc").unwrap();
        fs::write(dir.path().join("leer.txt"), b"").unwrap();
        fs::write(dir.path().join("fotos.txt"), "übersicht".as_bytes()).unwrap();
        let outcome = go(options(dir.path(), JobKind::Generate)).unwrap();
        assert!(outcome.baseline);
        assert!(!outcome.cancelled);
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(text.contains("900150983CD24FB0D6963F7D28E17F72  sub/meine datei.txt\n"));
        assert!(text.contains("D41D8CD98F00B204E9800998ECF8427E  leer.txt\n"));
        assert!(text.contains("fotos.txt\n"));
        assert!(!text.contains('\r'));
        assert!(!text.contains('\\'));
        let line = text
            .lines()
            .find(|line| line.contains("sub/meine datei.txt"))
            .unwrap();
        assert_eq!(&line.as_bytes()[32..34], b"  ");
        let json = last_run(dir.path());
        assert_eq!(json["action"], "created");
        assert_eq!(json["baseline"], true);
        assert_eq!(json["file_count"], 3);
        assert_eq!(json["unchanged_count"], 0);
        assert!(json["modified"].as_array().unwrap().is_empty());
        assert!(json["added"].as_array().unwrap().is_empty());
        assert!(json["removed"].as_array().unwrap().is_empty());
        assert!(json["timestamp"]
            .as_str()
            .unwrap()
            .parse::<jiff::Timestamp>()
            .is_ok());
        assert!(json["timestamp"].as_str().unwrap().ends_with("+00:00"));
        assert_eq!(outcome.summary.ok, 3);
    }

    /// A new run writes English values whatever language the UI has: in this test it is French, yet
    /// the history has no other text.
    #[test]
    fn a_run_writes_english_whatever_the_language() {
        crate::i18n::set(crate::i18n::Lang::Fr);
        let dir = TempDir::new();
        let jpeg_ok = [0xff, 0xd8, 0x11, 0x22, 0x33, 0xff, 0xd9];
        fs::write(dir.path().join("bild.jpg"), jpeg_ok).unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        fs::write(
            dir.path().join("bild.jpg"),
            [0xff, 0xd8, 0x11, 0x22, 0x99, 0x00, 0x10],
        )
        .unwrap();
        go(options(dir.path(), JobKind::Verify)).unwrap();
        crate::i18n::set(crate::i18n::Lang::De);
        let all = history(dir.path());
        assert_eq!(all["format"], crate::report::HISTORY_FORMAT);
        let runs = all["runs"].as_array().unwrap();
        assert_eq!(runs[0]["action"], "created");
        assert_eq!(runs[1]["action"], "verified");
        let bad = &runs[1]["modified"][0];
        assert_eq!(bad["integrity_status"], "possibly damaged");
        assert_eq!(bad["integrity_reason"], Msg::JpegNoEoi.en());
        let text = fs::read_to_string(dir.path().join("checksums.json")).unwrap();
        for german in ["erzeugt", "geprüft", "beschädigt", "fehlt"] {
            assert!(!text.contains(german), "{german} in {text}");
        }
    }

    #[test]
    fn verify_detects_added_modified_removed_and_damage() {
        let dir = TempDir::new();
        let jpeg_ok = [0xff, 0xd8, 0x11, 0x22, 0x33, 0xff, 0xd9];
        let jpeg_bad = [0xff, 0xd8, 0x11, 0x22, 0x99, 0x00, 0x10];
        fs::write(dir.path().join("ok.jpg"), jpeg_ok).unwrap();
        fs::write(dir.path().join("bad.jpg"), jpeg_ok).unwrap();
        fs::write(dir.path().join("gone.txt"), b"weg").unwrap();
        fs::write(dir.path().join("stay.txt"), b"bleibt").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        let md5_before = fs::read(dir.path().join("checksums.md5")).unwrap();

        let mut changed = jpeg_ok;
        changed[3] = 0x44;
        fs::write(dir.path().join("ok.jpg"), changed).unwrap();
        fs::write(dir.path().join("bad.jpg"), jpeg_bad).unwrap();
        fs::remove_file(dir.path().join("gone.txt")).unwrap();
        fs::write(dir.path().join("neu.txt"), b"neu").unwrap();

        let outcome = go(options(dir.path(), JobKind::Verify)).unwrap();
        assert!(!outcome.baseline);
        assert_eq!(
            fs::read(dir.path().join("checksums.md5")).unwrap(),
            md5_before
        );
        assert_eq!(outcome.summary.modified, 2);
        assert_eq!(outcome.summary.added, 1);
        assert_eq!(outcome.summary.removed, 1);
        assert_eq!(outcome.summary.damaged, 1);
        assert_eq!(outcome.summary.ok, 1);
        let json = last_run(dir.path());
        assert_eq!(json["action"], "verified");
        assert_eq!(json["unchanged_count"], 1);
        let modified = json["modified"].as_array().unwrap();
        assert_eq!(modified.len(), 2);
        let bad = modified
            .iter()
            .find(|entry| entry["path"] == "bad.jpg")
            .unwrap();
        assert_eq!(bad["integrity_status"], "possibly damaged");
        assert!(bad["integrity_reason"].as_str().unwrap().contains("EOI"));
        let ok = modified
            .iter()
            .find(|entry| entry["path"] == "ok.jpg")
            .unwrap();
        assert_eq!(ok["integrity_status"], "intact");
        assert_ne!(ok["old_hash"]["md5"], ok["new_hash"]["md5"]);
        assert!(json["added"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == "neu.txt"));
        assert!(json["removed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == "gone.txt"));
    }

    /// The interim state per file follows the same rule as the result: every read file appears
    /// there with the kind its row has at the end (a mismatch is provisionally just a mismatch, the
    /// structure check comes with the result). Only the end knows about deleted files. Create
    /// reports every file as added, carried-over checksums too.
    #[test]
    fn live_rows_match_the_final_rows() {
        let dir = TempDir::new();
        let root = dir.path();
        let jpeg_ok = [0xff, 0xd8, 0x11, 0x22, 0x33, 0xff, 0xd9];
        fs::write(root.join("bad.jpg"), jpeg_ok).unwrap();
        fs::write(root.join("b.txt"), b"bbb").unwrap();
        fs::write(root.join("same.txt"), b"gleich").unwrap();
        fs::write(root.join("gone.txt"), b"weg").unwrap();
        go(options(root, JobKind::Generate)).unwrap();
        fs::write(root.join("bad.jpg"), [0xff, 0xd8, 0x11, 0x22, 0x99, 0x00]).unwrap();
        fs::write(root.join("b.txt"), b"BBB").unwrap();
        fs::remove_file(root.join("gone.txt")).unwrap();
        fs::write(root.join("neu.txt"), b"neu").unwrap();

        let progress = Progress::new();
        let outcome = run_scan(
            &options(root, JobKind::Verify),
            &progress,
            &AtomicBool::new(false),
        )
        .unwrap();
        let mut live = progress.take_live();
        live.sort_by(|left, right| left.path.cmp(&right.path));
        let kinds: Vec<(&str, RowKind)> = live
            .iter()
            .map(|item| (item.path.as_str(), item.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                ("b.txt", RowKind::Modified),
                ("bad.jpg", RowKind::Modified),
                ("neu.txt", RowKind::Added),
                ("same.txt", RowKind::Unchanged),
            ]
        );
        for item in &live {
            let row = outcome
                .rows
                .iter()
                .find(|row| row.path == item.path)
                .unwrap();
            assert_eq!(row.kind, item.kind, "{}", item.path);
            assert_eq!(row.new_hash, item.new_hash, "{}", item.path);
        }
        let bad = outcome
            .rows
            .iter()
            .find(|row| row.path == "bad.jpg")
            .unwrap();
        assert_eq!(bad.mark(), Some(RowMark::Damaged), "only the result knows");
        assert!(progress.take_live().is_empty(), "already picked up");

        // Create after Verify: carried-over and read files, all added.
        let progress = Progress::new();
        let mut opts = options(root, JobKind::Generate);
        opts.known = outcome.known.map(Arc::new);
        run_scan(&opts, &progress, &AtomicBool::new(false)).unwrap();
        let live = progress.take_live();
        assert_eq!(live.len(), 4);
        assert!(live.iter().all(|item| item.kind == RowKind::Recorded
            && item.new_hash.md5.is_some()
            && item.message.is_none()));
    }

    #[test]
    fn recursion_symlink_and_output_exclusion() {
        let dir = TempDir::new();
        fs::write(dir.path().join("top.txt"), b"abc").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("nested.txt"), b"abc").unwrap();
        fs::write(dir.path().join("checksums.md5"), b"alt\n").unwrap();
        let mut opts = options(dir.path(), JobKind::Generate);
        opts.recursive = false;
        let outcome = go(opts).unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(text.contains("top.txt"));
        assert!(!text.contains("nested.txt"));
        assert!(!text.contains("checksums.md5  "));
        assert_eq!(outcome.summary.ok, 1);

        let mut opts = options(dir.path(), JobKind::Generate);
        opts.recursive = true;
        go(opts).unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(text.contains("sub/nested.txt"));
        assert!(!text.lines().any(|line| line.ends_with("checksums.md5")));
        assert!(!text.lines().any(|line| line.ends_with("checksums.json")));
    }

    #[test]
    fn every_run_is_appended_to_the_history_and_no_backup_is_written() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        fs::write(dir.path().join("b.txt"), b"abc").unwrap();
        // A report in the earlier single format becomes the first run.
        let old = crate::report::ChangeReport {
            timestamp: "2026-10-01T10:00:00+00:00".into(),
            base_directory: "alt".into(),
            hash_mode: "md5".into(),
            action: Action::Created,
            baseline: true,
            unchanged_count: 0,
            file_count: 2,
            modified: vec![],
            added: vec![],
            removed: vec![],
            errors: vec![],
            skipped_symlinks: 0,
            skipped_non_regular: 0,
            cancelled: false,
            warnings: vec![],
        };
        fs::write(
            dir.path().join("checksums.json"),
            old.to_pretty_json().unwrap(),
        )
        .unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        fs::write(dir.path().join("a.txt"), b"neu").unwrap();
        fs::remove_file(dir.path().join("b.txt")).unwrap();
        go(options(dir.path(), JobKind::Verify)).unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();

        let history = history(dir.path());
        assert_eq!(history["format"], "veritas-verlauf-1");
        let runs = history["runs"].as_array().unwrap();
        let actions: Vec<&str> = runs
            .iter()
            .map(|run| run["action"].as_str().unwrap())
            .collect();
        assert_eq!(actions, ["created", "created", "verified", "created"]);
        assert_eq!(runs[0]["base_directory"], "alt");
        // Replacing records the old state: old hash of a.txt, deleted b.txt.
        let replaced = &runs[3];
        assert_eq!(
            replaced["modified"][0]["old_hash"]["md5"],
            "900150983CD24FB0D6963F7D28E17F72"
        );
        assert_eq!(replaced["removed"][0]["path"], "b.txt");
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|name| !name.contains(".bak")), "{names:?}");
    }

    #[test]
    fn a_foreign_json_is_replaced_with_a_warning() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        fs::write(dir.path().join("checksums.json"), b"{\"fremd\": 1}").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        let runs = history(dir.path())["runs"].as_array().unwrap().clone();
        assert_eq!(runs.len(), 1);
        assert!(runs[0]["warnings"][0]
            .as_str()
            .unwrap()
            .contains("was not a Veritas history"));
    }

    #[test]
    fn without_subfolders_nested_entries_are_kept_not_removed() {
        let dir = TempDir::new();
        fs::write(dir.path().join("top.txt"), b"abc").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("nested.txt"), b"abc").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();

        let mut verify = options(dir.path(), JobKind::Verify);
        verify.recursive = false;
        let outcome = go(verify).unwrap();
        assert_eq!(outcome.summary.removed, 0);
        assert_eq!(outcome.summary.ok, 1);
        assert!(outcome
            .report
            .warnings
            .iter()
            .any(|warning| warning.contains("1 entries in subfolders not verified")));

        fs::write(dir.path().join("top.txt"), b"neu").unwrap();
        let mut generate = options(dir.path(), JobKind::Generate);
        generate.recursive = false;
        go(generate).unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(text.contains("900150983CD24FB0D6963F7D28E17F72  sub/nested.txt\n"));
        assert!(!text.contains("900150983CD24FB0D6963F7D28E17F72  top.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let dir = TempDir::new();
        let outside = TempDir::new();
        fs::write(dir.path().join("real.txt"), b"abc").unwrap();
        fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
        std::os::unix::fs::symlink("real.txt", dir.path().join("link.txt")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("linked-dir")).unwrap();
        let outcome = go(options(dir.path(), JobKind::Generate)).unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(text.contains("real.txt"));
        assert!(!text.contains("secret"));
        assert!(!text.contains("link.txt"));
        assert!(outcome.report.skipped_symlinks >= 2);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_is_an_error_not_a_checksum_line() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        let secret = dir.path().join("secret.txt");
        fs::write(&secret, b"no").unwrap();
        fs::write(dir.path().join("ok.txt"), b"abc").unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::File::open(&secret).is_ok() {
            let _ = fs::set_permissions(&secret, fs::Permissions::from_mode(0o644));
            return;
        }
        let progress = Progress::new();
        let outcome = run_scan(
            &options(dir.path(), JobKind::Generate),
            &progress,
            &AtomicBool::new(false),
        )
        .unwrap();
        let _ = fs::set_permissions(&secret, fs::Permissions::from_mode(0o644));
        // Even the interim state names the read error, with its reason.
        assert!(progress
            .take_live()
            .iter()
            .any(|item| item.path == "secret.txt"
                && item.kind == RowKind::Error
                && item.message.is_some()));
        assert!(outcome.summary.errors >= 1);
        assert!(outcome
            .rows
            .iter()
            .any(|row| { row.path == "secret.txt" && row.kind == RowKind::Error }));
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(!text.contains("secret.txt"));
        assert!(text.contains("ok.txt"));
    }

    #[test]
    fn cancel_writes_nothing() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        let progress = Progress::new();
        let cancel = AtomicBool::new(true);
        let outcome =
            run_scan(&options(dir.path(), JobKind::Generate), &progress, &cancel).unwrap();
        assert!(outcome.cancelled);
        assert!(outcome.history.is_none());
        assert!(!dir.path().join("checksums.md5").exists());
        assert!(!dir.path().join("checksums.json").exists());
        assert!(!progress.is_writing());
    }

    /// A finished run has announced the writing; a cancel after that no longer applies.
    #[test]
    fn a_finished_run_refuses_a_late_cancel() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        let progress = Progress::new();
        let cancel = AtomicBool::new(false);
        let outcome =
            run_scan(&options(dir.path(), JobKind::Generate), &progress, &cancel).unwrap();
        assert!(!outcome.cancelled);
        assert!(progress.is_writing());
        assert!(!progress.request_cancel(&cancel));
        assert!(dir.path().join("checksums.md5").exists());
        assert_eq!(outcome.recorded, Some(vec!["a.txt".to_string()]));
    }

    /// The run hands over the history it wrote; the UI does not read it again.
    #[test]
    fn the_outcome_carries_the_written_history() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        let first = go(options(dir.path(), JobKind::Generate)).unwrap();
        assert_eq!(first.history.map(|history| history.runs.len()), Some(1));
        let second = go(options(dir.path(), JobKind::Verify)).unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.json")).unwrap();
        let (on_disk, ours) = History::from_text(&text);
        assert!(ours);
        assert_eq!(second.history, Some(on_disk));
    }

    #[test]
    fn verify_missing_or_garbage_does_not_overwrite() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        let err = go(options(dir.path(), JobKind::Verify)).unwrap_err();
        assert!(err.to_string().contains("No checksum file"));

        fs::write(
            dir.path().join("checksums.md5"),
            b"this is not a checksum\n",
        )
        .unwrap();
        let err = go(options(dir.path(), JobKind::Verify)).unwrap_err();
        assert!(err.to_string().contains("invalid"));
        assert_eq!(
            fs::read(dir.path().join("checksums.md5")).unwrap(),
            b"this is not a checksum\n"
        );

        go(options(dir.path(), JobKind::Generate)).unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(text.contains("900150983CD24FB0D6963F7D28E17F72  a.txt\n"));
    }

    #[test]
    fn sha256_and_both_modes() {
        let dir = TempDir::new();
        fs::write(dir.path().join("abc.txt"), b"abc").unwrap();
        let mut opts = options(dir.path(), JobKind::Generate);
        opts.hash_mode = HashMode::Sha256;
        go(opts).unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.sha256")).unwrap();
        let expected = hash_slice(b"abc", HashMode::Sha256).sha256.unwrap();
        assert!(text.starts_with(&expected));
        assert!(text.contains(&format!("{expected}  abc.txt\n")));
        assert!(!dir.path().join("checksums.md5").exists());

        let mut opts = options(dir.path(), JobKind::Generate);
        opts.hash_mode = HashMode::Both;
        go(opts).unwrap();
        assert!(dir.path().join("checksums.md5").is_file());
        assert!(dir.path().join("checksums.sha256").is_file());
        let json = last_run(dir.path());
        assert_eq!(json["hash_mode"], "md5+sha256");

        fs::write(dir.path().join("abc.txt"), b"abcd").unwrap();
        let mut opts = options(dir.path(), JobKind::Verify);
        opts.hash_mode = HashMode::Both;
        let outcome = go(opts).unwrap();
        assert_eq!(outcome.summary.modified, 1);
        let json = last_run(dir.path());
        assert!(json["modified"][0]["old_hash"]["md5"].is_string());
        assert!(json["modified"][0]["old_hash"]["sha256"].is_string());
        assert!(json["modified"][0]["new_hash"]["sha256"].is_string());
    }

    const ABC: &str = "900150983CD24FB0D6963F7D28E17F72";

    /// A folder `{name}.partial` blocks the staging file – like a read-only medium, only on every
    /// system and without changing permissions.
    fn block_partial(dir: &Path, name: &str) {
        fs::create_dir(dir.join(format!("{name}.partial"))).unwrap();
    }

    fn runs(dir: &Path) -> usize {
        history(dir)["runs"].as_array().unwrap().len()
    }

    /// Verify delivers its result even if the history cannot be written.
    #[test]
    fn verify_keeps_its_result_when_the_history_cannot_be_written() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        fs::write(dir.path().join("a.txt"), b"abcd").unwrap();
        block_partial(dir.path(), "checksums.json");
        let outcome = go(options(dir.path(), JobKind::Verify)).unwrap();
        assert_eq!(outcome.summary.modified, 1);
        assert!(outcome.history.is_none());
        assert!(
            outcome
                .report
                .warnings
                .iter()
                .any(|warning| warning.contains("not in the history")),
            "{:?}",
            outcome.report.warnings
        );
        assert_eq!(runs(dir.path()), 1);
    }

    /// If the history fails, the old `.md5` stays, and no staging file is left behind.
    #[test]
    fn generate_keeps_the_old_md5_when_the_history_fails() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        let before = fs::read(dir.path().join("checksums.md5")).unwrap();
        fs::write(dir.path().join("a.txt"), b"abcd").unwrap();
        block_partial(dir.path(), "checksums.json");
        let err = go(options(dir.path(), JobKind::Generate)).unwrap_err();
        assert!(err.to_string().contains("Nothing was written"), "{err}");
        assert_eq!(fs::read(dir.path().join("checksums.md5")).unwrap(), before);
        assert!(!dir.path().join("checksums.md5.partial").exists());
        assert_eq!(runs(dir.path()), 1);
    }

    /// An unreadable history is not replaced by an empty one.
    #[test]
    fn an_unreadable_history_is_left_alone() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        fs::write(dir.path().join("checksums.md5"), format!("{ABC}  a.txt\n")).unwrap();
        // A folder in its place cannot be read as a file on any system.
        fs::create_dir(dir.path().join("checksums.json")).unwrap();
        let err = go(options(dir.path(), JobKind::Generate)).unwrap_err();
        assert!(err.to_string().contains("not readable"), "{err}");
        assert!(dir.path().join("checksums.json").is_dir());
        let outcome = go(options(dir.path(), JobKind::Verify)).unwrap();
        assert_eq!(outcome.summary.ok, 1);
        assert!(outcome
            .report
            .warnings
            .iter()
            .any(|warning| warning.contains("not readable")));
    }

    /// Veritas does not overwrite the history of another Veritas version (downgrade).
    #[test]
    fn a_history_in_another_format_is_not_replaced() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        fs::write(dir.path().join("checksums.md5"), format!("{ABC}  a.txt\n")).unwrap();
        let future = br#"{"format": "veritas-verlauf-2", "laeufe": []}"#;
        fs::write(dir.path().join("checksums.json"), future).unwrap();
        let err = go(options(dir.path(), JobKind::Generate)).unwrap_err();
        assert!(err.to_string().contains("veritas-verlauf-2"), "{err}");
        let outcome = go(options(dir.path(), JobKind::Verify)).unwrap();
        assert_eq!(outcome.summary.ok, 1);
        assert_eq!(fs::read(dir.path().join("checksums.json")).unwrap(), future);
    }

    /// A leftover `.partial` is replaced, not written into.
    #[test]
    fn a_leftover_partial_file_is_replaced() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        fs::write(dir.path().join("checksums.md5.partial"), b"alt").unwrap();
        fs::write(dir.path().join("checksums.json.partial"), b"alt").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        assert!(!dir.path().join("checksums.md5.partial").exists());
        assert!(!dir.path().join("checksums.json.partial").exists());
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert_eq!(text, format!("{ABC}  a.txt\n"));
    }

    /// A link `….partial` laid in advance does not lead to overwriting its target.
    #[test]
    fn a_planted_partial_link_is_not_followed() {
        let dir = TempDir::new();
        let outside = TempDir::new();
        let target = outside.path().join("ziel.txt");
        fs::write(&target, b"bleibt").unwrap();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        let link = dir.path().join("checksums.json.partial");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_file(&target, &link).is_err() {
            return; // Without permission for links (no developer mode) there is nothing to check.
        }
        go(options(dir.path(), JobKind::Verify)).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"bleibt");
        assert_eq!(runs(dir.path()), 2);
    }

    /// Entries under an unreadable folder count as read errors, not as deleted.
    #[test]
    fn entries_below_an_unreadable_folder_are_errors() {
        let mut previous = BTreeMap::new();
        for path in ["Fotos/a.jpg", "Fotos/b/c.jpg", "Fotos2/d.jpg", "e.txt"] {
            previous.insert(path.to_string(), Digests::default());
        }
        let walk = [FileIssue {
            path: "fotos".into(),
            message: "Permission denied: fotos".into(),
        }];
        let paths = |issues: Vec<FileIssue>| -> Vec<String> {
            issues.into_iter().map(|issue| issue.path).collect()
        };
        let folded = paths(entries_below_issues(&previous, &walk, &[], true));
        assert_eq!(folded, ["Fotos/a.jpg", "Fotos/b/c.jpg"]);
        assert!(entries_below_issues(&previous, &walk, &[], false).is_empty());
        // The chosen folder itself (`""`): everything.
        let root = [FileIssue {
            path: String::new(),
            message: "Permission denied".into(),
        }];
        assert_eq!(entries_below_issues(&previous, &root, &[], false).len(), 4);
        // An error without a place: everything that was not hashed.
        let unknown = [FileIssue {
            path: UNKNOWN_PLACE.into(),
            message: "Error reading directory".into(),
        }];
        let hashed = [CurrentFile {
            path: "e.txt".into(),
            absolute: PathBuf::from("e.txt"),
            size: 1,
            modified: None,
            hash: Digests::default(),
        }];
        let rest = paths(entries_below_issues(&previous, &unknown, &hashed, false));
        assert_eq!(rest, ["Fotos/a.jpg", "Fotos/b/c.jpg", "Fotos2/d.jpg"]);
    }

    /// If the second staging file fails, the first one is discarded.
    #[test]
    fn a_failed_md5_partial_discards_the_staged_history() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        let before = fs::read(dir.path().join("checksums.md5")).unwrap();
        fs::write(dir.path().join("a.txt"), b"abcd").unwrap();
        block_partial(dir.path(), "checksums.md5");
        let err = go(options(dir.path(), JobKind::Generate)).unwrap_err();
        assert!(err.to_string().contains("Nothing was written"), "{err}");
        assert_eq!(fs::read(dir.path().join("checksums.md5")).unwrap(), before);
        assert!(!dir.path().join("checksums.json.partial").exists());
        assert_eq!(runs(dir.path()), 1);
    }

    /// In a run: Create carries over the entries of an unreadable folder unchanged.
    #[cfg(unix)]
    #[test]
    fn generate_keeps_entries_of_an_unreadable_folder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("b.txt"), b"abc").unwrap();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        go(options(dir.path(), JobKind::Generate)).unwrap();
        fs::set_permissions(&sub, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read_dir(&sub).is_ok() {
            let _ = fs::set_permissions(&sub, fs::Permissions::from_mode(0o755));
            return; // readable as root
        }
        let verify = go(options(dir.path(), JobKind::Verify));
        let generate = go(options(dir.path(), JobKind::Generate));
        let _ = fs::set_permissions(&sub, fs::Permissions::from_mode(0o755));
        let verify = verify.unwrap();
        assert_eq!(verify.summary.removed, 0);
        assert!(verify
            .rows
            .iter()
            .any(|row| row.path == "sub/b.txt" && row.kind == RowKind::Error));
        generate.unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(text.contains("sub/b.txt"), "{text}");
        assert!(last_run(dir.path())["removed"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    /// The chosen checksum file with a different spelling does not hash itself.
    #[test]
    fn own_md5_in_another_spelling_is_not_hashed() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        fs::write(dir.path().join("FOTOS.MD5"), format!("{ABC}  a.txt\n")).unwrap();
        let mut opts = options(dir.path(), JobKind::Generate);
        opts.md5_file = Some("FOTOS.MD5".into());
        go(opts.clone()).unwrap();
        let text = fs::read_to_string(dir.path().join("FOTOS.MD5")).unwrap();
        assert_eq!(text, format!("{ABC}  a.txt\n"));
        opts.kind = JobKind::Verify;
        let outcome = go(opts).unwrap();
        assert_eq!(outcome.summary.added, 0);
        assert_eq!(outcome.summary.ok, 1);
    }

    /// In a run: an ANSI file with `./` is read; the warning names the encoding.
    #[test]
    fn verify_reads_an_ansi_md5_with_dot_slash() {
        let dir = TempDir::new();
        fs::write(dir.path().join("Köln.txt"), b"abc").unwrap();
        let mut ansi = ABC.as_bytes().to_vec();
        ansi.extend(b"  ./K\xf6ln.txt\r\n");
        fs::write(dir.path().join("checksums.md5"), ansi).unwrap();
        let outcome = go(options(dir.path(), JobKind::Verify)).unwrap();
        assert_eq!(outcome.summary.ok, 1, "{:?}", outcome.report.warnings);
        assert_eq!(outcome.summary.added + outcome.summary.removed, 0);
        assert!(outcome
            .report
            .warnings
            .iter()
            .any(|warning| warning.contains("Windows-1252")));
    }

    /// Opening and deleting only for regular files in the folder.
    #[test]
    fn only_plain_files_inside_the_folder_pass_the_guard() {
        let dir = TempDir::new();
        let outside = TempDir::new();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("a.txt"), b"abc").unwrap();
        fs::write(outside.path().join("x.txt"), b"x").unwrap();
        let file = dir.path().join("sub").join("a.txt");
        assert!(contained_file(dir.path(), &file).is_ok());
        assert!(contained_file(dir.path(), &dir.path().join("sub")).is_err());
        assert!(contained_file(dir.path(), &outside.path().join("x.txt")).is_err());
        let sneaky = dir.path().join("sub").join("..").join("..");
        assert!(contained_file(dir.path(), &sneaky.join("x.txt")).is_err());
        assert!(contained_file(dir.path(), &dir.path().join("fehlt.txt")).is_err());
    }

    /// Behind a link in the folder lies nothing that Veritas opens or deletes.
    #[test]
    fn the_guard_does_not_follow_links() {
        let dir = TempDir::new();
        let outside = TempDir::new();
        fs::write(outside.path().join("x.txt"), b"x").unwrap();
        let link = dir.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        // On Windows a junction: it needs no permission for links.
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(&link)
                .arg(outside.path())
                .output()
                .is_ok_and(|out| out.status.success());
            if !made {
                return;
            }
        }
        assert!(link.join("x.txt").is_file(), "the link points outside");
        let reason = contained_file(dir.path(), &link.join("x.txt")).unwrap_err();
        assert!(reason.contains("link"), "{reason}");
        let mut checked = HashSet::new();
        assert!(plain_file_meta(dir.path(), &link.join("x.txt"), &mut checked).is_err());
    }

    #[test]
    fn folder_name_style() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        let mut opts = options(dir.path(), JobKind::Generate);
        opts.name_style = NameStyle::FolderName;
        let names = output_names(dir.path(), NameStyle::FolderName);
        go(opts).unwrap();
        assert!(dir.path().join(&names.md5).is_file());
        assert!(dir.path().join(&names.json).is_file());
    }

    /// Sets the modification time to `secs` seconds before now (negative: into the future). Freshly
    /// written files otherwise lie within `MTIME_SLACK` and never count as stable.
    fn set_age(path: &Path, secs: i64) {
        let now = SystemTime::now();
        let time = if secs >= 0 {
            now - Duration::from_secs(secs as u64)
        } else {
            now + Duration::from_secs(secs.unsigned_abs())
        };
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

    /// Writes a file and sets its modification time back by one hour.
    fn write_aged(path: &Path, data: &[u8]) {
        fs::write(path, data).unwrap();
        set_age(path, 3600);
    }

    fn with_known(mut opts: ScanOptions, known: &Option<Known>) -> ScanOutcome {
        opts.known = known.clone().map(Arc::new);
        go(opts).unwrap()
    }

    fn md5_line(dir: &Path, name: &str) -> String {
        fs::read_to_string(dir.join("checksums.md5"))
            .unwrap()
            .lines()
            .find(|line| line.ends_with(&format!("  {name}")))
            .unwrap_or_default()
            .to_string()
    }

    /// Create after Verify reads only what is new or has changed since; deleted files drop out, the
    /// history stays complete.
    #[test]
    fn generate_after_verify_reads_only_new_and_changed_files() {
        let dir = TempDir::new();
        let root = dir.path();
        for (name, data) in [
            ("a.txt", &b"abc"[..]),
            ("b.txt", b"bbb"),
            ("c.txt", b"ccc"),
            ("d.txt", b"ddd"),
            ("gone.txt", b"weg"),
        ] {
            write_aged(&root.join(name), data);
        }
        let first = go(options(root, JobKind::Generate)).unwrap();
        assert_eq!(
            (first.read, first.reused),
            (5, 0),
            "everything without prior knowledge"
        );
        let verified = go(options(root, JobKind::Verify)).unwrap();
        assert_eq!((verified.read, verified.reused), (5, 0));
        let known = verified.known;
        assert_eq!(known.as_ref().map(Known::len), Some(5));
        assert_eq!(
            known.as_ref().map(|known| known.source),
            Some(JobKind::Verify)
        );

        // b.txt: different size; c.txt: same size, different modification time; new.txt is added,
        // gone.txt goes. d.txt: content silently changed, size and time restored – only Verify sees
        // that (limit, `docs/manual.md`).
        write_aged(&root.join("b.txt"), b"bbbb");
        fs::write(root.join("c.txt"), b"CCC").unwrap();
        set_age(&root.join("c.txt"), 1800);
        let d_time = fs::metadata(root.join("d.txt"))
            .unwrap()
            .modified()
            .unwrap();
        fs::write(root.join("d.txt"), b"DDD").unwrap();
        fs::File::options()
            .write(true)
            .open(root.join("d.txt"))
            .unwrap()
            .set_modified(d_time)
            .unwrap();
        write_aged(&root.join("neu.txt"), b"neu");
        fs::remove_file(root.join("gone.txt")).unwrap();

        // Verify never takes anything from prior knowledge.
        let checked = with_known(options(root, JobKind::Verify), &known);
        assert_eq!((checked.read, checked.reused), (5, 0));

        let mut opts = options(root, JobKind::Generate);
        opts.known = known.clone().map(Arc::new);
        let progress = Progress::new();
        let replaced = run_scan(&opts, &progress, &AtomicBool::new(false)).unwrap();
        assert_eq!(
            (replaced.read, replaced.reused),
            (3, 2),
            "b, c and new read"
        );
        // The pill counts only what was read: "3 of 3 files".
        let snap = progress.snapshot();
        assert_eq!((snap.files_done, snap.files_total, snap.reused), (3, 3, 2));
        assert_eq!(snap.bytes_total, 4 + 3 + 3, "b.txt, c.txt, neu.txt");
        assert!(!replaced.baseline);
        let ddd = hash_slice(b"ddd", HashMode::Md5).md5.unwrap();
        assert_eq!(md5_line(root, "d.txt"), format!("{ddd}  d.txt"));
        let bbbb = hash_slice(b"bbbb", HashMode::Md5).md5.unwrap();
        assert_eq!(md5_line(root, "b.txt"), format!("{bbbb}  b.txt"));
        let ccc = hash_slice(b"CCC", HashMode::Md5).md5.unwrap();
        assert_eq!(md5_line(root, "c.txt"), format!("{ccc}  c.txt"));
        assert!(md5_line(root, "gone.txt").is_empty());
        assert!(!md5_line(root, "neu.txt").is_empty());

        let run = last_run(root);
        assert_eq!(run["action"], "created");
        assert_eq!(run["baseline"], false);
        let paths = |key: &str| -> Vec<String> {
            run[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry["path"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(paths("modified"), ["b.txt", "c.txt"]);
        assert_eq!(paths("added"), ["neu.txt"]);
        assert_eq!(paths("removed"), ["gone.txt"]);
        assert_eq!(
            run["modified"][0]["old_hash"]["md5"],
            hash_slice(b"bbb", HashMode::Md5).md5.unwrap()
        );
        assert_eq!(run["modified"][0]["new_hash"]["md5"], bbbb);
        assert!(
            run["warnings"].as_array().unwrap().iter().any(|note| note
                .as_str()
                .unwrap()
                .starts_with("2 checksums reused from the last verification")),
            "{run}"
        );

        // A second Create right after reads nothing any more.
        let next = replaced.known;
        assert_eq!(next.as_ref().map(Known::len), Some(5));
        let again = with_known(options(root, JobKind::Generate), &next);
        assert_eq!((again.read, again.reused), (0, 5));
        assert_eq!(again.summary.ok, 5);
    }

    /// A file whose modification time is not at least `MTIME_SLACK` before reading could have
    /// changed further unnoticed: its checksum is not carried over.
    #[test]
    fn a_file_changed_close_to_reading_is_read_again() {
        let dir = TempDir::new();
        write_aged(&dir.path().join("alt.txt"), b"alt");
        fs::write(dir.path().join("frisch.txt"), b"frisch").unwrap();
        set_age(&dir.path().join("frisch.txt"), -3600);
        let known = go(options(dir.path(), JobKind::Generate)).unwrap().known;
        let known_ref = known.as_ref().unwrap();
        assert_eq!(known_ref.len(), 1);
        assert!(known_ref
            .files
            .contains_key(&path_key("alt.txt", FOLD_CASE)));
        let again = with_known(options(dir.path(), JobKind::Generate), &known);
        assert_eq!((again.read, again.reused), (1, 1));

        // Cancel while comparing: nothing carried over.
        let found = || {
            vec![Found {
                absolute: dir.path().join("alt.txt"),
                relative: "alt.txt".into(),
                size: 3,
                modified: None,
            }]
        };
        let split = split_known(known_ref, found(), 1, &AtomicBool::new(true)).unwrap();
        assert_eq!((split.reused.len(), split.to_read.len()), (0, 1));
        let split = split_known(known_ref, found(), 1, &AtomicBool::new(false)).unwrap();
        assert_eq!((split.reused.len(), split.to_read.len()), (1, 0));

        let now = SystemTime::now();
        let stamp = |modified| Stamp { size: 1, modified };
        assert!(stamp(now - Duration::from_secs(3)).settled_before(now));
        assert!(!stamp(now - Duration::from_secs(1)).settled_before(now));
        assert!(!stamp(now + Duration::from_secs(5)).settled_before(now));
    }

    /// Prior knowledge from another folder, with a different *Subfolders* setting, or from before a
    /// changed checksum file does not apply: Create then reads everything and says so.
    #[test]
    fn known_checksums_that_no_longer_fit_are_ignored() {
        let dir = TempDir::new();
        let other = TempDir::new();
        for root in [dir.path(), other.path()] {
            write_aged(&root.join("a.txt"), b"abc");
            go(options(root, JobKind::Generate)).unwrap();
        }
        let known = go(options(dir.path(), JobKind::Verify)).unwrap().known;
        let mismatch = |outcome: &ScanOutcome| {
            outcome.reused == 0
                && outcome.read == 1
                && outcome
                    .report
                    .warnings
                    .iter()
                    .any(|note| note.contains("no longer fit"))
        };

        let elsewhere = with_known(options(other.path(), JobKind::Generate), &known);
        assert!(mismatch(&elsewhere), "{:?}", elsewhere.report.warnings);

        let mut flat = options(dir.path(), JobKind::Generate);
        flat.recursive = false;
        let flat = with_known(flat, &known);
        assert!(mismatch(&flat), "{:?}", flat.report.warnings);

        // The `.md5` has changed since Verify (different size).
        let md5 = dir.path().join("checksums.md5");
        let mut text = fs::read_to_string(&md5).unwrap();
        text.push_str(&text.clone());
        fs::write(&md5, text).unwrap();
        let stale = with_known(options(dir.path(), JobKind::Generate), &known);
        assert!(mismatch(&stale), "{:?}", stale.report.warnings);
    }

    /// The structure check of a mismatching, since unchanged file comes from Verify; Create does
    /// not read the file again for it.
    #[test]
    fn generate_keeps_the_structure_check_from_verify() {
        let dir = TempDir::new();
        let path = dir.path().join("bild.jpg");
        write_aged(&path, &[0xff, 0xd8, 0x11, 0x22, 0x33, 0xff, 0xd9]);
        go(options(dir.path(), JobKind::Generate)).unwrap();
        write_aged(&path, &[0xff, 0xd8, 0x11, 0x22, 0x99, 0x00, 0x10]);
        let verified = go(options(dir.path(), JobKind::Verify)).unwrap();
        assert_eq!(verified.summary.damaged, 1);
        let mut known = verified.known.unwrap();
        let entry = known
            .files
            .get_mut(&path_key("bild.jpg", FOLD_CASE))
            .unwrap();
        let finding = entry.integrity.as_mut().unwrap();
        assert_eq!(finding.status, IntegrityStatus::PossiblyDamaged);
        // Marked, to make visible where the finding comes from.
        finding.reason = "from verify".into();

        let replaced = with_known(options(dir.path(), JobKind::Generate), &Some(known));
        assert_eq!((replaced.read, replaced.reused), (0, 1));
        assert_eq!(replaced.summary.damaged, 1);
        let run = last_run(dir.path());
        assert_eq!(run["modified"][0]["integrity_reason"], "from verify");
        assert_eq!(run["modified"][0]["integrity_status"], "possibly damaged");
    }

    /// Hex of `data` in the algorithm `algo`.
    fn hex(data: &[u8], algo: HashMode) -> String {
        hash_slice(data, algo).get(algo).unwrap().clone()
    }

    /// The folder name that the history and a new `.md5` next to an SHA file are named after.
    fn folder(dir: &Path) -> String {
        dir.file_name().unwrap().to_str().unwrap().to_string()
    }

    /// The latest run in the history `{folder name}.json`.
    fn last_folder_run(dir: &Path) -> serde_json::Value {
        let path = dir.join(format!("{}.json", folder(dir)));
        let value: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        value["runs"].as_array().unwrap().last().unwrap().clone()
    }

    /// Verify against `SHA256SUMS` (GNU), `.sha1` (BSD) and `sha512sums.txt`: hashing uses only the
    /// file's algorithm; the one wrong checksum mismatches, a missing file is deleted, a new one is
    /// new. Another SHA file in the folder is not a new file, the SHA file stays as it was, and the
    /// history names the algorithm.
    #[test]
    fn verify_reads_sha_files() {
        for (file, algo, foreign, bsd) in [
            ("SHA256SUMS", HashMode::Sha256, "andere.sha1", false),
            ("Bilder.sha1", HashMode::Sha1, "SHA512SUMS", true),
            ("sha512sums.txt", HashMode::Sha512, "x.sha256sum", false),
        ] {
            let dir = TempDir::new();
            fs::create_dir(dir.path().join("sub")).unwrap();
            fs::write(dir.path().join("a.txt"), b"abc").unwrap();
            fs::write(dir.path().join("sub").join("b.txt"), b"neu").unwrap();
            fs::write(dir.path().join("c.txt"), b"nicht gelistet").unwrap();
            fs::write(dir.path().join(foreign), b"fremd\n").unwrap();
            let line = |hash: String, path: &str| {
                if bsd {
                    format!("{} ({path}) = {}\n", algo.id().to_ascii_uppercase(), hash)
                } else {
                    format!("{}  {path}\n", hash.to_ascii_lowercase())
                }
            };
            let text = [
                line(hex(b"abc", algo), "a.txt"),
                line(hex(b"alt", algo), "sub/b.txt"),
                line(hex(b"weg", algo), "gone.txt"),
            ]
            .concat();
            fs::write(dir.path().join(file), &text).unwrap();

            let mut opts = options(dir.path(), JobKind::Verify);
            opts.hash_mode = algo;
            opts.md5_file = Some(file.into());
            let outcome = go(opts).unwrap();
            let row = |path: &str| outcome.rows.iter().find(|row| row.path == path).unwrap();
            assert_eq!(row("a.txt").kind, RowKind::Unchanged, "{file}");
            assert_eq!(row("sub/b.txt").kind, RowKind::Modified, "{file}");
            assert_eq!(row("gone.txt").kind, RowKind::Removed, "{file}");
            assert_eq!(row("c.txt").kind, RowKind::Added, "{file}");
            assert_eq!(outcome.rows.len(), 4, "{file}: {:?}", outcome.rows);
            let a = row("a.txt");
            assert_eq!(a.new_hash.get(algo), Some(&hex(b"abc", algo)));
            assert!(a.new_hash.md5.is_none() || algo == HashMode::Md5);
            assert_eq!(
                row("sub/b.txt").old_hash.get(algo),
                Some(&hex(b"alt", algo))
            );
            assert_eq!(fs::read_to_string(dir.path().join(file)).unwrap(), text);
            assert!(!dir
                .path()
                .join(format!("{}.md5", folder(dir.path())))
                .exists());

            let run = last_folder_run(dir.path());
            assert_eq!(run["action"], "verified");
            assert_eq!(run["hash_mode"], algo.id());
            assert_eq!(
                run["modified"][0]["new_hash"][algo.id()],
                hex(b"neu", algo).as_str()
            );
            assert!(run["modified"][0]["new_hash"]["md5"].is_null());
        }
    }

    /// "Create MD5" next to an SHA file: reads every file, writes `{folder name}.md5` and the
    /// history `{folder name}.json`; the SHA file stays unchanged and is not in the new `.md5`.
    #[test]
    fn generate_beside_a_sha_file_writes_the_folder_md5() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        fs::write(dir.path().join("b.txt"), b"bbb").unwrap();
        let sums = format!("{}  a.txt\n", hex(b"abc", HashMode::Sha256));
        fs::write(dir.path().join("SHA256SUMS"), &sums).unwrap();
        let mut verify = options(dir.path(), JobKind::Verify);
        verify.hash_mode = HashMode::Sha256;
        verify.md5_file = Some("SHA256SUMS".into());
        let verified = go(verify).unwrap();
        // What Verify read with SHA-256 is no use for MD5.
        let known = verified.known.map(Arc::new);

        let mut opts = options(dir.path(), JobKind::Generate);
        opts.md5_file = Some("SHA256SUMS".into());
        opts.known = known;
        assert!(!opts.known.as_ref().unwrap().usable_for(&opts));
        let outcome = go(opts).unwrap();
        assert!(outcome.baseline);
        assert_eq!(outcome.read, 2);
        assert_eq!(outcome.reused, 0);
        assert_eq!(
            outcome.recorded,
            Some(vec!["a.txt".to_string(), "b.txt".to_string()])
        );
        let md5 =
            fs::read_to_string(dir.path().join(format!("{}.md5", folder(dir.path())))).unwrap();
        assert_eq!(
            md5,
            format!("{ABC}  a.txt\n{}  b.txt\n", hex(b"bbb", HashMode::Md5))
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("SHA256SUMS")).unwrap(),
            sums
        );
        let run = last_folder_run(dir.path());
        assert_eq!(run["action"], "created");
        assert_eq!(run["hash_mode"], "md5");
        // The history is the same as for the Verify before.
        let path = dir.path().join(format!("{}.json", folder(dir.path())));
        let history: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(history["runs"].as_array().unwrap().len(), 2);
        assert_eq!(history["runs"][0]["hash_mode"], "sha256");
    }

    /// If an SHA file is already in the `.md5`, it stays an ordinary file – otherwise it would be
    /// deleted when verifying. An SHA file that is not listed does not appear as new.
    #[test]
    fn a_sha_file_listed_in_the_md5_stays() {
        let dir = TempDir::new();
        fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        // Hashed along up to 0.8.0: also an SHA file that is named like the `.md5`.
        for name in ["SHA256SUMS", "checksums.sha1"] {
            fs::write(dir.path().join(name), b"abc").unwrap();
        }
        fs::write(dir.path().join("x.sha1"), b"fremd").unwrap();
        fs::write(
            dir.path().join("checksums.md5"),
            format!(
                "{ABC}  a.txt
{ABC}  SHA256SUMS
{ABC}  checksums.sha1
"
            ),
        )
        .unwrap();
        let expected = ["SHA256SUMS", "a.txt", "checksums.sha1"];
        // Preview and run see the same.
        let preview = crate::preview::preview_files(
            dir.path(),
            "checksums.md5",
            true,
            &AtomicBool::new(false),
        );
        let listed: Vec<(&str, RowKind)> = preview
            .rows
            .iter()
            .filter(|row| row.kind != RowKind::Checksum)
            .map(|row| (row.path.as_str(), row.kind))
            .collect();
        assert_eq!(listed, expected.map(|path| (path, RowKind::Recorded)));
        let outcome = go(options(dir.path(), JobKind::Verify)).unwrap();
        let kinds: Vec<(&str, RowKind)> = outcome
            .rows
            .iter()
            .map(|row| (row.path.as_str(), row.kind))
            .collect();
        assert_eq!(kinds, expected.map(|path| (path, RowKind::Unchanged)));
        // Create keeps it in the `.md5`, the foreign one stays out.
        go(options(dir.path(), JobKind::Generate)).unwrap();
        let text = fs::read_to_string(dir.path().join("checksums.md5")).unwrap();
        assert!(
            text.contains(
                "  SHA256SUMS
"
            ) && text.contains(
                "  checksums.sha1
"
            ) && !text.contains("x.sha1"),
            "{text}"
        );
    }
}

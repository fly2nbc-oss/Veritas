use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, CornerRadius, FontFamily, FontId, Id, Key, Label, Layout, Margin,
    Modifiers, Order, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, Vec2,
};
use serde::{Deserialize, Serialize};

use crate::about;
use crate::checksum::{self, NameStyle};
use crate::crash;
use crate::hashing::HashMode;
use crate::history::{self, Change, Timeline};
use crate::i18n::{self, plain_io, Lang, Say, Texts};
use crate::integrity::IntegrityStatus;
use crate::list::{
    self, in_list, is_finding, mark_tag, paint_header, paint_row, row_md5, row_menu, Listed, Live,
    RowAction, Rows, CHIP_MARKS,
};
use crate::preview::{self, include_own_rows, md5_file_name, Preview, NO_FOLDER};
use crate::progress::{Progress, ProgressSnap};
use crate::report::{path_key, FOLD_CASE};
use crate::scanning::{
    contained_file, run_scan, FileRow, JobKind, Known, RowKind, RowMark, ScanOptions, ScanOutcome,
};
use crate::system::{move_to_trash, no_trash, show_in_folder};
use crate::theme::{self, icon, paint_fit, palette, radius, space, text, Role, COMPACT, CONTROL};
use crate::tree::{filesystem_roots, paint_tree_row, parent_chain, tree_label, Tree};

mod shot;

/// Sidebar with the folder tree, fixed width (R1).
pub(crate) const TREE_W: f32 = 232.0;
/// Rows in table and tree. Deliberate exception to §4.2 (32): the file list stays dense.
pub(crate) const ROW: f32 = 28.0;
/// Search field in the bar; narrower than before (220, then 180), so that at the smallest window
/// size the name of the checksum file keeps its room – also with the language button, in every
/// language and with Noto Sans on Linux.
const SEARCH_W: f32 = 140.0;
const PILL_W: f32 = 600.0;
const PILL_H: f32 = 48.0;
const NOTICE_W: f32 = 520.0;
/// Field for the thread count (up to three digits).
const THREADS_W: f32 = 48.0;
const SETTINGS_KEY: &str = "veritas-einstellungen";
/// Pause after an arrow key in the tree before the folder is read (seconds).
const DEBOUNCE: f64 = 0.15;
/// How long the first Esc counts during a run; a second one then cancels (R7).
const ARM_SECS: f64 = 4.0;
/// The list shows the interim state of a run every this many seconds – not in every frame, so that
/// it stays calm and does not constantly collect anew.
const LIVE_SECS: f64 = 2.0;
/// This many recent folders the empty state names.
const RECENT: usize = 3;
/// Window size at start and smallest window size (`main.rs`). At the smallest size the bar must
/// show everything without overlapping (`tests::the_bar_fits_the_smallest_window`).
pub const START_SIZE: [f32; 2] = [1180.0, 820.0];
pub const MIN_SIZE: [f32; 2] = [880.0, 620.0];
/// How long closing waits for a run that is writing right now.
const EXIT_WAIT: Duration = Duration::from_secs(10);

#[derive(Serialize, Deserialize)]
struct Settings {
    recursive: bool,
    threads: usize,
    /// Time (seconds since 1970) of the crash log that Veritas has already pointed out.
    #[serde(default)]
    crash_seen: u64,
    /// The recent folders, newest first; at most `RECENT`.
    #[serde(default)]
    recent: Vec<PathBuf>,
    /// The chosen language (`de`, `en` …); if it is missing, the system's language applies at
    /// start.
    #[serde(default)]
    language: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recursive: true,
            threads: default_threads(),
            crash_seen: 0,
            recent: Vec::new(),
            language: None,
        }
    }
}

struct Running {
    root: PathBuf,
    progress: Arc<Progress>,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Result<ScanOutcome, Say>>,
    /// The interim state in the list (`Live`).
    live: Live,
    /// From this time on (`input.time`) it is picked up next.
    live_due: f64,
}

/// The preview of a folder that is still to come.
struct Pending {
    /// Read only from this time on (`input.time`); arrow keys in the tree are debounced this way.
    due: f64,
    /// Set as soon as the thread reads. Every request has its own channel; if it is replaced,
    /// nobody listens any more, so an outdated result never arrives.
    rx: Option<Receiver<Preview>>,
    /// Set when the request lapses: the thread then stops early, so that at most one reads
    /// usefully.
    stop: Arc<AtomicBool>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Confirmation before replacing the checksum file (dialog card, R4).
struct Confirm {
    name: String,
    history: String,
    entries: usize,
    nested: usize,
    /// Checksums from the last run that Create carries over instead of reading the file.
    reuse: Option<Reuse>,
}

/// What the confirmation says about the known checksums.
struct Reuse {
    count: usize,
    source: JobKind,
    /// Time of day of the run, "14:02".
    at: String,
}

/// Confirmation before deleting (dialog card): the file goes to the Recycle Bin.
struct Delete {
    path: String,
    absolute: PathBuf,
    /// Is listed in the checksum file; afterwards it counts as deleted until "Create MD5" removes
    /// it.
    recorded: bool,
}

/// Moves a file to the Recycle Bin. Tests inject a dummy so that they do not fill the machine's
/// Recycle Bin.
type Trasher = fn(&Path) -> Result<(), String>;

/// Shows a file in the file manager. Tests inject a dummy so that no window opens.
type Revealer = fn(&Path) -> Result<(), String>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Done,
    Warn,
    Error,
}

/// Feedback per R6: notices fade, errors stay until Esc or click.
struct Notice {
    text: Say,
    tone: Tone,
    shown: f64,
    /// Stays like an error until Esc or click: the result of a run with a finding. Whoever comes
    /// back after an hour should still see it.
    sticky: bool,
}

impl Notice {
    /// Display time per §6: 5 s, plus 60 ms per character, at most 10 s. Errors and findings stay.
    fn hold(&self) -> Option<f64> {
        (self.tone != Tone::Error && !self.sticky)
            .then(|| (5.0 + 0.06 * self.text.get().chars().count() as f64).min(10.0))
    }

    /// Stays until it is closed.
    fn stays(&self) -> bool {
        self.hold().is_none()
    }

    /// Opacity at time `now`; then a 400 ms fade-out.
    fn opacity(&self, now: f64) -> f32 {
        let Some(hold) = self.hold() else {
            return 1.0;
        };
        let elapsed = now - self.shown;
        if elapsed <= hold {
            1.0
        } else {
            (1.0 - (elapsed - hold) / 0.4).clamp(0.0, 1.0) as f32
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Tree,
    List,
}

pub struct VeritasApp {
    root: Option<PathBuf>,
    md5_name: String,
    md5_exists: bool,
    /// An explicitly opened checksum file (`.md5` or SHA file). It applies instead of the rule
    /// (folder name, the only one, newest, otherwise SHA) as long as the folder stays.
    pinned_md5: Option<String>,
    recursive: bool,
    threads: usize,
    /// The folder is on a classic hard disk (HDD); runs hash one file after another there
    /// (`drive`). Read when the folder is opened.
    seeks: Option<bool>,
    /// Notes on the checksum file or the warnings of the last run, as stored (`i18n::shown`).
    filter_note: Vec<String>,
    selected: Option<String>,
    rows: Rows,
    /// The visible rows per filter and search, cached.
    listed: Listed,
    /// The preview that is being read right now or is about to be read.
    pending: Option<Pending>,
    /// The paths of the checksum file (`path_key`), from the preview or the last Create.
    recorded: HashSet<String>,
    /// If an SHA file applies: the paths of the `{folder name}.md5` next to it, which "Create MD5"
    /// would replace (`Preview::md5_recorded`); `None` if there is none.
    md5_recorded: Option<HashSet<String>>,
    /// The rows come from a run, not from the preview when opening.
    from_run: bool,
    /// Checksums, size and modification time from the last finished run in this folder. "Create
    /// MD5" carries them over for unchanged files. They lapse with every new preview (folder
    /// change, subfolders toggled, different `.md5`, failed run).
    known: Option<Arc<Known>>,
    /// The status bar on the left while no row is selected.
    status: Say,
    running: Option<Running>,
    confirm: Option<Confirm>,
    /// The first Esc in a run when there was nothing else to undo (time). A second Esc within
    /// `ARM_SECS` cancels.
    cancel_armed: Option<f64>,
    /// Confirmation "Cancel the run and close?" after the window was asked to close during a run.
    close_ask: bool,
    /// The confirmation is confirmed; the next close request goes through.
    close_confirmed: bool,
    /// "About Veritas" is open.
    about_open: bool,
    /// The folder's history, read when opening and after every run; otherwise why not.
    timeline: Result<Timeline, Say>,
    /// The history card is open.
    history_view: Option<history::View>,
    /// Confirmation before deleting.
    delete: Option<Delete>,
    trasher: Trasher,
    revealer: Revealer,
    drop_hover: bool,
    status_filter: Option<RowMark>,
    tree: Tree,
    roots_checked: f64,
    reveal: bool,
    query: String,
    pane: Pane,
    list_offset: Option<f32>,
    /// Scroll position and visible height of the list in the last frame.
    list_view: (f32, f32),
    notice: Option<Notice>,
    /// See `Settings::crash_seen`.
    crash_seen: u64,
    content_rect: Rect,
    /// Where the bar's controls were in the last frame; tests check that nothing overlaps at the
    /// smallest window size.
    #[cfg_attr(not(test), allow(dead_code))]
    bar_rects: Vec<(&'static str, Rect)>,
    /// Window title set last.
    title: String,
    /// The recent folders, newest first.
    recent: Vec<PathBuf>,
    /// Screenshot mode (`veritas --screenshot`): flow and target; the window then saves nothing,
    /// accepts no input and keeps its tree root.
    shot: Option<shot::Shot>,
}

impl VeritasApp {
    /// `start` is a folder or a file from the command line; for a file, its folder applies.
    pub fn new(cc: &eframe::CreationContext<'_>, start: Option<PathBuf>) -> Self {
        // The install check in CI waits for this line. stderr is redirected in CI and otherwise
        // block-buffered.
        eprintln!("start-up: window ready");
        let _ = std::io::Write::flush(&mut std::io::stderr());
        theme::install(&cc.egui_ctx);
        let settings: Settings = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, SETTINGS_KEY))
            .unwrap_or_default();
        // The saved language, on first start the system's (fallback English).
        i18n::set(
            settings
                .language
                .as_deref()
                .and_then(Lang::from_code)
                .unwrap_or_else(i18n::system_default),
        );
        let mut app = Self::with_settings(settings);
        if let Some(log) = crash::log_path() {
            let written = crash::last_written(&log);
            app.note_crash(&cc.egui_ctx, &log, written);
        }
        if let Some(path) = start {
            // A relative path like `.` would have no folder name for the .md5.
            let path = std::path::absolute(&path).unwrap_or(path);
            app.open_path(&cc.egui_ctx, path);
        }
        app
    }

    fn with_settings(settings: Settings) -> Self {
        Self {
            root: None,
            md5_name: String::new(),
            md5_exists: false,
            pinned_md5: None,
            recursive: settings.recursive,
            threads: settings.threads.clamp(1, 256),
            seeks: None,
            filter_note: Vec::new(),
            selected: None,
            rows: Rows::default(),
            listed: Listed::default(),
            pending: None,
            recorded: HashSet::new(),
            md5_recorded: None,
            from_run: false,
            known: None,
            status: Say::of(|t| t.pick_folder),
            running: None,
            confirm: None,
            cancel_armed: None,
            close_ask: false,
            close_confirmed: false,
            about_open: false,
            timeline: Err(Say::stored(NO_FOLDER)),
            history_view: None,
            delete: None,
            trasher: move_to_trash,
            revealer: show_in_folder,
            drop_hover: false,
            status_filter: None,
            tree: Tree::new(filesystem_roots()),
            roots_checked: 0.0,
            reveal: false,
            query: String::new(),
            pane: Pane::Tree,
            list_offset: None,
            list_view: (0.0, 0.0),
            notice: None,
            crash_seen: settings.crash_seen,
            content_rect: Rect::NOTHING,
            bar_rects: Vec::new(),
            title: "Veritas".into(),
            recent: settings.recent,
            shot: None,
        }
    }

    /// One-time notice per R12 when the crash log has grown since the last notice. As an error, so
    /// that it stays until it has been read.
    fn note_crash(&mut self, ctx: &egui::Context, log: &Path, written: Option<u64>) {
        let Some(written) = written.filter(|&written| written > self.crash_seen) else {
            return;
        };
        self.crash_seen = written;
        let log = log.display().to_string();
        self.notify(ctx, Tone::Error, Say::new(move |t| (t.crash_notice)(&log)));
    }

    fn notify(&mut self, ctx: &egui::Context, tone: Tone, text: impl Into<Say>) {
        self.notice = Some(Notice {
            text: text.into(),
            tone,
            shown: ctx.input(|input| input.time),
            sticky: false,
        });
    }

    /// Drives rarely change; listing them costs no disk access on Windows.
    fn refresh_roots(&mut self, ctx: &egui::Context) {
        // In screenshot mode the example's parent folder stays the only root.
        if self.shot.is_some() {
            return;
        }
        let now = ctx.input(|input| input.time);
        if now - self.roots_checked >= 2.0 {
            self.tree.set_roots(filesystem_roots());
            self.roots_checked = now;
        }
    }

    fn update_drops(&mut self, ctx: &egui::Context) {
        let hovered = ctx.input(|input| !input.raw.hovered_files.is_empty());
        self.drop_hover = hovered && self.running.is_none();
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        let more = dropped.len() > 1;
        let Some(path) = dropped
            .into_iter()
            .next()
            .map(|file| file.path().to_path_buf())
        else {
            return;
        };
        if self.running.is_some() {
            self.notify(ctx, Tone::Warn, Say::of(|t| t.drop_running));
            return;
        }
        let plain_file = path.is_file() && md5_file_name(&path).is_none();
        let first = tree_label(&path);
        self.open_path(ctx, path);
        if more {
            self.notify(ctx, Tone::Warn, Say::new(move |t| (t.drop_many)(&first)));
        } else if plain_file {
            self.notify(ctx, Tone::Done, Say::of(|t| t.drop_file));
        }
    }

    /// Opens a folder, a checksum file (exactly this one then applies) or the folder of another
    /// file.
    fn open_path(&mut self, ctx: &egui::Context, path: PathBuf) {
        if path.is_dir() {
            self.open_folder(ctx, path, true, None);
            return;
        }
        let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            return;
        };
        let md5 = md5_file_name(&path);
        self.open_folder(ctx, parent.to_path_buf(), true, md5);
    }

    fn select_folder(&mut self, ctx: &egui::Context, path: PathBuf, expand: bool) {
        self.open_folder(ctx, path, expand, None);
    }

    /// `md5`: explicitly opened checksum file in this folder.
    fn open_folder(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        expand: bool,
        md5: Option<String>,
    ) {
        if self.running.is_some() {
            self.notify(ctx, Tone::Warn, Say::of(|t| t.folder_running));
            return;
        }
        if !path.is_dir() {
            let shown = path.display().to_string();
            self.notify(
                ctx,
                Tone::Error,
                Say::new(move |t| (t.not_a_folder)(&shown)),
            );
            return;
        }
        for parent in parent_chain(&path) {
            if !self.tree.is_expanded(&parent) {
                self.tree.expand(&parent, false);
            }
        }
        if expand {
            self.tree.expand(&path, true);
        }
        self.reveal = true;
        let same = self.root.as_ref() == Some(&path);
        if !same || md5.is_some() {
            self.pinned_md5 = md5.clone();
        }
        if same && self.from_run && md5.is_none() {
            return;
        }
        // Only deliberately opened folders (click, drop, command line, empty state), not every stop
        // of the arrow keys in the tree.
        if expand {
            self.remember(&path);
        }
        self.root = Some(path);
        // Arrow keys in the tree (not `expand`) read only after a short pause; the selection moves
        // immediately anyway.
        self.load_preview(ctx, if expand { 0.0 } else { DEBOUNCE });
    }

    /// Remembers the folder for the empty state: at the front, without duplicates, at most
    /// `RECENT`.
    fn remember(&mut self, path: &Path) {
        self.recent.retain(|known| known != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(RECENT);
    }

    /// Reads the folder and its checksum file without hashing – in a thread of its own. Until the
    /// result arrives, the list is empty and the status bar says so; the folder itself is chosen
    /// immediately. `delay` waits that long first (arrow keys in the tree).
    fn load_preview(&mut self, ctx: &egui::Context, delay: f64) {
        let Some(root) = self.root.clone() else {
            return;
        };
        self.selected = None;
        self.status_filter = None;
        self.query.clear();
        self.list_offset = Some(0.0);
        self.filter_note.clear();
        self.from_run = false;
        self.known = None;
        self.history_view = None;
        self.delete = None;
        // An open confirmation belongs to the previous folder.
        self.confirm = None;
        self.rows.clear();
        self.recorded.clear();
        self.md5_recorded = None;
        self.md5_name.clear();
        self.md5_exists = false;
        self.seeks = None;
        self.timeline = Err(Say::of(|t| t.loading));
        let folder = tree_label(&root);
        self.status = Say::new(move |t| (t.reading_folder)(&folder));
        // A new request replaces every older one; its thread stops, its result no longer arrives.
        self.pending = Some(Pending {
            due: ctx.input(|input| input.time) + delay,
            rx: None,
            stop: Arc::new(AtomicBool::new(false)),
        });
        if delay <= 0.0 {
            self.start_preview(ctx);
        }
    }

    /// Starts reading the requested preview. If no thread can be created, the UI reads itself –
    /// better to stall briefly than never to finish.
    fn start_preview(&mut self, ctx: &egui::Context) {
        let Some(root) = self.root.clone() else {
            self.pending = None;
            return;
        };
        let pinned = self.pinned_md5.clone();
        let recursive = self.recursive;
        let Some(stop) = self
            .pending
            .as_ref()
            .map(|pending| Arc::clone(&pending.stop))
        else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("veritas-vorschau".into())
            .spawn(move || {
                let preview = preview::load(&root, pinned.as_deref(), recursive, &stop);
                // Whoever has chosen another folder in the meantime no longer listens.
                if tx.send(preview).is_ok() {
                    repaint.request_repaint();
                }
            });
        match spawned {
            Ok(_) => {
                if let Some(pending) = &mut self.pending {
                    pending.rx = Some(rx);
                }
            }
            Err(err) => {
                log::warn!("Preview without a thread: {err}");
                self.pending = None;
                let root = self.root.clone().unwrap_or_default();
                let stop = AtomicBool::new(false);
                let preview =
                    preview::load(&root, self.pinned_md5.as_deref(), self.recursive, &stop);
                self.apply_preview(preview);
            }
        }
    }

    /// Picks up a finished preview; a debounced one starts as soon as its time has come.
    fn poll_preview(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending else {
            return;
        };
        let Some(rx) = &pending.rx else {
            let left = pending.due - ctx.input(|input| input.time);
            if left > 0.0 {
                ctx.request_repaint_after(Duration::from_secs_f64(left));
            } else {
                self.start_preview(ctx);
            }
            return;
        };
        match rx.try_recv() {
            Ok(preview) => {
                self.pending = None;
                self.apply_preview(preview);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                // Without a preview at least the right checksum file applies.
                self.refresh_md5();
                self.timeline = Err(Say::of(|t| t.unread));
                self.status = Say::of(|t| t.folder_unreadable_status);
                self.notify(ctx, Tone::Error, Say::of(|t| t.folder_unreadable));
            }
        }
    }

    /// Adopts the preview that was read.
    fn apply_preview(&mut self, preview: Preview) {
        self.md5_name = preview.md5_name;
        self.md5_exists = preview.md5_exists;
        self.rows = Rows::from(preview.rows);
        self.filter_note = preview.note.into_iter().collect();
        self.timeline = preview.timeline.map_err(Say::stored);
        self.seeks = preview.seeks;
        self.recorded = preview.recorded;
        self.md5_recorded = preview.md5_recorded;
        let files = self
            .rows
            .iter()
            .filter(|row| !matches!(row.kind, RowKind::Checksum | RowKind::Report))
            .count();
        let (name, exists) = (self.md5_name.clone(), self.md5_exists);
        self.status = Say::new(move |t| (t.folder_status)(&name, files, exists));
    }

    fn refresh_md5(&mut self) {
        match &self.root {
            Some(root) => {
                (self.md5_name, self.md5_exists) =
                    preview::md5_choice(root, self.pinned_md5.as_deref());
            }
            None => {
                self.md5_name.clear();
                self.md5_exists = false;
            }
        }
    }

    /// The one place that says whether a run is allowed right now, and otherwise why not (R2, R3).
    fn can_start(&self, kind: JobKind) -> Result<(), Say> {
        if self.running.is_some() {
            return Err(Say::of(|t| t.run_busy));
        }
        if self.root.is_none() {
            return Err(Say::stored(NO_FOLDER));
        }
        // Only once the preview has arrived is it settled which checksum file applies.
        if self.pending.is_some() {
            return Err(Say::of(|t| t.loading));
        }
        if kind == JobKind::Verify && !self.md5_exists {
            let name = self.md5_name.clone();
            return Err(Say::new(move |t| (t.verify_needs_md5)(&name)));
        }
        Ok(())
    }

    fn request(&mut self, ctx: &egui::Context, kind: JobKind) {
        if let Err(reason) = self.can_start(kind) {
            self.notify(ctx, Tone::Warn, reason);
            return;
        }
        // If an SHA file applies, Create writes `{folder name}.md5` next to it; a confirmation is
        // asked only if that one already exists.
        let sha = self.algo() != HashMode::Md5;
        let replaces = if sha {
            self.md5_recorded.is_some()
        } else {
            self.md5_exists
        };
        if kind == JobKind::Generate && replaces {
            let reuse = self.usable_known().map(|known| Reuse {
                count: known.len(),
                source: known.source,
                at: i18n::clock(known.at),
            });
            // The numbers from the preview or the last Create, without reading the file once more
            // here on the UI thread.
            let recorded = match &self.md5_recorded {
                Some(recorded) if sha => recorded,
                _ => &self.recorded,
            };
            let nested = if self.recursive {
                0
            } else {
                recorded.iter().filter(|path| path.contains('/')).count()
            };
            let entries = recorded.len();
            let name = self.md5_target();
            let history = self
                .root
                .as_ref()
                .map(|root| checksum::names_for(root, &name).json)
                .unwrap_or_default();
            self.confirm = Some(Confirm {
                name,
                history,
                entries,
                nested,
                reuse,
            });
            ctx.memory_mut(|mem| mem.stop_text_input());
            return;
        }
        self.run(ctx, kind);
    }

    fn decide(&mut self, ctx: &egui::Context, replace: bool) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        if replace {
            self.run(ctx, JobKind::Generate);
        } else {
            self.status = Say::new(move |t| (t.md5_unchanged)(&confirm.name));
        }
    }

    /// The settings of a run in the chosen folder, without known checksums.
    fn scan_options(&self, root: PathBuf, kind: JobKind) -> ScanOptions {
        ScanOptions {
            root,
            recursive: self.recursive,
            threads: self.threads.clamp(1, 256),
            // Read when opening; the run does not ask the drive again.
            seeks: self.seeks,
            // Verify with the algorithm of the checksum file, Create always MD5.
            hash_mode: match kind {
                JobKind::Verify => self.algo(),
                JobKind::Generate => HashMode::Md5,
            },
            name_style: NameStyle::FolderName,
            md5_file: Some(self.md5_name.clone()),
            kind,
            known: None,
        }
    }

    /// The algorithm of the checksum file that applies – by its name; without a file MD5.
    fn algo(&self) -> HashMode {
        checksum::algorithm_of_name(&self.md5_name).unwrap_or(HashMode::Md5)
    }

    /// The `.md5` that "Create MD5" writes: the `.md5` that applies; if an SHA file applies,
    /// `{folder name}.md5` next to it (`checksum::names_for`). The SHA file stays untouched.
    fn md5_target(&self) -> String {
        match &self.root {
            Some(root) if self.algo() != HashMode::Md5 => {
                checksum::names_for(root, &self.md5_name).md5
            }
            _ => self.md5_name.clone(),
        }
    }

    /// The checksums of the last run, if "Create MD5" can carry them over now; otherwise they lapse
    /// here (e.g. because the `.md5` has changed since). Asks the file system once about the `.md5`
    /// but reads no file.
    fn usable_known(&mut self) -> Option<Arc<Known>> {
        let root = self.root.clone()?;
        let opts = self.scan_options(root, JobKind::Generate);
        if !self
            .known
            .as_ref()
            .is_some_and(|known| !known.is_empty() && known.usable_for(&opts))
        {
            self.known = None;
        }
        self.known.clone()
    }

    fn run(&mut self, ctx: &egui::Context, kind: JobKind) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let mut opts = self.scan_options(root.clone(), kind);
        // Verify always reads every file; only Create carries over known checksums.
        if kind == JobKind::Generate {
            opts.known = self.usable_known();
        }
        let progress = Arc::new(Progress::new());
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let progress_thread = Arc::clone(&progress);
        let cancel_thread = Arc::clone(&cancel);
        match std::thread::Builder::new()
            .name("veritas-scan".into())
            .spawn(move || {
                // The text of the error is English (`Msg`); the UI shows it translated.
                let result = run_scan(&opts, &progress_thread, &cancel_thread)
                    .map_err(|err| Say::stored(err.to_string()));
                log::info!("Run finished: {:?}, result to the window", opts.kind);
                let _ = tx.send(result);
            }) {
            Ok(_) => {
                // `{:?}`: a line break in the folder name does not forge a line in the log.
                log::info!("Run starts: {kind:?} in {:?}", root.display().to_string());
                self.cancel_armed = None;
                // A focus in the *Threads* field would stay stuck in the locked field and take Esc
                // (twice Esc cancels).
                ctx.memory_mut(|mem| mem.surrender_focus(threads_id()));
                self.remember(&root);
                // What an earlier run found counts as open again; the interim state fills it in.
                let mut live = Live::default();
                live.reset(&mut self.rows, &self.recorded);
                self.running = Some(Running {
                    root,
                    progress,
                    cancel,
                    rx,
                    live,
                    live_due: ctx.input(|input| input.time) + LIVE_SECS,
                });
                self.status = match kind {
                    JobKind::Generate => {
                        let name = self.md5_target();
                        Say::new(move |t| (t.status_generating)(&name))
                    }
                    JobKind::Verify => {
                        let name = self.md5_name.clone();
                        Say::new(move |t| (t.status_verifying)(&name))
                    }
                };
            }
            Err(err) => {
                log::warn!("Run could not start: {err}");
                let why = plain_io(&err);
                self.notify(
                    ctx,
                    Tone::Error,
                    Say::new(move |t| (t.run_not_started)(&why)),
                );
            }
        }
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let finished = self
            .running
            .as_ref()
            .and_then(|running| match running.rx.try_recv() {
                Ok(result) => Some((running.root.clone(), result)),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some((running.root.clone(), Err(Say::of(|t| t.run_died))))
                }
            });
        let finished_now = finished.is_some();
        if let Some((root, result)) = finished {
            // Undo the interim state first: a finished run replaces the rows anyway, a cancelled or
            // failed one leaves the list as it was before.
            if let Some(running) = self.running.take() {
                if self.root.as_ref() == Some(&running.root) {
                    running.live.restore(&mut self.rows);
                }
            }
            self.cancel_armed = None;
            match &result {
                // The log is always English, regardless of the UI language.
                Ok(outcome) => log::info!(
                    "Run taken over: {:?} in {:?}{}",
                    outcome.kind,
                    root.display().to_string(),
                    if outcome.cancelled { ", cancelled" } else { "" }
                ),
                Err(message) => log::warn!(
                    "Run failed in {:?}: {:?}",
                    root.display().to_string(),
                    message.in_lang(Lang::En.texts())
                ),
            }
            match result {
                Ok(outcome) => self.apply_outcome(ctx, root, outcome),
                Err(message) => {
                    // A failed run may also have changed files: re-read rows, checksum file and
                    // history, in the background.
                    if self.root.as_ref() == Some(&root) {
                        self.load_preview(ctx, 0.0);
                    }
                    self.status = message.clone();
                    self.notify(ctx, Tone::Error, message);
                }
            }
        }
        if finished_now {
            // Whoever is doing something else in the meantime learns about it through the taskbar.
            if !ctx.input(|input| input.viewport().focused.unwrap_or(true)) {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Informational,
                ));
            }
        }
        self.poll_live(ctx.input(|input| input.time));
        if self.running.is_some() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
        self.update_title(ctx);
    }

    /// Picks up the interim state of the running run every `LIVE_SECS` and shows it in the list;
    /// selection, filter and scroll position stay.
    fn poll_live(&mut self, now: f64) {
        let Some(running) = &mut self.running else {
            return;
        };
        if now < running.live_due || self.root.as_ref() != Some(&running.root) {
            return;
        }
        running.live_due = now + LIVE_SECS;
        running
            .live
            .apply(&mut self.rows, running.progress.take_live());
    }

    /// The window title shows the progress of a run, in the taskbar too.
    fn update_title(&mut self, ctx: &egui::Context) {
        let t = i18n::t();
        let title = match &self.running {
            Some(running) if running.progress.is_writing() => {
                format!("{} · Veritas", t.title_writing)
            }
            Some(running) if running.cancel.load(Ordering::SeqCst) => {
                format!("{} · Veritas", t.title_cancelling)
            }
            Some(running) => {
                let percent = (running.progress.snapshot().fraction() * 100.0).floor();
                format!("{} · Veritas", (t.percent)(percent as u32))
            }
            None => "Veritas".to_string(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn apply_outcome(&mut self, ctx: &egui::Context, root: PathBuf, outcome: ScanOutcome) {
        if self.root.as_ref() != Some(&root) {
            return;
        }
        // A preview from before the run is outdated.
        self.pending = None;
        // After every run the arrow keys belong to the list: ↓ would otherwise change the folder in
        // the tree and discard the result.
        self.pane = Pane::List;
        if outcome.cancelled {
            self.status = Say::of(|t| t.cancelled);
            self.notify(ctx, Tone::Done, self.status.clone());
            return;
        }
        // Create next to an SHA file wrote `{folder name}.md5`; that one applies from now on, the
        // SHA file stays where it was, unchanged.
        if outcome.recorded.is_some() {
            let target = self.md5_target();
            if target != self.md5_name {
                self.md5_name = target;
                self.pinned_md5 = None;
            }
            self.md5_recorded = None;
        }
        // The run wrote exactly the checksum file that applied (`md5_file`); read nothing again. It
        // has just written the history; without it (read-only) the old one stays.
        if let Some(history) = outcome.history {
            let name = checksum::names_for(&root, &self.md5_name).json;
            self.timeline = Timeline::new(name, history).map_err(Say::stored);
        }
        // The next Create carries over what this run read.
        self.known = outcome.known.map(Arc::new);
        let reused =
            (outcome.reused > 0).then_some((outcome.reused as usize, outcome.read as usize));
        if let Some(recorded) = outcome.recorded {
            self.md5_exists = true;
            self.recorded = recorded
                .iter()
                .map(|path| path_key(path, FOLD_CASE))
                .collect();
        }
        let name = self.md5_name.clone();
        let summary = outcome.summary;
        // Before adopting: when creating, all rows become "open", and the finding is then only in
        // the summary.
        let damaged_files = summary.damaged;
        self.filter_note = outcome.report.warnings.clone();
        self.rows = Rows::from(match outcome.kind {
            JobKind::Generate => recorded_after_generate(outcome.rows),
            JobKind::Verify => outcome.rows,
        });
        include_own_rows(&root, &name, &mut self.rows);
        self.from_run = true;
        self.selected = None;
        self.status_filter = None;
        self.list_offset = Some(0.0);
        let files = self
            .rows
            .iter()
            .filter(|row| row.kind == RowKind::Recorded)
            .count();
        let errors = summary.errors as usize;
        // The message is only built when shown, so that it follows a language change.
        let unreadable = move |t: &Texts| (errors > 0).then(|| (t.unreadable_files)(errors));
        let (tone, message) = match outcome.kind {
            JobKind::Generate if outcome.baseline => (
                if errors > 0 { Tone::Warn } else { Tone::Done },
                Say::new(move |t| sentences([Some((t.written)(&name, files)), unreadable(t)])),
            ),
            JobKind::Generate => {
                // The same words as the history card for a replace run.
                let changes = [
                    (
                        summary.modified.saturating_sub(damaged_files),
                        Change::Changed,
                    ),
                    (damaged_files, Change::Damaged),
                    (summary.added, Change::Added),
                    (summary.removed, Change::Dropped),
                ];
                let tone = if errors > 0 || damaged_files > 0 {
                    Tone::Warn
                } else {
                    Tone::Done
                };
                (
                    tone,
                    Say::new(move |t| {
                        let parts = i18n::count_parts_in(t, &changes);
                        let diff = if parts.is_empty() {
                            t.unchanged_since.to_string()
                        } else {
                            (t.changed_since)(&parts)
                        };
                        sentences([
                            Some((t.replaced)(&name, files)),
                            reused.map(|(reused, read)| (t.reused_read)(reused, read)),
                            Some(diff),
                            (damaged_files > 0).then(|| t.damage_kept.to_string()),
                            unreadable(t),
                        ])
                    }),
                )
            }
            JobKind::Verify => {
                let counts = *self.rows.counts();
                // The same words as the labels of the list, counted (`Texts::counted`).
                let found: Vec<(u64, Change)> = [
                    (RowMark::Wrong, Change::Differs),
                    (RowMark::Damaged, Change::Damaged),
                    (RowMark::Deleted, Change::Missing),
                    (RowMark::New, Change::New),
                    (RowMark::Error, Change::Unreadable),
                ]
                .into_iter()
                .map(|(mark, change)| (counts.get(mark) as u64, change))
                .collect();
                let ok = counts.get(RowMark::Ok);
                if found.iter().all(|(count, _)| *count == 0) {
                    (
                        Tone::Done,
                        Say::new(move |t| {
                            if ok == 1 {
                                (t.verified_one)(&name)
                            } else {
                                (t.verified_all)(&name, ok)
                            }
                        }),
                    )
                } else {
                    (
                        Tone::Warn,
                        Say::new(move |t| {
                            (t.verified_findings)(&name, &i18n::count_parts_in(t, &found))
                        }),
                    )
                }
            }
        };
        self.status = message.clone();
        self.notify(ctx, tone, message);
        if tone == Tone::Warn {
            if let Some(notice) = &mut self.notice {
                notice.sticky = true;
            }
        }
    }

    /// Asks the run to cancel. If it is already writing, that no longer works: it finishes rather
    /// than ending between checksum file and history.
    fn cancel_scan(&mut self) {
        self.cancel_armed = None;
        if let Some(running) = &self.running {
            self.status = if running.progress.request_cancel(&running.cancel) {
                Say::of(|t| t.cancelling)
            } else {
                Say::of(|t| t.writing)
            };
        }
    }

    /// The run is writing checksum file and history; cancelling is no longer possible.
    fn writing(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| running.progress.is_writing())
    }

    /// On closing: if the run is already writing, wait for it at most `wait`, so that it does not
    /// end between the two files. Otherwise the cancel signal is enough.
    fn stop_for_exit(&mut self, wait: Duration) {
        let Some(running) = &self.running else {
            return;
        };
        if running.progress.request_cancel(&running.cancel) {
            return;
        }
        log::info!("Closing waits until the run has written");
        if running.rx.recv_timeout(wait).is_err() {
            log::warn!("The run was still writing on closing");
        }
    }

    /// Whether the first Esc in a run still counts.
    fn armed(&self, now: f64) -> bool {
        self.cancel_armed
            .is_some_and(|since| now - since <= ARM_SECS)
    }

    /// Closing during a run asks first (dialog card); without a run the window closes at once.
    /// After "Cancel and close" the next request goes through.
    fn handle_close(&mut self, ctx: &egui::Context) {
        if self.running.is_none() {
            // The run has finished in the meantime: nothing left to cancel.
            self.close_ask = false;
            return;
        }
        if self.close_confirmed || !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.close_ask = true;
        ctx.memory_mut(|mem| mem.stop_text_input());
    }

    fn decide_close(&mut self, ctx: &egui::Context, close: bool) {
        self.close_ask = false;
        if close {
            self.cancel_scan();
            self.close_confirmed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let key = |mods: Modifiers, key: Key| ctx.input_mut(|input| input.consume_key(mods, key));
        if self.close_ask {
            if key(Modifiers::NONE, Key::Enter) {
                self.decide_close(ctx, true);
            } else if key(Modifiers::NONE, Key::Escape) {
                self.decide_close(ctx, false);
            }
            return;
        }
        if self.confirm.is_some() {
            if key(Modifiers::NONE, Key::Enter) {
                self.decide(ctx, true);
            } else if key(Modifiers::NONE, Key::Escape) {
                self.decide(ctx, false);
            }
            return;
        }
        if self.delete.is_some() {
            if key(Modifiers::NONE, Key::Enter) {
                self.decide_delete(ctx, true);
            } else if key(Modifiers::NONE, Key::Escape) {
                self.decide_delete(ctx, false);
            }
            return;
        }
        if self.history_view.is_some() {
            self.history_keys(ctx);
            return;
        }
        if self.about_open {
            if key(Modifiers::NONE, Key::Escape) || key(Modifiers::NONE, Key::F1) {
                self.about_open = false;
            }
            return;
        }
        if key(Modifiers::NONE, Key::F1) {
            self.open_about();
            return;
        }
        if key(Modifiers::COMMAND, Key::H) {
            ctx.memory_mut(|mem| mem.stop_text_input());
            self.open_history(ctx);
            return;
        }
        // Ctrl+L: the next language, as in Cerno.
        if key(Modifiers::COMMAND, Key::L) {
            self.set_language(i18n::current().next());
            return;
        }
        // Ctrl+, (otherwise "Settings") sets the focus into the *Threads* field in the bar.
        if key(Modifiers::COMMAND, Key::Comma) {
            if self.running.is_some() {
                self.notify(ctx, Tone::Warn, Say::of(|t| t.locked));
            } else {
                ctx.memory_mut(|mem| mem.request_focus(threads_id()));
            }
            return;
        }
        if key(Modifiers::COMMAND, Key::F) {
            ctx.memory_mut(|mem| mem.request_focus(search_id()));
        }
        if key(Modifiers::COMMAND, Key::Enter) {
            self.request(ctx, JobKind::Generate);
        }
        if key(Modifiers::NONE, Key::F5) {
            self.request(ctx, JobKind::Verify);
        }
        // Shift+F8 before F8: a pattern without Shift also matches with Shift held.
        if key(Modifiers::SHIFT, Key::F8) {
            self.next_finding(ctx, false);
        } else if key(Modifiers::NONE, Key::F8) {
            self.next_finding(ctx, true);
        }
        if self.root.is_some() {
            for (index, digit) in DIGITS.iter().enumerate().take(CHIP_MARKS.len()) {
                if key(Modifiers::ALT, *digit) {
                    self.toggle_chip(ctx, CHIP_MARKS[index]);
                }
            }
        } else if !ctx.input(|input| input.modifiers.alt) && !ctx.egui_wants_keyboard_input() {
            // Empty state: Enter opens the last folder, 2 and 3 the ones before.
            let recent = self.recent.len();
            if recent > 0 && key(Modifiers::NONE, Key::Enter) {
                self.open_recent(ctx, 0);
                return;
            }
            for (index, digit) in DIGITS.iter().enumerate().take(recent).skip(1) {
                if key(Modifiers::NONE, *digit) {
                    self.open_recent(ctx, index);
                    return;
                }
            }
        }
        // In the search field Esc, arrows, Home and End belong to the field.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        // egui closes an open context menu itself with Esc; consumed here, it would never get
        // there.
        if !egui::Popup::is_any_open(ctx) && key(Modifiers::NONE, Key::Escape) {
            self.step_back(ctx.input(|input| input.time));
        }
        if key(Modifiers::NONE, Key::Home) {
            self.jump_list(true);
        } else if key(Modifiers::NONE, Key::End) {
            self.jump_list(false);
        }
        match self.pane {
            Pane::Tree => self.tree_keys(ctx),
            Pane::List => self.list_keys(ctx),
        }
    }

    /// In the history card: ↑/↓ selects the run, ←/→ the tab, Esc and Ctrl+H close.
    fn history_keys(&mut self, ctx: &egui::Context) {
        let key = |mods: Modifiers, key: Key| ctx.input_mut(|input| input.consume_key(mods, key));
        if key(Modifiers::NONE, Key::Escape) || key(Modifiers::COMMAND, Key::H) {
            self.history_view = None;
            return;
        }
        let runs = self
            .timeline
            .as_ref()
            .map(|timeline| timeline.runs().len())
            .unwrap_or(0);
        let Some(view) = &mut self.history_view else {
            return;
        };
        // ↑/↓ apply only to the runs; on a file's tab they do not switch.
        if view.tab == history::Tab::Runs && key(Modifiers::NONE, Key::ArrowUp) {
            view.step(true, runs);
        } else if view.tab == history::Tab::Runs && key(Modifiers::NONE, Key::ArrowDown) {
            view.step(false, runs);
        } else if key(Modifiers::NONE, Key::ArrowLeft)
            || key(Modifiers::NONE, Key::ArrowRight)
            || key(Modifiers::COMMAND, Key::Tab)
        {
            view.switch();
        }
    }

    /// Esc undoes one level per press (R7): error, filter, search, selection. Only when nothing is
    /// left to undo does Esc cancel a run – and even then only on the second press, so that one Esc
    /// too many does not discard hours of work.
    fn step_back(&mut self, now: f64) {
        if self.notice.as_ref().is_some_and(Notice::stays) {
            self.notice = None;
        } else if self.status_filter.is_some() {
            self.status_filter = None;
        } else if !self.query.is_empty() {
            self.query.clear();
        } else if self.selected.is_some() {
            self.selected = None;
        } else if self.running.is_some() && !self.writing() {
            if self.armed(now) {
                self.cancel_scan();
            } else {
                self.cancel_armed = Some(now);
            }
        }
    }

    fn tree_keys(&mut self, ctx: &egui::Context) {
        let key = |key: Key| ctx.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        let (up, down, left, right, enter) = (
            key(Key::ArrowUp),
            key(Key::ArrowDown),
            key(Key::ArrowLeft),
            key(Key::ArrowRight),
            key(Key::Enter),
        );
        if !(up || down || left || right || enter) {
            return;
        }
        if enter {
            self.pane = Pane::List;
            if self.selected.is_none() {
                self.move_selection(1);
            }
            return;
        }
        let items = self.tree.items();
        if items.is_empty() {
            return;
        }
        let current = self
            .root
            .as_ref()
            .and_then(|root| items.iter().position(|item| &item.path == root));
        let target = match (current, up, down, left, right) {
            (None, ..) => Some(0),
            (Some(index), true, ..) => Some(index.saturating_sub(1)),
            (Some(index), _, true, ..) => Some((index + 1).min(items.len() - 1)),
            (Some(index), _, _, true, _) => {
                let item = &items[index];
                if item.expanded {
                    self.tree.collapse(&item.path);
                    None
                } else {
                    item.path
                        .parent()
                        .and_then(|parent| items.iter().position(|other| other.path == parent))
                }
            }
            (Some(index), _, _, _, true) => {
                let item = &items[index];
                if item.expanded {
                    items
                        .get(index + 1)
                        .filter(|child| child.depth > item.depth)
                        .map(|_| index + 1)
                } else {
                    self.tree.expand(&item.path, true);
                    None
                }
            }
            _ => None,
        };
        if let Some(path) = target
            .and_then(|index| items.get(index))
            .map(|item| item.path.clone())
        {
            if self.root.as_ref() != Some(&path) {
                self.select_folder(ctx, path, false);
            }
        }
    }

    fn list_keys(&mut self, ctx: &egui::Context) {
        let key = |key: Key| ctx.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        // One page: as many rows as are visible, at least one.
        let page = ((self.list_view.1 / ROW).floor() as isize - 1).max(1);
        if key(Key::ArrowUp) {
            self.move_selection(-1);
        } else if key(Key::ArrowDown) {
            self.move_selection(1);
        } else if key(Key::PageUp) {
            self.move_selection(-page);
        } else if key(Key::PageDown) {
            self.move_selection(page);
        } else if key(Key::ArrowLeft) {
            self.pane = Pane::Tree;
        } else if key(Key::Enter) {
            if let Some(path) = self.selected.clone() {
                self.open_row(ctx, &path);
            }
        } else if key(Key::Delete) {
            self.request_delete(ctx);
        }
    }

    /// Opens one of the recent folders. If it no longer exists, a notice says so and it drops out
    /// of the list. The arrow keys go to the list only once it has been opened.
    fn open_recent(&mut self, ctx: &egui::Context, index: usize) {
        let Some(path) = self.recent.get(index).cloned() else {
            return;
        };
        if !path.is_dir() {
            self.recent.retain(|known| known != &path);
            let shown = path.display().to_string();
            self.notify(ctx, Tone::Warn, Say::new(move |t| (t.recent_gone)(&shown)));
            return;
        }
        self.open_folder(ctx, path.clone(), true, None);
        if self.root.as_ref() == Some(&path) {
            self.pane = Pane::List;
        }
    }

    /// Filter chip by click or Alt+digit: turns on if files with this status exist, otherwise off.
    /// The selection stays as long as the filter shows it.
    fn toggle_chip(&mut self, ctx: &egui::Context, mark: RowMark) {
        if self.status_filter == Some(mark) {
            self.status_filter = None;
        } else if self.rows.counts().get(mark) > 0 {
            self.status_filter = Some(mark);
            let still_visible = self
                .selected
                .as_ref()
                .is_some_and(|path| self.rows.find(path).and_then(FileRow::mark) == Some(mark));
            if !still_visible {
                self.selected = None;
            }
        } else {
            self.notify(
                ctx,
                Tone::Warn,
                Say::new(move |t| format!("{}.", (t.chip_none)(t.mark(mark)))),
            );
            return;
        }
        self.list_offset = Some(0.0);
    }

    /// F8: the next file with a finding (mismatch, damaged, deleted, error) in the list, wrapping
    /// back to the start at the end; Shift+F8 goes backwards.
    fn next_finding(&mut self, ctx: &egui::Context, forward: bool) {
        let current = self.selected_position();
        let listed = self.listed.get(&self.rows, self.status_filter, &self.query);
        let findings: Vec<usize> = listed
            .iter()
            .enumerate()
            .filter(|(_, at)| self.rows[**at].mark().is_some_and(is_finding))
            .map(|(index, _)| index)
            .collect();
        if findings.is_empty() {
            self.notify(ctx, Tone::Done, Say::of(|t| t.no_findings));
            return;
        }
        let target = match (current, forward) {
            (None, true) => findings[0],
            (None, false) => findings[findings.len() - 1],
            (Some(at), true) => findings
                .iter()
                .copied()
                .find(|&index| index > at)
                .unwrap_or(findings[0]),
            (Some(at), false) => findings
                .iter()
                .copied()
                .rev()
                .find(|&index| index < at)
                .unwrap_or(findings[findings.len() - 1]),
        };
        self.selected = Some(self.rows[listed[target]].path.clone());
        self.pane = Pane::List;
        self.ensure_visible(target);
    }

    /// Where the selected row stands among the visible ones.
    fn selected_position(&mut self) -> Option<usize> {
        let at = self.rows.position(self.selected.as_deref()?)?;
        self.listed
            .get(&self.rows, self.status_filter, &self.query)
            .binary_search(&at)
            .ok()
    }

    fn move_selection(&mut self, step: isize) {
        let current = self.selected_position();
        let listed = self.listed.get(&self.rows, self.status_filter, &self.query);
        if listed.is_empty() {
            return;
        }
        let index = match current {
            None if step < 0 => listed.len() - 1,
            None => 0,
            Some(index) => index.saturating_add_signed(step).min(listed.len() - 1),
        };
        self.selected = Some(self.rows[listed[index]].path.clone());
        self.ensure_visible(index);
    }

    fn ensure_visible(&mut self, index: usize) {
        let (offset, height) = self.list_view;
        let top = index as f32 * ROW;
        let bottom = top + ROW;
        if top < offset {
            self.list_offset = Some(top);
        } else if bottom > offset + height {
            self.list_offset = Some(bottom - height);
        }
    }

    fn jump_list(&mut self, to_start: bool) {
        let listed = self.listed.get(&self.rows, self.status_filter, &self.query);
        let at = if to_start {
            listed.first()
        } else {
            listed.last()
        };
        if let Some(&at) = at {
            let end = listed.len() as f32 * ROW;
            self.selected = Some(self.rows[at].path.clone());
            self.pane = Pane::List;
            self.list_offset = Some(if to_start { 0.0 } else { end });
        }
    }

    /// Opens the file in the program that the system provides for it; the "History" row opens the
    /// history card.
    fn open_row(&mut self, ctx: &egui::Context, path: &str) {
        let Some(row) = self.rows.find(path) else {
            return;
        };
        if row.kind == RowKind::Report && self.timeline.is_ok() {
            self.open_history(ctx);
            return;
        }
        let absolute = row
            .absolute
            .clone()
            .filter(|_| row.kind != RowKind::Removed);
        let path = path.to_string();
        let Some(absolute) = absolute else {
            self.notify(ctx, Tone::Warn, Say::new(move |t| (t.open_gone)(&path)));
            return;
        };
        // Only a regular file that really lies in the folder.
        if let Some(reason) = self.outside_root(&absolute) {
            self.notify(
                ctx,
                Tone::Warn,
                Say::new(move |t| (t.open_refused)(&path, &i18n::shown_in(t, &reason))),
            );
            return;
        }
        if let Err(err) = open::that_detached(&absolute) {
            let why = plain_io(&err);
            self.notify(
                ctx,
                Tone::Error,
                Say::new(move |t| (t.open_failed)(&path, &why)),
            );
        }
    }

    /// Why `absolute` is not a regular file in the chosen folder, or `None` – as text as stored
    /// (`Msg`, `i18n::shown`).
    fn outside_root(&self, absolute: &Path) -> Option<String> {
        match &self.root {
            Some(root) => contained_file(root, absolute).err(),
            None => Some(NO_FOLDER.into()),
        }
    }

    /// Whether the history card works right now, and otherwise why not (R2).
    fn can_show_history(&self) -> Result<(), Say> {
        // Also during a run: the card reads only the history from when the folder was opened.
        self.timeline.as_ref().map(|_| ()).map_err(Clone::clone)
    }

    /// Opens the history card; if a file is selected, on its tab.
    fn open_history(&mut self, ctx: &egui::Context) {
        if let Err(reason) = self.can_show_history() {
            self.notify(ctx, Tone::Warn, reason);
            return;
        }
        let file = self
            .selected
            .as_ref()
            .and_then(|path| self.rows.find(path))
            .filter(|row| !matches!(row.kind, RowKind::Checksum | RowKind::Report))
            .map(|row| row.path.clone());
        let runs = self
            .timeline
            .as_ref()
            .map(|timeline| timeline.runs().len())
            .unwrap_or(0);
        self.about_open = false;
        self.history_view = Some(history::View::new(runs, file));
    }

    /// Asks whether the selected file should go to the Recycle Bin – always, a deliberate departure
    /// from R5. For Veritas' own files, missing files, during a run and where there is no Recycle
    /// Bin, a notice says why not.
    fn request_delete(&mut self, ctx: &egui::Context) {
        let Some(path) = self.selected.clone() else {
            return;
        };
        let Some(row) = self.rows.find(&path) else {
            return;
        };
        let absolute = row
            .absolute
            .clone()
            .filter(|_| !matches!(row.kind, RowKind::Removed | RowKind::Partial));
        let refused = if self.running.is_some() {
            Some(Say::of(|t| t.delete_running))
        } else if list::protected(row) {
            Some(Say::of(|t| t.delete_own))
        } else if let Some(absolute) = &absolute {
            // Only regular files in the folder; the Recycle Bin test uses the canonical path, so
            // that a link to a network drive does not slip through.
            match self
                .root
                .as_ref()
                .map(|root| contained_file(root, absolute))
            {
                Some(Ok(canon)) => no_trash(&canon),
                Some(Err(reason)) => {
                    let path = path.clone();
                    Some(Say::new(move |t| {
                        (t.delete_refused)(&path, &i18n::shown_in(t, &reason))
                    }))
                }
                None => Some(Say::stored(NO_FOLDER)),
            }
        } else {
            let path = path.clone();
            Some(Say::new(move |t| (t.delete_gone)(&path)))
        };
        if let Some(reason) = refused {
            self.notify(ctx, Tone::Warn, reason);
            return;
        }
        let Some(absolute) = absolute else {
            return;
        };
        let recorded = self.recorded.contains(&path_key(&path, FOLD_CASE));
        ctx.memory_mut(|mem| mem.stop_text_input());
        self.delete = Some(Delete {
            path,
            absolute,
            recorded,
        });
    }

    fn decide_delete(&mut self, ctx: &egui::Context, confirmed: bool) {
        let Some(delete) = self.delete.take() else {
            return;
        };
        let path = delete.path.clone();
        if !confirmed {
            self.status = Say::new(move |t| (t.kept)(&path));
            return;
        }
        // In a run nothing changes the rows except the interim state (`Live` remembers numbers).
        if self.running.is_some() {
            self.notify(ctx, Tone::Warn, Say::of(|t| t.delete_running));
            return;
        }
        // Once more directly before: between confirmation and Enter the folder may change.
        if let Some(reason) = self.outside_root(&delete.absolute) {
            self.notify(
                ctx,
                Tone::Warn,
                Say::new(move |t| (t.delete_refused)(&path, &i18n::shown_in(t, &reason))),
            );
            return;
        }
        if let Err(err) = (self.trasher)(&delete.absolute) {
            self.notify(
                ctx,
                Tone::Error,
                Say::new(move |t| (t.trash_failed)(&path, &err)),
            );
            return;
        }
        // The selection moves to the next visible row, as in a file manager – even if the row stays
        // but the filter no longer shows it as deleted.
        let gone = self.rows.position(&delete.path);
        let listed = self.listed.get(&self.rows, self.status_filter, &self.query);
        let at = gone.and_then(|gone| listed.binary_search(&gone).ok());
        let next = at.and_then(|at| {
            listed
                .get(at + 1)
                .or_else(|| at.checked_sub(1).and_then(|before| listed.get(before)))
                .map(|&row| self.rows[row].path.clone())
        });
        if delete.recorded {
            // As when opening: if it is in the .md5 and no longer exists, it is deleted.
            let at = self.rows.position(&delete.path);
            if let Some(row) = at.and_then(|at| self.rows.get_mut(at)) {
                if row.old_hash.shown().is_none() {
                    row.old_hash = row.new_hash.clone();
                }
                row.kind = RowKind::Removed;
                row.absolute = None;
                row.size = None;
                row.modified = None;
                row.integrity = None;
                row.message = None;
            }
        } else {
            self.rows.retain(|row| row.path != delete.path);
        }
        if self.selected_position().is_none() {
            self.selected = next;
        }
        let text = Say::new(move |t| (t.trashed)(&path));
        self.status = text.clone();
        self.notify(ctx, Tone::Done, text);
    }
}

impl eframe::App for VeritasApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
        let ctx = ui.ctx().clone();
        self.drive_shot(&ctx);
    }

    /// In screenshot mode only the set state counts: the user's pointer, keys and dropped files do
    /// not get into the picture.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if self.shot.is_some() {
            raw_input.events.clear();
            raw_input.hovered_files.clear();
            raw_input.dropped_files.clear();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Screenshot mode changes none of the user's settings.
        if self.shot.is_some() {
            return;
        }
        eframe::set_value(storage, SETTINGS_KEY, &self.settings());
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stop_for_exit(EXIT_WAIT);
    }
}

impl VeritasApp {
    /// One frame: input, bars, list and what lies over them. Without `eframe::Frame`, so that tests
    /// can call it with a bare egui context (`Context::run_ui`).
    fn show(&mut self, root: &mut egui::Ui) {
        let ctx = &root.ctx().clone();
        self.refresh_roots(ctx);
        self.update_drops(ctx);
        self.poll_job(ctx);
        self.poll_preview(ctx);
        self.handle_close(ctx);
        self.handle_keys(ctx);

        egui::Panel::top("werkzeug")
            .exact_size(theme::TOOLBAR)
            .frame(theme::bar_frame(
                ctx,
                Margin::symmetric(space::S3 as i8, space::S1 as i8),
            ))
            .show(root, |ui| self.draw_toolbar(ui));

        egui::Panel::bottom("status")
            .exact_size(theme::STATUS_BAR)
            .frame(theme::bar_frame(ctx, Margin::symmetric(space::S3 as i8, 2)))
            .show(root, |ui| self.draw_status(ui));

        egui::Panel::left("baum")
            .exact_size(TREE_W)
            .resizable(false)
            .frame(theme::bar_frame(
                ctx,
                Margin::symmetric(space::S2 as i8, space::S2 as i8),
            ))
            .show(root, |ui| self.draw_tree(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(palette(ctx).list)
                    .inner_margin(Margin {
                        left: space::S2 as i8,
                        right: 0,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(root, |ui| self.draw_list(ui));

        self.draw_progress(ctx);
        self.draw_notice(ctx);
        self.draw_confirm(ctx);
        self.draw_delete(ctx);
        self.draw_history(ctx);
        self.draw_about(ctx);
        self.draw_close(ctx);
    }

    /// Switches the UI language; `save` remembers it.
    fn set_language(&mut self, lang: Lang) {
        i18n::set(lang);
    }

    /// What `save` stores.
    fn settings(&self) -> Settings {
        Settings {
            recursive: self.recursive,
            threads: self.threads,
            crash_seen: self.crash_seen,
            recent: self.recent.clone(),
            language: Some(i18n::current().code().to_string()),
        }
    }

    fn open_about(&mut self) {
        self.history_view = None;
        self.about_open = true;
    }

    fn draw_about(&mut self, ctx: &egui::Context) {
        if self.about_open && about::overlay(ctx) {
            self.about_open = false;
        }
    }

    fn draw_history(&mut self, ctx: &egui::Context) {
        if self.history_view.is_none() {
            return;
        }
        if self.timeline.is_err() {
            self.history_view = None;
            return;
        }
        let (Some(view), Ok(timeline)) = (self.history_view.as_mut(), self.timeline.as_ref())
        else {
            return;
        };
        let out = history::overlay(ctx, timeline, view);
        if out.close {
            self.history_view = None;
        }
        if out.open_file {
            self.open_history_file(ctx);
        }
    }

    /// "Open as JSON": the history in the program the system provides for `.json`.
    fn open_history_file(&mut self, ctx: &egui::Context) {
        let file = match (&self.root, &self.timeline) {
            (Some(root), Ok(timeline)) => root.join(&timeline.file_name),
            _ => return,
        };
        // Like Open in the list: only a regular file in the folder, no link.
        let shown = file.display().to_string();
        if let Some(reason) = self.outside_root(&file) {
            self.notify(
                ctx,
                Tone::Warn,
                Say::new(move |t| (t.open_refused)(&shown, &i18n::shown_in(t, &reason))),
            );
        } else if let Err(err) = open::that_detached(&file) {
            let why = plain_io(&err);
            self.notify(
                ctx,
                Tone::Error,
                Say::new(move |t| (t.open_json_failed)(&shown, &why)),
            );
        }
    }

    /// The bar, from the left: Verify, Create MD5 (primary action), checksum file │ Theme │
    /// Subfolders, Threads │ Search, "About Veritas". The settings are right here, without a popup.
    /// Everything on the right has a fixed width; only the name of the checksum file shortens (R1).
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let p = palette(&ctx);
        let t = i18n::t();
        let verify = self.can_start(JobKind::Verify);
        let generate = self.can_start(JobKind::Generate);
        let idle = self.running.is_none();
        let mut wanted = None;
        let mut reload = false;
        self.bar_rects.clear();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(space::S2, 0.0);
            let info = theme::icon_button(ui, icon::INFO, CONTROL).on_hover_text(t.about_tip);
            if info.clicked() {
                self.open_about();
            }
            let language = self.draw_language(ui);
            let search = theme::text_field(
                ui,
                search_id(),
                &mut self.query,
                t.search_hint,
                SEARCH_W,
                Some(icon::SEARCH),
            )
            .on_hover_text(t.search_tip);
            theme::bar_separator(ui);
            let threads = self.draw_threads(ui, idle);
            let response = ui
                .add_enabled_ui(idle, |ui| {
                    theme::toggle_icon_button(ui, icon::FOLDER_TREE, CONTROL, self.recursive)
                })
                .inner;
            let tip = if self.recursive {
                t.subfolders_on
            } else {
                t.subfolders_off
            };
            let subfolders = response
                .on_hover_text(tip)
                .on_disabled_hover_text(format!("{tip}\n{}", t.locked));
            if subfolders.clicked() {
                self.recursive = !self.recursive;
                reload = true;
            }
            theme::bar_separator(ui);
            let mut pref = ctx.options(|opt| opt.theme_preference);
            let theme_rect = ui
                .scope(|ui| {
                    if theme::segments(ui, "thema", &mut pref, &theme_choices(t)) {
                        ctx.set_theme(pref);
                    }
                })
                .response
                .rect;
            theme::bar_separator(ui);
            self.bar_rects.extend([
                ("Über", info.rect),
                ("Sprache", language),
                ("Suche", search.rect),
                ("Threads", threads),
                ("Unterordner", subfolders.rect),
                ("Thema", theme_rect),
            ]);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(space::S2, 0.0);
                // Button row: the primary action sits at the far right (§8).
                let response = ui
                    .add_enabled_ui(verify.is_ok(), |ui| {
                        theme::button(
                            ui,
                            Role::Secondary,
                            Some(icon::SHIELD_CHECK),
                            t.verify,
                            0.0,
                            CONTROL,
                        )
                    })
                    .inner;
                let response = match &verify {
                    Ok(()) => response.on_hover_text((t.verify_tip)(&self.md5_name)),
                    Err(reason) => response.on_disabled_hover_text(reason.get()),
                };
                if response.clicked() {
                    wanted = Some(JobKind::Verify);
                }
                self.bar_rects.push(("Prüfen", response.rect));
                let response = ui
                    .add_enabled_ui(generate.is_ok(), |ui| {
                        theme::button(
                            ui,
                            Role::Primary,
                            Some(icon::FILE_PLUS),
                            t.generate,
                            0.0,
                            CONTROL,
                        )
                    })
                    .inner;
                let response = match &generate {
                    Ok(()) => response.on_hover_text((t.generate_tip)(&self.md5_target())),
                    Err(reason) => response.on_disabled_hover_text(reason.get()),
                };
                if response.clicked() {
                    wanted = Some(JobKind::Generate);
                }
                self.bar_rects.push(("MD5 erzeugen", response.rect));
                if !self.md5_name.is_empty() {
                    ui.add_space(space::S2);
                    let color = if self.md5_exists { p.text } else { p.muted };
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::splat(theme::ICON), Sense::hover());
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        icon::FILE_CHECK.to_string(),
                        FontId::new(theme::ICON, FontFamily::Proportional),
                        p.muted,
                    );
                    let tip = if self.md5_exists {
                        t.md5_here
                    } else {
                        t.md5_missing
                    };
                    let name = ui
                        .add(
                            Label::new(
                                RichText::new(&self.md5_name)
                                    .size(text::BODY)
                                    .family(theme::semibold())
                                    .color(color),
                            )
                            .truncate(),
                        )
                        .on_hover_text(tip);
                    self.bar_rects.push(("Prüfsummendatei", name.rect));
                }
            });
        });
        if let Some(kind) = wanted {
            self.request(&ctx, kind);
        }
        if reload {
            self.subfolders_changed(&ctx);
        }
    }

    /// The language button left of "About": a click opens the list of the five languages, each in
    /// itself, the chosen one ticked (§8 menu); `Ctrl+L` switches to the next, as in Cerno. Returns
    /// the area it occupies.
    fn draw_language(&mut self, ui: &mut egui::Ui) -> Rect {
        let t = i18n::t();
        let current = i18n::current();
        let button = theme::icon_button(ui, icon::LANGUAGES, CONTROL)
            .on_hover_text((t.language_tip)(current.name()));
        let mut picked = None;
        // As wide as the content, not `MENU_W`: five short names in 240 px looked empty.
        let width = theme::choice_width(ui, Lang::ALL.iter().map(|lang| lang.name()));
        egui::Popup::menu(&button).width(width).show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for lang in Lang::ALL {
                if theme::choice_row(ui, lang.name(), lang == current, width).clicked() {
                    picked = Some(lang);
                }
            }
        });
        if let Some(lang) = picked {
            self.set_language(lang);
        }
        button.rect
    }

    /// *Threads* in the bar: icon and field, locked during a run (R2). If the folder is on an HDD,
    /// a hard disk icon stands in place of the processor, in the same spot (R1), and the tooltip
    /// says that the number does not apply there. Returns the area it occupies.
    fn draw_threads(&mut self, ui: &mut egui::Ui, idle: bool) -> Rect {
        let p = palette(ui.ctx());
        let t = i18n::t();
        let hdd = self.seeks == Some(true);
        let tip = if hdd {
            format!("{}\n{}", t.threads_tip, t.threads_hdd)
        } else {
            t.threads_tip.to_string()
        };
        let field = ui
            .add_enabled_ui(idle, |ui| {
                theme::number_field(ui, threads_id(), &mut self.threads, 1..=256, THREADS_W)
            })
            .inner
            .on_hover_text(tip.as_str())
            .on_disabled_hover_text(format!("{tip}\n{}", t.locked));
        ui.add_space(-space::S1);
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(theme::ICON_BAR), Sense::hover());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            if hdd { icon::HARD_DRIVE } else { icon::CPU }.to_string(),
            FontId::new(theme::ICON_BAR, FontFamily::Proportional),
            if idle { p.muted } else { p.disabled },
        );
        response.on_hover_text(tip);
        field.rect.union(rect)
    }

    fn draw_status(&mut self, ui: &mut egui::Ui) {
        let p = palette(ui.ctx());
        let t = i18n::t();
        let counts = *self.rows.counts();
        let total = self.rows.len().max(10);
        let sample = i18n::count(total);
        let filter = self.status_filter;
        let mut clicked = None;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(space::S2, 0.0);
            for (index, mark) in CHIP_MARKS.iter().enumerate().rev() {
                let keys = format!("Alt+{}", index + 1);
                let count = counts.get(*mark);
                let active = filter == Some(*mark);
                let label = t.mark(*mark);
                let response = ui
                    .add_enabled_ui(count > 0 || active, |ui| {
                        theme::filter_chip(
                            ui,
                            mark_tag(*mark, p).0,
                            &i18n::count(count),
                            &sample,
                            label,
                            active,
                        )
                    })
                    .inner;
                let response = if active {
                    response.on_hover_text((t.chip_all)(&keys))
                } else {
                    response
                        .on_hover_text((t.chip_only)(label, &keys))
                        .on_disabled_hover_text((t.chip_none)(label))
                };
                if response.clicked() {
                    clicked = Some(*mark);
                }
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let detail = self.has_detail();
                // With an explanation it replaces the tooltip of the shortened text, instead of
                // appearing below it.
                let line = ui.add(
                    Label::new(
                        RichText::new(self.status_line())
                            .size(text::SMALL)
                            .color(p.muted),
                    )
                    .truncate()
                    .show_tooltip_when_elided(!detail),
                );
                if detail {
                    // The explanation is only built when the tooltip appears.
                    theme::lazy_tip(line, || self.status_detail().unwrap_or_default());
                }
            });
        });
        if let Some(mark) = clicked {
            let ctx = ui.ctx().clone();
            self.toggle_chip(&ctx, mark);
        }
    }

    fn status_line(&self) -> String {
        let t = i18n::t();
        if let Some(path) = &self.selected {
            if let Some(row) = self.rows.find(path) {
                let mut parts = vec![row.path.clone()];
                if let Some(mark) = row.mark() {
                    parts.push(t.mark(mark).to_string());
                }
                if let Some(short) = finding_short(t, row) {
                    parts.push(short.into());
                }
                if let Some(message) = &row.message {
                    parts.push(i18n::shown(message));
                }
                match row.kind {
                    RowKind::Report => parts.push(t.row_history.into()),
                    RowKind::Checksum => parts.push(t.row_md5.into()),
                    _ => {}
                }
                // Before the hashes, so that shortening the status bar hits it last.
                if let Ok(timeline) = &self.timeline {
                    if !matches!(
                        row.kind,
                        RowKind::Checksum | RowKind::Report | RowKind::Partial
                    ) {
                        let recorded = matches!(
                            row.kind,
                            RowKind::Recorded
                                | RowKind::Unchanged
                                | RowKind::Modified
                                | RowKind::Removed
                        );
                        if let Some(hint) = timeline.hint(&row.path, recorded) {
                            parts.push(hint);
                        }
                    }
                }
                let old = row.old_hash.shown().cloned().unwrap_or_default();
                let new = row.new_hash.shown().cloned().unwrap_or_default();
                if row.kind == RowKind::Modified && !old.is_empty() && !new.is_empty() {
                    parts.push((t.md5_then_now)(self.algo().label(), &old, &new));
                } else {
                    let md5 = row_md5(row);
                    if !md5.is_empty() {
                        parts.push(md5);
                    }
                }
                return parts.join(" · ");
            }
        }
        let mut line = self.status.get();
        for note in &self.filter_note {
            line.push(' ');
            line.push_str(&i18n::shown(note));
        }
        line
    }

    /// Whether `status_detail` has anything to say, without building the text.
    fn has_detail(&self) -> bool {
        self.selected
            .as_deref()
            .and_then(|path| self.rows.find(path))
            .and_then(FileRow::mark)
            .is_some_and(|mark| !matches!(mark, RowMark::Source | RowMark::Report))
    }

    /// The whole explanation of the selected file for the status bar's tooltip: what the status
    /// means and what to do next, plus structure finding and checksums.
    fn status_detail(&self) -> Option<String> {
        let t = i18n::t();
        let path = self.selected.as_ref()?;
        let row = self.rows.find(path)?;
        let md5 = &self.md5_name;
        let mut lines = vec![row.path.clone()];
        let reason = row
            .integrity
            .as_ref()
            .map(|finding| i18n::shown(&finding.reason))
            .unwrap_or_default();
        match row.mark() {
            Some(RowMark::Wrong) => {
                let intact = row.integrity.as_ref().map(|finding| finding.status)
                    == Some(IntegrityStatus::Intact);
                lines.push(if intact {
                    t.detail_changed_intact.into()
                } else {
                    t.detail_changed_unchecked.into()
                });
                lines.push(t.restore_help.into());
                if !reason.is_empty() {
                    lines.push((t.structure)(&reason));
                }
            }
            Some(RowMark::Damaged) => {
                lines.push(t.detail_damaged.into());
                lines.push(t.restore_help.into());
                if !reason.is_empty() {
                    lines.push((t.structure)(&reason));
                }
            }
            Some(RowMark::Deleted) => lines.push((t.detail_deleted)(md5)),
            Some(RowMark::New) => lines.push((t.detail_new)(md5)),
            Some(RowMark::Open) => lines.push((t.detail_open)(md5)),
            Some(RowMark::Ok) => lines.push((t.detail_ok)(md5)),
            Some(RowMark::Error) => {
                if let Some(message) = &row.message {
                    lines.push(i18n::shown(message));
                }
                lines.push(t.detail_error.into());
            }
            Some(RowMark::Source | RowMark::Report) | None => return None,
        }
        let old = row.old_hash.shown().cloned().unwrap_or_default();
        let new = row.new_hash.shown().cloned().unwrap_or_default();
        if row.kind == RowKind::Modified && !old.is_empty() && !new.is_empty() {
            lines.push((t.md5_then_now_lines)(self.algo().label(), &old, &new));
        }
        Some(lines.join("\n"))
    }

    /// "Subfolders" toggled: reload – also after a run whose result then no longer fits the
    /// setting.
    fn subfolders_changed(&mut self, ctx: &egui::Context) {
        self.load_preview(ctx, 0.0);
    }

    fn draw_confirm(&mut self, ctx: &egui::Context) {
        let Some(confirm) = &self.confirm else {
            return;
        };
        let t = i18n::t();
        let title = (t.replace_title)(&confirm.name);
        let body = confirm_body(t, confirm);
        if let Some(replace) = theme::dialog(
            ctx,
            "veritas-dialog",
            &title,
            &body,
            (t.replace, Role::Danger),
            t.keep,
        ) {
            self.decide(ctx, replace);
        }
    }

    fn draw_close(&mut self, ctx: &egui::Context) {
        if !self.close_ask {
            return;
        }
        let folder = self
            .running
            .as_ref()
            .map(|running| tree_label(&running.root))
            .unwrap_or_default();
        // If the run is already writing, it still finishes; the window waits for it briefly.
        let t = i18n::t();
        let (title, body, yes) = if self.writing() {
            (
                t.writing,
                (t.close_writing_body)(&folder, &self.md5_target()),
                t.close,
            )
        } else {
            (
                t.close_title,
                (t.close_body)(&folder, &self.md5_target()),
                t.cancel_close,
            )
        };
        if let Some(close) = theme::dialog(
            ctx,
            "veritas-close",
            title,
            &body,
            (yes, Role::Primary),
            t.keep_running,
        ) {
            self.decide_close(ctx, close);
        }
    }

    fn draw_delete(&mut self, ctx: &egui::Context) {
        let Some(delete) = &self.delete else {
            return;
        };
        let name = delete
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&delete.path)
            .to_string();
        let t = i18n::t();
        let title = (t.trash_title)(&name);
        let body = delete_body(t, delete, &self.md5_name);
        if let Some(confirmed) = theme::dialog(
            ctx,
            "veritas-delete",
            &title,
            &body,
            // Undoable, so not a danger button.
            (t.to_trash, Role::Primary),
            t.keep,
        ) {
            self.decide_delete(ctx, confirmed);
        }
    }

    /// Progress as a pill above the list (§8): what is running, how much, how much longer.
    fn draw_progress(&mut self, ctx: &egui::Context) {
        let Some(running) = &self.running else {
            return;
        };
        let p = palette(ctx);
        let t = i18n::t();
        let snap = running.progress.snapshot();
        // If the run is writing, there is no cancelling any more.
        let writing = running.progress.is_writing();
        let cancelling = !writing && running.cancel.load(Ordering::SeqCst);
        let armed = !writing && self.armed(ctx.input(|input| input.time));
        // On the left is what is happening right now: while hashing the file, otherwise the phase.
        // After Esc the announcement or the cancel.
        let (lead, lead_color) = if writing {
            (t.writing.trim_end_matches('.').to_string(), p.text)
        } else if cancelling {
            (t.cancelling_lead.to_string(), p.text)
        } else if armed {
            (t.armed_lead.to_string(), p.text)
        } else if snap.current.is_empty() {
            (
                i18n::shown(&snap.phase).trim_end_matches('…').to_string(),
                p.text,
            )
        } else {
            (snap.current.clone(), p.muted)
        };
        let keep_end = !cancelling && !armed && !snap.current.is_empty();
        // Countdown border per §8 while a second Esc cancels.
        let border = if armed || cancelling {
            p.warn
        } else {
            p.accent
        };
        let fraction = snap.fraction();
        let area = self.content_rect;
        let pos = Pos2::new(
            area.center().x - PILL_W / 2.0,
            area.bottom() - PILL_H - space::S4,
        );
        let tip = if snap.current.is_empty() {
            progress_text(&snap)
        } else {
            format!("{}\n{}", progress_text(&snap), snap.current)
        };
        let mut cancel = false;
        // Under the scrim of a dialog card (`theme::scrim` lays itself over it).
        egui::Area::new(Id::unique("veritas-progress"))
            .order(Order::Middle)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                theme::raised_frame(ctx, p.surface_raised, border, radius::LG)
                    .inner_margin(Margin::symmetric(space::S4 as i8, space::S2 as i8))
                    .show(ui, |ui| {
                        let inner = Vec2::new(PILL_W - 2.0 * space::S4, PILL_H - 2.0 * space::S2);
                        ui.set_min_size(inner);
                        ui.set_max_size(inner);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(space::S3, 0.0);
                            let button = ui
                                .add_enabled_ui(!writing, |ui| {
                                    theme::button(ui, Role::Secondary, None, t.cancel, 0.0, COMPACT)
                                })
                                .inner
                                .on_hover_text(t.cancel_tip)
                                .on_disabled_hover_text(format!("{} {}", t.writing, t.cancel_late));
                            if button.clicked() {
                                cancel = true;
                            }
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), inner.y),
                                Sense::hover(),
                            );
                            response.on_hover_text(tip);
                            // Text on top, track below (§8: track 4 px).
                            let text_y = rect.top() + space::S2;
                            // After Esc the whole line belongs to the announcement or the cancel.
                            let slots = if armed || cancelling {
                                0.0
                            } else {
                                paint_progress_numbers(ui, rect.right(), text_y, &snap)
                            };
                            paint_fit(
                                ui,
                                Pos2::new(rect.left(), text_y),
                                &lead,
                                (rect.width() - slots - space::S3).max(8.0),
                                FontId::new(text::BODY, FontFamily::Proportional),
                                lead_color,
                                keep_end,
                            );
                            let track = Rect::from_min_size(
                                Pos2::new(rect.left(), rect.bottom() - theme::TRACK - 2.0),
                                Vec2::new(rect.width(), theme::TRACK),
                            );
                            ui.painter().rect_filled(
                                track,
                                CornerRadius::same(radius::PILL),
                                p.line,
                            );
                            ui.painter().rect_filled(
                                Rect::from_min_size(
                                    track.min,
                                    Vec2::new(track.width() * fraction, track.height()),
                                ),
                                CornerRadius::same(radius::PILL),
                                p.accent,
                            );
                        });
                    });
            });
        if cancel {
            self.cancel_scan();
        }
    }

    /// Notice at the top center above the list (§8, R6). A click closes it.
    fn draw_notice(&mut self, ctx: &egui::Context) {
        let Some(notice) = &self.notice else {
            return;
        };
        let now = ctx.input(|input| input.time);
        let opacity = notice.opacity(now);
        if opacity <= 0.0 {
            self.notice = None;
            return;
        }
        if let Some(hold) = notice.hold() {
            let left = (notice.shown + hold - now).max(0.0);
            if left > 0.0 {
                ctx.request_repaint_after(Duration::from_secs_f64(left));
            } else {
                ctx.request_repaint();
            }
        }
        let p = palette(ctx);
        let (glyph, glyph_color, fill, border) = match notice.tone {
            Tone::Done => (icon::CIRCLE_CHECK, p.ok, p.surface_raised, p.line),
            Tone::Warn => (icon::TRIANGLE_ALERT, p.warn, p.surface_raised, p.line),
            Tone::Error => (icon::CIRCLE_ALERT, p.error, p.error_bg, p.error),
        };
        let text = notice.text.get();
        // Errors stay; their text can be selected (e.g. the path to the crash log). Close with × or
        // Esc, not with a click on the text.
        let selectable = notice.tone == Tone::Error;
        let mut closed = false;
        let area = self.content_rect;
        let max_text = (area.width() - 96.0).clamp(160.0, NOTICE_W);
        let shown = egui::Area::new(Id::unique("veritas-notice"))
            .order(Order::Middle)
            .pivot(Align2::CENTER_TOP)
            .fixed_pos(Pos2::new(area.center().x, area.top() + ROW + space::S3))
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                theme::raised_frame(ctx, fill, border, radius::MD)
                    .inner_margin(Margin::symmetric(space::S3 as i8, space::S2 as i8))
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(space::S2, 0.0);
                            ui.label(
                                RichText::new(glyph.to_string())
                                    .size(theme::ICON)
                                    .color(glyph_color),
                            );
                            ui.vertical(|ui| {
                                ui.set_max_width(max_text);
                                ui.add(
                                    Label::new(RichText::new(&text).size(text::BODY).color(p.text))
                                        .wrap()
                                        .selectable(selectable),
                                );
                            });
                            if selectable
                                && theme::icon_button(ui, icon::X, theme::COMPACT)
                                    .on_hover_text(i18n::t().close_esc)
                                    .clicked()
                            {
                                closed = true;
                            }
                        });
                    });
            });
        if !selectable {
            let tip = if notice.stays() {
                i18n::t().close_esc
            } else {
                i18n::t().close
            };
            let response = shown.response.interact(Sense::CLICK).on_hover_text(tip);
            closed |= response.clicked();
        }
        if closed {
            self.notice = None;
        }
    }

    fn draw_tree(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let items = self.tree.items();
        let selected = self.root.clone();
        let running = self.running.is_some();
        let tree_focused = self.pane == Pane::Tree;
        let mut toggle = None;
        let mut pick = None;
        let mut scrolled = false;
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let width = ui.available_width();
                for item in items.iter() {
                    let is_selected = selected.as_ref() == Some(&item.path);
                    let (response, chevron) =
                        paint_tree_row(ui, item, is_selected, tree_focused, width);
                    if is_selected && self.reveal {
                        ui.scroll_to_rect(response.rect, None);
                        scrolled = true;
                    }
                    let response = if running {
                        response.on_hover_text(i18n::t().tree_running)
                    } else {
                        response
                    };
                    if response.clicked() {
                        let on_chevron = response
                            .interact_pointer_pos()
                            .is_some_and(|pos| chevron.contains(pos));
                        if on_chevron {
                            toggle = Some(item.path.clone());
                        } else {
                            pick = Some(item.path.clone());
                        }
                    }
                }
            });
        if scrolled {
            self.reveal = false;
        }
        if let Some(path) = toggle {
            self.pane = Pane::Tree;
            if self.tree.is_expanded(&path) {
                self.tree.collapse(&path);
            } else {
                self.tree.expand(&path, true);
            }
        }
        if let Some(path) = pick {
            self.pane = Pane::Tree;
            self.select_folder(&ctx, path, true);
        }
    }

    fn draw_list(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let p = palette(&ctx);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        self.content_rect = ui.max_rect();
        if self.drop_hover {
            ui.painter().rect_stroke(
                ui.max_rect().shrink(2.0),
                CornerRadius::same(radius::LG),
                Stroke::new(2.0_f32, p.accent),
                egui::epaint::StrokeKind::Inside,
            );
        }
        let width = ui.available_width();
        paint_header(ui, width, self.algo().label());
        if self.root.is_none() {
            if let Some(index) = empty_state(ui, &self.recent) {
                self.open_recent(&ctx, index);
            }
            return;
        }
        let algo = self.algo().label();
        let listed = self.listed.get(&self.rows, self.status_filter, &self.query);
        if listed.is_empty() {
            let t = i18n::t();
            let note = if self.pending.is_some() {
                // The preview is coming soon; until then claim nothing – and do not read the folder
                // here on the UI thread.
                t.list_reading.to_string()
            } else if !self.rows.iter().any(in_list) {
                let has_subfolders = self
                    .root
                    .clone()
                    .is_some_and(|root| !self.tree.children(&root).is_empty());
                match (has_subfolders, self.recursive) {
                    (true, true) => t.no_files_sub_on.to_string(),
                    (true, false) => t.no_files_sub_off.to_string(),
                    (false, _) => t.no_files.to_string(),
                }
            } else if !self.query.trim().is_empty() {
                (t.no_hits)(self.query.trim())
            } else {
                t.no_status.to_string()
            };
            ui.add_space(space::S8);
            ui.horizontal(|ui| {
                ui.add_space(space::S6);
                ui.vertical(|ui| {
                    ui.set_max_width((width - 2.0 * space::S6).max(160.0));
                    ui.add(Label::new(RichText::new(note).size(text::LARGE).color(p.muted)).wrap());
                });
            });
            return;
        }
        let mut clicked = None;
        let mut opened = None;
        let mut action = None;
        let history_ok = self.timeline.is_ok();
        let timeline = self.timeline.as_ref().ok();
        let focused = self.pane == Pane::List;
        let mut scroll = ScrollArea::vertical()
            .id_salt("dateiliste")
            .auto_shrink([false, false]);
        if let Some(offset) = self.list_offset.take() {
            scroll = scroll.vertical_scroll_offset(offset);
        }
        let output = scroll.show_rows(ui, ROW, listed.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for &at in &listed[range] {
                let row = &self.rows[at];
                let selected = self.selected.as_deref() == Some(row.path.as_str());
                // The clock icon appears only on files that the history names.
                let history = timeline.filter(|timeline| {
                    !matches!(row.kind, RowKind::Checksum | RowKind::Report)
                        && !timeline.events(&row.path).is_empty()
                });
                let (response, history_clicked) =
                    paint_row(ui, row, selected, focused, width, history);
                if history_clicked {
                    action = Some((row.path.clone(), RowAction::History));
                }
                if response.double_clicked() {
                    opened = Some(row.path.clone());
                }
                if response.clicked() || response.secondary_clicked() {
                    clicked = Some(row.path.clone());
                }
                response.context_menu(|ui| {
                    if let Some(picked) = row_menu(ui, row, history_ok, algo) {
                        action = Some((row.path.clone(), picked));
                        ui.close();
                    }
                });
            }
        });
        self.list_view = (output.state.clamped_offset().y, output.inner_rect.height());
        if let Some(path) = clicked {
            self.selected = Some(path);
            self.pane = Pane::List;
        }
        if let Some(path) = opened {
            self.open_row(&ctx, &path);
        }
        if let Some((path, picked)) = action {
            self.selected = Some(path.clone());
            self.pane = Pane::List;
            match picked {
                RowAction::Open => self.open_row(&ctx, &path),
                RowAction::History => self.open_history(&ctx),
                RowAction::Delete => self.request_delete(&ctx),
                RowAction::Reveal => self.reveal_row(&ctx, &path),
                RowAction::CopyMd5 => self.copy_md5(&ctx, &path),
            }
        }
    }

    /// Shows the file in the file manager, selected in Explorer on Windows.
    fn reveal_row(&mut self, ctx: &egui::Context, path: &str) {
        let Some(row) = self.rows.find(path) else {
            return;
        };
        let absolute = row
            .absolute
            .clone()
            .filter(|_| row.kind != RowKind::Removed);
        let path = path.to_string();
        let Some(absolute) = absolute else {
            self.notify(ctx, Tone::Warn, Say::new(move |t| (t.reveal_gone)(&path)));
            return;
        };
        // Like Open and Delete: only a regular file in the folder.
        if let Some(reason) = self.outside_root(&absolute) {
            self.notify(
                ctx,
                Tone::Warn,
                Say::new(move |t| (t.reveal_refused)(&path, &i18n::shown_in(t, &reason))),
            );
            return;
        }
        if let Err(err) = (self.revealer)(&absolute) {
            self.notify(
                ctx,
                Tone::Error,
                Say::new(move |t| (t.reveal_failed)(&path, &err)),
            );
        }
    }

    /// Puts the file's checksum (MD5 or SHA) on the clipboard.
    fn copy_md5(&mut self, ctx: &egui::Context, path: &str) {
        let Some(md5) = self
            .rows
            .find(path)
            .map(row_md5)
            .filter(|md5| !md5.is_empty())
        else {
            return;
        };
        ctx.copy_text(md5);
        let path = path.to_string();
        let algo = self.algo().label();
        self.notify(
            ctx,
            Tone::Done,
            Say::new(move |t| (t.md5_copied)(algo, &path)),
        );
    }
}

/// Text of the dialog card before deleting.
fn delete_body(t: &Texts, delete: &Delete, md5_name: &str) -> String {
    sentences([
        Some((t.trash_body)(&delete.path)),
        delete.recorded.then(|| (t.trash_body_recorded)(md5_name)),
    ])
}

/// Text of the dialog card before replacing.
fn confirm_body(t: &Texts, confirm: &Confirm) -> String {
    // Right after Verify, "verify first" has already happened.
    let advise = !matches!(&confirm.reuse, Some(reuse) if reuse.source == JobKind::Verify);
    sentences([
        Some((t.replace_body)(
            &confirm.name,
            confirm.entries,
            &confirm.history,
            advise,
        )),
        confirm.reuse.as_ref().map(|reuse| {
            (t.replace_reuse)(reuse.count, reuse.source == JobKind::Verify, &reuse.at)
        }),
        (confirm.nested > 0).then(|| (t.replace_nested)(confirm.nested)),
    ])
}

/// Sentences joined into one text, separated by spaces; missing ones drop out.
fn sentences<const N: usize>(parts: [Option<String>; N]) -> String {
    parts.into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// Empty state (§8): one sentence, the recent folders (click, Enter for the first or their digit)
/// and the first keys. Returns the clicked recent folder.
fn empty_state(ui: &mut egui::Ui, recent: &[PathBuf]) -> Option<usize> {
    let p = palette(ui.ctx());
    let t = i18n::t();
    let mut picked = None;
    ui.add_space(space::S8);
    ui.horizontal(|ui| {
        ui.add_space(space::S6);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(space::S2, space::S2);
            ui.label(
                RichText::new(t.empty_title)
                    .size(text::TITLE)
                    .family(theme::semibold())
                    .color(p.text),
            );
            ui.label(RichText::new(t.empty_text).size(text::LARGE).color(p.muted));
            if !recent.is_empty() {
                ui.add_space(space::S2);
                theme::overline(ui, t.recent_title);
                for (index, path) in recent.iter().enumerate() {
                    let key = if index == 0 {
                        t.key_enter.to_string()
                    } else {
                        (index + 1).to_string()
                    };
                    let width = (ui.available_width() - space::S6).max(160.0);
                    if theme::link_row(ui, &key, &path.display().to_string(), width)
                        .on_hover_text((t.open_recent)(&path.display().to_string(), &key))
                        .clicked()
                    {
                        picked = Some(index);
                    }
                }
            }
            ui.add_space(space::S2);
            theme::overline(ui, t.keys_title);
            for (key, what) in t.empty_keys {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(space::S2, 0.0);
                    theme::keycap(ui, key);
                    ui.label(RichText::new(what).size(text::BODY).color(p.muted));
                });
            }
        });
    });
    picked
}

fn search_id() -> Id {
    Id::unique("veritas-suche")
}

fn threads_id() -> Id {
    Id::unique("veritas-threads")
}

fn theme_choices(t: &'static Texts) -> [theme::Choice<egui::ThemePreference>; 3] {
    [
        theme::Choice {
            glyph: icon::SUN,
            value: egui::ThemePreference::Light,
            tip: t.theme_light,
        },
        theme::Choice {
            glyph: icon::MOON,
            value: egui::ThemePreference::Dark,
            tip: t.theme_dark,
        },
        theme::Choice {
            glyph: icon::MONITOR,
            value: egui::ThemePreference::System,
            tip: t.theme_system,
        },
    ]
}

/// The numbers of the progress pill, right-aligned in fixed slots (§3): files, MB/s, time left.
/// Each slot is as wide as its longest value, so that nothing jumps (R1). Returns the occupied
/// width; to its left stands the file or phase.
fn paint_progress_numbers(ui: &egui::Ui, right: f32, y: f32, snap: &ProgressSnap) -> f32 {
    let p = palette(ui.ctx());
    let t = i18n::t();
    let font = FontId::new(text::BODY, FontFamily::Proportional);
    let measure = |sample: &str| {
        ui.ctx().fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(sample.to_owned(), font.clone(), p.text)
                .size()
                .x
        })
    };
    let mut slots: Vec<(String, String)> = Vec::new();
    if snap.files_total > 0 {
        let total = i18n::count(snap.files_total as usize);
        slots.push((
            (t.files_progress)(&i18n::count(snap.files_done as usize), &total),
            (t.files_progress)(&total, &total),
        ));
    }
    if snap.bytes_total > 0 {
        let speed = snap.mb_per_sec();
        let shown = if speed > 0.0 {
            i18n::speed(speed)
        } else {
            String::new()
        };
        slots.push((shown, i18n::speed(8888.0)));
        let eta = snap
            .eta()
            .map(|left| (t.left)(&i18n::eta(left)))
            .unwrap_or_default();
        slots.push((eta, (t.left)("88:88\u{a0}min")));
    }
    let gap = space::S3;
    let mut x = right;
    for (shown, sample) in slots.iter().rev() {
        let width = measure(sample);
        ui.painter().text(
            Pos2::new(x, y),
            Align2::RIGHT_CENTER,
            shown,
            font.clone(),
            p.text,
        );
        x -= width + gap;
    }
    if slots.is_empty() {
        0.0
    } else {
        right - x - gap
    }
}

fn progress_text(snap: &ProgressSnap) -> String {
    let t = i18n::t();
    let mut parts = vec![i18n::shown(&snap.phase).trim_end_matches('…').to_string()];
    if snap.files_total > 0 {
        if snap.files_done > 0 {
            parts.push((t.files_progress)(
                &i18n::count(snap.files_done as usize),
                &i18n::count(snap.files_total as usize),
            ));
        } else {
            parts.push((t.files)(snap.files_total as usize));
        }
    }
    if snap.reused > 0 {
        parts.push((t.reused_progress)(snap.reused as usize));
    }
    let speed = snap.mb_per_sec();
    if speed > 0.0 {
        parts.push(i18n::speed(speed));
    }
    if let Some(eta) = snap.eta() {
        parts.push((t.left)(&i18n::eta(eta)));
    }
    parts.join(" · ")
}

/// Short, for the status bar: what a mismatch means. The whole explanation is in the tooltip
/// (`status_detail`).
fn finding_short(t: &Texts, row: &FileRow) -> Option<&'static str> {
    if row.kind != RowKind::Modified {
        return None;
    }
    Some(match row.integrity.as_ref().map(|finding| finding.status) {
        Some(IntegrityStatus::PossiblyDamaged) => t.short_damaged,
        Some(IntegrityStatus::Intact) => t.short_intact,
        _ => t.short_unchecked,
    })
}

/// After "Create MD5" every hashed file stands in the file with its new hash. The comparison with
/// the old file stays in the message and in the JSON.
fn recorded_after_generate(rows: Vec<FileRow>) -> Vec<FileRow> {
    rows.into_iter()
        .filter_map(|mut row| match row.kind {
            RowKind::Removed => None,
            RowKind::Recorded | RowKind::Unchanged | RowKind::Modified | RowKind::Added => {
                row.kind = RowKind::Recorded;
                row.old_hash = row.new_hash.clone();
                row.integrity = None;
                Some(row)
            }
            RowKind::Error | RowKind::Partial | RowKind::Checksum | RowKind::Report => Some(row),
        })
        .collect()
}

/// Digits for Alt+1 … Alt+7 and the recent folders.
const DIGITS: [Key; 9] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

fn default_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .clamp(1, 256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashing::Digests;
    use crate::i18n::Msg;
    use crate::integrity::IntegrityFinding;
    use crate::list::mark_label;
    use crate::list::{row_date, row_size_text};
    use crate::progress::LiveRow;
    use crate::scanning::MarkCounts;
    use crate::theme::{DARK, LIGHT};

    /// The visible rows as in the list, as rows instead of numbers.
    fn listed_rows<'a>(
        rows: &'a [FileRow],
        filter: Option<RowMark>,
        query: &str,
    ) -> Vec<&'a FileRow> {
        let cached = Rows::from(rows.to_vec());
        Listed::default()
            .get(&cached, filter, query)
            .iter()
            .map(|&at| &rows[at])
            .collect()
    }

    impl VeritasApp {
        /// Waits for the preview that is being read or is waiting debounced, and adopts it – like
        /// `poll_preview`, only blocking. No sleeping in the test.
        fn settle(&mut self, ctx: &egui::Context) {
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.rx.is_none())
            {
                self.start_preview(ctx);
            }
            let Some(pending) = self.pending.take() else {
                return;
            };
            let preview = pending.rx.as_ref().unwrap().recv().unwrap();
            self.apply_preview(preview);
        }
    }

    fn row(kind: RowKind, md5: &str, integrity: Option<IntegrityStatus>) -> FileRow {
        FileRow {
            path: "a.txt".into(),
            absolute: None,
            kind,
            size: if kind == RowKind::Removed {
                None
            } else {
                Some(4)
            },
            modified: None,
            old_hash: crate::hashing::Digests::of(crate::hashing::HashMode::Md5, md5.into()),
            new_hash: if kind == RowKind::Removed {
                Digests::default()
            } else {
                crate::hashing::Digests::of(crate::hashing::HashMode::Md5, md5.into())
            },
            integrity: integrity.map(|status| IntegrityFinding {
                status,
                reason: "test".into(),
            }),
            message: None,
        }
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "veritas-ui-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(long_path(dir))
        }
    }

    /// The path with long names. On the CI's Windows runners `%TEMP%` lies under an 8.3 short name
    /// (`RUNNER~1`); the tree reads long names, however, and would otherwise not find the folder.
    /// `canonicalize` returns `\\?\C:\…` – the prefix is dropped. The temp folder is not a link, so
    /// this only changes the spelling.
    fn long_path(dir: PathBuf) -> PathBuf {
        let canon = dir.canonicalize().unwrap();
        let text = canon.to_string_lossy();
        match text.strip_prefix(r"\\?\") {
            Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
            _ => canon,
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn status_filter_keeps_only_the_chosen_mark() {
        let rows = vec![
            row(RowKind::Unchanged, "AA", None),
            row(RowKind::Modified, "BB", None),
            row(RowKind::Removed, "CC", None),
            row(
                RowKind::Modified,
                "DD",
                Some(IntegrityStatus::PossiblyDamaged),
            ),
            row(RowKind::Added, "EE", None),
            row(RowKind::Error, "FF", None),
            row(RowKind::Partial, "GG", None),
        ];
        assert_eq!(listed_rows(&rows, None, "").len(), 6);
        let ok = listed_rows(&rows, Some(RowMark::Ok), "");
        assert_eq!(ok.len(), 1);
        assert_eq!(FileRow::mark(ok[0]), Some(RowMark::Ok));
        assert_eq!(listed_rows(&rows, Some(RowMark::Wrong), "").len(), 1);
        assert_eq!(listed_rows(&rows, Some(RowMark::Damaged), "").len(), 1);
        assert_eq!(listed_rows(&rows, Some(RowMark::Deleted), "").len(), 1);
        assert_eq!(listed_rows(&rows, Some(RowMark::New), "").len(), 1);
        assert_eq!(listed_rows(&rows, Some(RowMark::Error), "").len(), 1);
        assert!(listed_rows(&rows, Some(RowMark::Open), "").is_empty());
        let counts = MarkCounts::of(&rows);
        assert_eq!(counts.get(RowMark::Error), 1);
        assert_eq!(counts.get(RowMark::New), 1);
        let mut named = row(RowKind::Removed, "EE", None);
        named.path = "Urlaub/IMG_1234.HEIC".into();
        let named_rows = [named];
        let searched = listed_rows(&named_rows, None, "img");
        assert_eq!(searched.len(), 1);
        assert!(listed_rows(&rows, None, "img").is_empty());
    }

    #[test]
    fn opening_shows_checksum_state_without_hashing() {
        let dir = Scratch::new("open");
        let present_a = "AA".repeat(16);
        let present_b = "BB".repeat(16);
        let missing = "CC".repeat(16);
        let nested = "DD".repeat(16);
        let nested_missing = "EE".repeat(16);
        std::fs::write(
            dir.0.join("probe.md5"),
            format!(
                "{present_a} da.jpg\n{present_b} db.jpg\n{missing} weg.jpg\n\
                 {nested} sub/dc.jpg\n{nested_missing} sub/fort.jpg\n"
            ),
        )
        .unwrap();
        std::fs::write(dir.0.join("da.jpg"), b"eins").unwrap();
        std::fs::write(dir.0.join("db.jpg"), b"zwei").unwrap();
        std::fs::create_dir(dir.0.join("sub")).unwrap();
        std::fs::write(dir.0.join("sub").join("dc.jpg"), b"drei").unwrap();
        std::fs::write(dir.0.join("probe.json"), b"{}").unwrap();

        let rows = preview::preview_files(&dir.0, "probe.md5", true, &AtomicBool::new(false)).rows;
        let find = |path: &str| rows.iter().find(|row| row.path == path).unwrap();
        // The history appears as a row of its own, neutral, in the list.
        let history = find("probe.json");
        assert_eq!(FileRow::mark(history), Some(RowMark::Report));
        assert_eq!(mark_label(RowMark::Report), "Verlauf");
        assert!(history.absolute.is_some());
        assert!(listed_rows(&rows, Some(RowMark::Open), "")
            .iter()
            .all(|row| row.path != "probe.json"));
        let da = find("da.jpg");
        assert_eq!(FileRow::mark(da), Some(RowMark::Open));
        assert_eq!(FileRow::mark(find("db.jpg")), Some(RowMark::Open));
        assert!(da.new_hash.md5.is_none());
        assert_eq!(da.old_hash.md5.as_deref(), Some(present_a.as_str()));
        assert!(da.modified.is_some());
        assert_eq!(FileRow::mark(find("weg.jpg")), Some(RowMark::Deleted));
        // A file in the subfolder exists: open, not deleted.
        let dc = find("sub/dc.jpg");
        assert_eq!(FileRow::mark(dc), Some(RowMark::Open));
        assert_eq!(dc.size, Some(4));
        assert_eq!(FileRow::mark(find("sub/fort.jpg")), Some(RowMark::Deleted));
        let source = find("probe.md5");
        assert_eq!(FileRow::mark(source), Some(RowMark::Source));
        assert_eq!(mark_label(RowMark::Source), "md5");
        assert_eq!(mark_tag(RowMark::Source, &DARK).0, DARK.muted);
        assert!(in_list(source));
        assert!(listed_rows(&rows, Some(RowMark::Ok), "")
            .iter()
            .all(|row| row.path != "probe.md5"));

        // Without subfolders their entries stay out.
        let flat = preview::preview_files(&dir.0, "probe.md5", false, &AtomicBool::new(false)).rows;
        assert!(flat.iter().all(|row| !row.path.starts_with("sub/")));
        assert!(flat.iter().any(|row| row.path == "weg.jpg"));
    }

    #[test]
    fn deleted_row_is_yellow_with_md5_and_empty_size_and_date() {
        let row = row(RowKind::Removed, "AABBCCDD", None);
        assert!(in_list(&row));
        assert_eq!(FileRow::mark(&row), Some(RowMark::Deleted));
        assert_eq!(mark_label(RowMark::Deleted), "gelöscht");
        assert_eq!(mark_tag(RowMark::Deleted, &DARK), (DARK.warn, DARK.warn_bg));
        assert_eq!(
            mark_tag(RowMark::Deleted, &DARK),
            mark_tag(RowMark::Wrong, &DARK)
        );
        assert!(row_size_text(&row).is_empty());
        assert!(row_date(&row).is_empty());
        assert_eq!(row_md5(&row), "AABBCCDD");
    }

    #[test]
    fn colors_follow_the_check() {
        assert_eq!(
            FileRow::mark(&row(RowKind::Recorded, "AA", None)),
            Some(RowMark::Open)
        );
        assert_eq!(
            FileRow::mark(&row(RowKind::Unchanged, "AA", None)),
            Some(RowMark::Ok)
        );
        assert_eq!(
            FileRow::mark(&row(
                RowKind::Modified,
                "AA",
                Some(IntegrityStatus::NotChecked)
            )),
            Some(RowMark::Wrong)
        );
        assert_eq!(
            FileRow::mark(&row(RowKind::Modified, "AA", Some(IntegrityStatus::Intact))),
            Some(RowMark::Wrong)
        );
        let damaged_row = row(
            RowKind::Modified,
            "AA",
            Some(IntegrityStatus::PossiblyDamaged),
        );
        assert!(damaged_row.damaged());
        assert_eq!(FileRow::mark(&damaged_row), Some(RowMark::Damaged));
        // Open is neutral, new is violet: blue means "selected" (§2.4).
        assert_eq!(mark_tag(RowMark::Open, &DARK).0, DARK.muted);
        assert_ne!(mark_tag(RowMark::Open, &DARK).0, DARK.accent);
        assert_eq!(mark_tag(RowMark::New, &DARK), (DARK.new, DARK.new_bg));
        assert_eq!(mark_tag(RowMark::New, &LIGHT), (LIGHT.new, LIGHT.new_bg));
        assert_ne!(
            mark_tag(RowMark::New, &DARK).0,
            mark_tag(RowMark::Open, &DARK).0
        );
        assert_eq!(mark_tag(RowMark::Ok, &DARK).0, DARK.ok);
        assert_eq!(mark_tag(RowMark::Wrong, &DARK).0, DARK.warn);
        assert_eq!(mark_tag(RowMark::Damaged, &DARK).0, DARK.error);
        assert_eq!(mark_tag(RowMark::Error, &DARK).0, DARK.error);
    }

    #[test]
    fn generate_turns_every_hashed_file_into_recorded() {
        let mut modified = row(RowKind::Modified, "AA", Some(IntegrityStatus::Intact));
        modified.new_hash.md5 = Some("BB".into());
        let rows = vec![
            row(RowKind::Unchanged, "AA", None),
            modified,
            row(RowKind::Added, "CC", None),
            row(RowKind::Removed, "DD", None),
            row(RowKind::Error, "EE", None),
        ];
        let after = recorded_after_generate(rows);
        assert_eq!(after.len(), 4);
        assert_eq!(
            after
                .iter()
                .filter(|row| row.kind == RowKind::Recorded)
                .count(),
            3
        );
        let changed = after
            .iter()
            .find(|row| row.new_hash.md5.as_deref() == Some("BB"))
            .unwrap();
        assert_eq!(changed.old_hash.md5.as_deref(), Some("BB"));
        assert!(changed.integrity.is_none());
        assert!(after.iter().any(|row| row.kind == RowKind::Error));
    }

    /// Where a text stands in the frame (the start of its first line), if it occurs in it.
    fn text_at(output: &egui::FullOutput, wanted: &str) -> Option<Pos2> {
        fn find(shape: &egui::Shape, wanted: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == wanted => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, wanted)),
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .find_map(|clipped| find(&clipped.shape, wanted))
    }

    /// The language button opens the list of languages, a click chooses; Ctrl+L goes to the next.
    /// What is already there – also a stored text in the status bar – follows at once, and `save`
    /// remembers the language.
    #[test]
    fn the_language_menu_and_ctrl_l_switch_the_language() {
        let ctx = context(egui::Theme::Light);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.status = Say::stored(Msg::RunCancelled.en());
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(i18n::current(), Lang::De);
        assert_eq!(app.status_line(), Msg::RunCancelled.de());
        assert_eq!(app.settings().language.as_deref(), Some("de"));

        let button = bar_rect(&app, "Sprache").center();
        frame(&ctx, &mut app, click(button));
        frame(&ctx, &mut app, vec![]);
        let output = frame_with(&ctx, &mut app, input(vec![]));
        let english = text_at(&output, Lang::En.name())
            .unwrap_or_else(|| panic!("The language list shows no “{}”", Lang::En.name()));
        assert!(english.y > button.y, "the list sits below the button");
        frame(&ctx, &mut app, click(english));
        frame(&ctx, &mut app, vec![]);
        assert_eq!(i18n::current(), Lang::En);
        assert_eq!(app.status_line(), Msg::RunCancelled.en());
        assert_eq!(app.settings().language.as_deref(), Some("en"));
        assert!(
            text_at(&frame_with(&ctx, &mut app, input(vec![])), Lang::De.name()).is_none(),
            "the list closes after the choice"
        );

        frame(&ctx, &mut app, vec![press(Key::L, Modifiers::COMMAND)]);
        assert_eq!(i18n::current(), Lang::Fr);
        assert_eq!(app.settings().language.as_deref(), Some("fr"));
        for _ in 0..4 {
            frame(&ctx, &mut app, vec![press(Key::L, Modifiers::COMMAND)]);
        }
        assert_eq!(
            i18n::current(),
            Lang::En,
            "Ctrl+L cycles through the languages"
        );
        i18n::set(Lang::De);
    }

    /// A saved language applies; an unknown or missing one falls back to the system's (in `new`).
    /// Older settings without the field still read.
    #[test]
    fn settings_without_a_language_still_load() {
        let old: Settings = serde_json::from_str(r#"{"recursive": false, "threads": 3}"#).unwrap();
        assert!(old.language.is_none());
        assert!(!old.recursive);
        let saved: Settings =
            serde_json::from_str(r#"{"recursive": true, "threads": 3, "language": "it"}"#).unwrap();
        assert_eq!(
            saved.language.as_deref().and_then(Lang::from_code),
            Some(Lang::It)
        );
    }

    /// Numbers, sizes and date follow the language; time left is the same everywhere.
    #[test]
    fn numbers_and_sizes_follow_the_language() {
        let (de, en) = (Lang::De.texts(), Lang::En.texts());
        assert_eq!(i18n::count_in(de, 0), "0");
        assert_eq!(i18n::count_in(de, 999), "999");
        assert_eq!(i18n::count_in(de, 1_234), "1.234");
        assert_eq!(i18n::count_in(de, 1_234_567), "1.234.567");
        assert_eq!(i18n::count_in(en, 1_234_567), "1,234,567");
        assert_eq!(i18n::size_in(de, 12), "12\u{a0}B");
        assert_eq!(i18n::size_in(de, 1536), "1,5\u{a0}KB");
        assert_eq!(i18n::size_in(en, 1536), "1.5\u{a0}KB");
        assert_eq!(i18n::eta(Duration::from_secs(42)), "42\u{a0}s");
        assert_eq!(i18n::eta(Duration::from_secs(185)), "3:05\u{a0}min");
        assert_eq!(
            i18n::count_parts_in(
                de,
                &[(2, Change::Differs), (0, Change::New), (1, Change::Missing)]
            ),
            "2 abweichend, 1 gelöscht"
        );
        assert_eq!((de.files)(1), "1 Datei");
        assert_eq!((de.files)(0), "0 Dateien");
        assert_eq!((de.files)(1_234), "1.234 Dateien");
        assert_eq!((en.files)(1_234), "1,234 files");
        let at = jiff::civil::date(2026, 10, 12).at(19, 2, 0, 0);
        assert_eq!(at.strftime(de.date_time).to_string(), "12.10.2026 19:02");
        assert_eq!(at.strftime(en.date_time).to_string(), "2026-10-12 19:02");
        for lang in Lang::ALL {
            let shown = at.strftime(lang.texts().date_time).to_string();
            assert!(
                shown.contains("2026") && shown.contains("19:02"),
                "{lang:?}"
            );
        }
    }

    /// Context menu: "Show in folder" calls the file manager (here a dummy), "Copy MD5" puts the
    /// checksum on the clipboard. Both are also drawn as a menu.
    #[test]
    fn the_menu_shows_the_folder_and_copies_the_md5() {
        static SHOWN: AtomicBool = AtomicBool::new(false);
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Light);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.revealer = |path| {
            SHOWN.store(path.ends_with("a.txt"), Ordering::Relaxed);
            Ok(())
        };
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.reveal_row(&ctx, "a.txt");
        assert!(SHOWN.load(Ordering::Relaxed));
        assert!(app.notice.is_none());
        app.copy_md5(&ctx, "a.txt");
        assert!(app
            .notice
            .as_ref()
            .is_some_and(|notice| notice.text.get().contains("kopiert")));
        // The menu itself: all rows drawn, nothing chosen.
        let row = app
            .rows
            .iter()
            .find(|row| row.path == "a.txt")
            .cloned()
            .unwrap();
        let mut output = ctx.run_ui(input(Vec::new()), |ui| {
            assert!(row_menu(ui, &row, true, "MD5").is_none());
        });
        output.textures_delta.clear();
    }

    /// A mismatching file says in the status bar what that means, and in the tooltip what to do.
    #[test]
    fn a_differing_file_explains_itself() {
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        let mut changed = row(RowKind::Modified, "AA", Some(IntegrityStatus::Intact));
        changed.new_hash.md5 = Some("BB".into());
        app.rows.push(changed);
        app.selected = Some("a.txt".into());
        let line = app.status_line();
        assert!(line.contains("Anfang und Ende in Ordnung"), "{line}");
        let detail = app.status_detail().unwrap();
        assert!(detail.contains("aus der Sicherung zurückholen"), "{detail}");
        assert!(detail.contains("Strukturprüfung: test"), "{detail}");
        // A frame with a selected row paints the tooltip without error.
        frame(&ctx, &mut app, Vec::new());
    }

    #[test]
    fn notices_fade_and_errors_stay() {
        let done = Notice {
            text: "kurz".into(),
            tone: Tone::Done,
            shown: 0.0,
            sticky: false,
        };
        assert_eq!(done.opacity(4.9), 1.0);
        assert!(done.opacity(5.4) < 1.0);
        assert_eq!(done.opacity(6.0), 0.0);
        let long = Notice {
            text: "x".repeat(500).into(),
            tone: Tone::Warn,
            shown: 0.0,
            sticky: false,
        };
        assert_eq!(long.hold(), Some(10.0));
        let error = Notice {
            text: "Fehler".into(),
            tone: Tone::Error,
            shown: 0.0,
            sticky: false,
        };
        assert_eq!(error.opacity(1000.0), 1.0);
        // A finding stays like an error.
        let finding = Notice {
            text: "Prüfung: 1 abweichend".into(),
            tone: Tone::Warn,
            shown: 0.0,
            sticky: true,
        };
        assert_eq!(finding.opacity(1000.0), 1.0);
        assert!(finding.stays());
    }

    #[test]
    fn crash_log_is_mentioned_once() {
        let ctx = egui::Context::default();
        let log = Path::new("crash.log");
        let mut app = VeritasApp::with_settings(Settings::default());
        app.note_crash(&ctx, log, None);
        assert!(app.notice.is_none());

        app.note_crash(&ctx, log, Some(100));
        let notice = app.notice.take().unwrap();
        assert!(notice.tone == Tone::Error);
        assert!(notice.text.get().contains("crash.log"));
        assert_eq!(app.crash_seen, 100);

        // The same state at the next start: no second notice.
        let mut again = VeritasApp::with_settings(Settings {
            crash_seen: app.crash_seen,
            ..Settings::default()
        });
        again.note_crash(&ctx, log, Some(100));
        assert!(again.notice.is_none());
        again.note_crash(&ctx, log, Some(101));
        assert!(again.notice.is_some());
    }

    /// Dimensions of the UI on the 4 px grid (§4.1). `ROW` is 28 instead of 32 (§4.2): a deliberate
    /// exception for a dense file list.
    #[test]
    fn measures_follow_the_grid() {
        assert_eq!(ROW, 28.0);
        assert_eq!(PILL_H, 48.0);
        const { assert!(PILL_W >= 300.0) };
        let cols = crate::list::columns(1000.0);
        for value in [
            TREE_W,
            ROW,
            crate::tree::INDENT,
            SEARCH_W,
            PILL_W,
            NOTICE_W,
            theme::DIALOG_W,
            THREADS_W,
            cols.status,
            cols.size,
            cols.date,
            cols.md5,
            crate::list::columns(600.0).md5,
        ] {
            assert_eq!(value % 4.0, 0.0, "{value} is not on the grid");
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn tree_starts_at_the_filesystem_root() {
        assert_eq!(filesystem_roots(), vec![PathBuf::from("/")]);
        assert_eq!(
            crate::tree::start_root(&filesystem_roots()),
            Some(PathBuf::from("/"))
        );
        assert_eq!(
            parent_chain(Path::new("/tmp/veritas-demo/dichte")),
            vec![
                PathBuf::from("/"),
                PathBuf::from("/tmp"),
                PathBuf::from("/tmp/veritas-demo"),
            ]
        );
    }

    #[test]
    #[cfg(windows)]
    fn drives_come_from_the_bitmask() {
        let roots = filesystem_roots();
        assert!(!roots.is_empty());
        assert!(roots
            .iter()
            .all(|root| root.to_string_lossy().ends_with(":\\")));
    }

    fn context(theme: egui::Theme) -> egui::Context {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        ctx.set_theme(theme);
        ctx
    }

    fn press(key: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// Input for a frame as large as the start window.
    fn input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 820.0))),
            events,
            ..Default::default()
        }
    }

    /// A frame with a bare egui context; returns the output.
    fn frame_with(
        ctx: &egui::Context,
        app: &mut VeritasApp,
        input: egui::RawInput,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(input, |ui| app.show(ui));
        // Without a screen there is no renderer to take over the textures.
        output.textures_delta.clear();
        assert!(!output.shapes.is_empty());
        output
    }

    /// A frame with a bare egui context, as large as the start window.
    fn frame(ctx: &egui::Context, app: &mut VeritasApp, events: Vec<egui::Event>) {
        frame_with(ctx, app, input(events));
    }

    /// A run that does not finish as long as the test holds the sender. Returns the sender and the
    /// cancel signal.
    fn endless_run(
        app: &mut VeritasApp,
        root: PathBuf,
    ) -> (mpsc::Sender<Result<ScanOutcome, Say>>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        app.running = Some(Running {
            root,
            progress: Arc::new(Progress::new()),
            cancel: Arc::clone(&cancel),
            rx,
            live: Live::default(),
            live_due: LIVE_SECS,
        });
        (tx, cancel)
    }

    /// The commands to the window from a frame.
    fn commands(output: &egui::FullOutput) -> Vec<egui::ViewportCommand> {
        output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|viewport| viewport.commands.clone())
            .unwrap_or_default()
    }

    /// Folder with a checksum file and two files, one of them in a subfolder.
    fn folder_with_md5() -> Scratch {
        let dir = Scratch::new("ablauf");
        std::fs::create_dir(dir.0.join("sub")).unwrap();
        std::fs::write(dir.0.join("a.txt"), b"abc").unwrap();
        std::fs::write(dir.0.join("sub").join("b.txt"), b"abc").unwrap();
        let name = format!("{}.md5", dir.0.file_name().unwrap().to_str().unwrap());
        std::fs::write(
            dir.0.join(name),
            "900150983CD24FB0D6963F7D28E17F72  a.txt\n\
             900150983CD24FB0D6963F7D28E17F72  sub/b.txt\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn whole_window_draws_in_both_themes() {
        let dir = folder_with_md5();
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let ctx = context(theme);
            let mut app = VeritasApp::with_settings(Settings::default());
            // Empty state, then folder on an HDD, notice and dialog card; areas need two frames.
            frame(&ctx, &mut app, Vec::new());
            app.select_folder(&ctx, dir.0.clone(), true);
            app.settle(&ctx);
            assert!(app.md5_exists);
            app.seeks = Some(true);
            app.notify(&ctx, Tone::Error, "Test error");
            frame(&ctx, &mut app, Vec::new());
            frame(&ctx, &mut app, Vec::new());
            app.request(&ctx, JobKind::Generate);
            frame(&ctx, &mut app, Vec::new());
            frame(&ctx, &mut app, Vec::new());
            let card = ctx
                .memory(|mem| mem.area_rect(Id::unique("veritas-dialog")))
                .unwrap();
            assert!(card.width() >= 440.0, "{card:?}");
            let notice = ctx.memory(|mem| mem.area_rect(Id::unique("veritas-notice")));
            assert!(notice.is_some());
            // "About Veritas", the history card on both tabs and the confirmation before deleting;
            // everything fits into the start window.
            app.confirm = None;
            app.about_open = true;
            frame(&ctx, &mut app, Vec::new());
            frame(&ctx, &mut app, Vec::new());
            fits(&ctx, "veritas-about");
            app.about_open = false;
            write_history(&dir.0);
            app.timeline = preview::history_of(&dir.0, &app.md5_name).map_err(Say::stored);
            for file in [Some("a.txt".to_string()), None] {
                app.history_view = Some(history::View::new(3, file));
                frame(&ctx, &mut app, Vec::new());
                frame(&ctx, &mut app, Vec::new());
                let card = fits(&ctx, "veritas-history");
                assert!(card.width() >= 880.0, "{card:?}");
            }
            app.history_view = None;
            app.selected = Some("a.txt".into());
            app.request_delete(&ctx);
            frame(&ctx, &mut app, Vec::new());
            frame(&ctx, &mut app, Vec::new());
            fits(&ctx, "veritas-delete");
            app.delete = None;
        }
    }

    /// Ctrl as on Windows and Linux: `ctrl` and `command` at once.
    const CTRL: Modifiers = Modifiers {
        alt: false,
        ctrl: true,
        shift: false,
        mac_cmd: false,
        command: true,
    };

    /// The card lies within the start window.
    fn fits(ctx: &egui::Context, id: &str) -> Rect {
        let card = ctx
            .memory(|mem| mem.area_rect(Id::unique(id)))
            .unwrap_or_else(|| panic!("{id} is missing"));
        assert!(
            card.height() <= 820.0 && card.width() <= 1180.0,
            "{id}: {card:?}"
        );
        card
    }

    /// Puts the history from `history::tests::sample` next to the .md5.
    fn write_history(dir: &Path) -> String {
        let name = format!("{}.json", dir.file_name().unwrap().to_str().unwrap());
        let text = history::tests::sample().to_pretty_json().unwrap();
        std::fs::write(dir.join(&name), text).unwrap();
        name
    }

    #[test]
    fn f1_opens_about_and_escape_closes_it() {
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        frame(&ctx, &mut app, vec![press(Key::F1, Modifiers::NONE)]);
        assert!(app.about_open);
        // While the card is open, F5 starts nothing.
        frame(&ctx, &mut app, vec![press(Key::F5, Modifiers::NONE)]);
        assert!(app.notice.is_none());
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(!app.about_open);
    }

    #[test]
    fn ctrl_h_shows_the_history_and_a_chosen_file() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        frame(&ctx, &mut app, Vec::new());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        // Without a history Ctrl+H names the reason.
        assert!(app
            .can_show_history()
            .unwrap_err()
            .get()
            .starts_with("Noch kein Verlauf"));
        frame(&ctx, &mut app, vec![press(Key::H, CTRL)]);
        assert!(app.history_view.is_none());
        assert!(app.notice.is_some());
        app.notice = None;

        let json = write_history(&dir.0);
        app.select_folder(&ctx, dir.0.clone(), false);
        app.settle(&ctx);
        assert!(app.timeline.is_ok());
        // With a selected file its tab is in front, the newest run is selected.
        app.selected = Some("a.txt".into());
        app.pane = Pane::List;
        frame(&ctx, &mut app, vec![press(Key::H, CTRL)]);
        let view = app.history_view.clone().unwrap();
        assert_eq!(view.tab, history::Tab::File);
        assert_eq!((view.file.as_deref(), view.run), (Some("a.txt"), 2));
        // ← switches to the runs, ↓ selects the older one.
        frame(&ctx, &mut app, vec![press(Key::ArrowLeft, Modifiers::NONE)]);
        frame(&ctx, &mut app, vec![press(Key::ArrowDown, Modifiers::NONE)]);
        let view = app.history_view.clone().unwrap();
        assert_eq!((view.tab, view.run), (history::Tab::Runs, 1));
        // Under the card F5 and Ctrl+Enter start nothing.
        frame(
            &ctx,
            &mut app,
            vec![
                press(Key::F5, Modifiers::NONE),
                press(Key::Enter, Modifiers::COMMAND),
            ],
        );
        assert!(app.running.is_none() && app.confirm.is_none());
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(app.history_view.is_none());
        // Enter on the "History" row shows the card, not another program.
        app.selected = Some(json);
        frame(&ctx, &mut app, vec![press(Key::Enter, Modifiers::NONE)]);
        let view = app.history_view.clone().unwrap();
        assert_eq!((view.tab, view.file), (history::Tab::Runs, None));
    }

    #[test]
    fn the_status_line_names_the_history_before_the_hashes() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        write_history(&dir.0);
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.selected = Some("a.txt".into());
        let line = app.status_line();
        assert!(line.contains("Verlauf: keine Änderung seit"), "{line}");
        // A mismatching file that the history knows: first the history, then the hashes.
        let mut changed = row(RowKind::Modified, "AA", Some(IntegrityStatus::Intact));
        changed.path = "IMG.JPG".into();
        changed.new_hash = crate::hashing::Digests::of(crate::hashing::HashMode::Md5, "BB".into());
        app.rows.push(changed);
        app.selected = Some("IMG.JPG".into());
        let line = app.status_line();
        let hint = line.find("Verlauf: ").unwrap();
        assert!(hint < line.find("MD5 gespeichert").unwrap(), "{line}");
    }

    #[test]
    fn delete_always_asks_and_uses_the_trash() {
        let dir = folder_with_md5();
        std::fs::write(dir.0.join("frei.txt"), b"x").unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        // Not the machine's Recycle Bin: the dummy deletes the temp file.
        app.trasher = |path| std::fs::remove_file(path).map_err(|err| err.to_string());
        frame(&ctx, &mut app, Vec::new());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.selected = Some("a.txt".into());
        app.pane = Pane::List;
        // Del asks; Esc keeps the file.
        frame(&ctx, &mut app, vec![press(Key::Delete, Modifiers::NONE)]);
        assert!(app.delete.as_ref().is_some_and(|delete| delete.recorded));
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(app.delete.is_none());
        assert!(dir.0.join("a.txt").is_file());
        // Del and Enter: it is in the .md5 and afterwards deleted, with its hash.
        frame(&ctx, &mut app, vec![press(Key::Delete, Modifiers::NONE)]);
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![press(Key::Enter, Modifiers::NONE)]);
        assert!(!dir.0.join("a.txt").exists());
        let gone = app.rows.iter().find(|row| row.path == "a.txt").unwrap();
        assert_eq!(gone.kind, RowKind::Removed);
        assert_eq!(row_md5(gone), "900150983CD24FB0D6963F7D28E17F72");
        // A file that is not in the .md5 disappears from the list.
        app.selected = Some("frei.txt".into());
        frame(&ctx, &mut app, vec![press(Key::Delete, Modifiers::NONE)]);
        assert!(app.delete.as_ref().is_some_and(|delete| !delete.recorded));
        frame(&ctx, &mut app, vec![press(Key::Enter, Modifiers::NONE)]);
        assert!(!app.rows.iter().any(|row| row.path == "frei.txt"));
        // Veritas does not delete the checksum file; a notice says why.
        let md5 = app.md5_name.clone();
        app.selected = Some(md5.clone());
        app.notice = None;
        frame(&ctx, &mut app, vec![press(Key::Delete, Modifiers::NONE)]);
        assert!(app.delete.is_none());
        assert!(app
            .notice
            .as_ref()
            .is_some_and(|notice| notice.text.get().contains("löscht Veritas nicht")));
        assert!(dir.0.join(md5).is_file());
    }

    /// Del and double-click only for regular files in the folder – an error row that names a
    /// folder, and a path to the outside, are rejected, without starting a program.
    #[test]
    fn delete_and_open_refuse_folders_and_paths_outside() {
        let dir = folder_with_md5();
        let outside = Scratch::new("draussen");
        std::fs::write(outside.0.join("x.txt"), b"x").unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.trasher = |path| std::fs::remove_file(path).map_err(|err| err.to_string());
        frame(&ctx, &mut app, Vec::new());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.pane = Pane::List;
        let mut folder = row(RowKind::Error, "", None);
        folder.path = "sub".into();
        folder.absolute = Some(dir.0.join("sub"));
        let mut far = row(RowKind::Added, "", None);
        far.path = "weit.txt".into();
        far.absolute = Some(outside.0.join("x.txt"));
        app.rows.push(folder);
        app.rows.push(far);
        for (path, why) in [("sub", "keine reguläre Datei"), ("weit.txt", "nicht im")] {
            app.selected = Some(path.into());
            app.notice = None;
            frame(&ctx, &mut app, vec![press(Key::Delete, Modifiers::NONE)]);
            assert!(app.delete.is_none(), "{path}");
            assert!(
                app.notice
                    .as_ref()
                    .is_some_and(|notice| notice.text.get().contains(why)),
                "{path}: {:?}",
                app.notice.as_ref().map(|notice| &notice.text)
            );
            app.notice = None;
            app.open_row(&ctx, path);
            assert!(app
                .notice
                .as_ref()
                .is_some_and(|notice| notice.text.get().contains("lässt sich nicht öffnen")));
        }
        assert!(dir.0.join("sub").join("b.txt").is_file());
        assert!(outside.0.join("x.txt").is_file());
    }

    /// An open "Replace?" confirmation lapses when another folder is opened.
    #[test]
    fn replace_question_is_dropped_on_another_folder() {
        let first = folder_with_md5();
        let second = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, first.0.clone(), true);
        app.settle(&ctx);
        frame(&ctx, &mut app, vec![press(Key::Enter, Modifiers::COMMAND)]);
        assert!(app.confirm.is_some());
        // Like dropping: `open_path` with the other folder.
        app.open_path(&ctx, second.0.clone());
        app.settle(&ctx);
        assert!(app.confirm.is_none());
        frame(&ctx, &mut app, vec![press(Key::Enter, Modifiers::NONE)]);
        assert!(app.running.is_none(), "Enter no longer replaces anything");
    }

    /// Toggling "Subfolders" also reloads after a run.
    #[test]
    fn subfolder_switch_after_a_run_reloads() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        frame(&ctx, &mut app, vec![press(Key::F5, Modifiers::NONE)]);
        for _ in 0..500 {
            if app.running.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            frame(&ctx, &mut app, Vec::new());
        }
        assert!(app.from_run, "{}", app.status);
        assert!(app.rows.iter().any(|row| row.path == "sub/b.txt"));
        app.recursive = false;
        app.subfolders_changed(&ctx);
        assert!(!app.from_run);
        app.settle(&ctx);
        assert!(app.pending.is_none());
        assert!(
            app.rows
                .iter()
                .any(|row| row.path == "a.txt" && row.kind == RowKind::Recorded),
            "the preview has arrived"
        );
        assert!(!app.rows.iter().any(|row| row.path == "sub/b.txt"));
    }

    /// The preview follows no link – an entry behind a link to the outside is deleted and has no
    /// path to open or delete.
    #[test]
    fn preview_does_not_follow_links() {
        let dir = Scratch::new("vorschau-link");
        let outside = Scratch::new("vorschau-draussen");
        std::fs::write(outside.0.join("x.txt"), b"abc").unwrap();
        std::fs::write(
            dir.0.join("probe.md5"),
            "900150983CD24FB0D6963F7D28E17F72  link/x.txt\n",
        )
        .unwrap();
        let link = dir.0.join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside.0, &link).unwrap();
        // On Windows a junction: it needs no permission for links.
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(&link)
                .arg(&outside.0)
                .output()
                .is_ok_and(|out| out.status.success());
            if !made {
                return;
            }
        }
        assert!(link.join("x.txt").is_file());
        let rows = preview::preview_files(&dir.0, "probe.md5", true, &AtomicBool::new(false)).rows;
        let entry = rows.iter().find(|row| row.path == "link/x.txt").unwrap();
        assert_eq!(entry.kind, RowKind::Removed);
        assert!(entry.absolute.is_none());
        // Before removing the folder, detach the junction instead of emptying its target.
        let _ = std::fs::remove_dir(&link);
    }

    /// An ANSI `.md5` in the preview shows its files as open, with a notice.
    #[test]
    fn preview_reads_an_ansi_md5() {
        let dir = Scratch::new("ansi");
        std::fs::write(dir.0.join("Köln.txt"), b"abc").unwrap();
        let mut ansi = b"900150983CD24FB0D6963F7D28E17F72  K\xf6ln.txt\r\n".to_vec();
        ansi.extend(b"900150983CD24FB0D6963F7D28E17F72  ./fehlt.txt\r\n");
        std::fs::write(dir.0.join("probe.md5"), ansi).unwrap();
        let preview::Listing { rows, note, .. } =
            preview::preview_files(&dir.0, "probe.md5", true, &AtomicBool::new(false));
        let kind = |path: &str| rows.iter().find(|row| row.path == path).map(|row| row.kind);
        assert_eq!(kind("Köln.txt"), Some(RowKind::Recorded));
        assert_eq!(kind("fehlt.txt"), Some(RowKind::Removed));
        assert!(note.is_some_and(|note| note.contains("Windows-1252")));
    }

    #[cfg(windows)]
    #[test]
    fn network_paths_have_no_trash() {
        assert!(no_trash(Path::new(r"\\server\share\a.txt")).is_some());
        assert!(no_trash(&std::env::temp_dir().join("a.txt")).is_none());
    }

    #[test]
    fn replace_asks_first_and_escape_keeps_the_file() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        let md5_path = dir.0.join(&app.md5_name);
        let before = std::fs::read(&md5_path).unwrap();
        assert_eq!(
            listed_rows(&app.rows, Some(RowMark::Open), "").len(),
            2,
            "a.txt and sub/b.txt are open"
        );

        frame(&ctx, &mut app, vec![press(Key::Enter, Modifiers::COMMAND)]);
        let confirm = app.confirm.as_ref().expect("Ctrl+Enter asks first");
        assert_eq!(confirm.entries, 2);
        assert!(app.running.is_none());
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(app.confirm.is_none());
        assert!(app.running.is_none());
        assert!(app.status.get().contains("bleibt unverändert"));
        assert_eq!(std::fs::read(&md5_path).unwrap(), before);

        std::fs::write(dir.0.join("a.txt"), b"neu").unwrap();
        frame(&ctx, &mut app, vec![press(Key::Enter, Modifiers::COMMAND)]);
        frame(&ctx, &mut app, vec![press(Key::Enter, Modifiers::NONE)]);
        assert!(app.confirm.is_none());
        assert!(app.running.is_some(), "Enter replaces");
        for _ in 0..500 {
            if app.running.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            frame(&ctx, &mut app, Vec::new());
        }
        assert!(app.running.is_none());
        assert!(app.status.get().contains("ersetzt"), "{}", app.status);
        // The same words as the history card for a replace run.
        assert!(app.status.get().contains("1 geändert"), "{}", app.status);
        assert!(app.status.get().contains("2 Dateien"), "{}", app.status);
        // After replacing, every file stands in the list with its new hash.
        assert_eq!(
            app.rows
                .iter()
                .filter(|row| row.kind == RowKind::Recorded)
                .count(),
            2
        );
        assert_ne!(std::fs::read(&md5_path).unwrap(), before);
        // Instead of a backup copy the history records the replacement.
        let history = checksum::names_from_md5_filename(&app.md5_name).json;
        assert!(app
            .rows
            .iter()
            .any(|row| row.path == history && row.kind == RowKind::Report));
        let names: Vec<String> = std::fs::read_dir(&dir.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|name| !name.contains(".bak")), "{names:?}");
        let body = confirm_body(
            i18n::t(),
            &Confirm {
                name: "Urlaub.md5".into(),
                history: "Urlaub.json".into(),
                entries: 2,
                nested: 0,
                reuse: None,
            },
        );
        assert!(body.contains("Verlauf in Urlaub.json"), "{body}");
        assert!(!body.contains("Übernimmt"), "{body}");
        assert!(!body.contains("verloren"), "{body}");
        // Replace also carries over unnoticed changes; the text says so.
        assert!(body.contains("Im Zweifel vorher prüfen (F5)"), "{body}");
        assert!(body.contains("2 Prüfsummen"), "{body}");
    }

    #[test]
    fn an_opened_md5_is_used_instead_of_the_rule() {
        let dir = folder_with_md5();
        let rule = format!("{}.md5", dir.0.file_name().unwrap().to_str().unwrap());
        std::fs::write(
            dir.0.join("alt.md5"),
            "900150983CD24FB0D6963F7D28E17F72  a.txt\n",
        )
        .unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.open_path(&ctx, dir.0.join("alt.md5"));
        app.settle(&ctx);
        assert_eq!(app.md5_name, "alt.md5");
        // A click on the same folder in the tree keeps the opened file.
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert_eq!(app.md5_name, "alt.md5");
        // Another folder and back: the rule applies again.
        let other = Scratch::new("anderer");
        app.select_folder(&ctx, other.0.clone(), true);
        app.settle(&ctx);
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert_eq!(app.md5_name, rule);
        // A file other than a .md5 opens only its folder.
        app.open_path(&ctx, dir.0.join("a.txt"));
        app.settle(&ctx);
        assert_eq!(app.root.as_ref(), Some(&dir.0));
        assert_eq!(app.md5_name, rule);
    }

    /// A folder with only `SHA256SUMS`: it applies, Verify computes SHA-256 and shows it that way;
    /// "Create MD5" does not ask (there is no `.md5` yet), writes `{folder name}.md5` next to it,
    /// and that one applies afterwards. The SHA file stays unchanged and disappears from the list.
    #[test]
    fn a_sha_file_is_verified_and_an_md5_written_beside_it() {
        let dir = Scratch::new("sha");
        std::fs::write(dir.0.join("a.txt"), b"abc").unwrap();
        std::fs::write(dir.0.join("b.txt"), b"neu").unwrap();
        let sha = |data: &[u8]| {
            crate::hashing::hash_slice(data, HashMode::Sha256)
                .sha256
                .unwrap()
        };
        let sums = format!(
            "{}  a.txt\n{}  b.txt\n",
            sha(b"abc").to_ascii_lowercase(),
            sha(b"alt")
        );
        std::fs::write(dir.0.join("SHA256SUMS"), &sums).unwrap();
        let ctx = context(egui::Theme::Light);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert_eq!(app.md5_name, "SHA256SUMS");
        assert!(app.md5_exists);
        assert_eq!(app.algo(), HashMode::Sha256);
        let source = app.rows.find("SHA256SUMS").unwrap();
        assert_eq!(source.kind, RowKind::Checksum);
        assert_eq!(
            row_md5(app.rows.find("a.txt").unwrap()),
            sha(b"abc"),
            "the preview shows the SHA-256 from the file"
        );
        frame(&ctx, &mut app, Vec::new());

        app.request(&ctx, JobKind::Verify);
        finish_run(&ctx, &mut app);
        assert_eq!(app.rows.find("a.txt").unwrap().kind, RowKind::Unchanged);
        assert_eq!(app.rows.find("b.txt").unwrap().kind, RowKind::Modified);
        app.selected = Some("b.txt".into());
        let line = app.status_line();
        assert!(line.contains("SHA-256 gespeichert"), "{line}");
        assert!(line.contains(&sha(b"neu")), "{line}");
        frame(&ctx, &mut app, Vec::new());

        let md5 = format!("{}.md5", dir.0.file_name().unwrap().to_str().unwrap());
        assert_eq!(app.md5_target(), md5);
        app.request(&ctx, JobKind::Generate);
        assert!(app.confirm.is_none(), "no .md5 there, nothing to replace");
        finish_run(&ctx, &mut app);
        assert_eq!(app.md5_name, md5);
        assert!(app.pinned_md5.is_none());
        assert_eq!(app.algo(), HashMode::Md5);
        assert_eq!(
            std::fs::read_to_string(dir.0.join("SHA256SUMS")).unwrap(),
            sums
        );
        let written = std::fs::read_to_string(dir.0.join(&md5)).unwrap();
        assert!(written.contains("  a.txt\n") && written.contains("  b.txt\n"));
        assert!(!written.contains("SHA256SUMS"), "{written}");
        assert!(app.rows.find("SHA256SUMS").is_none());
        assert_eq!(app.rows.find(&md5).unwrap().kind, RowKind::Checksum);
        // One history for both: the SHA-256 run and the creation of the `.md5`. The card shows it
        // with both tabs.
        let timeline = app.timeline.as_ref().unwrap();
        assert_eq!(timeline.runs().len(), 2);
        assert_eq!(timeline.runs()[0].hash_mode, "sha256");
        assert!(timeline.hint("b.txt", true).is_some());
        app.selected = Some("b.txt".into());
        for file in [Some("b.txt".to_string()), None] {
            app.history_view = Some(history::View::new(2, file));
            frame(&ctx, &mut app, Vec::new());
            frame(&ctx, &mut app, Vec::new());
        }
        app.history_view = None;
        // Toggling subfolders re-reads – and stays with the new `.md5`.
        app.load_preview(&ctx, 0.0);
        app.settle(&ctx);
        assert_eq!(app.md5_name, md5);
        assert!(app.rows.find("SHA256SUMS").is_none());
    }

    /// An explicitly opened SHA file next to a `.md5`: "Create MD5" replaces the `.md5`, and the
    /// confirmation names it and its entries, not the SHA file.
    #[test]
    fn a_pinned_sha_file_beside_an_md5_asks_about_the_md5() {
        let dir = folder_with_md5();
        let md5 = format!("{}.md5", dir.0.file_name().unwrap().to_str().unwrap());
        let sums = "A9993E364706816ABA3E25717850C26C9CD0D89D  a.txt\n";
        std::fs::write(dir.0.join("SHA1SUMS"), sums).unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        // Without instruction the `.md5` applies.
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert_eq!(app.md5_name, md5);
        assert!(app.rows.find("SHA1SUMS").is_none(), "not “new”");
        app.open_path(&ctx, dir.0.join("SHA1SUMS"));
        app.settle(&ctx);
        assert_eq!(app.md5_name, "SHA1SUMS");
        assert_eq!(app.algo(), HashMode::Sha1);
        app.request(&ctx, JobKind::Verify);
        finish_run(&ctx, &mut app);
        assert_eq!(app.rows.find("a.txt").unwrap().kind, RowKind::Unchanged);

        app.request(&ctx, JobKind::Generate);
        let confirm = app.confirm.as_ref().expect("the .md5 already exists");
        assert_eq!(confirm.name, md5);
        assert_eq!(confirm.entries, 2);
        assert_eq!(
            confirm.history,
            format!("{}.json", dir.0.file_name().unwrap().to_str().unwrap())
        );
        assert!(confirm.reuse.is_none(), "SHA-1 is no use for MD5");
        app.decide(&ctx, true);
        finish_run(&ctx, &mut app);
        assert_eq!(app.md5_name, md5);
        assert_eq!(
            std::fs::read_to_string(dir.0.join("SHA1SUMS")).unwrap(),
            sums
        );
    }

    /// A folder is read in the background. Until then a run waits with a reason, an outdated result
    /// is discarded, arrow keys in the tree read only after a short pause.
    #[test]
    fn the_preview_loads_in_the_background() {
        let dir = folder_with_md5();
        let other = Scratch::new("vorschau-alt");
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        assert_eq!(app.root.as_ref(), Some(&dir.0), "chosen at once");
        assert!(app.pending.is_some() && app.rows.is_empty());
        assert!(app.status.get().contains("wird gelesen"), "{}", app.status);
        let loading = i18n::t().loading;
        assert_eq!(app.can_start(JobKind::Verify).unwrap_err().get(), loading);
        assert_eq!(app.can_start(JobKind::Generate).unwrap_err().get(), loading);
        app.settle(&ctx);
        assert!(app.md5_exists && app.can_start(JobKind::Verify).is_ok());
        assert!(app.rows.iter().any(|row| row.path == "a.txt"));

        // A replaced request tells its thread that it can stop.
        let stop = Arc::new(AtomicBool::new(false));
        app.pending = Some(Pending {
            due: f64::MAX,
            rx: None,
            stop: Arc::clone(&stop),
        });
        app.select_folder(&ctx, other.0.clone(), true);
        assert!(stop.load(Ordering::Relaxed));
        let early = preview::load(&dir.0, None, true, &stop);
        assert!(
            early.rows.is_empty() && early.timeline.is_err(),
            "ended early"
        );
        app.settle(&ctx);
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);

        // If reading breaks off, a notice says so, with its own reason for the history; nothing
        // stays locked.
        let (tx, rx) = mpsc::channel::<Preview>();
        drop(tx);
        app.pending = Some(Pending {
            due: 0.0,
            rx: Some(rx),
            stop: Arc::new(AtomicBool::new(false)),
        });
        frame(&ctx, &mut app, Vec::new());
        assert!(app.pending.is_none());
        assert!(app
            .notice
            .as_ref()
            .is_some_and(|notice| notice.tone == Tone::Error));
        assert_eq!(
            app.timeline.as_ref().err().map(Say::get).as_deref(),
            Some(i18n::t().unread)
        );
        assert!(app.can_start(JobKind::Verify).is_ok());
        app.notice = None;

        // Arrow keys in the tree: the selection jumps at once, reading happens only after the
        // pause.
        app.pane = Pane::Tree;
        frame(&ctx, &mut app, vec![press(Key::ArrowDown, Modifiers::NONE)]);
        let sub = dir.0.join("sub");
        assert_eq!(app.root.as_ref(), Some(&sub));
        assert!(
            app.pending
                .as_ref()
                .is_some_and(|pending| pending.rx.is_none()),
            "not started yet"
        );
        // The frame says that reading is in progress, instead of "No files".
        frame(&ctx, &mut app, Vec::new());
        app.settle(&ctx);
        assert!(app.pending.is_none());
        assert!(app.rows.iter().any(|row| row.path == "b.txt"));
    }

    /// Frames without changes rebuild neither the visible rows nor the tree.
    #[test]
    fn idle_frames_reuse_the_caches() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        frame(&ctx, &mut app, Vec::new());
        let revision = app.rows.revision();
        let listed = app
            .listed
            .get(&app.rows, app.status_filter, &app.query)
            .as_ptr();
        let items = app.tree.items();
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(app.rows.revision(), revision);
        assert!(std::rc::Rc::ptr_eq(&items, &app.tree.items()));
        let again = app
            .listed
            .get(&app.rows, app.status_filter, &app.query)
            .as_ptr();
        assert_eq!(again, listed, "not collected anew");
    }

    #[test]
    fn verify_without_md5_is_locked_with_a_reason() {
        let dir = Scratch::new("ohne");
        std::fs::write(dir.0.join("a.txt"), b"abc").unwrap();
        let ctx = context(egui::Theme::Light);
        let mut app = VeritasApp::with_settings(Settings::default());
        assert!(app
            .can_start(JobKind::Generate)
            .unwrap_err()
            .get()
            .contains("Ordner wählen"));
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert!(!app.md5_exists);
        assert!(app.can_start(JobKind::Generate).is_ok());
        let reason = app.can_start(JobKind::Verify).unwrap_err().get();
        assert!(reason.contains("noch keine"), "{reason}");
        frame(&ctx, &mut app, vec![press(Key::F5, Modifiers::NONE)]);
        assert!(app.running.is_none());
        let notice = app.notice.as_ref().unwrap();
        assert_eq!(notice.text.get(), reason);
        assert!(notice.tone == Tone::Warn);
    }

    #[test]
    fn arrows_move_through_the_list_and_escape_steps_back() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.pane = Pane::List;
        frame(&ctx, &mut app, vec![press(Key::ArrowDown, Modifiers::NONE)]);
        let first = app.selected.clone().unwrap();
        frame(&ctx, &mut app, vec![press(Key::ArrowDown, Modifiers::NONE)]);
        assert_ne!(app.selected.as_ref(), Some(&first));
        frame(&ctx, &mut app, vec![press(Key::Home, Modifiers::NONE)]);
        assert_eq!(app.selected.as_ref(), Some(&first));
        app.status_filter = Some(RowMark::Open);
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(app.status_filter.is_none());
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(app.selected.is_none());
    }

    /// A click (press and release) at `pos`.
    fn click(pos: Pos2) -> Vec<egui::Event> {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        vec![egui::Event::PointerMoved(pos), button(true), button(false)]
    }

    fn bar_rect(app: &VeritasApp, name: &str) -> Rect {
        app.bar_rects
            .iter()
            .find(|(label, _)| *label == name)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("{name} is missing from the bar: {:?}", app.bar_rects))
    }

    /// At the smallest window size, in the worst state (long .md5 name, HDD, 256 threads, run),
    /// everything in the bar sits side by side, nothing overlaps, nothing sticks out. Only the name
    /// of the checksum file shortens; it keeps 80 px and 5 % reserve (§10). In every language, with
    /// the system's fonts and with the embedded ones as on Linux (Noto Sans).
    #[test]
    fn the_bar_fits_the_smallest_window() {
        let dir = folder_with_md5();
        for lang in Lang::ALL {
            i18n::set(lang);
            for (theme, embedded) in [
                (egui::Theme::Dark, false),
                (egui::Theme::Light, false),
                (egui::Theme::Light, true),
            ] {
                bar_fits(&dir, lang, theme, embedded);
            }
        }
        i18n::set(Lang::De);
    }

    fn bar_fits(dir: &Scratch, lang: Lang, theme: egui::Theme, embedded: bool) {
        let ctx = context(theme);
        if embedded {
            ctx.set_fonts(crate::fonts::embedded_definitions());
        }
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.md5_name = "Urlaubsfotos Sommer 2026 Kroatien und Slowenien.md5".into();
        app.seeks = Some(true);
        app.threads = 256;
        let (_tx, _cancel) = endless_run(&mut app, dir.0.clone());
        let small = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::from(MIN_SIZE))),
            ..Default::default()
        };
        frame_with(&ctx, &mut app, small());
        frame_with(&ctx, &mut app, small());
        let rects = app.bar_rects.clone();
        assert_eq!(rects.len(), 9, "{rects:?}");
        let bar = Rect::from_min_max(
            Pos2::new(space::S3, 0.0),
            Pos2::new(MIN_SIZE[0] - space::S3, theme::TOOLBAR),
        );
        for (name, rect) in &rects {
            assert!(rect.is_positive(), "{lang:?}, {name}: {rect:?}");
            assert!(
                bar.expand(0.5).contains_rect(*rect),
                "{lang:?}: {name} sticks out of the bar: {rect:?}"
            );
        }
        for (index, (a, ra)) in rects.iter().enumerate() {
            for (b, rb) in &rects[index + 1..] {
                let overlap = ra.intersect(*rb);
                assert!(
                    !overlap.is_positive(),
                    "{lang:?}: {a} {ra:?} overlaps {b} {rb:?}"
                );
            }
        }
        // The checksum file keeps enough room for a short name, with 5 % reserve.
        let name = bar_rect(&app, "Prüfsummendatei");
        assert!(
            name.width() >= 80.0 * 1.05,
            "{lang:?} (shorten the buttons “{}”, “{}”), embedded {embedded}: {name:?} \
             {rects:#?}",
            lang.texts().verify,
            lang.texts().generate
        );
        // Controls of a row are equally tall (§4.2).
        for control in [
            "Über",
            "Sprache",
            "Suche",
            "Unterordner",
            "Thema",
            "Prüfen",
            "MD5 erzeugen",
        ] {
            assert_eq!(
                bar_rect(&app, control).height(),
                CONTROL,
                "{control}: {:?}",
                bar_rect(&app, control)
            );
        }
    }

    /// Theme, subfolders and threads are in the bar: a click toggles, Ctrl+, sets the focus into
    /// the *Threads* field, ↑ counts. During a run subfolders and threads are locked (R2), Ctrl+,
    /// says why, and a focus in the field does not take Esc away.
    #[test]
    fn the_bar_holds_the_settings() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Light);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        frame(&ctx, &mut app, Vec::new());
        // Theme: the middle segment is "dark".
        let group = bar_rect(&app, "Thema");
        let moon = Pos2::new(group.left() + 3.0 + CONTROL * 1.5, group.center().y);
        frame(&ctx, &mut app, click(moon));
        assert_eq!(
            ctx.options(|opt| opt.theme_preference),
            egui::ThemePreference::Dark
        );
        // Subfolders: a click turns it off and re-reads the folder.
        assert!(app.recursive);
        let toggle = bar_rect(&app, "Unterordner").center();
        frame(&ctx, &mut app, click(toggle));
        assert!(!app.recursive);
        app.settle(&ctx);
        frame(&ctx, &mut app, click(toggle));
        assert!(app.recursive);
        app.settle(&ctx);
        // Ctrl+, sets the focus into the field, ↑ counts, Esc gives it back.
        app.threads = 4;
        frame(&ctx, &mut app, vec![press(Key::Comma, CTRL)]);
        frame(&ctx, &mut app, Vec::new());
        assert!(ctx.memory(|mem| mem.has_focus(threads_id())));
        frame(&ctx, &mut app, vec![press(Key::ArrowUp, Modifiers::NONE)]);
        assert_eq!(app.threads, 5);
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        frame(&ctx, &mut app, Vec::new());
        assert!(!ctx.memory(|mem| mem.has_focus(threads_id())));
        assert_eq!(app.threads, 5);
        frame(&ctx, &mut app, vec![press(Key::Comma, CTRL)]);
        frame(&ctx, &mut app, Vec::new());
        assert!(ctx.memory(|mem| mem.has_focus(threads_id())));
        // A run begins with the focus in the field: the focus goes, Esc belongs to Veritas again.
        frame(&ctx, &mut app, vec![press(Key::F5, Modifiers::NONE)]);
        assert!(app.running.is_some());
        frame(&ctx, &mut app, Vec::new());
        assert!(!ctx.memory(|mem| mem.has_focus(threads_id())));
        assert!(!ctx.egui_wants_keyboard_input());
        finish_run(&ctx, &mut app);
        // During a run: a click on Subfolders does nothing, Ctrl+, names the reason.
        let (_tx, _cancel) = endless_run(&mut app, dir.0.clone());
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, click(toggle));
        assert!(app.recursive, "locked during a run");
        frame(&ctx, &mut app, vec![press(Key::Comma, CTRL)]);
        frame(&ctx, &mut app, Vec::new());
        assert!(!ctx.memory(|mem| mem.has_focus(threads_id())));
        assert!(app
            .notice
            .as_ref()
            .is_some_and(|notice| notice.text.get() == i18n::t().locked));
    }

    /// Status, checksum, finding and message of every row – what the interim state may change.
    fn marks_of(rows: &Rows) -> Vec<(String, RowKind, Option<String>, Option<String>)> {
        rows.iter()
            .map(|row| {
                (
                    row.path.clone(),
                    row.kind,
                    row.new_hash.md5.clone(),
                    row.message.clone(),
                )
            })
            .collect()
    }

    /// A frame at a point in time (`input.time`).
    fn frame_at(ctx: &egui::Context, app: &mut VeritasApp, time: f64) {
        frame_with(
            ctx,
            app,
            egui::RawInput {
                time: Some(time),
                ..input(Vec::new())
            },
        );
    }

    /// During a run the list shows the interim state of the read files every `LIVE_SECS`, with
    /// counts; rows, selection and scroll position stay. A file without a row is not added. A
    /// cancel restores the list as it was before the run.
    #[test]
    fn a_run_shows_its_progress_in_the_list_and_a_cancel_restores_it() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.selected = Some("a.txt".into());
        frame_at(&ctx, &mut app, 0.0);
        let before = marks_of(&app.rows);
        let offset = app.list_view;
        assert_eq!(app.rows.counts().get(RowMark::Open), 2);
        let (tx, _cancel) = endless_run(&mut app, dir.0.clone());
        let progress = Arc::clone(&app.running.as_ref().unwrap().progress);
        let live = |path: &str, kind, md5: &str| LiveRow {
            path: path.into(),
            kind,
            new_hash: Digests::of(HashMode::Md5, md5.into()),
            message: None,
        };
        progress.push_live(live("a.txt", RowKind::Unchanged, "A1"));
        progress.push_live(live("sub/b.txt", RowKind::Modified, "B2"));
        progress.push_live(live("sub/neu.txt", RowKind::Added, "N3"));

        frame_at(&ctx, &mut app, 1.0);
        assert_eq!(marks_of(&app.rows), before, "not before {LIVE_SECS} s");
        frame_at(&ctx, &mut app, LIVE_SECS + 0.5);
        let shown = |app: &VeritasApp, path: &str| app.rows.find(path).and_then(FileRow::mark);
        assert_eq!(shown(&app, "a.txt"), Some(RowMark::Ok));
        assert_eq!(shown(&app, "sub/b.txt"), Some(RowMark::Wrong));
        assert_eq!(
            app.rows.find("sub/b.txt").unwrap().new_hash.md5.as_deref(),
            Some("B2")
        );
        let counts = app.rows.counts();
        assert_eq!(
            (
                counts.get(RowMark::Open),
                counts.get(RowMark::Ok),
                counts.get(RowMark::Wrong)
            ),
            (0, 1, 1)
        );
        assert_eq!(app.rows.len(), before.len(), "no row is added");
        assert_eq!(app.selected.as_deref(), Some("a.txt"));
        assert_eq!(app.list_view, offset);

        // The next interim state only after another `LIVE_SECS`.
        progress.push_live(LiveRow {
            path: "a.txt".into(),
            kind: RowKind::Error,
            new_hash: Digests::default(),
            message: Some("Access denied".into()),
        });
        frame_at(&ctx, &mut app, LIVE_SECS + 1.0);
        assert_eq!(shown(&app, "a.txt"), Some(RowMark::Ok));
        frame_at(&ctx, &mut app, 2.0 * LIVE_SECS + 1.0);
        assert_eq!(shown(&app, "a.txt"), Some(RowMark::Error));

        // A delete confirmation that got into the run does not change the rows.
        app.trasher = |path| std::fs::remove_file(path).map_err(|err| err.to_string());
        app.delete = Some(Delete {
            path: "a.txt".into(),
            absolute: dir.0.join("a.txt"),
            recorded: true,
        });
        let during = marks_of(&app.rows);
        app.decide_delete(&ctx, true);
        assert!(dir.0.join("a.txt").exists());
        assert_eq!(marks_of(&app.rows), during);

        // Cancelled: everything as before the run.
        let opts = app.scan_options(dir.0.clone(), JobKind::Verify);
        let cancelled = run_scan(&opts, &Progress::new(), &AtomicBool::new(true)).unwrap();
        assert!(cancelled.cancelled);
        tx.send(Ok(cancelled)).unwrap();
        frame_at(&ctx, &mut app, 2.0 * LIVE_SECS + 1.5);
        assert!(app.running.is_none());
        assert_eq!(marks_of(&app.rows), before);
        assert_eq!(app.selected.as_deref(), Some("a.txt"));

        // Finished: afterwards the result applies, not the interim state.
        let (tx, _cancel) = endless_run(&mut app, dir.0.clone());
        let progress = Arc::clone(&app.running.as_ref().unwrap().progress);
        progress.push_live(live("a.txt", RowKind::Modified, "A1"));
        frame_at(&ctx, &mut app, 10.0);
        assert_eq!(shown(&app, "a.txt"), Some(RowMark::Wrong));
        let done = run_scan(&opts, &Progress::new(), &AtomicBool::new(false)).unwrap();
        tx.send(Ok(done)).unwrap();
        frame_at(&ctx, &mut app, 10.5);
        assert!(app.running.is_none());
        assert_eq!(shown(&app, "a.txt"), Some(RowMark::Ok));
        assert_eq!(shown(&app, "sub/b.txt"), Some(RowMark::Ok));
    }

    /// A new run begins with open rows, even if an earlier run had already checked them; at the end
    /// its result applies.
    #[test]
    fn a_new_run_starts_with_open_rows() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        let opts = app.scan_options(dir.0.clone(), JobKind::Verify);
        let done = run_scan(&opts, &Progress::new(), &AtomicBool::new(false)).unwrap();
        app.apply_outcome(&ctx, dir.0.clone(), done);
        assert_eq!(app.rows.counts().get(RowMark::Ok), 2);
        app.run(&ctx, JobKind::Verify);
        let counts = *app.rows.counts();
        assert_eq!((counts.get(RowMark::Ok), counts.get(RowMark::Open)), (0, 2));
        finish_run(&ctx, &mut app);
        assert_eq!(app.rows.counts().get(RowMark::Ok), 2);
    }

    /// The interim state of a real run hits the rows of the preview, even if the `.md5` spells a
    /// subfolder differently from the disk (Windows), and shows there what the result shows.
    #[test]
    fn the_progress_of_a_real_run_lands_on_the_preview_rows() {
        let dir = folder_with_md5();
        let name = format!("{}.md5", dir.0.file_name().unwrap().to_str().unwrap());
        let nested = if FOLD_CASE { "SUB/b.txt" } else { "sub/b.txt" };
        std::fs::write(
            dir.0.join(name),
            format!(
                "900150983CD24FB0D6963F7D28E17F72  a.txt\n\
                 900150983CD24FB0D6963F7D28E17F72  {nested}\n"
            ),
        )
        .unwrap();
        std::fs::write(dir.0.join("a.txt"), b"anders").unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert!(app.rows.find(nested).is_some(), "spelling of the .md5");
        let progress = Progress::new();
        let opts = app.scan_options(dir.0.clone(), JobKind::Verify);
        let outcome = run_scan(&opts, &progress, &AtomicBool::new(false)).unwrap();
        let mut live = Live::default();
        live.apply(&mut app.rows, progress.take_live());
        assert_eq!(
            app.rows.find("a.txt").and_then(FileRow::mark),
            Some(RowMark::Wrong)
        );
        assert_eq!(
            app.rows.find(nested).and_then(FileRow::mark),
            Some(RowMark::Ok)
        );
        for row in app.rows.iter().filter(|row| row.kind != RowKind::Checksum) {
            let key = path_key(&row.path, FOLD_CASE);
            let done = outcome
                .rows
                .iter()
                .find(|done| path_key(&done.path, FOLD_CASE) == key)
                .unwrap();
            assert_eq!((row.kind, &row.new_hash), (done.kind, &done.new_hash));
        }
    }

    /// During a run Esc first undoes filter, search and selection; cancelling happens only with the
    /// second Esc after that, and only while the first one still counts.
    #[test]
    fn escape_cancels_a_run_only_after_everything_else_and_twice() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.pane = Pane::List;
        let (_tx, cancel) = endless_run(&mut app, dir.0.clone());
        app.status_filter = Some(RowMark::Open);
        app.query = "a".into();
        app.selected = Some("a.txt".into());
        let esc = || vec![press(Key::Escape, Modifiers::NONE)];
        frame(&ctx, &mut app, esc());
        assert!(app.status_filter.is_none());
        frame(&ctx, &mut app, esc());
        assert!(app.query.is_empty());
        frame(&ctx, &mut app, esc());
        assert!(app.selected.is_none());
        assert!(!cancel.load(Ordering::Relaxed), "no cancel yet");
        // The first Esc with nothing to undo only announces.
        frame(&ctx, &mut app, esc());
        assert!(app.cancel_armed.is_some());
        assert!(!cancel.load(Ordering::Relaxed));
        frame(&ctx, &mut app, Vec::new());
        // An old Esc no longer counts: it announces anew.
        app.cancel_armed = Some(-2.0 * ARM_SECS);
        frame(&ctx, &mut app, esc());
        assert!(!cancel.load(Ordering::Relaxed));
        frame(&ctx, &mut app, esc());
        assert!(cancel.load(Ordering::Relaxed), "the second Esc cancels");
        assert!(app.cancel_armed.is_none());
    }

    /// Lets a run run in frames until it is finished.
    fn finish_run(ctx: &egui::Context, app: &mut VeritasApp) {
        for _ in 0..500 {
            if app.running.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            frame(ctx, app, Vec::new());
        }
        assert!(app.running.is_none());
    }

    /// After a run the arrow keys belong to the list, even if the tree had them before; ↓ selects a
    /// file and does not change the folder.
    #[test]
    fn after_a_run_the_arrows_belong_to_the_list() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.pane = Pane::Tree;
        frame(&ctx, &mut app, vec![press(Key::F5, Modifiers::NONE)]);
        finish_run(&ctx, &mut app);
        assert!(app.from_run);
        assert!(app.pane == Pane::List);
        frame(&ctx, &mut app, vec![press(Key::ArrowDown, Modifiers::NONE)]);
        assert_eq!(app.root.as_ref(), Some(&dir.0), "the folder stays");
        assert!(app.selected.is_some());
        assert!(app.from_run, "the result stays");
    }

    /// A verify result with a finding stays until Esc; the title shows the progress.
    #[test]
    fn a_finding_stays_until_escape() {
        let dir = folder_with_md5();
        std::fs::write(dir.0.join("a.txt"), b"anders").unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        let (_tx, _cancel) = endless_run(&mut app, dir.0.clone());
        let output = frame_with(&ctx, &mut app, input(Vec::new()));
        assert!(commands(&output).iter().any(
            |command| matches!(command, egui::ViewportCommand::Title(title) if title.contains('%'))
        ));
        app.running = None;
        frame(&ctx, &mut app, vec![press(Key::F5, Modifiers::NONE)]);
        finish_run(&ctx, &mut app);
        assert_eq!(app.title, "Veritas");
        let notice = app.notice.as_ref().unwrap();
        assert!(
            notice.text.get().contains("1 abweichend"),
            "{}",
            notice.text
        );
        assert!(notice.stays());
        app.selected = None;
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(app.notice.is_none());
    }

    /// The recent folders survive a restart; in the empty state Enter opens the last one, 2 the one
    /// before.
    #[test]
    fn recent_folders_open_from_the_empty_state() {
        let first = folder_with_md5();
        let second = Scratch::new("zweiter");
        let ctx = context(egui::Theme::Light);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, first.0.clone(), true);
        app.settle(&ctx);
        app.select_folder(&ctx, second.0.clone(), true);
        app.settle(&ctx);
        app.select_folder(&ctx, first.0.clone(), true);
        app.settle(&ctx);
        assert_eq!(app.recent, vec![first.0.clone(), second.0.clone()]);
        // Arrow keys in the tree open folders only temporarily: they do not count as recent.
        app.pane = Pane::Tree;
        frame(&ctx, &mut app, vec![press(Key::ArrowDown, Modifiers::NONE)]);
        frame(&ctx, &mut app, vec![press(Key::ArrowDown, Modifiers::NONE)]);
        assert_eq!(app.recent, vec![first.0.clone(), second.0.clone()]);
        let mut again = VeritasApp::with_settings(Settings {
            recent: app.recent.clone(),
            ..Settings::default()
        });
        frame(&ctx, &mut again, Vec::new());
        frame(&ctx, &mut again, vec![press(Key::Num2, Modifiers::NONE)]);
        assert_eq!(again.root.as_ref(), Some(&second.0));
        let mut fresh = VeritasApp::with_settings(Settings {
            recent: app.recent.clone(),
            ..Settings::default()
        });
        frame(&ctx, &mut fresh, vec![press(Key::Enter, Modifiers::NONE)]);
        assert_eq!(fresh.root.as_ref(), Some(&first.0));
        // At most three.
        let more: Vec<Scratch> = (0..4).map(|n| Scratch::new(&format!("r{n}"))).collect();
        for dir in &more {
            app.select_folder(&ctx, dir.0.clone(), true);
            app.settle(&ctx);
        }
        assert_eq!(app.recent.len(), RECENT);
        assert_eq!(app.recent[0], more[3].0);
        // A vanished folder drops out of the list; the keys stay with the tree.
        let gone = Scratch::new("weg");
        let mut lost = VeritasApp::with_settings(Settings {
            recent: vec![gone.0.clone(), first.0.clone()],
            ..Settings::default()
        });
        std::fs::remove_dir_all(&gone.0).unwrap();
        frame(&ctx, &mut lost, vec![press(Key::Enter, Modifiers::NONE)]);
        assert!(lost.root.is_none());
        assert!(lost.pane == Pane::Tree);
        assert_eq!(lost.recent, vec![first.0.clone()]);
        assert!(lost
            .notice
            .as_ref()
            .is_some_and(|notice| notice.text.get().contains("gibt es nicht mehr")));
    }

    /// Chip click and Alt+digit go through the same function: the selection stays if the filter
    /// shows it, otherwise it drops.
    #[test]
    fn a_chip_keeps_the_selection_it_shows() {
        let dir = folder_with_md5();
        std::fs::write(dir.0.join("neu.txt"), b"x").unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.selected = Some("a.txt".into());
        let open = app.rows.find("a.txt").and_then(FileRow::mark).unwrap();
        app.toggle_chip(&ctx, open);
        assert_eq!(app.selected.as_deref(), Some("a.txt"), "stays visible");
        app.toggle_chip(&ctx, open);
        assert!(app.status_filter.is_none());
        let new = app.rows.find("neu.txt").and_then(FileRow::mark).unwrap();
        assert_ne!(new, open);
        app.toggle_chip(&ctx, new);
        assert_eq!(app.status_filter, Some(new));
        assert!(app.selected.is_none(), "a.txt is no longer visible");
    }

    /// If a file listed in the `.md5` is deleted while a filter is active, the selection moves to a
    /// visible row. "Is in the .md5" comes from the preview.
    #[test]
    fn deleting_under_a_filter_moves_the_selection() {
        let dir = folder_with_md5();
        std::fs::write(dir.0.join("b.txt"), b"abc").unwrap();
        let name = format!("{}.md5", dir.0.file_name().unwrap().to_str().unwrap());
        std::fs::write(
            dir.0.join(&name),
            "900150983CD24FB0D6963F7D28E17F72  a.txt\n\
             900150983CD24FB0D6963F7D28E17F72  b.txt\n\
             900150983CD24FB0D6963F7D28E17F72  sub/b.txt\n",
        )
        .unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.trasher = |path| std::fs::remove_file(path).map_err(|err| err.to_string());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        let open = app.rows.find("a.txt").and_then(FileRow::mark).unwrap();
        app.toggle_chip(&ctx, open);
        app.selected = Some("a.txt".into());
        app.request_delete(&ctx);
        assert!(app.delete.as_ref().is_some_and(|delete| delete.recorded));
        app.decide_delete(&ctx, true);
        assert_eq!(
            app.rows.find("a.txt").map(|row| row.kind),
            Some(RowKind::Removed)
        );
        assert_eq!(
            app.selected.as_deref(),
            Some("b.txt"),
            "the next visible one"
        );
        assert!(app.selected_position().is_some());
    }

    /// Sets the modification time back by an hour so that reading counts as stable
    /// (`scanning::MTIME_SLACK`).
    fn age(path: &Path) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
    }

    /// The pill counts only what is read; the tooltip names the carried-over checksums.
    #[test]
    fn the_pill_counts_only_files_that_are_read() {
        let progress = Progress::new();
        progress.set_phase("Prüfsummen werden berechnet…");
        progress.set_reused(12_340);
        progress.set_files_total(3);
        progress.inc_files();
        let text = progress_text(&progress.snapshot());
        assert_eq!(
            text,
            "Prüfsummen werden berechnet · 1 von 3 Dateien · 12.340 Prüfsummen übernommen"
        );
    }

    /// Create after Verify carries over the checksums and says so in confirmation and notice;
    /// folder change, toggling subfolders and a changed `.md5` discard them.
    #[test]
    fn generate_after_verify_reuses_the_checksums_and_says_so() {
        let dir = folder_with_md5();
        let other = folder_with_md5();
        age(&dir.0.join("a.txt"));
        age(&dir.0.join("sub").join("b.txt"));
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert!(app.known.is_none(), "Open does not hash");
        // Without a previous run the confirmation says nothing about carrying over.
        app.request(&ctx, JobKind::Generate);
        assert!(app.confirm.take().unwrap().reuse.is_none());

        app.run(&ctx, JobKind::Verify);
        finish_run(&ctx, &mut app);
        assert_eq!(app.known.as_ref().map(|known| known.len()), Some(2));
        app.request(&ctx, JobKind::Generate);
        let asked = app.confirm.as_ref().unwrap();
        let body = confirm_body(i18n::t(), asked);
        assert!(
            body.contains("Übernimmt bis zu 2 Prüfsummen aus der Prüfung von"),
            "{body}"
        );
        assert!(!body.contains("Im Zweifel vorher prüfen"), "{body}");
        app.decide(&ctx, true);
        finish_run(&ctx, &mut app);
        assert!(
            app.status
                .get()
                .contains("2 Prüfsummen übernommen, 0 Dateien gelesen."),
            "{}",
            app.status
        );
        assert_eq!(app.known.as_ref().map(|known| known.len()), Some(2));

        // A changed `.md5` invalidates the prior knowledge.
        let md5 = dir.0.join(&app.md5_name);
        std::fs::write(&md5, "900150983CD24FB0D6963F7D28E17F72  a.txt\n").unwrap();
        app.request(&ctx, JobKind::Generate);
        assert!(app.confirm.take().unwrap().reuse.is_none());
        assert!(app.known.is_none());

        // Toggling subfolders discards it.
        app.run(&ctx, JobKind::Verify);
        finish_run(&ctx, &mut app);
        assert!(app.known.is_some());
        app.recursive = false;
        app.subfolders_changed(&ctx);
        assert!(app.known.is_none());
        app.settle(&ctx);

        // Another folder likewise.
        app.run(&ctx, JobKind::Verify);
        finish_run(&ctx, &mut app);
        assert!(app.known.is_some());
        app.select_folder(&ctx, other.0.clone(), true);
        assert!(app.known.is_none());
        app.settle(&ctx);
    }

    /// The "Replace?" confirmation counts from the preview, after Create from the result – also
    /// entries in subfolders that the list without subfolders does not show.
    #[test]
    fn the_replace_question_counts_without_reading_again() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings {
            recursive: false,
            ..Settings::default()
        });
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert!(app.rows.find("sub/b.txt").is_none());
        app.request(&ctx, JobKind::Generate);
        let asked = app.confirm.take().unwrap();
        assert_eq!((asked.entries, asked.nested), (2, 1));
        // Create with one more file: the numbers come from the result.
        std::fs::write(dir.0.join("c.txt"), b"c").unwrap();
        app.run(&ctx, JobKind::Generate);
        finish_run(&ctx, &mut app);
        assert!(app.from_run, "{}", app.status);
        // If someone read the file now, they would find the same; the confirmation is not needed.
        std::fs::write(dir.0.join(&app.md5_name), "").unwrap();
        app.request(&ctx, JobKind::Generate);
        let asked = app.confirm.take().unwrap();
        assert_eq!((asked.entries, asked.nested), (3, 1));
    }

    /// F8 jumps to the next finding, Shift+F8 back, Alt+digit toggles a chip, Page Down scrolls.
    #[test]
    fn keys_jump_to_findings_chips_and_pages() {
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        let dir = Scratch::new("tasten");
        app.root = Some(dir.0.clone());
        let named = |path: &str, kind, integrity| {
            let mut row = row(kind, "AA", integrity);
            row.path = path.into();
            row
        };
        *app.rows = vec![
            named("a", RowKind::Unchanged, None),
            named("b", RowKind::Modified, Some(IntegrityStatus::Intact)),
            named("c", RowKind::Unchanged, None),
            named("d", RowKind::Removed, None),
            named("e", RowKind::Added, None),
        ];
        let f8 = |mods| vec![press(Key::F8, mods)];
        frame(&ctx, &mut app, f8(Modifiers::NONE));
        assert_eq!(app.selected.as_deref(), Some("b"));
        assert!(app.pane == Pane::List);
        frame(&ctx, &mut app, f8(Modifiers::NONE));
        assert_eq!(app.selected.as_deref(), Some("d"));
        frame(&ctx, &mut app, f8(Modifiers::NONE));
        assert_eq!(
            app.selected.as_deref(),
            Some("b"),
            "wraps around at the end"
        );
        frame(&ctx, &mut app, f8(Modifiers::SHIFT));
        assert_eq!(app.selected.as_deref(), Some("d"), "Shift+F8 backwards");
        // Alt+2 is "ok", once more turns it off; Alt+1 ("open") does not exist here.
        frame(&ctx, &mut app, vec![press(Key::Num2, Modifiers::ALT)]);
        assert_eq!(app.status_filter, Some(RowMark::Ok));
        frame(&ctx, &mut app, vec![press(Key::Num2, Modifiers::ALT)]);
        assert_eq!(app.status_filter, None);
        frame(&ctx, &mut app, vec![press(Key::Num1, Modifiers::ALT)]);
        assert_eq!(app.status_filter, None);
        assert!(app.notice.is_some());
        // Page Down moves more than one row (here to the end), Page Up back.
        app.selected = Some("a".into());
        frame(&ctx, &mut app, vec![press(Key::PageDown, Modifiers::NONE)]);
        assert_eq!(app.selected.as_deref(), Some("e"));
        frame(&ctx, &mut app, vec![press(Key::PageUp, Modifiers::NONE)]);
        assert_eq!(app.selected.as_deref(), Some("a"));
    }

    /// Closing during a run asks via dialog card; Esc lets it keep running, Enter cancels and
    /// closes.
    #[test]
    fn closing_during_a_run_asks_first() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Light);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        let close = || {
            let mut raw = input(Vec::new());
            raw.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    events: vec![egui::ViewportEvent::Close],
                    ..Default::default()
                },
            );
            raw
        };
        // Without a run closing goes through.
        let output = frame_with(&ctx, &mut app, close());
        assert!(!commands(&output).contains(&egui::ViewportCommand::CancelClose));
        assert!(!app.close_ask);

        let (_tx, cancel) = endless_run(&mut app, dir.0.clone());
        let output = frame_with(&ctx, &mut app, close());
        assert!(commands(&output).contains(&egui::ViewportCommand::CancelClose));
        assert!(app.close_ask);
        frame(&ctx, &mut app, Vec::new());
        fits(&ctx, "veritas-close");
        // Esc: let it keep running.
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(!app.close_ask && !cancel.load(Ordering::Relaxed));
        // Close once more, Enter: cancel and close.
        frame_with(&ctx, &mut app, close());
        let output = frame_with(
            &ctx,
            &mut app,
            input(vec![press(Key::Enter, Modifiers::NONE)]),
        );
        assert!(cancel.load(Ordering::Relaxed));
        assert!(commands(&output).contains(&egui::ViewportCommand::Close));
        // The next request goes through, without a new confirmation.
        let output = frame_with(&ctx, &mut app, close());
        assert!(!commands(&output).contains(&egui::ViewportCommand::CancelClose));
        assert!(!app.close_ask);
    }

    /// Esc closes an open context menu and undoes nothing else.
    #[test]
    fn escape_closes_the_context_menu_first() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Light);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        frame(&ctx, &mut app, Vec::new());
        let click = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        // Right-click on the row a.txt: search from the top until it is selected.
        let mut opened = false;
        for y in (40..400).step_by(7) {
            let pos = Pos2::new(600.0, y as f32);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(pos), click(pos, true)],
            );
            frame(&ctx, &mut app, vec![click(pos, false)]);
            if app.selected.as_deref() == Some("a.txt") {
                opened = true;
                break;
            }
        }
        assert!(opened, "row a.txt not hit");
        frame(&ctx, &mut app, Vec::new());
        assert!(egui::Popup::is_any_open(&ctx), "the menu is open");
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        frame(&ctx, &mut app, Vec::new());
        assert!(!egui::Popup::is_any_open(&ctx), "Esc closes the menu");
        assert_eq!(
            app.selected.as_deref(),
            Some("a.txt"),
            "the selection stays"
        );
        // The next Esc belongs to Veritas again.
        frame(&ctx, &mut app, vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(app.selected.is_none());
    }

    /// If the run is already writing, neither Esc nor button nor closing cancels it; closing waits
    /// for it, a run before that only gets the signal.
    #[test]
    fn a_writing_run_is_not_cancelled_but_awaited() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.pane = Pane::List;

        // Before writing: closing only gives the signal and does not wait.
        let (_tx, cancel) = endless_run(&mut app, dir.0.clone());
        app.stop_for_exit(Duration::from_secs(600));
        assert!(cancel.load(Ordering::SeqCst));

        let (tx, cancel) = endless_run(&mut app, dir.0.clone());
        let progress = app
            .running
            .as_ref()
            .map(|r| Arc::clone(&r.progress))
            .unwrap();
        assert!(progress.begin_writing(&cancel));
        let esc = || vec![press(Key::Escape, Modifiers::NONE)];
        frame(&ctx, &mut app, esc());
        frame(&ctx, &mut app, esc());
        assert!(app.cancel_armed.is_none(), "Esc announces nothing");
        assert!(!cancel.load(Ordering::SeqCst));
        app.cancel_scan();
        assert!(!cancel.load(Ordering::SeqCst), "nor does the button");
        assert_eq!(app.status.get(), i18n::t().writing);

        // The close dialog says that it is waiting.
        app.close_ask = true;
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, Vec::new());
        fits(&ctx, "veritas-close");
        app.decide_close(&ctx, true);
        assert!(app.close_confirmed && !cancel.load(Ordering::SeqCst));

        // `on_exit` waits until the run sends its result.
        let sender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            tx.send(Err("fertig".into())).unwrap();
        });
        let start = std::time::Instant::now();
        app.stop_for_exit(Duration::from_secs(600));
        assert!(start.elapsed() < Duration::from_secs(60));
        sender.join().unwrap();
    }

    /// After a failed run the preview re-reads the folder; the error stays in status bar and
    /// notice.
    #[test]
    fn a_failed_run_reloads_the_folder() {
        let dir = folder_with_md5();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        app.rows.clear();
        let (tx, _cancel) = endless_run(&mut app, dir.0.clone());
        tx.send(Err("Platte voll.".into())).unwrap();
        frame(&ctx, &mut app, Vec::new());
        assert!(app.running.is_none());
        // The error is carried by the notice, which stays until it is clicked away. The status bar
        // shows the folder again as soon as the preview is finished – depending on speed, already
        // in this frame.
        assert!(app.notice.as_ref().is_some_and(|notice| {
            notice.tone == Tone::Error && notice.text.get() == "Platte voll."
        }));
        app.settle(&ctx);
        assert!(!app.from_run);
        assert!(app.rows.find("a.txt").is_some(), "read anew");
    }

    /// "Open as JSON" checks like Open that the history is a regular file in the folder. Only the
    /// rejection is tested – otherwise a program would open.
    #[test]
    fn the_history_file_opens_only_as_a_plain_file() {
        let dir = folder_with_md5();
        let name = write_history(&dir.0);
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        assert!(app.timeline.is_ok());
        std::fs::remove_file(dir.0.join(&name)).unwrap();
        std::fs::create_dir(dir.0.join(&name)).unwrap();
        app.open_history_file(&ctx);
        assert!(app.notice.as_ref().is_some_and(|notice| {
            notice.tone == Tone::Warn && notice.text.get().contains("lässt sich nicht öffnen:")
        }));
    }

    /// "Show in folder" shows only regular files in the folder; otherwise it does not call the file
    /// manager at all.
    #[test]
    fn reveal_refuses_paths_outside_the_folder() {
        static SHOWN: AtomicBool = AtomicBool::new(false);
        let dir = folder_with_md5();
        let outside = Scratch::new("draussen");
        std::fs::write(outside.0.join("fremd.txt"), b"x").unwrap();
        let ctx = context(egui::Theme::Dark);
        let mut app = VeritasApp::with_settings(Settings::default());
        app.revealer = |_| {
            SHOWN.store(true, Ordering::Relaxed);
            Ok(())
        };
        app.select_folder(&ctx, dir.0.clone(), true);
        app.settle(&ctx);
        for target in [outside.0.join("fremd.txt"), dir.0.join("sub")] {
            let at = app.rows.position("a.txt").unwrap();
            app.rows[at].absolute = Some(target.clone());
            app.notice = None;
            app.reveal_row(&ctx, "a.txt");
            assert!(!SHOWN.load(Ordering::Relaxed), "{target:?} not shown");
            assert!(app
                .notice
                .as_ref()
                .is_some_and(|notice| notice.text.get().contains("lässt sich nicht zeigen:")));
        }
    }
}

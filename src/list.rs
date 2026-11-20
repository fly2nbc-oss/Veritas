//! The file list: rows with fast access, the visible rows per filter and search, labels, and
//! drawing a row.
//!
//! Derived data (counts, path index, lowercase keys for the search, visible rows) is rebuilt only
//! when rows, filter or search change – not in every frame.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Id, Pos2, Rect, Sense, Stroke, Vec2,
};

use crate::checksum;
use crate::hashing::{Digests, HashMode};
use crate::history::Timeline;
use crate::i18n;
use crate::integrity::IntegrityFinding;
use crate::progress::LiveRow;
use crate::report::{path_key, FOLD_CASE};
use crate::scanning::{FileRow, MarkCounts, RowKind, RowMark};
use crate::theme::{self, icon, paint_fit, palette, space, text, Palette, Tag};
use crate::ui::ROW;

/// Every version of the rows gets a new number, including a completely new list. That way `Listed`
/// knows it has to collect again, and nobody has to remember to tell it.
static REVISION: AtomicU64 = AtomicU64::new(1);

fn next_revision() -> u64 {
    REVISION.fetch_add(1, Ordering::Relaxed)
}

/// The rows of the list. Reading works like a `Vec`; every writing access yields a new version and
/// discards derived data, which is rebuilt on the next read.
pub(crate) struct Rows {
    rows: Vec<FileRow>,
    revision: u64,
    /// Path → first row with this path.
    index: OnceCell<HashMap<String, usize>>,
    counts: OnceCell<MarkCounts>,
    /// Paths in lowercase for the search.
    lower: OnceCell<Vec<String>>,
}

impl Default for Rows {
    fn default() -> Self {
        Self::from(Vec::new())
    }
}

impl From<Vec<FileRow>> for Rows {
    fn from(rows: Vec<FileRow>) -> Self {
        Self {
            rows,
            revision: next_revision(),
            index: OnceCell::new(),
            counts: OnceCell::new(),
            lower: OnceCell::new(),
        }
    }
}

impl Deref for Rows {
    type Target = Vec<FileRow>;

    fn deref(&self) -> &Vec<FileRow> {
        &self.rows
    }
}

impl DerefMut for Rows {
    fn deref_mut(&mut self) -> &mut Vec<FileRow> {
        self.revision = next_revision();
        self.index.take();
        self.counts.take();
        self.lower.take();
        &mut self.rows
    }
}

impl Rows {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// Number of the first row with this path.
    pub(crate) fn position(&self, path: &str) -> Option<usize> {
        self.index
            .get_or_init(|| {
                let mut index = HashMap::with_capacity(self.rows.len());
                for (at, row) in self.rows.iter().enumerate() {
                    index.entry(row.path.clone()).or_insert(at);
                }
                index
            })
            .get(path)
            .copied()
    }

    /// The first row with this path.
    pub(crate) fn find(&self, path: &str) -> Option<&FileRow> {
        self.position(path).map(|at| &self.rows[at])
    }

    /// Rows per status, for chips and message.
    pub(crate) fn counts(&self) -> &MarkCounts {
        self.counts.get_or_init(|| MarkCounts::of(&self.rows))
    }

    /// Changes status, checksums or messages but no path: a new version, the counts are rebuilt;
    /// path index and search keys stay (they depend only on the paths).
    fn edit_marks(&mut self, edit: impl FnOnce(&mut [FileRow])) {
        self.revision = next_revision();
        self.counts.take();
        edit(&mut self.rows);
    }

    fn lower(&self) -> &[String] {
        self.lower.get_or_init(|| {
            self.rows
                .iter()
                .map(|row| row.path.to_lowercase())
                .collect()
        })
    }
}

/// The interim state of a run in the list: the UI picks up the per-file results every few seconds
/// (`Progress::take_live`) and puts them into the existing rows. Rows are neither added nor
/// dropped, the order stays (R1); a file without a row (new in a subfolder) only appears with the
/// result. What was overwritten is remembered per row: a cancelled or failed run restores the list.
#[derive(Default)]
pub(crate) struct Live {
    /// `path_key` → row; built once on the first pickup. The preview reads entries in subfolders
    /// with the spelling of the `.md5`, while the run reports the spelling on disk.
    index: Option<HashMap<String, usize>>,
    /// Per row, its state before the first interim state.
    saved: HashMap<usize, Saved>,
}

/// What an interim state changes on a row.
struct Saved {
    kind: RowKind,
    new_hash: Digests,
    integrity: Option<IntegrityFinding>,
    message: Option<String>,
}

/// A status that only a run assigns and that a new run determines afresh.
fn from_a_run(kind: RowKind) -> bool {
    matches!(
        kind,
        RowKind::Unchanged | RowKind::Modified | RowKind::Error
    )
}

/// Sets the status of a row and, the first time, remembers its state from before.
fn mark(
    saved: &mut HashMap<usize, Saved>,
    at: usize,
    row: &mut FileRow,
    kind: RowKind,
    new_hash: Digests,
    message: Option<String>,
) {
    saved.entry(at).or_insert_with(|| Saved {
        kind: row.kind,
        new_hash: row.new_hash.clone(),
        integrity: row.integrity.clone(),
        message: row.message.clone(),
    });
    row.kind = kind;
    row.new_hash = new_hash;
    row.integrity = None;
    row.message = message;
}

impl Live {
    /// Puts the picked-up results into the rows – one new version per pickup, not per file.
    /// Checksum file, history and entries shown as deleted stay as they are; the latter are only
    /// settled by the result.
    pub(crate) fn apply(&mut self, rows: &mut Rows, items: Vec<LiveRow>) {
        if items.is_empty() {
            return;
        }
        let index = self.index.get_or_insert_with(|| {
            let mut index = HashMap::with_capacity(rows.len());
            for (at, row) in rows.iter().enumerate() {
                index.entry(path_key(&row.path, FOLD_CASE)).or_insert(at);
            }
            index
        });
        let saved = &mut self.saved;
        rows.edit_marks(|rows| {
            for item in items {
                let Some(&at) = index.get(&path_key(&item.path, FOLD_CASE)) else {
                    continue;
                };
                let Some(row) = rows.get_mut(at) else {
                    continue;
                };
                if matches!(
                    row.kind,
                    RowKind::Checksum | RowKind::Report | RowKind::Removed
                ) {
                    continue;
                }
                mark(saved, at, row, item.kind, item.new_hash, item.message);
            }
        });
    }

    /// At the start of a run: every row with the result of an earlier run (ok, mismatch, damaged,
    /// error) counts as open again – or as new if the `.md5` does not list it (`recorded`, per
    /// `path_key`), as in the preview. That way the finished part stands out from the rest. Open
    /// and new already look like that; *deleted* stays as the last known state (the preview says so
    /// too, without reading) until the result settles it. Checksum file and history stay.
    pub(crate) fn reset(&mut self, rows: &mut Rows, recorded: &HashSet<String>) {
        if !rows.iter().any(|row| from_a_run(row.kind)) {
            return;
        }
        let saved = &mut self.saved;
        rows.edit_marks(|rows| {
            for (at, row) in rows.iter_mut().enumerate() {
                if !from_a_run(row.kind) {
                    continue;
                }
                let kind = if recorded.contains(&path_key(&row.path, FOLD_CASE)) {
                    RowKind::Recorded
                } else {
                    RowKind::Added
                };
                mark(saved, at, row, kind, Digests::default(), None);
            }
        });
    }

    /// Resets every changed row to its state before the run.
    pub(crate) fn restore(self, rows: &mut Rows) {
        if self.saved.is_empty() {
            return;
        }
        rows.edit_marks(|rows| {
            for (at, saved) in self.saved {
                if let Some(row) = rows.get_mut(at) {
                    row.kind = saved.kind;
                    row.new_hash = saved.new_hash;
                    row.integrity = saved.integrity;
                    row.message = saved.message;
                }
            }
        });
    }
}

/// The rows that filter and search currently show, as numbers into `Rows` (ascending).
#[derive(Default)]
pub(crate) struct Listed {
    revision: u64,
    filter: Option<RowMark>,
    query: String,
    indices: Vec<usize>,
}

impl Listed {
    /// The visible rows; collected anew only when rows, filter or search have changed.
    pub(crate) fn get(&mut self, rows: &Rows, filter: Option<RowMark>, query: &str) -> &[usize] {
        let query = query.trim();
        if self.revision != rows.revision() || self.filter != filter || self.query != query {
            let needle = query.to_lowercase();
            let lower = if needle.is_empty() {
                &[][..]
            } else {
                rows.lower()
            };
            self.indices = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    in_list(row) && filter.is_none_or(|mark| row.mark() == Some(mark))
                })
                .filter(|(at, _)| needle.is_empty() || lower[*at].contains(&needle))
                .map(|(at, _)| at)
                .collect();
            self.revision = rows.revision();
            self.filter = filter;
            self.query = query.to_string();
        }
        &self.indices
    }
}

/// The chips of the status bar, always all of them and in this order (R1).
pub(crate) const CHIP_MARKS: [RowMark; 7] = [
    RowMark::Open,
    RowMark::Ok,
    RowMark::Wrong,
    RowMark::Damaged,
    RowMark::Deleted,
    RowMark::New,
    RowMark::Error,
];

/// `algo`: the algorithm of the checksum file ("MD5", "SHA-256"), header of the last column.
pub(crate) fn paint_header(ui: &mut egui::Ui, width: f32, algo: &str) {
    let p = palette(ui.ctx());
    let cols = columns(width);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, ROW), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, p.surface_muted);
    let font = FontId::new(text::HEAD, theme::semibold());
    let mut x = rect.left() + space::S2;
    let y = rect.center().y;
    for (index, (label, column)) in header_labels(i18n::t(), algo)
        .into_iter()
        .zip([cols.status, cols.path, cols.size, cols.date, cols.md5])
        .enumerate()
    {
        // "Size" is right-aligned above its numbers.
        let (pos, align) = if index == SIZE_COLUMN {
            (Pos2::new(x + column - space::S3, y), Align2::RIGHT_CENTER)
        } else {
            (Pos2::new(x, y), Align2::LEFT_CENTER)
        };
        ui.painter().text(pos, align, label, font.clone(), p.muted);
        x += column;
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0_f32, p.line),
    );
}

/// A row of the file list. `history` is set if the history names the file: then a clock icon sits
/// at the right of the "File name" column, always in the same place (R1). Returns whether it was
/// clicked.
pub(crate) fn paint_row(
    ui: &mut egui::Ui,
    row: &FileRow,
    selected: bool,
    focused: bool,
    width: f32,
    history: Option<&Timeline>,
) -> (egui::Response, bool) {
    let p = palette(ui.ctx());
    let cols = columns(width);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW), Sense::CLICK);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0_f32, p.line),
    );
    theme::row_ground(ui, rect, selected, focused, response.hovered());
    let mut x = rect.left() + space::S2;
    let y = rect.center().y;
    match row.mark() {
        Some(mark @ (RowMark::Source | RowMark::Report)) => {
            let glyph = if mark == RowMark::Source {
                icon::FILE_CHECK
            } else {
                icon::FILE_JSON
            };
            ui.painter().text(
                Pos2::new(x, y),
                Align2::LEFT_CENTER,
                glyph.to_string(),
                FontId::new(theme::ICON, FontFamily::Proportional),
                if selected { p.accent_strong } else { p.muted },
            );
            ui.painter().text(
                Pos2::new(x + theme::ICON + space::S1, y),
                Align2::LEFT_CENTER,
                own_label(row, mark),
                FontId::new(text::SMALL, FontFamily::Proportional),
                if selected { p.text } else { p.muted },
            );
        }
        Some(mark) => {
            let (fg, bg) = mark_tag(mark, p);
            theme::status_tag(ui, Pos2::new(x, y), mark_label(mark), fg, bg);
        }
        None => {}
    }
    x += cols.status;
    let secondary = if selected { p.text } else { p.muted };
    let font = FontId::new(text::BODY, FontFamily::Proportional);
    let icon_w = if history.is_some() {
        theme::COMPACT + space::S1
    } else {
        0.0
    };
    let truncated = paint_fit(
        ui,
        Pos2::new(x, y),
        &row.path,
        cols.path - space::S2 - icon_w,
        font.clone(),
        p.text,
        true,
    );
    let mut history_clicked = false;
    if let Some(timeline) = history {
        // Click target 24 × 24 (§4.2), always in the same place (R1).
        let spot = Rect::from_center_size(
            Pos2::new(x + cols.path - space::S2 - theme::COMPACT / 2.0, y),
            Vec2::splat(theme::COMPACT),
        );
        let symbol = ui.interact(
            spot,
            Id::unique(("veritas-verlauf-symbol", &row.path)),
            Sense::CLICK,
        );
        let color = if symbol.hovered() || selected {
            p.accent_strong
        } else {
            p.muted
        };
        ui.painter().text(
            spot.center(),
            Align2::CENTER_CENTER,
            icon::HISTORY.to_string(),
            FontId::new(theme::ICON, FontFamily::Proportional),
            color,
        );
        // The text is only built when the tooltip appears, not per row and frame.
        history_clicked = theme::lazy_tip(symbol, || {
            let hint = timeline.hint(&row.path, true).unwrap_or_default();
            format!("{hint}\n{}", i18n::t().history_tip)
        })
        .clicked();
    }
    x += cols.path;
    // Numbers right-aligned, so that the digits line up.
    ui.painter().text(
        Pos2::new(x + cols.size - space::S3, y),
        Align2::RIGHT_CENTER,
        row_size_text(row),
        font.clone(),
        secondary,
    );
    x += cols.size;
    paint_fit(
        ui,
        Pos2::new(x, y),
        &row_date(row),
        cols.date - space::S2,
        font,
        secondary,
        false,
    );
    x += cols.date;
    paint_fit(
        ui,
        Pos2::new(x, y),
        &row_md5(row),
        cols.md5 - space::S2,
        FontId::new(text::MONO, FontFamily::Monospace),
        p.text,
        false,
    );
    let response = if truncated {
        response.on_hover_text(&row.path)
    } else {
        response
    };
    (response, history_clicked)
}

/// The "Size" column in `header_labels`.
const SIZE_COLUMN: usize = 2;

/// The column headers from the left: status, file name, size, modified, algorithm ("MD5").
pub(crate) fn header_labels<'a>(t: &i18n::Texts, algo: &'a str) -> [&'a str; 5] {
    [t.col_status, t.col_name, t.col_size, t.col_modified, algo]
}

pub(crate) struct Cols {
    pub(crate) status: f32,
    pub(crate) path: f32,
    pub(crate) size: f32,
    pub(crate) date: f32,
    pub(crate) md5: f32,
}

pub(crate) fn columns(total: f32) -> Cols {
    let status = 112.0;
    let size = 76.0;
    let date = 128.0;
    let md5 = if total < 760.0 { 200.0 } else { 248.0 };
    let path = (total - status - size - date - md5 - space::S2).max(80.0);
    Cols {
        status,
        path,
        size,
        date,
        md5,
    }
}

/// Cancelled interim states never appear; everything else is in the list.
pub(crate) fn in_list(row: &FileRow) -> bool {
    row.kind != RowKind::Partial
}

/// A status that F8 jumps to: what needs attention after Verify.
pub(crate) fn is_finding(mark: RowMark) -> bool {
    matches!(
        mark,
        RowMark::Wrong | RowMark::Damaged | RowMark::Deleted | RowMark::Error
    )
}

/// The label of a status in the chosen language.
pub(crate) fn mark_label(mark: RowMark) -> &'static str {
    i18n::t().mark(mark)
}

/// The label of an own row: for an SHA file as the source, its algorithm ("sha256") instead of
/// "md5".
fn own_label(row: &FileRow, mark: RowMark) -> &'static str {
    match checksum::algorithm_of_name(&row.path) {
        Some(algo) if mark == RowMark::Source && algo != HashMode::Md5 => algo.id(),
        _ => mark_label(mark),
    }
}

/// Checksum files in the chosen folder (`.md5`, SHA files) are never deleted by Veritas, even if
/// they are in the list – like the own checksum file and the history.
pub(crate) fn protected(row: &FileRow) -> bool {
    matches!(row.kind, RowKind::Checksum | RowKind::Report)
        || (!row.path.contains('/') && checksum::algorithm_of_name(&row.path).is_some())
}

/// Text and fill color of the status label. Open, new and Veritas' own files are neutral; blue
/// stays reserved for the selection (§2.4).
pub(crate) fn mark_tag(mark: RowMark, p: &Palette) -> (Color32, Color32) {
    match mark {
        RowMark::Open | RowMark::Source | RowMark::Report => Tag::Neutral,
        RowMark::New => Tag::New,
        RowMark::Ok => Tag::Ok,
        RowMark::Wrong | RowMark::Deleted => Tag::Warn,
        RowMark::Damaged | RowMark::Error => Tag::Error,
    }
    .colors(p)
}

pub(crate) fn row_size_text(row: &FileRow) -> String {
    if row.kind == RowKind::Removed {
        return String::new();
    }
    row.size.map(i18n::size).unwrap_or_default()
}

pub(crate) fn row_date(row: &FileRow) -> String {
    if row.kind == RowKind::Removed {
        return String::new();
    }
    let Some(modified) = row.modified else {
        return String::new();
    };
    jiff::Timestamp::try_from(modified)
        .map(i18n::date_time)
        .unwrap_or_default()
}

/// The row's checksum, current or from the checksum file (MD5 or SHA).
pub(crate) fn row_md5(row: &FileRow) -> String {
    if row.kind == RowKind::Removed {
        return row.old_hash.shown().cloned().unwrap_or_default();
    }
    row.new_hash
        .shown()
        .or_else(|| row.old_hash.shown())
        .cloned()
        .unwrap_or_default()
}

/// What a row's context menu triggers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowAction {
    Open,
    History,
    Reveal,
    CopyMd5,
    Delete,
}

/// Context menu of a row: Open, History, Move to Recycle Bin – with key, locked with a reason.
/// `algo` names the algorithm when copying ("Copy SHA-256").
pub(crate) fn row_menu(
    ui: &mut egui::Ui,
    row: &FileRow,
    history_ok: bool,
    algo: &str,
) -> Option<RowAction> {
    let t = i18n::t();
    let own = matches!(row.kind, RowKind::Checksum | RowKind::Report);
    let gone = row.kind == RowKind::Removed;
    let gone_why = gone.then_some(t.why_gone);
    let mut picked = None;
    ui.set_min_width(theme::MENU_W);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let mut item = |ui: &mut egui::Ui, label: &str, keys: &str, why: Option<&str>, what| {
        if theme::menu_row(ui, label, keys, why).clicked() && why.is_none() {
            picked = Some(what);
        }
    };
    let open_label = if row.kind == RowKind::Report {
        t.menu_history
    } else {
        t.menu_open
    };
    item(ui, open_label, t.key_enter, gone_why, RowAction::Open);
    if !own {
        item(
            ui,
            t.menu_file_history,
            &t.ctrl("H"),
            (!history_ok).then_some(t.why_no_history),
            RowAction::History,
        );
    }
    item(ui, t.menu_reveal, "", gone_why, RowAction::Reveal);
    let md5_why = row_md5(row).is_empty().then(|| (t.why_no_md5)(algo));
    if !own {
        item(
            ui,
            &(t.menu_copy)(algo),
            "",
            md5_why.as_deref(),
            RowAction::CopyMd5,
        );
    }
    theme::menu_separator(ui);
    let why = if protected(row) {
        Some(t.delete_own)
    } else {
        gone_why
    };
    item(ui, t.menu_trash, t.key_delete, why, RowAction::Delete);
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashing::Digests;

    fn row(path: &str, kind: RowKind) -> FileRow {
        FileRow {
            path: path.into(),
            absolute: None,
            kind,
            size: None,
            modified: None,
            old_hash: Digests::default(),
            new_hash: Digests::default(),
            integrity: None,
            message: None,
        }
    }

    /// The cache follows rows, filter and search – even if the rows are changed from outside
    /// without anybody saying so.
    #[test]
    fn listed_follows_rows_filter_and_query() {
        let mut rows = Rows::from(vec![
            row("A.jpg", RowKind::Unchanged),
            row("b.txt", RowKind::Added),
            row("c.txt", RowKind::Partial),
        ]);
        let mut listed = Listed::default();
        assert_eq!(listed.get(&rows, None, ""), &[0, 1]);
        assert_eq!(listed.get(&rows, Some(RowMark::New), ""), &[1]);
        assert_eq!(listed.get(&rows, None, " a.JPG "), &[0]);
        assert_eq!(rows.counts().get(RowMark::Ok), 1);
        assert_eq!(rows.position("b.txt"), Some(1));
        rows.push(row("a2.jpg", RowKind::Unchanged));
        assert_eq!(listed.get(&rows, None, "a"), &[0, 3]);
        assert_eq!(rows.counts().get(RowMark::Ok), 2);
        rows.retain(|row| row.path != "A.jpg");
        assert_eq!(rows.position("b.txt"), Some(0));
        assert_eq!(listed.get(&rows, None, "a"), &[2]);
        // A completely new list counts as new even if it has the same length.
        rows = Rows::from(vec![row("x", RowKind::Removed); 3]);
        assert_eq!(listed.get(&rows, None, ""), &[0, 1, 2]);
        assert_eq!(rows.position("x"), Some(0), "the first row with the path");
    }

    /// At the start of a run, results of an earlier run go back to open (or new, if the `.md5` does
    /// not list the file); deleted, new and the own files stay. A cancel restores every row
    /// exactly, even after an interim state.
    #[test]
    fn a_new_run_starts_open_and_a_cancel_restores_every_row() {
        let md5 = |hex: &str| Digests::of(HashMode::Md5, hex.into());
        let mut damaged = row("bad.jpg", RowKind::Modified);
        damaged.new_hash = md5("B2");
        damaged.integrity = Some(IntegrityFinding {
            status: crate::integrity::IntegrityStatus::PossiblyDamaged,
            reason: "EOI".into(),
        });
        let mut ok = row("ok.txt", RowKind::Unchanged);
        ok.new_hash = md5("O1");
        let mut broken = row("weg.txt", RowKind::Error);
        broken.message = Some("Access denied".into());
        let mut rows = Rows::from(vec![
            damaged,
            row("gone.txt", RowKind::Removed),
            row("neu.txt", RowKind::Added),
            ok,
            row("sum.md5", RowKind::Checksum),
            broken,
        ]);
        let snapshot = |rows: &Rows| -> Vec<_> {
            rows.iter()
                .map(|row| {
                    (
                        row.kind,
                        row.new_hash.clone(),
                        row.integrity.clone(),
                        row.message.clone(),
                    )
                })
                .collect()
        };
        let before = snapshot(&rows);
        let recorded: HashSet<String> = ["bad.jpg", "gone.txt", "ok.txt"]
            .iter()
            .map(|path| path_key(path, FOLD_CASE))
            .collect();
        let mut live = Live::default();
        live.reset(&mut rows, &recorded);
        let kinds: Vec<RowKind> = rows.iter().map(|row| row.kind).collect();
        assert_eq!(
            kinds,
            [
                RowKind::Recorded,
                RowKind::Removed,
                RowKind::Added,
                RowKind::Recorded,
                RowKind::Checksum,
                RowKind::Added,
            ]
        );
        assert!(rows
            .iter()
            .all(|row| row.new_hash.md5.is_none() && row.integrity.is_none()));
        assert_eq!(rows.counts().get(RowMark::Open), 2);
        live.apply(
            &mut rows,
            vec![LiveRow {
                path: "ok.txt".into(),
                kind: RowKind::Unchanged,
                new_hash: md5("O1"),
                message: None,
            }],
        );
        assert_eq!(rows.counts().get(RowMark::Ok), 1);
        live.restore(&mut rows);
        assert_eq!(snapshot(&rows), before);
    }

    /// Veritas never deletes checksum files in the folder – including an SHA file that is listed as
    /// an ordinary file in the `.md5`; the source shows its algorithm.
    #[test]
    fn checksum_files_are_protected_and_labelled() {
        for (path, kind, expected) in [
            ("SHA256SUMS", RowKind::Unchanged, true),
            ("x.sha1", RowKind::Recorded, true),
            ("alt.md5", RowKind::Added, true),
            ("Fotos.json", RowKind::Report, true),
            ("sub/SHA256SUMS", RowKind::Unchanged, false),
            ("bild.jpg", RowKind::Unchanged, false),
        ] {
            assert_eq!(protected(&row(path, kind)), expected, "{path}");
        }
        let source = |path: &str| own_label(&row(path, RowKind::Checksum), RowMark::Source);
        assert_eq!(source("sha512sums.txt"), "sha512");
        assert_eq!(source("Fotos.md5"), mark_label(RowMark::Source));
        let mut sha = row("a.txt", RowKind::Recorded);
        sha.old_hash = Digests::of(HashMode::Sha1, "AB".into());
        assert_eq!(row_md5(&sha), "AB");
        assert_eq!(
            header_labels(i18n::t(), HashMode::Sha256.label())[4],
            "SHA-256"
        );
    }
}

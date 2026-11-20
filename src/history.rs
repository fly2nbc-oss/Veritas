//! The history as a card (R8): all runs from `{folder name}.json`, the newest on top, and for
//! each run which files it names. If a file is selected, the second tab shows its entries
//! across all runs. The clock icon on a file that the history names, `Ctrl+H` and a double
//! click on the "Verlauf" row open the card – there is deliberately no button in the bar.
//! `Esc`, `Ctrl+H` and a click outside close it.
//!
//! The history is read once when the folder is opened and after each run, never per frame.
//! It stays in storage order (oldest first); the card shows it reversed.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Label, Layout, Pos2, RichText,
    ScrollArea, Sense, Stroke, Vec2,
};

use crate::format::local_time;
use crate::hashing::Digests;
use crate::i18n::{self, Lang, Msg, Texts};
use crate::integrity::IntegrityStatus;
use crate::report::{path_key, Action, ChangeReport, History, FOLD_CASE};
use crate::scanning::RowMark;
use crate::theme::{
    self, icon, paint_fit, palette, radius, space, text, Palette, Role, Tag, CONTROL,
};

/// Width of the card; narrower if the window requires it.
const CARD_W: f32 = 920.0;
/// Maximum height of the card; it stays the same height no matter which run is selected (R1).
const CARD_H: f32 = 680.0;
/// Column with the runs.
const RUNS_W: f32 = 300.0;
/// Row of the run list (§4.2).
const RUN_ROW: f32 = 32.0;
/// Row of the file table; like the file list 28 instead of 32 (exception to §4.2).
const ENTRY_ROW: f32 = 28.0;
/// Column for the status tag, as in the file list.
pub(crate) const TAG_W: f32 = 112.0;

/// What a run records about a file. Verify reports, create takes over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Verify: the checksum differs from the .md5.
    Differs,
    /// The checksum differs, and the structure may be defective.
    Damaged,
    /// Create: the new checksum has been in the .md5 since then.
    Changed,
    /// Verify: in the folder, but not in the .md5.
    New,
    /// Create: newly added to the .md5.
    Added,
    /// Verify: in the .md5, but missing from the folder.
    Missing,
    /// Create: removed from the .md5.
    Dropped,
    /// Not readable.
    Unreadable,
}

impl Change {
    /// The label in the chosen language.
    pub fn label(self) -> &'static str {
        self.label_in(i18n::t())
    }

    /// What verify reports is named as in the file list; only create has its own words.
    pub fn label_in(self, t: &Texts) -> &'static str {
        match self {
            Self::Differs => t.mark(RowMark::Wrong),
            Self::Damaged => t.mark(RowMark::Damaged),
            Self::Changed => t.change_changed,
            Self::New => t.mark(RowMark::New),
            Self::Added => t.change_added,
            Self::Missing => t.mark(RowMark::Deleted),
            Self::Dropped => t.change_dropped,
            Self::Unreadable => t.mark(RowMark::Error),
        }
    }

    /// The same color pairs as the tags of the file list: what verify finds is yellow or
    /// red, “neu” violet; what create takes over (also “aufgenommen”) is neutral – after that
    /// the file is in the .md5 and no longer new.
    pub fn colors(self, p: &Palette) -> (Color32, Color32) {
        match self {
            Self::Differs | Self::Missing => Tag::Warn,
            Self::Damaged | Self::Unreadable => Tag::Error,
            Self::New => Tag::New,
            Self::Changed | Self::Added | Self::Dropped => Tag::Neutral,
        }
        .colors(p)
    }
}

/// A file in a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub change: Change,
    /// Checksum before, from the .md5; empty if there was none.
    pub old: String,
    /// Checksum after, as read; empty if there is none.
    pub new: String,
    /// Reason of the structure check or error message, as stored in the history (translated for
    /// display by `i18n::shown`).
    pub note: String,
}

fn hex(digests: &Digests) -> String {
    digests.shown().cloned().unwrap_or_default()
}

/// A run of “MD5 erzeugen”; otherwise “Prüfen”.
fn generated(run: &ChangeReport) -> bool {
    run.action == Action::Created
}

/// All entries of a run, sorted by path.
pub fn entries(run: &ChangeReport) -> Vec<Entry> {
    let made = generated(run);
    let mut out = Vec::with_capacity(
        run.modified.len() + run.added.len() + run.removed.len() + run.errors.len(),
    );
    for item in &run.modified {
        let change = if item.integrity_status == IntegrityStatus::PossiblyDamaged {
            Change::Damaged
        } else if made {
            Change::Changed
        } else {
            Change::Differs
        };
        out.push(Entry {
            path: item.path.clone(),
            change,
            old: hex(&item.old_hash),
            new: hex(&item.new_hash),
            note: item.integrity_reason.clone(),
        });
    }
    for item in &run.added {
        out.push(Entry {
            path: item.path.clone(),
            change: if made { Change::Added } else { Change::New },
            old: String::new(),
            new: hex(&item.hash),
            note: String::new(),
        });
    }
    for item in &run.removed {
        out.push(Entry {
            path: item.path.clone(),
            change: if made {
                Change::Dropped
            } else {
                Change::Missing
            },
            old: hex(&item.hash),
            new: String::new(),
            note: String::new(),
        });
    }
    for item in &run.errors {
        out.push(Entry {
            path: item.path.clone(),
            change: Change::Unreadable,
            old: item.old_hash.as_ref().map(hex).unwrap_or_default(),
            new: String::new(),
            note: item.message.clone(),
        });
    }
    out.sort_by(|left, right| left.path.cmp(&right.path));
    out
}

/// Time of a run in local time, like the “Geändert” column of the file list.
fn when(t: &Texts, run: &ChangeReport) -> String {
    local(run, t.date_time)
}

/// Only the day of a run.
fn day(t: &Texts, run: &ChangeReport) -> String {
    local(run, t.date)
}

fn local(run: &ChangeReport, format: &str) -> String {
    run.timestamp
        .parse::<jiff::Timestamp>()
        .map(|time| local_time(time, format))
        .unwrap_or_else(|_| run.timestamp.clone())
}

/// “MD5 erzeugt” (the first), “MD5 ersetzt” or “Geprüft”.
fn action_label(t: &Texts, run: &ChangeReport) -> &'static str {
    match (generated(run), run.baseline) {
        (true, true) => t.action_created,
        (true, false) => t.action_replaced,
        (false, _) => t.action_verified,
    }
}

fn short_action(t: &Texts, run: &ChangeReport) -> &'static str {
    match (generated(run), run.baseline) {
        (true, true) => t.short_created,
        (true, false) => t.short_replaced,
        (false, _) => t.short_verified,
    }
}

/// Short, for the run list: “13 Einträge”, “alles stimmt”, “68 Dateien”.
fn run_note(t: &Texts, run: &ChangeReport) -> String {
    if run.cancelled {
        return t.run_cancelled.into();
    }
    if run.baseline {
        return (t.files)(run.file_count as usize);
    }
    match run.modified.len() + run.added.len() + run.removed.len() + run.errors.len() {
        0 if generated(run) => t.no_change.into(),
        0 => t.all_ok.into(),
        count => (t.entries)(count),
    }
}

/// The numbers of a run in one sentence.
fn facts(t: &Texts, run: &ChangeReport) -> String {
    if run.baseline {
        return (t.first_facts)(run.file_count as usize);
    }
    let made = generated(run);
    let mut parts = vec![
        (t.files)(run.file_count as usize),
        (t.unchanged_count)(&i18n::count_in(t, run.unchanged_count as usize)),
    ];
    for (count, change) in [
        (
            run.modified.len(),
            if made {
                Change::Changed
            } else {
                Change::Differs
            },
        ),
        (
            run.added.len(),
            if made { Change::Added } else { Change::New },
        ),
        (
            run.removed.len(),
            if made {
                Change::Dropped
            } else {
                Change::Missing
            },
        ),
        (run.errors.len(), Change::Unreadable),
    ] {
        if count > 0 {
            parts.push((t.counted)(change, count));
        }
    }
    if run.cancelled {
        parts.push(t.run_cancelled.into());
    }
    parts.join(" · ")
}

/// The finding of a run in the run list: red if something was possibly damaged or
/// unreadable; yellow if verify found a mismatch or a missing file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finding {
    Damaged,
    Differs,
}

impl Finding {
    fn of(run: &ChangeReport) -> Option<Self> {
        let damaged = run
            .modified
            .iter()
            .any(|item| item.integrity_status == IntegrityStatus::PossiblyDamaged);
        if damaged || !run.errors.is_empty() {
            Some(Self::Damaged)
        } else if !generated(run) && !(run.modified.is_empty() && run.removed.is_empty()) {
            Some(Self::Differs)
        } else {
            None
        }
    }

    /// Icon, color and word (R9 – never color alone).
    fn flag(self, p: &Palette) -> (char, Color32, &'static str) {
        let t = i18n::t();
        match self {
            Self::Damaged => (icon::CIRCLE_ALERT, p.error, t.flag_damaged),
            Self::Differs => (icon::TRIANGLE_ALERT, p.warn, t.flag_differs),
        }
    }
}

/// What the card shows for a run, computed once on load instead of in every frame.
struct RunView {
    /// All entries, sorted by path (`entries`).
    entries: Vec<Entry>,
    finding: Option<Finding>,
}

impl RunView {
    fn of(run: &ChangeReport) -> Self {
        Self {
            entries: entries(run),
            finding: Finding::of(run),
        }
    }
}

/// The texts of a run in one language – computed once per language, when the card or the
/// status bar needs them for the first time, not in every frame.
struct RunText {
    /// Only the day, for the status bar and the page of a file.
    day: String,
    /// Time in local time, for the run list.
    when: String,
    /// “12.10.2026 19:02   ·   MD5 ersetzt”
    title: String,
    /// On the right in the run list: “ersetzt · 2 Einträge”.
    note: String,
    /// The numbers in one sentence.
    facts: String,
}

impl RunText {
    fn of(t: &Texts, run: &ChangeReport) -> Self {
        let when = when(t, run);
        Self {
            day: day(t, run),
            title: format!("{when}   ·   {}", action_label(t, run)),
            when,
            note: format!("{} · {}", short_action(t, run), run_note(t, run)),
            facts: facts(t, run),
        }
    }
}

/// The history of a folder and, per file, the runs that name it.
pub struct Timeline {
    /// File name of the history, e.g. `Holiday.json`.
    pub file_name: String,
    history: History,
    /// Per run, what the card shows; same order as `history.runs`.
    views: Vec<RunView>,
    /// Per language (position in `Lang::ALL`) the texts of the runs, only when needed.
    texts: [OnceLock<Vec<RunText>>; Lang::ALL.len()],
    /// Key by `path_key` → (run, entry in its `entries`), oldest first.
    by_file: HashMap<String, Vec<(usize, usize)>>,
}

impl Timeline {
    /// Reads the history. The error says why there is none – for the tooltip (R2).
    pub fn load(path: &Path) -> Result<Self, String> {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = match crate::checksum::read_limited(path, crate::checksum::MAX_HISTORY_BYTES) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(Msg::NoHistory.fill(&[&name]));
            }
            Err(err) => return Err(Msg::Unreadable.fill(&[&name, &err.to_string()])),
        };
        let (history, ours) = History::from_text(&text);
        if !ours {
            return Err(Msg::NotOurHistory.fill(&[&name]));
        }
        Self::new(name, history)
    }

    pub fn new(file_name: String, history: History) -> Result<Self, String> {
        if history.runs.is_empty() {
            return Err(Msg::HistoryEmpty.fill(&[&file_name]));
        }
        let views: Vec<RunView> = history.runs.iter().map(RunView::of).collect();
        let mut by_file: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        for (index, view) in views.iter().enumerate() {
            for (at, entry) in view.entries.iter().enumerate() {
                by_file
                    .entry(path_key(&entry.path, FOLD_CASE))
                    .or_default()
                    .push((index, at));
            }
        }
        Ok(Self {
            file_name,
            history,
            views,
            texts: Default::default(),
            by_file,
        })
    }

    /// The texts of the runs in the chosen language.
    fn texts(&self) -> &[RunText] {
        let lang = i18n::current();
        let at = Lang::ALL
            .iter()
            .position(|known| *known == lang)
            .unwrap_or(0);
        let t = lang.texts();
        self.texts
            .get(at)
            .map(|cell| {
                cell.get_or_init(|| {
                    self.history
                        .runs
                        .iter()
                        .map(|run| RunText::of(t, run))
                        .collect()
                })
                .as_slice()
            })
            .unwrap_or_default()
    }

    pub fn runs(&self) -> &[ChangeReport] {
        &self.history.runs
    }

    /// The entries of a run, sorted by path.
    pub fn entries(&self, run: usize) -> &[Entry] {
        self.views
            .get(run)
            .map(|view| view.entries.as_slice())
            .unwrap_or_default()
    }

    /// The entries of a file across all runs, oldest first, as (run, entry in
    /// `entries`). On Windows ignoring case, like the list.
    pub fn events(&self, path: &str) -> &[(usize, usize)] {
        self.by_file
            .get(&path_key(path, FOLD_CASE))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// An entry from `events`.
    pub fn entry(&self, run: usize, at: usize) -> Option<&Entry> {
        self.entries(run).get(at)
    }

    /// For the status bar: the last entry of the file. Without an entry, and if it is in the
    /// .md5 (`recorded`): since when the history knows of no change.
    pub fn hint(&self, path: &str, recorded: bool) -> Option<String> {
        let t = i18n::t();
        let texts = self.texts();
        let events = self.events(path);
        let Some(&(run, at)) = events.last() else {
            return recorded
                .then(|| texts.first())
                .flatten()
                .map(|first| (t.hint_none)(&first.day));
        };
        let last = self.entry(run, at)?;
        let day = &texts.get(run)?.day;
        Some(match events.len() {
            1 => (t.hint_last)(last.change.label_in(t), day),
            count => (t.hint_last_many)(last.change.label_in(t), day, count),
        })
    }
}

/// The tabs of the card.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Runs,
    File,
}

/// What the card shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    pub tab: Tab,
    /// Selected run as an index in storage order (oldest first).
    pub run: usize,
    /// The file of the second tab.
    pub file: Option<String>,
    /// Scroll the selected run into view in the next frame.
    pub reveal: bool,
    /// Scroll position of the run list in the last frame; with it `reveal` also scrolls to a run
    /// that the list is not currently drawing. `None` as long as this card has not yet
    /// drawn the list – otherwise egui remembers the position of a previously opened card.
    runs_offset: Option<f32>,
}

impl View {
    /// The newest run is selected; with a file, its tab comes first.
    pub fn new(runs: usize, file: Option<String>) -> Self {
        Self {
            tab: if file.is_some() { Tab::File } else { Tab::Runs },
            run: runs.saturating_sub(1),
            file,
            reveal: true,
            runs_offset: None,
        }
    }

    /// ↑ selects the newer run, ↓ the older – the list has the newest on top.
    pub fn step(&mut self, newer: bool, runs: usize) {
        if runs == 0 {
            return;
        }
        self.run = if newer {
            (self.run + 1).min(runs - 1)
        } else {
            self.run.saturating_sub(1)
        };
        self.tab = Tab::Runs;
        self.reveal = true;
    }

    /// Switches the tab if there is a file.
    pub fn switch(&mut self) {
        if self.file.is_some() {
            self.tab = match self.tab {
                Tab::Runs => Tab::File,
                Tab::File => Tab::Runs,
            };
            self.reveal = true;
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CardOutput {
    pub close: bool,
    /// Opens the history in the program the system provides for JSON.
    pub open_file: bool,
}

/// The card over the whole window.
pub fn overlay(ctx: &egui::Context, timeline: &Timeline, view: &mut View) -> CardOutput {
    let mut out = CardOutput::default();
    view.run = view.run.min(timeline.runs().len().saturating_sub(1));
    if view.file.is_none() {
        view.tab = Tab::Runs;
    }
    let screen = ctx.content_rect();
    let width = (screen.width() - 2.0 * space::S6).clamp(320.0, CARD_W);
    let height = (screen.height() - 2.0 * space::S6).clamp(260.0, CARD_H);
    let inner_w = width - 2.0 * space::S6;
    // Header, tabs, footer and spacing; the rest belongs to the content.
    let body_h =
        (height - 2.0 * space::S6 - CONTROL - theme::TAB_H - CONTROL - 3.0 * space::S3).max(120.0);
    let (outside, ()) = theme::card(ctx, "veritas-history", |ui| {
        ui.set_width(inner_w);
        header(ui, timeline, &mut out);
        ui.add_space(space::S3);
        let t = i18n::t();
        let mut labels = vec![t.tab_runs.to_string()];
        if let Some(file) = &view.file {
            labels.push((t.tab_file)(file_label(file)));
        }
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let at = usize::from(view.tab == Tab::File);
        if let Some(index) = theme::tabs(ui, "veritas-history-tab", &labels, at, inner_w, "← / →")
        {
            view.tab = if index == 1 { Tab::File } else { Tab::Runs };
        }
        ui.add_space(space::S3);
        let size = Vec2::new(inner_w, body_h);
        ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
            ui.set_min_size(size);
            match (view.tab, view.file.clone()) {
                (Tab::File, Some(file)) => file_page(ui, timeline, &file, view),
                _ => runs_page(ui, timeline, view, body_h),
            }
        });
        ui.add_space(space::S3);
        footer(ui, timeline, view, &mut out);
    });
    out.close |= outside;
    out
}

/// The file name without folder, for the tab.
fn file_label(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// “Verlauf”, file name and number of runs, the × on the right.
fn header(ui: &mut egui::Ui, timeline: &Timeline, out: &mut CardOutput) {
    let p = palette(ui.ctx());
    let t = i18n::t();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(space::S3, 0.0);
        ui.label(
            RichText::new(t.history_title)
                .size(text::VALUE)
                .family(theme::semibold())
                .color(p.text),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::icon_button(ui, icon::X, theme::CLOSE)
                .on_hover_text(t.close_esc)
                .clicked()
            {
                out.close = true;
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(
                    Label::new(
                        RichText::new(format!(
                            "{}   ·   {}",
                            timeline.file_name,
                            (t.runs)(timeline.runs().len())
                        ))
                        .size(text::BODY)
                        .color(p.muted),
                    )
                    .truncate(),
                );
            });
        });
    });
}

/// The runs on the left, the selected one on the right.
fn runs_page(ui: &mut egui::Ui, timeline: &Timeline, view: &mut View, height: f32) {
    let p = palette(ui.ctx());
    let total = ui.available_width();
    let list_w = RUNS_W.min(total * 0.4);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let size = Vec2::new(list_w, height);
        ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
            ui.set_min_size(size);
            run_list(ui, timeline, view, list_w, height);
        });
        let (gap, _) = ui.allocate_exact_size(Vec2::new(space::S4, height), Sense::hover());
        ui.painter()
            .vline(gap.center().x, gap.y_range(), Stroke::new(1.0_f32, p.line));
        let rest = (total - list_w - space::S4).max(120.0);
        let size = Vec2::new(rest, height);
        ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
            ui.set_min_size(size);
            run_details(ui, timeline, view, rest);
        });
    });
}

/// The runs, newest on top. Only the visible rows are drawn.
fn run_list(ui: &mut egui::Ui, timeline: &Timeline, view: &mut View, width: f32, height: f32) {
    let p = palette(ui.ctx());
    let count = timeline.views.len();
    let texts = timeline.texts();
    let mut scroll = ScrollArea::vertical()
        .id_salt("veritas-history-runs")
        .max_height(height)
        .auto_shrink([false, false]);
    if view.reveal && count > 0 {
        // The selected run is at position `count - 1 - run`; scroll only until it is
        // fully visible.
        let top = (count - 1 - view.run.min(count - 1)) as f32 * RUN_ROW;
        match view.runs_offset {
            // First frame of this card: the run at the top, i.e. the newest right at the top.
            None => scroll = scroll.vertical_scroll_offset(top),
            Some(offset) if top < offset => scroll = scroll.vertical_scroll_offset(top),
            Some(offset) if top + RUN_ROW > offset + height => {
                scroll = scroll.vertical_scroll_offset(top + RUN_ROW - height);
            }
            Some(_) => {}
        }
    }
    // The row spacing counts toward the row height in `show_rows`.
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let output = scroll.show_rows(ui, RUN_ROW, count, |ui, range| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for position in range {
            let index = count - 1 - position;
            let run = &timeline.views[index];
            let Some(text) = texts.get(index) else {
                continue;
            };
            let selected = index == view.run;
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(width - space::S2, RUN_ROW), Sense::CLICK);
            if selected {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(radius::MD), p.accent_subtle);
            } else if response.hovered() {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(radius::MD), p.hover);
            }
            let y = rect.center().y;
            let flag = run.finding.map(|finding| finding.flag(p));
            if let Some((glyph, color, _)) = flag {
                ui.painter().text(
                    Pos2::new(rect.left() + space::S2, y),
                    Align2::LEFT_CENTER,
                    glyph.to_string(),
                    FontId::new(theme::ICON, FontFamily::Proportional),
                    color,
                );
            }
            ui.painter().text(
                Pos2::new(rect.left() + space::S2 + theme::ICON + space::S2, y),
                Align2::LEFT_CENTER,
                &text.when,
                FontId::new(text::BODY, FontFamily::Proportional),
                p.text,
            );
            ui.painter().text(
                Pos2::new(rect.right() - space::S2, y),
                Align2::RIGHT_CENTER,
                &text.note,
                FontId::new(text::SMALL, FontFamily::Proportional),
                if selected { p.text } else { p.muted },
            );
            let response = match flag {
                Some((_, _, word)) => response.on_hover_text(word),
                None => response,
            };
            if response.clicked() {
                view.run = index;
            }
        }
    });
    view.runs_offset = Some(output.state.clamped_offset().y);
    view.reveal = false;
}

fn run_details(ui: &mut egui::Ui, timeline: &Timeline, view: &mut View, width: f32) {
    let p = palette(ui.ctx());
    let t = i18n::t();
    let (Some(run), Some(shown), Some(text)) = (
        timeline.runs().get(view.run),
        timeline.views.get(view.run),
        timeline.texts().get(view.run),
    ) else {
        return;
    };
    ui.spacing_mut().item_spacing = Vec2::new(0.0, space::S1);
    ui.add(
        Label::new(
            RichText::new(&text.title)
                .size(text::VALUE)
                .family(theme::semibold())
                .color(p.text),
        )
        .truncate(),
    );
    ui.add(Label::new(RichText::new(&text.facts).size(text::BODY).color(p.muted)).wrap());
    for warning in &run.warnings {
        ui.add(
            Label::new(
                RichText::new(i18n::shown(warning))
                    .size(text::SMALL)
                    .color(p.muted),
            )
            .wrap(),
        );
    }
    ui.add_space(space::S3);
    let list = &shown.entries;
    if list.is_empty() {
        let note = if run.baseline {
            t.baseline_note
        } else if generated(run) {
            t.generate_none
        } else {
            t.verify_none
        };
        ui.add(Label::new(RichText::new(note).size(text::BODY).color(p.text)).wrap());
        return;
    }
    let mut picked = None;
    ScrollArea::vertical()
        .id_salt(("veritas-history-entries", view.run))
        .auto_shrink([false, false])
        .show_rows(ui, ENTRY_ROW, list.len(), |ui, range| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for entry in &list[range] {
                if entry_row(ui, entry, width - space::S2).clicked() {
                    picked = Some(entry.path.clone());
                }
            }
        });
    if let Some(path) = picked {
        view.file = Some(path);
        view.tab = Tab::File;
    }
}

/// A row of the file table: tag, path, reason. The checksums are in the tooltip.
fn entry_row(ui: &mut egui::Ui, entry: &Entry, width: f32) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ENTRY_ROW), Sense::CLICK);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0_f32, p.line),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(radius::MD), p.hover);
    }
    let y = rect.center().y;
    let (fg, bg) = entry.change.colors(p);
    theme::status_tag(
        ui,
        Pos2::new(rect.left() + space::S2, y),
        entry.change.label(),
        fg,
        bg,
    );
    let note = i18n::shown(&entry.note);
    let note_w = if note.is_empty() {
        0.0
    } else {
        (width * 0.35).min(260.0)
    };
    let path_w = width - space::S2 - TAG_W - note_w - space::S2;
    paint_fit(
        ui,
        Pos2::new(rect.left() + space::S2 + TAG_W, y),
        &entry.path,
        path_w,
        FontId::new(text::BODY, FontFamily::Proportional),
        p.text,
        true,
    );
    if note_w > 0.0 {
        paint_fit(
            ui,
            Pos2::new(rect.right() - note_w - space::S2, y),
            &note,
            note_w,
            FontId::new(text::SMALL, FontFamily::Proportional),
            p.muted,
            false,
        );
    }
    // The text is built only when the tooltip appears.
    theme::lazy_tip(response, || {
        let t = i18n::t();
        let mut tip = vec![entry.path.clone()];
        if !entry.old.is_empty() {
            tip.push(format!("{}  {}", t.before, entry.old));
        }
        if !entry.new.is_empty() {
            tip.push(format!("{}  {}", t.after, entry.new));
        }
        if !note.is_empty() {
            tip.push(note);
        }
        tip.push(t.entry_tip.into());
        tip.join("\n")
    })
}

/// All entries of a file, newest on top. A click on the date shows the run.
fn file_page(ui: &mut egui::Ui, timeline: &Timeline, file: &str, view: &mut View) {
    let p = palette(ui.ctx());
    let t = i18n::t();
    let texts = timeline.texts();
    let events = timeline.events(file);
    ui.spacing_mut().item_spacing = Vec2::new(0.0, space::S1);
    ui.add(
        Label::new(
            RichText::new(file)
                .size(text::VALUE)
                .family(theme::semibold())
                .color(p.text),
        )
        .truncate(),
    );
    let since = texts
        .first()
        .map(|first| first.day.as_str())
        .unwrap_or_default();
    let summary = match events.len() {
        0 => (t.file_none)(since),
        count => (t.file_events)(count, since),
    };
    ui.add(Label::new(RichText::new(summary).size(text::BODY).color(p.muted)).wrap());
    ui.add_space(space::S3);
    let mut picked = None;
    ScrollArea::vertical()
        .id_salt("veritas-history-file")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(space::S2, space::S1);
            for &(index, at) in events.iter().rev() {
                let (Some(entry), Some(run)) = (timeline.entry(index, at), texts.get(index)) else {
                    continue;
                };
                ui.horizontal(|ui| {
                    let (tag, _) = ui.allocate_exact_size(Vec2::new(TAG_W, 24.0), Sense::hover());
                    let (fg, bg) = entry.change.colors(p);
                    theme::status_tag(
                        ui,
                        Pos2::new(tag.left(), tag.center().y),
                        entry.change.label(),
                        fg,
                        bg,
                    );
                    // Clickable, so in the link tone (`accent-strong`) and with a hand pointer.
                    let date = ui
                        .add(
                            Label::new(
                                RichText::new(&run.title)
                                    .size(text::BODY)
                                    .color(p.accent_strong),
                            )
                            .sense(Sense::CLICK)
                            .selectable(false),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(t.show_run);
                    if date.clicked() {
                        picked = Some(index);
                    }
                });
                for (label, value) in [(t.before, &entry.old), (t.after, &entry.new)] {
                    if value.is_empty() {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(TAG_W);
                        ui.label(RichText::new(label).size(text::SMALL).color(p.muted));
                        ui.label(
                            RichText::new(value)
                                .size(text::MONO)
                                .family(FontFamily::Monospace)
                                .color(p.text),
                        );
                    });
                }
                if !entry.note.is_empty() {
                    let note = i18n::shown(&entry.note);
                    ui.horizontal(|ui| {
                        ui.add_space(TAG_W);
                        ui.add(
                            Label::new(RichText::new(note).size(text::SMALL).color(p.muted)).wrap(),
                        );
                    });
                }
                ui.add_space(space::S2);
            }
        });
    if let Some(index) = picked {
        view.run = index;
        view.tab = Tab::Runs;
        view.reveal = true;
    }
}

/// Keys on the left, “Als JSON öffnen” on the right.
fn footer(ui: &mut egui::Ui, timeline: &Timeline, view: &View, out: &mut CardOutput) {
    let p = palette(ui.ctx());
    let t = i18n::t();
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::button(
                ui,
                Role::Secondary,
                Some(icon::FILE_JSON),
                t.open_json,
                0.0,
                CONTROL,
            )
            .on_hover_text((t.open_json_tip)(&timeline.file_name))
            .clicked()
            {
                out.open_file = true;
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                // Key hints uniform: “key effect”, separated by “ · ”.
                let keys = match (view.tab, view.file.is_some()) {
                    (Tab::File, _) => t.keys_file,
                    (Tab::Runs, true) => t.keys_runs_file,
                    (Tab::Runs, false) => t.keys_runs,
                };
                ui.add(Label::new(RichText::new(keys).size(text::SMALL).color(p.muted)).truncate());
            });
        });
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::report::{ErrorEntry, ModifiedEntry, PathEntry};

    fn md5(hex: &str) -> Digests {
        crate::hashing::Digests::of(crate::hashing::HashMode::Md5, hex.into())
    }

    fn run(timestamp: &str, action: Action, baseline: bool) -> ChangeReport {
        ChangeReport {
            timestamp: timestamp.into(),
            base_directory: "/x".into(),
            hash_mode: "md5".into(),
            action,
            baseline,
            unchanged_count: 0,
            file_count: 3,
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

    fn modified(path: &str, status: IntegrityStatus) -> ModifiedEntry {
        ModifiedEntry {
            path: path.into(),
            old_hash: md5("AA"),
            new_hash: md5("BB"),
            integrity_status: status,
            integrity_reason: "Grund".into(),
            size: None,
        }
    }

    /// Created, replaced (one file changed, one taken over), verified (one differs and
    /// is damaged, one is missing, one is unreadable).
    pub(crate) fn sample() -> History {
        let first = run("2025-09-05T17:08:35+00:00", Action::Created, true);
        let mut second = run("2025-09-13T21:24:52+00:00", Action::Created, false);
        second
            .modified
            .push(modified("IMG.JPG", IntegrityStatus::Intact));
        second.added.push(PathEntry {
            path: "neu.txt".into(),
            hash: md5("CC"),
            size: None,
        });
        let mut third = run("2026-10-15T20:39:08+00:00", Action::Verified, false);
        third
            .modified
            .push(modified("img.jpg", IntegrityStatus::PossiblyDamaged));
        third.removed.push(PathEntry {
            path: "weg.txt".into(),
            hash: md5("DD"),
            size: None,
        });
        third.errors.push(ErrorEntry {
            path: "zu.txt".into(),
            message: "Zugriff verweigert".into(),
            old_hash: Some(md5("EE")),
        });
        History::new(vec![first, second, third])
    }

    #[test]
    fn what_a_run_says_depends_on_the_action() {
        let history = sample();
        let replaced: Vec<_> = entries(&history.runs[1])
            .iter()
            .map(|entry| (entry.path.clone(), entry.change))
            .collect();
        assert_eq!(
            replaced,
            vec![
                ("IMG.JPG".to_string(), Change::Changed),
                ("neu.txt".to_string(), Change::Added),
            ]
        );
        let checked = entries(&history.runs[2]);
        let changes: Vec<_> = checked.iter().map(|entry| entry.change).collect();
        assert_eq!(
            changes,
            vec![Change::Damaged, Change::Missing, Change::Unreadable]
        );
        // The error entry carries the old hash, the missing one its stored hash.
        assert_eq!(checked[2].old, "EE");
        assert_eq!(checked[1].old, "DD");
        let t = Lang::De.texts();
        assert_eq!(action_label(t, &history.runs[0]), "MD5 erzeugt");
        assert_eq!(action_label(t, &history.runs[1]), "MD5 ersetzt");
        assert_eq!(action_label(t, &history.runs[2]), "Geprüft");
        assert_eq!(run_note(t, &history.runs[0]), "3 Dateien");
        assert_eq!(run_note(t, &history.runs[1]), "2 Einträge");
        assert_eq!(
            facts(t, &history.runs[2]),
            "3 Dateien · 0 unverändert · 1 abweichend · 1 gelöscht · 1 Fehler"
        );
        assert!(Finding::of(&history.runs[1]).is_none());
        assert_eq!(Finding::of(&history.runs[2]), Some(Finding::Damaged));
        assert_eq!(
            Finding::Damaged.flag(&crate::theme::DARK).1,
            crate::theme::DARK.error
        );
    }

    #[test]
    fn a_file_gathers_its_entries_over_all_runs() {
        let t = Lang::De.texts();
        let timeline = Timeline::new("x.json".into(), sample()).unwrap();
        let events = timeline.events("img.jpg");
        if FOLD_CASE {
            // On Windows, IMG.JPG and img.jpg are the same file.
            assert_eq!(events.len(), 2);
            assert_eq!(events[0].0, 1);
            let (run, at) = events[1];
            assert_eq!(timeline.entry(run, at).unwrap().change, Change::Damaged);
            assert_eq!(timeline.entries(2)[at].path, "img.jpg");
            assert_eq!(
                timeline.hint("Img.jpg", true).unwrap(),
                format!(
                    "Verlauf: beschädigt am {} (2 Einträge)",
                    day(t, &timeline.runs()[2])
                )
            );
        } else {
            assert_eq!(events.len(), 1);
        }
        assert_eq!(
            timeline.hint("neu.txt", true).unwrap(),
            format!("Verlauf: aufgenommen am {}", day(t, &timeline.runs()[1]))
        );
        // Without an entry: unchanged since the first run – but only what is in the .md5.
        assert_eq!(
            timeline.hint("alt.txt", true).unwrap(),
            format!(
                "Verlauf: keine Änderung seit {}",
                day(t, &timeline.runs()[0])
            )
        );
        assert_eq!(timeline.hint("alt.txt", false), None);
    }

    /// A history from Veritas before the language choice: German values for `action` and
    /// `integrity_status`, German reasons, messages and warnings.
    const GERMAN_HISTORY: &str = r#"{
  "format": "veritas-verlauf-1",
  "runs": [
    {
      "timestamp": "2026-10-12T17:02:00+00:00",
      "base_directory": "/medien/Urlaub",
      "hash_mode": "md5",
      "action": "erzeugt",
      "baseline": true,
      "unchanged_count": 0,
      "file_count": 3,
      "modified": [], "added": [], "removed": [], "errors": [],
      "skipped_symlinks": 0, "skipped_non_regular": 0, "cancelled": false,
      "warnings": []
    },
    {
      "timestamp": "2026-10-15T20:39:08+00:00",
      "base_directory": "/medien/Urlaub",
      "hash_mode": "md5",
      "action": "geprüft",
      "baseline": false,
      "unchanged_count": 1,
      "file_count": 3,
      "modified": [{
        "path": "IMG_1.JPG",
        "old_hash": {"md5": "AA"}, "new_hash": {"md5": "BB"},
        "integrity_status": "möglicherweise beschädigt",
        "integrity_reason": "JPEG: EOI (FF D9) fehlt am Dateiende"
      }],
      "added": [],
      "removed": [],
      "errors": [{"path": "zu.txt", "message": "Keine Leseberechtigung: zu.txt"}],
      "skipped_symlinks": 0, "skipped_non_regular": 0, "cancelled": false,
      "warnings": ["Ein freier alter Text ohne Vorlage"]
    }
  ]
}"#;

    /// An old German history is read and shown in each language in that language; a free text
    /// without a recognisable template stays as it is.
    #[test]
    fn an_old_german_history_is_shown_in_the_chosen_language() {
        let (history, ours) = History::from_text(GERMAN_HISTORY);
        assert!(ours, "a German history is ours");
        assert_eq!(history.runs[0].action, Action::Created);
        assert_eq!(history.runs[1].action, Action::Verified);
        assert_eq!(
            history.runs[1].modified[0].integrity_status,
            IntegrityStatus::PossiblyDamaged
        );
        let timeline = Timeline::new("Urlaub.json".into(), history.clone()).unwrap();
        let checked = entries(&history.runs[1]);
        assert_eq!(checked[0].change, Change::Damaged);
        assert_eq!(checked[1].change, Change::Unreadable);
        for lang in [Lang::En, Lang::Fr] {
            let t = lang.texts();
            assert_eq!(
                i18n::shown_in(t, &checked[0].note),
                (t.msg)(Msg::JpegNoEoi),
                "{lang:?}"
            );
            assert_eq!(
                i18n::shown_in(t, &checked[1].note),
                (t.msg)(Msg::ReadDeniedPlain).replace("{}", "zu.txt"),
                "{lang:?}"
            );
            assert_eq!(action_label(t, &history.runs[0]), t.action_created);
            assert_eq!(action_label(t, &history.runs[1]), t.action_verified);
            assert_eq!(checked[0].change.label_in(t), t.mark(RowMark::Damaged));
            let said = facts(t, &history.runs[1]);
            assert!(said.contains(&(t.files)(3)), "{lang:?}: {said}");
            assert!(!said.contains("Dateien"), "{lang:?}: {said}");
            assert_eq!(
                i18n::shown_in(t, &history.runs[1].warnings[0]),
                "Ein freier alter Text ohne Vorlage"
            );
            // The timeline admits the same runs in the language that currently applies.
            i18n::set(lang);
            assert_eq!(timeline.runs().len(), 2);
            assert_eq!(
                i18n::shown(&timeline.entry(1, 0).unwrap().note),
                (t.msg)(Msg::JpegNoEoi)
            );
        }
        i18n::set(Lang::De);
        // German shows the old text as it was stored.
        let de = Lang::De.texts();
        assert_eq!(
            i18n::shown_in(de, &checked[0].note),
            "JPEG: EOI (FF D9) fehlt am Dateiende"
        );
    }

    /// An old German and a new English run in one file: both are read. When written anew, all
    /// fixed values are English; free texts stay as they are.
    #[test]
    fn a_mixed_history_is_read_and_written_back_in_english() {
        let (mut history, ours) = History::from_text(GERMAN_HISTORY);
        assert!(ours);
        let mut newer = history.runs[1].clone();
        newer.timestamp = "2026-11-20T09:00:00+00:00".into();
        newer.modified[0].integrity_reason = Msg::JpegNoEoi.en().into();
        history.push(newer);
        let bytes = history.to_pretty_json().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let (again, ours) = History::from_text(&text);
        assert!(ours);
        assert_eq!(again.runs.len(), 3);
        assert_eq!(again.format, crate::report::HISTORY_FORMAT);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["runs"][0]["action"], "created");
        assert_eq!(value["runs"][1]["action"], "verified");
        assert_eq!(
            value["runs"][1]["modified"][0]["integrity_status"],
            "possibly damaged"
        );
        for german in ["\"erzeugt\"", "\"geprüft\"", "möglicherweise beschädigt"] {
            assert!(!text.contains(german), "{german} in {text}");
        }
        // The old reason stays stored in German, the new one in English; both are shown in the
        // interface language.
        assert_eq!(
            again.runs[1].modified[0].integrity_reason,
            "JPEG: EOI (FF D9) fehlt am Dateiende"
        );
        let en = Lang::En.texts();
        for run in &again.runs[1..] {
            assert_eq!(
                i18n::shown_in(en, &run.modified[0].integrity_reason),
                Msg::JpegNoEoi.en()
            );
        }
        let de = Lang::De.texts();
        assert_eq!(
            i18n::shown_in(de, &again.runs[2].modified[0].integrity_reason),
            Msg::JpegNoEoi.de()
        );
    }

    #[test]
    fn missing_or_foreign_history_says_why() {
        let dir = std::env::temp_dir().join(format!("veritas-verlauf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Urlaub.json");
        let _ = std::fs::remove_file(&path);
        let err = Timeline::load(&path).err().unwrap();
        assert!(err.starts_with("No history yet"), "{err}");
        std::fs::write(&path, "{\"etwas\": 1}").unwrap();
        assert_eq!(
            Timeline::load(&path).err().unwrap(),
            "Urlaub.json is not a Veritas history."
        );
        std::fs::write(&path, sample().to_pretty_json().unwrap()).unwrap();
        assert_eq!(Timeline::load(&path).unwrap().runs().len(), 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The run list draws only what is visible – and still scrolls to a run that
    /// it is not currently drawing.
    #[test]
    fn the_run_list_scrolls_to_a_run_it_does_not_draw() {
        let mut history = sample();
        let template = history.runs[0].clone();
        history.runs = vec![template; 200];
        let timeline = Timeline::new("x.json".into(), history).unwrap();
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut view = View::new(200, None);
        let draw = |view: &mut View| {
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1180.0, 820.0),
                    )),
                    ..Default::default()
                };
                let mut output = ctx.run_ui(input, |ui| {
                    overlay(ui.ctx(), &timeline, view);
                });
                output.textures_delta.clear();
            }
        };
        let offset = |view: &View| view.runs_offset.unwrap_or(-1.0);
        draw(&mut view);
        assert_eq!(offset(&view), 0.0, "the newest is on top");
        view.run = 0;
        view.reveal = true;
        draw(&mut view);
        assert!(offset(&view) > 150.0 * RUN_ROW, "{}", offset(&view));
        view.step(true, 200);
        draw(&mut view);
        assert!(
            offset(&view) > 150.0 * RUN_ROW,
            "one step does not scroll far"
        );
        // A newly opened card shows the newest run again, regardless of what the previous card
        // showed.
        let mut fresh = View::new(200, None);
        draw(&mut fresh);
        assert_eq!(offset(&fresh), 0.0);
        view.run = 199;
        view.reveal = true;
        draw(&mut view);
        assert_eq!(offset(&view), 0.0);
    }

    #[test]
    fn arrows_walk_the_runs_newest_first() {
        let mut view = View::new(3, None);
        assert_eq!((view.run, view.tab), (2, Tab::Runs));
        view.step(true, 3);
        assert_eq!(view.run, 2);
        view.step(false, 3);
        view.step(false, 3);
        view.step(false, 3);
        assert_eq!(view.run, 0);
        view.switch();
        assert_eq!(view.tab, Tab::Runs, "ohne Datei gibt es nur einen Reiter");
        let mut view = View::new(3, Some("a.txt".into()));
        assert_eq!(view.tab, Tab::File);
        view.switch();
        assert_eq!(view.tab, Tab::Runs);
    }
}

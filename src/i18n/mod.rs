//! Interface languages: German, English, French, Spanish and Italian – following the pattern
//! of Cerno (design system §10).
//!
//! Each language is a `Texts` literal; a forgotten text is a compile error. Texts with values
//! are functions, so that each language chooses its own word order. The chosen language
//! applies to the whole program (`set`, `current`), read in every frame.
//!
//! What ends up in files does not depend on the language: the history writes English values
//! (`Msg`, `report::Action`, `IntegrityStatus`) and keeps reading the German ones of older
//! histories; `veritas.log` and `crash.log` are English; the `.md5` has no texts. The interface
//! shows stored texts in the chosen language via `shown`.
//!
//! Tests: without `set`, German applies. `set` takes effect in tests only in its own thread,
//! because the tests run in parallel in one process.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::history::Change;

mod de;
mod en;
mod es;
mod fr;
mod it;
mod msg;

pub use msg::Msg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
    Fr,
    Es,
    It,
}

impl Lang {
    /// Order in the language menu and for `Ctrl+L`.
    pub const ALL: [Lang; 5] = [Self::De, Self::En, Self::Fr, Self::Es, Self::It];

    pub fn code(self) -> &'static str {
        match self {
            Self::De => "de",
            Self::En => "en",
            Self::Fr => "fr",
            Self::Es => "es",
            Self::It => "it",
        }
    }

    /// `de`, `de-DE`, `de_CH.UTF-8` → German.
    pub fn from_code(code: &str) -> Option<Self> {
        let primary = code.split(['-', '_', '.']).next()?.to_ascii_lowercase();
        Self::ALL.into_iter().find(|lang| lang.code() == primary)
    }

    /// The name of the language in itself.
    pub fn name(self) -> &'static str {
        match self {
            Self::De => "Deutsch",
            Self::En => "English",
            Self::Fr => "Français",
            Self::Es => "Español",
            Self::It => "Italiano",
        }
    }

    /// The next language (`Ctrl+L`), after the last one the first again.
    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|lang| *lang == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn texts(self) -> &'static Texts {
        match self {
            Self::De => &de::TEXTS,
            Self::En => &en::TEXTS,
            Self::Fr => &fr::TEXTS,
            Self::Es => &es::TEXTS,
            Self::It => &it::TEXTS,
        }
    }
}

/// The chosen language as a position in `Lang::ALL`; German at the start, until
/// `VeritasApp::new` sets it.
static CURRENT: AtomicU8 = AtomicU8::new(0);

#[cfg(test)]
thread_local! {
    /// In tests: the language of this thread (see module).
    static LOCAL: std::cell::Cell<Option<Lang>> = const { std::cell::Cell::new(None) };
}

pub fn current() -> Lang {
    #[cfg(test)]
    if let Some(lang) = LOCAL.with(std::cell::Cell::get) {
        return lang;
    }
    Lang::ALL
        .get(usize::from(CURRENT.load(Ordering::Relaxed)))
        .copied()
        .unwrap_or(Lang::De)
}

pub fn set(lang: Lang) {
    #[cfg(test)]
    LOCAL.with(|local| local.set(Some(lang)));
    #[cfg(not(test))]
    CURRENT.store(
        Lang::ALL
            .iter()
            .position(|known| *known == lang)
            .unwrap_or(0) as u8,
        Ordering::Relaxed,
    );
}

/// The texts of the chosen language.
pub fn t() -> &'static Texts {
    current().texts()
}

/// The system language, if Veritas speaks it; otherwise English. Only at the first start,
/// after that the stored one applies.
pub fn system_default() -> Lang {
    sys_locale::get_locale()
        .as_deref()
        .and_then(Lang::from_code)
        .unwrap_or(Lang::En)
}

/// A text that comes into being only when shown, in the chosen language – for everything that
/// stays longer (status bar, notices, reasons): after a language change it speaks the new
/// language without anyone having to rebuild it.
#[derive(Clone)]
pub struct Say(Arc<dyn Fn(&Texts) -> String + Send + Sync>);

impl Say {
    pub fn new(text: impl Fn(&Texts) -> String + Send + Sync + 'static) -> Self {
        Self(Arc::new(text))
    }

    /// A fixed text of a language, e.g. `Say::of(|t| t.locked)`.
    pub fn of(text: fn(&Texts) -> &'static str) -> Self {
        Self::new(move |t| text(t).to_string())
    }

    /// A stored text from a run or preview (`Msg`), displayed via `shown`.
    pub fn stored(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::new(move |t| msg::shown_in(t, &text))
    }

    /// The text in the chosen language.
    pub fn get(&self) -> String {
        self.in_lang(t())
    }

    pub fn in_lang(&self, t: &Texts) -> String {
        (self.0)(t)
    }
}

/// A finished text – if it is a stored one (`Msg`), it appears translated.
impl From<&str> for Say {
    fn from(text: &str) -> Self {
        Self::stored(text)
    }
}

impl From<String> for Say {
    fn from(text: String) -> Self {
        Self::stored(text)
    }
}

/// The text in the chosen language.
impl std::fmt::Display for Say {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.get())
    }
}

impl std::fmt::Debug for Say {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Say({:?})", self.get())
    }
}

/// A stored text from a run, preview or history in the chosen language (`Msg`).
pub fn shown(text: &str) -> String {
    msg::shown_in(t(), text)
}

/// The same in the language of `t`.
pub fn shown_in(t: &Texts, text: &str) -> String {
    msg::shown_in(t, text)
}

/// Number with the thousands separator `sep`: "1.234", "1,234", "1 234".
pub fn number(value: usize, sep: &str) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 * sep.len());
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push_str(sep);
        }
        out.push(ch);
    }
    out
}

/// Number in the chosen language.
pub fn count(value: usize) -> String {
    count_in(t(), value)
}

pub fn count_in(t: &Texts, value: usize) -> String {
    number(value, t.thousands)
}

/// Size in the chosen language: binary steps (1 KB = 1024 B) like Explorer, one decimal,
/// non-breaking space before the unit (§10): "1,5 MB", "1.5 MB", "1,5 Mo".
pub fn size(bytes: u64) -> String {
    size_in(t(), bytes)
}

pub fn size_in(t: &Texts, bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < t.size_units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let name = t.size_units.get(unit).copied().unwrap_or_default();
    if unit == 0 {
        format!("{}\u{a0}{name}", count_in(t, bytes as usize))
    } else {
        let text = format!("{value:.1}").replace('.', &t.decimal.to_string());
        format!("{text}\u{a0}{name}")
    }
}

/// Throughput of the progress pill: "735 MB/s" (French "Mo/s").
pub fn speed(mb_per_sec: f64) -> String {
    let unit = t().size_units.get(2).copied().unwrap_or("MB");
    format!("{mb_per_sec:.0}\u{a0}{unit}/s")
}

/// Remaining time: "42 s", "3:05 min", "1:02 h" – the same in all five languages.
pub fn eta(left: Duration) -> String {
    let secs = left.as_secs();
    if secs < 60 {
        format!("{secs}\u{a0}s")
    } else if secs < 3600 {
        format!("{}:{:02}\u{a0}min", secs / 60, secs % 60)
    } else {
        format!("{}:{:02}\u{a0}h", secs / 3600, (secs % 3600) / 60)
    }
}

/// Date and time in local time, as the language writes it (`Texts::date_time`).
pub fn date_time(time: jiff::Timestamp) -> String {
    crate::format::local_time(time, t().date_time)
}

/// Only the time, "14:02" – the same in all five languages.
pub fn clock(time: jiff::Timestamp) -> String {
    crate::format::local_time(time, "%H:%M")
}

/// "2 abweichend, 1 neu", "2 mismatches, 1 new" – only the parts whose count is greater than
/// 0, each in the form that fits the number (`Texts::counted`).
pub fn count_parts_in(t: &Texts, parts: &[(u64, Change)]) -> String {
    parts
        .iter()
        .filter(|(value, _)| *value > 0)
        .map(|(value, change)| (t.counted)(*change, *value as usize))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A message of the system in the chosen language where its kind is known; otherwise the text
/// of the system.
pub fn plain_io(err: &std::io::Error) -> String {
    let t = t();
    match err.kind() {
        std::io::ErrorKind::NotFound => t.sys_not_found.into(),
        std::io::ErrorKind::PermissionDenied => t.sys_denied.into(),
        _ => (t.sys_message)(&err.to_string()),
    }
}

/// All texts of a language. Each field says where the text appears and, for fixed room, how
/// long it may be. Texts with values are functions; the values come in the order of the
/// parameters. Each language decides the plural itself (`files`, `entries`, `counted` …).
/// Non-breaking spaces (U+00A0): between number and unit, before `%`, in French also before
/// `:`, `;`, `?`, `!` and inside « ».
pub struct Texts {
    // Numbers, sizes, date.
    /// Date and time (strftime of jiff), column "Geändert" (128 px) and history.
    pub date_time: &'static str,
    /// Only the day (strftime).
    pub date: &'static str,
    /// Thousands separator (".", ",", narrow non-breaking space U+202F …).
    pub thousands: &'static str,
    /// Decimal separator in sizes ("1,5 MB").
    pub decimal: char,
    /// Units: byte, kilobyte, megabyte, gigabyte, terabyte (French o, Ko, Mo …).
    pub size_units: [&'static str; 5],
    /// Percent in the window title during a run, "42 %".
    pub percent: fn(u32) -> String,
    /// "1 Datei", "1.234 Dateien" – the number with `thousands`.
    pub files: fn(usize) -> String,
    /// "1 Prüfsumme", "3 Prüfsummen".
    pub checksums: fn(usize) -> String,
    /// "1 Eintrag", "3 Einträge" (entries of the `.md5` or of the history).
    pub entries: fn(usize) -> String,
    /// "1 Lauf", "3 Läufe" (header of the history card).
    pub runs: fn(usize) -> String,

    // Key names as they appear on the keyboards of the language.
    /// Strg / Ctrl.
    pub key_ctrl: &'static str,
    /// Eingabe / Enter / Entrée / Intro / Invio.
    pub key_enter: &'static str,
    /// Entf / Del / Suppr / Supr / Canc.
    pub key_delete: &'static str,

    // Bar. Buttons with text are the only ones whose width the language decides: at a window
    // width of 880 px the bar must fit (test).
    /// Tooltip of the i button.
    pub about_tip: &'static str,
    /// Tooltip of the language button; the name of the chosen language in itself.
    pub language_tip: fn(&str) -> String,
    /// Placeholder in the search field (180 px with icon, ~14 characters).
    pub search_hint: &'static str,
    /// German: „Dateinamen filtern (Strg+F)“.
    pub search_tip: &'static str,
    /// Tooltip on the *Threads* field and on the processor icon.
    pub threads_tip: &'static str,
    /// Addition to the tooltip when the folder is on an HDD.
    pub threads_hdd: &'static str,
    /// Reason while a run locks subfolders and threads (tooltip, notice after Ctrl+,).
    pub locked: &'static str,
    /// Tooltip of the *Subfolders* switch, on or off.
    pub subfolders_on: &'static str,
    /// German: „Unterordner: aus. Prüfen und Erzeugen nehmen nur Dateien direkt im Ordner;
    /// Einträge in Unterordnern bleiben …“.
    pub subfolders_off: &'static str,
    /// Tooltips of the three theme segments.
    pub theme_light: &'static str,
    /// German: „Thema: dunkel“.
    pub theme_dark: &'static str,
    /// German: „Thema: wie das System“.
    pub theme_system: &'static str,
    /// Button (secondary button with icon), short: ~10 characters.
    pub verify: &'static str,
    /// Tooltip; the name of the `.md5`.
    pub verify_tip: fn(&str) -> String,
    /// Primary button with icon, short: ~14 characters.
    pub generate: &'static str,
    /// Tooltip; the name of the `.md5`.
    pub generate_tip: fn(&str) -> String,
    /// Tooltip on the name of the `.md5` in the bar: it exists / does not exist yet.
    pub md5_here: &'static str,
    /// German: „Liegt noch nicht im Ordner. „MD5 erzeugen“ legt sie an.“.
    pub md5_missing: &'static str,

    // Reasons (tooltips of locked buttons, notices).
    /// The preview of the folder is still coming.
    pub loading: &'static str,
    /// A run is already writing; also title of the confirmation when closing, and pill.
    pub writing: &'static str,
    /// The preview was aborted; the history is missing.
    pub unread: &'static str,
    /// German: „Es läuft schon ein Lauf. Zweimal Esc bricht ihn ab.“.
    pub run_busy: &'static str,
    /// Verify without `.md5`; the name of the `.md5`.
    pub verify_needs_md5: fn(&str) -> String,

    // Status bar on the left (shortened, in full in the tooltip).
    /// Before the first folder.
    pub pick_folder: &'static str,
    /// The folder (name) is being read.
    pub reading_folder: fn(&str) -> String,
    /// German: „Der Ordner ließ sich nicht lesen.“.
    pub folder_unreadable_status: &'static str,
    /// After opening: name of the `.md5`, number of files, whether the `.md5` exists yet.
    pub folder_status: fn(&str, usize, bool) -> String,
    /// Confirmation declined; name of the `.md5`.
    pub md5_unchanged: fn(&str) -> String,
    /// During create or verify; name of the `.md5`.
    pub status_generating: fn(&str) -> String,
    /// German: „Prüfung gegen {md5}…“. Values: `{md5}` name of the `.md5`.
    pub status_verifying: fn(&str) -> String,
    /// After Esc during a run.
    pub cancelling: &'static str,
    /// Status line of a selected "Verlauf" or "md5" row.
    pub row_history: &'static str,
    /// German: „Doppelklick öffnet sie“.
    pub row_md5: &'static str,
    /// Deviating file: stored and current checksum (status bar, one line).
    /// Values: algorithm ("MD5", "SHA-256", the same in every language), old, new.
    pub md5_then_now: fn(&str, &str, &str) -> String,
    /// The same in the tooltip, two lines, the hashes one below the other (algorithm in front
    /// in both lines, so that they stay aligned).
    pub md5_then_now_lines: fn(&str, &str, &str) -> String,
    /// Short in the status bar: deviating and structure possibly defective / intact / not
    /// checked.
    pub short_damaged: &'static str,
    /// German: „Inhalt geändert; Anfang und Ende in Ordnung – Schaden im Inneren nicht
    /// ausgeschlossen“.
    pub short_intact: &'static str,
    /// German: „Inhalt geändert; Struktur dieses Dateityps nicht geprüft“.
    pub short_unchecked: &'static str,
    /// Tooltip of the status bar per status: what it means and what to do.
    pub detail_changed_intact: &'static str,
    /// German: „Der Inhalt hat sich geändert. Die Struktur dieses Dateityps prüft Veritas nicht.“.
    pub detail_changed_unchecked: &'static str,
    /// German: „Der Inhalt hat sich geändert, und die Struktur ist möglicherweise defekt.“.
    pub detail_damaged: &'static str,
    /// Next step for a deviating file.
    pub restore_help: &'static str,
    /// The reason of the structure check.
    pub structure: fn(&str) -> String,
    /// Name of the `.md5` in the following ones.
    pub detail_deleted: fn(&str) -> String,
    /// German: „Liegt im Ordner, steht aber nicht in {md5}. „MD5 erzeugen“ nimmt sie auf.“.
    /// Values: `{md5}` name of the `.md5`.
    pub detail_new: fn(&str) -> String,
    /// German: „Steht in {md5}; noch nicht geprüft. „Prüfen“ (F5) liest sie.“. Values: `{md5}`
    /// name of the `.md5`.
    pub detail_open: fn(&str) -> String,
    /// German: „Stimmt mit {md5} überein.“. Values: `{md5}` name of the `.md5`.
    pub detail_ok: fn(&str) -> String,
    /// German: „Ist sie in einem anderen Programm geöffnet oder gesperrt? Dann freigeben und noch
    /// einmal prüfen (F5).“.
    pub detail_error: &'static str,

    // Chips on the right in the status bar (label like the statuses below).
    /// Tooltip on the active chip; the keys (Alt+1 …).
    pub chip_all: fn(&str) -> String,
    /// Tooltip; status, keys.
    pub chip_only: fn(&str, &str) -> String,
    /// Tooltip on the locked chip and notice after Alt+digit; the status.
    pub chip_none: fn(&str) -> String,

    // Result of a run (notice and status bar).
    /// German: „Abgebrochen. Es wurde nichts geschrieben.“.
    pub cancelled: &'static str,
    /// German: „Der Lauf wurde unerwartet beendet.“.
    pub run_died: &'static str,
    /// The run did not start; message of the system.
    pub run_not_started: fn(&str) -> String,
    /// Create took over checksums: how many, how many files read.
    pub reused_read: fn(usize, usize) -> String,
    /// Files with "Fehler" status.
    pub unreadable_files: fn(usize) -> String,
    /// First `.md5`; name, files.
    pub written: fn(&str, usize) -> String,
    /// `.md5` replaced; name, files. The sentences below follow.
    pub replaced: fn(&str, usize) -> String,
    /// German: „Gegenüber der alten Datei hat sich nichts geändert.“.
    pub unchanged_since: &'static str,
    /// The changes, e.g. "2 geändert, 1 aufgenommen".
    pub changed_since: fn(&str) -> String,
    /// German: „Die neue Datei hat auch den möglicherweise beschädigten Stand übernommen; der
    /// Verlauf nennt die alten Prüfsu …“.
    pub damage_kept: &'static str,
    /// Verify; name of the `.md5`.
    pub verified_one: fn(&str) -> String,
    /// Verify; name, number of files.
    pub verified_all: fn(&str, usize) -> String,
    /// Verify; name, the findings ("1 abweichend, 2 neu").
    pub verified_findings: fn(&str, &str) -> String,
    /// Window title during a run (before " · Veritas").
    pub title_writing: &'static str,
    /// German: „Wird abgebrochen“.
    pub title_cancelling: &'static str,

    // Notices.
    /// Crash log since the last start; its path.
    pub crash_notice: fn(&str) -> String,
    /// German: „Während eines Laufs lässt sich kein Ordner ablegen. Zweimal Esc bricht ab.“.
    pub drop_running: &'static str,
    /// Several dropped; the name of the first.
    pub drop_many: fn(&str) -> String,
    /// German: „Datei abgelegt – es gilt der Ordner, in dem sie liegt.“.
    pub drop_file: &'static str,
    /// German: „Den Ordner erst nach dem Lauf wechseln. Zweimal Esc bricht ab.“.
    pub folder_running: &'static str,
    /// Path.
    pub not_a_folder: fn(&str) -> String,
    /// German: „Der Ordner ließ sich nicht lesen. Links im Baum noch einmal wählen.“.
    pub folder_unreadable: &'static str,
    /// Path.
    pub recent_gone: fn(&str) -> String,
    /// German: „Keine Befunde in der Liste.“.
    pub no_findings: &'static str,
    /// File.
    pub open_gone: fn(&str) -> String,
    /// File, reason (sentence part, see `Msg::NotInFolder`).
    pub open_refused: fn(&str, &str) -> String,
    /// File, message of the system.
    pub open_failed: fn(&str, &str) -> String,
    /// History, message of the system.
    pub open_json_failed: fn(&str, &str) -> String,
    /// German: „Während eines Laufs wird nichts gelöscht. Zweimal Esc bricht ab.“.
    pub delete_running: &'static str,
    /// Also reason in the context menu.
    pub delete_own: &'static str,
    /// File, reason (sentence part).
    pub delete_refused: fn(&str, &str) -> String,
    /// German: „{path} liegt nicht mehr im Ordner.“. Values: `{path}` path of the file.
    pub delete_gone: fn(&str) -> String,
    /// Delete declined; file.
    pub kept: fn(&str) -> String,
    /// File, message of the recycle bin.
    pub trash_failed: fn(&str, &str) -> String,
    /// German: „{path} liegt im Papierkorb.“. Values: `{path}` path of the file.
    pub trashed: fn(&str) -> String,
    /// German: „{path} ist gelöscht; es gibt nichts zu zeigen.“. Values: `{path}` path of the file.
    pub reveal_gone: fn(&str) -> String,
    /// File, reason (sentence part).
    pub reveal_refused: fn(&str, &str) -> String,
    /// File, message of the system.
    pub reveal_failed: fn(&str, &str) -> String,
    /// German: „MD5 von {path} kopiert.“. Values: algorithm ("MD5", "SHA-256"), `{path}` path
    /// of the file.
    pub md5_copied: fn(&str, &str) -> String,
    /// Tooltip on the × of notice, cards, dialogs.
    pub close_esc: &'static str,
    /// Tooltip of a notice that fades by itself.
    pub close: &'static str,

    // Dialog cards (buttons ~24 characters).
    /// Name of the `.md5`.
    pub replace_title: fn(&str) -> String,
    /// German: „Ersetzen“.
    pub replace: &'static str,
    /// German: „Behalten“.
    pub keep: &'static str,
    /// Name of the `.md5`, number of its entries, name of the history, whether "vorher prüfen"
    /// is there.
    pub replace_body: fn(&str, usize, &str, bool) -> String,
    /// Addition: checksums from the last run; number, whether from verify (else create), time.
    pub replace_reuse: fn(usize, bool, &str) -> String,
    /// Addition when subfolders are off; number of entries.
    pub replace_nested: fn(usize) -> String,
    /// Closing during a run that is already writing; folder, name of the `.md5`.
    pub close_writing_body: fn(&str, &str) -> String,
    /// German: „Lauf abbrechen und schließen?“.
    pub close_title: &'static str,
    /// Folder, name of the `.md5`.
    pub close_body: fn(&str, &str) -> String,
    /// German: „Abbrechen und schließen“.
    pub cancel_close: &'static str,
    /// German: „Weiterlaufen lassen“.
    pub keep_running: &'static str,
    /// File name.
    pub trash_title: fn(&str) -> String,
    /// German: „In den Papierkorb“.
    pub to_trash: &'static str,
    /// File.
    pub trash_body: fn(&str) -> String,
    /// Addition if it is listed in the `.md5`; name of the `.md5`.
    pub trash_body_recorded: fn(&str) -> String,
    /// Tooltip on the button triggered by Enter; label.
    pub enter_tip: fn(&str) -> String,
    /// Tooltip on button and × triggered by Esc; label.
    pub esc_tip: fn(&str) -> String,

    // Progress pill (600 px).
    /// German: „Wird abgebrochen – es wird nichts geschrieben…“.
    pub cancelling_lead: &'static str,
    /// German: „Noch einmal Esc bricht ab – es wird nichts geschrieben“.
    pub armed_lead: &'static str,
    /// Small button on the right.
    pub cancel: &'static str,
    /// German: „Abbrechen (zweimal Esc). Es wird nichts geschrieben.“.
    pub cancel_tip: &'static str,
    /// After `writing`, when cancelling is no longer possible.
    pub cancel_late: &'static str,
    /// "120 von 1.234 Dateien" – both numbers already formatted.
    pub files_progress: fn(&str, &str) -> String,
    /// Number of checksums taken over.
    pub reused_progress: fn(usize) -> String,
    /// Remaining time ("3:05 min").
    pub left: fn(&str) -> String,

    // Folder tree.
    /// German: „Den Ordner erst nach dem Lauf wechseln (zweimal Esc bricht ab).“.
    pub tree_running: &'static str,

    // File list.
    /// German: „Wird gelesen…“.
    pub list_reading: &'static str,
    /// German: „Keine Dateien direkt in diesem Ordner. „MD5 erzeugen“ nimmt die Unterordner mit.“.
    pub no_files_sub_on: &'static str,
    /// German: „Keine Dateien direkt in diesem Ordner. Für die Unterordner „Unterordner“ in der
    /// Leiste einschalten.“.
    pub no_files_sub_off: &'static str,
    /// German: „Keine Dateien in diesem Ordner.“.
    pub no_files: &'static str,
    /// Search text.
    pub no_hits: fn(&str) -> String,
    /// German: „Keine Dateien mit diesem Status.“.
    pub no_status: &'static str,
    /// Column headers: status 112 px, size 76 px (right-aligned), modified 128 px.
    pub col_status: &'static str,
    /// German: „Dateiname“.
    pub col_name: &'static str,
    /// German: „Größe“.
    pub col_size: &'static str,
    /// German: „Geändert“.
    pub col_modified: &'static str,
    /// Status labels (column 112 px, chips of the status bar): one word, short.
    pub mark_open: &'static str,
    /// German: „ok“.
    pub mark_ok: &'static str,
    /// German: „abweichend“.
    pub mark_wrong: &'static str,
    /// German: „beschädigt“.
    pub mark_damaged: &'static str,
    /// German: „gelöscht“.
    pub mark_deleted: &'static str,
    /// German: „neu“.
    pub mark_new: &'static str,
    /// German: „Fehler“.
    pub mark_error: &'static str,
    /// Veritas's own files in the list.
    pub mark_source: &'static str,
    /// German: „Verlauf“.
    pub mark_report: &'static str,
    /// What create records in the history (label as above).
    pub change_changed: &'static str,
    /// German: „aufgenommen“.
    pub change_added: &'static str,
    /// German: „entfernt“.
    pub change_dropped: &'static str,
    /// A status with its count, where several files are counted: message after verify and create
    /// (`verified_findings`, `changed_since`) and run list of the history card. The same word as
    /// the label (`mark_*`, `change_*`), in the form the number requires, the number with
    /// `thousands`: "2 abweichend", "2 mismatches", "2 modifiés" (French 0 and 1 singular),
    /// "faltan 2". The chips of the status bar, by contrast, show the label: it is the name of
    /// the filter (also in `chip_only`), and a chip must not change its width when its number
    /// jumps from 1 to 2 during a run (R1).
    pub counted: fn(Change, usize) -> String,
    /// Tooltip on the clock icon, after the last entry.
    pub history_tip: &'static str,
    /// Context menu (240 px with key).
    pub menu_history: &'static str,
    /// German: „Öffnen“.
    pub menu_open: &'static str,
    /// German: „Verlauf der Datei“.
    pub menu_file_history: &'static str,
    /// German: „Im Ordner zeigen“.
    pub menu_reveal: &'static str,
    /// German: „MD5 kopieren“. Value: algorithm ("MD5", "SHA-256"). Menu width: room for
    /// "SHA-512".
    pub menu_copy: fn(&str) -> String,
    /// German: „In den Papierkorb …“.
    pub menu_trash: &'static str,
    /// Reasons of locked menu rows.
    pub why_gone: &'static str,
    /// German: „Für diesen Ordner gibt es noch keinen Verlauf.“.
    pub why_no_history: &'static str,
    /// German: „Für diese Datei gibt es noch keine MD5. „Prüfen“ (F5) berechnet sie.“.
    /// Value: algorithm ("MD5", "SHA-256").
    pub why_no_md5: fn(&str) -> String,

    // Empty state.
    /// German: „Ordner wählen“.
    pub empty_title: &'static str,
    /// German: „Links im Baum einen Ordner wählen oder einen Ordner ins Fenster ziehen.“.
    pub empty_text: &'static str,
    /// German: „Zuletzt geöffnet“.
    pub recent_title: &'static str,
    /// Tooltip; path, key.
    pub open_recent: fn(&str, &str) -> String,
    /// German: „Tasten“.
    pub keys_title: &'static str,
    /// Key cap and effect, at most 40 characters (§10).
    pub empty_keys: [(&'static str, &'static str); 5],

    // "Über Veritas" (F1).
    /// German: „Über Veritas“.
    pub about_title: &'static str,
    /// German: „Esc / F1 schließt“.
    pub about_close: &'static str,
    /// German: „Veritas prüft Dateien und Ordner mit MD5 und meldet, ob eine abweichende
    /// Mediendatei noch strukturell intakt …“.
    pub about_text: &'static str,
    /// German: „Version“.
    pub about_version: &'static str,
    /// German: „Copyright“.
    pub about_copyright: &'static str,
    /// German: „Lizenz“.
    pub about_license: &'static str,
    /// German: „Quellcode“.
    pub about_source: &'static str,
    /// German: „Fehler melden“.
    pub report_title: &'static str,
    /// German: „Etwas funktioniert nicht? Beschreiben Sie es in einem GitHub-Issue – dafür braucht
    /// es ein kostenloses GitHub …“.
    pub report_text: &'static str,
    /// German: „Fehler auf GitHub melden“.
    pub report_link: &'static str,
    /// German: „Wünsche“.
    pub wish_title: &'static str,
    /// German: „Eine Idee, die das Prüfen mit Veritas einfacher oder sicherer macht? Schreiben Sie
    /// sie als Wunsch – ebenfall …“.
    pub wish_text: &'static str,
    /// German: „Wunsch auf GitHub schreiben“.
    pub wish_link: &'static str,
    /// German: „Drittanbieter“.
    pub third_title: &'static str,
    /// German: „Veritas nutzt freie Bibliotheken und Schriften anderer: Noto Sans und DejaVu Sans
    /// Mono (Linux), die Symbole …“.
    pub third_text: &'static str,
    /// German: „Alle Drittanbieter und Lizenzen“.
    pub third_link: &'static str,

    // History card (Ctrl+H).
    /// German: „Verlauf“.
    pub history_title: &'static str,
    /// Tab.
    pub tab_runs: &'static str,
    /// Tab; file name.
    pub tab_file: fn(&str) -> String,
    /// Kind of a run in its heading.
    pub action_created: &'static str,
    /// German: „MD5 ersetzt“.
    pub action_replaced: &'static str,
    /// German: „Geprüft“.
    pub action_verified: &'static str,
    /// The same short in the run list (300 px with date).
    pub short_created: &'static str,
    /// German: „ersetzt“.
    pub short_replaced: &'static str,
    /// German: „geprüft“.
    pub short_verified: &'static str,
    /// German: „abgebrochen“.
    pub run_cancelled: &'static str,
    /// German: „keine Änderung“.
    pub no_change: &'static str,
    /// German: „alles stimmt“.
    pub all_ok: &'static str,
    /// First run; number of files.
    pub first_facts: fn(usize) -> String,
    /// Number (formatted) of unchanged files.
    pub unchanged_count: fn(&str) -> String,
    /// Tooltip of the icon in the run list.
    pub flag_damaged: &'static str,
    /// German: „Befund: abweichend oder gelöscht“.
    pub flag_differs: &'static str,
    /// German: „Bei der ersten Prüfsummendatei hält der Verlauf keine einzelnen Dateien fest. Jede
    /// Datei, die später nicht a …“.
    pub baseline_note: &'static str,
    /// German: „Gegenüber der bisherigen .md5 hat sich nichts geändert.“.
    pub generate_none: &'static str,
    /// German: „Alle Dateien stimmen mit der .md5 überein.“.
    pub verify_none: &'static str,
    /// Checksum before / after.
    pub before: &'static str,
    /// German: „danach“.
    pub after: &'static str,
    /// German: „Klick zeigt alle Einträge dieser Datei.“.
    pub entry_tip: &'static str,
    /// File without entry; date of the first run.
    pub file_none: fn(&str) -> String,
    /// Number of entries, date of the first run.
    pub file_events: fn(usize, &str) -> String,
    /// German: „Diesen Lauf zeigen“.
    pub show_run: &'static str,
    /// Button at the bottom right.
    pub open_json: &'static str,
    /// Name of the history.
    pub open_json_tip: fn(&str) -> String,
    /// Key hints at the bottom left: tab of a file / runs with file / runs.
    pub keys_file: &'static str,
    /// German: „↑ / ↓ Lauf  ·  ← / → Reiter  ·  Esc schließt“.
    pub keys_runs_file: &'static str,
    /// German: „↑ / ↓ Lauf  ·  Klick auf eine Datei zeigt ihre Einträge  ·  Esc schließt“.
    pub keys_runs: &'static str,
    /// Status bar; date of the first run.
    pub hint_none: fn(&str) -> String,
    /// Status bar; kind (label), date.
    pub hint_last: fn(&str, &str) -> String,
    /// The same with the number of all entries.
    pub hint_last_many: fn(&str, &str, usize) -> String,

    // Messages of the system, recycle bin.
    /// German: „nicht gefunden“.
    pub sys_not_found: &'static str,
    /// German: „keine Berechtigung“.
    pub sys_denied: &'static str,
    /// Text of the system.
    pub sys_message: fn(&str) -> String,
    /// German: „Auf Netzwerkpfaden gibt es keinen Papierkorb; Veritas löscht dort nicht.“.
    pub no_trash_unc: &'static str,
    /// German: „Auf Netzlaufwerken gibt es keinen Papierkorb; Veritas löscht dort nicht.“.
    pub no_trash_network: &'static str,
    /// German: „Auf Wechseldatenträgern gibt es keinen Papierkorb; Veritas löscht dort nicht.“.
    pub no_trash_removable: &'static str,

    /// Texts from run, preview and structure check (`Msg`): template with the same placeholders
    /// as the German one.
    pub msg: fn(Msg) -> &'static str,
}

impl Texts {
    /// The label of a status.
    pub fn mark(&self, mark: crate::scanning::RowMark) -> &'static str {
        use crate::scanning::RowMark;
        match mark {
            RowMark::Open => self.mark_open,
            RowMark::Ok => self.mark_ok,
            RowMark::Wrong => self.mark_wrong,
            RowMark::Damaged => self.mark_damaged,
            RowMark::Deleted => self.mark_deleted,
            RowMark::New => self.mark_new,
            RowMark::Error => self.mark_error,
            RowMark::Source => self.mark_source,
            RowMark::Report => self.mark_report,
        }
    }

    /// `Strg+H`, `Ctrl+L` …
    pub fn ctrl(&self, key: &str) -> String {
        format!("{}+{key}", self.key_ctrl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanning::RowMark;
    use crate::theme::{self, space, text};
    use eframe::egui::{self, FontFamily, FontId};

    #[test]
    fn codes_and_cycle() {
        assert_eq!(Lang::from_code("de-DE"), Some(Lang::De));
        assert_eq!(Lang::from_code("es_ES.UTF-8"), Some(Lang::Es));
        assert_eq!(Lang::from_code("FR"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("it-CH"), Some(Lang::It));
        assert_eq!(Lang::from_code("en_GB"), Some(Lang::En));
        assert_eq!(Lang::from_code("pt-BR"), None);
        assert_eq!(Lang::from_code(""), None);
        let mut lang = Lang::De;
        for expected in [Lang::En, Lang::Fr, Lang::Es, Lang::It, Lang::De] {
            lang = lang.next();
            assert_eq!(lang, expected);
        }
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
            assert!(!lang.name().is_empty());
        }
        // The system language is one of the five or English – never none.
        assert!(Lang::ALL.contains(&system_default()));
    }

    /// In tests `set` applies only in its own thread; another test still sees German.
    #[test]
    fn set_stays_in_the_test_thread() {
        set(Lang::It);
        assert_eq!(current(), Lang::It);
        let other = std::thread::spawn(current).join().unwrap();
        assert_eq!(other, Lang::De);
        set(Lang::De);
    }

    /// Each language reproduces the values it gets (a template that loses one is an error), and
    /// no text is empty.
    #[test]
    fn texts_show_their_values() {
        for lang in Lang::ALL {
            let t = lang.texts();
            let name = lang.name();
            let has = |text: String, parts: &[&str]| {
                for part in parts {
                    assert!(text.contains(part), "{name}: „{part}“ fehlt in „{text}“");
                }
            };
            has((t.percent)(42), &["42"]);
            for count in [t.files, t.checksums, t.entries, t.runs] {
                has(count(1), &["1"]);
                has(count(1234), &["234"]);
                assert_ne!(count(1), count(2), "{name}: singular and plural");
            }
            for one in [
                t.language_tip,
                t.verify_tip,
                t.generate_tip,
                t.verify_needs_md5,
                t.reading_folder,
                t.md5_unchanged,
                t.status_generating,
                t.status_verifying,
                t.structure,
                t.detail_deleted,
                t.detail_new,
                t.detail_open,
                t.detail_ok,
                t.chip_all,
                t.chip_none,
                t.run_not_started,
                t.changed_since,
                t.verified_one,
                t.crash_notice,
                t.drop_many,
                t.not_a_folder,
                t.recent_gone,
                t.open_gone,
                t.delete_gone,
                t.kept,
                t.trashed,
                t.reveal_gone,
                t.menu_copy,
                t.why_no_md5,
                t.replace_title,
                t.trash_title,
                t.trash_body,
                t.trash_body_recorded,
                t.enter_tip,
                t.esc_tip,
                t.left,
                t.no_hits,
                t.tab_file,
                t.unchanged_count,
                t.file_none,
                t.open_json_tip,
                t.hint_none,
                t.sys_message,
            ] {
                has(one("W1"), &["W1"]);
            }
            for three in [t.md5_then_now, t.md5_then_now_lines] {
                has(three("SHA-512", "W1", "W2"), &["SHA-512", "W1", "W2"]);
            }
            for two in [
                t.md5_copied,
                t.chip_only,
                t.verified_findings,
                t.open_refused,
                t.open_failed,
                t.open_json_failed,
                t.delete_refused,
                t.trash_failed,
                t.reveal_refused,
                t.reveal_failed,
                t.close_writing_body,
                t.close_body,
                t.files_progress,
                t.open_recent,
                t.hint_last,
            ] {
                has(two("W1", "W2"), &["W1", "W2"]);
            }
            for folder_exists in [true, false] {
                has(
                    (t.folder_status)("A.md5", 12, folder_exists),
                    &["A.md5", "12"],
                );
            }
            has((t.reused_read)(7, 9), &["7", "9"]);
            has((t.unreadable_files)(3), &["3"]);
            has((t.written)("A.md5", 5), &["A.md5", "5"]);
            has((t.replaced)("A.md5", 5), &["A.md5", "5"]);
            has((t.verified_all)("A.md5", 5), &["A.md5", "5"]);
            for advise in [true, false] {
                has(
                    (t.replace_body)("A.md5", 4, "A.json", advise),
                    &["A.md5", "4", "A.json"],
                );
            }
            assert_ne!(
                (t.replace_body)("A.md5", 4, "A.json", true),
                (t.replace_body)("A.md5", 4, "A.json", false),
                "{name}: the advice to verify first"
            );
            for verify in [true, false] {
                has((t.replace_reuse)(6, verify, "14:02"), &["6", "14:02"]);
            }
            has((t.replace_nested)(8), &["8"]);
            has((t.reused_progress)(6), &["6"]);
            has((t.first_facts)(6), &["6"]);
            has((t.file_events)(6, "W1"), &["6", "W1"]);
            has((t.hint_last_many)("W1", "W2", 6), &["W1", "W2", "6"]);
            for fixed in [
                t.about_tip,
                t.search_hint,
                t.verify,
                t.generate,
                t.writing,
                t.cancel,
                t.close_title,
                t.empty_title,
                t.about_text,
                t.history_title,
                t.baseline_note,
                t.key_ctrl,
                t.key_enter,
                t.key_delete,
            ] {
                assert!(!fixed.trim().is_empty(), "{name}");
            }
            assert!(t.ctrl("H").ends_with("+H"), "{name}");
            for (key, what) in t.empty_keys {
                assert!(!key.is_empty() && !what.is_empty(), "{name}");
            }
        }
    }

    /// Counted statuses: per language 1 and 2 for each, as they appear in the messages after a
    /// run and in the run list. German stays predicative and unchanged, the other languages
    /// set the plural.
    #[test]
    fn counted_statuses_take_the_plural() {
        use Change::*;
        /// Status, with 1, with 2.
        type Row = (Change, &'static str, &'static str);
        let table: [(Lang, [Row; 8]); 5] = [
            (
                Lang::De,
                [
                    (Differs, "1 abweichend", "2 abweichend"),
                    (Damaged, "1 beschädigt", "2 beschädigt"),
                    (Changed, "1 geändert", "2 geändert"),
                    (New, "1 neu", "2 neu"),
                    (Added, "1 aufgenommen", "2 aufgenommen"),
                    (Missing, "1 gelöscht", "2 gelöscht"),
                    (Dropped, "1 entfernt", "2 entfernt"),
                    (Unreadable, "1 Fehler", "2 Fehler"),
                ],
            ),
            (
                Lang::En,
                [
                    (Differs, "1 mismatch", "2 mismatches"),
                    (Damaged, "1 damaged", "2 damaged"),
                    (Changed, "1 updated", "2 updated"),
                    (New, "1 new", "2 new"),
                    (Added, "1 added", "2 added"),
                    (Missing, "1 missing", "2 missing"),
                    (Dropped, "1 removed", "2 removed"),
                    (Unreadable, "1 error", "2 errors"),
                ],
            ),
            (
                Lang::Fr,
                [
                    (Differs, "1 modifié", "2 modifiés"),
                    (Damaged, "1 corrompu", "2 corrompus"),
                    (Changed, "1 mis à jour", "2 mis à jour"),
                    (New, "1 nouveau", "2 nouveaux"),
                    (Added, "1 ajouté", "2 ajoutés"),
                    (Missing, "1 absent", "2 absents"),
                    (Dropped, "1 retiré", "2 retirés"),
                    (Unreadable, "1 erreur", "2 erreurs"),
                ],
            ),
            (
                Lang::Es,
                [
                    (Differs, "1 distinto", "2 distintos"),
                    (Damaged, "1 dañado", "2 dañados"),
                    (Changed, "1 modificado", "2 modificados"),
                    (New, "1 nuevo", "2 nuevos"),
                    (Added, "1 añadido", "2 añadidos"),
                    (Missing, "falta 1", "faltan 2"),
                    (Dropped, "1 quitado", "2 quitados"),
                    (Unreadable, "1 error", "2 errores"),
                ],
            ),
            (
                Lang::It,
                [
                    (Differs, "1 diverso", "2 diversi"),
                    (Damaged, "1 corrotto", "2 corrotti"),
                    (Changed, "1 aggiornato", "2 aggiornati"),
                    (New, "1 nuovo", "2 nuovi"),
                    (Added, "1 aggiunto", "2 aggiunti"),
                    (Missing, "1 mancante", "2 mancanti"),
                    (Dropped, "1 rimosso", "2 rimossi"),
                    (Unreadable, "1 errore", "2 errori"),
                ],
            ),
        ];
        for (lang, rows) in table {
            let t = lang.texts();
            for (change, one, two) in rows {
                assert_eq!((t.counted)(change, 1), one, "{lang:?} {change:?}");
                assert_eq!((t.counted)(change, 2), two, "{lang:?} {change:?}");
                // The singular is the word of the list (Spanish: "falta" with the number after it).
                assert!(one.contains(change.label_in(t)), "{lang:?} {change:?}");
            }
        }
        // French: 0 in the singular; large numbers with the thousands separator of the language.
        assert_eq!((Lang::Fr.texts().counted)(Differs, 0), "0 modifié");
        assert_eq!(
            (Lang::Fr.texts().counted)(New, 1234),
            "1\u{202f}234 nouveaux"
        );
        assert_eq!((Lang::En.texts().counted)(Unreadable, 0), "0 errors");
        assert_eq!((Lang::Es.texts().counted)(Missing, 1234), "faltan 1.234");
        assert_eq!((Lang::It.texts().counted)(Damaged, 1234), "1.234 corrotti");
        assert_eq!((Lang::De.texts().counted)(Unreadable, 1234), "1.234 Fehler");
        // Next to it in the run list: the unchanged files, also with plural.
        assert_eq!((Lang::It.texts().unchanged_count)("1"), "1 invariato");
        assert_eq!((Lang::It.texts().unchanged_count)("2"), "2 invariati");
        assert_eq!((Lang::Fr.texts().unchanged_count)("2"), "2 inchangés");
        assert_eq!(
            count_parts_in(
                Lang::Fr.texts(),
                &[(1, Differs), (0, Missing), (3, New), (2, Unreadable)]
            ),
            "1 modifié, 3 nouveaux, 2 erreurs"
        );
    }

    /// Width of a text in a font, with the fonts of the interface.
    fn width(ctx: &egui::Context, text: &str, font: FontId) -> f32 {
        ctx.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE)
                .size()
                .x
        })
    }

    /// Width of a button (`theme::button`, without icon).
    fn button(ctx: &egui::Context, label: &str, primary: bool) -> f32 {
        let width = width(
            ctx,
            label,
            FontId::new(text::BODY, FontFamily::Proportional),
        ) + 2.0 * space::S3;
        if primary {
            width.max(120.0)
        } else {
            width
        }
    }

    /// Contexts with the system fonts and with the embedded ones (Linux, Noto Sans).
    fn contexts() -> Vec<egui::Context> {
        [false, true]
            .into_iter()
            .map(|embedded| {
                let ctx = egui::Context::default();
                theme::install(&ctx);
                if embedded {
                    ctx.set_fonts(crate::fonts::embedded_definitions());
                }
                ctx.run_ui(egui::RawInput::default(), |_| {})
                    .textures_delta
                    .clear();
                ctx
            })
            .collect()
    }

    /// Reserve for labels with fixed room (§10). It applies to the translations; German is the
    /// existing text (guideline: unchanged) and only has to fit.
    const RESERVE: f32 = 1.05;

    /// This much is left for the status line on the left of the chips at 880 px at least (it
    /// shortens with tooltip). German leaves a good 50 px with four-digit numbers.
    const STATUS_LINE_MIN: f32 = 40.0;

    /// Labels with fixed room fit in every language, with a 5 % reserve (§10): status labels in
    /// their column, column headers, date, buttons of the dialog cards, menu rows, chips of the
    /// status bar, search field, key lines of the empty state. Checked at the smallest window
    /// size, with the system fonts and the embedded ones. The message names language and field –
    /// shorten it there.
    #[test]
    fn fixed_labels_fit_in_every_language() {
        let cols = crate::list::columns(crate::ui::MIN_SIZE[0]);
        let small = FontId::new(text::SMALL, FontFamily::Proportional);
        let body = FontId::new(text::BODY, FontFamily::Proportional);
        let head = FontId::new(text::HEAD, theme::semibold());
        let marks = [
            RowMark::Open,
            RowMark::Ok,
            RowMark::Wrong,
            RowMark::Damaged,
            RowMark::Deleted,
            RowMark::New,
            RowMark::Error,
        ];
        // Chips of the status bar (`theme::filter_chip`), with a four-digit sample number: fixed
        // part (margins, dot, number, gap) and labels.
        let chips = |ctx: &egui::Context, t: &Texts| -> (f32, f32) {
            let sample = width(ctx, &count_in(t, 8888), small.clone());
            let fixed = marks.len() as f32 * (3.0 * space::S2 + 12.0 + 2.0 * space::S1 + sample);
            let labels = marks
                .iter()
                .map(|mark| width(ctx, t.mark(*mark), small.clone()))
                .sum::<f32>();
            (fixed, labels)
        };
        let mut misfits = Vec::new();
        for ctx in contexts() {
            for lang in Lang::ALL {
                let t = lang.texts();
                let reserve = if lang == Lang::De { 1.0 } else { RESERVE };
                let mut fits = |what: &str, text: &str, need: f32, room: f32| {
                    if need * reserve > room {
                        misfits.push(format!(
                            "{lang:?} {what} „{text}“: {need:.0} px (+5 %) > {room:.0} px"
                        ));
                    }
                };
                // Status label: dot, text, margins (`theme::status_tag`), in the "Status" column
                // of the list and of the history card (112 px).
                let tag = |label: &str| {
                    2.0 * space::S2 + 6.0 + space::S1 + width(&ctx, label, small.clone())
                };
                // In the list the label starts S2 after the start of the row.
                for mark in marks {
                    let label = t.mark(mark);
                    fits("mark_*", label, tag(label), cols.status - space::S2);
                }
                // In the history card it has its own column (`history::TAG_W`).
                for (field, label) in [
                    ("change_changed", t.change_changed),
                    ("change_added", t.change_added),
                    ("change_dropped", t.change_dropped),
                ] {
                    fits(field, label, tag(label), crate::history::TAG_W);
                }
                let heads = crate::list::header_labels(t, "SHA-512");
                let rooms = [cols.status, 80.0, cols.size, cols.date, cols.md5];
                for (label, room) in heads.iter().zip(rooms) {
                    fits(
                        "col_*",
                        label,
                        width(&ctx, label, head.clone()),
                        room - space::S3,
                    );
                }
                let at = jiff::civil::date(2026, 12, 28).at(23, 58, 0, 0);
                let date = at.strftime(t.date_time).to_string();
                fits(
                    "date_time",
                    &date,
                    width(&ctx, &date, body.clone()),
                    cols.date - space::S2,
                );
                // Dialog cards: both buttons in one row.
                for (yes, no, primary) in [
                    (t.replace, t.keep, true),
                    (t.to_trash, t.keep, true),
                    (t.cancel_close, t.keep_running, true),
                    (t.close, t.keep_running, true),
                ] {
                    let need = button(&ctx, yes, primary) + space::S2 + button(&ctx, no, false);
                    fits(
                        "Dialogknöpfe",
                        &format!("{yes} | {no}"),
                        need,
                        theme::DIALOG_W - 2.0 * space::S6,
                    );
                }
                // Menu rows: text on the left, key on the right (`theme::menu_row`).
                for (label, keys) in [
                    (t.menu_history, t.key_enter),
                    (t.menu_open, t.key_enter),
                    (t.menu_file_history, &*t.ctrl("H")),
                    (t.menu_reveal, ""),
                    (&*(t.menu_copy)("SHA-512"), ""),
                    (t.menu_trash, t.key_delete),
                ] {
                    let need = width(&ctx, label, body.clone())
                        + space::S4
                        + width(&ctx, keys, small.clone())
                        + 2.0 * space::S3;
                    fits("menu_*", label, need, theme::MENU_W);
                }
                // The reserve applies to the words; the status line keeps `STATUS_LINE_MIN`.
                let (fixed, labels) = chips(&ctx, t);
                let bar = crate::ui::MIN_SIZE[0] - 2.0 * space::S3;
                fits("Chips (mark_*)", "", labels, bar - STATUS_LINE_MIN - fixed);
                // Placeholder in the search field (140 px with magnifier).
                fits(
                    "search_hint",
                    t.search_hint,
                    width(&ctx, t.search_hint, body.clone()),
                    140.0 - theme::ICON - 3.0 * space::S2,
                );
                // Small button of the progress pill.
                fits(
                    "cancel",
                    t.cancel,
                    width(&ctx, t.cancel, small.clone()),
                    120.0,
                );
                // Key lines of the empty state: key cap, gap, text – next to the tree.
                let central = crate::ui::MIN_SIZE[0] - crate::ui::TREE_W - 2.0 * space::S6;
                for (key, what) in t.empty_keys {
                    let need = width(&ctx, key, small.clone())
                        + 14.0
                        + space::S2
                        + width(&ctx, what, body.clone());
                    fits("empty_keys", what, need, central);
                }
            }
        }
        misfits.dedup();
        assert!(misfits.is_empty(), "{misfits:#?}");
    }

    /// French: before `:`, `;`, `?`, `!`, `%` and inside « » there is a non-breaking space (§10),
    /// not an ordinary one. Checked in the source of the texts.
    #[test]
    fn french_keeps_signs_with_their_word() {
        let source = include_str!("fr.rs");
        let mut found = Vec::new();
        for (number, line) in source.lines().enumerate() {
            let line = line.trim();
            if line.starts_with("//") {
                continue;
            }
            for bad in [" :", " ;", " ?", " !", " %", "« ", " »"] {
                // Only in strings, not in code (`a: b`, `|x| …`); ` %` only as a percent sign,
                // not in `%H` or `%PDF-`.
                let in_text = line
                    .split('"')
                    .enumerate()
                    .any(|(index, part)| index % 2 == 1 && signs_in(part, bad));
                if in_text {
                    found.push(format!("fr.rs:{}: „{bad}“", number + 1));
                }
            }
        }
        assert!(found.is_empty(), "U+00A0 statt Leerzeichen: {found:#?}");
    }

    /// Is `bad` in the text, as punctuation? A `%` followed by a letter or `%` is a format
    /// character (`%H`, `%%EOF`), not a percent sign.
    fn signs_in(part: &str, bad: &str) -> bool {
        part.match_indices(bad).any(|(at, _)| {
            bad != " %"
                || !part
                    .get(at + bad.len()..)
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(|next| next.is_alphanumeric() || next == '%')
        })
    }

    #[test]
    fn signs_in_tells_percent_from_formats() {
        assert!(signs_in("42 %", " %"));
        assert!(signs_in("{} % done", " %"));
        assert!(!signs_in("%d.%m.%Y %H:%M", " %"));
        assert!(!signs_in("header %PDF- and %%EOF", " %"));
        assert!(signs_in("Quoi ?", " ?"));
        assert!(!signs_in("Quoi?", " ?"));
    }

    #[test]
    fn numbers_sizes_and_times() {
        let (de, en) = (Lang::De.texts(), Lang::En.texts());
        assert_eq!(number(1_234_567, "\u{202f}"), "1\u{202f}234\u{202f}567");
        assert_eq!(size_in(de, 0), "0\u{a0}B");
        assert_eq!(size_in(de, 5 * 1024 * 1024 * 1024), "5,0\u{a0}GB");
        assert_eq!(size_in(en, 3 * 1024 * 1024 / 2), "1.5\u{a0}MB");
        assert_eq!(eta(Duration::from_secs(3720)), "1:02\u{a0}h");
        assert_eq!(
            count_parts_in(en, &[(0, Change::New), (1234, Change::Missing)]),
            "1,234 missing"
        );
        assert_eq!(
            count_parts_in(en, &[(2, Change::Differs), (1, Change::Unreadable)]),
            "2 mismatches, 1 error"
        );
        let err = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert_eq!(plain_io(&err), de.sys_not_found);
        let other = std::io::Error::other("kaputt");
        assert!(plain_io(&other).contains("kaputt"));
        // `Say` follows the language in which it is shown.
        let say = Say::of(|t| t.cancel);
        assert_eq!(say.in_lang(en), en.cancel);
        assert_eq!(say.in_lang(de), de.cancel);
        assert_eq!(format!("{say}"), de.cancel);
        assert!(format!("{say:?}").contains(de.cancel));
        let stored: Say = Msg::RunCancelled.en().into();
        assert_eq!(stored.in_lang(de), Msg::RunCancelled.de());
        let owned: Say = String::from("frei").into();
        assert_eq!(owned.get(), "frei");
    }

    /// The extra line of the confirmation before replacing: the verb agrees with the number of
    /// entries in subfolders, even for one.
    #[test]
    fn nested_entries_agree_with_their_number() {
        let expected = [
            (
                Lang::De,
                "1 Eintrag in Unterordnern bleibt",
                "2 Einträge in Unterordnern bleiben",
            ),
            (
                Lang::En,
                "1 entry in subfolders stays as it is",
                "2 entries in subfolders stay as they are",
            ),
            (
                Lang::Fr,
                "1 entrée dans des sous-dossiers reste inchangée,",
                "2 entrées dans des sous-dossiers restent inchangées,",
            ),
            (
                Lang::Es,
                "1 entrada de las subcarpetas se queda como está,",
                "2 entradas de las subcarpetas se quedan como están,",
            ),
            (
                Lang::It,
                "1 voce nelle sottocartelle resta invariata,",
                "2 voci nelle sottocartelle restano invariate,",
            ),
        ];
        for (lang, one, two) in expected {
            let t = lang.texts();
            let (said_one, said_two) = ((t.replace_nested)(1), (t.replace_nested)(2));
            assert!(said_one.starts_with(one), "{lang:?}: {said_one}");
            assert!(said_two.starts_with(two), "{lang:?}: {said_two}");
        }
    }
}

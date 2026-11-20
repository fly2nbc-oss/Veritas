//! German – the texts as Veritas showed them before the language choice.

use super::{Msg, Texts};
use crate::history::Change;

fn n(value: usize) -> String {
    super::number(value, ".")
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{} {many}", n(count))
    }
}

fn files(count: usize) -> String {
    plural(count, "Datei", "Dateien")
}

fn checksums(count: usize) -> String {
    plural(count, "Prüfsumme", "Prüfsummen")
}

fn entries(count: usize) -> String {
    plural(count, "Eintrag", "Einträge")
}

/// "2 abweichend", "2 Fehler": the status words are predicative and stay the same for every
/// number – so this is the label itself.
fn counted(change: Change, count: usize) -> String {
    format!("{} {}", n(count), change.label_in(&TEXTS))
}

pub static TEXTS: Texts = Texts {
    date_time: "%d.%m.%Y %H:%M",
    date: "%d.%m.%Y",
    thousands: ".",
    decimal: ',',
    size_units: ["B", "KB", "MB", "GB", "TB"],
    percent: |value| format!("{value}\u{a0}%"),
    files,
    checksums,
    entries,
    runs: |count| plural(count, "Lauf", "Läufe"),

    key_ctrl: "Strg",
    key_enter: "Eingabe",
    key_delete: "Entf",

    about_tip: "Über Veritas (F1)",
    language_tip: |name| format!("Sprache: {name} (Strg+L)"),
    search_hint: "Suchen",
    search_tip: "Dateinamen filtern (Strg+F)",
    threads_tip: "Threads: Dateien, die gleichzeitig gehasht werden (Strg+,; ↑/↓ zählen). Auf \
                  einer Festplatte (HDD) immer eine, das ist dort schneller. Gilt ab dem nächsten \
                  Lauf.",
    threads_hdd: "Dieser Ordner liegt auf einer Festplatte (HDD): Veritas hasht hier eine Datei \
                  nach der anderen, die Zahl gilt hier nicht.",
    locked: "Während eines Laufs gesperrt. Zweimal Esc bricht ab.",
    subfolders_on: "Unterordner: an. Prüfen und Erzeugen nehmen auch die Dateien in Unterordnern. \
                    Klick: nur Dateien direkt im Ordner.",
    subfolders_off: "Unterordner: aus. Prüfen und Erzeugen nehmen nur Dateien direkt im Ordner; \
                     Einträge in Unterordnern bleiben unverändert. Klick: Unterordner mitnehmen.",
    theme_light: "Thema: hell",
    theme_dark: "Thema: dunkel",
    theme_system: "Thema: wie das System",
    verify: "Prüfen",
    verify_tip: |md5| format!("Prüft den Ordner gegen {md5} (F5)"),
    generate: "MD5 erzeugen",
    generate_tip: |md5| {
        format!("Schreibt {md5} mit den Prüfsummen aller Dateien im Ordner (Strg+Eingabe)")
    },
    md5_here: "Prüfsummendatei dieses Ordners",
    md5_missing: "Liegt noch nicht im Ordner. „MD5 erzeugen“ legt sie an.",

    loading: "Der Ordner wird noch gelesen. Gleich noch einmal versuchen.",
    writing: "Wird geschrieben – gleich fertig.",
    unread: "Der Verlauf ließ sich nicht lesen. Den Ordner links noch einmal wählen.",
    run_busy: "Es läuft schon ein Lauf. Zweimal Esc bricht ihn ab.",
    verify_needs_md5: |md5| format!("Im Ordner liegt noch keine {md5}. Erst „MD5 erzeugen“."),

    pick_folder: "Ordner im Baum wählen.",
    reading_folder: |folder| format!("„{folder}“ wird gelesen…"),
    folder_unreadable_status: "Der Ordner ließ sich nicht lesen.",
    folder_status: |md5, count, exists| {
        if exists {
            format!("{md5} · {}", files(count))
        } else {
            format!("Noch keine {md5} · {}", files(count))
        }
    },
    md5_unchanged: |md5| format!("{md5} bleibt unverändert."),
    status_generating: |md5| format!("{md5} wird geschrieben…"),
    status_verifying: |md5| format!("Prüfung gegen {md5}…"),
    cancelling: "Wird abgebrochen…",
    row_history: "alle Läufe dieses Ordners; Doppelklick oder Strg+H zeigt sie",
    row_md5: "Doppelklick öffnet sie",
    md5_then_now: |algo, old, new| format!("{algo} gespeichert {old}, jetzt {new}"),
    md5_then_now_lines: |algo, old, new| {
        format!("{algo} gespeichert {old}\n{algo} jetzt       {new}")
    },
    short_damaged: "Inhalt geändert, Struktur möglicherweise defekt",
    short_intact: "Inhalt geändert; Anfang und Ende in Ordnung – Schaden im Inneren nicht \
                   ausgeschlossen",
    short_unchecked: "Inhalt geändert; Struktur dieses Dateityps nicht geprüft",
    detail_changed_intact: "Der Inhalt hat sich geändert. Anfang und Ende sind in Ordnung – das \
                            schließt einen Schaden im Inneren nicht aus.",
    detail_changed_unchecked: "Der Inhalt hat sich geändert. Die Struktur dieses Dateityps \
                               prüft Veritas nicht.",
    detail_damaged: "Der Inhalt hat sich geändert, und die Struktur ist möglicherweise defekt.",
    restore_help: "Nicht selbst bearbeitet? Dann aus der Sicherung zurückholen.",
    structure: |reason| format!("Strukturprüfung: {reason}"),
    detail_deleted: |md5| {
        format!(
            "Steht in {md5}, liegt aber nicht mehr im Ordner. Absichtlich gelöscht? Dann trägt \
             „MD5 erzeugen“ sie aus."
        )
    },
    detail_new: |md5| {
        format!("Liegt im Ordner, steht aber nicht in {md5}. „MD5 erzeugen“ nimmt sie auf.")
    },
    detail_open: |md5| format!("Steht in {md5}; noch nicht geprüft. „Prüfen“ (F5) liest sie."),
    detail_ok: |md5| format!("Stimmt mit {md5} überein."),
    detail_error: "Ist sie in einem anderen Programm geöffnet oder gesperrt? Dann freigeben und \
                   noch einmal prüfen (F5).",

    chip_all: |keys| format!("Alle Dateien zeigen (Esc, {keys})"),
    chip_only: |label, keys| format!("Nur „{label}“ zeigen ({keys})"),
    chip_none: |label| format!("Keine Datei mit Status „{label}“"),

    cancelled: "Abgebrochen. Es wurde nichts geschrieben.",
    run_died: "Der Lauf wurde unerwartet beendet.",
    run_not_started: |why| {
        format!(
            "Der Lauf konnte nicht starten ({why}). Veritas neu starten und noch einmal versuchen."
        )
    },
    reused_read: |reused, read| {
        format!("{} übernommen, {} gelesen.", checksums(reused), files(read))
    },
    unreadable_files: |count| format!("{} nicht lesbar (Status „Fehler“).", files(count)),
    written: |md5, count| format!("{md5} geschrieben: {}.", files(count)),
    replaced: |md5, count| format!("{md5} ersetzt: {}.", files(count)),
    unchanged_since: "Gegenüber der alten Datei hat sich nichts geändert.",
    changed_since: |parts| format!("Gegenüber der alten Datei: {parts}."),
    damage_kept: "Die neue Datei hat auch den möglicherweise beschädigten Stand übernommen; der \
                  Verlauf nennt die alten Prüfsummen.",
    verified_one: |md5| format!("Prüfung gegen {md5}: Die Datei ist ok."),
    verified_all: |md5, count| format!("Prüfung gegen {md5}: alle {} ok.", files(count)),
    verified_findings: |md5, parts| format!("Prüfung gegen {md5}: {parts}."),
    title_writing: "Wird geschrieben",
    title_cancelling: "Wird abgebrochen",

    crash_notice: |log| {
        format!("Veritas wurde zuletzt durch einen Programmfehler gestört. Bericht: {log}")
    },
    drop_running: "Während eines Laufs lässt sich kein Ordner ablegen. Zweimal Esc bricht ab.",
    drop_many: |first| format!("Mehrere abgelegt – es gilt nur das erste: „{first}“."),
    drop_file: "Datei abgelegt – es gilt der Ordner, in dem sie liegt.",
    folder_running: "Den Ordner erst nach dem Lauf wechseln. Zweimal Esc bricht ab.",
    not_a_folder: |path| {
        format!("„{path}“ ist kein Ordner (mehr). Links im Baum einen anderen wählen.")
    },
    folder_unreadable: "Der Ordner ließ sich nicht lesen. Links im Baum noch einmal wählen.",
    recent_gone: |path| {
        format!("„{path}“ gibt es nicht mehr; er steht nicht mehr unter den letzten Ordnern.")
    },
    no_findings: "Keine Befunde in der Liste.",
    open_gone: |path| format!("{path} ist gelöscht und lässt sich nicht öffnen."),
    open_refused: |path, why| format!("{path} lässt sich nicht öffnen: {why}."),
    open_failed: |path, why| {
        format!(
            "{path} lässt sich nicht öffnen ({why}). Ist ein Programm für diesen Dateityp \
             eingerichtet? „Im Ordner zeigen“ im Kontextmenü hilft weiter."
        )
    },
    open_json_failed: |path, why| {
        format!(
            "{path} lässt sich nicht öffnen ({why}). Jeder Editor oder Browser zeigt die Datei \
             ebenso."
        )
    },
    delete_running: "Während eines Laufs wird nichts gelöscht. Zweimal Esc bricht ab.",
    delete_own: "Prüfsummendatei und Verlauf löscht Veritas nicht.",
    delete_refused: |path, why| format!("{path} {why}; Veritas löscht es nicht."),
    delete_gone: |path| format!("{path} liegt nicht mehr im Ordner."),
    kept: |path| format!("{path} bleibt."),
    trash_failed: |path, why| {
        format!(
            "{path} ließ sich nicht in den Papierkorb verschieben ({why}). Ist sie noch in einem \
             Programm geöffnet? Dann das Programm schließen und noch einmal versuchen."
        )
    },
    trashed: |path| format!("{path} liegt im Papierkorb."),
    reveal_gone: |path| format!("{path} ist gelöscht; es gibt nichts zu zeigen."),
    reveal_refused: |path, why| format!("Der Ordner von {path} lässt sich nicht zeigen: {why}."),
    reveal_failed: |path, why| format!("Der Ordner von {path} lässt sich nicht zeigen ({why})."),
    md5_copied: |algo, path| format!("{algo} von {path} kopiert."),
    close_esc: "Schließen (Esc)",
    close: "Schließen",

    replace_title: |md5| format!("{md5} ersetzen?"),
    replace: "Ersetzen",
    keep: "Behalten",
    replace_body: |md5, count, history, advise| {
        let advice = if advise {
            " Im Zweifel vorher prüfen (F5)."
        } else {
            ""
        };
        format!(
            "In {md5} stehen {}. „Ersetzen“ schreibt die Datei mit dem jetzigen Stand des Ordners \
             neu. Danach gilt dieser Stand als richtig – auch für Dateien, die sich unbemerkt \
             verändert haben.{advice} Welche Prüfsummen sich ändern, hält der Verlauf in {history} \
             fest; eine Sicherungskopie (.md5.bak) entsteht nicht.",
            checksums(count)
        )
    },
    replace_reuse: |count, verify, at| {
        let from = if verify {
            "der Prüfung"
        } else {
            "dem Erzeugen"
        };
        format!(
            "Übernimmt bis zu {} aus {from} von {at}; gelesen werden nur Dateien, die seitdem neu \
             sind, eine andere Größe oder Änderungszeit haben oder nicht lesbar waren.",
            checksums(count)
        )
    },
    replace_nested: |count| {
        let stay = if count == 1 { "bleibt" } else { "bleiben" };
        format!(
            "{} in Unterordnern {stay} unverändert, weil „Unterordner“ aus ist.",
            entries(count)
        )
    },
    close_writing_body: |folder, md5| {
        format!(
            "Veritas schreibt gerade das Ergebnis für „{folder}“ (Verlauf, beim Erzeugen auch \
             {md5}). Beim Schließen wartet das Fenster, bis es geschrieben ist."
        )
    },
    close_title: "Lauf abbrechen und schließen?",
    close_body: |folder, md5| {
        format!(
            "Veritas hasht gerade „{folder}“. Beim Schließen bricht der Lauf ab; es wird nichts \
             geschrieben – weder {md5} noch der Verlauf."
        )
    },
    cancel_close: "Abbrechen und schließen",
    keep_running: "Weiterlaufen lassen",
    trash_title: |name| format!("{name} in den Papierkorb?"),
    to_trash: "In den Papierkorb",
    trash_body: |path| {
        format!("{path} wird in den Papierkorb verschoben und lässt sich dort wiederherstellen.")
    },
    trash_body_recorded: |md5| {
        format!(
            "Die Prüfsumme bleibt in {md5}: Die Datei steht danach als „gelöscht“ in der Liste, \
             bis „MD5 erzeugen“ sie austrägt. Das hält dann auch der Verlauf fest."
        )
    },
    enter_tip: |label| format!("{label} (Eingabe)"),
    esc_tip: |label| format!("{label} (Esc)"),

    cancelling_lead: "Wird abgebrochen – es wird nichts geschrieben…",
    armed_lead: "Noch einmal Esc bricht ab – es wird nichts geschrieben",
    cancel: "Abbrechen",
    cancel_tip: "Abbrechen (zweimal Esc). Es wird nichts geschrieben.",
    cancel_late: "Abbrechen geht nicht mehr.",
    files_progress: |done, total| format!("{done} von {total} Dateien"),
    reused_progress: |count| format!("{} übernommen", checksums(count)),
    left: |eta| format!("noch {eta}"),

    tree_running: "Den Ordner erst nach dem Lauf wechseln (zweimal Esc bricht ab).",

    list_reading: "Wird gelesen…",
    no_files_sub_on: "Keine Dateien direkt in diesem Ordner. „MD5 erzeugen“ nimmt die \
                      Unterordner mit.",
    no_files_sub_off: "Keine Dateien direkt in diesem Ordner. Für die Unterordner \
                       „Unterordner“ in der Leiste einschalten.",
    no_files: "Keine Dateien in diesem Ordner.",
    no_hits: |query| format!("Keine Treffer für „{query}“."),
    no_status: "Keine Dateien mit diesem Status.",
    col_status: "Status",
    col_name: "Dateiname",
    col_size: "Größe",
    col_modified: "Geändert",
    mark_open: "offen",
    mark_ok: "ok",
    mark_wrong: "abweichend",
    mark_damaged: "beschädigt",
    mark_deleted: "gelöscht",
    mark_new: "neu",
    mark_error: "Fehler",
    mark_source: "md5",
    mark_report: "Verlauf",
    change_changed: "geändert",
    change_added: "aufgenommen",
    change_dropped: "entfernt",
    counted,
    history_tip: "Alle Einträge dieser Datei zeigen (Strg+H)",
    menu_history: "Verlauf zeigen",
    menu_open: "Öffnen",
    menu_file_history: "Verlauf der Datei",
    menu_reveal: "Im Ordner zeigen",
    menu_copy: |algo| format!("{algo} kopieren"),
    menu_trash: "In den Papierkorb …",
    why_gone: "Die Datei ist gelöscht.",
    why_no_history: "Für diesen Ordner gibt es noch keinen Verlauf.",
    why_no_md5: |algo| {
        format!("Für diese Datei gibt es noch keine {algo}. „Prüfen“ (F5) berechnet sie.")
    },

    empty_title: "Ordner wählen",
    empty_text: "Links im Baum einen Ordner wählen oder einen Ordner ins Fenster ziehen.",
    recent_title: "Zuletzt geöffnet",
    open_recent: |path, key| format!("{path} öffnen ({key})"),
    keys_title: "Tasten",
    empty_keys: [
        ("Strg+Eingabe", "MD5 erzeugen"),
        ("F5", "Prüfen"),
        ("Strg+H", "Verlauf: wann sich was geändert hat"),
        ("Entf", "Datei in den Papierkorb, mit Rückfrage"),
        ("Esc", "eine Ebene zurück; im Lauf zweimal: abbrechen"),
    ],

    about_title: "Über Veritas",
    about_close: "Esc / F1 schließt",
    about_text: "Veritas prüft Dateien und Ordner mit MD5 und meldet, ob eine abweichende \
                 Mediendatei noch strukturell intakt ist – lokal, ohne Konto und ohne Cloud. \
                 Freie Software: Der Quellcode ist offen.",
    about_version: "Version",
    about_copyright: "Copyright",
    about_license: "Lizenz",
    about_source: "Quellcode",
    report_title: "Fehler melden",
    report_text: "Etwas funktioniert nicht? Beschreiben Sie es in einem GitHub-Issue – dafür \
                  braucht es ein kostenloses GitHub-Konto. Version und System sind schon \
                  eingetragen; Dateinamen und Pfade braucht es nicht.",
    report_link: "Fehler auf GitHub melden",
    wish_title: "Wünsche",
    wish_text: "Eine Idee, die das Prüfen mit Veritas einfacher oder sicherer macht? Schreiben \
                Sie sie als Wunsch – ebenfalls mit GitHub-Konto.",
    wish_link: "Wunsch auf GitHub schreiben",
    third_title: "Drittanbieter",
    third_text: "Veritas nutzt freie Bibliotheken und Schriften anderer: Noto Sans und DejaVu \
                 Sans Mono (Linux), die Symbole von Lucide und die Ersatzschriften von egui \
                 (Windows). Ihre Lizenztexte liegen dem Programm bei.",
    third_link: "Alle Drittanbieter und Lizenzen",

    history_title: "Verlauf",
    tab_runs: "Läufe",
    tab_file: |name| format!("Datei: {name}"),
    action_created: "MD5 erzeugt",
    action_replaced: "MD5 ersetzt",
    action_verified: "Geprüft",
    short_created: "erzeugt",
    short_replaced: "ersetzt",
    short_verified: "geprüft",
    run_cancelled: "abgebrochen",
    no_change: "keine Änderung",
    all_ok: "alles stimmt",
    first_facts: |count| format!("{} in der ersten Prüfsummendatei.", files(count)),
    unchanged_count: |count| format!("{count} unverändert"),
    flag_damaged: "Befund: beschädigt oder nicht lesbar",
    flag_differs: "Befund: abweichend oder gelöscht",
    baseline_note: "Bei der ersten Prüfsummendatei hält der Verlauf keine einzelnen Dateien \
                    fest. Jede Datei, die später nicht als geändert erscheint, hat noch diesen \
                    Stand.",
    generate_none: "Gegenüber der bisherigen .md5 hat sich nichts geändert.",
    verify_none: "Alle Dateien stimmen mit der Prüfsummendatei überein.",
    before: "vorher",
    after: "danach",
    entry_tip: "Klick zeigt alle Einträge dieser Datei.",
    file_none: |since| {
        format!(
            "Kein Eintrag seit {since}: Kein Lauf hat diese Datei als abweichend, neu, gelöscht \
             oder nicht lesbar gemeldet."
        )
    },
    file_events: |count, since| format!("{} seit {since}, der neueste oben.", entries(count)),
    show_run: "Diesen Lauf zeigen",
    open_json: "Als JSON öffnen",
    open_json_tip: |name| format!("{name} im Programm öffnen, das das System für JSON vorsieht"),
    keys_file: "← / → Reiter  ·  Klick auf ein Datum zeigt den Lauf  ·  Esc schließt",
    keys_runs_file: "↑ / ↓ Lauf  ·  ← / → Reiter  ·  Esc schließt",
    keys_runs: "↑ / ↓ Lauf  ·  Klick auf eine Datei zeigt ihre Einträge  ·  Esc schließt",
    hint_none: |since| format!("Verlauf: keine Änderung seit {since}"),
    hint_last: |what, day| format!("Verlauf: {what} am {day}"),
    hint_last_many: |what, day, count| format!("Verlauf: {what} am {day} ({} Einträge)", n(count)),

    sys_not_found: "nicht gefunden",
    sys_denied: "keine Berechtigung",
    sys_message: |text| format!("Meldung des Systems: {text}"),
    no_trash_unc: "Auf Netzwerkpfaden gibt es keinen Papierkorb; Veritas löscht dort nicht.",
    no_trash_network: "Auf Netzlaufwerken gibt es keinen Papierkorb; Veritas löscht dort nicht.",
    no_trash_removable: "Auf Wechseldatenträgern gibt es keinen Papierkorb; Veritas löscht dort \
                         nicht.",

    msg: Msg::de,
};

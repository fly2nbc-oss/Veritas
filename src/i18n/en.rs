//! English. Dates in ISO order (2026-10-12 19:02): unambiguous for readers in the US and the
//! UK alike, and the order a file list sorts by.

use super::{Msg, Texts};
use crate::history::Change;

fn n(value: usize) -> String {
    super::number(value, ",")
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{} {many}", n(count))
    }
}

fn files(count: usize) -> String {
    plural(count, "file", "files")
}

fn checksums(count: usize) -> String {
    plural(count, "checksum", "checksums")
}

fn entries(count: usize) -> String {
    plural(count, "entry", "entries")
}

/// "1 mismatch", "2 mismatches", "2 errors"; the adjectives and participles stay the same.
fn counted(change: Change, count: usize) -> String {
    let (one, many) = match change {
        Change::Differs => ("mismatch", "mismatches"),
        Change::Damaged => ("damaged", "damaged"),
        Change::Changed => ("updated", "updated"),
        Change::New => ("new", "new"),
        Change::Added => ("added", "added"),
        Change::Missing => ("missing", "missing"),
        Change::Dropped => ("removed", "removed"),
        Change::Unreadable => ("error", "errors"),
    };
    plural(count, one, many)
}

pub static TEXTS: Texts = Texts {
    date_time: "%Y-%m-%d %H:%M",
    date: "%Y-%m-%d",
    thousands: ",",
    decimal: '.',
    size_units: ["B", "KB", "MB", "GB", "TB"],
    percent: |value| format!("{value}%"),
    files,
    checksums,
    entries,
    runs: |count| plural(count, "run", "runs"),

    key_ctrl: "Ctrl",
    key_enter: "Enter",
    key_delete: "Del",

    about_tip: "About Veritas (F1)",
    language_tip: |name| format!("Language: {name} (Ctrl+L)"),
    search_hint: "Search",
    search_tip: "Filter file names (Ctrl+F)",
    threads_tip: "Threads: files hashed at the same time (Ctrl+,; ↑/↓ count). Always one on a \
                  hard disk (HDD), which is faster there. Applies from the next run.",
    threads_hdd: "This folder is on a hard disk (HDD): Veritas hashes one file after another \
                  here, so the number does not apply.",
    locked: "Locked during a run. Press Esc twice to cancel.",
    subfolders_on: "Subfolders: on. Verify and Create also include the files in subfolders. \
                    Click: only files directly in the folder.",
    subfolders_off: "Subfolders: off. Verify and Create only include files directly in the \
                     folder; entries in subfolders stay as they are. Click: include subfolders.",
    theme_light: "Theme: light",
    theme_dark: "Theme: dark",
    theme_system: "Theme: same as the system",
    verify: "Verify",
    verify_tip: |md5| format!("Verify the folder against {md5} (F5)"),
    generate: "Create MD5",
    generate_tip: |md5| {
        format!("Write {md5} with the checksums of all files in the folder (Ctrl+Enter)")
    },
    md5_here: "Checksum file of this folder",
    md5_missing: "Not in the folder yet. “Create MD5” creates it.",

    loading: "The folder is still being read. Try again in a moment.",
    writing: "Writing – almost done.",
    unread: "The history could not be read. Choose the folder on the left again.",
    run_busy: "A run is already in progress. Press Esc twice to cancel it.",
    verify_needs_md5: |md5| format!("There is no {md5} in the folder yet. “Create MD5” first."),

    pick_folder: "Choose a folder in the tree.",
    reading_folder: |folder| format!("Reading “{folder}”…"),
    folder_unreadable_status: "The folder could not be read.",
    folder_status: |md5, count, exists| {
        if exists {
            format!("{md5} · {}", files(count))
        } else {
            format!("No {md5} yet · {}", files(count))
        }
    },
    md5_unchanged: |md5| format!("{md5} stays as it is."),
    status_generating: |md5| format!("Writing {md5}…"),
    status_verifying: |md5| format!("Verifying against {md5}…"),
    cancelling: "Cancelling…",
    row_history: "all runs of this folder; double-click or Ctrl+H shows them",
    row_md5: "double-click opens it",
    md5_then_now: |algo, old, new| format!("{algo} recorded {old}, now {new}"),
    md5_then_now_lines: |algo, old, new| format!("{algo} recorded {old}\n{algo} now      {new}"),
    short_damaged: "content changed, structure possibly broken",
    short_intact: "content changed; start and end are fine – damage inside not ruled out",
    short_unchecked: "content changed; structure of this file type not checked",
    detail_changed_intact: "The content has changed. Start and end are fine – that does not \
                            rule out damage inside.",
    detail_changed_unchecked: "The content has changed. Veritas does not check the structure \
                               of this file type.",
    detail_damaged: "The content has changed, and the structure may be broken.",
    restore_help: "Did you not edit it yourself? Then restore it from your backup.",
    structure: |reason| format!("Structure check: {reason}"),
    detail_deleted: |md5| {
        format!(
            "Listed in {md5}, but no longer in the folder. Deleted on purpose? Then “Create MD5” \
             removes it."
        )
    },
    detail_new: |md5| format!("In the folder, but not listed in {md5}. “Create MD5” adds it."),
    detail_open: |md5| format!("Listed in {md5}; not verified yet. “Verify” (F5) reads it."),
    detail_ok: |md5| format!("Matches {md5}."),
    detail_error: "Is it open or locked in another program? Then release it and verify again \
                   (F5).",

    chip_all: |keys| format!("Show all files (Esc, {keys})"),
    chip_only: |label, keys| format!("Show only “{label}” ({keys})"),
    chip_none: |label| format!("No file with status “{label}”"),

    cancelled: "Cancelled. Nothing was written.",
    run_died: "The run ended unexpectedly.",
    run_not_started: |why| {
        format!("The run could not start ({why}). Restart Veritas and try again.")
    },
    reused_read: |reused, read| format!("{} reused, {} read.", checksums(reused), files(read)),
    unreadable_files: |count| format!("{} not readable (status “error”).", files(count)),
    written: |md5, count| format!("{md5} written: {}.", files(count)),
    replaced: |md5, count| format!("{md5} replaced: {}.", files(count)),
    unchanged_since: "Nothing has changed since the old file.",
    changed_since: |parts| format!("Since the old file: {parts}."),
    damage_kept: "The new file has also taken over the possibly damaged state; the history \
                  keeps the old checksums.",
    verified_one: |md5| format!("Verified against {md5}: the file is ok."),
    verified_all: |md5, count| format!("Verified against {md5}: all {} ok.", files(count)),
    verified_findings: |md5, parts| format!("Verified against {md5}: {parts}."),
    title_writing: "Writing",
    title_cancelling: "Cancelling",

    crash_notice: |log| format!("Veritas was last interrupted by a program error. Report: {log}"),
    drop_running: "You cannot drop a folder during a run. Press Esc twice to cancel.",
    drop_many: |first| format!("Several items dropped – only the first counts: “{first}”."),
    drop_file: "File dropped – the folder it is in counts.",
    folder_running: "Change the folder after the run. Press Esc twice to cancel.",
    not_a_folder: |path| {
        format!("“{path}” is not a folder (any more). Choose another one in the tree on the left.")
    },
    folder_unreadable: "The folder could not be read. Choose it again in the tree on the left.",
    recent_gone: |path| {
        format!("“{path}” no longer exists; it has been removed from the recent folders.")
    },
    no_findings: "No findings in the list.",
    open_gone: |path| format!("{path} is missing and cannot be opened."),
    open_refused: |path, why| format!("{path} cannot be opened: {why}."),
    open_failed: |path, why| {
        format!(
            "{path} cannot be opened ({why}). Is a program set up for this file type? “Show in \
             folder” in the context menu helps."
        )
    },
    open_json_failed: |path, why| {
        format!("{path} cannot be opened ({why}). Any editor or browser shows the file as well.")
    },
    delete_running: "Nothing is deleted during a run. Press Esc twice to cancel.",
    delete_own: "Veritas does not delete the checksum file or the history.",
    delete_refused: |path, why| format!("{path} {why}; Veritas does not delete it."),
    delete_gone: |path| format!("{path} is no longer in the folder."),
    kept: |path| format!("{path} stays."),
    trash_failed: |path, why| {
        format!(
            "{path} could not be moved to the Recycle Bin ({why}). Is it still open in a \
             program? Then close the program and try again."
        )
    },
    trashed: |path| format!("{path} is in the Recycle Bin."),
    reveal_gone: |path| format!("{path} is missing; there is nothing to show."),
    reveal_refused: |path, why| format!("The folder of {path} cannot be shown: {why}."),
    reveal_failed: |path, why| format!("The folder of {path} cannot be shown ({why})."),
    md5_copied: |algo, path| format!("{algo} of {path} copied."),
    close_esc: "Close (Esc)",
    close: "Close",

    replace_title: |md5| format!("Replace {md5}?"),
    replace: "Replace",
    keep: "Keep",
    replace_body: |md5, count, history, advise| {
        let advice = if advise {
            " If in doubt, verify first (F5)."
        } else {
            ""
        };
        format!(
            "{md5} lists {}. “Replace” writes the file anew with the folder as it is now. From \
             then on, this state counts as correct – even for files that have changed \
             unnoticed.{advice} The history in {history} records which checksums change; no \
             backup copy (.md5.bak) is made.",
            checksums(count)
        )
    },
    replace_reuse: |count, verify, at| {
        let from = if verify {
            "the verification"
        } else {
            "creating the file"
        };
        format!(
            "Reuses up to {} from {from} at {at}; only files that are new since then, have a \
             different size or modification time, or were not readable are read.",
            checksums(count)
        )
    },
    replace_nested: |count| {
        let stay = if count == 1 {
            "stays as it is"
        } else {
            "stay as they are"
        };
        format!(
            "{} in subfolders {stay}, because “Subfolders” is off.",
            entries(count)
        )
    },
    close_writing_body: |folder, md5| {
        format!(
            "Veritas is writing the result for “{folder}” (the history, and {md5} when \
             creating). On closing, the window waits until it is written."
        )
    },
    close_title: "Cancel the run and close?",
    close_body: |folder, md5| {
        format!(
            "Veritas is hashing “{folder}”. Closing cancels the run; nothing is written – neither \
             {md5} nor the history."
        )
    },
    cancel_close: "Cancel and close",
    keep_running: "Keep running",
    trash_title: |name| format!("Move {name} to the Recycle Bin?"),
    to_trash: "Move to Recycle Bin",
    trash_body: |path| {
        format!("{path} is moved to the Recycle Bin and can be restored from there.")
    },
    trash_body_recorded: |md5| {
        format!(
            "The checksum stays in {md5}: the file then shows as “missing” in the list until \
             “Create MD5” removes it. The history records that as well."
        )
    },
    enter_tip: |label| format!("{label} (Enter)"),
    esc_tip: |label| format!("{label} (Esc)"),

    cancelling_lead: "Cancelling – nothing is written…",
    armed_lead: "Press Esc again to cancel – nothing is written",
    cancel: "Cancel",
    cancel_tip: "Cancel (Esc twice). Nothing is written.",
    cancel_late: "It can no longer be cancelled.",
    files_progress: |done, total| format!("{done} of {total} files"),
    reused_progress: |count| format!("{} reused", checksums(count)),
    left: |eta| format!("{eta} left"),

    tree_running: "Change the folder after the run (Esc twice cancels).",

    list_reading: "Reading…",
    no_files_sub_on: "No files directly in this folder. “Create MD5” includes the subfolders.",
    no_files_sub_off: "No files directly in this folder. For the subfolders, switch on \
                       “Subfolders” in the toolbar.",
    no_files: "No files in this folder.",
    no_hits: |query| format!("No matches for “{query}”."),
    no_status: "No files with this status.",
    col_status: "Status",
    col_name: "File name",
    col_size: "Size",
    col_modified: "Modified",
    mark_open: "pending",
    mark_ok: "ok",
    mark_wrong: "mismatch",
    mark_damaged: "damaged",
    mark_deleted: "missing",
    mark_new: "new",
    mark_error: "error",
    mark_source: "md5",
    mark_report: "history",
    change_changed: "updated",
    change_added: "added",
    change_dropped: "removed",
    counted,
    history_tip: "Show all entries of this file (Ctrl+H)",
    menu_history: "Show history",
    menu_open: "Open",
    menu_file_history: "History of the file",
    menu_reveal: "Show in folder",
    menu_copy: |algo| format!("Copy {algo}"),
    menu_trash: "Move to Recycle Bin …",
    why_gone: "The file is missing.",
    why_no_history: "There is no history for this folder yet.",
    why_no_md5: |algo| format!("There is no {algo} for this file yet. “Verify” (F5) computes it."),

    empty_title: "Choose a folder",
    empty_text: "Choose a folder in the tree on the left, or drag a folder into the window.",
    recent_title: "Recently opened",
    open_recent: |path, key| format!("Open {path} ({key})"),
    keys_title: "Keys",
    empty_keys: [
        ("Ctrl+Enter", "Create MD5"),
        ("F5", "Verify"),
        ("Ctrl+H", "History: what changed when"),
        ("Del", "Move file to Recycle Bin, with a prompt"),
        ("Esc", "one step back; in a run twice: cancel"),
    ],

    about_title: "About Veritas",
    about_close: "Esc / F1 closes",
    about_text: "Veritas verifies files and folders with MD5 and tells you whether a media file \
                 that no longer matches is still structurally intact – locally, without an \
                 account and without a cloud. Free software: the source code is open.",
    about_version: "Version",
    about_copyright: "Copyright",
    about_license: "Licence",
    about_source: "Source code",
    report_title: "Report a bug",
    report_text: "Something does not work? Describe it in a GitHub issue – this needs a free \
                  GitHub account. Version and system are already filled in; file names and \
                  paths are not needed.",
    report_link: "Report a bug on GitHub",
    wish_title: "Wishes",
    wish_text: "An idea that would make verifying with Veritas easier or safer? Write it down as \
                a wish – also with a GitHub account.",
    wish_link: "Write a wish on GitHub",
    third_title: "Third parties",
    third_text: "Veritas uses free libraries and fonts by others: Noto Sans and DejaVu Sans Mono \
                 (Linux), the icons of Lucide and the fallback fonts of egui (Windows). Their \
                 licence texts come with the program.",
    third_link: "All third parties and licences",

    history_title: "History",
    tab_runs: "Runs",
    tab_file: |name| format!("File: {name}"),
    action_created: "MD5 created",
    action_replaced: "MD5 replaced",
    action_verified: "Verified",
    short_created: "created",
    short_replaced: "replaced",
    short_verified: "verified",
    run_cancelled: "cancelled",
    no_change: "no change",
    all_ok: "all ok",
    first_facts: |count| format!("{} in the first checksum file.", files(count)),
    unchanged_count: |count| format!("{count} unchanged"),
    flag_damaged: "Finding: damaged or not readable",
    flag_differs: "Finding: mismatch or missing",
    baseline_note: "For the first checksum file, the history does not record single files. \
                    Every file that does not show up as changed later still has this state.",
    generate_none: "Nothing has changed since the previous .md5.",
    verify_none: "All files match the checksum file.",
    before: "before",
    after: "after",
    entry_tip: "Click shows all entries of this file.",
    file_none: |since| {
        format!(
            "No entry since {since}: no run has reported this file as a mismatch, new, missing \
             or not readable."
        )
    },
    file_events: |count, since| format!("{} since {since}, newest at the top.", entries(count)),
    show_run: "Show this run",
    open_json: "Open as JSON",
    open_json_tip: |name| format!("Open {name} in the program the system uses for JSON"),
    keys_file: "← / → tab  ·  click a date to show the run  ·  Esc closes",
    keys_runs_file: "↑ / ↓ run  ·  ← / → tab  ·  Esc closes",
    keys_runs: "↑ / ↓ run  ·  click a file to show its entries  ·  Esc closes",
    hint_none: |since| format!("History: no change since {since}"),
    hint_last: |what, day| format!("History: {what} on {day}"),
    hint_last_many: |what, day, count| format!("History: {what} on {day} ({} entries)", n(count)),

    sys_not_found: "not found",
    sys_denied: "no permission",
    sys_message: |text| format!("message from the system: {text}"),
    no_trash_unc: "Network paths have no Recycle Bin; Veritas does not delete there.",
    no_trash_network: "Network drives have no Recycle Bin; Veritas does not delete there.",
    no_trash_removable: "Removable drives have no Recycle Bin; Veritas does not delete there.",

    msg: Msg::en,
};

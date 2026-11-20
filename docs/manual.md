# Veritas – manual

Operation, file formats and limits in detail. The [README](../README.md) gives the introduction.

The interface comes in five languages (see *Language* below). This manual quotes the labels as they appear in the English interface.

## Operation

The folder tree is on the left, starting from `/` or the drives. A click selects the folder, the arrow expands it. The file list fills the rest of the window. Its columns are “Status”, “File name”, “Size”, “Modified” and “MD5”. The arrow keys belong to the tree or the list; a frame around the selected row shows where they act at the moment, the other side shows its selection only as a filled area. After every run they belong to the list.

The toolbar at the top: on the left “Verify” and “Create MD5” with the name of the checksum file, on the right the settings, then the search, the language button and “About Veritas” (the *i* icon). The settings are right in the toolbar; there is no separate window for them:

- **Theme** – three icons: sun for light, moon for dark, screen for “same as the system”.
- **Subfolders** – folder-tree icon; highlighted means on.
- **Threads** – number field with a processor icon; Ctrl+, moves the focus into it, ↑/↓ count, Esc leaves the field. If the folder is on a spinning hard disk (HDD), a hard-disk icon replaces the processor.

All three survive a restart, as do the last three folders. During a run, “Subfolders” and “Threads” are locked; the tooltip says why.

**Language:** The icon to the left of *i* opens the list of languages – Deutsch, English, Français, Español, Italiano, each written in itself, the chosen one ticked. Ctrl+L switches to the next one. The interface changes at once, even in the middle of a run and with the history card open, and Veritas remembers the choice. On the first start the system language applies, English otherwise. Each language writes numbers, sizes and dates its own way (English 1,234 files and 2026-10-12 19:02, French 1 234 fichiers and 1,5 Mo), and where Veritas counts files, the status word takes the form that fits the number (“2 mismatches”, “2 modifiés”). The tags in the list and the filter chips, by contrast, always name the status in its basic form. The files Veritas writes do not depend on the language: the `.md5` has no text, the history and the logs are in English (see *JSON history*).

Without a selected folder, the list names the most recently opened folders: Enter opens the last one, 2 and 3 the ones before it; a click does the same.

1. Select a folder in the tree, drag a folder into the window or start `veritas <folder>`. A `.md5` or SHA file can be opened the same way (drag it, `veritas <file.md5>`, or in Explorer “Open with → Veritas”): then exactly this file applies as long as the folder stays selected. Any other file opens its folder. The list briefly says “Reading…” and then shows what the selected `.md5` contains, without hashing: if the file exists, it is “pending”, if not, it is “missing”. This holds for entries in subfolders too.
2. “Create MD5” (Ctrl+Enter) writes `{folder name}.md5` and appends a run to the history `{folder name}.json`. The file name is shown in the toolbar.
3. If the file already exists, a dialog card asks: “Replace” or “Keep”. Replacing accepts the current state as correct – including files that changed unnoticed; if in doubt, verify first (F5). Afterwards every file appears in the list with its new hash; what changed compared with the old file is named by the notice and the history. Veritas makes no backup copy (`.md5.bak`) – the history plays that role.
   **After a verify, create reads only new and changed files.** If Veritas has just verified the folder (or just created the `.md5`), it knows checksum, size and modification time of every file it read. “Create MD5” then takes over the checksum of every file whose size and modification time stayed the same, without reading it; only files that are new, have changed since, or could not be read last time are read. The structure check of a mismatching file then also comes from the verify. The confirmation says so (“Reuses up to 12,340 checksums from the verification at 14:02 …”), the pill counts only the files that are read (“3 of 3 files”, with “12,340 checksums reused” in the tooltip), and the notice afterwards names both. These known checksums lapse as soon as another folder is selected, “Subfolders” is switched, another `.md5` is opened or the `.md5` has changed; then create reads everything again. “Verify” always reads every file.
4. “Verify” (F5) compares the folder with exactly this file. Without a `.md5` the button is disabled; the tooltip gives the reason.
5. During a run, a pill above the list shows the file being hashed (otherwise the phase), files, MB/s and time remaining; the window title shows the progress in percent, in the taskbar too. A run starts with pending rows (findings of an earlier run no longer apply until the file is read again), and every 2 s the list shows the state of the files already read (when verifying “ok”, “mismatch” or “error”, when creating “pending” with the new MD5; the chips count along); “damaged”, “missing” and new files in subfolders appear only with the result, and a cancel restores the list. “Cancel” cancels, and so does Esc – but only when Esc has nothing else left to undo (error, filter, search, selection), and only on the second press: the first announces it (“Press Esc again to cancel”), the second within 4 s cancels. Closing the window during a run asks first (“Cancel the run and close?”). A cancel writes nothing. Once the run is writing at the end (the history, and when creating also the `.md5`), it can no longer be canceled (“Writing – almost done.”); on closing, the window then waits briefly for it. The history card can be viewed during a run, too.
6. After the run, a notice states the result. If verifying found something (or replacing took over damaged or unreadable files), it stays until you click it or press Esc. If the window is not in the foreground, it flashes in the taskbar.
7. A click on a row shows in the status bar the path, the status, what the history knows about the file (for example “History: updated on 2025-09-13”), for “mismatch” and “damaged” what that means, and the hashes. The status bar's tooltip explains the status with the next step: for “mismatch” the content has changed; if the beginning and end are fine, that does not rule out damage inside – if you did not edit the file yourself, restore it from a backup. Double-click or Enter opens the file in the program the system provides for it. A right-click shows a menu: “Open”, “History of the file”, “Show in folder” (on Windows selected in Explorer), “Copy MD5”, “Move to Recycle Bin …” (the trash on Linux).
8. The chips at the bottom right count every status and filter the list; Alt+1 to Alt+7 switch them in their order (“pending”, “ok”, “mismatch”, “damaged”, “missing”, “new”, “error”). F8 jumps to the next file with a finding (“mismatch”, “damaged”, “missing”, “error”), Shift+F8 to the previous one.
9. The checksum file (“md5”) and the history (“history”) appear as neutral rows in the list. A double-click on the `.md5` opens it; one on “history” shows the history card.
10. **History of a file:** If the history names a file – “mismatch”, “new”, “missing” or unreadable – a small clock icon appears at the right of its “File name” column. A click on it (or Ctrl+H with the file selected) shows the history card with all its entries: when, in which run, hash before and after. The “Runs” tab lists every run, newest first, and on the right which files it names.
11. **Deleting:** Del (or right-click → “Move to Recycle Bin …”) always asks first and then moves the file into the trash. If it is in the `.md5`, it is “missing” afterwards until “Create MD5” drops it; otherwise it disappears from the list and the selection moves to the next row. Veritas does not delete the `.md5` or the history. On Windows, Veritas does not delete on network and removable drives, because they have no Recycle Bin.

Subfolders are included by default. With “Subfolders” off, entries in subfolders stay out: verifying does not report them as “missing”, and “Create MD5” takes them over unchanged. Switching reloads the folder, even after a run. The number of threads starts at the number of logical CPUs (1–256) and applies from the next run. If the folder is on an HDD, Veritas always hashes one file at a time, because the read head would otherwise jump between files; the hard-disk icon then appears beside “Threads”, and the tooltip says so. On Windows the HDD is recognised by the drive's seek time (*seek penalty*), on Linux by `queue/rotational`.

## Keys

| Key | Does |
| --- | --- |
| Ctrl+Enter | Create MD5 |
| F5 | Verify |
| Double-click, Enter | Open the file; on the “history” row, the history card |
| Ctrl+H | History card; with a file selected, its entries (there ↑/↓ pick a run on the “Runs” tab, ←/→ or Ctrl+Tab switch tabs) |
| Del | Move the file to the trash, always with a confirmation (Enter confirms, Esc keeps) |
| Ctrl+F | Search |
| Ctrl+, | Focus the “Threads” field (locked during a run) |
| Ctrl+L | Next language (Deutsch, English, Français, Español, Italiano) |
| ↑ / ↓ | Change row or folder |
| Page Up / Page Down | One page in the list |
| ← / → | Collapse and expand folders; ← in the list jumps to the tree, Enter in the tree to the list |
| Home / End | Start / end of the list |
| F8 / Shift+F8 | Next / previous file with a finding |
| Alt+1 … Alt+7 | Switch the chips “pending”, “ok”, “mismatch”, “damaged”, “missing”, “new”, “error” on and off |
| Enter, 2, 3 | Without a selected folder: open one of the last folders |
| F1 | About Veritas: version, license, report a bug |
| Esc | One level back: context menu, dialog, error or finding, filter, search, selection; during a run after that, twice: cancel |

## File names

The window writes `{folder name}.md5` as long as no matching file is in the folder; otherwise the existing or the explicitly opened `.md5` applies. The history has the same name with `.json` instead of `.md5`. These files are not hashed themselves, on Windows in any capitalization (`FOTOS.MD5`).

**SHA files** Veritas also reads and verifies; writing is always MD5. They are recognized by name, case-insensitively: `*.sha1`, `*.sha256`, `*.sha512`, also with `sum` (`*.sha256sum`), and the collective files `SHA1SUMS`, `SHA256SUMS`, `SHA512SUMS`, also with `.txt`. The name determines the algorithm. A SHA file applies only if there is no `.md5` in the folder (with several the newest, with equal time SHA-256 before SHA-512 before SHA-1), or if it was explicitly opened or dropped. “Verify” then computes only that algorithm; the column is named after it, for example “SHA-256”, and the history `{folder name}.json` records it in `hash_mode` (`"sha256"`). “Create MD5” leaves the SHA file unchanged, reads every file and writes `{folder name}.md5` beside it – only if that already exists does Veritas ask before replacing. Afterwards the `.md5` applies. SHA files in the folder are not hashed themselves and do not appear as “new”, unless the applying checksum file names them; Veritas does not delete them, just as it does not delete any `.md5`.

Writing happens in `….partial` files first. When creating, the history moves into place first, then the `.md5`: if anything fails, the old `.md5` stays. If “Verify” cannot write the history (on a read-only medium, say), it still shows its result and says that the run is not in the history.

## Checksum format

Uppercase hex, exactly two spaces, relative paths with `/`, line ending `\n`:

```text
900150983CD24FB0D6963F7D28E17F72  sub/my file.txt
D41D8CD98F00B204E9800998ECF8427E  empty.txt
```

SHA-1, SHA-256 and SHA-512 are the same with 40, 64 and 128 hex characters. When reading, lowercase, the binary marker ` *` and a single space are accepted too, as is the BSD format (`shasum --tag`, `sha256sum --tag`, `md5` on macOS):

```text
SHA256 (sub/my file.txt) = 3A6EB0790F39AC87C94F3856B2DD2C5D110E6811602261A9A923D3BB23ADC8B7
```

A line whose hash does not have the length of the algorithm (from the file's name or, in the BSD format, the name at the start) is skipped with a notice. Paths with `\` or a line break are escaped as in GNU coreutils. A leading `./` and doubled `/` do not count (`find . -exec md5sum`).

Every path must stay inside the chosen folder. Rejected are `..`, absolute paths and, on Windows, also `\\server\share`, `\\?\…`, drives (`C:`) and names with a colon; there `\` counts as `/`, also in escaped lines. On Linux `a:b.txt` is an ordinary name. Before opening and deleting, Veritas checks once more that the file is a regular file inside the folder and that no link leads there.

A `.md5` may be UTF-8 (with or without BOM), UTF-16 with BOM or ANSI (Windows-1252); if it is not UTF-8, a notice says how it was read. Veritas always writes UTF-8. It does not read a `.md5` larger than 256 MiB or a history larger than 1 GiB; the message names the file and the limit.

## JSON history

Every “Create MD5” and every “Verify” appends a run to `{folder name}.json`, the oldest first; nothing is trimmed. A cancel writes nothing. A report in the earlier single-run format (up to 0.5.0) becomes the first entry at the next run. A foreign file under this name is replaced; the new run says so under `warnings`. If the history is unreadable or has the format of another Veritas version, it is left untouched: “Create MD5” then stops without writing anything, “Verify” shows its result with a notice.

```json
{
  "format": "veritas-verlauf-1",
  "runs": [
    {
      "timestamp": "2026-10-12T19:02:00+00:00",
      "base_directory": "/media/photos",
      "hash_mode": "md5",
      "action": "created",
      "baseline": false,
      "unchanged_count": 10,
      "file_count": 12,
      "modified": [
        {
          "path": "images/a.jpg",
          "old_hash": { "md5": "AAAA…" },
          "new_hash": { "md5": "BBBB…" },
          "integrity_status": "possibly damaged",
          "integrity_reason": "JPEG: EOI (FF D9) missing at the end of the file",
          "size": 12
        }
      ],
      "added": [{ "path": "new.txt", "hash": { "md5": "CCCC…" }, "size": 3 }],
      "removed": [{ "path": "old.txt", "hash": { "md5": "DDDD…" } }],
      "errors": [
        { "path": "locked.txt", "message": "No permission to read: locked.txt", "old_hash": { "md5": "EEEE…" } }
      ],
      "skipped_symlinks": 0,
      "skipped_non_regular": 0,
      "cancelled": false,
      "warnings": []
    }
  ]
}
```

`action` is `created` or `verified`. `integrity_status` is `intact`, `possibly damaged` or `not checked`. Veritas writes all values, reasons, messages and warnings in English, whatever the language of the interface; the field names and the format identifier `veritas-verlauf-1` are the same as before. The history card shows every run in the chosen language.

**Older histories** wrote these values in German (`erzeugt`, `geprüft`, `intakt`, `möglicherweise beschädigt`, `nicht geprüft`) and German reasons and messages. Veritas still reads them, even mixed with new runs in one file, and shows them in the chosen language as well; a free text that Veritas does not recognise appears as it is stored. At the next run, Veritas rewrites the whole history; the fixed values (`action`, `integrity_status`) then become English in the old runs, too, while the texts of the old runs stay German. A version from before the language choice cannot read a history written this way and replaces it with a warning – so do not go back to an older version after the change, or save `{folder name}.json` first.

In the “MD5 and SHA-256” mode `old_hash` and `new_hash` carry both values. The structure check runs only on a hash mismatch, not on new or missing files and not on matching hashes.

**Instead of `.md5.bak`:** A run with `"action": "created"` and `"baseline": false` replaced an existing `.md5`. The old file follows from that run's new `.md5`: drop the entries in `added`, take the `old_hash` instead of the new one for `modified`, and add `removed` and the `errors` with `old_hash` (unreadable when replacing). The same holds for later replacements, run by run backwards.

Retrieving: the clock icon on a file, Ctrl+H or a double-click on the “history” row show the history card. “Open as JSON” opens the file itself in the program the system provides for `.json`; any editor or browser shows it just as well.

## Integrity

| Type | Check |
| --- | --- |
| JPEG | SOI `FF D8`, EOI `FF D9` after the start of the image data in the last 64 KiB; data after it counts as intact |
| PNG | Signature, chunks complete up to IEND; data after it counts as intact |
| WebP / AVI | RIFF header and a plausible block size |
| HEIC/HEIF | `ftyp` with heic, heif, mif1, msf1 and related brands |
| TIFF | `II*\0` or `MM\0*` (BigTIFF `II+\0`, `MM\0+`) and an IFD offset inside the file |
| MP4/MOV | `ftyp` and `moov` (older QuickTime files also without `ftyp`), all atoms up to the end of the file by seeking; a truncated atom (usually `mdat`) means damaged, bytes without an atom name after it do not; media data is not loaded |
| MKV | EBML header `1A 45 DF A3` |
| DOCX/XLSX/PPTX | ZIP local header, central directory and `[Content_Types].xml`; password-protected files (OLE container) are not checked |
| PDF | starts with `%PDF-`, `%%EOF` in the last 2048 bytes |
| others | not checked |
| empty or unreadable | possibly damaged, with a short reason |

This is a structure hint, not proof that the content is semantically valid.

## Colors

Status colors always come with text, as a tag in the “Status” column. In the light theme the list sits on white so that selection and tags stay clear. In the run list of the history card, a run with a finding carries an icon (triangle: “mismatch” or “missing”, circle with exclamation mark: “damaged” or unreadable).

- Grey: “pending” (checksum present, not verified yet).
- Violet: “new” (in the folder, but not in the `.md5`). The same in the history card; what “Create MD5” takes in there (“added”) stays grey.
- Blue is reserved for the selection.
- Green: “ok”, the checksum matches.
- Yellow: “mismatch” or “missing” (the path is in the `.md5` and missing on disk). Both use the same warning color.
- Red: “damaged” (the checksum differs and the structure is possibly broken) or “error” (unreadable).

## Limits

- Symbolic links (on Windows junctions too) are not followed and not hashed, not even in the preview when opening. The chosen folder itself may be a link.
- If a subfolder is unreadable, its entries in the `.md5` count as errors, not as missing; “Create MD5” takes them over unchanged.
- Special files (sockets, devices, FIFOs) are skipped.
- File names must be UTF-8. On Windows, Veritas compares file names without regard to capitalization, elsewhere exactly.
- Files are read buffered in 1 MiB pieces, on Windows with the “front to back” hint to the cache. Veritas does not map files into memory (mmap); that keeps memory use small even with large videos.
- MP4/MOV: at most 10,000 top-level atoms, without loading the media data; beyond that “not checked”. A `moov` deep in an unusual tree can be missed. A ZIP central directory above 16 MiB or 100,000 entries is “not checked”, too.
- PDF looks for `%%EOF` only in the last 2048 bytes. JPEG looks for EOI only in the last 64 KiB. Motion photos with an appended video therefore count as “damaged” when their hash changes.
- A cancel writes no checksum file and no JSON.
- “Create MD5” after a verify detects changes only by size and modification time. If a program changes the content and restores size and modification time, create takes over the old checksum; only “Verify” finds that. Only what did not change while being read, and whose modification time was at least 2 s before the read, is taken over (FAT and exFAT store it only to 2 s); create reads everything else again.
- If Veritas runs into a program error, it appends the message, code location and version to `crash.log` in the data folder (Windows `%APPDATA%\Veritas\data`, Linux `~/.local/share/veritas`) and points to it at the next start. Besides that, `veritas.log` records the start and end of every run and warnings from the window and the graphics – helpful when the display freezes. Both are in English, whatever the language of the interface. Older versions called the crash log `absturz.log`; the first start renames it to `crash.log` as long as there is none yet. Both logs can contain paths; look through them before attaching them to an issue.
- Deleting goes only into the trash; Veritas never deletes for good. If a file is too large for the drive's Recycle Bin, Windows itself asks whether to delete it permanently – only then, after Veritas's confirmation. On Linux, Veritas puts the file into the freedesktop.org trash; on another drive that is its `.Trash-<uid>`.
- The interface needs a display and OpenGL. Without a display the GUI cannot sensibly be tested; the library tests run anyway.

# Changelog

All notable changes to Veritas are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

## [0.8.0] – 2026-11-20

Veritas speaks five languages, verifies and writes more safely, is easier to use from the keyboard and a third smaller; “Create MD5” reads only what is new after a verify. Also contains 0.7.2, which was never released separately.

### Added

- **Five languages** – the interface is available in German, English, French, Spanish and Italian, all fully translated. The icon to the left of *i* opens the list of languages, Ctrl+L switches to the next one; the interface changes at once, even during a run. Veritas remembers the choice; on the first start the system language applies (English otherwise). Numbers, sizes and dates follow the language (English: 1,234 files, 2026-10-12 19:02), and counted statuses take the plural of the language (“2 mismatches”, “2 modifiés”, “2 diversi”). The picture in the README shows the English interface.
- **Create after verify reads only new and changed files** – right after a verify (or a create), “Create MD5” takes over the checksum of every file whose size and modification time stayed the same, and reads only new, changed and previously unreadable files. The confirmation, progress pill and notice say how many are taken over and how many are read; the history stays complete. Verify still reads every file.
- **Reading and verifying SHA checksum files** – if there is no `.md5` in the folder, a SHA file applies: `*.sha1`, `*.sha256`, `*.sha512` (also `*.sha256sum` …) and `SHA1SUMS`, `SHA256SUMS`, `SHA512SUMS` (also `.txt`), in GNU or BSD format (`SHA256 (file) = hash`). Opened or dropped explicitly, it applies next to a `.md5` too. *Verify* then computes SHA-1, SHA-256 or SHA-512, the column and the status bar name the algorithm, the history records it in `hash_mode`. Writing is still MD5 only: “Create MD5” leaves the SHA file unchanged and writes `{folder name}.md5` beside it. Foreign SHA files in the folder do not show up as “new” and cannot be deleted.
- **Interim state in the list** – during *Verify* and “Create MD5”, the list shows the status of the files already read every 2 s (ok, mismatch, error; when creating, the new MD5), and the chips count along. A new run starts with pending rows, even after an earlier verify; missing stays until the result. Rows, selection and scroll position stay; whether a mismatching file is damaged shows only in the result. A cancel restores the list.
- **Run log** – `veritas.log` records the start and end of every run and warnings from the window and the graphics. If the display freezes, it shows whether the run finished anyway and whether drawing hit an error.
- **Recent folders** – without a selected folder, the list names the last three; Enter opens the last one, 2 and 3 the ones before it.
- **Keys** – F8 / Shift+F8 jump to the next or previous file with a finding, Page Up/Down scroll the list, Alt+1 … Alt+7 switch the chips.
- **Context menu** – “Show in folder” (on Windows selected in Explorer) and “Copy MD5”.
- **Progress in the window title** – in the taskbar too; if the window is not in the foreground when a run ends, it flashes.
- **Chinese, Japanese and Korean file names** appear with a font instead of as boxes. Veritas loads a system font for this only at the first such character (Windows: Microsoft YaHei, Yu Gothic, Malgun Gothic; Linux: Noto Sans CJK, if installed); at start-up it costs nothing.

### Changed

- **History in English** – new runs write English values to `{folder name}.json` (`"action": "created"`/`"verified"`, `"integrity_status": "intact"`, `"possibly damaged"`, `"not checked"`) and English reasons, messages and warnings, whatever the language of the interface. Field names and the format identifier stay. Veritas still reads older German histories, mixed ones too, and shows them in the chosen language. Note: an older version then cannot read the history and replaces it – do not switch back without saving `{folder name}.json`.
- **Logs in English** – `veritas.log` and the crash log are in English, regardless of the language; the crash log is now called `crash.log`. If an `absturz.log` exists, the first start renames it, as long as there is no `crash.log` yet.
- **Narrower search field** (180 → 140 px), so that the language button has room even in the smallest window.
- **Esc during a run** – first undoes errors, filter, search and selection; a run is canceled only with a second Esc after the announcement “Press Esc again to cancel”. One Esc too many no longer throws away hours of work. Closing the window during a run asks first (“Cancel the run and close?”).
- **After the run, in the list** – the arrow keys then belong to the list; after a click in the tree, ↓ used to change the folder and discard the result. The side with the keyboard focus shows its selection with a frame.
- **A result with a finding stays** until a click or Esc, instead of fading after 10 s.
- **Clearer texts** – in the status bar, “mismatch” says what it means and its tooltip gives the next step; the confirmation before replacing warns that files that changed unnoticed are taken over too; a folder containing only subfolders no longer looks empty; error messages name the next step, and error notices can be selected. The same words as in the history card, plurals correct, dates as 12.10.2026 19:02 in German; the German “About Veritas” addresses the user formally (“Sie”).
- **Progress pill** – names the file being hashed; the numbers sit in fixed places and no longer jump.
- **“new” is violet** – grey could not be told apart from “pending”. Tag, filter chip and history card show “new” in a color of its own; “pending” stays grey, and blue still means only “selected”.
- **Light theme: list on white** – selection, hover and tags were barely visible on the light grey ground. Links in “About Veritas” have more contrast in the dark theme.
- **“Move to Recycle Bin” is a blue primary button** – the trash can be undone; red stays for replacing the `.md5`.
- **Settings in the toolbar** – theme (sun, moon, screen), subfolders (folder-tree icon) and threads are right in the toolbar; the cog and its popup are gone. Ctrl+, moves the focus into the “Threads” field. If the folder is on a spinning hard disk (HDD), a hard-disk icon appears beside “Threads”. During a run, “Subfolders” and “Threads” stay locked, with the reason.
- **Polish following the design system** – findings in the run list as an icon with a word, “Threads” as a number field (↑/↓ count), context menu with menu rows, larger click targets, tabs that no longer change width, right-aligned “Size” column, pill and notice beneath the dimming behind dialog cards, a disabled primary button stays blue. The history card is also open during a run.
- **Smaller packages** (Windows, measured): `veritas.exe` 11.2 → 7.2 MB, portable zip 5.1 → 3.2 MB, installer 5.0 → 3.8 MB. The Linux packages are smaller too, but not measured.
- **Short README, detailed manual** – the README gives only the introduction; operation, all keys, formats and limits are in `docs/manual.md`.
- **Bug report** – the form has a field for `crash.log` and `veritas.log`.
- **Changing folders no longer blocks the interface** – a thread of its own reads the checksum file, files, history and drive type; until it is done, the status bar says “Reading …” and “Verify”/“Create MD5” wait with the reason in the tooltip. Arrow keys in the tree move the selection at once; reading starts only after 150 ms of rest.

### Fixed

- **Confirmation before replacing with one entry in subfolders** – says “1 entry … stays as it is” instead of “stay”.
- **Desktop entry** (Linux) – describes only MD5 now; the interface offers no SHA-256.
- **Paths from a `.md5` stay inside the folder** – on Windows, escaped lines (`\..\x`, `\\server\share`, `\\?\…`, `C:`) slipped past the check; even opening queried such paths, and double-click and Del acted outside the folder. A central check now rejects them, and before opening and deleting Veritas checks that the file is a regular file inside the folder, with no link or junction in between. The trash test uses the real path.
- **The preview no longer follows links** – like the run. Del on an error row that names a folder no longer deletes anything.
- **Creating does not lose the old `.md5`** – first all intermediate files are written, then the history, the `.md5` last. If the history fails (file open, disk full), the old `.md5` stays.
- **Verifying on read-only media** – the result appears even if the history cannot be written, with a notice. Write errors no longer say “No permission to read”.
- **The history is not replaced by an empty one** – if it is unreadable or in the format of another Veritas version, it stays, and creating stops.
- **Pre-placed `.partial` files** – a leftover intermediate file is removed and created anew; a link in its place no longer overwrites its target.
- **Unreadable subfolder** – its files counted as missing and dropped out of the `.md5` when creating. Now they are read errors and stay in the `.md5`.
- **`./` in foreign `.md5` files** – `find . -exec md5sum` made everything double, missing and new. On Linux, `a:b.txt` is read again.
- **Own `.md5` in another capitalization** (`FOTOS.MD5`) entered itself on Windows and was then always a mismatch.
- **`.md5` in UTF-16 or ANSI** – is now read (with a notice) instead of showing everything as new; creating no longer pretends to have a baseline when the old file is unreadable.
- **Size limits** – a `.md5` above 256 MiB and a history above 1 GiB are rejected with a clear message instead of blocking the interface.
- **The “Replace …?” confirmation** lapses when another folder is dropped; before, Enter replaced that folder's `.md5`.
- **Switching “Subfolders”** reloads after a run, too.
- **Structure check** – truncated MP4 files with `moov` before `mdat` no longer count as intact. No longer wrongly “damaged”: QuickTime without `ftyp` (intact), password-protected Office files (not checked), BigTIFF, PNG with appended data. Large ZIP directories are read buffered and with a limit.
- **Run log** – folder names are escaped in `veritas.log`; a line break in a name no longer forges a line.
- **Canceling shortly before writing still wrote** – a cancel that arrives before writing now really writes nothing. Once the run is writing, it can no longer be canceled: pill, Esc and the close confirmation say “Writing – almost done”, and on closing the window waits up to 10 s so the run does not end between `.md5` and history.
- **Esc closes an open context menu** – before, Esc undid filter, search or selection behind it, and the menu stayed open.
- **“Open as JSON”** checks, like opening, that the history is a regular file in the folder.
- **Selection after deleting with an active filter** – moves to the next visible row instead of staying on the now hidden one. A chip click and Alt+digit treat the selection the same way.
- **Vanished last folder** – drops out of the list of recent folders; the arrow keys stay with the tree.
- **After a failed run** Veritas reads folder, `.md5` and history again so the list is right. If reading a folder is cut short, the history card names the right reason.

### Under the hood

- **README picture by script** – `veritas --screenshot <image.png> … <folder>` takes a picture of a fixed size, without simulating input and without touching settings, crash log or run log; `tools/screenshot.ps1` uses it to produce `screenshots/veritas-main.png` (light) and `veritas-main-dark.png` from the invented example data in `docs/examples/`. New direct dependency `image` (PNG only), which was already in the build through eframe.
- **Quality checks with `scripts/quality.sh`:** one command for local runs, pre-commit and CI – a guard check against weakened checks, format, clippy, tests with coverage (`cargo llvm-cov`, base in `.quality-coverage`) and vulnerabilities in dependencies (`osv-scanner`); `cargo deny` stays a separate step. A strict lint profile under `[lints]` in `Cargo.toml` (among others no `&s[a..b]`, no `unsafe` except in three Windows calls, every block justified); three slices in `checksum.rs` and the `panic!` in `build.rs` were replaced for it, without changing behavior.
- **Efficiency:** The file list recomputes visible rows, counts per status and search keys only when rows, filter or search change, instead of in every frame; the selection finds its row through an index. The history card stores entries and texts per run once at load time and draws only the visible runs. After a run, the interface builds the history from what the run wrote instead of reading it again. The folder tree is rebuilt only after a change, tooltip texts are built only when shown, the drive type is asked once per folder.
- **Rust version:** `rust-version` in `Cargo.toml` says 1.99 like `rust-toolchain.toml` (egui 0.37 needs 1.95 anyway, the old value 1.89 was wrong). `libxkbcommon-x11-0` is now listed only under `depends` in the Debian package.
- **Less reading in the UI thread:** The “Replace …?” and delete confirmations count from the preview or the last create instead of reading the `.md5` again; after a run, checksum file and history come from the result. A replaced preview stops reading early; the request number `preview_gen` is gone, every request has a channel of its own.
- **Hashing without a second buffer:** Every thread reads into a reused 1 MiB buffer of its own instead of allocating 2 MiB per file (the `BufReader` in front of it gained nothing). Measured (release, MD5, one thread, files in the cache): 160 × 8 MiB 640 → 645 MiB/s, 20,000 × 16 KiB 65 → 173 MiB/s.
- **Tidied up:** `ui.rs` hands pure helpers to `preview.rs`, `list.rs`, `tree.rs`, `format.rs` and `system.rs`; a card scaffold `theme::card` for dialog cards, “About Veritas”, history and settings; tags and status colors in one place (`theme::Tag`); `run_scan` in phases with `FileRow` constructors; one count per status, one reader for `.md5`, dates in one place. Code used only by tests was removed or limited to tests.
- **Lean build** instead of a switch to Tauri (checked on 2026-11-19: AppImage 70–100 MB, more memory, porting would take weeks). The exe on Windows shrinks from 11.16 to 7.20 MB in four steps: release profile with `opt-level = "s"`, fat LTO and `strip` −1.95 MB (`"z"` got bigger at 9.52 MB; no `panic = "abort"`); no more embedded Noto Sans/DejaVu Sans Mono on Windows (Segoe UI, Consolas and Semibold come from the system, behind them Arial/Tahoma and egui's fonts as the last fallback) −1.99 MB; eframe feature `links` dropped −0.20 MB and about 35 fewer packages in `Cargo.lock` (`webbrowser`, `url`, ICU), the links in “About Veritas” are opened by `open`; static C runtime +0.18 MB, but `VCRUNTIME140.dll` (the same size) and with it the redistributable part of `build.ps1` are gone, so the script no longer needs Visual Studio tools. `image`/`png` stay (eframe and the clipboard pull them in). On Linux, Noto Sans and DejaVu Sans Mono stay embedded as a fallback, `default_fonts` is off there.
- **Hash throughput** (1 GiB, release, `hash_slice`): MD5 745 → 735 MiB/s (−1.3 %), SHA-256 2280 → 2255 MiB/s (−1 %). `md-5` and `sha2` run with `opt-level = 3`; without this override SHA-256 would drop by about 14 % and MD5 by 2 %.
- New test: every character of the interface texts has a glyph in the least favorable font chain without system fonts (Linux emulated on Windows, too); arrows that were missing in Noto Sans (embedded) and Ubuntu-Light now come from the monospace fonts.
- **egui and eframe 0.37** (before: 0.31): `App::ui` instead of `update`, panels inside a `Ui`, a new font API. The warnings about unmaintained crates (`ttf-parser`, `paste`) are gone with it. Only egui's fonts Ubuntu-Light, egui-icons and Hack are built in, no emoji fonts any more.
- `md-5` and `sha2` 0.11 (together, both on `digest` 0.11); `log` 0.4.35, `open` 5.4.5; `clipboard-win` 5.4.3 instead of a yanked version.
- Dependabot groups `egui`/`eframe` and `md-5`/`sha2` into one pull request each, even for big jumps.
- Every action in CI is pinned to a commit; Dependabot proposes grouped updates for actions and dependencies every month.
- **jiff instead of chrono** for timestamps: chrono has counted as unmaintained since November 2026 (RUSTSEC-2026-0543) and made `cargo deny` fail. The format in the history stays the same (RFC 3339 with `+00:00`); chrono now comes only through `trash`.
- `cargo deny check` (`deny.toml`: vulnerabilities, licenses, sources) replaces `cargo audit` in CI; clippy and tests run with `--locked`, every job has a time limit.
- New: `SECURITY.md`. The code of conduct names an e-mail address for reports.
- **Example file and license notices:** The test file `tests/fixtures/example.md5` has invented file names (the hash is the MD5 of the name); `LICENSE` is the unmodified Apache-2.0 text; `NOTICE` and `THIRD_PARTY.md` name versions, copyrights and sources and contain trademark notices; `.gitattributes` and `.gitignore` follow the repository standard.
- **CI:** The `workflows` job checks the workflow with `actionlint` and `zizmor`. The release job takes the notes from this changelog, creates the draft with `gh release create --verify-tag` and, in the public repository, produces a build provenance attestation (`gh attestation verify`). Windows, packages and install tests run on every push in the public repository, in a private one still only on tags and manual runs. `actions: write` is dropped, `cargo-deb` is pinned to 3.8.0, `dtolnay/rust-toolchain` points to a commit of the `master` line.
- “About Veritas” links `THIRD_PARTY.md` on `main` instead of on the version's tag (the tag may be missing).

## 0.7.2 – 2026-11-07 (never released separately, contained in 0.8.0)

DJI photos no longer count as damaged by mistake, and a program error leaves a trace.

### Added

- **Crash log** – a program error used to end the window without a trace. Now the message, code location and version are in `absturz.log`, and the next start points to it.

### Fixed

- **JPEGs with data after the end of the image** – some cameras (DJI) append a few KiB of padding. Veritas reported such photos as “possibly damaged” as soon as their hash changed. Now they count as intact; the reason names the bytes after the image.

### Under the hood

- JPEG: EOI is searched for in the last 64 KiB instead of 128 bytes and counts only after the start of the image data (SOS), so that the EOI of a thumbnail does not hide a truncated image. A header that reaches beyond the end of the file counts as damaged.
- New module `crash`: a panic hook that appends to `absturz.log` in eframe's data folder (starting over at 1 MiB).
- Instructions for AI agents are in `AGENTS.md`; `CLAUDE.md` only imports it.

## 0.7.1 – 2026-10-18

Verifying needs less memory and is faster on spinning hard disks (HDD).

### Fixed

- **Memory while verifying** – large files were read with mmap; on Windows the pages read stayed in the process. When verifying 57 GB of photos and videos, memory use rose to 2–4 GB, and Windows logged a memory warning. Veritas now reads every file buffered in pieces of 1 MiB.

### Changed

- **HDD: one file at a time** – otherwise the read head jumps between the files. On an 8 TB HDD, one thread managed 222–234 MB/s, 16 threads only 164–178 MB/s. “Threads” stays the upper limit for SSDs; if the chosen folder is on an HDD, the settings say so.

### Under the hood

- The dependency `memmap2` is gone.
- New module `drive`: `IOCTL_STORAGE_QUERY_PROPERTY` (*seek penalty*) on Windows, `/sys/dev/block/…/queue/rotational` on Linux.

## 0.7.0 – 2026-10-15

The history can be seen in Veritas itself, and files can be deleted into the trash.

### Added

- **History card** – all runs of a folder, the newest on top, and for each run which files it names: changed, new, missing, unreadable, with the hash before and after. A second tab shows all entries of one file. Opens with `Ctrl+H`, with a double-click on the “history” row and through the clock icon on a file; ↑/↓ picks the run, ←/→ the tab. “Open as JSON” opens the file itself.
- **Clock icon on the file** – only on files the history names, at the right of the “File name” column. For the selected file, the status bar names the last entry, for example “History: updated on 2025-09-13”, or since when it has been unchanged.
- **Move to the trash** – `Del` or right-click → “Move to Recycle Bin …”, always with a confirmation. A file from the `.md5` then appears as “missing” in the list until “Create MD5” drops it. Veritas does not delete the `.md5` or the history; on Windows it also deletes nothing on network and removable drives, because they have no Recycle Bin.
- **Context menu** on every row: “Open”, “History of the file”, “Move to Recycle Bin” – disabled items name the reason.

### Changed

- **F1 shows only “About Veritas”**; the tabs for the keys and for tips are gone. The keys are still in the tooltips and in the empty window. The icon in the toolbar is now an *i*.
- The “history” row opens the history card on double-click instead of another program.

### Under the hood

- New dependency `trash` (MIT) for the trash on Windows and Linux.
- The history is read once when the folder is opened and after every run, never per frame.

## 0.6.0 – 2026-10-15

Veritas shows the right state when opening, keeps every run in the history, opens files on double-click and follows design system v2 in the details too.

### Added

- **Double-click opens the file** – or Enter on the selected row – in the program the system provides for it. For a missing file, a notice says why that is not possible.
- **History instead of overwriting** – every “Create MD5” and every “Verify” appends a run to `{folder name}.json`; nothing drops out. A report in the earlier single-run format becomes the first run. Veritas makes no backup copy (`.md5.bak`): when it replaces a `.md5`, the history records what changed and, together with the new file, yields the old one. Unreadable files keep their old hash in the error entry.
- **Checksum file and history in the list** – as neutral rows “md5” and “history”; a double-click opens them.
- **Open a specific `.md5`** – drag it into the window, `veritas file.md5` or “Open with → Veritas”: then exactly this file applies as long as the folder stays selected.
- **`veritas <folder>`** opens the folder at start-up – as the Linux desktop entry (`Exec=veritas %f`) always intended.
- **Help with keys, tips and “About Veritas”** (`F1` or the help button): version, license, source code and prefilled links to report a bug or write a wish on GitHub.
- **Keyboard** – ↑/↓ change row or folder, ←/→ collapse and expand folders, Enter opens, `Ctrl+F` searches, `Ctrl+,` opens the settings, Esc undoes one level per press.
- **Progress** as a pill above the list: phase, files, MB/s and time remaining, with “Cancel”.
- **Settings persist** – theme, subfolders and threads survive a restart.

### Changed

- **Confirmation before replacing** as a dialog card instead of in the toolbar; Enter replaces, Esc keeps.
- **Disabled buttons name the reason** – “Verify” is disabled without a `.md5`, both buttons without a folder and during a run.
- **“pending” is grey** instead of blue; blue now means only “selected”. New files are called “new”, unreadable ones “error”.
- **The status appears as a tag** in the “Status” column; the list sits on a lighter surface so that all status colors read well in both themes.
- **Fixed chips** in the status bar: all seven always in the same place, with a fixed width; a count of 0 is greyed out.
- **The status line explains the selected row** – for “damaged” the reason (for example *PNG: file too short*), for “mismatch” the stored and the current hash.
- **After replacing**, every file appears in the list with its new hash; what changed compared with the old file is named by the notice.
- **Sizes, font and icons per design system v2** – toolbar 40 px, controls 32 px, status bar 28 px, rows 28 px; Lucide icons instead of drawn ones, the cog too.

### Fixed

- **Files in subfolders** appeared as “missing” when a folder was opened, until you verified.
- **Unreadable files** were missing from the list and the count; verifying still reported “completed”.
- **With “Subfolders” off**, “Verify” reported all entries in subfolders as missing, and replacing discarded their checksums. Now they stay unchanged.
- **Capitalization** – on Windows, `IMG.JPG` and `img.jpg` counted as two files (missing and new).
- **Large folders and network drives** slowed the interface down: Veritas queried drives and folders in every frame and drew all rows. 10,000 files now scroll smoothly.
- **Changing the folder during a run** is blocked; before, Veritas discarded the result although the file had been written.

### Under the hood

- `rfd` is removed; the `.deb` no longer needs GTK.
- The Lucide subset now comes from `lucide-static` 0.460.0 (`tools/make_lucide_subset.py`).
- Packages contain `NOTICE`, `THIRD_PARTY.md` and the license texts of all embedded fonts, including the egui replacement fonts.
- Tests check contrasts where the colors are painted, and the flow in the window without a display.

## 0.5.0 – 2026-10-15

### Changed

- The column is called “File name”; the chosen MD5 file is marked.
- When opening, the status appears at once, without hashing.
- A run can be stopped with a bar and “Cancel”, without writing a checksum file.

## 0.4.0 – 2026-10-15

### Fixed

- MD5 files with only one space between hash and file name (`HASH FILE`, as many Windows tools write them) are read.

## 0.3.0 – 2026-10-15

### Added

- The status counters are clickable and filter the list.

## 0.2.0 – 2026-10-15

### Changed

- The counters are in the status bar, the cog at the top right.

## 0.1.1 – 2026-10-15

### Added

- Folder tree on the left; theme, subfolders and threads in the cog.
- The MD5 file is named like the folder, otherwise the only or the newest `*.md5` applies.

### Fixed

- The Windows build compiles again (a test used Linux paths).

## 0.1.0 – 2026-10-15

The first version.

### Added

- MD5 and, optionally, SHA-256 checksums for folders, with a JSON report.
- Structure check on hash mismatches: JPEG, PNG, WebP, AVI, HEIC/HEIF, TIFF, MP4/MOV, MKV, Office Open XML and PDF.
- Native interface (egui/eframe) following design system v2; missing files are yellow.
- CI with Windows installer, portable zip, AppImage and `.deb`.

[Unreleased]: https://github.com/fly2nbc-oss/Veritas/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/fly2nbc-oss/Veritas/releases/tag/v0.8.0

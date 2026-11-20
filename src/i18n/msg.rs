//! Texts delivered by run, preview and structure check – some of which end up in the history
//! (`{folder name}.json`): reasons of the structure check (`integrity_reason`), error
//! messages per file (`message`), warnings of a run (`warnings`), plus progress phases and
//! reasons why a file is not in the folder.
//!
//! What is always written is the **English** template (`Msg::en`, decided on
//! 2026-11-20), independent of the interface language. Older histories contain the German one
//! (`Msg::de`). Display goes through `shown_in`: if it recognises a stored text by its English
//! or German template, it shows the template of the chosen language (`Texts::msg`) with the
//! same values; otherwise the text as it is (messages of the system, free texts of older
//! versions).
//!
//! The two templates here are a file format: whoever changes them changes what new histories
//! contain, and old ones are no longer recognised (they then appear untranslated). The display
//! in German is `de`, in English `en`; the other languages have their own templates in their
//! file.
//!
//! Placeholders in the templates, in the same order in every language:
//! - `{}` a value as it is (file name, path, format name, message of the system),
//! - `{n}` a number; the display writes it with the thousands separator of the language,
//! - `{p}` number with word, "3 files" (old: "3 Dateien"); the display writes it anew,
//! - `{m}` an embedded text that can itself be a template (translated along with it).

use super::Texts;

/// Creates `Msg`: name, explanation for the translators, German and English template.
macro_rules! messages {
    ($($(#[doc = $doc:literal])* $name:ident { de: $de:literal, en: $en:literal, })*) => {
        /// A text from run, preview or structure check (see module).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Msg {
            $($(#[doc = $doc])* $name,)*
        }

        impl Msg {
            /// All texts, in the order in which `shown_in` tries them.
            pub const ALL: &'static [Msg] = &[$(Msg::$name,)*];

            /// The German template: as in older histories, and the German display.
            pub const fn de(self) -> &'static str {
                match self {
                    $(Msg::$name => $de,)*
                }
            }

            /// The English template: how Veritas writes the text, and the English display.
            pub const fn en(self) -> &'static str {
                match self {
                    $(Msg::$name => $en,)*
                }
            }
        }
    };
}

messages! {
    // Progress phases (pill, tooltip). Only displayed, never stored.
    /// Progress before a run does anything.
    PhaseReady {
        de: "Bereit",
        en: "Ready",
    }
    /// Run, phase 1.
    PhaseReadMd5 {
        de: "Prüfsummendatei wird gelesen…",
        en: "Reading the checksum file…",
    }
    /// Run, phase 2: list the files in the folder.
    PhaseWalk {
        de: "Verzeichnis wird gelesen…",
        en: "Reading the folder…",
    }
    /// Create after verify: take over known checksums.
    PhaseMatch {
        de: "Bekannte Prüfsummen werden abgeglichen…",
        en: "Matching known checksums…",
    }
    /// Run, phase 3: hashing.
    PhaseHash {
        de: "Prüfsummen werden berechnet…",
        en: "Computing checksums…",
    }
    /// Structure check of deviating files.
    PhaseIntegrity {
        de: "Integrität wird geprüft…",
        en: "Checking integrity…",
    }
    /// Write `.md5` and history.
    PhaseWrite {
        de: "Dateien werden geschrieben…",
        en: "Writing files…",
    }
    /// Run finished.
    PhaseDone {
        de: "Fertig",
        en: "Done",
    }

    // Reasons why a file does not count as a regular file in the folder. Sentence parts after the
    // file name: "a.jpg is not in the chosen folder" – lowercase, no period.
    /// No folder chosen (also tooltip of locked buttons).
    NoFolder {
        de: "Erst links einen Ordner wählen.",
        en: "Choose a folder on the left first.",
    }
    /// Sentence part after the file name.
    NotInFolder {
        de: "liegt nicht im gewählten Ordner",
        en: "is not in the chosen folder",
    }
    /// Sentence part after the file name.
    IsLink {
        de: "ist ein Link oder liegt hinter einem; Veritas folgt Links nicht",
        en: "is a link or lies behind one; Veritas does not follow links",
    }
    /// Sentence part after the file name.
    NotRegularFile {
        de: "ist keine reguläre Datei",
        en: "is not a regular file",
    }
    /// Sentence part after the file name.
    NotAFileInFolder {
        de: "ist keine Datei im Ordner",
        en: "is not a file in the folder",
    }

    // Preview and history when opening; `{}` is a file name.
    /// The `.md5` is not UTF-8; `{}` file, `{}` encoding (e.g. Windows-1252).
    PreviewNotUtf8 {
        de: "{} ist kein UTF-8, als {} gelesen.",
        en: "{} is not UTF-8, read as {}.",
    }
    /// File not readable; `{}` file, `{}` message of the system.
    Unreadable {
        de: "{} ist nicht lesbar: {}",
        en: "{} is not readable: {}",
    }
    /// No history yet; `{}` name of the history.
    NoHistory {
        de: "Noch kein Verlauf. „MD5 erzeugen“ oder „Prüfen“ legt {} an.",
        en: "No history yet. “Create MD5” or “Verify” creates {}.",
    }
    /// `{}` name of the file.
    NotOurHistory {
        de: "{} ist kein Verlauf von Veritas.",
        en: "{} is not a Veritas history.",
    }
    /// `{}` name of the history.
    HistoryEmpty {
        de: "{} enthält noch keinen Lauf.",
        en: "{} contains no run yet.",
    }
    /// File over the limit; `{}` name, `{n}` size in MiB, `{n}` limit in MiB.
    TooLarge {
        de: "{} ist mit {n} MiB zu groß, Veritas liest höchstens {n} MiB. Ist es die richtige Datei?",
        en: "{} is too large at {n} MiB; Veritas reads at most {n} MiB. Is it the right file?",
    }

    // Errors per file ("error" status, `errors[].message` in the history).
    /// No permission to read.
    FileReadDenied {
        de: "Keine Leseberechtigung",
        en: "No permission to read",
    }
    /// File vanished between listing and reading.
    FileVanished {
        de: "Datei beim Lesen verschwunden",
        en: "File disappeared while reading",
    }
    /// `{m}` message of the system.
    FileReadError {
        de: "Lesefehler: {m}",
        en: "Read error: {m}",
    }
    /// `{}` path.
    NotRegular {
        de: "Keine reguläre Datei: {}",
        en: "Not a regular file: {}",
    }
    /// Folder points to itself; `{m}` path.
    SymlinkLoop {
        de: "Symlink-Schleife: {m}",
        en: "Symlink loop: {m}",
    }
    /// Folder without permission; `{m}` path.
    DirDenied {
        de: "Keine Berechtigung: {m}",
        en: "No permission: {m}",
    }
    /// Folder not readable; `{m}` path.
    DirReadError {
        de: "Lesefehler im Verzeichnis: {m}",
        en: "Error reading the folder: {m}",
    }
    /// Location of an error the system does not name (instead of a path).
    UnknownPlace {
        de: "(unbekannt)",
        en: "(unknown)",
    }
    /// File name cannot be displayed.
    BadName {
        de: "Dateiname ist kein gültiges UTF-8 oder liegt außerhalb des Ordners",
        en: "File name is not valid UTF-8 or lies outside the folder",
    }
    /// Entry below an unreadable folder; `{m}` the error of the folder.
    NotChecked {
        de: "Nicht geprüft – {m}",
        en: "Not checked – {m}",
    }
    /// Line of a cancelled run.
    RunCancelledRow {
        de: "Lauf abgebrochen, kein Abgleich.",
        en: "Run cancelled, not compared.",
    }

    // Errors of a run (`Error`, notice and status bar). `{}` path, `{}` message of the
    // system.
    /// `{}` path.
    NotADirectory {
        de: "Kein Ordner: {}",
        en: "Not a folder: {}",
    }
    ReadDeniedAt {
        de: "Keine Leseberechtigung: {} ({})",
        en: "No permission to read: {} ({})",
    }
    ReadDeniedPlain {
        de: "Keine Leseberechtigung: {}",
        en: "No permission to read: {}",
    }
    NotFoundAt {
        de: "Nicht gefunden: {} ({})",
        en: "Not found: {} ({})",
    }
    NotFoundPlain {
        de: "Nicht gefunden: {}",
        en: "Not found: {}",
    }
    IoErrorAt {
        de: "Ein-/Ausgabefehler: {} ({})",
        en: "Input/output error: {} ({})",
    }
    IoErrorPlain {
        de: "Ein-/Ausgabefehler: {}",
        en: "Input/output error: {}",
    }
    WriteDenied {
        de: "Keine Schreibberechtigung: {} ({})",
        en: "No permission to write: {} ({})",
    }
    WriteReadOnly {
        de: "Der Datenträger ist schreibgeschützt: {} ({})",
        en: "The drive is write-protected: {} ({})",
    }
    WriteFull {
        de: "Der Datenträger ist voll: {} ({})",
        en: "The drive is full: {} ({})",
    }
    /// An old `.partial` could not be replaced.
    WriteInTheWay {
        de: "Eine Zwischendatei liegt im Weg: {} ({})",
        en: "A temporary file is in the way: {} ({})",
    }
    WriteFailed {
        de: "Schreibfehler: {} ({})",
        en: "Write error: {} ({})",
    }
    /// `{}` path of the `.md5`.
    ChecksumMissing {
        de: "Keine Prüfsummendatei gefunden: {}. Mit „MD5 erzeugen“ anlegen.",
        en: "No checksum file found: {}. Create it with “Create MD5”.",
    }
    /// History could not be written as JSON; `{}` message.
    JsonFailed {
        de: "JSON konnte nicht erzeugt werden: {}",
        en: "JSON could not be created: {}",
    }
    RunCancelled {
        de: "Lauf abgebrochen",
        en: "Run cancelled",
    }
    /// `{}` message of the system.
    ThreadPool {
        de: "Thread-Pool konnte nicht gestartet werden: {}",
        en: "The thread pool could not be started: {}",
    }
    /// History written, `.md5` not; `{m}` the write error.
    RenameFailed {
        de: "{m}. Der Verlauf nennt diesen Lauf schon, die Prüfsummendatei blieb unverändert.",
        en: "{m}. The history already lists this run; the checksum file stayed unchanged.",
    }

    // Warnings of a run (status bar, history card; `warnings` in the history).
    /// Create after verify; `{p}` checksums, `{p}` files read.
    ReuseFromVerify {
        de: "{p} aus dem letzten Prüfen übernommen (Größe und Änderungszeit unverändert), {p} gelesen.",
        en: "{p} reused from the last verification (size and modification time unchanged), {p} read.",
    }
    /// Create after create; `{p}` checksums, `{p}` files read.
    ReuseFromGenerate {
        de: "{p} aus dem letzten Erzeugen übernommen (Größe und Änderungszeit unverändert), {p} gelesen.",
        en: "{p} reused from the last creation (size and modification time unchanged), {p} read.",
    }
    /// Create after a run whose checksums no longer fit.
    KnownStale {
        de: "Die Prüfsummen des letzten Laufs passen nicht mehr (Prüfsummendatei oder Einstellungen geändert); alle Dateien wurden gelesen.",
        en: "The checksums of the last run no longer fit (checksum file or settings changed); all files were read.",
    }
    /// Create with subfolders off; `{n}` entries.
    NestedKept {
        de: "{n} Einträge in Unterordnern unverändert übernommen (Unterordner ist aus).",
        en: "{n} entries in subfolders kept as they are (Subfolders is off).",
    }
    /// Verify with subfolders off; `{n}` entries.
    NestedSkipped {
        de: "{n} Einträge in Unterordnern nicht geprüft (Unterordner ist aus).",
        en: "{n} entries in subfolders not verified (Subfolders is off).",
    }
    /// Create; `{n}` entries below unreadable folders.
    BelowUnreadableKept {
        de: "{n} Einträge liegen in nicht lesbaren Ordnern: nicht geprüft, unverändert übernommen.",
        en: "{n} entries are in folders that cannot be read: not verified, kept as they are.",
    }
    /// Verify; `{n}` entries below unreadable folders.
    BelowUnreadable {
        de: "{n} Einträge liegen in nicht lesbaren Ordnern: nicht geprüft.",
        en: "{n} entries are in folders that cannot be read: not verified.",
    }
    /// Cancellation during the structure check.
    AbortedIntegrity {
        de: "Abgebrochen während der Integritätsprüfung. Es wurde nichts geschrieben.",
        en: "Cancelled during the integrity check. Nothing was written.",
    }
    /// MD5 and SHA-256 at once, a file is missing; `{}` path.
    AlgoMissing {
        de: "{} fehlt, dieser Algorithmus wird nicht verglichen.",
        en: "{} is missing; this algorithm is not compared.",
    }
    /// Create replaces an unusable `.md5`; `{m}` the reason.
    ReplacingInvalid {
        de: "Vorherige Prüfsummendatei wird ersetzt: {m}",
        en: "The previous checksum file is being replaced: {m}",
    }
    /// `.md5` without a valid line; `{}` path, `{m}` the first reasons.
    ChecksumInvalid {
        de: "Prüfsummendatei ungültig ({}): {m}",
        en: "Checksum file invalid ({}): {m}",
    }
    /// `{}` path, `{}` encoding.
    NotUtf8Read {
        de: "{} ist kein UTF-8 und wurde als {} gelesen.",
        en: "{} is not UTF-8 and was read as {}.",
    }
    /// Line of the `.md5`; `{n}` line number, `{m}` reason.
    Line {
        de: "Zeile {n}: {m}",
        en: "line {n}: {m}",
    }
    /// Line of the `.md5`; `{}` path of the `.md5`, `{n}` line number, `{m}` reason.
    LineWarning {
        de: "{}: Zeile {n}: {m}",
        en: "{}: line {n}: {m}",
    }
    /// `{}` path of the `.md5`, `{}` algorithm, `{}` entry.
    HashLenMismatch {
        de: "{}: Hashlänge passt nicht zu {}: {}",
        en: "{}: hash length does not fit {}: {}",
    }
    /// `{n}` links.
    SymlinksSkipped {
        de: "{n} symbolische Links übersprungen (Veritas folgt Links nicht).",
        en: "{n} symbolic links skipped (Veritas does not follow links).",
    }
    /// `{n}` files.
    SpecialSkipped {
        de: "{n} Sonderdateien übersprungen (keine regulären Dateien).",
        en: "{n} special files skipped (not regular files).",
    }
    /// Create aborts before writing; `{m}` the reason.
    NothingWritten {
        de: "{m} Es wurde nichts geschrieben.",
        en: "{m} Nothing was written.",
    }
    /// Verify, history not writable; `{m}` the reason.
    NotInHistory {
        de: "{m} Dieser Lauf steht nicht im Verlauf.",
        en: "{m} This run is not in the history.",
    }
    /// `{}` name, `{}` message of the system.
    HistoryUnreadable {
        de: "Der Verlauf {} ist nicht lesbar ({}) und bleibt unverändert.",
        en: "The history {} is not readable ({}) and stays unchanged.",
    }
    /// `{}` name, `{}` format name.
    HistoryOtherFormat {
        de: "Der Verlauf {} hat das Format „{}“ einer anderen Veritas-Version; diese Version überschreibt ihn nicht.",
        en: "The history {} has the format “{}” of another Veritas version; this version does not overwrite it.",
    }
    /// `{}` name.
    HistoryReplaced {
        de: "{} war kein Veritas-Verlauf und wurde ersetzt.",
        en: "{} was not a Veritas history and was replaced.",
    }

    // Reasons why a line of the `.md5` does not count (after "line 3: ").
    /// `{}` path.
    DuplicatePath {
        de: "Pfad {} mehrfach, letzter Hash gilt",
        en: "path {} more than once, the last hash counts",
    }
    /// Up to 0.8.0 (only MD5 and SHA-256); stays so that older histories are recognised.
    HashLength {
        de: "Hash muss 32 (MD5) oder 64 (SHA-256) Hex-Zeichen haben",
        en: "the hash must have 32 (MD5) or 64 (SHA-256) hex digits",
    }
    HashLengthAny {
        de: "Hash muss 32 (MD5), 40 (SHA-1), 64 (SHA-256) oder 128 (SHA-512) Hex-Zeichen haben",
        en: "the hash must have 32 (MD5), 40 (SHA-1), 64 (SHA-256) or 128 (SHA-512) hex digits",
    }
    /// BSD format `SHA256 (file) = hash`; `{}` is the algorithm in front ("SHA-256").
    TagLength {
        de: "Zeile im BSD-Format braucht „(Datei) = Hash“ mit einem {}-Hash",
        en: "a BSD-style line needs “(file) = hash” with a {} hash",
    }
    NoSeparator {
        de: "nach dem Hash fehlen zwei Leerzeichen oder eine Binär-Markierung",
        en: "two spaces or a binary mark are missing after the hash",
    }
    PathMissing {
        de: "Pfad fehlt",
        en: "path missing",
    }
    PathOutside {
        de: "Pfad darf den Ordner nicht verlassen",
        en: "the path must not leave the folder",
    }
    PathNul {
        de: "Pfad enthält ein Nullzeichen",
        en: "the path contains a null character",
    }
    PathRelative {
        de: "Pfad muss relativ sein",
        en: "the path must be relative",
    }
    PathDrive {
        de: "Pfad muss relativ sein (Laufwerk oder Doppelpunkt)",
        en: "the path must be relative (drive or colon)",
    }

    // Reasons of the structure check (status bar, tooltip, history card; `integrity_reason` in
    // the history). Technical terms of the formats (SOI, EOI, IEND, ftyp, moov, EBML, EOCD)
    // stay as they are. `{}` at the start is the format name (JPEG, PNG, WebP, AVI, HEIF/HEIC,
    // TIFF, BigTIFF, PDF).
    ReadDenied {
        de: "Datei nicht lesbar: keine Berechtigung",
        en: "File not readable: no permission",
    }
    /// `{}` message of the system.
    ReadFailed {
        de: "Datei nicht lesbar: {}",
        en: "File not readable: {}",
    }
    Empty {
        de: "Datei ist leer",
        en: "File is empty",
    }
    HeadUnreadableAny {
        de: "Dateikopf nicht lesbar",
        en: "File header not readable",
    }
    UnknownFormat {
        de: "Unbekanntes Dateiformat, keine Strukturprüfung",
        en: "Unknown file format, no structure check",
    }
    TooShort {
        de: "{}: Datei zu kurz",
        en: "{}: file too short",
    }
    HeadUnreadable {
        de: "{}: Dateikopf nicht lesbar",
        en: "{}: file header not readable",
    }
    TailUnreadable {
        de: "{}: Dateiende nicht lesbar",
        en: "{}: end of file not readable",
    }
    JpegNoSoi {
        de: "JPEG: SOI (FF D8) fehlt",
        en: "JPEG: SOI (FF D8) missing",
    }
    JpegOk {
        de: "JPEG: SOI (FF D8) und EOI (FF D9) vorhanden",
        en: "JPEG: SOI (FF D8) and EOI (FF D9) present",
    }
    /// `{n}` bytes after the end of the image.
    JpegOkTrailing {
        de: "JPEG: SOI (FF D8) und EOI (FF D9) vorhanden, danach {n} Bytes weitere Daten",
        en: "JPEG: SOI (FF D8) and EOI (FF D9) present, then {n} more bytes",
    }
    JpegNoEoi {
        de: "JPEG: EOI (FF D9) fehlt am Dateiende",
        en: "JPEG: EOI (FF D9) missing at the end of the file",
    }
    JpegCut {
        de: "JPEG: Datei endet vor den Bilddaten",
        en: "JPEG: file ends before the image data",
    }
    PngNoSignature {
        de: "PNG: Signatur fehlt",
        en: "PNG: signature missing",
    }
    PngNoIend {
        de: "PNG: IEND-Chunk fehlt am Dateiende",
        en: "PNG: IEND chunk missing at the end of the file",
    }
    PngChunkUnreadable {
        de: "PNG: Chunk nicht lesbar",
        en: "PNG: chunk not readable",
    }
    PngChunkPastEnd {
        de: "PNG: Chunk reicht über das Dateiende, IEND fehlt",
        en: "PNG: chunk reaches past the end of the file, IEND missing",
    }
    PngOk {
        de: "PNG: Signatur und IEND-Chunk vorhanden",
        en: "PNG: signature and IEND chunk present",
    }
    /// `{n}` bytes after IEND.
    PngOkTrailing {
        de: "PNG: Signatur und IEND-Chunk vorhanden, danach {n} Bytes angehängt",
        en: "PNG: signature and IEND chunk present, then {n} bytes appended",
    }
    PngManyChunks {
        de: "PNG: sehr viele Chunks, Ende nicht geprüft",
        en: "PNG: very many chunks, end not checked",
    }
    RiffNoSignature {
        de: "{}: RIFF-Signatur fehlt",
        en: "{}: RIFF signature missing",
    }
    RiffTooLarge {
        de: "{}: RIFF-Größe überschreitet das Dateiende",
        en: "{}: RIFF size exceeds the end of the file",
    }
    RiffOk {
        de: "{}: RIFF-Signatur vorhanden",
        en: "{}: RIFF signature present",
    }
    HeifNoFtyp {
        de: "HEIF/HEIC: ftyp-Box fehlt",
        en: "HEIF/HEIC: ftyp box missing",
    }
    HeifFtypShort {
        de: "HEIF/HEIC: ftyp-Box unvollständig",
        en: "HEIF/HEIC: ftyp box incomplete",
    }
    /// `{}` the marker (four characters).
    HeifBrand {
        de: "HEIF/HEIC: ftyp mit Marke {}",
        en: "HEIF/HEIC: ftyp with brand {}",
    }
    HeifNoBrand {
        de: "HEIF/HEIC: ftyp ohne heic/heif/mif1/msf1-Marke",
        en: "HEIF/HEIC: ftyp without a heic/heif/mif1/msf1 brand",
    }
    TiffShort {
        de: "{}: Datei kürzer als der Header",
        en: "{}: file shorter than the header",
    }
    TiffNoSignature {
        de: "TIFF: Signatur II* oder MM* fehlt",
        en: "TIFF: signature II* or MM* missing",
    }
    TiffIfdOutside {
        de: "{}: IFD-Offset liegt außerhalb der Datei",
        en: "{}: IFD offset lies outside the file",
    }
    TiffLittle {
        de: "TIFF: Little-Endian-Header und IFD-Offset gültig",
        en: "TIFF: little-endian header and IFD offset valid",
    }
    TiffBig {
        de: "TIFF: Big-Endian-Header und IFD-Offset gültig",
        en: "TIFF: big-endian header and IFD offset valid",
    }
    BigTiffBad {
        de: "BigTIFF: Header ungültig (Offsetgröße nicht 8)",
        en: "BigTIFF: header invalid (offset size not 8)",
    }
    BigTiffOk {
        de: "BigTIFF: Header und IFD-Offset gültig",
        en: "BigTIFF: header and IFD offset valid",
    }
    /// `{}` name of the atom (e.g. mdat).
    Mp4AtomCut {
        de: "MP4/MOV: Atom {} reicht über das Dateiende (abgeschnitten)",
        en: "MP4/MOV: atom {} reaches past the end of the file (truncated)",
    }
    /// `{n}` bytes after the atoms.
    Mp4Trailing {
        de: "MP4/MOV: Atome vollständig, danach {n} Bytes angehängt",
        en: "MP4/MOV: atoms complete, then {n} bytes appended",
    }
    Mp4CutBeforeMoov {
        de: "MP4/MOV: ftyp vorhanden, Atomstruktur vor moov abgeschnitten",
        en: "MP4/MOV: ftyp present, atom structure truncated before moov",
    }
    Mp4AtomTooLarge {
        de: "MP4/MOV: Atomgröße überschreitet das Dateiende",
        en: "MP4/MOV: atom size exceeds the end of the file",
    }
    Mp4HeaderUnreadable {
        de: "MP4/MOV: Atom-Header nicht lesbar",
        en: "MP4/MOV: atom header not readable",
    }
    Mp4ManyAtoms {
        de: "MP4/MOV: moov vorhanden, sehr viele Atome, Ende nicht geprüft",
        en: "MP4/MOV: moov present, very many atoms, end not checked",
    }
    Mp4Ok {
        de: "MP4/MOV: ftyp und moov vorhanden",
        en: "MP4/MOV: ftyp and moov present",
    }
    Mp4QuickTime {
        de: "MP4/MOV: moov vorhanden (QuickTime ohne ftyp)",
        en: "MP4/MOV: moov present (QuickTime without ftyp)",
    }
    Mp4NoFtyp {
        de: "MP4/MOV: ftyp-Box fehlt",
        en: "MP4/MOV: ftyp box missing",
    }
    Mp4NoMoov {
        de: "MP4/MOV: moov-Box im durchsuchten Bereich nicht gefunden",
        en: "MP4/MOV: moov box not found in the searched range",
    }
    MkvNoHeader {
        de: "MKV: EBML-Header 1A 45 DF A3 fehlt",
        en: "MKV: EBML header 1A 45 DF A3 missing",
    }
    MkvPastEnd {
        de: "MKV: EBML-Header über das Dateiende hinaus",
        en: "MKV: EBML header reaches past the end of the file",
    }
    MkvShort {
        de: "MKV: EBML-Header unvollständig",
        en: "MKV: EBML header incomplete",
    }
    MkvOk {
        de: "MKV: EBML-Header vorhanden",
        en: "MKV: EBML header present",
    }
    MkvLength {
        de: "MKV: EBML-Länge nicht lesbar",
        en: "MKV: EBML length not readable",
    }
    PdfNoHead {
        de: "PDF: Kopf %PDF- fehlt",
        en: "PDF: header %PDF- missing",
    }
    PdfOk {
        de: "PDF: Kopf %PDF- und %%EOF im letzten Fenster (2048 Bytes)",
        en: "PDF: header %PDF- and %%EOF in the last window (2048 bytes)",
    }
    PdfNoEof {
        de: "PDF: %%EOF fehlt im letzten Fenster (2048 Bytes)",
        en: "PDF: %%EOF missing in the last window (2048 bytes)",
    }
    OfficeShort {
        de: "Office-Datei zu kurz für ein ZIP",
        en: "Office file too short for a ZIP",
    }
    OfficeUnreadable {
        de: "Office-Datei nicht lesbar",
        en: "Office file not readable",
    }
    OfficeOle {
        de: "Office-Datei verschlüsselt oder im OLE-Format, Struktur nicht geprüft",
        en: "Office file encrypted or in OLE format, structure not checked",
    }
    OfficeNoZip {
        de: "Office-Datei beginnt nicht mit einem ZIP-Local-Header",
        en: "Office file does not start with a ZIP local header",
    }
    ZipNoEocd {
        de: "ZIP-Endverzeichnis (EOCD) fehlt",
        en: "ZIP end of central directory (EOCD) missing",
    }
    ZipEocdUnreadable {
        de: "ZIP-Endverzeichnis nicht lesbar",
        en: "ZIP end of central directory not readable",
    }
    ZipDirOutside {
        de: "ZIP-Inhaltsverzeichnis liegt außerhalb der Datei",
        en: "ZIP central directory lies outside the file",
    }
    ZipDirHuge {
        de: "ZIP-Inhaltsverzeichnis sehr groß, nicht geprüft",
        en: "ZIP central directory very large, not checked",
    }
    OoxmlOk {
        de: "Office-Open-XML: ZIP-Struktur und [Content_Types].xml vorhanden",
        en: "Office Open XML: ZIP structure and [Content_Types].xml present",
    }
    NoContentTypes {
        de: "[Content_Types].xml fehlt",
        en: "[Content_Types].xml missing",
    }
    ZipManyEntries {
        de: "ZIP mit sehr vielen Einträgen, nicht vollständig geprüft",
        en: "ZIP with very many entries, not fully checked",
    }
    /// `{}` message of the system.
    ZipDirUnreadableWhy {
        de: "ZIP-Inhaltsverzeichnis nicht lesbar: {}",
        en: "ZIP central directory not readable: {}",
    }
    ZipDirUnreadable {
        de: "ZIP-Inhaltsverzeichnis nicht lesbar",
        en: "ZIP central directory not readable",
    }
    ZipDirBroken {
        de: "ZIP-Inhaltsverzeichnis beschädigt",
        en: "ZIP central directory damaged",
    }
    ZipNameUnreadable {
        de: "ZIP-Dateiname nicht lesbar",
        en: "ZIP file name not readable",
    }
    ZipLocalMissing {
        de: "Lokaler ZIP-Header von [Content_Types].xml fehlt",
        en: "Local ZIP header of [Content_Types].xml missing",
    }
    ZipLocalUnreadable {
        de: "Lokaler ZIP-Header nicht lesbar",
        en: "Local ZIP header not readable",
    }
    ZipLocalBad {
        de: "Lokaler ZIP-Header von [Content_Types].xml ungültig",
        en: "Local ZIP header of [Content_Types].xml invalid",
    }
    /// Cancellation before the structure check of this file.
    RunCancelledStructure {
        de: "Lauf abgebrochen, Struktur nicht geprüft",
        en: "Run cancelled, structure not checked",
    }
}

/// The placeholders of a template.
const MARKS: [&str; 4] = ["{}", "{n}", "{p}", "{m}"];

impl Msg {
    /// The text as Veritas writes it: the English template with the values.
    pub fn fill(self, args: &[&str]) -> String {
        fill(self.en(), args)
    }
}

/// Puts `args` into the placeholders of `template` in order (of every kind).
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out =
        String::with_capacity(template.len() + args.iter().map(|arg| arg.len()).sum::<usize>());
    let mut rest = template;
    let mut args = args.iter();
    while let Some((at, mark)) = next_mark(rest) {
        out.push_str(rest.get(..at).unwrap_or_default());
        out.push_str(args.next().copied().unwrap_or_default());
        rest = rest.get(at + mark.len()..).unwrap_or_default();
    }
    out.push_str(rest);
    out
}

/// The first placeholder in `text`: position and placeholder.
fn next_mark(text: &str) -> Option<(usize, &'static str)> {
    MARKS
        .iter()
        .filter_map(|mark| text.find(mark).map(|at| (at, *mark)))
        .min_by_key(|(at, _)| *at)
}

/// The placeholders of a template in their order – the same in every language.
pub fn marks(template: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some((at, mark)) = next_mark(rest) {
        out.push(mark);
        rest = rest.get(at + mark.len()..).unwrap_or_default();
    }
    out
}

/// The values if `text` is built from `template`; each placeholder takes as little as
/// possible, the last one the rest.
fn captures(template: &str, text: &str) -> Option<Vec<(&'static str, String)>> {
    let kinds = marks(template);
    if kinds.is_empty() {
        return (template == text).then(Vec::new);
    }
    let mut literals = Vec::with_capacity(kinds.len() + 1);
    let mut rest = template;
    while let Some((at, mark)) = next_mark(rest) {
        literals.push(rest.get(..at).unwrap_or_default());
        rest = rest.get(at + mark.len()..).unwrap_or_default();
    }
    literals.push(rest);
    let (first, middle_and_last) = literals.split_first()?;
    let (last, middle) = middle_and_last.split_last()?;
    let mut rest = text.strip_prefix(first)?.strip_suffix(last)?;
    let mut values = Vec::with_capacity(kinds.len());
    for (kind, literal) in kinds.iter().zip(middle) {
        let (value, after) = rest.split_once(literal)?;
        values.push((*kind, value.to_string()));
        rest = after;
    }
    values.push((*kinds.last()?, rest.to_string()));
    Some(values)
}

/// The order of recognition: first templates without placeholders, so that e.g. "Cancelled
/// during … Nothing was written." does not count as "{m} Nothing was written."; within each
/// group English first (new histories), then German (older histories).
fn candidates() -> impl Iterator<Item = (Msg, &'static str)> {
    let forms = |fixed: bool| {
        Msg::ALL
            .iter()
            .map(|msg| (*msg, msg.en()))
            .chain(Msg::ALL.iter().map(|msg| (*msg, msg.de())))
            .filter(move |(_, template)| marks(template).is_empty() == fixed)
    };
    forms(true).chain(forms(false))
}

/// Which text `text` is, with its values (kind of placeholder, value).
fn recognise(text: &str) -> Option<(Msg, Vec<(&'static str, String)>)> {
    candidates().find_map(|(msg, template)| captures(template, text).map(|values| (msg, values)))
}

/// The text in the language of `t`: if it fits a template, that of the language with the same
/// values; otherwise unchanged.
pub fn shown_in(t: &Texts, text: &str) -> String {
    let Some((msg, values)) = recognise(text) else {
        return text.to_string();
    };
    let values: Vec<String> = values
        .into_iter()
        .map(|(kind, value)| value_in(t, kind, &value))
        .collect();
    let values: Vec<&str> = values.iter().map(String::as_str).collect();
    fill((t.msg)(msg), &values)
}

/// A value in the language of `t`, depending on the placeholder.
fn value_in(t: &Texts, kind: &str, value: &str) -> String {
    match kind {
        "{n}" => stored_count(value)
            .map(|count| super::count_in(t, count))
            .unwrap_or_else(|| value.to_string()),
        "{p}" => phrase_in(t, value).unwrap_or_else(|| value.to_string()),
        "{m}" => shown_in(t, value),
        _ => value.to_string(),
    }
}

/// "1234", "12" or old "1.234" → number; otherwise nothing.
fn stored_count(text: &str) -> Option<usize> {
    let mut groups = text.split('.');
    let head = groups.next()?;
    if head.is_empty() || head.len() > 3 && text.contains('.') {
        return None;
    }
    let mut digits = head.to_string();
    for group in groups {
        if group.len() != 3 {
            return None;
        }
        digits.push_str(group);
    }
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// "3 files", "1 checksum" (old "3 Dateien", "1 Prüfsumme") in the language of `t`.
fn phrase_in(t: &Texts, text: &str) -> Option<String> {
    let (count, word) = text.split_once(' ')?;
    let count = stored_count(count)?;
    match word {
        "file" | "files" | "Datei" | "Dateien" => Some((t.files)(count)),
        "checksum" | "checksums" | "Prüfsumme" | "Prüfsummen" => Some((t.checksums)(count)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    /// Example values per placeholder; `{m}` another known text.
    fn sample(kind: &str, index: usize, de: bool) -> String {
        match (kind, de) {
            ("{n}", _) => format!("{}", 1200 + index),
            ("{p}", false) => format!("{} files", 3 + index),
            ("{p}", true) => format!("{} Dateien", 3 + index),
            ("{m}", false) => Msg::PathMissing.en().to_string(),
            ("{m}", true) => Msg::PathMissing.de().to_string(),
            _ => format!("wert{index}.jpg"),
        }
    }

    /// Every text is recognised as exactly this text – English as well as German, with its
    /// values. Otherwise the display would show a wrong template.
    #[test]
    fn every_message_is_recognised_as_itself() {
        for de in [false, true] {
            for msg in Msg::ALL {
                let template = if de { msg.de() } else { msg.en() };
                let values: Vec<String> = marks(template)
                    .iter()
                    .enumerate()
                    .map(|(index, kind)| sample(kind, index, de))
                    .collect();
                let refs: Vec<&str> = values.iter().map(String::as_str).collect();
                let text = fill(template, &refs);
                let found = recognise(&text).map(|(found, _)| found);
                assert_eq!(found, Some(*msg), "{text}");
            }
        }
    }

    /// An old German text appears in English as Veritas writes it today, and a new English one
    /// in German as Veritas wrote it before the language choice.
    #[test]
    fn old_and_new_texts_meet() {
        let (de, en) = (Lang::De.texts(), Lang::En.texts());
        for msg in Msg::ALL {
            let de_values: Vec<String> = marks(msg.de())
                .iter()
                .enumerate()
                .map(|(index, kind)| sample(kind, index, true))
                .collect();
            let en_values: Vec<String> = marks(msg.en())
                .iter()
                .enumerate()
                .map(|(index, kind)| sample(kind, index, false))
                .collect();
            let de_refs: Vec<&str> = de_values.iter().map(String::as_str).collect();
            let en_refs: Vec<&str> = en_values.iter().map(String::as_str).collect();
            let old = fill(msg.de(), &de_refs);
            let new = msg.fill(&en_refs);
            // Numbers get thousands separators in the display.
            let plain = |text: String| text.replace([',', '.'], "");
            assert_eq!(plain(shown_in(en, &old)), plain(new.clone()), "{msg:?}");
            assert_eq!(plain(shown_in(de, &new)), plain(old), "{msg:?}");
        }
    }

    /// Every language has the same placeholders in the same order in every template.
    #[test]
    fn every_language_keeps_the_placeholders() {
        for msg in Msg::ALL {
            assert_eq!(marks(msg.en()), marks(msg.de()), "{msg:?}");
        }
        for lang in Lang::ALL {
            let t = lang.texts();
            for msg in Msg::ALL {
                let own = (t.msg)(*msg);
                assert_eq!(marks(own), marks(msg.en()), "{lang:?} {msg:?}: {own}");
                assert!(!own.trim().is_empty(), "{lang:?} {msg:?}");
            }
        }
    }

    #[test]
    fn stored_text_is_shown_in_the_language() {
        let (de, en) = (Lang::De.texts(), Lang::En.texts());
        let reuse = Msg::ReuseFromVerify.fill(&["1234 checksums", "1 file"]);
        let shown = shown_in(de, &reuse);
        assert!(
            shown.contains("1.234 Prüfsummen") && shown.contains("1 Datei"),
            "{shown}"
        );
        let old = fill(Msg::ReuseFromVerify.de(), &["1.234 Prüfsummen", "1 Datei"]);
        let shown = shown_in(en, &old);
        assert!(
            shown.contains("1,234 checksums") && shown.contains("1 file"),
            "{shown}"
        );
        let nested = Msg::NotChecked.fill(&[&Msg::DirDenied.fill(&["fotos"])]);
        assert_eq!(
            shown_in(de, &nested),
            "Nicht geprüft – Keine Berechtigung: fotos"
        );
        let jpeg = Msg::TooShort.fill(&["JPEG"]);
        assert_eq!(shown_in(de, &jpeg), "JPEG: Datei zu kurz");
        // Unknown (message of the system, free text of an older version): stays.
        assert_eq!(
            shown_in(en, "Zugriff verweigert (os error 5)"),
            "Zugriff verweigert (os error 5)"
        );
        // Without placeholders first: not "{m} Nothing was written."
        assert_eq!(
            shown_in(de, Msg::AbortedIntegrity.en()),
            Msg::AbortedIntegrity.de()
        );
    }

    #[test]
    fn stored_counts_are_read() {
        assert_eq!(stored_count("12"), Some(12));
        assert_eq!(stored_count("1.234.567"), Some(1_234_567));
        assert_eq!(stored_count("1234"), Some(1234));
        assert_eq!(stored_count("12.34"), None);
        assert_eq!(stored_count("1234.567"), None);
        assert_eq!(stored_count("x"), None);
        assert_eq!(stored_count(""), None);
    }
}

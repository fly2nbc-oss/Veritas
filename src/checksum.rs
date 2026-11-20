use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read};
use std::path::Path;
use std::time::SystemTime;

use crate::hashing::HashMode;
use crate::i18n::Msg;
use crate::report::{path_key, FOLD_CASE};

/// Largest checksum file Veritas reads: 256 MiB, a little over two million lines.
pub(crate) const MAX_MD5_BYTES: u64 = 256 * 1024 * 1024;
/// Largest history Veritas reads. The history grows without a cap; the limit only guards
/// against a huge foreign file.
pub(crate) const MAX_HISTORY_BYTES: u64 = 1024 * 1024 * 1024;

/// `checksums.md5` or `<folder name>.md5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameStyle {
    Checksums,
    FolderName,
}

/// The files of a run: the checksum file per algorithm, and the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputNames {
    pub stem: String,
    pub md5: String,
    pub sha256: String,
    pub sha1: String,
    pub sha512: String,
    pub json: String,
}

impl OutputNames {
    fn from_stem(stem: String) -> Self {
        Self {
            md5: format!("{stem}.md5"),
            sha256: format!("{stem}.sha256"),
            sha1: format!("{stem}.sha1"),
            sha512: format!("{stem}.sha512"),
            json: format!("{stem}.json"),
            stem,
        }
    }

    /// The checksum file of the algorithm `algo`; `Both` has no single one (MD5).
    pub fn file(&self, algo: HashMode) -> &str {
        match algo {
            HashMode::Md5 | HashMode::Both => &self.md5,
            HashMode::Sha1 => &self.sha1,
            HashMode::Sha256 => &self.sha256,
            HashMode::Sha512 => &self.sha512,
        }
    }

    fn file_mut(&mut self, algo: HashMode) -> &mut String {
        match algo {
            HashMode::Md5 | HashMode::Both => &mut self.md5,
            HashMode::Sha1 => &mut self.sha1,
            HashMode::Sha256 => &mut self.sha256,
            HashMode::Sha512 => &mut self.sha512,
        }
    }
}

pub fn output_names(root: &Path, style: NameStyle) -> OutputNames {
    let stem = match style {
        NameStyle::Checksums => "checksums".to_string(),
        NameStyle::FolderName => folder_stem(root),
    };
    OutputNames::from_stem(stem)
}

/// File names in the root folder that Veritas writes itself and does not hash, as
/// comparison keys (`path_key`), so case-insensitive on Windows. Query them with
/// `is_excluded_name`.
pub fn excluded_filenames(root: &Path, names: &OutputNames) -> HashSet<String> {
    let folder = folder_stem(root);
    let mut set = HashSet::new();
    let mut add = |file: String| {
        set.insert(path_key(&format!("{file}.partial"), FOLD_CASE));
        set.insert(path_key(&file, FOLD_CASE));
    };
    for stem in ["checksums", names.stem.as_str(), folder.as_str()] {
        for ext in ["md5", "sha256", "json"] {
            add(format!("{stem}.{ext}"));
        }
    }
    // The chosen file exactly as it is named (`FOTOS.MD5`, `SHA256SUMS`). Other SHA files
    // (`.sha1`, `.sha512`, also a chosen one) are not listed here: they only drop out if the
    // checksum file in effect does not name them (`is_sha_filename`), because up to 0.8.0
    // they were hashed along and can appear in a `.md5`.
    for file in [&names.md5, &names.sha256, &names.json] {
        add(file.clone());
    }
    set
}

/// Whether `name` is one of Veritas's own files from `excluded_filenames`.
pub fn is_excluded_name(excluded: &HashSet<String>, name: &str) -> bool {
    excluded.contains(&path_key(name, FOLD_CASE))
}

/// The algorithm named by the name of a checksum file (case-insensitive): `*.md5`; `*.sha1`,
/// `*.sha256`, `*.sha512`, also with `sum` (`*.sha256sum`); the collective files `SHA1SUMS`,
/// `SHA256SUMS`, `SHA512SUMS`, also with `.txt`. Otherwise `None` – a file without such a name
/// never counts as a checksum file.
pub fn algorithm_of_name(name: &str) -> Option<HashMode> {
    if is_md5_filename(name) {
        return Some(HashMode::Md5);
    }
    let lower = name.to_ascii_lowercase();
    let sums = lower.strip_suffix(".txt").unwrap_or(&lower);
    [
        ("sha1", HashMode::Sha1),
        ("sha256", HashMode::Sha256),
        ("sha512", HashMode::Sha512),
    ]
    .into_iter()
    .find(|(tag, _)| {
        sums == format!("{tag}sums")
            || [format!(".{tag}"), format!(".{tag}sum")]
                .iter()
                .any(|ext| lower.len() > ext.len() && lower.ends_with(ext.as_str()))
    })
    .map(|(_, algo)| algo)
}

/// A SHA checksum file by its name (`algorithm_of_name`), i.e. not a `.md5`.
pub fn is_sha_filename(name: &str) -> bool {
    algorithm_of_name(name).is_some_and(|algo| algo != HashMode::Md5)
}

/// Reads at most `limit` bytes. If the file is larger, returns `FileTooLarge` with a message
/// naming the file, its size and the limit.
pub(crate) fn read_limited(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    if size > limit {
        return Err(too_large(path, size, limit));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(too_large(path, bytes.len() as u64, limit));
    }
    Ok(bytes)
}

fn too_large(path: &Path, size: u64, limit: u64) -> io::Error {
    const MIB: u64 = 1024 * 1024;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    io::Error::new(
        io::ErrorKind::FileTooLarge,
        Msg::TooLarge.fill(&[
            &name,
            &size.div_ceil(MIB).to_string(),
            &(limit / MIB).to_string(),
        ]),
    )
}

/// Text of a checksum file from its bytes: UTF-8 (with or without BOM), UTF-16 with BOM,
/// otherwise Windows-1252 (ANSI, as many older Windows tools write it). The second field
/// names the encoding if it was not UTF-8.
pub(crate) fn decode_text(bytes: &[u8]) -> (String, Option<&'static str>) {
    if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return (String::from_utf8_lossy(rest).into_owned(), None);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
        return (decode_utf16(rest, u16::from_le_bytes), Some("UTF-16"));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        return (decode_utf16(rest, u16::from_be_bytes), Some("UTF-16"));
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_string(), None),
        Err(_) => (
            bytes.iter().map(|&byte| cp1252(byte)).collect(),
            Some("Windows-1252"),
        ),
    }
}

fn decode_utf16(bytes: &[u8], word: fn([u8; 2]) -> u16) -> String {
    let units = bytes.as_chunks::<2>().0.iter().map(|pair| word(*pair));
    char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// One byte in Windows-1252. 0x80–0x9F differ from Latin-1; the five unassigned positions
/// stay control characters, as in WHATWG.
fn cp1252(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}',
        '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}',
        '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}',
        '\u{178}',
    ];
    match byte {
        0x80..=0x9f => HIGH[usize::from(byte - 0x80)],
        _ => char::from(byte),
    }
}

/// A checksum file that has been read.
pub(crate) struct ChecksumFile {
    /// Path → hex checksum, as `parse_checksum` returns it.
    pub(crate) entries: BTreeMap<String, String>,
    /// Warnings about lines that could not be read.
    pub(crate) line_warnings: Vec<String>,
    /// The encoding if it was not UTF-8 (`decode_text`).
    pub(crate) encoding: Option<&'static str>,
    /// The file contains more than whitespace.
    pub(crate) nonempty: bool,
}

/// Reads and parses a checksum file, with size limit and encoding detection.
/// The preview and the run both read through this so that they see the same thing.
pub(crate) fn read_checksum_file(path: &Path) -> io::Result<ChecksumFile> {
    let bytes = read_limited(path, MAX_MD5_BYTES)?;
    let (text, encoding) = decode_text(&bytes);
    let (entries, line_warnings) = parse_checksum(&text);
    Ok(ChecksumFile {
        entries,
        line_warnings,
        encoding,
        nonempty: text.chars().any(|ch| !ch.is_whitespace()),
    })
}

/// Which checksum file belongs to this folder.
///
/// `{folder name}.md5` if it exists. Otherwise the only other `*.md5`.
/// If there are several and none is named like the folder, the newest wins.
/// If there is no `.md5`, a SHA file (`algorithm_of_name`) applies: the newest, at equal
/// time SHA-256 before SHA-512 before SHA-1.
/// If there is no file at all, the new one is named `{folder name}.md5`, never
/// `checksums.md5`.
pub fn choose_md5_file(root: &Path) -> String {
    let preferred = preferred_md5_name(root);
    let mut md5 = Vec::new();
    let mut sha = Vec::new();
    if let Ok(read) = std::fs::read_dir(root) {
        for entry in read.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Some(algo) = algorithm_of_name(&name) else {
                continue;
            };
            if !entry.path().is_file() {
                continue;
            }
            let modified = entry.metadata().and_then(|meta| meta.modified()).ok();
            if algo == HashMode::Md5 {
                md5.push((name, modified, 0));
            } else {
                sha.push((name, modified, sha_rank(algo)));
            }
        }
    }
    if md5.iter().any(|(name, _, _)| name == &preferred) {
        return preferred;
    }
    let mut found = if md5.is_empty() { sha } else { md5 };
    found.sort_by(|left, right| {
        newer_first(left.1, right.1)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    found
        .into_iter()
        .next()
        .map_or(preferred, |(name, _, _)| name)
}

/// Precedence of a SHA file at equal modification time: SHA-256, SHA-512, SHA-1.
fn sha_rank(algo: HashMode) -> u8 {
    match algo {
        HashMode::Sha256 => 0,
        HashMode::Sha512 => 1,
        _ => 2,
    }
}

fn newer_first(left: Option<SystemTime>, right: Option<SystemTime>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

pub(crate) fn is_md5_filename(name: &str) -> bool {
    name.len() > 4 && name.to_ascii_lowercase().ends_with(".md5")
}

fn preferred_md5_name(root: &Path) -> String {
    if root.file_name().and_then(|name| name.to_str()).is_none() {
        return "root.md5".to_string();
    }
    format!("{}.md5", folder_stem(root))
}

/// Checksum and JSON names for a concrete MD5 file.
pub fn names_from_md5_filename(filename: &str) -> OutputNames {
    let stem = if is_md5_filename(filename) {
        filename.get(..filename.len() - 4).unwrap_or(filename)
    } else {
        filename
    }
    .to_string();
    OutputNames {
        md5: filename.to_string(),
        ..OutputNames::from_stem(stem)
    }
}

/// The names for the checksum file `filename` in the folder `root`. A `.md5` like
/// `names_from_md5_filename`. A SHA file is foreign: it is only read; the history and a new
/// `.md5` are named like the folder (`{folder name}.json`, `{folder name}.md5`), so that the
/// history continues after "MD5 erzeugen" (create MD5).
pub fn names_for(root: &Path, filename: &str) -> OutputNames {
    match algorithm_of_name(filename) {
        Some(algo) if algo != HashMode::Md5 => {
            let mut names = names_from_md5_filename(&preferred_md5_name(root));
            *names.file_mut(algo) = filename.to_string();
            names
        }
        _ => names_from_md5_filename(filename),
    }
}

fn folder_stem(root: &Path) -> String {
    let raw = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("checksums");
    let cleaned: String = raw
        .chars()
        .map(|ch| {
            if ch == '/' || ch == '\\' || ch == '\0' {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "checksums".to_string()
    } else {
        cleaned.to_string()
    }
}

/// One line in md5sum/sha256sum style: uppercase hex, exactly two spaces, `\n`.
/// When reading, a single space is accepted too.
/// Paths with `\` or a line break are escaped as in GNU coreutils.
pub fn format_line(hash: &str, path: &str) -> String {
    let hash = hash.to_ascii_uppercase();
    if path.contains('\\') || path.contains('\n') {
        let escaped = path.replace('\\', "\\\\").replace('\n', "\\n");
        format!("\\{hash}  {escaped}\n")
    } else {
        format!("{hash}  {path}\n")
    }
}

pub fn format_checksum(entries: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (path, hash) in entries {
        out.push_str(&format_line(hash, path));
    }
    out
}

/// Reads checksum lines. The return value is the path and the hash in uppercase.
/// Defective lines end up in the warning list; valid lines are kept.
pub fn parse_checksum(text: &str) -> (BTreeMap<String, String>, Vec<String>) {
    parse_checksum_for(text, cfg!(windows))
}

/// `parse_checksum` with the path rules of Windows (`windows`) or Linux, so the tests
/// check both on every system.
pub(crate) fn parse_checksum_for(
    text: &str,
    windows: bool,
) -> (BTreeMap<String, String>, Vec<String>) {
    let mut map = BTreeMap::new();
    let mut warnings = Vec::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (index, raw) in text.split('\n').enumerate() {
        let line_no = index + 1;
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        match parse_line(line, windows) {
            Ok((path, hash)) => {
                if map.insert(path.clone(), hash).is_some() {
                    warnings.push(
                        Msg::Line.fill(&[&line_no.to_string(), &Msg::DuplicatePath.fill(&[&path])]),
                    );
                }
            }
            Err(reason) => warnings.push(Msg::Line.fill(&[&line_no.to_string(), &reason])),
        }
    }
    (map, warnings)
}

/// The hex lengths a line may have: MD5, SHA-1, SHA-256, SHA-512. Which of them applies is
/// given by the name of the file; the run rejects a line of another length (`read_previous`).
const HEX_LENGTHS: [usize; 4] = [32, 40, 64, 128];

fn parse_line(line: &str, windows: bool) -> Result<(String, String), String> {
    let (escaped, body) = if let Some(rest) = line.strip_prefix('\\') {
        (true, rest)
    } else {
        (false, line)
    };
    if let Some(tagged) = parse_tagged(body) {
        let (path_raw, hash) = tagged?;
        let path = if escaped {
            unescape(path_raw)
        } else {
            path_raw.replace('\\', "/")
        };
        return Ok((normalize_relative(&path, windows)?, hash));
    }
    let bytes = body.as_bytes();
    let mut hex_len = 0;
    while hex_len < bytes.len() && bytes[hex_len].is_ascii_hexdigit() {
        hex_len += 1;
    }
    if !HEX_LENGTHS.contains(&hex_len) {
        return Err(Msg::HashLengthAny.en().into());
    }
    if hex_len >= bytes.len() || bytes[hex_len] != b' ' {
        return Err(Msg::NoSeparator.en().into());
    }
    let hash = body.get(..hex_len).unwrap_or_default().to_ascii_uppercase();
    let rest = body.get(hex_len + 1..).unwrap_or_default();
    // GNU: two spaces (text) or a space and `*` (binary).
    // Many Windows tools write only one space: `HASH FILE`.
    let path_raw = if let Some(path) = rest.strip_prefix(' ') {
        path
    } else if let Some(path) = rest.strip_prefix('*') {
        path
    } else {
        rest
    };
    if path_raw.is_empty() {
        return Err(Msg::PathMissing.en().into());
    }
    let path = if escaped {
        unescape(path_raw)
    } else {
        path_raw.replace('\\', "/")
    };
    let path = normalize_relative(&path, windows)?;
    Ok((path, hash))
}

/// A line in BSD format (`md5`, `shasum --tag`, `sha256sum --tag` on Linux):
/// `SHA256 (file) = hash`. `None` if the line does not start like that; otherwise the path
/// (raw) and the hash in uppercase, or why the line is invalid. The path reaches up to the
/// last `) = `, so it may itself contain parentheses. The length of the hash must match the
/// name in front.
fn parse_tagged(body: &str) -> Option<Result<(&str, String), String>> {
    let (algo, rest) = [
        ("MD5", HashMode::Md5),
        ("SHA1", HashMode::Sha1),
        ("SHA256", HashMode::Sha256),
        ("SHA512", HashMode::Sha512),
    ]
    .into_iter()
    .find_map(|(tag, algo)| {
        body.strip_prefix(tag)
            .and_then(|rest| rest.strip_prefix(" ("))
            .map(|rest| (algo, rest))
    })?;
    let Some((path, hash)) = rest.rsplit_once(") = ") else {
        return Some(Err(Msg::TagLength.fill(&[algo.label()])));
    };
    let hash = hash.trim_end();
    if Some(hash.len()) != algo.hex_len() || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Some(Err(Msg::TagLength.fill(&[algo.label()])));
    }
    if path.is_empty() {
        return Some(Err(Msg::PathMissing.en().into()));
    }
    Some(Ok((path, hash.to_ascii_uppercase())))
}

/// The single check that a path from a checksum file stays inside the chosen folder.
/// Returns the path with `/`, without `./` and empty segments (`find . -exec md5sum`).
///
/// Rejected are absolute paths (`/x`), on Windows also `\\host\share`, `\\?\…`,
/// drives (`C:`, `C:x`) and any colon (alternate data streams), and `..`.
/// On Windows `\` counts like `/`, and a segment consisting only of dots and spaces
/// (`...`, `.. `) is forbidden because Windows shortens it to `..` or `.`. On Linux
/// `a:b.txt` is an ordinary file name.
pub(crate) fn normalize_relative(path: &str, windows: bool) -> Result<String, String> {
    const OUTSIDE: &str = Msg::PathOutside.en();
    let path = if windows {
        path.replace('\\', "/")
    } else {
        path.to_string()
    };
    if path.contains('\0') {
        return Err(Msg::PathNul.en().into());
    }
    if path.starts_with('/') {
        return Err(Msg::PathRelative.en().into());
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(OUTSIDE.into());
        }
        if windows {
            if part.contains(':') {
                return Err(Msg::PathDrive.en().into());
            }
            if part.trim_end_matches([' ', '.']).is_empty() {
                return Err(OUTSIDE.into());
            }
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(Msg::PathMissing.en().into());
    }
    Ok(parts.join("/"))
}

fn unescape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_of(text: &str, path: &str) -> String {
        text.lines()
            .find(|line| line.contains(path) || unescape(line).contains(path))
            .unwrap()
            .to_string()
    }

    #[test]
    fn md5sum_shape_uppercase_two_spaces_slash_and_specials() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "ordner/meine datei.txt".into(),
            "d41d8cd98f00b204e9800998ecf8427e".into(),
        );
        entries.insert("fotos/übersicht.md".into(), "ab".repeat(16));
        entries.insert("pfad/mit\\slash.txt".into(), "cd".repeat(16));
        entries.insert("zeile/mit\nbruch.txt".into(), "ef".repeat(16));
        let text = format_checksum(&entries);
        assert!(!text.contains('\r'));
        assert!(text.ends_with('\n'));

        let space = line_of(&text, "ordner/meine datei.txt");
        assert!(space.starts_with("D41D8CD98F00B204E9800998ECF8427E  ordner/meine datei.txt"));
        assert_eq!(&space.as_bytes()[32..34], b"  ");
        assert_ne!(space.as_bytes()[34], b' ');
        assert!(!space.contains('\\'));

        let unicode = line_of(&text, "fotos/übersicht.md");
        assert!(unicode.contains("fotos/übersicht.md"));
        assert_eq!(&unicode.as_bytes()[32..34], b"  ");

        let slash = text
            .lines()
            .find(|line| line.starts_with('\\') && line.contains("slash"))
            .unwrap();
        assert!(slash.contains("mit\\\\slash.txt"));

        let newline = text
            .lines()
            .find(|line| line.starts_with('\\') && line.contains("bruch"))
            .unwrap();
        assert!(newline.contains("mit\\nbruch.txt"));

        // A `\` in a name exists only on Linux; Windows reads it as a separator.
        let (parsed, warnings) = parse_checksum_for(&text, false);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(parsed.len(), 4);
        assert_eq!(
            parsed.get("ordner/meine datei.txt").map(String::as_str),
            Some("D41D8CD98F00B204E9800998ECF8427E")
        );
        let ab = "AB".repeat(16);
        let cd = "CD".repeat(16);
        let ef = "EF".repeat(16);
        assert_eq!(
            parsed.get("fotos/übersicht.md").map(String::as_str),
            Some(ab.as_str())
        );
        assert_eq!(
            parsed.get("pfad/mit\\slash.txt").map(String::as_str),
            Some(cd.as_str())
        );
        assert_eq!(
            parsed.get("zeile/mit\nbruch.txt").map(String::as_str),
            Some(ef.as_str())
        );
    }

    #[test]
    fn parses_example_checksum_file() {
        // Made-up names; the hash is the MD5 of the file name. The main part uses the format of
        // many Windows tools (one space); at the end come subfolders, umlauts, backslashes,
        // lowercase letters, the binary marker and two spaces.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/example.md5");
        let text = std::fs::read_to_string(&path).unwrap();
        let (map, warnings) = parse_checksum(&text);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(map.len(), 96);
        let hash = |name: &str| map.get(name).map(String::as_str);
        assert_eq!(
            hash("DJI_0001.JPG"),
            Some("12AC279377BCBDEC683E79B5485B5AEF")
        );
        assert_eq!(
            hash("Urlaub/Strand/DSC_0042.JPG"),
            Some("DE01C397FD17863EE92E23C2B115DF5A")
        );
        assert_eq!(
            hash("Fotos/Übersicht Süd.jpeg"),
            Some("BEF09C473B58D2E737625A129F631B29")
        );
        // Backslashes in the path become `/`.
        assert_eq!(
            hash("Urlaub/Strand/DSC_0044.JPG"),
            Some("822B5271889A18FDF65A524FD21798B4")
        );
        // Lowercase letters in the hash, binary marker ` *` and two spaces.
        assert_eq!(
            hash("Urlaub/IMG_1236.HEIC"),
            Some("5853114C778E5F2EB6DA76F7CFD9E5A3")
        );
        assert_eq!(
            hash("Urlaub/Strand/DSC_0045.JPG"),
            Some("75A24E277AFE6145BAC291142FED48D3")
        );
        assert_eq!(
            hash("Urlaub/IMG_1237.HEIC"),
            Some("367DC48DA7F01115470A204049162FD9")
        );
    }

    #[test]
    fn parse_accepts_lowercase_and_binary_marker() {
        let text = "\
900150983cd24fb0d6963f7d28e17f72  ordner/datei.txt
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa *bin/datei.bin
";
        let (map, warnings) = parse_checksum(text);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            map.get("ordner/datei.txt").map(String::as_str),
            Some("900150983CD24FB0D6963F7D28E17F72")
        );
        assert_eq!(
            map.get("bin/datei.bin").map(String::as_str),
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
        );
    }

    #[test]
    fn rejects_parent_segments_and_keeps_other_lines() {
        let text = "\
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA  ok/datei.txt
BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB  ../geheim.txt
keine-hash-zeile
";
        let (map, warnings) = parse_checksum(text);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("ok/datei.txt"));
        assert!(warnings.len() >= 2);
    }

    const H: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    /// Escaped lines (leading `\`) must not lead out of the folder on Windows.
    #[test]
    fn escaped_windows_paths_cannot_leave_the_folder() {
        for path in [
            "..\\\\..\\\\geheim.txt",
            "\\\\x.txt",
            "\\\\\\\\host\\\\share\\\\x.txt",
            "\\\\\\\\?\\\\C:\\\\x.txt",
            "C:\\\\x.txt",
            "C:x.txt",
            "a\\\\..\\\\..\\\\x.txt",
            "a.txt:strom",
            "...\\\\x.txt",
            ".. \\\\x.txt",
        ] {
            let text = format!("\\{H}  {path}\n");
            let (map, warnings) = parse_checksum_for(&text, true);
            assert!(map.is_empty(), "{path} → {map:?}");
            assert_eq!(warnings.len(), 1, "{path}");
        }
        // The same, unescaped.
        for path in ["..\\x.txt", "\\\\host\\share\\x.txt", "C:\\x.txt", "/x.txt"] {
            let (map, _) = parse_checksum_for(&format!("{H}  {path}\n"), true);
            assert!(map.is_empty(), "{path} → {map:?}");
        }
        // An escaped subfolder remains allowed.
        let (map, warnings) = parse_checksum_for(&format!("\\{H}  a\\\\b.txt\n"), true);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(map.contains_key("a/b.txt"));
    }

    /// `./` and duplicate separators (`find . -exec md5sum`) are ignored.
    #[test]
    fn dot_slash_and_empty_segments_are_removed() {
        for windows in [false, true] {
            let text = format!("{H}  ./a/./b.txt\n{H}  ./c.txt\n{H}  d//e.txt\n");
            let (map, warnings) = parse_checksum_for(&text, windows);
            assert!(warnings.is_empty(), "{warnings:?}");
            let keys: Vec<&str> = map.keys().map(String::as_str).collect();
            assert_eq!(keys, ["a/b.txt", "c.txt", "d/e.txt"]);
            let (map, warnings) = parse_checksum_for(&format!("{H}  ./\n"), windows);
            assert!(map.is_empty() && warnings.len() == 1);
        }
    }

    /// On Linux `a:b.txt` is a file name; Veritas writes it and reads it back.
    #[test]
    fn colon_is_a_plain_character_on_linux_only() {
        let text = format!("{H}  a:b.txt\n");
        let (map, warnings) = parse_checksum_for(&text, false);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(map.contains_key("a:b.txt"));
        let (map, _) = parse_checksum_for(&text, true);
        assert!(map.is_empty());
        // An absolute path stays forbidden on Linux too.
        let (map, _) = parse_checksum_for(&format!("\\{H}  /etc/x\n"), false);
        assert!(map.is_empty());
    }

    /// UTF-16 with BOM and ANSI (Windows-1252) are read; UTF-8 stays UTF-8.
    #[test]
    fn decodes_utf16_and_windows_1252() {
        let line = format!("{H}  Übersicht – Köln.jpg\r\n");
        let mut le = vec![0xff, 0xfe];
        le.extend(line.encode_utf16().flat_map(u16::to_le_bytes));
        let mut be = vec![0xfe, 0xff];
        be.extend(line.encode_utf16().flat_map(u16::to_be_bytes));
        for (bytes, label) in [(le, Some("UTF-16")), (be, Some("UTF-16"))] {
            let (text, used) = decode_text(&bytes);
            assert_eq!(used, label);
            assert_eq!(text, line);
        }
        let mut ansi = H.as_bytes().to_vec();
        ansi.extend(b"  \xdcbersicht \x96 K\xf6ln \x80.jpg\n");
        let (text, used) = decode_text(&ansi);
        assert_eq!(used, Some("Windows-1252"));
        assert_eq!(text, format!("{H}  Übersicht – Köln €.jpg\n"));
        let mut bom = vec![0xef, 0xbb, 0xbf];
        bom.extend(line.as_bytes());
        assert_eq!(decode_text(&bom), (line.clone(), None));
        assert_eq!(decode_text(line.as_bytes()), (line, None));
    }

    /// A file that is too large is not read; the message names the file and the limit.
    #[test]
    fn too_large_files_are_refused_with_a_clear_message() {
        let dir = scratch("gross");
        let path = dir.join("riesig.md5");
        std::fs::write(&path, vec![b'x'; 2048]).unwrap();
        let err = read_limited(&path, 1024).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::FileTooLarge);
        let text = err.to_string();
        assert!(
            text.contains("riesig.md5") && text.contains("too large"),
            "{text}"
        );
        assert_eq!(read_limited(&path, 4096).unwrap().len(), 2048);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Veritas's own file in another spelling (`FOTOS.MD5`) is not hashed.
    #[test]
    fn own_files_are_excluded_in_any_spelling() {
        let names = names_from_md5_filename("FOTOS.MD5");
        let set = excluded_filenames(Path::new("/x/Fotos"), &names);
        assert!(is_excluded_name(&set, "FOTOS.MD5"));
        assert!(is_excluded_name(&set, "FOTOS.MD5.partial"));
        assert!(is_excluded_name(&set, "FOTOS.json"));
        assert!(is_excluded_name(&set, "Fotos.md5"));
        assert!(!is_excluded_name(&set, "bild.jpg"));
        if cfg!(windows) {
            assert!(is_excluded_name(&set, "fotos.MD5"));
            assert!(is_excluded_name(&set, "FOTOS.JSON"));
        }
    }

    #[test]
    fn folder_name_style_uses_directory() {
        let names = output_names(Path::new("/medien/Urlaub 2024"), NameStyle::FolderName);
        assert_eq!(names.md5, "Urlaub 2024.md5");
        assert_eq!(names.sha256, "Urlaub 2024.sha256");
        assert_eq!(names.json, "Urlaub 2024.json");
        let standard = output_names(Path::new("/medien/Urlaub 2024"), NameStyle::Checksums);
        assert_eq!(standard.md5, "checksums.md5");
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("veritas-md5-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn missing_md5_uses_the_folder_name() {
        let dir = scratch("leer");
        assert_eq!(
            choose_md5_file(&dir),
            format!("{}.md5", dir.file_name().unwrap().to_str().unwrap())
        );
        assert_ne!(choose_md5_file(&dir), "checksums.md5");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn folder_name_wins_over_a_newer_file() {
        let dir = scratch("ordner");
        let preferred = dir.join(format!(
            "{}.md5",
            dir.file_name().unwrap().to_str().unwrap()
        ));
        std::fs::write(&preferred, b"alt\n").unwrap();
        let other = dir.join("andere.md5");
        std::fs::write(&other, b"neu\n").unwrap();
        let later = std::fs::metadata(&preferred).unwrap().modified().unwrap()
            + std::time::Duration::from_secs(5);
        let _ = filetime_set(&other, later);
        assert_eq!(
            choose_md5_file(&dir),
            preferred.file_name().unwrap().to_str().unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_other_md5_is_used() {
        let dir = scratch("eins");
        std::fs::write(dir.join("notiz.md5"), b"x\n").unwrap();
        assert_eq!(choose_md5_file(&dir), "notiz.md5");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn several_without_folder_name_use_the_newest() {
        let dir = scratch("viele");
        let older = dir.join("alt.md5");
        let newer = dir.join("neu.md5");
        std::fs::write(&older, b"a\n").unwrap();
        std::fs::write(&newer, b"b\n").unwrap();
        let stamp = std::fs::metadata(&older).unwrap().modified().unwrap()
            + std::time::Duration::from_secs(30);
        let _ = filetime_set(&newer, stamp);
        assert_eq!(choose_md5_file(&dir), "neu.md5");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn filetime_set(path: &std::path::Path, time: std::time::SystemTime) -> std::io::Result<()> {
        let file = std::fs::File::options().write(true).open(path)?;
        file.set_modified(time)
    }

    /// The name tells the algorithm, case-insensitively; a name without an algorithm is not a
    /// checksum file.
    #[test]
    fn the_name_tells_the_algorithm() {
        for (name, algo) in [
            ("Fotos.md5", Some(HashMode::Md5)),
            ("FOTOS.MD5", Some(HashMode::Md5)),
            ("a.sha1", Some(HashMode::Sha1)),
            ("a.SHA1SUM", Some(HashMode::Sha1)),
            ("SHA1SUMS", Some(HashMode::Sha1)),
            ("debian.sha256", Some(HashMode::Sha256)),
            ("debian.sha256sum", Some(HashMode::Sha256)),
            ("SHA256SUMS", Some(HashMode::Sha256)),
            ("sha256sums.txt", Some(HashMode::Sha256)),
            ("Sha256Sums.TXT", Some(HashMode::Sha256)),
            ("x.sha512", Some(HashMode::Sha512)),
            ("x.sha512sum", Some(HashMode::Sha512)),
            ("SHA512SUMS.txt", Some(HashMode::Sha512)),
            (".md5", None),
            (".sha256", None),
            ("checksums.txt", None),
            ("SHA256SUMS.asc", None),
            ("MD5SUMS", None),
            ("bild.jpg", None),
            ("x.sha384", None),
            ("x.sha2560", None),
        ] {
            assert_eq!(algorithm_of_name(name), algo, "{name}");
            assert_eq!(
                is_sha_filename(name),
                algo.is_some_and(|algo| algo != HashMode::Md5),
                "{name}"
            );
        }
    }

    const SHA1: &str = "A9993E364706816ABA3E25717850C26C9CD0D89D";
    const SHA256: &str = "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD";

    /// GNU lines come with 32, 40, 64 and 128 hex characters, with two spaces, ` *` or one
    /// space; any other length is an error of the line.
    #[test]
    fn gnu_lines_take_every_sha_length() {
        let sha512 = "AB".repeat(64);
        let text = format!(
            "{}  a.txt\n{} *b.bin\n{} c d.txt\n{}  e.txt\n{}  zu-kurz.txt\n",
            SHA1.to_ascii_lowercase(),
            SHA256,
            sha512,
            H,
            "AB".repeat(24),
        );
        let (map, warnings) = parse_checksum_for(&text, false);
        assert_eq!(map.get("a.txt").map(String::as_str), Some(SHA1));
        assert_eq!(map.get("b.bin").map(String::as_str), Some(SHA256));
        assert_eq!(map.get("c d.txt"), Some(&sha512));
        assert_eq!(map.get("e.txt").map(String::as_str), Some(H));
        assert_eq!(map.len(), 4);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains(Msg::HashLengthAny.en()),
            "{warnings:?}"
        );
    }

    /// BSD format: `SHA256 (file) = hash`, also `MD5 (…)` and escaped. The path reaches up to
    /// the last `) = `; length and name in front must match; the path rules apply as usual.
    #[test]
    fn bsd_tagged_lines_are_read() {
        let sha512 = "cd".repeat(64);
        let text = format!(
            "SHA256 (Fotos/Bild (1).jpg) = {}\n\
             SHA1 (a.txt) = {SHA1}\n\
             MD5 (b.txt) = {H}\n\
             SHA512 (c.txt) = {sha512}\r\n\
             \\SHA256 (mit\\\\slash.txt) = {SHA256}\n\
             SHA256 (Fotos\\x.jpg) = {SHA256}\n\
             SHA256 (falsch.txt) = {SHA1}\n\
             SHA1 (ohne-gleich.txt) {SHA1}\n\
             SHA256 () = {SHA256}\n\
             SHA256 (../geheim.txt) = {SHA256}\n",
            SHA256.to_ascii_lowercase(),
        );
        let (map, warnings) = parse_checksum_for(&text, false);
        let hash = |name: &str| map.get(name).map(String::as_str);
        assert_eq!(hash("Fotos/Bild (1).jpg"), Some(SHA256));
        assert_eq!(hash("a.txt"), Some(SHA1));
        assert_eq!(hash("b.txt"), Some(H));
        assert_eq!(hash("c.txt"), Some(sha512.to_ascii_uppercase().as_str()));
        assert_eq!(hash("mit\\slash.txt"), Some(SHA256));
        // Unescaped, `\` is a separator as in GNU lines.
        assert_eq!(hash("Fotos/x.jpg"), Some(SHA256));
        assert_eq!(map.len(), 6, "{map:?}");
        assert_eq!(warnings.len(), 4, "{warnings:?}");
        assert!(warnings[0].contains(&Msg::TagLength.fill(&["SHA-256"])));
        assert!(warnings[1].contains(&Msg::TagLength.fill(&["SHA-1"])));
        assert!(warnings[2].contains(Msg::PathMissing.en()));
        assert!(warnings[3].contains(Msg::PathOutside.en()));
    }

    /// Without a `.md5`, a SHA file applies: the newest, at equal time SHA-256 before SHA-512
    /// before SHA-1. Any `.md5` takes precedence, even an older one.
    #[test]
    fn sha_files_count_only_without_an_md5() {
        let dir = scratch("sha-wahl");
        let stamp = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1 << 30);
        for name in ["SHA1SUMS", "x.sha512", "sha256sums.txt", "bild.jpg"] {
            let path = dir.join(name);
            std::fs::write(&path, b"x\n").unwrap();
            filetime_set(&path, stamp).unwrap();
        }
        assert_eq!(choose_md5_file(&dir), "sha256sums.txt");
        // A newer SHA-1 file takes precedence.
        filetime_set(
            &dir.join("SHA1SUMS"),
            stamp + std::time::Duration::from_secs(60),
        )
        .unwrap();
        assert_eq!(choose_md5_file(&dir), "SHA1SUMS");
        // Any `.md5` takes precedence over any SHA file, even an older one.
        let md5 = dir.join("alt.md5");
        std::fs::write(&md5, b"x\n").unwrap();
        filetime_set(&md5, stamp - std::time::Duration::from_secs(60)).unwrap();
        assert_eq!(choose_md5_file(&dir), "alt.md5");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// For a SHA file, the history and a new `.md5` are named like the folder; the SHA file
    /// itself is the file of its algorithm, and it counts among Veritas' own files.
    #[test]
    fn names_for_a_sha_file_follow_the_folder() {
        let root = Path::new("/medien/Urlaub 2024");
        let names = names_for(root, "SHA256SUMS");
        assert_eq!(names.md5, "Urlaub 2024.md5");
        assert_eq!(names.json, "Urlaub 2024.json");
        assert_eq!(names.file(HashMode::Sha256), "SHA256SUMS");
        assert_eq!(names.file(HashMode::Md5), "Urlaub 2024.md5");
        let names = names_for(root, "andere.sha1");
        assert_eq!(names.file(HashMode::Sha1), "andere.sha1");
        assert_eq!(names.json, "Urlaub 2024.json");
        let set = excluded_filenames(root, &names);
        assert!(is_excluded_name(&set, "Urlaub 2024.md5"));
        assert!(is_excluded_name(&set, "Urlaub 2024.json"));
        // For SHA-1/512 files, the list of the file in effect decides (`drop_foreign_checksums`).
        assert!(!is_excluded_name(&set, "andere.sha1"));
        assert!(!is_excluded_name(&set, "Urlaub 2024.sha1"));
        // A `.md5` as before.
        assert_eq!(
            names_for(root, "alt.md5"),
            names_from_md5_filename("alt.md5")
        );
        assert_eq!(names_for(root, "alt.md5").json, "alt.json");
    }
}

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::i18n::Msg;

/// Result of the structure check. Relevant only on a hash mismatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityStatus {
    // English is written (since the language choice); older histories contain the German values.
    #[serde(rename = "intact", alias = "intakt")]
    Intact,
    #[serde(rename = "possibly damaged", alias = "möglicherweise beschädigt")]
    PossiblyDamaged,
    #[serde(rename = "not checked", alias = "nicht geprüft")]
    NotChecked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityFinding {
    pub status: IntegrityStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Jpeg,
    Png,
    Webp,
    Heif,
    Tiff,
    Mp4,
    Mkv,
    Avi,
    Ooxml,
    Pdf,
}

pub fn check_path(path: &Path) -> IntegrityFinding {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(err) => return unreadable(err),
    };
    let len = match file.metadata() {
        Ok(meta) => meta.len(),
        Err(err) => return unreadable(err),
    };
    check_reader(&mut file, len, path)
}

/// Like `check_path`, but for bytes in memory; only the tests need it.
#[cfg(test)]
pub fn check_bytes(hint: &Path, data: &[u8]) -> IntegrityFinding {
    check_reader(&mut io::Cursor::new(data), data.len() as u64, hint)
}

fn unreadable(err: io::Error) -> IntegrityFinding {
    let reason = if err.kind() == io::ErrorKind::PermissionDenied {
        Msg::ReadDenied.en().to_string()
    } else {
        Msg::ReadFailed.fill(&[&err.to_string()])
    };
    finding(IntegrityStatus::PossiblyDamaged, reason)
}

pub fn check_reader<R: Read + Seek>(reader: &mut R, len: u64, hint: &Path) -> IntegrityFinding {
    if len == 0 {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::Empty.en());
    }
    let head_len = usize::try_from(len.min(64)).unwrap_or(64);
    let mut head = vec![0u8; head_len];
    if read_exact_at(reader, 0, &mut head).is_err() {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::HeadUnreadableAny.en(),
        );
    }
    let kind = match kind_from_extension(hint).or_else(|| sniff(&head)) {
        Some(kind) => kind,
        None => return finding(IntegrityStatus::NotChecked, Msg::UnknownFormat.en()),
    };
    match kind {
        Kind::Jpeg => check_jpeg(reader, len),
        Kind::Png => check_png(reader, len),
        Kind::Webp => check_riff(reader, len, b"WEBP", "WebP"),
        Kind::Avi => check_riff(reader, len, b"AVI ", "AVI"),
        Kind::Heif => check_heif(reader, len),
        Kind::Tiff => check_tiff(&head, len),
        Kind::Mp4 => check_mp4(reader, len),
        Kind::Mkv => check_mkv(&head, len),
        Kind::Ooxml => check_ooxml(reader, len),
        Kind::Pdf => check_pdf(reader, len),
    }
}

fn finding(status: IntegrityStatus, reason: impl Into<String>) -> IntegrityFinding {
    IntegrityFinding {
        status,
        reason: reason.into(),
    }
}

fn kind_from_extension(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" => Kind::Jpeg,
        "png" => Kind::Png,
        "webp" => Kind::Webp,
        "heic" | "heif" | "hif" => Kind::Heif,
        "tif" | "tiff" => Kind::Tiff,
        "mp4" | "m4v" | "mov" => Kind::Mp4,
        "mkv" | "mk3d" => Kind::Mkv,
        "avi" => Kind::Avi,
        "docx" | "xlsx" | "pptx" | "docm" | "xlsm" | "pptm" => Kind::Ooxml,
        "pdf" => Kind::Pdf,
        _ => return None,
    })
}

fn sniff(head: &[u8]) -> Option<Kind> {
    if head.len() >= 2 && head[0] == 0xff && head[1] == 0xd8 {
        return Some(Kind::Jpeg);
    }
    if head.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(Kind::Png);
    }
    if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(Kind::Webp);
    }
    if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"AVI " {
        return Some(Kind::Avi);
    }
    if head.starts_with(b"%PDF-") {
        return Some(Kind::Pdf);
    }
    if head.len() >= 4 && head[..4] == [0x1a, 0x45, 0xdf, 0xa3] {
        return Some(Kind::Mkv);
    }
    if head.len() >= 4
        && (head[..4] == *b"II*\0"
            || head[..4] == *b"MM\0*"
            || head[..4] == *b"II+\0"
            || head[..4] == *b"MM\0+")
    {
        return Some(Kind::Tiff);
    }
    if head.len() >= 12 && &head[4..8] == b"ftyp" {
        if brands(head).any(is_heif_brand) {
            return Some(Kind::Heif);
        }
        if brands(head).any(is_avif_brand) {
            return None;
        }
        return Some(Kind::Mp4);
    }
    None
}

fn brands(head: &[u8]) -> impl Iterator<Item = &[u8]> {
    let end = ftyp_limit(head);
    (8..end)
        .step_by(4)
        .filter(move |offset| offset + 4 <= end && *offset != 12)
        .filter_map(|offset| head.get(offset..offset + 4))
}

fn ftyp_limit(head: &[u8]) -> usize {
    if head.len() < 8 {
        return head.len();
    }
    let size = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
    if size < 16 {
        return head.len().min(32);
    }
    size.min(head.len())
}

fn is_heif_brand(brand: &[u8]) -> bool {
    matches!(
        brand,
        b"heic"
            | b"heix"
            | b"hevc"
            | b"hevx"
            | b"heim"
            | b"heis"
            | b"heif"
            | b"hevm"
            | b"hevs"
            | b"mif1"
            | b"msf1"
            | b"mif2"
            | b"msf2"
            | b"MiHB"
            | b"MiHE"
            | b"miaf"
    )
}

fn is_avif_brand(brand: &[u8]) -> bool {
    matches!(brand, b"avif" | b"avis")
}

/// Range at the end of the file in which the end of image (EOI) must lie. Some cameras
/// (DJI) append padding data after the end of the image, up to a few KiB.
const JPEG_TAIL: usize = 64 * 1024;

fn check_jpeg<R: Read + Seek>(reader: &mut R, len: u64) -> IntegrityFinding {
    if len < 4 {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TooShort.fill(&["JPEG"]),
        );
    }
    let mut soi = [0u8; 2];
    if read_exact_at(reader, 0, &mut soi).is_err() {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::HeadUnreadable.fill(&["JPEG"]),
        );
    }
    if soi != [0xff, 0xd8] {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::JpegNoSoi.en());
    }
    // An EOI before the image data belongs to a thumbnail in the header (EXIF), not to the image.
    let scan = match jpeg_scan_start(reader, len) {
        Ok(scan) => scan,
        Err(reason) => return finding(IntegrityStatus::PossiblyDamaged, reason),
    };
    let tail = match read_suffix(reader, len, JPEG_TAIL) {
        Ok(tail) => tail,
        Err(_) => {
            return finding(
                IntegrityStatus::PossiblyDamaged,
                Msg::TailUnreadable.fill(&["JPEG"]),
            )
        }
    };
    let tail_start = len - tail.len() as u64;
    let eoi = tail
        .windows(2)
        .rposition(|pair| pair == [0xff, 0xd9])
        .map(|index| tail_start + index as u64)
        .filter(|&pos| pos >= scan);
    match eoi {
        Some(pos) if pos + 2 == len => finding(IntegrityStatus::Intact, Msg::JpegOk.en()),
        Some(pos) => finding(
            IntegrityStatus::Intact,
            Msg::JpegOkTrailing.fill(&[&(len - pos - 2).to_string()]),
        ),
        None => finding(IntegrityStatus::PossiblyDamaged, Msg::JpegNoEoi.en()),
    }
}

/// Walks the marker segments from SOI and returns the start of the image data (SOS). If the
/// layout does not fit the scheme, the whole file after SOI counts; if it ends before SOS, it is
/// truncated.
fn jpeg_scan_start<R: Read + Seek>(reader: &mut R, len: u64) -> Result<u64, String> {
    const CUT: &str = Msg::JpegCut.en();
    let mut pos = 2u64;
    // Real files have a few dozen segments; the limit caps the work on unrelated data.
    for _ in 0..4096 {
        if pos + 2 > len {
            return Err(CUT.into());
        }
        let mut marker = [0u8; 2];
        read_exact_at(reader, pos, &mut marker).map_err(|_| Msg::HeadUnreadable.fill(&["JPEG"]))?;
        match marker {
            [0xff, 0xda] => return Ok(pos),
            // Fill byte before a marker.
            [0xff, 0xff] => pos += 1,
            // Marker without a length.
            [0xff, 0x01 | 0xd0..=0xd7] => pos += 2,
            [0xff, 0xd8 | 0xd9] | [0xff, 0x00] => return Ok(2),
            [0xff, _] => {
                if pos + 4 > len {
                    return Err(CUT.into());
                }
                let mut size = [0u8; 2];
                read_exact_at(reader, pos + 2, &mut size)
                    .map_err(|_| Msg::HeadUnreadable.fill(&["JPEG"]))?;
                let size = u64::from(u16::from_be_bytes(size));
                if size < 2 {
                    return Ok(2);
                }
                pos += 2 + size;
            }
            _ => return Ok(2),
        }
    }
    Ok(2)
}

fn check_png<R: Read + Seek>(reader: &mut R, len: u64) -> IntegrityFinding {
    const SIG: &[u8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    if len < SIG.len() as u64 + 12 {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TooShort.fill(&["PNG"]),
        );
    }
    let mut sig = [0u8; 8];
    if read_exact_at(reader, 0, &mut sig).is_err() {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::HeadUnreadable.fill(&["PNG"]),
        );
    }
    if sig != SIG {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::PngNoSignature.en());
    }
    // From chunk to chunk up to IEND (by seek; the image data is not read). Data
    // after IEND is no damage, as with JPEG; a chunk past the end of the file is.
    let mut pos = 8u64;
    for _ in 0..PNG_MAX_CHUNKS {
        if pos.saturating_add(12) > len {
            return finding(IntegrityStatus::PossiblyDamaged, Msg::PngNoIend.en());
        }
        let mut header = [0u8; 8];
        if read_exact_at(reader, pos, &mut header).is_err() {
            return finding(
                IntegrityStatus::PossiblyDamaged,
                Msg::PngChunkUnreadable.en(),
            );
        }
        let size = u64::from(u32::from_be_bytes([
            header[0], header[1], header[2], header[3],
        ]));
        let end = pos.saturating_add(12).saturating_add(size);
        if end > len {
            return finding(IntegrityStatus::PossiblyDamaged, Msg::PngChunkPastEnd.en());
        }
        if &header[4..8] == b"IEND" {
            return if end == len {
                finding(IntegrityStatus::Intact, Msg::PngOk.en())
            } else {
                finding(
                    IntegrityStatus::Intact,
                    Msg::PngOkTrailing.fill(&[&(len - end).to_string()]),
                )
            };
        }
        pos = end;
    }
    finding(IntegrityStatus::NotChecked, Msg::PngManyChunks.en())
}

/// At most this many PNG chunks; after that the file counts as not checked.
const PNG_MAX_CHUNKS: u32 = 1_000_000;

fn check_riff<R: Read + Seek>(
    reader: &mut R,
    len: u64,
    form: &[u8; 4],
    label: &str,
) -> IntegrityFinding {
    if len < 12 {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TooShort.fill(&[label]),
        );
    }
    let mut head = [0u8; 12];
    if read_exact_at(reader, 0, &mut head).is_err() {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::HeadUnreadable.fill(&[label]),
        );
    }
    if &head[0..4] != b"RIFF" || &head[8..12] != form {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::RiffNoSignature.fill(&[label]),
        );
    }
    let declared = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as u64;
    if declared.saturating_add(8) > len {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::RiffTooLarge.fill(&[label]),
        );
    }
    finding(IntegrityStatus::Intact, Msg::RiffOk.fill(&[label]))
}

fn check_heif<R: Read + Seek>(reader: &mut R, len: u64) -> IntegrityFinding {
    if len < 16 {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TooShort.fill(&["HEIF/HEIC"]),
        );
    }
    let take = usize::try_from(len.min(4096)).unwrap_or(4096);
    let mut head = vec![0u8; take];
    if read_exact_at(reader, 0, &mut head).is_err() {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::HeadUnreadable.fill(&["HEIF/HEIC"]),
        );
    }
    if head.len() < 12 || &head[4..8] != b"ftyp" {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::HeifNoFtyp.en());
    }
    let size = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as u64;
    if size < 16 || size > len {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::HeifFtypShort.en());
    }
    if brands(&head).any(is_heif_brand) {
        let major = String::from_utf8_lossy(&head[8..12]);
        finding(IntegrityStatus::Intact, Msg::HeifBrand.fill(&[&major]))
    } else {
        finding(IntegrityStatus::PossiblyDamaged, Msg::HeifNoBrand.en())
    }
}

fn check_tiff(head: &[u8], len: u64) -> IntegrityFinding {
    if head.len() < 8 || len < 8 {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TiffShort.fill(&["TIFF"]),
        );
    }
    if &head[..4] == b"II+\0" || &head[..4] == b"MM\0+" {
        return check_bigtiff(head, len);
    }
    let little = &head[..4] == b"II*\0";
    let big = &head[..4] == b"MM\0*";
    if !little && !big {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::TiffNoSignature.en());
    }
    let offset = if little {
        u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as u64
    } else {
        u32::from_be_bytes([head[4], head[5], head[6], head[7]]) as u64
    };
    if offset < 8 || offset.saturating_add(2) > len {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TiffIfdOutside.fill(&["TIFF"]),
        );
    }
    if little {
        finding(IntegrityStatus::Intact, Msg::TiffLittle.en())
    } else {
        finding(IntegrityStatus::Intact, Msg::TiffBig.en())
    }
}

/// BigTIFF (version 43): offset size 8 and a 64-bit offset to the first IFD.
fn check_bigtiff(head: &[u8], len: u64) -> IntegrityFinding {
    if head.len() < 16 || len < 16 {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TiffShort.fill(&["BigTIFF"]),
        );
    }
    let little = head[0] == b'I';
    let word = |at: usize| {
        let pair = [head[at], head[at + 1]];
        if little {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        }
    };
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&head[8..16]);
    let offset = if little {
        u64::from_le_bytes(raw)
    } else {
        u64::from_be_bytes(raw)
    };
    if word(4) != 8 || word(6) != 0 {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::BigTiffBad.en());
    }
    if offset < 16 || offset.saturating_add(8) > len {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TiffIfdOutside.fill(&["BigTIFF"]),
        );
    }
    finding(IntegrityStatus::Intact, Msg::BigTiffOk.en())
}

/// Top-level atoms are skipped by seek; the media-data body is not loaded.
/// It continues to the end of the file even after `moov`: if `moov` is at the front, only a
/// truncated `mdat` shows that the file was cut short.
fn check_mp4<R: Read + Seek>(reader: &mut R, len: u64) -> IntegrityFinding {
    const MAX_ATOMS: u32 = 10_000;
    let mut pos = 0u64;
    let mut saw_ftyp = false;
    let mut saw_moov = false;
    let mut atoms = 0u32;
    while pos.saturating_add(8) <= len && atoms < MAX_ATOMS {
        atoms += 1;
        let atom = match read_atom(reader, pos, len) {
            Ok(Some(atom)) => atom,
            Ok(None) => break,
            // After `moov`: an atom with a readable name is truncated (usually `mdat`);
            // bytes without an atom name are appended data, as with JPEG.
            Err(AtomError::Truncated(kind)) if saw_moov => {
                return if is_atom_name(&kind) {
                    finding(
                        IntegrityStatus::PossiblyDamaged,
                        Msg::Mp4AtomCut.fill(&[&String::from_utf8_lossy(&kind)]),
                    )
                } else {
                    finding(
                        IntegrityStatus::Intact,
                        Msg::Mp4Trailing.fill(&[&(len - pos).to_string()]),
                    )
                };
            }
            Err(AtomError::Truncated(_)) => {
                return if saw_ftyp && !saw_moov {
                    finding(IntegrityStatus::PossiblyDamaged, Msg::Mp4CutBeforeMoov.en())
                } else {
                    finding(IntegrityStatus::PossiblyDamaged, Msg::Mp4AtomTooLarge.en())
                };
            }
            Err(AtomError::Unreadable) => {
                return finding(
                    IntegrityStatus::PossiblyDamaged,
                    Msg::Mp4HeaderUnreadable.en(),
                );
            }
        };
        if &atom.kind == b"ftyp" {
            saw_ftyp = true;
        }
        if &atom.kind == b"moov" {
            saw_moov = true;
        }
        pos = pos.saturating_add(atom.size);
    }
    let complete = pos.saturating_add(8) > len;
    match (saw_ftyp, saw_moov) {
        (_, true) if !complete => finding(IntegrityStatus::NotChecked, Msg::Mp4ManyAtoms.en()),
        (true, true) => finding(IntegrityStatus::Intact, Msg::Mp4Ok.en()),
        // Older QuickTime files begin directly with moov, mdat or wide, without ftyp.
        (false, true) => finding(IntegrityStatus::Intact, Msg::Mp4QuickTime.en()),
        (false, false) => finding(IntegrityStatus::PossiblyDamaged, Msg::Mp4NoFtyp.en()),
        (true, false) => finding(IntegrityStatus::PossiblyDamaged, Msg::Mp4NoMoov.en()),
    }
}

/// Whether four bytes look like an atom name (`mdat`, `moov`, `©xyz`): printable ASCII
/// or QuickTime's copyright sign.
fn is_atom_name(kind: &[u8; 4]) -> bool {
    kind.iter()
        .all(|&byte| byte == 0xa9 || (0x20..0x7f).contains(&byte))
}

struct Atom {
    kind: [u8; 4],
    size: u64,
}

enum AtomError {
    /// The atom extends past the end of the file; the name is known.
    Truncated([u8; 4]),
    Unreadable,
}

fn read_atom<R: Read + Seek>(
    reader: &mut R,
    pos: u64,
    len: u64,
) -> Result<Option<Atom>, AtomError> {
    if pos.saturating_add(8) > len {
        return Ok(None);
    }
    let mut header = [0u8; 8];
    read_exact_at(reader, pos, &mut header).map_err(|_| AtomError::Unreadable)?;
    let size32 = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let kind = [header[4], header[5], header[6], header[7]];
    let (header_len, size) = if size32 == 1 {
        if pos.saturating_add(16) > len {
            return Err(AtomError::Truncated(kind));
        }
        let mut ext = [0u8; 8];
        read_exact_at(reader, pos + 8, &mut ext).map_err(|_| AtomError::Unreadable)?;
        (16u64, u64::from_be_bytes(ext))
    } else if size32 == 0 {
        (8u64, len - pos)
    } else {
        (8u64, u64::from(size32))
    };
    if size < header_len || pos.saturating_add(size) > len {
        return Err(AtomError::Truncated(kind));
    }
    Ok(Some(Atom { kind, size }))
}

fn check_mkv(head: &[u8], len: u64) -> IntegrityFinding {
    if head.len() < 4 || head[..4] != [0x1a, 0x45, 0xdf, 0xa3] {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::MkvNoHeader.en());
    }
    match ebml_size(head.get(4..).unwrap_or_default()) {
        Some((size, width)) => {
            let header_end = 4u64.saturating_add(width as u64);
            if size != u64::MAX && size < 1024 * 1024 && header_end.saturating_add(size) > len {
                finding(IntegrityStatus::PossiblyDamaged, Msg::MkvPastEnd.en())
            } else if header_end > len {
                finding(IntegrityStatus::PossiblyDamaged, Msg::MkvShort.en())
            } else {
                finding(IntegrityStatus::Intact, Msg::MkvOk.en())
            }
        }
        None => finding(IntegrityStatus::PossiblyDamaged, Msg::MkvLength.en()),
    }
}

fn ebml_size(data: &[u8]) -> Option<(u64, usize)> {
    let first = *data.first()?;
    if first == 0 {
        return None;
    }
    let width = first.leading_zeros() as usize + 1;
    if width > 8 || data.len() < width {
        return None;
    }
    let mut value = u64::from(first) & ((1u64 << (8 - width)) - 1);
    for byte in data.iter().take(width).skip(1) {
        value = (value << 8) | u64::from(*byte);
    }
    let unknown = (1u64 << (7 * width)) - 1;
    if value == unknown {
        Some((u64::MAX, width))
    } else {
        Some((value, width))
    }
}

fn check_pdf<R: Read + Seek>(reader: &mut R, len: u64) -> IntegrityFinding {
    if len < 5 {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::TooShort.fill(&["PDF"]),
        );
    }
    let mut start = [0u8; 5];
    if read_exact_at(reader, 0, &mut start).is_err() {
        return finding(
            IntegrityStatus::PossiblyDamaged,
            Msg::HeadUnreadable.fill(&["PDF"]),
        );
    }
    if &start != b"%PDF-" {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::PdfNoHead.en());
    }
    let tail = match read_suffix(reader, len, 2048) {
        Ok(tail) => tail,
        Err(_) => {
            return finding(
                IntegrityStatus::PossiblyDamaged,
                Msg::TailUnreadable.fill(&["PDF"]),
            )
        }
    };
    if contains(&tail, b"%%EOF") {
        finding(IntegrityStatus::Intact, Msg::PdfOk.en())
    } else {
        finding(IntegrityStatus::PossiblyDamaged, Msg::PdfNoEof.en())
    }
}

/// Checks ZIP via the end record and central directory, without loading the payload.
fn check_ooxml<R: Read + Seek>(reader: &mut R, len: u64) -> IntegrityFinding {
    if len < 22 {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::OfficeShort.en());
    }
    let mut sig = [0u8; 8];
    if read_exact_at(reader, 0, &mut sig).is_err() {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::OfficeUnreadable.en());
    }
    // Password-protected Office files are an OLE container instead of a ZIP.
    if sig == OLE_SIGNATURE {
        return finding(IntegrityStatus::NotChecked, Msg::OfficeOle.en());
    }
    if sig[..4] != [0x50, 0x4b, 0x03, 0x04] {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::OfficeNoZip.en());
    }
    let eocd = match locate_eocd(reader, len) {
        Ok(Some(eocd)) => eocd,
        Ok(None) => return finding(IntegrityStatus::PossiblyDamaged, Msg::ZipNoEocd.en()),
        Err(_) => {
            return finding(
                IntegrityStatus::PossiblyDamaged,
                Msg::ZipEocdUnreadable.en(),
            )
        }
    };
    if eocd.cd_offset.saturating_add(eocd.cd_size) > len {
        return finding(IntegrityStatus::PossiblyDamaged, Msg::ZipDirOutside.en());
    }
    // A directory this large is no damage, just more than Veritas reads.
    if eocd.cd_size > ZIP_MAX_CD_BYTES {
        return finding(IntegrityStatus::NotChecked, Msg::ZipDirHuge.en());
    }
    match cd_has_content_types(reader, &eocd, len) {
        Ok(Some(true)) => finding(IntegrityStatus::Intact, Msg::OoxmlOk.en()),
        Ok(Some(false)) => finding(IntegrityStatus::PossiblyDamaged, Msg::NoContentTypes.en()),
        Ok(None) => finding(IntegrityStatus::NotChecked, Msg::ZipManyEntries.en()),
        Err(message) => finding(IntegrityStatus::PossiblyDamaged, message),
    }
}

/// Header of an OLE container (compound file), such as a password-protected DOCX.
const OLE_SIGNATURE: [u8; 8] = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
/// Largest ZIP central directory that Veritas looks through.
const ZIP_MAX_CD_BYTES: u64 = 16 * 1024 * 1024;
/// At most this many entries in the ZIP central directory.
const ZIP_MAX_ENTRIES: u32 = 100_000;

struct Eocd {
    cd_offset: u64,
    cd_size: u64,
}

fn locate_eocd<R: Read + Seek>(reader: &mut R, len: u64) -> io::Result<Option<Eocd>> {
    let window = len.min(65_535 + 22);
    let tail_start = len - window;
    let mut tail = vec![0u8; usize::try_from(window).unwrap_or(usize::MAX)];
    read_exact_at(reader, tail_start, &mut tail)?;
    if tail.len() < 22 {
        return Ok(None);
    }
    let mut index = tail.len() - 22;
    loop {
        if tail[index] == 0x50
            && tail.get(index + 1) == Some(&0x4b)
            && tail.get(index + 2) == Some(&0x05)
            && tail.get(index + 3) == Some(&0x06)
        {
            let comment_len = u16::from_le_bytes([tail[index + 20], tail[index + 21]]) as usize;
            if index + 22 + comment_len == tail.len() {
                let cd_size = u32::from_le_bytes([
                    tail[index + 12],
                    tail[index + 13],
                    tail[index + 14],
                    tail[index + 15],
                ]) as u64;
                let cd_offset = u32::from_le_bytes([
                    tail[index + 16],
                    tail[index + 17],
                    tail[index + 18],
                    tail[index + 19],
                ]) as u64;
                let entries = u16::from_le_bytes([tail[index + 10], tail[index + 11]]);
                if cd_size == u32::MAX as u64 || cd_offset == u32::MAX as u64 || entries == u16::MAX
                {
                    let eocd_abs = tail_start + index as u64;
                    return zip64_eocd(reader, eocd_abs);
                }
                return Ok(Some(Eocd { cd_offset, cd_size }));
            }
        }
        if index == 0 {
            break;
        }
        index -= 1;
    }
    Ok(None)
}

fn zip64_eocd<R: Read + Seek>(reader: &mut R, eocd_abs: u64) -> io::Result<Option<Eocd>> {
    if eocd_abs < 20 {
        return Ok(None);
    }
    let mut locator = [0u8; 20];
    read_exact_at(reader, eocd_abs - 20, &mut locator)?;
    if locator[0..4] != [0x50, 0x4b, 0x06, 0x07] {
        return Ok(None);
    }
    let offset = le_u64_at(&locator, 8);
    let mut record = [0u8; 56];
    read_exact_at(reader, offset, &mut record)?;
    if record[0..4] != [0x50, 0x4b, 0x06, 0x06] {
        return Ok(None);
    }
    let cd_size = le_u64_at(&record, 40);
    let cd_offset = le_u64_at(&record, 48);
    Ok(Some(Eocd { cd_offset, cd_size }))
}

/// Little-endian `u64` from `at`. Callers read only from buffers of fixed length.
fn le_u64_at<const N: usize>(bytes: &[u8; N], at: usize) -> u64 {
    u64::from_le_bytes(std::array::from_fn(|index| bytes[at + index]))
}

/// `Some(found)`, or `None` if the directory has more than `ZIP_MAX_ENTRIES`.
fn cd_has_content_types<R: Read + Seek>(
    reader: &mut R,
    eocd: &Eocd,
    len: u64,
) -> Result<Option<bool>, String> {
    // Buffered: otherwise every entry costs three small read calls to the disk.
    let mut reader = io::BufReader::with_capacity(64 * 1024, reader);
    reader
        .seek(SeekFrom::Start(eocd.cd_offset))
        .map_err(|err| Msg::ZipDirUnreadableWhy.fill(&[&err.to_string()]))?;
    let mut remaining = eocd.cd_size;
    let mut found = false;
    let mut entries = 0u32;
    while remaining >= 46 {
        if entries >= ZIP_MAX_ENTRIES {
            return Ok(None);
        }
        entries += 1;
        let mut header = [0u8; 46];
        reader
            .read_exact(&mut header)
            .map_err(|_| Msg::ZipDirBroken.en().to_string())?;
        if header[0..4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(Msg::ZipDirBroken.en().into());
        }
        let name_len = u16::from_le_bytes([header[28], header[29]]) as u64;
        let extra_len = u16::from_le_bytes([header[30], header[31]]) as u64;
        let comment_len = u16::from_le_bytes([header[32], header[33]]) as u64;
        let local_off = u32::from_le_bytes([header[42], header[43], header[44], header[45]]) as u64;
        let tail = name_len
            .saturating_add(extra_len)
            .saturating_add(comment_len);
        let total = 46u64.saturating_add(tail);
        if total > remaining {
            return Err(Msg::ZipDirBroken.en().into());
        }
        let mut name = vec![0u8; usize::try_from(name_len).unwrap_or(0)];
        reader
            .read_exact(&mut name)
            .map_err(|_| Msg::ZipNameUnreadable.en().to_string())?;
        // `seek_relative` keeps the buffer where `seek` would discard it.
        reader
            .seek_relative(i64::try_from(extra_len.saturating_add(comment_len)).unwrap_or(i64::MAX))
            .map_err(|_| Msg::ZipDirBroken.en().to_string())?;
        if name == b"[Content_Types].xml" {
            if local_off != u32::MAX as u64 {
                if local_off.saturating_add(4) > len {
                    return Err(Msg::ZipLocalMissing.en().into());
                }
                let mut local = [0u8; 4];
                read_exact_at(&mut reader, local_off, &mut local)
                    .map_err(|_| Msg::ZipLocalUnreadable.en().to_string())?;
                if local != [0x50, 0x4b, 0x03, 0x04] {
                    return Err(Msg::ZipLocalBad.en().into());
                }
                reader
                    .seek(SeekFrom::Start(
                        eocd.cd_offset + (eocd.cd_size - remaining) + total,
                    ))
                    .map_err(|_| Msg::ZipDirUnreadable.en().to_string())?;
            }
            found = true;
        }
        remaining -= total;
    }
    Ok(Some(found))
}

fn read_exact_at<R: Read + Seek>(reader: &mut R, offset: u64, buf: &mut [u8]) -> io::Result<()> {
    reader.seek(SeekFrom::Start(offset))?;
    reader.read_exact(buf)
}

fn read_suffix<R: Read + Seek>(reader: &mut R, len: u64, max_n: usize) -> io::Result<Vec<u8>> {
    let n = usize::try_from(len).unwrap_or(usize::MAX).min(max_n);
    let mut buf = vec![0u8; n];
    if n == 0 {
        return Ok(buf);
    }
    let start = len - n as u64;
    read_exact_at(reader, start, &mut buf)?;
    Ok(buf)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let size = (8 + body.len()) as u32;
        let mut out = size.to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    fn stored_zip(name: &str, data: &[u8]) -> Vec<u8> {
        let name_b = name.as_bytes();
        let mut local = Vec::new();
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u32.to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name_b);
        local.extend_from_slice(data);

        let mut cd = Vec::new();
        cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(name_b);

        let mut eocd = Vec::new();
        eocd.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        eocd.extend_from_slice(&0u16.to_le_bytes());
        eocd.extend_from_slice(&0u16.to_le_bytes());
        eocd.extend_from_slice(&1u16.to_le_bytes());
        eocd.extend_from_slice(&1u16.to_le_bytes());
        eocd.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        eocd.extend_from_slice(&(local.len() as u32).to_le_bytes());
        eocd.extend_from_slice(&0u16.to_le_bytes());

        local.extend(cd);
        local.extend(eocd);
        local
    }

    fn assert_status(path: &str, data: &[u8], status: IntegrityStatus) {
        let finding = check_bytes(Path::new(path), data);
        assert_eq!(finding.status, status, "{path}: {}", finding.reason);
    }

    /// SOI, APP1 with embedded thumbnail (own SOI/EOI), SOS, image data.
    fn jpeg_with_thumbnail(scan_data: &[u8], eoi: bool) -> Vec<u8> {
        let thumb = [0xff, 0xd8, 0x12, 0x34, 0xff, 0xd9];
        let mut out = vec![0xff, 0xd8, 0xff, 0xe1];
        out.extend_from_slice(&((thumb.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&thumb);
        out.extend_from_slice(&[0xff, 0xda, 0x00, 0x04, 0x01, 0x02]);
        out.extend_from_slice(scan_data);
        if eoi {
            out.extend_from_slice(&[0xff, 0xd9]);
        }
        out
    }

    #[test]
    fn jpeg_data_after_eoi_is_intact() {
        // DJI photos: a few KiB of padding after the end of the image.
        let mut dji = jpeg_with_thumbnail(&[0x11; 4000], true);
        dji.extend(std::iter::repeat_n(0x00, 3742));
        let finding = check_bytes(Path::new("dji.jpg"), &dji);
        assert_eq!(
            finding.status,
            IntegrityStatus::Intact,
            "{}",
            finding.reason
        );
        assert!(
            finding.reason.contains("3742 more bytes"),
            "{}",
            finding.reason
        );

        let plain = check_bytes(Path::new("a.jpg"), &jpeg_with_thumbnail(&[0x11; 10], true));
        assert_eq!(plain.reason, "JPEG: SOI (FF D8) and EOI (FF D9) present");

        let mut far = jpeg_with_thumbnail(&[0x11; 10], true);
        far.extend(std::iter::repeat_n(0x00, JPEG_TAIL));
        assert_status("weit.jpg", &far, IntegrityStatus::PossiblyDamaged);
    }

    #[test]
    fn jpeg_thumbnail_eoi_does_not_hide_a_cut_image() {
        let cut = jpeg_with_thumbnail(&[0x11; 100], false);
        let finding = check_bytes(Path::new("cut.jpg"), &cut);
        assert_eq!(finding.status, IntegrityStatus::PossiblyDamaged);
        assert!(finding.reason.contains("EOI"), "{}", finding.reason);

        // Truncated in the middle of the header: the APP1 segment extends past the end of the file.
        let head = [0xff, 0xd8, 0xff, 0xe1, 0x01, 0x00, 0x45, 0x78, 0xff, 0xd9];
        let finding = check_bytes(Path::new("kopf.jpg"), &head);
        assert_eq!(finding.status, IntegrityStatus::PossiblyDamaged);
        assert_eq!(finding.reason, "JPEG: file ends before the image data");
    }

    #[test]
    fn fixtures_valid_and_broken() {
        assert_status("a.jpg", &[0xff, 0xd8, 0xff, 0xd9], IntegrityStatus::Intact);
        assert_status(
            "a.jpg",
            &[0xff, 0xd8, 0x00, 0x11, 0x22],
            IntegrityStatus::PossiblyDamaged,
        );
        let mut png = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend_from_slice(&[0, 0, 0, 0]);
        png.extend_from_slice(b"IEND");
        png.extend_from_slice(&[0xae, 0x42, 0x60, 0x82]);
        assert_status("a.png", &png, IntegrityStatus::Intact);
        assert_status(
            "a.png",
            &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4],
            IntegrityStatus::PossiblyDamaged,
        );

        assert_status(
            "a.pdf",
            b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n%%EOF\n",
            IntegrityStatus::Intact,
        );
        assert_status(
            "a.pdf",
            b"%PDF-1.4\nhello\n",
            IntegrityStatus::PossiblyDamaged,
        );

        let docx = stored_zip("[Content_Types].xml", b"<Types></Types>");
        assert_status("a.docx", &docx, IntegrityStatus::Intact);
        let other = stored_zip("word/document.xml", b"<w/>");
        assert_status("a.docx", &other, IntegrityStatus::PossiblyDamaged);
        assert_status(
            "a.docx",
            b"PK\x03\x04truncated",
            IntegrityStatus::PossiblyDamaged,
        );

        assert_status("leer.jpg", b"", IntegrityStatus::PossiblyDamaged);
        let empty = check_bytes(Path::new("leer.jpg"), b"");
        assert!(empty.reason.contains("empty"));

        assert_status("notiz.txt", b"hello", IntegrityStatus::NotChecked);
    }

    #[test]
    fn more_containers() {
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&4u32.to_le_bytes());
        webp.extend_from_slice(b"WEBP");
        assert_status("a.webp", &webp, IntegrityStatus::Intact);
        let mut broken_webp = b"RIFF".to_vec();
        broken_webp.extend_from_slice(&1000u32.to_le_bytes());
        broken_webp.extend_from_slice(b"WEBP");
        assert_status("a.webp", &broken_webp, IntegrityStatus::PossiblyDamaged);

        let mut avi = b"RIFF".to_vec();
        avi.extend_from_slice(&4u32.to_le_bytes());
        avi.extend_from_slice(b"AVI ");
        assert_status("a.avi", &avi, IntegrityStatus::Intact);

        let tiff = b"II*\x00\x08\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_status("a.tif", tiff, IntegrityStatus::Intact);
        assert_status(
            "a.tif",
            b"II*\x00\xff\xff\xff\xff",
            IntegrityStatus::PossiblyDamaged,
        );

        assert_status(
            "a.mkv",
            &[0x1a, 0x45, 0xdf, 0xa3, 0x80],
            IntegrityStatus::Intact,
        );
        assert_status(
            "a.mkv",
            &[0x1a, 0x45, 0x00, 0x00],
            IntegrityStatus::PossiblyDamaged,
        );

        let heic = atom(b"ftyp", b"heic\0\0\0\0mif1");
        assert_status("bild.heic", &heic, IntegrityStatus::Intact);
        let not_heif = atom(b"ftyp", b"isom\0\0\0\0isom");
        assert_status("bild.heic", &not_heif, IntegrityStatus::PossiblyDamaged);

        let mut mp4 = atom(b"ftyp", b"isom\0\0\0\0isom");
        mp4.extend(atom(b"mdat", b"payload-bytes"));
        mp4.extend(atom(b"moov", b""));
        assert_status("film.mp4", &mp4, IntegrityStatus::Intact);
        let ftyp_only = atom(b"ftyp", b"isom\0\0\0\0isom");
        assert_status("film.mov", &ftyp_only, IntegrityStatus::PossiblyDamaged);
    }

    /// `moov` before `mdat` – a truncated file shows up at the truncated `mdat`.
    #[test]
    fn mp4_with_moov_first_and_a_cut_mdat_is_damaged() {
        let mut whole = atom(b"ftyp", b"isom\0\0\0\0isom");
        whole.extend(atom(b"moov", b"trak"));
        whole.extend(atom(b"mdat", &[0x55; 400]));
        assert_status("film.mp4", &whole, IntegrityStatus::Intact);
        let cut = &whole[..whole.len() - 100];
        let finding = check_bytes(Path::new("film.mp4"), cut);
        assert_eq!(finding.status, IntegrityStatus::PossiblyDamaged);
        assert!(finding.reason.contains("mdat"), "{}", finding.reason);
        // Appended bytes without an atom name are no damage.
        let mut tail = whole.clone();
        tail.extend([0xff; 16]);
        let finding = check_bytes(Path::new("film.mp4"), &tail);
        assert_eq!(
            finding.status,
            IntegrityStatus::Intact,
            "{}",
            finding.reason
        );
        assert!(finding.reason.contains("16 bytes"), "{}", finding.reason);
    }

    /// QuickTime without ftyp, password-protected DOCX, BigTIFF and PNG with trailing data no
    /// longer count as damaged.
    #[test]
    fn formats_that_were_wrongly_damaged() {
        let mut quicktime = atom(b"wide", b"");
        quicktime.extend(atom(b"mdat", &[1; 32]));
        quicktime.extend(atom(b"moov", b"mvhd"));
        let finding = check_bytes(Path::new("alt.mov"), &quicktime);
        assert_eq!(
            finding.status,
            IntegrityStatus::Intact,
            "{}",
            finding.reason
        );
        assert!(finding.reason.contains("without ftyp"));

        let mut ole = OLE_SIGNATURE.to_vec();
        ole.extend([0u8; 504]);
        assert_status("geheim.docx", &ole, IntegrityStatus::NotChecked);

        let mut big = b"II+\0\x08\0\0\0".to_vec();
        big.extend(16u64.to_le_bytes());
        big.extend([0u8; 16]);
        assert_status("gross.tif", &big, IntegrityStatus::Intact);
        assert_status("ohne-endung", &big, IntegrityStatus::Intact);
        let mut big_be = b"MM\0+\0\x08\0\0".to_vec();
        big_be.extend(4096u64.to_be_bytes());
        assert_status("gross.tif", &big_be, IntegrityStatus::PossiblyDamaged);

        let mut png = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend(13u32.to_be_bytes());
        png.extend(b"IHDR");
        png.extend([0u8; 13 + 4]);
        png.extend(0u32.to_be_bytes());
        png.extend(b"IEND\xaeB`\x82");
        assert_status("a.png", &png, IntegrityStatus::Intact);
        let mut appended = png.clone();
        appended.extend([0x42; 100]);
        let finding = check_bytes(Path::new("a.png"), &appended);
        assert_eq!(
            finding.status,
            IntegrityStatus::Intact,
            "{}",
            finding.reason
        );
        assert!(finding.reason.contains("100 bytes"));
        // Truncated stays damaged.
        assert_status(
            "a.png",
            &png[..png.len() - 14],
            IntegrityStatus::PossiblyDamaged,
        );
    }

    /// A ZIP central directory with more entries than allowed is `NotChecked`.
    #[test]
    fn zip_with_too_many_entries_is_not_checked() {
        let mut zip = b"PK\x03\x04".to_vec();
        let cd_offset = zip.len() as u32;
        let mut entry = vec![0u8; 46];
        entry[..4].copy_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        let count = ZIP_MAX_ENTRIES as usize + 1;
        for _ in 0..count {
            zip.extend_from_slice(&entry);
        }
        let cd_size = (46 * count) as u32;
        zip.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        zip.extend_from_slice(&[0u8; 6]);
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&cd_size.to_le_bytes());
        zip.extend_from_slice(&cd_offset.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        assert_status("viele.xlsx", &zip, IntegrityStatus::NotChecked);
    }
}

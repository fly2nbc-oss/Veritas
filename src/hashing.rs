use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use md5::Digest;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::i18n::Msg;
use crate::progress::Progress;

const CHUNK: usize = 1024 * 1024;

/// MD5 is the default and the only thing the window writes. Veritas reads and verifies SHA-1,
/// SHA-256 and SHA-512 from foreign checksum files (`.sha256`, `SHA256SUMS` …);
/// `Both` (MD5 and SHA-256) is available only in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashMode {
    Md5,
    Sha256,
    Both,
    Sha1,
    Sha512,
}

impl HashMode {
    /// The individual algorithms, without `Both`.
    pub const SINGLE: [HashMode; 4] = [
        HashMode::Md5,
        HashMode::Sha1,
        HashMode::Sha256,
        HashMode::Sha512,
    ];

    /// Identifier in the history (`hash_mode`) – a file format, never translated.
    pub fn id(self) -> &'static str {
        match self {
            HashMode::Md5 => "md5",
            HashMode::Sha256 => "sha256",
            HashMode::Both => "md5+sha256",
            HashMode::Sha1 => "sha1",
            HashMode::Sha512 => "sha512",
        }
    }

    /// Name for the column header and messages, the same in every language.
    pub fn label(self) -> &'static str {
        match self {
            HashMode::Md5 => "MD5",
            HashMode::Sha256 => "SHA-256",
            HashMode::Both => "MD5 + SHA-256",
            HashMode::Sha1 => "SHA-1",
            HashMode::Sha512 => "SHA-512",
        }
    }

    /// Number of hex characters of a hash; `Both` has no single length.
    pub fn hex_len(self) -> Option<usize> {
        match self {
            HashMode::Md5 => Some(32),
            HashMode::Sha1 => Some(40),
            HashMode::Sha256 => Some(64),
            HashMode::Sha512 => Some(128),
            HashMode::Both => None,
        }
    }

    /// The algorithms this mode computes.
    pub fn parts(self) -> &'static [HashMode] {
        match self {
            HashMode::Md5 => &[HashMode::Md5],
            HashMode::Sha256 => &[HashMode::Sha256],
            HashMode::Both => &[HashMode::Md5, HashMode::Sha256],
            HashMode::Sha1 => &[HashMode::Sha1],
            HashMode::Sha512 => &[HashMode::Sha512],
        }
    }

    pub fn includes_md5(self) -> bool {
        self.parts().contains(&HashMode::Md5)
    }

    pub fn includes_sha256(self) -> bool {
        self.parts().contains(&HashMode::Sha256)
    }
}

/// Hex hashes in uppercase. Missing algorithms stay empty and do not appear in the history;
/// older histories only know `md5` and `sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Digests {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
}

impl Digests {
    /// Only the one hash of the algorithm `algo`.
    pub fn of(algo: HashMode, hash: String) -> Self {
        let mut digests = Self::default();
        digests.set(algo, hash);
        digests
    }

    /// The hash of the algorithm `algo`; `None` for `Both`.
    pub fn get(&self, algo: HashMode) -> Option<&String> {
        match algo {
            HashMode::Md5 => self.md5.as_ref(),
            HashMode::Sha1 => self.sha1.as_ref(),
            HashMode::Sha256 => self.sha256.as_ref(),
            HashMode::Sha512 => self.sha512.as_ref(),
            HashMode::Both => None,
        }
    }

    /// Sets the hash of the algorithm `algo`; `Both` changes nothing.
    pub fn set(&mut self, algo: HashMode, hash: String) {
        let slot = match algo {
            HashMode::Md5 => &mut self.md5,
            HashMode::Sha1 => &mut self.sha1,
            HashMode::Sha256 => &mut self.sha256,
            HashMode::Sha512 => &mut self.sha512,
            HashMode::Both => return,
        };
        *slot = Some(hash);
    }

    /// The hash that the list and the history show: MD5, otherwise the first SHA present.
    pub fn shown(&self) -> Option<&String> {
        [
            HashMode::Md5,
            HashMode::Sha256,
            HashMode::Sha512,
            HashMode::Sha1,
        ]
        .into_iter()
        .find_map(|algo| self.get(algo))
    }

    /// `Some(true/false)` if at least one algorithm is present on both sides.
    /// `None` if the hash sets do not overlap.
    pub fn comparable_equal(&self, other: &Digests) -> Option<bool> {
        let mut compared = false;
        let mut same = true;
        for algo in HashMode::SINGLE {
            if let (Some(left), Some(right)) = (self.get(algo), other.get(algo)) {
                compared = true;
                same &= left == right;
            }
        }
        compared.then_some(same)
    }
}

struct Acc {
    md5: Option<md5::Md5>,
    sha1: Option<sha1::Sha1>,
    sha256: Option<sha2::Sha256>,
    sha512: Option<sha2::Sha512>,
}

impl Acc {
    fn new(mode: HashMode) -> Self {
        let parts = mode.parts();
        Self {
            md5: parts.contains(&HashMode::Md5).then(md5::Md5::new),
            sha1: parts.contains(&HashMode::Sha1).then(sha1::Sha1::new),
            sha256: parts.contains(&HashMode::Sha256).then(sha2::Sha256::new),
            sha512: parts.contains(&HashMode::Sha512).then(sha2::Sha512::new),
        }
    }

    fn update(&mut self, data: &[u8]) {
        if let Some(hasher) = &mut self.md5 {
            hasher.update(data);
        }
        if let Some(hasher) = &mut self.sha1 {
            hasher.update(data);
        }
        if let Some(hasher) = &mut self.sha256 {
            hasher.update(data);
        }
        if let Some(hasher) = &mut self.sha512 {
            hasher.update(data);
        }
    }

    fn finish(self) -> Digests {
        Digests {
            md5: self.md5.map(|hasher| to_hex(hasher.finalize())),
            sha256: self.sha256.map(|hasher| to_hex(hasher.finalize())),
            sha1: self.sha1.map(|hasher| to_hex(hasher.finalize())),
            sha512: self.sha512.map(|hasher| to_hex(hasher.finalize())),
        }
    }
}

pub fn to_hex(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn hash_slice(data: &[u8], mode: HashMode) -> Digests {
    let mut acc = Acc::new(mode);
    acc.update(data);
    acc.finish()
}

/// Hashes a regular file, buffered in 1 MiB chunks.
///
/// No mmap: on Windows, mapped pages stay in the process's working set; verifying large
/// videos took 2–4 GB that way. Buffered is just as fast on SSD and HDD.
pub fn hash_file(
    path: &Path,
    mode: HashMode,
    progress: Option<&Progress>,
    cancel: Option<&AtomicBool>,
) -> Result<Digests, Error> {
    if is_cancelled(cancel) {
        return Err(Error::Cancelled);
    }
    let file = open_sequential(path).map_err(|source| Error::Io {
        path: Some(path.to_path_buf()),
        source,
    })?;
    let meta = file.metadata().map_err(|source| Error::Io {
        path: Some(path.to_path_buf()),
        source,
    })?;
    if !meta.is_file() {
        return Err(Error::Message(
            Msg::NotRegular.fill(&[&path.display().to_string()]),
        ));
    }
    hash_reader(file, mode, progress, cancel, path)
}

/// Opens for reading from front to back. On Windows the cache may then read ahead and drop
/// data that has already been read (`FILE_FLAG_SEQUENTIAL_SCAN`).
fn open_sequential(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        options.custom_flags(FILE_FLAG_SEQUENTIAL_SCAN);
    }
    options.open(path)
}

thread_local! {
    /// Read buffer per thread: allocated once instead of per file. A `BufReader` in front would
    /// gain nothing – reads happen in chunks of `CHUNK` anyway.
    static BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn hash_reader(
    mut file: File,
    mode: HashMode,
    progress: Option<&Progress>,
    cancel: Option<&AtomicBool>,
    path: &Path,
) -> Result<Digests, Error> {
    BUFFER.with_borrow_mut(|buf| {
        buf.resize(CHUNK, 0);
        let mut acc = Acc::new(mode);
        loop {
            if is_cancelled(cancel) {
                return Err(Error::Cancelled);
            }
            let read = file.read(buf).map_err(|source| Error::Io {
                path: Some(path.to_path_buf()),
                source,
            })?;
            if read == 0 {
                break;
            }
            acc.update(&buf[..read]);
            if let Some(progress) = progress {
                progress.add_bytes(read as u64);
            }
        }
        Ok(acc.finish())
    })
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn known_vectors_uppercase() {
        let md5 = hash_slice(b"abc", HashMode::Md5);
        assert_eq!(md5.md5.as_deref(), Some("900150983CD24FB0D6963F7D28E17F72"));
        assert!(md5.sha256.is_none());
        let empty = hash_slice(b"", HashMode::Md5);
        assert_eq!(
            empty.md5.as_deref(),
            Some("D41D8CD98F00B204E9800998ECF8427E")
        );
        let sha = hash_slice(b"abc", HashMode::Sha256);
        assert_eq!(
            sha.sha256.as_deref(),
            Some("BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD")
        );
        let both = hash_slice(b"abc", HashMode::Both);
        assert!(both.md5.is_some() && both.sha256.is_some());
        assert!(both.sha1.is_none() && both.sha512.is_none());
        let sha1 = hash_slice(b"abc", HashMode::Sha1);
        assert_eq!(
            sha1.sha1.as_deref(),
            Some("A9993E364706816ABA3E25717850C26C9CD0D89D")
        );
        assert!(sha1.md5.is_none() && sha1.sha256.is_none());
        let sha512 = hash_slice(b"abc", HashMode::Sha512);
        assert_eq!(
            sha512.sha512.as_deref(),
            Some(concat!(
                "DDAF35A193617ABACC417349AE20413112E6FA4E89A97EA20A9EEEE64B55D39A",
                "2192992A274FC1A836BA3C23A3FEEBBD454D4423643CE80E2A9AC94FA54CA49F"
            ))
        );
        for algo in HashMode::SINGLE {
            let hex = hash_slice(b"abc", algo);
            assert_eq!(hex.get(algo).map(String::len), algo.hex_len());
            assert_eq!(hex.shown(), hex.get(algo));
        }
    }

    #[test]
    fn file_hash_matches_bytes_across_chunks() {
        let dir = std::env::temp_dir().join(format!("veritas-hash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let small = dir.join("small.txt");
        fs::write(&small, b"abc").unwrap();
        assert_eq!(
            hash_file(&small, HashMode::Md5, None, None)
                .unwrap()
                .md5
                .as_deref(),
            Some("900150983CD24FB0D6963F7D28E17F72")
        );
        let empty = dir.join("empty.txt");
        fs::write(&empty, b"").unwrap();
        assert_eq!(
            hash_file(&empty, HashMode::Md5, None, None)
                .unwrap()
                .md5
                .as_deref(),
            Some("D41D8CD98F00B204E9800998ECF8427E")
        );

        let big = dir.join("big.bin");
        let mut payload = vec![0u8; 2 * CHUNK + 4096];
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }
        {
            let mut file = File::create(&big).unwrap();
            file.write_all(&payload).unwrap();
        }
        let via_file = hash_file(&big, HashMode::Both, None, None).unwrap();
        let via_mem = hash_slice(&payload, HashMode::Both);
        assert_eq!(via_file, via_mem);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_flag_stops_hashing() {
        let dir = std::env::temp_dir().join(format!("veritas-cancel-hash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.bin");
        fs::write(&path, vec![1u8; 64]).unwrap();
        let flag = AtomicBool::new(true);
        let err = hash_file(&path, HashMode::Md5, None, Some(&flag)).unwrap_err();
        assert!(matches!(err, Error::Cancelled));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn comparable_hashes() {
        let left = Digests::of(HashMode::Md5, "A".into());
        let mut right = left.clone();
        right.set(HashMode::Sha256, "B".into());
        assert_eq!(left.comparable_equal(&right), Some(true));
        let changed = Digests::of(HashMode::Md5, "C".into());
        assert_eq!(left.comparable_equal(&changed), Some(false));
        let other = Digests::of(HashMode::Sha256, "B".into());
        assert_eq!(left.comparable_equal(&other), None);
        // SHA-1 and SHA-512 compare the same way.
        for algo in [HashMode::Sha1, HashMode::Sha512] {
            let one = Digests::of(algo, "X".into());
            assert_eq!(
                one.comparable_equal(&Digests::of(algo, "X".into())),
                Some(true)
            );
            assert_eq!(
                one.comparable_equal(&Digests::of(algo, "Y".into())),
                Some(false)
            );
            assert_eq!(one.comparable_equal(&left), None);
        }
        // An old history without the new fields reads as before; empty ones are absent in the JSON.
        let old: Digests = serde_json::from_str(r#"{"md5": "A"}"#).unwrap();
        assert_eq!(old, left);
        assert_eq!(serde_json::to_string(&other).unwrap(), r#"{"sha256":"B"}"#);
    }
}

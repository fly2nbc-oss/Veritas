use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::hashing::Digests;
use crate::i18n::Msg;
use crate::scanning::RowKind;

/// Result of one file as soon as the run knows it, for the interim state of the list. The kind is
/// derived in the run by the same rule as at the end (`report::verdict`); the structure check only
/// arrives with the result, so until then a mismatch is just "mismatch".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRow {
    /// Relative path, as the run found it on disk.
    pub path: String,
    pub kind: RowKind,
    pub new_hash: Digests,
    /// Reason for `RowKind::Error`, in English as in the result.
    pub message: Option<String>,
}

/// Progress that the scan thread writes and the UI reads.
#[derive(Debug)]
pub struct Progress {
    phase: Mutex<String>,
    /// The file whose hash has just started (relative); empty outside of hashing.
    current: Mutex<String>,
    files_done: AtomicU64,
    files_total: AtomicU64,
    bytes_done: AtomicU64,
    bytes_total: AtomicU64,
    /// Create after a run: this many checksums the run carries over without reading. They do not
    /// count in `files_total`; the pill counts only what is read.
    reused: AtomicU64,
    started: Mutex<Instant>,
    /// The run is writing the checksum file and the history; from here on there is no cancelling.
    writing: AtomicBool,
    /// Results per file since the last pickup (`take_live`). The UI picks them up every few
    /// seconds; afterwards the buffer is empty again.
    live: Mutex<Vec<LiveRow>>,
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

impl Progress {
    pub fn new() -> Self {
        Self {
            phase: Mutex::new(Msg::PhaseReady.en().to_string()),
            current: Mutex::new(String::new()),
            files_done: AtomicU64::new(0),
            files_total: AtomicU64::new(0),
            bytes_done: AtomicU64::new(0),
            bytes_total: AtomicU64::new(0),
            reused: AtomicU64::new(0),
            started: Mutex::new(Instant::now()),
            writing: AtomicBool::new(false),
            live: Mutex::new(Vec::new()),
        }
    }

    /// From the run thread: the result of one file for the interim state.
    pub fn push_live(&self, row: LiveRow) {
        self.live
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(row);
    }

    /// From the UI: all results since the last pickup, in the order they arrived. The buffer is
    /// empty afterwards.
    pub fn take_live(&self) -> Vec<LiveRow> {
        std::mem::take(&mut *self.live.lock().unwrap_or_else(|err| err.into_inner()))
    }

    /// From the run thread right before writing: `true` if writing is allowed. After this, a cancel
    /// no longer applies. Together with `request_cancel` it forms a handshake (both `SeqCst`, write
    /// first, then read): at least one side sees the other. So the run does not write if the UI
    /// reports the cancel as accepted.
    pub fn begin_writing(&self, cancel: &AtomicBool) -> bool {
        self.writing.store(true, Ordering::SeqCst);
        if cancel.load(Ordering::SeqCst) {
            self.writing.store(false, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// From the UI: asks for a cancel. `false` if the run is already writing; then it finishes,
    /// because a cancel would land between the checksum file and the history.
    pub fn request_cancel(&self, cancel: &AtomicBool) -> bool {
        if self.is_writing() {
            return false;
        }
        cancel.store(true, Ordering::SeqCst);
        !self.is_writing()
    }

    /// Whether the run is writing right now (see `begin_writing`).
    pub fn is_writing(&self) -> bool {
        self.writing.load(Ordering::SeqCst)
    }

    /// New phase; the current file no longer applies.
    pub fn set_phase(&self, phase: impl Into<String>) {
        *self.phase.lock().unwrap_or_else(|err| err.into_inner()) = phase.into();
        self.current
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
    }

    /// The file whose hash is just starting. With several threads, the one started last.
    pub fn set_current(&self, path: &str) {
        let mut current = self.current.lock().unwrap_or_else(|err| err.into_inner());
        current.clear();
        current.push_str(path);
    }

    pub fn reset_clock(&self) {
        *self.started.lock().unwrap_or_else(|err| err.into_inner()) = Instant::now();
    }

    pub fn set_files_total(&self, n: u64) {
        self.files_total.store(n, Ordering::Relaxed);
    }

    pub fn set_bytes_total(&self, n: u64) {
        self.bytes_total.store(n, Ordering::Relaxed);
    }

    pub fn set_reused(&self, n: u64) {
        self.reused.store(n, Ordering::Relaxed);
    }

    pub fn add_bytes(&self, n: u64) {
        self.bytes_done.fetch_add(n, Ordering::Relaxed);
    }

    pub fn inc_files(&self) {
        self.files_done.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> ProgressSnap {
        let phase = self
            .phase
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let current = self
            .current
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let started = *self.started.lock().unwrap_or_else(|err| err.into_inner());
        ProgressSnap {
            phase,
            current,
            files_done: self.files_done.load(Ordering::Relaxed),
            files_total: self.files_total.load(Ordering::Relaxed),
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
            reused: self.reused.load(Ordering::Relaxed),
            elapsed: started.elapsed(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressSnap {
    pub phase: String,
    /// See `Progress::set_current`.
    pub current: String,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// See `Progress::set_reused`.
    pub reused: u64,
    pub elapsed: Duration,
}

impl ProgressSnap {
    /// Throughput in MB/s, based on 1024×1024 bytes.
    pub fn mb_per_sec(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs < 0.05 || self.bytes_done == 0 {
            return 0.0;
        }
        (self.bytes_done as f64 / secs) / (1024.0 * 1024.0)
    }

    pub fn fraction(&self) -> f32 {
        if self.bytes_total > 0 {
            return (self.bytes_done as f32 / self.bytes_total as f32).clamp(0.0, 1.0);
        }
        if self.files_total > 0 {
            return (self.files_done as f32 / self.files_total as f32).clamp(0.0, 1.0);
        }
        0.0
    }

    pub fn eta(&self) -> Option<Duration> {
        if self.bytes_done == 0 || self.bytes_total <= self.bytes_done {
            return None;
        }
        let secs = self.elapsed.as_secs_f64();
        if secs < 0.2 {
            return None;
        }
        let rate = self.bytes_done as f64 / secs;
        if rate <= 0.0 {
            return None;
        }
        let left = (self.bytes_total - self.bytes_done) as f64 / rate;
        Some(Duration::from_secs_f64(left))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An accepted cancel holds back the writing; a cancel during writing is rejected and has no
    /// effect.
    #[test]
    fn cancel_and_writing_exclude_each_other() {
        let progress = Progress::new();
        let cancel = AtomicBool::new(false);
        assert!(progress.request_cancel(&cancel), "accepted before writing");
        assert!(
            !progress.begin_writing(&cancel),
            "nothing is written afterwards"
        );
        assert!(!progress.is_writing());

        let progress = Progress::new();
        let cancel = AtomicBool::new(false);
        assert!(progress.begin_writing(&cancel));
        assert!(progress.is_writing());
        assert!(!progress.request_cancel(&cancel), "rejected while writing");
        assert!(!cancel.load(Ordering::SeqCst), "the signal stays off");
    }
}

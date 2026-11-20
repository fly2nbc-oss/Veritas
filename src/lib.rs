//! Veritas checks file collections locally: MD5 checksums (also reads SHA-1/256/512),
//! a JSON change log and a structural integrity check.
// The program must not abort; tests may (`clippy.toml`).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod about;
mod checksum;
pub mod crash;
mod drive;
mod error;
mod fonts;
mod format;
mod hashing;
mod history;
mod i18n;
mod integrity;
mod list;
mod preview;
mod progress;
mod report;
mod scanning;
pub mod screenshot;
mod system;
mod theme;
mod tree;
pub mod ui;

pub use checksum::{format_checksum, format_line, output_names, parse_checksum, NameStyle};
pub use error::Error;
pub use hashing::{hash_file, hash_slice, Digests, HashMode};
pub use integrity::{check_path, IntegrityFinding, IntegrityStatus};
pub use progress::Progress;
pub use report::{classify, ChangeReport};
pub use scanning::{run_scan, FileRow, JobKind, RowKind, ScanOptions, ScanOutcome, Summary};

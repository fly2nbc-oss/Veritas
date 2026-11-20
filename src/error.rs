use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::i18n::Msg;

/// Error of a run. The text (`Display`) is English and built from `Msg` templates: that is how
/// it appears in `veritas.log` and in the history, and the interface shows it via `i18n::shown`
/// in the chosen language.
#[derive(Debug)]
pub enum Error {
    NotADirectory(PathBuf),
    Io {
        path: Option<PathBuf>,
        source: io::Error,
    },
    /// Error while writing (`.md5`, history) – its own text, so that a write-protected medium
    /// does not appear as "No permission to read".
    Write {
        path: PathBuf,
        source: io::Error,
    },
    ChecksumMissing {
        path: PathBuf,
    },
    ChecksumInvalid {
        path: PathBuf,
        detail: String,
    },
    Cancelled,
    ThreadPool(String),
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Error::NotADirectory(path) => Msg::NotADirectory.fill(&[&path.display().to_string()]),
            Error::Io { path, source } => {
                let (with_path, plain) = io_kind(source);
                match path {
                    Some(path) => {
                        with_path.fill(&[&path.display().to_string(), &source.to_string()])
                    }
                    None => plain.fill(&[&source.to_string()]),
                }
            }
            Error::Write { path, source } => {
                write_kind(source).fill(&[&path.display().to_string(), &source.to_string()])
            }
            Error::ChecksumMissing { path } => {
                Msg::ChecksumMissing.fill(&[&path.display().to_string()])
            }
            Error::ChecksumInvalid { path, detail } => {
                Msg::ChecksumInvalid.fill(&[&path.display().to_string(), detail])
            }
            Error::Cancelled => Msg::RunCancelled.en().to_string(),
            Error::ThreadPool(detail) => Msg::ThreadPool.fill(&[detail]),
            Error::Message(detail) => detail.clone(),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } | Error::Write { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Read error: the template with path and the one without.
fn io_kind(err: &io::Error) -> (Msg, Msg) {
    match err.kind() {
        io::ErrorKind::PermissionDenied => (Msg::ReadDeniedAt, Msg::ReadDeniedPlain),
        io::ErrorKind::NotFound => (Msg::NotFoundAt, Msg::NotFoundPlain),
        _ => (Msg::IoErrorAt, Msg::IoErrorPlain),
    }
}

/// Write error: the template with path.
fn write_kind(err: &io::Error) -> Msg {
    match err.kind() {
        io::ErrorKind::PermissionDenied => Msg::WriteDenied,
        io::ErrorKind::ReadOnlyFilesystem => Msg::WriteReadOnly,
        io::ErrorKind::StorageFull => Msg::WriteFull,
        io::ErrorKind::NotFound => Msg::NotFoundAt,
        io::ErrorKind::AlreadyExists => Msg::WriteInTheWay,
        _ => Msg::WriteFailed,
    }
}

pub fn file_problem(err: &Error) -> String {
    match err {
        Error::Io { source, .. } if source.kind() == io::ErrorKind::PermissionDenied => {
            Msg::FileReadDenied.en().to_string()
        }
        Error::Io { source, .. } if source.kind() == io::ErrorKind::NotFound => {
            Msg::FileVanished.en().to_string()
        }
        Error::Io { source, .. } => Msg::FileReadError.fill(&[&source.to_string()]),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A write error is not called "No permission to read". The text is English as in the
    /// history; the interface translates it (`i18n::shown`).
    #[test]
    fn write_errors_read_differently_from_read_errors() {
        let denied = || io::Error::from(io::ErrorKind::PermissionDenied);
        let write = Error::Write {
            path: PathBuf::from("Fotos.json"),
            source: denied(),
        };
        let read = Error::Io {
            path: Some(PathBuf::from("a.jpg")),
            source: denied(),
        };
        assert!(write
            .to_string()
            .starts_with("No permission to write: Fotos.json"));
        assert!(read.to_string().starts_with("No permission to read"));
        let full = Error::Write {
            path: PathBuf::from("x"),
            source: io::Error::from(io::ErrorKind::StorageFull),
        };
        assert!(full.to_string().contains("full"));
    }
}

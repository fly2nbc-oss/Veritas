//! Crash and run log. On Windows the release EXE is a windowed program: what goes to stderr is
//! seen by nobody. Therefore:
//! - A panic appends message and call chain to `crash.log`; the next start points it out.
//!   Before the language choice the file was called `absturz.log`; the start renames it once
//!   (`adopt_old_log`).
//! - Warnings and errors from eframe, winit and OpenGL as well as the start and end of every
//!   run go to `veritas.log` (crate `log`). If the display freezes, it shows there whether an
//!   error occurred while drawing and whether the run finished anyway.
//!
//! Both logs are English, independent of the interface language – error reports look the same
//! everywhere (decided on 2026-11-20).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Name of the file in the data folder (`eframe::storage_dir`).
pub const LOG_NAME: &str = "crash.log";

/// What it was called before the language choice.
const OLD_LOG_NAME: &str = "absturz.log";

/// Run log in the same folder.
pub const RUN_LOG_NAME: &str = "veritas.log";

/// From this size on a log starts anew, so that it does not grow without limit.
const MAX_LEN: u64 = 1024 * 1024;

/// Windows: `%APPDATA%\Veritas\data\crash.log`, Linux: `~/.local/share/veritas/crash.log`.
pub fn log_path() -> Option<PathBuf> {
    eframe::storage_dir("Veritas").map(|dir| dir.join(LOG_NAME))
}

/// Sets the panic hook. The previous hook keeps running afterwards (message on stderr).
pub fn install() {
    if let Some(path) = log_path() {
        adopt_old_log(&path);
        install_at(path);
    }
}

/// Adopts an `absturz.log` of older versions as `crash.log` as long as there is none yet:
/// renaming keeps content and modification time, so the notice at start (`last_written`)
/// stays as it was. If a `crash.log` already exists, the old one is left untouched – nothing
/// is lost.
fn adopt_old_log(path: &Path) {
    let Some(old) = path.parent().map(|dir| dir.join(OLD_LOG_NAME)) else {
        return;
    };
    if old.is_file() && !path.exists() {
        let _ = fs::rename(&old, path);
    }
}

fn install_at(path: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let text = entry(
            &jiff::ZonedDateTime::now()
                .strftime("%Y-%m-%d %H:%M:%S")
                .to_string(),
            thread.name().unwrap_or("unnamed"),
            &info.to_string(),
            &std::backtrace::Backtrace::force_capture().to_string(),
        );
        // A failure while writing must not hide the panic.
        let _ = append(&path, &text);
        previous(info);
    }));
}

/// Routes `log` messages to `veritas.log`: from Veritas from info level, from all other crates
/// (eframe, winit, glutin, egui_glow) from warning level.
pub fn install_run_log() {
    let Some(path) = eframe::storage_dir("Veritas").map(|dir| dir.join(RUN_LOG_NAME)) else {
        return;
    };
    let logger = FileLogger {
        path,
        lock: std::sync::Mutex::new(()),
    };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    log::info!(
        "Start: Veritas {} · {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}

struct FileLogger {
    path: PathBuf,
    /// Keeps lines from several threads apart.
    lock: std::sync::Mutex<()>,
}

fn wanted(metadata: &log::Metadata<'_>) -> bool {
    let own = metadata.target() == "veritas" || metadata.target().starts_with("veritas::");
    metadata.level()
        <= if own {
            log::Level::Info
        } else {
            log::Level::Warn
        }
}

fn line(time: &str, record: &log::Record<'_>) -> String {
    let thread = std::thread::current();
    format!(
        "{time} {:<5} [{}] {}: {}
",
        record.level(),
        thread.name().unwrap_or("unnamed"),
        record.target(),
        record.args()
    )
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        wanted(metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        if !wanted(record.metadata()) {
            return;
        }
        let text = line(
            &jiff::ZonedDateTime::now()
                .strftime("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            record,
        );
        let _guard = self.lock.lock().unwrap_or_else(|err| err.into_inner());
        let _ = append(&self.path, &text);
    }

    fn flush(&self) {}
}

/// An entry: time, version, system, thread, message with code location, call chain.
fn entry(time: &str, thread: &str, message: &str, backtrace: &str) -> String {
    format!(
        "=== {time} · Veritas {} · {} {} · Thread {thread}\n{message}\n\n{backtrace}\n\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

fn append(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let restart = fs::metadata(path).is_ok_and(|meta| meta.len() > MAX_LEN);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(!restart)
        .truncate(restart)
        .open(path)?;
    file.write_all(text.as_bytes())
}

/// Time of the last entry in seconds since 1970, if the log exists.
pub fn last_written(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|age| age.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("veritas-crash-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn entry_names_version_thread_and_location() {
        let text = entry(
            "2026-11-07 22:52:10",
            "main",
            "panicked at src/ui.rs:1:2:\nindex out of bounds",
            "0: veritas::ui::x",
        );
        assert!(text.starts_with("=== 2026-11-07 22:52:10 · Veritas "));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        assert!(text.contains("Thread main"));
        assert!(text.contains("src/ui.rs:1:2"));
        assert!(text.contains("0: veritas::ui::x"));
    }

    #[test]
    fn append_creates_folder_keeps_old_entries_and_restarts_when_large() {
        let dir = temp_dir("append");
        let path = dir.join("data").join(LOG_NAME);
        assert_eq!(last_written(&path), None);
        append(&path, "eins\n").unwrap();
        append(&path, "zwei\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "eins\nzwei\n");
        assert!(last_written(&path).is_some());

        fs::write(&path, vec![b'x'; MAX_LEN as usize + 1]).unwrap();
        append(&path, "drei\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "drei\n");
        let _ = fs::remove_dir_all(&dir);
    }

    /// An `absturz.log` of an older version becomes `crash.log`, with content and modification
    /// time; if a `crash.log` already exists, the old one is left untouched.
    #[test]
    fn the_old_crash_log_is_adopted_without_loss() {
        let dir = temp_dir("adopt");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(LOG_NAME);
        let old = dir.join(OLD_LOG_NAME);
        fs::write(&old, "alt\n").unwrap();
        let written = last_written(&old);
        adopt_old_log(&path);
        assert!(!old.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "alt\n");
        assert_eq!(last_written(&path), written);

        fs::write(&old, "noch einer\n").unwrap();
        adopt_old_log(&path);
        assert_eq!(fs::read_to_string(&old).unwrap(), "noch einer\n");
        assert_eq!(fs::read_to_string(&path).unwrap(), "alt\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_log_keeps_own_info_and_foreign_warnings() {
        let meta = |level, target| log::Metadata::builder().level(level).target(target).build();
        assert!(wanted(&meta(log::Level::Info, "veritas::ui")));
        assert!(wanted(&meta(log::Level::Info, "veritas")));
        assert!(!wanted(&meta(log::Level::Debug, "veritas::ui")));
        assert!(wanted(&meta(
            log::Level::Error,
            "eframe::native::glow_integration"
        )));
        assert!(wanted(&meta(log::Level::Warn, "winit")));
        assert!(!wanted(&meta(log::Level::Info, "eframe")));
        assert!(!wanted(&meta(log::Level::Info, "veritasx")));

        let args = format_args!("swap_buffers failed: lost");
        let record = log::Record::builder()
            .level(log::Level::Error)
            .target("eframe")
            .args(args)
            .build();
        let text = line("2026-11-07 23:51:08.123", &record);
        assert!(
            text.starts_with("2026-11-07 23:51:08.123 ERROR ["),
            "{text}"
        );
        assert!(
            text.ends_with(
                "] eframe: swap_buffers failed: lost
"
            ),
            "{text}"
        );
    }

    #[test]
    fn hook_writes_a_panic_from_another_thread() {
        let dir = temp_dir("hook");
        let path = dir.join(LOG_NAME);
        install_at(path.clone());
        let joined = std::thread::Builder::new()
            .name("probe".into())
            .spawn(|| panic!("probe for the crash log"))
            .unwrap()
            .join();
        // Back to the default hook, so that other tests do not write into the log.
        drop(std::panic::take_hook());
        assert!(joined.is_err());
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("Thread probe"), "{text}");
        assert!(text.contains("probe for the crash log"), "{text}");
        assert!(text.contains("src"), "{text}");
        let _ = fs::remove_dir_all(&dir);
    }
}

//! Flow of the screenshot mode (`screenshot`): open folder, wait for the preview, verify on
//! request, select a file with a finding, request the picture, save, close. Each step sets
//! the state directly – no simulated keys or clicks.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, ColorImage};

use super::{Pane, Settings, VeritasApp};
use crate::scanning::{JobKind, RowMark};
use crate::screenshot::{self, Args, Outcome};
use crate::theme;
use crate::tree::Tree;

/// How long a capture may take, verify included.
const TIMEOUT: Duration = Duration::from_secs(90);
/// Frames without change before the capture: an `Area` appears only in the second frame, the
/// list scrolls to the selection.
const SETTLE_FRAMES: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    /// Set pixels per point and window size.
    Scale,
    /// The preview of the folder is being read.
    Loading,
    /// The verify run is running.
    Running,
    /// Count quiet frames until size and content are settled.
    Settling(u32),
    /// The picture has been requested.
    Waiting,
    /// Done or failed; the window closes.
    Done,
}

pub(super) struct Shot {
    pub(super) args: Args,
    pub(super) phase: Phase,
    started: Instant,
    /// Frames in a row in which the window did not have the requested size.
    misfits: u32,
    outcome: Outcome,
    image: Option<Receiver<Arc<ColorImage>>>,
}

impl VeritasApp {
    /// The window of the screenshot mode (`screenshot::run`).
    pub fn screenshot(cc: &eframe::CreationContext<'_>, args: Args, outcome: Outcome) -> Self {
        Self::for_shot(&cc.egui_ctx, args, outcome)
    }

    /// Without stored settings, without crash notice. The tree has the parent folder of the shown
    /// folder as its only root: it shows only folder names, no path with the user name
    /// (`C:\Users\…`).
    pub(super) fn for_shot(ctx: &egui::Context, args: Args, outcome: Outcome) -> Self {
        theme::install(ctx);
        ctx.set_theme(args.theme);
        // Only for the capture; `save` writes nothing in screenshot mode.
        crate::i18n::set(args.lang);
        // Fixed thread count instead of the machine's: the picture is the same everywhere.
        let mut app = Self::with_settings(Settings {
            threads: 8,
            ..Settings::default()
        });
        let folder = std::path::absolute(&args.folder).unwrap_or_else(|_| args.folder.clone());
        let root = folder
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| folder.clone(), PathBuf::from);
        app.tree = Tree::new(vec![root]);
        // A missing value does not silently open the parent folder (open_path).
        if folder.is_dir() {
            app.open_path(ctx, folder);
        }
        app.shot = Some(Shot {
            args,
            phase: Phase::Scale,
            started: Instant::now(),
            misfits: 0,
            outcome,
            image: None,
        });
        app
    }

    /// One step per frame, after `show`.
    pub(super) fn drive_shot(&mut self, ctx: &egui::Context) {
        let Some(phase) = self.shot.as_ref().map(|shot| shot.phase) else {
            return;
        };
        if phase == Phase::Done {
            return;
        }
        // The picture arrives via a callback that does not trigger a new frame.
        ctx.request_repaint();
        if self
            .shot
            .as_ref()
            .is_some_and(|shot| shot.started.elapsed() > TIMEOUT)
        {
            self.finish_shot(
                ctx,
                Err(format!(
                    "No picture after {} s – folder too large or window covered?",
                    TIMEOUT.as_secs()
                )),
            );
            return;
        }
        // A notice (result of the run) does not belong in the picture; the status bar says it.
        if let Some(notice) = self.notice.take() {
            if notice.tone == super::Tone::Error {
                self.finish_shot(ctx, Err(notice.text.get()));
                return;
            }
        }
        match phase {
            Phase::Scale => {
                if let Some(shot) = &mut self.shot {
                    if let Some(scale) = shot.args.scale {
                        ctx.set_pixels_per_point(scale);
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(shot.args.size.into()));
                    shot.phase = Phase::Loading;
                }
            }
            Phase::Loading => {
                if self.root.is_none() {
                    self.finish_shot(ctx, Err("The folder could not be opened.".into()));
                } else if self.pending.is_none() {
                    let verify = self.shot.as_ref().is_some_and(|shot| shot.args.verify);
                    if verify {
                        if let Err(reason) = self.can_start(JobKind::Verify) {
                            self.finish_shot(ctx, Err(reason.get()));
                            return;
                        }
                        self.run(ctx, JobKind::Verify);
                        self.set_phase(Phase::Running);
                    } else {
                        self.select_finding();
                        self.set_phase(Phase::Settling(0));
                    }
                }
            }
            Phase::Running => {
                if self.running.is_none() {
                    self.select_finding();
                    self.set_phase(Phase::Settling(0));
                }
            }
            Phase::Settling(frames) => {
                let size = self
                    .shot
                    .as_ref()
                    .map_or(egui::Vec2::ZERO, |shot| shot.args.size.into());
                let fits = (ctx.content_rect().size() - size).abs().max_elem() < 1.0;
                if !fits {
                    self.set_phase(Phase::Settling(0));
                    // Some window systems ignore the first resize.
                    if let Some(shot) = &mut self.shot {
                        shot.misfits += 1;
                        if shot.misfits % 30 == 0 {
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                        }
                    }
                } else if frames + 1 < SETTLE_FRAMES {
                    self.set_phase(Phase::Settling(frames + 1));
                } else {
                    let (tx, rx) = mpsc::channel();
                    let repaint = ctx.clone();
                    ctx.request_screenshot(move |image| {
                        let _ = tx.send(image);
                        repaint.request_repaint();
                    });
                    if let Some(shot) = &mut self.shot {
                        shot.image = Some(rx);
                        shot.phase = Phase::Waiting;
                    }
                }
            }
            Phase::Waiting => {
                let image = self
                    .shot
                    .as_ref()
                    .and_then(|shot| shot.image.as_ref())
                    .and_then(|rx| rx.try_recv().ok());
                if let Some(image) = image {
                    let result = self.save_shot(&image);
                    self.finish_shot(ctx, result);
                }
            }
            Phase::Done => {}
        }
    }

    fn set_phase(&mut self, phase: Phase) {
        if let Some(shot) = &mut self.shot {
            shot.phase = phase;
        }
    }

    /// Selects the first file with a finding – "abweichend" first, then "beschädigt" –, so that
    /// the status bar shows its explanation. The arrow keys then belong to the list.
    fn select_finding(&mut self) {
        let wanted = [RowMark::Wrong, RowMark::Damaged];
        let found = wanted.iter().find_map(|mark| {
            self.rows
                .iter()
                .find(|row| row.mark() == Some(*mark))
                .map(|row| row.path.clone())
        });
        if let Some(path) = found {
            self.selected = Some(path);
            self.pane = Pane::List;
        }
    }

    /// Checks the size (with a fixed `--scale`) and writes the PNG.
    pub(super) fn save_shot(&self, image: &ColorImage) -> Result<PathBuf, String> {
        let Some(shot) = &self.shot else {
            return Err("Not in screenshot mode.".into());
        };
        if let Some([w, h]) = shot.args.pixels() {
            if image.size != [w, h] {
                return Err(format!(
                    "The picture has {}×{} pixels instead of {w}×{h}. Does the window fit on \
                     the screen? Otherwise choose a smaller --size or --scale.",
                    image.size[0], image.size[1]
                ));
            }
        }
        screenshot::write_png(&shot.args.out, image)?;
        Ok(shot.args.out.clone())
    }

    fn finish_shot(&mut self, ctx: &egui::Context, result: Result<PathBuf, String>) {
        if let Some(shot) = &mut self.shot {
            shot.phase = Phase::Done;
            match shot.outcome.lock() {
                Ok(mut slot) => *slot = result,
                Err(poisoned) => *poisoned.into_inner() = result,
            }
        }
        if self.running.is_some() {
            // Otherwise handle_close would ask ("Lauf abbrechen und schließen?") – and nobody
            // answers, there is no input here.
            self.decide_close(ctx, true);
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Mutex;

    use super::*;
    use crate::scanning::{run_scan, ScanOptions};

    /// Copies `docs/examples/Urlaub 2026` into a fresh temp folder – a verify run appends to the
    /// history, so the checked-in files stay untouched.
    struct Example {
        base: PathBuf,
        folder: PathBuf,
    }

    impl Example {
        fn copy(name: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "veritas-beispiel-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/examples/Urlaub 2026");
            let folder = base.join("Fotos").join("Urlaub 2026");
            copy_dir(&source, &folder);
            Self { base, folder }
        }
    }

    impl Drop for Example {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    fn shot_args(folder: &Path, out: &Path) -> Args {
        Args {
            out: out.to_path_buf(),
            theme: egui::Theme::Dark,
            lang: crate::i18n::Lang::De,
            verify: true,
            size: [960.0, 640.0],
            scale: Some(1.5),
            folder: folder.to_path_buf(),
        }
    }

    /// Verifying the example data yields exactly the statuses the README picture should show.
    #[test]
    fn the_example_folder_shows_every_status() {
        let example = Example::copy("status");
        let opts = ScanOptions {
            root: example.folder.clone(),
            recursive: true,
            threads: 2,
            seeks: Some(false),
            hash_mode: crate::hashing::HashMode::Md5,
            name_style: crate::checksum::NameStyle::FolderName,
            md5_file: Some("Urlaub 2026.md5".into()),
            kind: JobKind::Verify,
            known: None,
        };
        let progress = crate::progress::Progress::new();
        let outcome =
            run_scan(&opts, &progress, &std::sync::atomic::AtomicBool::new(false)).unwrap();
        let mark = |path: &str| {
            outcome
                .rows
                .iter()
                .find(|row| row.path == path)
                .and_then(|row| row.mark())
        };
        for ok in [
            "IMG_0407.jpg",
            "IMG_0408.jpg",
            "IMG_0409.jpg",
            "IMG_0410.jpg",
            "IMG_0411.jpg",
            "IMG_0412.jpg",
            "Strand.png",
            "Reiseplan.pdf",
            "Notizen.txt",
            "Drohne/DJI_0001.MP4",
            "Drohne/DJI_0002.JPG",
        ] {
            assert_eq!(mark(ok), Some(RowMark::Ok), "{ok}");
        }
        assert_eq!(mark("IMG_0413.jpg"), Some(RowMark::Wrong));
        assert_eq!(mark("IMG_0414.jpg"), Some(RowMark::Damaged));
        assert_eq!(mark("IMG_0415.jpg"), Some(RowMark::Deleted));
        assert_eq!(mark("Packliste.txt"), Some(RowMark::New));
        assert_eq!(outcome.summary.errors, 0);
        // The history is neutralised by hand (path, time); it must still be valid.
        assert!(
            outcome.report.warnings.is_empty(),
            "{:?}",
            outcome.report.warnings
        );
    }

    /// A frame as in frame::App::ui; returns the commands to the window.
    fn frame(ctx: &egui::Context, app: &mut VeritasApp) -> Vec<egui::ViewportCommand> {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960.0, 640.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.show(ui);
            app.drive_shot(ui.ctx());
        });
        output.textures_delta.clear();
        output
            .viewport_output
            .remove(&egui::ViewportId::ROOT)
            .map(|viewport| viewport.commands)
            .unwrap_or_default()
    }

    /// Frames until the capture waits or is done; returns the request for the picture.
    fn run_until_request(
        ctx: &egui::Context,
        app: &mut VeritasApp,
    ) -> Option<egui::ScreenshotCallback> {
        let mut request = None;
        for _ in 0..1000 {
            if app
                .shot
                .as_ref()
                .is_some_and(|shot| matches!(shot.phase, Phase::Waiting | Phase::Done))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            for command in frame(ctx, app) {
                if let egui::ViewportCommand::Screenshot(callback) = command {
                    request = Some(callback);
                }
            }
        }
        request
    }

    /// Without a screen up to the request for the picture: folder open, verified, "abweichend"
    /// selected, no notice in the picture, the tree names only folders.
    #[test]
    fn the_shot_runs_to_the_request_without_user_state() {
        let example = Example::copy("ablauf");
        let out = example.base.join("bild.png");
        let ctx = egui::Context::default();
        let outcome: Outcome = Arc::new(Mutex::new(Err("offen".into())));
        let mut app =
            VeritasApp::for_shot(&ctx, shot_args(&example.folder, &out), Arc::clone(&outcome));
        assert_eq!(app.crash_seen, 0);
        assert_eq!(app.threads, 8);
        assert!(app.recent.len() <= 1);
        let request = run_until_request(&ctx, &mut app);
        let shot = app.shot.as_ref().unwrap();
        assert_eq!(shot.phase, Phase::Waiting, "{:?}", outcome.lock().unwrap());
        assert_eq!(ctx.pixels_per_point(), 1.5);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        assert_eq!(app.selected.as_deref(), Some("IMG_0413.jpg"));
        assert!(app.notice.is_none());
        assert!(app.status_line().contains("abweichend"));
        // The tree starts at the parent folder: no drives, no user folder.
        let labels: Vec<String> = app
            .tree
            .items()
            .iter()
            .map(|item| crate::tree::tree_label(&item.path))
            .collect();
        assert_eq!(labels, ["Fotos", "Urlaub 2026", "Drohne"]);
        app.refresh_roots(&ctx);
        app.roots_checked = -10.0;
        app.refresh_roots(&ctx);
        assert_eq!(app.tree.items().len(), 3, "the drives do not come back");

        // A picture of the wrong size is not saved.
        let small = ColorImage::filled([10, 10], egui::Color32::BLACK);
        assert!(app.save_shot(&small).unwrap_err().contains("10×10"));
        assert!(!out.exists());
        // The window delivers the picture via the callback; the next frame saves and closes.
        request.unwrap().complete(Arc::new(ColorImage::filled(
            [1440, 960],
            egui::Color32::WHITE,
        )));
        let commands = frame(&ctx, &mut app);
        assert_eq!(app.shot.as_ref().unwrap().phase, Phase::Done);
        assert_eq!(outcome.lock().unwrap().as_ref(), Ok(&out));
        assert!(out.is_file());
        assert!(commands.contains(&egui::ViewportCommand::Close));
    }

    /// Without --verify: no run, no selection (the preview has no findings), a picture
    /// nevertheless.
    #[test]
    fn without_verify_the_preview_is_shot() {
        let example = Example::copy("vorschau");
        let ctx = egui::Context::default();
        let outcome: Outcome = Arc::new(Mutex::new(Err("offen".into())));
        let mut args = shot_args(&example.folder, &example.base.join("b.png"));
        args.verify = false;
        args.scale = None;
        let mut app = VeritasApp::for_shot(&ctx, args, Arc::clone(&outcome));
        assert!(run_until_request(&ctx, &mut app).is_some());
        assert_eq!(app.shot.as_ref().unwrap().phase, Phase::Waiting);
        assert!(!app.from_run);
        assert!(app.selected.is_none());
        // Without --scale any picture size applies.
        let image = ColorImage::filled([7, 5], egui::Color32::WHITE);
        assert!(app.save_shot(&image).is_ok());
    }

    /// --verify without a checksum file ends with the reason instead of waiting.
    #[test]
    fn verify_without_md5_says_why() {
        let example = Example::copy("ohne-md5");
        std::fs::remove_file(example.folder.join("Urlaub 2026.md5")).unwrap();
        let ctx = egui::Context::default();
        let outcome: Outcome = Arc::new(Mutex::new(Err("offen".into())));
        let mut app = VeritasApp::for_shot(
            &ctx,
            shot_args(&example.folder, &example.base.join("b.png")),
            Arc::clone(&outcome),
        );
        assert!(run_until_request(&ctx, &mut app).is_none());
        assert_eq!(app.shot.as_ref().unwrap().phase, Phase::Done);
        let message = outcome.lock().unwrap().clone().unwrap_err();
        assert!(message.contains("noch keine"), "{message}");
    }

    /// If the time runs out during a run, it aborts and the window closes without a
    /// confirmation – nobody could answer it here.
    #[test]
    fn a_timeout_during_a_run_closes_without_asking() {
        let example = Example::copy("zeit");
        let ctx = egui::Context::default();
        let outcome: Outcome = Arc::new(Mutex::new(Err("offen".into())));
        let mut app = VeritasApp::for_shot(
            &ctx,
            shot_args(&example.folder, &example.base.join("b.png")),
            Arc::clone(&outcome),
        );
        let (_tx, rx) = mpsc::channel();
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.running = Some(super::super::Running {
            root: example.folder.clone(),
            progress: Arc::new(crate::progress::Progress::new()),
            cancel: Arc::clone(&cancel),
            rx,
            live: crate::list::Live::default(),
            live_due: 0.0,
        });
        if let Some(shot) = &mut app.shot {
            shot.phase = Phase::Running;
            shot.started = Instant::now()
                .checked_sub(TIMEOUT + Duration::from_secs(1))
                .unwrap();
        }
        let commands = frame(&ctx, &mut app);
        assert_eq!(app.shot.as_ref().unwrap().phase, Phase::Done);
        assert!(outcome
            .lock()
            .unwrap()
            .clone()
            .unwrap_err()
            .contains("No picture"));
        assert!(cancel.load(std::sync::atomic::Ordering::SeqCst));
        assert!(app.close_confirmed);
        assert!(!app.close_ask);
        assert!(commands.contains(&egui::ViewportCommand::Close));
    }

    /// Pointer, keys and dragged files of the user do not reach the capture; the normal window
    /// gets them.
    #[test]
    fn real_input_is_dropped() {
        let raw = || egui::RawInput {
            events: vec![egui::Event::PointerMoved(egui::pos2(10.0, 10.0))],
            hovered_files: vec![egui::HoveredFile::default()],
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let outcome: Outcome = Arc::new(Mutex::new(Err("offen".into())));
        let missing = std::env::temp_dir().join("veritas-does-not-exist-7f3b");
        let mut app =
            VeritasApp::for_shot(&ctx, shot_args(&missing, &missing.join("b.png")), outcome);
        let mut input = raw();
        eframe::App::raw_input_hook(&mut app, &ctx, &mut input);
        assert!(input.events.is_empty());
        assert!(input.hovered_files.is_empty());
        let mut normal = VeritasApp::with_settings(Settings::default());
        let mut input = raw();
        eframe::App::raw_input_hook(&mut normal, &ctx, &mut input);
        assert_eq!(input.events.len(), 1);
        assert_eq!(input.hovered_files.len(), 1);
    }

    /// The screenshot mode stores no settings (last folders, threads, crash notice) – `save`
    /// leaves the storage empty.
    #[test]
    fn the_shot_saves_no_settings() {
        #[derive(Default)]
        struct Probe(Vec<String>);
        impl eframe::Storage for Probe {
            fn get_string(&self, _key: &str) -> Option<String> {
                None
            }
            fn set_string(&mut self, key: &str, _value: String) {
                self.0.push(key.into());
            }
            fn remove_string(&mut self, key: &str) {
                self.0.push(key.into());
            }
            fn flush(&mut self) {}
        }
        let example = Example::copy("speicher");
        let ctx = egui::Context::default();
        let outcome: Outcome = Arc::new(Mutex::new(Err("offen".into())));
        let mut app = VeritasApp::for_shot(
            &ctx,
            shot_args(&example.folder, &example.base.join("b.png")),
            outcome,
        );
        let mut probe = Probe::default();
        eframe::App::save(&mut app, &mut probe);
        assert!(probe.0.is_empty(), "{:?}", probe.0);
        // Control: the normal window does store.
        let mut normal = VeritasApp::with_settings(Settings::default());
        eframe::App::save(&mut normal, &mut probe);
        assert_eq!(probe.0.len(), 1);
    }

    /// A folder that does not exist ends with an error instead of an empty picture.
    #[test]
    fn a_missing_folder_fails() {
        let ctx = egui::Context::default();
        let outcome: Outcome = Arc::new(Mutex::new(Err("offen".into())));
        let missing = std::env::temp_dir().join("veritas-does-not-exist-7f3a");
        let mut app = VeritasApp::for_shot(
            &ctx,
            shot_args(&missing, &missing.join("b.png")),
            Arc::clone(&outcome),
        );
        for _ in 0..5 {
            frame(&ctx, &mut app);
        }
        assert_eq!(app.shot.as_ref().unwrap().phase, Phase::Done);
        assert!(outcome.lock().unwrap().is_err());
    }
}

//! Screenshot mode for the README picture: `veritas --screenshot <image.png> … <folder>`.
//!
//! Opens a window of fixed size, opens the folder, verifies it on request, selects a file
//! with a finding and saves the drawn picture as PNG. No keys or clicks are simulated: the
//! state is set directly (`ui::shot`), the window discards real input. The user's settings,
//! crash log and run log stay untouched: eframe saves into its own temp folder, panic hook
//! and `veritas.log` stay off.

use std::ffi::OsString;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use eframe::egui::{self, ColorImage};
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

use crate::i18n::Lang;
use crate::ui::{VeritasApp, MIN_SIZE};

/// Window size in points when `--size` is missing.
pub const DEFAULT_SIZE: [f32; 2] = [1180.0, 760.0];

/// Short help for wrong arguments.
pub const USAGE: &str = "Usage: veritas [folder or checksum file]\n       veritas --screenshot \
                         <image.png> [--theme light|dark] [--lang de|en|fr|es|it] [--verify] \
                         [--size WxH] [--scale F] <folder>";

/// What the command line asks for.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// The normal window, optionally with a folder or a file.
    Window(Option<PathBuf>),
    /// Take a picture and exit.
    Screenshot(Args),
}

/// Settings of a capture.
#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    /// Target of the PNG.
    pub out: PathBuf,
    pub theme: egui::Theme,
    /// Interface language only for this capture (nothing is stored); German if not given, like
    /// the README picture.
    pub lang: Lang,
    /// Verify before the capture (F5), so that the list shows findings.
    pub verify: bool,
    /// Inner window size in points.
    pub size: [f32; 2],
    /// Pixels per point; if not given, the scale of the screen applies. If given, the picture has
    /// exactly `size × scale` pixels, independent of the screen.
    pub scale: Option<f32>,
    /// The folder the picture shows.
    pub folder: PathBuf,
}

impl Args {
    /// The expected picture size in pixels, if `scale` fixes it.
    pub fn pixels(&self) -> Option<[usize; 2]> {
        let scale = self.scale?;
        Some([
            (self.size[0] * scale).round() as usize,
            (self.size[1] * scale).round() as usize,
        ])
    }
}

/// Reads the arguments without the program name. Without `--screenshot`, the first argument
/// counts as a folder or file, as before (`Exec=veritas %f`).
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let args: Vec<OsString> = args.into_iter().collect();
    if !args.iter().any(|arg| arg == "--screenshot") {
        return Ok(Command::Window(args.into_iter().next().map(PathBuf::from)));
    }
    let mut out = None;
    let mut theme = egui::Theme::Light;
    let mut lang = Lang::De;
    let mut verify = false;
    let mut size = DEFAULT_SIZE;
    let mut scale = None;
    let mut folder = None;
    let mut rest = args.into_iter();
    while let Some(arg) = rest.next() {
        let text = arg.to_string_lossy().into_owned();
        let mut value = |name: &str| rest.next().ok_or_else(|| format!("{name} needs a value."));
        match text.as_str() {
            "--screenshot" => out = Some(PathBuf::from(value("--screenshot")?)),
            "--theme" => {
                theme = match value("--theme")?.to_string_lossy().as_ref() {
                    // `hell` and `dunkel` are the former German values, still accepted.
                    "light" | "hell" => egui::Theme::Light,
                    "dark" | "dunkel" => egui::Theme::Dark,
                    other => {
                        return Err(format!("--theme: \"{other}\" – allowed are light, dark."))
                    }
                }
            }
            "--lang" => {
                let code = value("--lang")?.to_string_lossy().into_owned();
                lang = Lang::from_code(&code)
                    .filter(|known| known.code() == code)
                    .ok_or_else(|| {
                        format!("--lang: \"{code}\" – allowed are de, en, fr, es, it.")
                    })?;
            }
            "--verify" => verify = true,
            "--size" => size = parse_size(&value("--size")?.to_string_lossy())?,
            "--scale" => scale = Some(parse_scale(&value("--scale")?.to_string_lossy())?),
            flag if flag.starts_with("--") => return Err(format!("Unknown option {flag}.")),
            _ if folder.is_some() => return Err("Only one folder, please.".into()),
            _ => folder = Some(PathBuf::from(arg)),
        }
    }
    let out = out.ok_or("--screenshot needs an output file.")?;
    if out
        .extension()
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("png"))
    {
        return Err("The output file must end in .png.".into());
    }
    let folder = folder.ok_or("Which folder? It is the last argument.")?;
    Ok(Command::Screenshot(Args {
        out,
        theme,
        lang,
        verify,
        size,
        scale,
        folder,
    }))
}

/// `1180x760`: at least as large as the smallest window, at most 4000 points.
fn parse_size(text: &str) -> Result<[f32; 2], String> {
    let invalid = || format!("--size: \"{text}\" – expected WxH, e.g. 1180x760.");
    let (w, h) = text.split_once(['x', 'X', '×']).ok_or_else(invalid)?;
    let w: u32 = w.trim().parse().map_err(|_| invalid())?;
    let h: u32 = h.trim().parse().map_err(|_| invalid())?;
    let size = [w as f32, h as f32];
    if size[0] < MIN_SIZE[0] || size[1] < MIN_SIZE[1] || w > 4000 || h > 4000 {
        return Err(format!(
            "--size: {w}x{h} – at least {}x{}, at most 4000x4000.",
            MIN_SIZE[0], MIN_SIZE[1]
        ));
    }
    Ok(size)
}

/// `1.5`: between 0.5 and 4.
fn parse_scale(text: &str) -> Result<f32, String> {
    match text.trim().replace(',', ".").parse::<f32>() {
        Ok(scale) if (0.5..=4.0).contains(&scale) => Ok(scale),
        _ => Err(format!(
            "--scale: \"{text}\" – expected a number from 0.5 to 4."
        )),
    }
}

/// Saves the picture as PNG (RGB, best compression). The window is opaque; the alpha channel
/// would contribute nothing and cost space.
pub fn write_png(path: &Path, image: &ColorImage) -> Result<(), String> {
    let [width, height] = image.size;
    let fail = |err: &dyn std::fmt::Display| format!("\"{}\": {err}", path.display());
    let (w, h) = match (u32::try_from(width), u32::try_from(height)) {
        (Ok(w), Ok(h)) if w > 0 && h > 0 => (w, h),
        _ => return Err(fail(&"empty image")),
    };
    let mut rgb = Vec::with_capacity(width * height * 3);
    for pixel in &image.pixels {
        rgb.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
    }
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|err| fail(&err))?;
    }
    let mut writer = BufWriter::new(File::create(path).map_err(|err| fail(&err))?);
    PngEncoder::new_with_quality(&mut writer, CompressionType::Best, FilterType::Adaptive)
        .write_image(&rgb, w, h, ExtendedColorType::Rgb8)
        .map_err(|err| fail(&err))?;
    writer.flush().map_err(|err| fail(&err))
}

/// Result of the capture, shared between window and `run`.
pub type Outcome = Arc<Mutex<Result<PathBuf, String>>>;

/// Takes the picture; exit code 0 only if it is written. Messages go to stderr.
pub fn run(args: Args) -> ExitCode {
    if !args.folder.is_dir() {
        eprintln!("veritas: \"{}\" is not a folder.", args.folder.display());
        return ExitCode::FAILURE;
    }
    // Own file for eframe (theme, window position, egui memory): the user's settings stay as
    // they are. eframe writes exactly this file (pp.ron content).
    let storage =
        std::env::temp_dir().join(format!("veritas-screenshot-{}.ron", std::process::id()));
    let outcome: Outcome = Arc::new(Mutex::new(Err(
        "The window was closed before the picture was taken.".into(),
    )));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(args.size)
            .with_resizable(false)
            // Does not take the focus from the user.
            .with_active(false),
        persist_window: false,
        persistence_path: Some(storage.clone()),
        ..Default::default()
    };
    let shared = Arc::clone(&outcome);
    let started = eframe::run_native(
        "Veritas",
        options,
        Box::new(move |cc| Ok(Box::new(VeritasApp::screenshot(cc, args, shared)))),
    );
    let _ = std::fs::remove_file(&storage);
    if let Err(err) = started {
        eprintln!("veritas: The window could not be opened: {err}");
        return ExitCode::FAILURE;
    }
    let result = outcome
        .lock()
        .map(|result| result.clone())
        .unwrap_or_else(|err| err.into_inner().clone());
    match result {
        Ok(path) => {
            eprintln!("Picture saved: {}", path.display());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("veritas: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Command, String> {
        parse(list.iter().map(OsString::from))
    }

    #[test]
    fn without_the_flag_the_first_argument_opens() {
        assert_eq!(args(&[]), Ok(Command::Window(None)));
        assert_eq!(
            args(&["D:/Fotos", "egal"]),
            Ok(Command::Window(Some(PathBuf::from("D:/Fotos"))))
        );
    }

    #[test]
    fn a_full_screenshot_call() {
        let Ok(Command::Screenshot(shot)) = args(&[
            "--screenshot",
            "bild.png",
            "--theme",
            "dark",
            "--lang",
            "fr",
            "--verify",
            "--size",
            "960x640",
            "--scale",
            "1,5",
            "Urlaub 2026",
        ]) else {
            panic!("not a screenshot call");
        };
        assert_eq!(shot.out, PathBuf::from("bild.png"));
        assert_eq!(shot.theme, egui::Theme::Dark);
        assert_eq!(shot.lang, Lang::Fr);
        assert!(shot.verify);
        assert_eq!(shot.size, [960.0, 640.0]);
        assert_eq!(shot.scale, Some(1.5));
        assert_eq!(shot.pixels(), Some([1440, 960]));
        assert_eq!(shot.folder, PathBuf::from("Urlaub 2026"));
    }

    #[test]
    fn defaults_and_order() {
        let Ok(Command::Screenshot(shot)) = args(&["Ordner", "--screenshot", "a.PNG"]) else {
            panic!("not a screenshot call");
        };
        assert_eq!(shot.theme, egui::Theme::Light);
        assert_eq!(shot.lang, Lang::De);
        assert!(!shot.verify);
        assert_eq!(shot.size, DEFAULT_SIZE);
        assert_eq!(shot.scale, None);
        assert_eq!(shot.pixels(), None);
    }

    #[test]
    fn the_theme_takes_english_and_german_names() {
        for (name, theme) in [
            ("light", egui::Theme::Light),
            ("hell", egui::Theme::Light),
            ("dark", egui::Theme::Dark),
            ("dunkel", egui::Theme::Dark),
        ] {
            let Ok(Command::Screenshot(shot)) =
                args(&["--screenshot", "a.png", "--theme", name, "x"])
            else {
                panic!("{name}: not a screenshot call");
            };
            assert_eq!(shot.theme, theme, "{name}");
        }
    }

    #[test]
    fn invalid_calls_say_why() {
        let cases: &[(&[&str], &str)] = &[
            (&["--screenshot"], "needs a value"),
            (&["--screenshot", "a.png"], "Which folder"),
            (&["--screenshot", "a.jpg", "x"], ".png"),
            (
                &["--screenshot", "a.png", "--theme", "blau", "x"],
                "light, dark",
            ),
            (
                &["--screenshot", "a.png", "--size", "100x100", "x"],
                "at least",
            ),
            (&["--screenshot", "a.png", "--size", "breit", "x"], "WxH"),
            (
                &["--screenshot", "a.png", "--size", "5000x900", "x"],
                "at most",
            ),
            (&["--screenshot", "a.png", "--scale", "9", "x"], "0.5 to 4"),
            (
                &["--screenshot", "a.png", "--scale", "viel", "x"],
                "0.5 to 4",
            ),
            (
                &["--screenshot", "a.png", "--laut", "x"],
                "Unknown option --laut",
            ),
            (&["--screenshot", "a.png", "x", "y"], "Only one folder"),
            (
                &["--screenshot", "a.png", "--lang", "pt", "x"],
                "de, en, fr, es, it",
            ),
            (
                &["--screenshot", "a.png", "--lang", "de-DE", "x"],
                "de, en, fr, es, it",
            ),
        ];
        for (list, why) in cases {
            let err = args(list).unwrap_err();
            assert!(err.contains(why), "{list:?}: {err}");
        }
    }

    #[test]
    fn a_missing_folder_fails_before_any_window() {
        let missing = std::env::temp_dir().join("veritas-does-not-exist-9c1d");
        let code = run(Args {
            out: missing.join("b.png"),
            theme: egui::Theme::Light,
            lang: Lang::De,
            verify: false,
            size: DEFAULT_SIZE,
            scale: None,
            folder: missing,
        });
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::FAILURE));
    }

    #[test]
    fn png_has_the_pixels_of_the_image() {
        let dir = std::env::temp_dir().join(format!("veritas-png-{}", std::process::id()));
        let path = dir.join("unter").join("bild.png");
        let mut image = ColorImage::filled([3, 2], egui::Color32::from_rgb(10, 20, 30));
        image.pixels[5] = egui::Color32::from_rgb(200, 100, 50);
        write_png(&path, &image).unwrap();
        let read = image::load_from_memory(&std::fs::read(&path).unwrap())
            .unwrap()
            .to_rgb8();
        assert_eq!(read.dimensions(), (3, 2));
        assert_eq!(read.get_pixel(0, 0).0, [10, 20, 30]);
        assert_eq!(read.get_pixel(2, 1).0, [200, 100, 50]);
        assert!(write_png(&path, &ColorImage::filled([0, 0], egui::Color32::BLACK)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

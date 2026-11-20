//! Fonts: interface font, monospace, the Lucide icons and fallback fonts.
//!
//! - **Windows:** Segoe UI, Segoe UI Semibold and Consolas come from the system. None of them
//!   is embedded; that saves about 2 MB in the EXE. If they are missing, Arial and Tahoma
//!   or Lucida Console and Courier New follow. As the last fallback egui brings its own fonts
//!   (Cargo feature `default_fonts`, enabled only on Windows), so that no family is empty
//!   and text always stays visible.
//! - **Linux:** Noto Sans and DejaVu Sans Mono from the system, otherwise the embedded files.
//!   `default_fonts` is off there; the embedded fonts are the last fallback.
//! - **Lucide:** the icon subset is always embedded.
//! - **CJK:** file names with Chinese, Japanese or Korean characters are drawn by a system
//!   font that is loaded only at the first such character (`msyh.ttc` is 19 MB, which
//!   should cost nothing at start-up).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, FallbackRequest, FontData, FontDefinitions, FontFamily, FontInsert, FontTweak,
};

use crate::theme::semibold;

// Embedded only where they are the last fallback (Linux); tests check this case on
// Windows as well.
#[cfg(any(not(windows), test))]
const NOTO_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
#[cfg(any(not(windows), test))]
const NOTO_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/NotoSans-SemiBold.ttf");
#[cfg(any(not(windows), test))]
const DEJAVU_MONO: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono.ttf");
/// The icon subset (Lucide), always embedded.
pub const LUCIDE: &[u8] = include_bytes!("../assets/fonts/lucide-subset.ttf");

/// The three UI fonts. `None` if none was found: the setup then leaves the
/// family with the fonts egui brings along instead of entering a name without data.
#[derive(Default)]
struct Faces {
    regular: Option<FontData>,
    semibold: Option<FontData>,
    mono: Option<FontData>,
}

/// The fonts for `egui::Context::set_fonts`, from the system fonts.
pub fn definitions() -> FontDefinitions {
    build(FontDefinitions::default(), system_faces())
}

/// Assembles the families. The chain of each family starts with its own font, then
/// Lucide, then whatever `base` brings by itself (`FontDefinitions::default()`: depending
/// on the features nothing, with `default_fonts` Ubuntu-Light, Hack and egui-icons). Built-in
/// fonts are never addressed by name, so that the setup is the same with and without
/// `default_fonts`; the tests pass in `FontDefinitions::empty()` to simulate Linux on
/// Windows.
fn build(base: FontDefinitions, faces: Faces) -> FontDefinitions {
    let mut fonts = base;
    fonts.font_data.insert(
        "lucide".to_owned(),
        Arc::new(FontData::from_static(LUCIDE).tweak(FontTweak {
            scale: 1.0,
            y_offset_factor: 0.0,
            y_offset: 1.0,
            ..FontTweak::default()
        })),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "lucide".to_owned());
    }
    if let Some(data) = faces.regular {
        prepend(&mut fonts, FontFamily::Proportional, "ui-regular", data);
    }
    if let Some(data) = faces.mono {
        prepend(&mut fonts, FontFamily::Monospace, "ui-mono", data);
    }
    // The monospace fonts (DejaVu Sans Mono, Hack, Consolas) have arrows, math signs and
    // symbols that the embedded Noto Sans and Ubuntu-Light lack: they sit behind the UI
    // font, without their names appearing here.
    let mono = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let tail = fonts.families.entry(FontFamily::Proportional).or_default();
    for name in mono {
        if !tail.contains(&name) {
            tail.push(name);
        }
    }
    // Semibold first, then the same chain as the regular font, but without it: what
    // semibold lacks falls back to Lucide and egui's fonts, not to another weight. If semibold
    // is missing entirely, the regular font also carries the headings.
    let proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut bold = Vec::new();
    if let Some(data) = faces.semibold {
        fonts
            .font_data
            .insert("ui-semibold".to_owned(), Arc::new(data));
        bold.push("ui-semibold".to_owned());
        bold.extend(proportional.into_iter().filter(|name| name != "ui-regular"));
    } else {
        bold.extend(proportional);
    }
    fonts.families.insert(semibold(), bold);
    fonts
}

fn prepend(fonts: &mut FontDefinitions, family: FontFamily, name: &str, data: FontData) {
    fonts.font_data.insert(name.to_owned(), Arc::new(data));
    fonts
        .families
        .entry(family)
        .or_default()
        .insert(0, name.to_owned());
}

/// Reads a font file; `index` selects the face in a `.ttc`.
fn read_face(path: &Path, index: u32) -> Option<FontData> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() <= 100 {
        return None;
    }
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    Some(data)
}

/// The first readable file in the list.
fn read_first(paths: &[PathBuf]) -> Option<FontData> {
    paths.iter().find_map(|path| read_face(path, 0))
}

#[cfg(windows)]
fn system_faces() -> Faces {
    let dir = windows_fonts_dir();
    let first = |names: &[&str]| {
        read_first(
            &names
                .iter()
                .map(|name| dir.join(name))
                .collect::<Vec<PathBuf>>(),
        )
    };
    Faces {
        regular: first(&["segoeui.ttf", "arial.ttf", "tahoma.ttf"]),
        semibold: first(&["seguisb.ttf", "arialbd.ttf", "tahomabd.ttf"]),
        mono: first(&["consola.ttf", "lucon.ttf", "cour.ttf"]),
    }
}

#[cfg(not(windows))]
fn system_faces() -> Faces {
    let embedded = embedded_faces();
    let paths = |list: &[&str]| list.iter().map(PathBuf::from).collect::<Vec<PathBuf>>();
    Faces {
        regular: read_first(&paths(&[
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/opentype/noto/NotoSans-Regular.ttf",
        ]))
        // Not DejaVu Sans from the system: it runs much wider than Noto Sans, and the space tests
        // of the five languages measure with Noto. Without Noto on the system, the embedded font
        // applies.
        .or(embedded.regular),
        semibold: read_first(&paths(&[
            "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
            "/usr/share/fonts/opentype/noto/NotoSans-SemiBold.ttf",
        ]))
        .or(embedded.semibold),
        mono: read_first(&paths(&[
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        ]))
        .or(embedded.mono),
    }
}

/// The font families as on Linux (only the embedded fonts) – for tests that
/// measure widths with Noto Sans on Windows too.
#[cfg(test)]
pub(crate) fn embedded_definitions() -> FontDefinitions {
    build(FontDefinitions::empty(), embedded_faces())
}

/// The embedded fonts, the last fallback on Linux.
#[cfg(any(not(windows), test))]
fn embedded_faces() -> Faces {
    Faces {
        regular: Some(FontData::from_static(NOTO_REGULAR)),
        semibold: Some(FontData::from_static(NOTO_SEMIBOLD)),
        mono: Some(FontData::from_static(DEJAVU_MONO)),
    }
}

#[cfg(windows)]
fn windows_fonts_dir() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
        .join("Fonts")
}

/// Writing system of a character that the UI fonts do not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cjk {
    Hangul,
    Kana,
    Han,
}

fn cjk(ch: char) -> Option<Cjk> {
    match u32::from(ch) {
        0x1100..=0x11FF | 0x3130..=0x318F | 0xA960..=0xA97F | 0xAC00..=0xD7FF | 0xFFA0..=0xFFDC => {
            Some(Cjk::Hangul)
        }
        0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9F => Some(Cjk::Kana),
        0x2E80..=0x2FDF
        | 0x3000..=0x303F
        | 0x3100..=0x312F
        | 0x3190..=0x31EF
        | 0x3200..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFFEF
        | 0x2_0000..=0x3_FFFF => Some(Cjk::Han),
        _ => None,
    }
}

/// Candidates per writing system as (file, face in the `.ttc`); the first one present wins.
/// On Windows these are file names in the font folder, on Linux absolute paths.
#[cfg(windows)]
fn candidates(script: Cjk) -> &'static [(&'static str, u32)] {
    match script {
        Cjk::Hangul => &[("malgun.ttf", 0)],
        Cjk::Kana => &[("YuGothM.ttc", 0), ("meiryo.ttc", 0), ("msgothic.ttc", 0)],
        Cjk::Han => &[("msyh.ttc", 0), ("YuGothM.ttc", 0), ("simsun.ttc", 0)],
    }
}

#[cfg(not(windows))]
fn candidates(script: Cjk) -> &'static [(&'static str, u32)] {
    // Noto CJK lies in different folders depending on the distribution; face 0 also contains
    // Hangul.
    match script {
        Cjk::Hangul | Cjk::Kana | Cjk::Han => &[
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 0),
            (
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                0,
            ),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
            (
                "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
                0,
            ),
        ],
    }
}

#[cfg(windows)]
fn candidate_path(file: &str) -> PathBuf {
    windows_fonts_dir().join(file)
}

#[cfg(not(windows))]
fn candidate_path(file: &str) -> PathBuf {
    PathBuf::from(file)
}

/// Registers a fallback font with egui that is loaded from the system for CJK characters,
/// but only when such a character is to be drawn for the first time. Each file is read
/// at most once. If none is found, the box for the character remains.
pub fn add_fallback_provider(ctx: &egui::Context) {
    let loaded: Mutex<HashMap<&'static str, Option<FontData>>> = Mutex::default();
    ctx.add_font_provider(Arc::new(move |request: &FallbackRequest<'_>| {
        let script = cjk(request.cluster.chars().next()?)?;
        let mut loaded = loaded.lock().ok()?;
        for &(file, index) in candidates(script) {
            let data = loaded
                .entry(file)
                .or_insert_with(|| read_face(&candidate_path(file), index));
            if let Some(data) = data {
                return Some(FontInsert::new(file, data.clone(), Vec::new()));
            }
        }
        None
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::FontId;

    /// Characters a translation will need soon, even if it is not there yet: Spanish,
    /// French (guillemets, non-breaking spaces before `:` and in numbers), Italian.
    const COMING: [char; 12] = [
        'ñ', 'ç', 'œ', '«', '»', '¿', '¡', 'é', 'è', 'à', '\u{a0}', '\u{202f}',
    ];

    /// All characters beyond ASCII that appear in the sources before the tests (interface texts
    /// in all languages under `src/i18n/`, messages), also as an escape `\u{…}`,
    /// plus `COMING`. Comments do not count. `fonts.rs` itself is left out
    /// (CJK examples in the test).
    fn ui_characters() -> Vec<char> {
        let mut chars = COMING.to_vec();
        let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        let mut files = 0;
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "rs")
                    || path.file_name().is_some_and(|name| name == "fonts.rs")
                {
                    continue;
                }
                files += 1;
                for line in std::fs::read_to_string(&path).unwrap().lines() {
                    let line = line.trim();
                    if line == "#[cfg(test)]" {
                        break;
                    }
                    if line.starts_with("//") {
                        continue;
                    }
                    chars.extend(line.chars().filter(|ch| !ch.is_ascii()));
                    chars.extend(escapes(line));
                }
            }
        }
        // `src/`, `src/i18n/` (seven files) and `src/ui/`.
        assert!(files > 20, "only {files} source files found");
        chars.sort_unstable();
        chars.dedup();
        chars
    }

    /// Characters from `\u{…}` in the strings of a line. Character literals (`'\u{81}'`,
    /// the table for Windows-1252 in `checksum.rs`) are not interface text.
    fn escapes(line: &str) -> Vec<char> {
        line.split('"')
            .skip(1)
            .step_by(2)
            .flat_map(|text| text.split("\\u{").skip(1))
            .filter_map(|rest| rest.split('}').next())
            .filter_map(|hex| u32::from_str_radix(hex, 16).ok())
            .filter_map(char::from_u32)
            .collect()
    }

    #[test]
    fn escapes_are_read() {
        assert_eq!(
            escapes(r#"x("a\u{a0}b", "\u{202f}")"#),
            vec!['\u{a0}', '\u{202f}']
        );
        assert!(escapes(r"'\u{81}', '\u{8d}'").is_empty());
        assert!(escapes(r#""kein Escape""#).is_empty());
    }

    fn context_with(fonts: FontDefinitions) -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        ctx
    }

    fn missing(ctx: &egui::Context, family: &FontFamily, chars: &[char]) -> Vec<String> {
        let font = FontId::new(16.0, family.clone());
        chars
            .iter()
            .filter(|ch| !ctx.fonts_mut(|fonts| fonts.has_glyph(&font, **ch)))
            .map(|ch| format!("U+{:04X} {ch}", u32::from(*ch)))
            .collect()
    }

    /// The worst states, in which no system font folder yields anything.
    fn worst_cases() -> Vec<(&'static str, FontDefinitions)> {
        let mut cases = vec![
            // Linux: embedded fonts, `default_fonts` off (`empty()` is the default there).
            (
                "eingebettet ohne egui-Schriften",
                build(FontDefinitions::empty(), embedded_faces()),
            ),
            (
                "eingebettet mit egui-Schriften",
                build(FontDefinitions::default(), embedded_faces()),
            ),
        ];
        // Windows without Segoe UI, Arial, Tahoma, Consolas: only egui's fonts. On
        // Linux this state is unreachable and the default is empty.
        if cfg!(windows) {
            cases.push((
                "Windows ohne Systemschriften",
                build(FontDefinitions::default(), Faces::default()),
            ));
        }
        cases
    }

    /// A family without a font draws nothing (“No text will be visible”) and egui's default
    /// is a box instead of an error. Therefore: in every reachable state
    /// each family has fonts, and every name in a chain has data.
    #[test]
    fn no_family_is_ever_empty_or_names_a_missing_font() {
        let mut states = worst_cases();
        states.push((
            "Systemschriften",
            build(FontDefinitions::default(), system_faces()),
        ));
        for (state, fonts) in states {
            for family in [FontFamily::Proportional, FontFamily::Monospace, semibold()] {
                let chain = &fonts.families[&family];
                assert!(!chain.is_empty(), "{state}: {family:?} is empty");
                for name in chain {
                    assert!(
                        fonts.font_data.contains_key(name),
                        "{state}: {family:?} nennt {name} ohne Daten"
                    );
                }
            }
        }
    }

    /// If semibold is missing, the regular font carries the headings.
    #[test]
    fn bold_falls_back_to_the_regular_face_when_semibold_is_missing() {
        let mut faces = embedded_faces();
        faces.semibold = None;
        let fonts = build(FontDefinitions::empty(), faces);
        assert!(fonts.families[&semibold()].contains(&"ui-regular".to_owned()));
        assert!(!fonts.font_data.contains_key("ui-semibold"));
        let fonts = build(FontDefinitions::empty(), embedded_faces());
        assert_eq!(fonts.families[&semibold()][0], "ui-semibold");
        assert!(!fonts.families[&semibold()].contains(&"ui-regular".to_owned()));
    }

    /// egui's default draws a box where a font is missing instead of failing;
    /// green tests say nothing about that. This test checks every character of the
    /// interface against the fallback chain without any system font.
    #[test]
    fn every_ui_character_has_a_glyph_without_system_fonts() {
        let chars = ui_characters();
        assert!(chars.contains(&'ä') && chars.contains(&'→'), "{chars:?}");
        assert!(
            chars.contains(&'ñ') && chars.contains(&'\u{202f}'),
            "{chars:?}"
        );
        let mut all_gaps = Vec::new();
        for (state, fonts) in worst_cases() {
            let ctx = context_with(fonts);
            for family in [FontFamily::Proportional, FontFamily::Monospace, semibold()] {
                let gaps = missing(&ctx, &family, &chars);
                if !gaps.is_empty() {
                    all_gaps.push(format!("{state}, {family:?}: {gaps:?}"));
                }
            }
        }
        assert!(
            all_gaps.is_empty(),
            "no glyph for {all_gaps:#?}. Add a font or replace the character"
        );
    }

    #[test]
    fn scripts_are_told_apart() {
        assert_eq!(cjk('한'), Some(Cjk::Hangul));
        assert_eq!(cjk('の'), Some(Cjk::Kana));
        assert_eq!(cjk('カ'), Some(Cjk::Kana));
        assert_eq!(cjk('日'), Some(Cjk::Han));
        assert_eq!(cjk('，'), Some(Cjk::Han));
        for ch in ['a', 'ä', 'ß', '…', '→', '€', 'Ж', '\u{E06D}'] {
            assert_eq!(cjk(ch), None, "{ch}");
        }
        for script in [Cjk::Hangul, Cjk::Kana, Cjk::Han] {
            assert!(!candidates(script).is_empty());
        }
    }

    /// With a CJK system font (not present on every machine) a file name like
    /// “日本語” gets glyphs, without anything having been loaded at startup.
    #[test]
    fn cjk_file_names_find_a_system_font_if_there_is_one() {
        // In the order of 日 の 한.
        let present = [Cjk::Han, Cjk::Kana, Cjk::Hangul]
            .into_iter()
            .map(|script| {
                candidates(script)
                    .iter()
                    .any(|(file, _)| candidate_path(file).is_file())
            })
            .collect::<Vec<bool>>();
        let ctx = egui::Context::default();
        ctx.set_fonts(build(FontDefinitions::empty(), embedded_faces()));
        add_fallback_provider(&ctx);
        let mut drawn = Vec::new();
        for ch in ['日', 'の', '한'] {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.label(ch.to_string());
            });
            output.textures_delta.clear();
            drawn.push(ctx.fonts_mut(|fonts| {
                fonts.has_glyph(&FontId::new(16.0, FontFamily::Proportional), ch)
            }));
        }
        // Without a file it stays at the box. With a file there is the glyph for Han, which every
        // candidate has. The Windows fonts and Noto CJK have Kana and Hangul, a sparse
        // fallback font on Linux may not: there only the first direction holds (no file, no glyph).
        assert_eq!(drawn[0], present[0], "glyph for 日");
        assert!(!drawn[1] || present[1], "glyph for の without font file");
        assert!(!drawn[2] || present[2], "glyph for 한 without font file");
    }
}

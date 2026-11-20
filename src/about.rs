//! “Über Veritas” (R8): what Veritas is, version, license and source code, where to report
//! bugs or write feature requests, and the third-party components. `F1` or the “i” button
//! show it as a dialog card over the window; `Esc`, `F1` and a click outside close it. The links
//! only open the browser; Veritas sends nothing.

use eframe::egui::{self, Align, Grid, Label, Layout, Link, RichText, ScrollArea, Vec2};

use crate::i18n;
use crate::theme::{self, icon, palette, space, text};

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const COPYRIGHT: &str = concat!("© 2026 ", env!("CARGO_PKG_AUTHORS"));
/// Width of the card; narrower if the window requires it.
const CARD_W: f32 = 560.0;

/// “Über Veritas” as a dialog card over the whole window. Returns `true` when it should
/// be closed.
pub fn overlay(ctx: &egui::Context) -> bool {
    let p = palette(ctx);
    let mut close = false;
    let screen = ctx.content_rect();
    let width = (screen.width() - 2.0 * space::S6).clamp(280.0, CARD_W);
    let max_height = (screen.height() - 2.0 * space::S6).max(200.0);
    let (outside, ()) = theme::card(ctx, "veritas-about", |ui| {
        let inner_w = width - 2.0 * space::S6;
        ui.set_width(inner_w);
        ui.set_max_height(max_height - 2.0 * space::S6);
        header(ui, &mut close);
        ui.add_space(space::S4);
        ScrollArea::vertical()
            .auto_shrink([false, true])
            .max_height(max_height - 2.0 * space::S6 - 90.0)
            .show(ui, |ui| {
                ui.set_width(inner_w);
                about(ui);
            });
        ui.add_space(space::S3);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(i18n::t().about_close)
                    .size(text::SMALL)
                    .color(p.muted),
            );
        });
    });
    close || outside
}

/// “Über Veritas” and the × on the right.
fn header(ui: &mut egui::Ui, close: &mut bool) {
    let p = palette(ui.ctx());
    let t = i18n::t();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(space::S3, 0.0);
        ui.label(
            RichText::new(t.about_title)
                .size(text::VALUE)
                .family(theme::semibold())
                .color(p.text),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::icon_button(ui, icon::X, theme::CLOSE)
                .on_hover_text(t.close_esc)
                .clicked()
            {
                *close = true;
            }
        });
    });
}

/// What it is, version, copyright, license and source code; where to report bugs or make
/// requests – a GitHub issue with version and system, never a path – and the third-party
/// components.
fn about(ui: &mut egui::Ui) {
    let p = palette(ui.ctx());
    let t = i18n::t();
    let paragraph = |ui: &mut egui::Ui, body: &str| {
        ui.add(Label::new(RichText::new(body).size(text::BODY).color(p.text)).wrap());
    };
    let heading = |ui: &mut egui::Ui, title: &str| {
        ui.add_space(space::S4);
        theme::overline(ui, title);
    };
    ui.spacing_mut().item_spacing.y = space::S2;
    paragraph(ui, t.about_text);
    ui.add_space(space::S1);
    Grid::new("veritas-about-facts")
        .num_columns(2)
        .spacing(Vec2::new(space::S6, space::S2))
        .show(ui, |ui| {
            for (label, value) in [
                (t.about_version, VERSION),
                (t.about_copyright, COPYRIGHT),
                (t.about_license, env!("CARGO_PKG_LICENSE")),
            ] {
                ui.label(RichText::new(label).size(text::BODY).color(p.muted));
                ui.label(RichText::new(value).size(text::BODY).color(p.text));
                ui.end_row();
            }
            ui.label(
                RichText::new(t.about_source)
                    .size(text::BODY)
                    .color(p.muted),
            );
            link(ui, REPOSITORY.trim_start_matches("https://"), REPOSITORY);
            ui.end_row();
        });

    let system = system_name();
    heading(ui, t.report_title);
    paragraph(ui, t.report_text);
    link(
        ui,
        t.report_link,
        &issue_url("bug_report.yml", VERSION, &system),
    );

    heading(ui, t.wish_title);
    paragraph(ui, t.wish_text);
    link(
        ui,
        t.wish_link,
        &issue_url("feature_request.yml", VERSION, &system),
    );

    heading(ui, t.third_title);
    paragraph(ui, t.third_text);
    link(ui, t.third_link, &third_party_url());
}

/// A link in `accent-strong` (interaction; in dark, `accent` would have only
/// 4.4:1 on `surface-raised`) that opens in the browser. egui opens nothing itself: the
/// eframe feature `links` is off (it would pull in `webbrowser`, `url` and the ICU data),
/// `open` starts the browser, as on a double-click on a file.
fn link(ui: &mut egui::Ui, label: &str, url: &str) {
    let response = ui.add(Link::new(
        RichText::new(label)
            .size(text::BODY)
            .color(palette(ui.ctx()).accent_strong),
    ));
    if response.clicked() {
        if let Err(err) = open::that_detached(url) {
            log::warn!("Link not opened ({url}): {err}");
        }
    }
}

/// A new GitHub issue from one of the repository's forms, with version and system
/// filled in (the field IDs of the forms).
fn issue_url(template: &str, version: &str, system: &str) -> String {
    format!(
        "{REPOSITORY}/issues/new?template={template}&version={}&os={}",
        encode(version),
        encode(system)
    )
}

/// The third-party components. The state on `main`, because the tag of a version does not
/// always exist yet while a release is under way, and a link to it would lead nowhere.
fn third_party_url() -> String {
    format!("{REPOSITORY}/blob/main/THIRD_PARTY.md")
}

/// Percent-encodes everything except the unreserved characters of a URL.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// Operating system and architecture for the issue, e.g. “Windows (x86_64)” or
/// “Ubuntu 24.04.1 LTS (x86_64)”.
fn system_name() -> String {
    let arch = std::env::consts::ARCH;
    let os = match std::env::consts::OS {
        "windows" => "Windows".to_string(),
        "macos" => "macOS".to_string(),
        "linux" => std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("PRETTY_NAME="))
                    .map(|name| name.trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "Linux".to_string()),
        other => other.to_string(),
    };
    format!("{os} ({arch})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_carries_version_and_system() {
        assert_eq!(
            issue_url("bug_report.yml", "0.6.0", "Windows (x86_64)"),
            "https://github.com/fly2nbc-oss/Veritas/issues/new?template=bug_report.yml\
             &version=0.6.0&os=Windows%20%28x86_64%29"
        );
        assert_eq!(
            third_party_url(),
            "https://github.com/fly2nbc-oss/Veritas/blob/main/THIRD_PARTY.md"
        );
        assert!(system_name().contains(std::env::consts::ARCH));
    }

    /// The address reaches the browser unchanged: `open` passes it on as an argument or
    /// environment variable, never through a shell (`&` and `%28` stay intact). Nothing is started
    /// here, `commands` only builds the commands.
    #[test]
    fn the_browser_gets_the_url_unchanged() {
        let url = issue_url("bug_report.yml", "0.7.2", "Windows (x86_64)");
        let commands = open::commands(&url);
        assert!(!commands.is_empty());
        for command in commands {
            let in_args = command
                .get_args()
                .any(|arg| arg == std::ffi::OsStr::new(&url));
            let in_env = command
                .get_envs()
                .any(|(_, value)| value == Some(std::ffi::OsStr::new(&url)));
            assert!(in_args || in_env, "{command:?}");
        }
    }

    /// The forms the links point to are in the repository and have the fields
    /// `version` and `os`.
    #[test]
    fn the_issue_forms_exist_with_the_prefilled_fields() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/ISSUE_TEMPLATE");
        let bug = std::fs::read_to_string(dir.join("bug_report.yml")).unwrap();
        assert!(bug.contains("id: version") && bug.contains("id: os"));
        let wish = std::fs::read_to_string(dir.join("feature_request.yml")).unwrap();
        assert!(wish.contains("id: version"));
    }
}

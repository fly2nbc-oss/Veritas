//! Semantic tokens and components from UI design system v2 (2026-10-14).
//! Components use only these tokens, no hex values.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Id, Label, Layout,
    Margin, Order, Painter, Pos2, Rect, RichText, Sense, Stroke, TextEdit, TextStyle, Vec2,
};
use egui::epaint::{Shadow, StrokeKind};
use egui::text::{LayoutJob, TextFormat};

/// Font sizes of the roles from §3.
pub mod text {
    pub const OVERLINE: f32 = 11.0;
    pub const SMALL: f32 = 12.0;
    /// Table header, semibold and muted.
    pub const HEAD: f32 = 12.0;
    pub const BODY: f32 = 13.0;
    pub const LARGE: f32 = 15.0;
    pub const VALUE: f32 = 17.0;
    pub const TITLE: f32 = 22.0;
    pub const MONO: f32 = 12.0;
}

/// 4 px grid (§4.1).
pub mod space {
    pub const S1: f32 = 4.0;
    pub const S2: f32 = 8.0;
    pub const S3: f32 = 12.0;
    pub const S4: f32 = 16.0;
    pub const S6: f32 = 24.0;
    pub const S8: f32 = 32.0;
}

/// Radii (§4.3).
pub mod radius {
    pub const SM: u8 = 4;
    pub const MD: u8 = 6;
    pub const LG: u8 = 8;
    pub const XL: u8 = 12;
    /// Full pill. egui stores radii as `u8`; 255 rounds heights up to 32 px completely.
    pub const PILL: u8 = 255;
}

/// Controls of a row (§4.2).
pub const CONTROL: f32 = 32.0;
/// Compact in toolbars: chips, segments.
pub const COMPACT: f32 = 24.0;
pub const TOOLBAR: f32 = 40.0;
/// Track of the progress bar (§8).
pub const TRACK: f32 = 4.0;
/// Icon sizes (§7): 16 inline, 20 in toolbars and tabs.
pub const ICON: f32 = 16.0;
pub const ICON_BAR: f32 = 20.0;
/// × at the top right of cards (§8 dialog card: 28).
pub const CLOSE: f32 = 28.0;
/// Height of the tab bar.
pub const TAB_H: f32 = 36.0;
/// Height of a status tag in a row.
const TAG_H: f32 = 20.0;
pub const STATUS_BAR: f32 = 28.0;

/// Lucide icons from `lucide-static@0.460.0`, cut out by
/// `tools/make_lucide_subset.py`.
pub mod icon {
    pub const CHECK: char = '\u{E070}';
    pub const CHEVRON_DOWN: char = '\u{E071}';
    pub const CHEVRON_RIGHT: char = '\u{E073}';
    pub const CIRCLE_ALERT: char = '\u{E07B}';
    pub const CIRCLE_CHECK: char = '\u{E226}';
    pub const CPU: char = '\u{E0AD}';
    pub const FILE_CHECK: char = '\u{E0C5}';
    pub const FILE_JSON: char = '\u{E36F}';
    pub const FILE_PLUS: char = '\u{E0CD}';
    pub const FOLDER: char = '\u{E0DC}';
    pub const FOLDER_TREE: char = '\u{E340}';
    pub const HARD_DRIVE: char = '\u{E0F0}';
    pub const HISTORY: char = '\u{E1F5}';
    pub const INFO: char = '\u{E0FE}';
    pub const LANGUAGES: char = '\u{E103}';
    pub const MONITOR: char = '\u{E121}';
    pub const MOON: char = '\u{E122}';
    pub const SEARCH: char = '\u{E155}';
    pub const SHIELD_CHECK: char = '\u{E1FF}';
    pub const SUN: char = '\u{E17C}';
    pub const TRIANGLE_ALERT: char = '\u{E193}';
    pub const X: char = '\u{E1B2}';
}

/// Complete token set from §2. This screen does not need `canvas`, `highlight` and
/// `scrim_light` (there is no popup any more now that the settings are in the toolbar);
/// they remain part of the palette.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub canvas: Color32,
    pub surface: Color32,
    pub surface_raised: Color32,
    pub surface_muted: Color32,
    pub hover: Color32,
    pub surface_pressed: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub accent: Color32,
    pub accent_strong: Color32,
    pub accent_subtle: Color32,
    pub highlight: Color32,
    pub accent_fill: Color32,
    pub accent_fill_hover: Color32,
    pub danger_fill: Color32,
    pub danger_fill_hover: Color32,
    pub ok: Color32,
    pub ok_bg: Color32,
    pub warn: Color32,
    pub warn_bg: Color32,
    pub error: Color32,
    pub error_bg: Color32,
    pub scrim: Color32,
    pub scrim_light: Color32,
    /// Background of the file list – not a token from §2 but Veritas' choice from them: dark `bg`,
    /// light `surface`. On `bg` (#F2F4F7), selection, hover and tag backgrounds would be barely
    /// visible (decided on 2026-11-19).
    pub list: Color32,
    /// Status “neu” (in the folder, missing from the .md5): text and icon. A content color
    /// of Veritas per §2.4, not a system token – violet, so that “neu” stands out from “offen”
    /// (neutral) and from the selection (blue) (decided on 2026-11-19).
    pub new: Color32,
    /// Background of the “neu” tag.
    pub new_bg: Color32,
}

pub const DARK: Palette = Palette {
    bg: rgb(0x14, 0x14, 0x14),
    canvas: rgb(0x14, 0x14, 0x14),
    surface: rgb(0x20, 0x20, 0x20),
    surface_raised: rgb(0x26, 0x26, 0x26),
    surface_muted: rgb(0x2A, 0x2A, 0x2A),
    hover: rgb(0x2A, 0x2A, 0x2A),
    surface_pressed: rgb(0x33, 0x33, 0x33),
    line: rgb(0x35, 0x35, 0x35),
    line_strong: rgb(0x70, 0x70, 0x70),
    text: rgb(0xE6, 0xE6, 0xE6),
    muted: rgb(0x9A, 0x9A, 0x9A),
    disabled: rgb(0x5A, 0x5A, 0x5A),
    accent: rgb(0x5B, 0x8E, 0xC4),
    accent_strong: rgb(0x7A, 0xAC, 0xD8),
    accent_subtle: rgb(0x1E, 0x3A, 0x52),
    highlight: rgb(0x16, 0x26, 0x38),
    accent_fill: rgb(0x45, 0x7A, 0xAE),
    accent_fill_hover: rgb(0x3F, 0x6E, 0x9E),
    danger_fill: rgb(0xC9, 0x4A, 0x3F),
    danger_fill_hover: rgb(0xB4, 0x43, 0x3A),
    ok: rgb(0x2E, 0xCC, 0x8B),
    ok_bg: rgb(0x0E, 0x33, 0x26),
    warn: rgb(0xF0, 0xA3, 0x30),
    warn_bg: rgb(0x3A, 0x26, 0x00),
    error: rgb(0xE6, 0x6A, 0x5E),
    error_bg: rgb(0x3D, 0x12, 0x10),
    scrim: Color32::from_black_alpha(166),
    scrim_light: Color32::from_black_alpha(77),
    list: rgb(0x14, 0x14, 0x14),
    new: rgb(0xC3, 0x9D, 0xF0),
    new_bg: rgb(0x33, 0x20, 0x4F),
};

pub const LIGHT: Palette = Palette {
    bg: rgb(0xF2, 0xF4, 0xF7),
    canvas: rgb(0xE3, 0xE3, 0xE3),
    surface: rgb(0xFF, 0xFF, 0xFF),
    surface_raised: rgb(0xFF, 0xFF, 0xFF),
    surface_muted: rgb(0xF8, 0xF9, 0xFB),
    hover: rgb(0xEE, 0xF2, 0xF7),
    surface_pressed: rgb(0xE3, 0xE9, 0xF0),
    line: rgb(0xD4, 0xDC, 0xE6),
    line_strong: rgb(0x7F, 0x8F, 0xA3),
    text: rgb(0x1A, 0x23, 0x30),
    muted: rgb(0x52, 0x63, 0x7A),
    disabled: rgb(0x9A, 0xA5, 0xB3),
    accent: rgb(0x3B, 0x5F, 0x8A),
    accent_strong: rgb(0x2D, 0x4D, 0x73),
    accent_subtle: rgb(0xEB, 0xF0, 0xF7),
    highlight: rgb(0xEB, 0xF0, 0xF7),
    accent_fill: rgb(0x3B, 0x5F, 0x8A),
    accent_fill_hover: rgb(0x2D, 0x4D, 0x73),
    danger_fill: rgb(0xC0, 0x39, 0x2B),
    danger_fill_hover: rgb(0xA9, 0x32, 0x26),
    ok: rgb(0x17, 0x78, 0x4F),
    ok_bg: rgb(0xE6, 0xF4, 0xEE),
    warn: rgb(0xAD, 0x5A, 0x00),
    warn_bg: rgb(0xFE, 0xF3, 0xE6),
    error: rgb(0xC0, 0x39, 0x2B),
    error_bg: rgb(0xFD, 0xEC, 0xEB),
    // Premultiplied equivalent of #1A2330 at 45 % and 15 %.
    scrim: Color32::from_rgba_premultiplied(12, 16, 22, 115),
    scrim_light: Color32::from_rgba_premultiplied(4, 5, 7, 38),
    list: rgb(0xFF, 0xFF, 0xFF),
    new: rgb(0x7A, 0x3F, 0xC0),
    new_bg: rgb(0xF4, 0xED, 0xFC),
};

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub fn on_fill() -> Color32 {
    Color32::WHITE
}

/// Content of a locked primary or danger button: white at half opacity.
pub fn on_fill_disabled() -> Color32 {
    Color32::WHITE.gamma_multiply(0.5)
}

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

fn palette_of(theme: egui::Theme) -> &'static Palette {
    match theme {
        egui::Theme::Dark => &DARK,
        egui::Theme::Light => &LIGHT,
    }
}

pub fn palette(ctx: &egui::Context) -> &'static Palette {
    palette_of(ctx.theme())
}

/// `shadow` from §2.2 and §2.3, only under `surface-raised`.
fn shadow_of(theme: egui::Theme) -> Shadow {
    match theme {
        egui::Theme::Dark => Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(102),
        },
        egui::Theme::Light => Shadow {
            offset: [0, 10],
            blur: 28,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(0x1A, 0x23, 0x30, 20),
        },
    }
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(crate::fonts::definitions());
    crate::fonts::add_fallback_provider(ctx);
    ctx.set_style_of(egui::Theme::Dark, style_for(egui::Theme::Dark));
    ctx.set_style_of(egui::Theme::Light, style_for(egui::Theme::Light));
    ctx.options_mut(|opt| opt.fallback_theme = egui::Theme::Dark);
}

pub fn bar_frame(ctx: &egui::Context, margin: Margin) -> Frame {
    Frame::new().fill(palette(ctx).surface).inner_margin(margin)
}

/// Card, list, notice or progress pill: `surface-raised` with border and shadow.
pub fn raised_frame(ctx: &egui::Context, fill: Color32, border: Color32, radius: u8) -> Frame {
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(radius)
        .shadow(shadow_of(ctx.theme()))
}

/// Paints without egui's graying out. Locked components set `disabled` themselves (§5).
fn token_painter(ui: &egui::Ui) -> Painter {
    ui.ctx()
        .layer_painter(ui.layer_id())
        .with_clip_rect(ui.clip_rect())
}

/// Dims everything behind a card and catches clicks outside it.
/// Returns `true` if the click landed outside the card.
pub fn scrim(ctx: &egui::Context, id: &str, color: Color32) -> bool {
    let screen = ctx.content_rect();
    let shown = egui::Area::new(Id::unique(id))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .constrain(false)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::CLICK);
            ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
            response.clicked()
        });
    // The progress pill and notices are also on `Middle`; the scrim belongs above them,
    // the card (`Foreground`) above that.
    ctx.move_to_top(shown.response.layer_id);
    shown.inner
}

/// The scaffold of every card over the window – dialog card, “Über Veritas”, history: centered
/// in the window, dimming (`{id}-scrim`), surface in the foreground (`id`), raised frame,
/// inner padding S6, corners XL. Returns whether the click landed outside, and what `add` returns.
pub fn card<R>(ctx: &egui::Context, id: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> (bool, R) {
    let p = palette(ctx);
    let outside = scrim(ctx, &format!("{id}-scrim"), p.scrim);
    let area = egui::Area::new(Id::unique(id))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO);
    let inner = area
        .show(ctx, |ui| {
            raised_frame(ctx, p.surface_raised, p.line, radius::XL)
                .inner_margin(Margin::same(space::S6 as i8))
                .show(ui, add)
                .inner
        })
        .inner;
    (outside, inner)
}

/// Width of the dialog cards.
pub const DIALOG_W: f32 = 440.0;
/// Dialog card (R4): title, text, on the right the secondary button and outermost the primary
/// or danger button (`role`), × at the top.
/// `Some(true)` confirms; `Some(false)` cancels (×, secondary button, click outside). Enter and
/// Esc are handled by `handle_keys`.
pub fn dialog(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    body: &str,
    (yes, role): (&str, Role),
    no: &str,
) -> Option<bool> {
    let p = palette(ctx);
    let mut decision = None;
    let (outside, ()) = card(ctx, id, |ui| {
        ui.set_width(DIALOG_W);
        ui.spacing_mut().item_spacing = Vec2::new(space::S2, 0.0);
        ui.horizontal(|ui| {
            ui.add(
                Label::new(
                    RichText::new(title)
                        .size(text::VALUE)
                        .family(semibold())
                        .color(p.text),
                )
                .truncate(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, icon::X, CLOSE)
                    .on_hover_text((crate::i18n::t().esc_tip)(no))
                    .clicked()
                {
                    decision = Some(false);
                }
            });
        });
        ui.add_space(space::S3);
        ui.add(Label::new(RichText::new(body).size(text::BODY).color(p.text)).wrap());
        ui.add_space(space::S6);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if button(ui, role, None, yes, 0.0, CONTROL)
                .on_hover_text((crate::i18n::t().enter_tip)(yes))
                .clicked()
            {
                decision = Some(true);
            }
            if button(ui, Role::Secondary, None, no, 0.0, CONTROL)
                .on_hover_text((crate::i18n::t().esc_tip)(no))
                .clicked()
            {
                decision = Some(false);
            }
        });
    });
    // A click outside cancels, unless a button has already decided.
    if outside && decision.is_none() {
        decision = Some(false);
    }
    decision
}

pub fn overline(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let p = palette(ui.ctx());
    let mut job = LayoutJob::default();
    job.append(
        &label.to_uppercase(),
        0.0,
        TextFormat {
            font_id: FontId::new(text::OVERLINE, semibold()),
            extra_letter_spacing: 0.5,
            color: p.muted,
            valign: Align::Center,
            ..Default::default()
        },
    );
    ui.add(Label::new(job).selectable(false))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Primary,
    Secondary,
    Danger,
}

/// Primary, secondary or danger button (§8). Takes no keyboard focus (R7).
pub fn button(
    ui: &mut egui::Ui,
    role: Role,
    icon: Option<char>,
    label: &str,
    min_width: f32,
    height: f32,
) -> egui::Response {
    let p = palette(ui.ctx());
    let enabled = ui.is_enabled();
    let compact = height <= COMPACT;
    let pad_x = if compact { space::S2 } else { space::S3 };
    let font = FontId::new(
        if compact { text::SMALL } else { text::BODY },
        FontFamily::Proportional,
    );
    let icon_px = ICON;
    let galley = ui
        .ctx()
        .fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font, p.text));
    let gap = if icon.is_some() { space::S2 } else { 0.0 };
    let icon_w = if icon.is_some() { icon_px } else { 0.0 };
    let min_width = if role == Role::Primary {
        min_width.max(120.0)
    } else {
        min_width
    };
    let width = (galley.size().x + icon_w + gap + pad_x * 2.0)
        .max(min_width)
        .round();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::CLICK);
    let hovered = enabled && response.hovered();
    let pressed = enabled && response.is_pointer_button_down_on();

    // Locked (§5): surface unchanged, content muted. Primary and danger buttons keep
    // their fill; `text-disabled` on it would be barely legible in dark (1.5:1), so white at
    // half opacity is used (`on_fill_disabled`).
    let (fill, stroke, fg, icon_color) = if !enabled {
        match role {
            Role::Primary => (
                p.accent_fill,
                Stroke::NONE,
                on_fill_disabled(),
                on_fill_disabled(),
            ),
            Role::Danger => (
                p.danger_fill,
                Stroke::NONE,
                on_fill_disabled(),
                on_fill_disabled(),
            ),
            Role::Secondary => (
                p.surface_muted,
                Stroke::new(1.0_f32, p.line),
                p.disabled,
                p.disabled,
            ),
        }
    } else {
        match role {
            Role::Primary => {
                let fill = if hovered || pressed {
                    p.accent_fill_hover
                } else {
                    p.accent_fill
                };
                (fill, Stroke::NONE, on_fill(), on_fill())
            }
            Role::Danger => {
                let fill = if hovered || pressed {
                    p.danger_fill_hover
                } else {
                    p.danger_fill
                };
                (fill, Stroke::NONE, on_fill(), on_fill())
            }
            Role::Secondary => {
                let fill = if pressed {
                    p.surface_pressed
                } else if hovered {
                    p.hover
                } else {
                    p.surface_muted
                };
                let border = if hovered { p.accent } else { p.line };
                let icon_color = if hovered { p.accent_strong } else { p.muted };
                (fill, Stroke::new(1.0_f32, border), p.text, icon_color)
            }
        }
    };

    let painter = token_painter(ui);
    painter.rect(
        rect,
        CornerRadius::same(radius::MD),
        fill,
        stroke,
        StrokeKind::Inside,
    );
    let content_w = icon_w + gap + galley.size().x;
    let mut x = rect.center().x - content_w / 2.0;
    let y = rect.center().y;
    if let Some(glyph) = icon {
        painter.text(
            Pos2::new(x, y),
            Align2::LEFT_CENTER,
            glyph.to_string(),
            FontId::new(icon_px, FontFamily::Proportional),
            icon_color,
        );
        x += icon_w + gap;
    }
    painter.galley_with_override_text_color(Pos2::new(x, y - galley.size().y / 2.0), galley, fg);
    response
}

/// Icon button (§8): at rest `text-muted`, under the pointer `accent-strong` on `hover`.
/// Takes no keyboard focus (R7). In 32 px toolbars a 20 px icon, otherwise 16 px.
pub fn icon_button(ui: &mut egui::Ui, glyph: char, size: f32) -> egui::Response {
    toggle_icon_button(ui, glyph, size, false)
}

/// Icon button with state (§7, §8): “on” (`on`) with `accent-subtle` surface and
/// `accent-strong` icon, e.g. “Unterordner” in the toolbar while it is on.
pub fn toggle_icon_button(ui: &mut egui::Ui, glyph: char, size: f32, on: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let enabled = ui.is_enabled();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::CLICK);
    let painter = token_painter(ui);
    if enabled && on {
        painter.rect_filled(rect, CornerRadius::same(radius::MD), p.accent_subtle);
    } else if enabled && response.hovered() {
        let fill = if response.is_pointer_button_down_on() {
            p.surface_pressed
        } else {
            p.hover
        };
        painter.rect_filled(rect, CornerRadius::same(radius::MD), fill);
    }
    let color = if !enabled {
        p.disabled
    } else if on || response.hovered() {
        p.accent_strong
    } else {
        p.muted
    };
    let px = if size >= CONTROL { ICON_BAR } else { ICON };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph.to_string(),
        FontId::new(px, FontFamily::Proportional),
        color,
    );
    response
}

/// A segment of the view switcher: icon, value and tooltip (which says what the icon
/// means, §7).
pub struct Choice<T> {
    pub glyph: char,
    pub value: T,
    pub tip: &'static str,
}

/// Height of a segment in a toolbar: the group (2 px inside, 1 px border) is then as tall as
/// buttons and fields (§4.2: everything in a row the same height).
pub const BAR_SEGMENT: f32 = CONTROL - 2.0 * (2.0 + 1.0);

/// View switcher (§8): segment group with `line` border, `radius-lg`, 2 px inside; parts 32
/// wide and `BAR_SEGMENT` (26) tall – not 30 as in §8 and not compact 24, so that the group
/// is as tall as the buttons and fields beside it (§4.2) – with an icon in toolbar size (20),
/// only the chosen one is filled (`accent-subtle`, border `accent`, icon `accent-strong`).
/// Takes no keyboard focus (R7).
pub fn segments<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    group: &str,
    current: &mut T,
    items: &[Choice<T>],
) -> bool {
    let p = palette(ui.ctx());
    Frame::new()
        .stroke(Stroke::new(1.0_f32, p.line))
        .corner_radius(radius::LG)
        .inner_margin(2.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            // Not `ui.horizontal`: it would take `interact_size.y` as its starting height and put
            // the group lower in a toolbar than buttons and fields.
            let size = Vec2::new(CONTROL * items.len() as f32, BAR_SEGMENT);
            ui.push_id(group, |ui| {
                ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
                    segment_row(ui, current, items)
                })
                .inner
            })
            .inner
        })
        .inner
}

fn segment_row<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    items: &[Choice<T>],
) -> bool {
    let p = palette(ui.ctx());
    let enabled = ui.is_enabled();
    let mut changed = false;
    for item in items {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(CONTROL, BAR_SEGMENT), Sense::CLICK);
        let painter = token_painter(ui);
        let selected = *current == item.value;
        if selected {
            painter.rect(
                rect,
                CornerRadius::same(radius::MD),
                p.accent_subtle,
                Stroke::new(1.0_f32, p.accent),
                StrokeKind::Inside,
            );
        } else if response.hovered() && enabled {
            painter.rect_filled(rect, CornerRadius::same(radius::MD), p.hover);
        }
        let color = if !enabled {
            p.disabled
        } else if selected || response.hovered() {
            p.accent_strong
        } else {
            p.muted
        };
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            item.glyph.to_string(),
            FontId::new(ICON_BAR, FontFamily::Proportional),
            color,
        );
        if response.clicked() && *current != item.value {
            *current = item.value;
            changed = true;
        }
        let _ = response
            .on_hover_text(item.tip)
            .on_disabled_hover_text(item.tip);
    }
    changed
}

/// Field (§8): 32 tall, recessed, `line-strong` border, in focus `accent` border and ring.
/// An icon sits at the left inside the field.
pub fn text_field(
    ui: &mut egui::Ui,
    id: Id,
    value: &mut String,
    hint: &str,
    outer_width: f32,
    icon: Option<char>,
) -> egui::Response {
    let p = palette(ui.ctx());
    let icon_space = if icon.is_some() {
        ICON + space::S2
    } else {
        0.0
    };
    let margin = Margin {
        left: (space::S2 + icon_space) as i8,
        right: space::S2 as i8,
        top: space::S1 as i8,
        bottom: space::S1 as i8,
    };
    let output = TextEdit::singleline(value)
        .id(id)
        .hint_text(RichText::new(hint).color(p.muted).size(text::BODY))
        .desired_width(outer_width - margin.sum().x)
        .min_size(Vec2::new(outer_width, CONTROL))
        .margin(margin)
        .align(Align2::LEFT_CENTER)
        .font(FontId::new(text::BODY, FontFamily::Proportional))
        .text_color(p.text)
        .show(ui);
    // Since egui 0.37 the response is the outer rectangle, border included.
    let response = output.response.response;
    let frame = response.rect;
    if let Some(glyph) = icon {
        ui.painter().text(
            Pos2::new(frame.left() + space::S2, frame.center().y),
            Align2::LEFT_CENTER,
            glyph.to_string(),
            FontId::new(ICON, FontFamily::Proportional),
            p.muted,
        );
    }
    if response.has_focus() {
        focus_ring(ui, frame);
    }
    response
}

/// Number field: a field (§8) for an integer in `range`. Accepts only digits; ↑/↓ count
/// one step when focused. A value outside the range counts as the nearest bound and is shown
/// as such once the field is left.
pub fn number_field(
    ui: &mut egui::Ui,
    id: Id,
    value: &mut usize,
    range: std::ops::RangeInclusive<usize>,
    outer_width: f32,
) -> egui::Response {
    let buffer_id = id.with("text");
    let focused = ui.memory(|mem| mem.has_focus(id));
    let mut buffer = if focused {
        ui.data_mut(|data| data.get_temp::<String>(buffer_id))
            .unwrap_or_else(|| value.to_string())
    } else {
        value.to_string()
    };
    let mut response = text_field(ui, id, &mut buffer, "", outer_width, None);
    buffer.retain(|ch| ch.is_ascii_digit());
    let (low, high) = (*range.start(), *range.end());
    if let Ok(parsed) = buffer.parse::<usize>() {
        let clamped = parsed.clamp(low, high);
        if clamped != *value {
            *value = clamped;
            response.mark_changed();
        }
    }
    if response.has_focus() {
        let (up, down) = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            )
        });
        if up || down {
            *value = if up {
                value.saturating_add(1).min(high)
            } else {
                value.saturating_sub(1).max(low)
            };
            buffer = value.to_string();
            response.mark_changed();
        }
        ui.data_mut(|data| data.insert_temp(buffer_id, buffer));
    } else {
        ui.data_mut(|data| data.remove::<String>(buffer_id));
    }
    response
}

/// Vertical divider between groups of a toolbar (§4.1: 16 between groups, here per
/// side the toolbar's spacing of 8, plus a 1 px `line`).
pub fn bar_separator(ui: &mut egui::Ui) {
    let p = palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, ICON_BAR), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0_f32, p.line),
    );
}

/// Filter chip (§8) with status dot and counter. The width depends only on `count_sample`
/// and the label, not on the number (R1). An active chip shows a × instead of the
/// dot, in the same place.
pub fn filter_chip(
    ui: &mut egui::Ui,
    dot: Color32,
    count: &str,
    count_sample: &str,
    label: &str,
    active: bool,
) -> egui::Response {
    let p = palette(ui.ctx());
    let enabled = ui.is_enabled();
    // Digits of the UI font are equally wide; a monospace font would sit higher than the label.
    let count_font = FontId::new(text::SMALL, FontFamily::Proportional);
    let label_font = FontId::new(text::SMALL, FontFamily::Proportional);
    let fg = if enabled { p.text } else { p.disabled };
    let sample = ui
        .ctx()
        .fonts_mut(|fonts| fonts.layout_no_wrap(count_sample.to_owned(), count_font.clone(), fg));
    let count_galley = ui
        .ctx()
        .fonts_mut(|fonts| fonts.layout_no_wrap(count.to_owned(), count_font, fg));
    let label_galley = ui
        .ctx()
        .fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), label_font, fg));
    let dot_d = 6.0;
    let slot = 12.0;
    let width = (space::S2
        + slot
        + space::S1
        + sample.size().x
        + space::S1
        + label_galley.size().x
        + space::S2)
        .round();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, COMPACT), Sense::CLICK);
    let painter = token_painter(ui);
    let hovered = enabled && response.hovered();
    let (fill, border) = if active {
        (p.accent_subtle, p.accent)
    } else if hovered {
        (p.hover, p.line)
    } else {
        (p.surface_muted, p.line)
    };
    painter.rect(
        rect,
        CornerRadius::same(radius::MD),
        fill,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    let y = rect.center().y;
    let mut x = rect.left() + space::S2;
    let slot_center = Pos2::new(x + slot / 2.0, y);
    if active {
        painter.text(
            slot_center,
            Align2::CENTER_CENTER,
            icon::X.to_string(),
            FontId::new(slot, FontFamily::Proportional),
            p.accent_strong,
        );
    } else {
        painter.circle_filled(
            slot_center,
            dot_d / 2.0,
            if enabled { dot } else { p.disabled },
        );
    }
    x += slot + space::S1;
    let count_x = x + sample.size().x - count_galley.size().x;
    painter.galley_with_override_text_color(
        Pos2::new(count_x, y - count_galley.size().y / 2.0),
        count_galley,
        fg,
    );
    x += sample.size().x + space::S1;
    painter.galley_with_override_text_color(
        Pos2::new(x, y - label_galley.size().y / 2.0),
        label_galley,
        fg,
    );
    response
}

/// Status tag (§8): pill, status color on `*-bg`, `small`, with dot. Sits at the left,
/// vertically centered at `left_center`.
/// What a status tag reports. The color pairs live only here, the same for file list and
/// history: pending and taken-over entries are neutral, new is violet (`Palette::new`); blue stays
/// reserved for the selection (§2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Neutral,
    New,
    Ok,
    Warn,
    Error,
}

impl Tag {
    /// Text and surface color.
    pub fn colors(self, p: &Palette) -> (Color32, Color32) {
        match self {
            Tag::Neutral => (p.muted, p.surface_muted),
            Tag::New => (p.new, p.new_bg),
            Tag::Ok => (p.ok, p.ok_bg),
            Tag::Warn => (p.warn, p.warn_bg),
            Tag::Error => (p.error, p.error_bg),
        }
    }
}

pub fn status_tag(ui: &egui::Ui, left_center: Pos2, label: &str, fg: Color32, bg: Color32) {
    let galley = ui.ctx().fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            label.to_owned(),
            FontId::new(text::SMALL, FontFamily::Proportional),
            fg,
        )
    });
    let dot_d = 6.0;
    let height = TAG_H;
    let width = space::S2 + dot_d + space::S1 + galley.size().x + space::S2;
    let rect = Rect::from_min_size(
        Pos2::new(left_center.x, left_center.y - height / 2.0),
        Vec2::new(width, height),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(radius::PILL), bg);
    painter.circle_filled(
        Pos2::new(rect.left() + space::S2 + dot_d / 2.0, rect.center().y),
        dot_d / 2.0,
        fg,
    );
    painter.galley_with_override_text_color(
        Pos2::new(
            rect.left() + space::S2 + dot_d + space::S1,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
}

/// Tab (§8) across the whole `width`: `body`; active `text` semibold with a 2 px `accent` line,
/// otherwise `text-muted`. `keys` goes in the tooltip. Returns the other tab that was clicked.
pub fn tabs(
    ui: &mut egui::Ui,
    group: &str,
    labels: &[&str],
    current: usize,
    width: f32,
    keys: &str,
) -> Option<usize> {
    let p = palette(ui.ctx());
    let (strip, _) = ui.allocate_exact_size(Vec2::new(width, TAB_H), Sense::hover());
    ui.painter().hline(
        strip.x_range(),
        strip.bottom() - 0.5,
        Stroke::new(1.0_f32, p.line),
    );
    let mut x = strip.left();
    let mut picked = None;
    for (index, label) in labels.iter().enumerate() {
        let active = index == current;
        let font = if active {
            FontId::new(text::BODY, semibold())
        } else {
            FontId::new(text::BODY, FontFamily::Proportional)
        };
        let color = if active { p.text } else { p.muted };
        let galley = ui
            .ctx()
            .fonts_mut(|fonts| fonts.layout_no_wrap((*label).to_owned(), font, color));
        // Width always by the semibold text, so that a switch moves nothing (R1).
        let bold_w = ui
            .ctx()
            .fonts_mut(|fonts| {
                fonts.layout_no_wrap(
                    (*label).to_owned(),
                    FontId::new(text::BODY, semibold()),
                    color,
                )
            })
            .size()
            .x;
        let rect = Rect::from_min_size(
            Pos2::new(x, strip.top()),
            Vec2::new(bold_w + 2.0 * space::S3, strip.height()),
        );
        let response = ui
            .interact(rect, Id::unique((group, index)), Sense::CLICK)
            .on_hover_text(format!("{label} ({keys})"));
        if response.hovered() && !active {
            ui.painter().rect_filled(
                rect.shrink2(Vec2::new(0.0, 4.0)),
                CornerRadius::same(radius::MD),
                p.hover,
            );
        }
        ui.painter().galley(
            Pos2::new(
                rect.left() + space::S3,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
        if active {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(rect.left(), rect.bottom() - 2.0),
                    rect.right_bottom(),
                ),
                CornerRadius::ZERO,
                p.accent,
            );
        }
        if response.clicked() && !active {
            picked = Some(index);
        }
        x = rect.right() + space::S1;
    }
    picked
}

/// Keycap (§8): `small`, `surface-muted`, `line` border, `radius-sm`, padding 7 × 3.
pub fn keycap(ui: &mut egui::Ui, key: &str) -> egui::Response {
    let p = palette(ui.ctx());
    let galley = ui.ctx().fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            key.to_owned(),
            FontId::new(text::SMALL, FontFamily::Proportional),
            p.text,
        )
    });
    let size = galley.size() + Vec2::new(14.0, 6.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(radius::SM),
        p.surface_muted,
        Stroke::new(1.0_f32, p.line),
        StrokeKind::Inside,
    );
    ui.painter()
        .galley_with_override_text_color(rect.min + Vec2::new(7.0, 3.0), galley, p.text);
    response
}

/// Minimum width of a list or menu (§8: 240–340).
pub const MENU_W: f32 = 240.0;

/// Menu row (§8): 32 tall, text `body`, shortcut on the right `small` `text-muted`; highlighted
/// `accent-subtle` (`radius-md`), the shortcut then `text`. Locked with the reason in the
/// tooltip (R2); a click on a locked row does nothing – the caller checks `why`. Takes no
/// keyboard focus (R7).
pub fn menu_row(ui: &mut egui::Ui, label: &str, keys: &str, why: Option<&str>) -> egui::Response {
    let p = palette(ui.ctx());
    let enabled = why.is_none();
    let width = ui.available_width().max(MENU_W);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, CONTROL), Sense::CLICK);
    let lit = enabled && response.hovered();
    if lit {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(radius::MD), p.accent_subtle);
    }
    let fg = if enabled { p.text } else { p.disabled };
    let keys_color = if !enabled {
        p.disabled
    } else if lit {
        p.text
    } else {
        p.muted
    };
    let y = rect.center().y;
    ui.painter().text(
        Pos2::new(rect.left() + space::S3, y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(text::BODY, FontFamily::Proportional),
        fg,
    );
    if !keys.is_empty() {
        ui.painter().text(
            Pos2::new(rect.right() - space::S3, y),
            Align2::RIGHT_CENTER,
            keys,
            FontId::new(text::SMALL, FontFamily::Proportional),
            keys_color,
        );
    }
    match why {
        Some(why) => response.on_hover_text(why),
        None => response,
    }
}

/// Width of a list of `choice_row`s: as wide as the longest text plus check-mark column and
/// right margin – deliberately narrower than `MENU_W` (§8), so that a short list like the
/// languages does not look wide and empty.
pub fn choice_width<'a>(ui: &egui::Ui, labels: impl IntoIterator<Item = &'a str>) -> f32 {
    let p = palette(ui.ctx());
    let widest = labels
        .into_iter()
        .map(|label| {
            ui.ctx()
                .fonts_mut(|fonts| {
                    fonts.layout_no_wrap(
                        label.to_owned(),
                        FontId::new(text::BODY, FontFamily::Proportional),
                        p.text,
                    )
                })
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    (CHECK_COLUMN + widest + space::S4).ceil()
}

/// Menu row for choosing from a list (§8): column 30 with the check mark of the chosen row,
/// text `body`; highlighted `accent-subtle`. `width` comes from `choice_width`. Takes no
/// keyboard focus (R7).
pub fn choice_row(ui: &mut egui::Ui, label: &str, chosen: bool, width: f32) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, CONTROL), Sense::CLICK);
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(radius::MD), p.accent_subtle);
    }
    let y = rect.center().y;
    if chosen {
        ui.painter().text(
            Pos2::new(rect.left() + CHECK_COLUMN / 2.0, y),
            Align2::CENTER_CENTER,
            icon::CHECK.to_string(),
            FontId::new(ICON, FontFamily::Proportional),
            p.accent_strong,
        );
    }
    ui.painter().text(
        Pos2::new(rect.left() + CHECK_COLUMN, y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(text::BODY, FontFamily::Proportional),
        p.text,
    );
    response
}

/// Column for checkbox or check mark before the text of a menu row (§8).
const CHECK_COLUMN: f32 = 30.0;

/// Clickable row with keycap and text as a link (`accent-strong`), e.g. a recently
/// opened folder in the empty state. 32 tall (click target §4.2), `hover` under the pointer.
/// Takes no keyboard focus (R7).
pub fn link_row(ui: &mut egui::Ui, key: &str, label: &str, width: f32) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, CONTROL), Sense::CLICK);
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(radius::MD), p.hover);
    }
    let font = FontId::new(text::SMALL, FontFamily::Proportional);
    let galley = ui
        .ctx()
        .fonts_mut(|fonts| fonts.layout_no_wrap(key.to_owned(), font, p.text));
    let cap = Rect::from_min_size(
        Pos2::new(
            rect.left() + space::S1,
            rect.center().y - (galley.size().y + 6.0) / 2.0,
        ),
        galley.size() + Vec2::new(14.0, 6.0),
    );
    ui.painter().rect(
        cap,
        CornerRadius::same(radius::SM),
        p.surface_muted,
        Stroke::new(1.0_f32, p.line),
        StrokeKind::Inside,
    );
    ui.painter()
        .galley_with_override_text_color(cap.min + Vec2::new(7.0, 3.0), galley, p.text);
    let text_left = cap.right() + space::S2;
    let label_font = FontId::new(text::BODY, FontFamily::Proportional);
    // One line; if it is too long it ends in “…” (§10), the tooltip gives the full text.
    let mut job = LayoutJob::simple_singleline(label.to_owned(), label_font, p.accent_strong);
    job.wrap = egui::text::TextWrapping {
        max_width: (rect.right() - text_left - space::S2).max(8.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let shown = ui.ctx().fonts_mut(|fonts| fonts.layout_job(job));
    let text_y = rect.center().y - shown.size().y / 2.0;
    ui.painter()
        .galley(Pos2::new(text_left, text_y), shown, p.accent_strong);
    response
}

/// Divider between groups of a menu: `line`, `space-1` above and below.
pub fn menu_separator(ui: &mut egui::Ui) {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 2.0 * space::S1 + 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0_f32, p.line),
    );
}

/// Width of the ring around the chosen row in the pane with keyboard focus (§4.4).
pub const FOCUS_RING: f32 = 2.0;

/// Ground of a row in list or tree (§5): chosen `accent-subtle`; if the keyboard focus is
/// in this pane (`focused`), plus an `accent-strong` ring – so you can see where ↑/↓ go.
/// The other pane shows its selection only as a fill. `hover` under the pointer.
pub fn row_ground(ui: &egui::Ui, rect: Rect, selected: bool, focused: bool, hovered: bool) {
    let p = palette(ui.ctx());
    let corner = CornerRadius::same(radius::MD);
    if selected {
        ui.painter().rect_filled(rect, corner, p.accent_subtle);
        if focused {
            ui.painter().rect_stroke(
                rect,
                corner,
                Stroke::new(FOCUS_RING, p.accent_strong),
                StrokeKind::Inside,
            );
        }
    } else if hovered {
        ui.painter().rect_filled(rect, corner, p.hover);
    }
}

/// Focus ring (§5): 2 px `accent-strong`, 1 px gap.
fn focus_ring(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_stroke(
        rect.expand(1.0),
        CornerRadius::same(radius::MD),
        Stroke::new(FOCUS_RING, palette(ui.ctx()).accent_strong),
        StrokeKind::Outside,
    );
}

fn style_for(theme: egui::Theme) -> egui::Style {
    let p = palette_of(theme);
    let mut style = theme.default_style();
    let mut visuals = theme.default_visuals();
    visuals.dark_mode = matches!(theme, egui::Theme::Dark);
    visuals.override_text_color = None;
    visuals.widgets.noninteractive = widget(p.surface, p.surface, p.line, p.text, radius::SM);
    visuals.widgets.inactive = widget(
        p.surface_muted,
        p.surface_muted,
        p.line_strong,
        p.text,
        radius::MD,
    );
    visuals.widgets.hovered = widget(p.hover, p.hover, p.line_strong, p.text, radius::MD);
    visuals.widgets.active = widget(
        p.surface_pressed,
        p.surface_pressed,
        p.accent,
        p.text,
        radius::MD,
    );
    visuals.widgets.open = widget(p.surface, p.surface, p.line, p.text, radius::MD);
    visuals.selection.bg_fill = p.accent_subtle;
    visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);
    // `accent` has only 4.4:1 in dark on `surface-raised`.
    visuals.hyperlink_color = p.accent_strong;
    visuals.faint_bg_color = p.surface_muted;
    visuals.extreme_bg_color = match theme {
        egui::Theme::Dark => p.bg,
        egui::Theme::Light => Color32::WHITE,
    };
    visuals.code_bg_color = visuals.extreme_bg_color;
    visuals.warn_fg_color = p.warn;
    visuals.error_fg_color = p.error;
    visuals.window_corner_radius = CornerRadius::same(radius::XL);
    visuals.menu_corner_radius = CornerRadius::same(radius::LG);
    visuals.window_fill = p.surface_raised;
    visuals.window_stroke = Stroke::new(1.0_f32, p.line);
    visuals.window_shadow = shadow_of(theme);
    visuals.popup_shadow = visuals.window_shadow;
    visuals.window_highlight_topmost = false;
    visuals.panel_fill = p.bg;
    visuals.text_cursor.stroke = Stroke::new(2.0_f32, p.accent_strong);
    visuals.striped = false;
    visuals.interact_cursor = None;
    visuals.button_frame = true;
    visuals.collapsing_header_frame = false;
    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(space::S2, space::S2);
    style.spacing.button_padding = Vec2::new(space::S3, space::S2);
    // Do not set to 32: `ui.horizontal` takes this height as its starting value.
    style.spacing.interact_size = Vec2::new(40.0, 18.0);
    style.spacing.window_margin = Margin::same(space::S6 as i8);
    style.spacing.menu_margin = Margin::same(space::S4 as i8);
    style.spacing.icon_width = 14.0;
    style.spacing.icon_spacing = space::S2;
    style.spacing.tooltip_width = 360.0;
    style.animation_time = 0.1;
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(text::SMALL, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(text::BODY, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(text::BODY, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::new(text::TITLE, semibold()));
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(text::MONO, FontFamily::Monospace),
    );
    style
}

fn widget(
    fill: Color32,
    weak: Color32,
    border: Color32,
    fg: Color32,
    radius: u8,
) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: weak,
        bg_stroke: Stroke::new(1.0_f32, border),
        fg_stroke: Stroke::new(1.0_f32, fg),
        corner_radius: CornerRadius::same(radius),
        expansion: 0.0,
    }
}

/// Tooltip like `Response::on_hover_text`, whose text is built only when it appears – for
/// texts that would otherwise be built needlessly per row and frame.
pub fn lazy_tip(response: egui::Response, text: impl FnOnce() -> String) -> egui::Response {
    response.on_hover_ui(|ui| {
        // Like `on_hover_text`: the width of the tooltip, not narrower.
        ui.set_max_width(ui.spacing().tooltip_width);
        ui.add(Label::new(text()));
    })
}

/// Paints the text, shortened with “…” if needed. Returns whether it was shortened.
pub(crate) fn paint_fit(
    ui: &egui::Ui,
    pos: Pos2,
    text: &str,
    width: f32,
    font: FontId,
    color: Color32,
    keep_end: bool,
) -> bool {
    if text.is_empty() {
        return false;
    }
    let (shown, truncated) = ellipsize(ui, text, font.clone(), color, width.max(8.0), keep_end);
    ui.painter()
        .text(pos, Align2::LEFT_CENTER, shown, font, color);
    truncated
}

fn ellipsize(
    ui: &egui::Ui,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
    keep_end: bool,
) -> (String, bool) {
    let measure = |sample: &str| -> f32 {
        ui.ctx().fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(sample.to_owned(), font.clone(), color)
                .size()
                .x
        })
    };
    if measure(text) <= width {
        return (text.to_owned(), false);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut best = 0;
    let mut lo = 0;
    let mut hi = chars.len();
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let sample = clipped(&chars, mid, keep_end);
        if measure(&sample) <= width {
            best = mid;
            lo = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            hi = mid - 1;
        }
    }
    (clipped(&chars, best, keep_end), true)
}

fn clipped(chars: &[char], keep: usize, keep_end: bool) -> String {
    if keep == 0 || chars.is_empty() {
        return "…".into();
    }
    if keep >= chars.len() {
        return chars.iter().collect();
    }
    if keep_end {
        let tail: String = chars[chars.len() - keep..].iter().collect();
        format!("…{tail}")
    } else {
        let head: String = chars[..keep].iter().collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::LUCIDE;
    use eframe::egui::{FontData, FontDefinitions};
    use std::sync::Arc;

    fn contrast(a: Color32, b: Color32) -> f32 {
        fn channel(c: u8) -> f32 {
            let s = f32::from(c) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        fn lum(c: Color32) -> f32 {
            0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
        }
        let (l1, l2) = (lum(a), lum(b));
        let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (hi + 0.05) / (lo + 0.05)
    }

    fn assert_text(name: &str, fg: Color32, bg: Color32) {
        let ratio = contrast(fg, bg);
        assert!(ratio >= 4.5, "{name} contrast {ratio:.2} is below 4.5:1");
    }

    #[test]
    fn dark_contrast_meets_aa() {
        let p = DARK;
        assert_text("text/bg", p.text, p.bg);
        assert_text("text/surface", p.text, p.surface);
        assert_text("text/raised", p.text, p.surface_raised);
        assert_text("muted/surface", p.muted, p.surface);
        assert_text("white/accent-fill", on_fill(), p.accent_fill);
        assert_text("white/danger-fill", on_fill(), p.danger_fill);
        assert_text("ok", p.ok, p.ok_bg);
        assert_text("warn", p.warn, p.warn_bg);
        assert_text("new", p.new, p.new_bg);
        assert_text("error/bg", p.error, p.error_bg);
        assert_text("error/surface", p.error, p.surface);
        assert_text("accent/surface", p.accent, p.surface);
        assert_text("text/accent-subtle", p.text, p.accent_subtle);
        assert!(contrast(p.line_strong, p.surface) >= 3.0);
        assert_ne!(p.scrim, p.scrim_light);
    }

    #[test]
    fn light_contrast_meets_aa() {
        let p = LIGHT;
        assert_text("text/bg", p.text, p.bg);
        assert_text("text/surface", p.text, p.surface);
        assert_text("muted/bg", p.muted, p.bg);
        assert_text("white/accent-fill", on_fill(), p.accent_fill);
        assert_text("white/danger-fill", on_fill(), p.danger_fill);
        assert_text("ok", p.ok, p.ok_bg);
        assert_text("warn", p.warn, p.warn_bg);
        assert_text("new", p.new, p.new_bg);
        assert_text("error/bg", p.error, p.error_bg);
        assert_text("text/accent-subtle", p.text, p.accent_subtle);
        assert!(contrast(p.line_strong, Color32::WHITE) >= 3.0);
    }

    /// Every color against the surface on which Veritas really paints it: list on `list`
    /// (dark `bg`, light `surface`), rows under the pointer and chosen, tags on `*-bg`,
    /// chips, status bar, notices, dialog, links and buttons under the pointer.
    #[test]
    fn text_meets_aa_where_it_is_painted() {
        assert_eq!(DARK.list, DARK.bg);
        assert_eq!(LIGHT.list, LIGHT.surface);
        for (theme, p) in [("dunkel", DARK), ("hell", LIGHT)] {
            let pairs = [
                ("Etikett ok", p.ok, p.ok_bg),
                ("Etikett warn", p.warn, p.warn_bg),
                ("Etikett error", p.error, p.error_bg),
                ("Etikett neutral", p.muted, p.surface_muted),
                ("Etikett neu", p.new, p.new_bg),
                ("Neu auf Liste", p.new, p.list),
                ("Neu auf bg", p.new, p.bg),
                ("Neu auf surface", p.new, p.surface),
                ("Neu auf surface_raised", p.new, p.surface_raised),
                ("Neu auf Chip", p.new, p.surface_muted),
                ("Neu gewählt", p.new, p.accent_subtle),
                ("Neu Hover", p.new, p.hover),
                ("Zeile Text", p.text, p.list),
                ("Zeile Zweittext", p.muted, p.list),
                ("Leerzustand", p.muted, p.list),
                ("Symbol gewählt", p.accent_strong, p.accent_subtle),
                ("Link", p.accent_strong, p.surface_raised),
                ("Primärknopf Hover", on_fill(), p.accent_fill_hover),
                ("Gefahr-Knopf Hover", on_fill(), p.danger_fill_hover),
                ("Hinweis ok", p.ok, p.surface_raised),
                ("Hinweis warn", p.warn, p.surface_raised),
                ("Hinweis Fehler", p.error, p.error_bg),
                ("Laufliste Fehler", p.error, p.surface_raised),
                ("Zeile Hover Text", p.text, p.hover),
                ("Zeile Hover Zweittext", p.muted, p.hover),
                ("Zeile gewählt", p.text, p.accent_subtle),
                ("Kopf", p.muted, p.surface_muted),
                ("Chip", p.text, p.surface_muted),
                ("Chip aktiv", p.text, p.accent_subtle),
                ("Statusleiste", p.muted, p.surface),
                ("Baum", p.text, p.surface),
                ("Hinweis", p.text, p.surface_raised),
                ("Fehler-Hinweis", p.text, p.error_bg),
                ("Dialog Zweittext", p.muted, p.surface_raised),
            ];
            for (name, fg, bg) in pairs {
                assert_text(&format!("{theme}: {name}"), fg, bg);
            }
        }
    }

    /// Non-text per §11.3 (≥ 3:1): where the border carries the state – ring around the chosen
    /// row in the pane with focus, focus ring, border of active chips and segments, border of the
    /// progress pill, fields. The selection in the other pane is deliberately only a fill.
    #[test]
    fn selection_is_visible_where_it_is_painted() {
        for (theme, p) in [("dunkel", DARK), ("hell", LIGHT)] {
            let pairs = [
                ("Ring auf Liste", p.accent_strong, p.list),
                ("Ring auf Baum", p.accent_strong, p.surface),
                ("Ring auf Auswahl", p.accent_strong, p.accent_subtle),
                ("Chip aktiv", p.accent, p.surface),
                ("Segment gewählt", p.accent, p.surface_raised),
                ("Segment gewählt in der Leiste", p.accent, p.surface),
                (
                    "Schalter an in der Leiste",
                    p.accent_strong,
                    p.accent_subtle,
                ),
                ("Pille", p.accent, p.list),
                ("Pille Warnung", p.warn, p.list),
                ("Feld", p.line_strong, p.surface),
            ];
            for (name, fg, bg) in pairs {
                let ratio = contrast(fg, bg);
                assert!(
                    ratio >= 3.0,
                    "{theme}: {name} contrast {ratio:.2} is below 3:1"
                );
            }
        }
    }

    /// Tag surfaces stand out from the list: “neu” at least as clearly as the
    /// weakest of the existing status surfaces (light `warn_bg`, ≈ 1.09:1).
    #[test]
    fn tag_grounds_stand_out_on_the_list() {
        for (theme, p) in [("dunkel", DARK), ("hell", LIGHT)] {
            let weakest = [p.ok_bg, p.warn_bg, p.error_bg]
                .into_iter()
                .map(|bg| contrast(bg, p.list))
                .fold(f32::INFINITY, f32::min);
            let new = contrast(p.new_bg, p.list);
            assert!(
                new >= weakest && new >= 1.08,
                "{theme}: new_bg {new:.3} against list, weakest {weakest:.3}"
            );
            // Violet is neither blue (selection) nor red (error).
            assert_ne!(p.new, p.accent);
            assert_ne!(p.new, p.error);
        }
    }

    #[test]
    fn measures_follow_the_grid() {
        assert_eq!(CONTROL, 32.0);
        assert_eq!(COMPACT, 24.0);
        assert_eq!(TOOLBAR, 40.0);
        assert_eq!(STATUS_BAR, 28.0);
        assert_eq!(text::BODY, 13.0);
        for value in [
            space::S1,
            space::S2,
            space::S3,
            space::S4,
            space::S6,
            space::S8,
        ] {
            assert_eq!(value % 4.0, 0.0);
        }
    }

    #[test]
    fn every_icon_is_in_the_subset() {
        let ctx = egui::Context::default();
        let mut fonts = FontDefinitions::empty();
        fonts
            .font_data
            .insert("lucide".to_owned(), Arc::new(FontData::from_static(LUCIDE)));
        fonts
            .families
            .insert(FontFamily::Proportional, vec!["lucide".to_owned()]);
        fonts
            .families
            .insert(FontFamily::Monospace, vec!["lucide".to_owned()]);
        ctx.set_fonts(fonts);
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        let font = FontId::new(16.0, FontFamily::Proportional);
        // Control check: a code point outside the subset really is missing.
        assert!(!ctx.fonts_mut(|fonts| fonts.has_glyph(&font, '\u{E06D}')));
        for glyph in [
            icon::CHEVRON_DOWN,
            icon::CHEVRON_RIGHT,
            icon::CIRCLE_ALERT,
            icon::CIRCLE_CHECK,
            icon::CPU,
            icon::FILE_CHECK,
            icon::FILE_JSON,
            icon::FILE_PLUS,
            icon::FOLDER,
            icon::FOLDER_TREE,
            icon::HARD_DRIVE,
            icon::HISTORY,
            icon::INFO,
            icon::LANGUAGES,
            icon::CHECK,
            icon::MONITOR,
            icon::MOON,
            icon::SEARCH,
            icon::SHIELD_CHECK,
            icon::SUN,
            icon::TRIANGLE_ALERT,
            icon::X,
        ] {
            assert!(
                ctx.fonts_mut(|fonts| fonts.has_glyph(&font, glyph)),
                "U+{:04X} fehlt in lucide-subset.ttf",
                glyph as u32
            );
        }
    }

    /// The number field counts with ↑/↓ when focused and stays within its bounds.
    #[test]
    fn number_field_counts_and_clamps() {
        let ctx = egui::Context::default();
        install(&ctx);
        let id = Id::unique("zahl");
        let mut value = 255_usize;
        let run = |events: Vec<egui::Event>, value: &mut usize| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 300.0),
                )),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                number_field(ui, id, value, 1..=256, 72.0);
            });
            output.textures_delta.clear();
        };
        run(Vec::new(), &mut value);
        ctx.memory_mut(|mem| mem.request_focus(id));
        run(Vec::new(), &mut value);
        let up = egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        run(vec![up.clone()], &mut value);
        assert_eq!(value, 256);
        run(vec![up], &mut value);
        assert_eq!(value, 256, "not over the limit");
        // Typed: only digits count, a value that is too large counts as the bound.
        run(vec![egui::Event::Text("x".into())], &mut value);
        assert_eq!(value, 256);
    }

    /// The field is as large as ordered; icon and focus frame hang on this rectangle.
    #[test]
    fn text_field_reports_its_outer_frame() {
        let ctx = egui::Context::default();
        install(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let mut rect = egui::Rect::NOTHING;
        let mut value = String::new();
        let mut output = ctx.run_ui(input, |ui| {
            rect = text_field(
                ui,
                Id::unique("suche"),
                &mut value,
                "Suchen",
                240.0,
                Some(icon::SEARCH),
            )
            .rect;
        });
        output.textures_delta.clear();
        assert_eq!(rect.width(), 240.0, "{rect:?}");
        assert_eq!(rect.height(), CONTROL, "{rect:?}");
    }

    #[test]
    fn a_choice_list_is_as_wide_as_its_longest_label() {
        let ctx = egui::Context::default();
        install(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let (mut short, mut long, mut row) = (0.0, 0.0, egui::Rect::NOTHING);
        let mut output = ctx.run_ui(input, |ui| {
            short = choice_width(ui, ["Deutsch", "English"]);
            long = choice_width(ui, ["Deutsch", "English", "Deutsch und English"]);
            row = choice_row(ui, "Deutsch", true, short).rect;
        });
        output.textures_delta.clear();
        assert!(short < MENU_W, "kurze Liste schmaler als MENU_W: {short}");
        assert!(
            short > CHECK_COLUMN + 40.0,
            "Platz für Häkchen und Text: {short}"
        );
        assert!(long > short, "der längste Text bestimmt die Breite");
        assert_eq!(row.width(), short, "{row:?}");
    }

    #[test]
    fn semibold_family_lays_out() {
        let ctx = egui::Context::default();
        install(&ctx);
        ctx.set_theme(egui::Theme::Dark);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let mut title_width = 0.0;
        let mut icon_width = 0.0;
        let mut output = ctx.run_ui(input, |ui| {
            title_width = ui
                .ctx()
                .fonts_mut(|fonts| {
                    fonts.layout_no_wrap(
                        "Veritas".to_owned(),
                        FontId::new(text::TITLE, semibold()),
                        DARK.text,
                    )
                })
                .size()
                .x;
            icon_width = ui
                .ctx()
                .fonts_mut(|fonts| {
                    fonts.layout_no_wrap(
                        icon::FOLDER.to_string(),
                        FontId::new(16.0, FontFamily::Proportional),
                        DARK.text,
                    )
                })
                .size()
                .x;
        });
        output.textures_delta.clear();
        assert!(title_width > 20.0);
        assert!(icon_width > 0.0);
    }
}

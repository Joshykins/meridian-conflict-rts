//! The studio's look: the game's interface palette (near-black frosted panels,
//! thin white hairlines, one red-orange accent) and its Barlow fonts, so the
//! tool reads as part of Meridian Conflict rather than a stock egui window.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle,
};
use std::sync::Arc;

pub const BG0: Color32 = Color32::from_rgb(0x0B, 0x0B, 0x0D);
pub const BG1: Color32 = Color32::from_rgb(0x10, 0x10, 0x13);
pub const BG2: Color32 = Color32::from_rgb(0x16, 0x16, 0x1A);
pub const BG3: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x23);
pub const BG4: Color32 = Color32::from_rgb(0x28, 0x28, 0x2E);

pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF0);
pub const DIM: Color32 = Color32::from_rgb(0xA3, 0xA3, 0xA0);
pub const FAINT: Color32 = Color32::from_rgb(0x66, 0x66, 0x6A);

pub const ACCENT: Color32 = Color32::from_rgb(0xFF, 0x5A, 0x24);
pub const ACCENT_DEEP: Color32 = Color32::from_rgb(0xA8, 0x30, 0x0F);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xB4, 0x3C);
pub const BAD: Color32 = Color32::from_rgb(0xFF, 0x3B, 0x3B);
pub const GOOD: Color32 = Color32::from_rgb(0x6C, 0xD0, 0x8C);

/// White at a low alpha: the hairline every edge and grid line is drawn with.
pub fn line(alpha: u8) -> Color32 {
    Color32::from_white_alpha(alpha)
}

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Mixes `a` toward `b` by `t`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// A track's colour from its `0xRRGGBB`.
pub fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn to_rgb(c: Color32) -> u32 {
    ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | c.b() as u32
}

/// Colours offered for new tracks, cycling; muted so the accent still stands out.
pub const TRACK_COLOURS: [u32; 10] = [
    0xE0603A, 0xD9A441, 0x8FB85A, 0x4FB3A5, 0x4D8FD6, 0x7F6FD8, 0xC062B8, 0xD65A7A, 0x9A9A96,
    0x6FA0B8,
];

pub fn section_colour(kind: mc_music::SectionKind) -> Color32 {
    use mc_music::SectionKind::*;
    match kind {
        Intro => Color32::from_rgb(0x4D, 0x8F, 0xD6),
        Loop => Color32::from_rgb(0x8A, 0x8A, 0x90),
        Bridge => Color32::from_rgb(0x7F, 0x6F, 0xD8),
        Stinger => WARN,
        Ending => ACCENT,
    }
}

pub fn semi() -> FontFamily {
    FontFamily::Name("semi".into())
}
pub fn light() -> FontFamily {
    FontFamily::Name("light".into())
}

/// Names, figures and button text.
pub fn font_semi(size: f32) -> FontId {
    FontId::new(size, semi())
}
/// Body text.
pub fn font_body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
/// Large display figures.
pub fn font_light(size: f32) -> FontId {
    FontId::new(size, light())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "barlow".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../mc-render/assets/fonts/Barlow-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        "barlow-semi".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../mc-render/assets/fonts/BarlowSemiCondensed-SemiBold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "barlow-light".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../mc-render/assets/fonts/BarlowSemiCondensed-Light.ttf"
        ))),
    );
    // The stock fonts stay behind Barlow as fallbacks for arrows and symbols.
    let fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let family = |first: &str| {
        let mut v = vec![first.to_string()];
        v.extend(fallback.iter().cloned());
        v
    };
    fonts
        .families
        .insert(FontFamily::Proportional, family("barlow"));
    fonts.families.insert(semi(), family("barlow-semi"));
    fonts.families.insert(light(), family("barlow-light"));
    ctx.set_fonts(fonts);

    ctx.global_style_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, font_body(11.0)),
            (TextStyle::Body, font_body(13.5)),
            (TextStyle::Button, font_semi(14.0)),
            (TextStyle::Heading, font_light(22.0)),
            (
                TextStyle::Monospace,
                FontId::new(12.5, FontFamily::Monospace),
            ),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(7.0, 3.0);
        style.spacing.interact_size = egui::vec2(24.0, 20.0);
        style.spacing.combo_width = 90.0;
        style.spacing.window_margin = Margin::same(10);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.scroll.bar_width = 6.0;
        style.spacing.scroll.floating = true;
        style.interaction.tooltip_delay = 0.35;

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = None;
        v.panel_fill = BG1;
        v.window_fill = BG2;
        v.extreme_bg_color = BG0;
        v.faint_bg_color = BG2;
        v.code_bg_color = BG0;
        v.window_stroke = Stroke::new(1.0, line(30));
        v.window_corner_radius = CornerRadius::same(4);
        v.menu_corner_radius = CornerRadius::same(3);
        v.window_shadow = Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(140),
        };
        v.popup_shadow = Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(140),
        };
        v.selection.bg_fill = with_alpha(ACCENT, 90);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;
        v.warn_fg_color = WARN;
        v.error_fg_color = BAD;
        v.text_cursor.stroke = Stroke::new(1.5, ACCENT);
        v.striped = false;
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };

        let r = CornerRadius::same(2);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = BG2;
        w.noninteractive.weak_bg_fill = BG2;
        w.noninteractive.bg_stroke = Stroke::new(1.0, line(22));
        w.noninteractive.fg_stroke = Stroke::new(1.0, DIM);
        w.noninteractive.corner_radius = r;

        w.inactive.bg_fill = BG3;
        w.inactive.weak_bg_fill = BG3;
        w.inactive.bg_stroke = Stroke::new(1.0, line(18));
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = r;

        w.hovered.bg_fill = BG4;
        w.hovered.weak_bg_fill = BG4;
        w.hovered.bg_stroke = Stroke::new(1.0, line(60));
        w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 0.0;

        w.active.bg_fill = ACCENT_DEEP;
        w.active.weak_bg_fill = ACCENT_DEEP;
        w.active.bg_stroke = Stroke::new(1.0, ACCENT);
        w.active.fg_stroke = Stroke::new(1.0, TEXT);
        w.active.corner_radius = r;
        w.active.expansion = 0.0;

        w.open.bg_fill = BG4;
        w.open.weak_bg_fill = BG4;
        w.open.bg_stroke = Stroke::new(1.0, line(50));
        w.open.fg_stroke = Stroke::new(1.0, TEXT);
        w.open.corner_radius = r;
    });
}

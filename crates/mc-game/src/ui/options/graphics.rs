//! The preset and its optional manual resolution/AA overrides.

use super::*;
use crate::settings::Quality;

pub(super) fn draw(ui: &mut Ui, settings: &mut Settings, x: f32, width: f32, y: &mut f32) -> bool {
    let mut row = || {
        let r = Rect::new(x, *y, width, 46.0);
        *y += 48.0;
        r
    };
    let mut changed = ui.toggle(
        id("fullscreen", 0),
        row(),
        "Full Screen",
        "Borderless",
        &mut settings.fullscreen,
    );
    changed |= ui.toggle(
        id("vsync", 0),
        row(),
        "Vertical Sync",
        "",
        &mut settings.vsync,
    );
    changed |= preset(ui, settings, row());
    changed |= render_scale(ui, settings, row());
    changed |= antialiasing(ui, settings, row());
    changed
}

fn preset(ui: &mut Ui, settings: &mut Settings, r: Rect) -> bool {
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    let end = ui.text(
        r.x + 16.0,
        r.mid_y(),
        type_scale::BODY,
        rgb(palette::TEXT, 0.82),
        "Quality",
    );
    let note = if settings.quality_label() == "Custom" {
        format!("Based on {}", settings.quality.label())
    } else {
        settings.quality.description().to_owned()
    };
    ui.text(
        end + 12.0,
        r.mid_y(),
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        &note,
    );
    let step = ui.stepper(
        id("quality", 0),
        Rect::new(r.right() - 180.0, r.y + 7.0, 164.0, 32.0),
        settings.quality_label(),
        rgb(palette::TEXT, 1.0),
        true,
    );
    if step == 0 {
        return false;
    }
    let at = Quality::ALL
        .iter()
        .position(|&q| q == settings.quality)
        .unwrap_or(0) as i32;
    // The first click from Custom restores its base preset, even at an end.
    let at = if settings.quality_label() == "Custom" {
        at
    } else {
        (at + step).clamp(0, Quality::ALL.len() as i32 - 1)
    };
    settings.apply_quality(Quality::ALL[at as usize]);
    true
}

fn render_scale(ui: &mut Ui, settings: &mut Settings, r: Rect) -> bool {
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    let end = ui.text(
        r.x + 16.0,
        r.mid_y(),
        type_scale::BODY,
        rgb(palette::TEXT, 0.82),
        "Render Scale",
    );
    let note = match settings.render_scale {
        s if s > 1.0 => "Supersampled",
        s if s < 1.0 => "Faster, FSR upscaled",
        _ => "Native",
    };
    ui.text(
        end + 12.0,
        r.mid_y(),
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        note,
    );
    let step = ui.stepper(
        id("render-scale", 0),
        Rect::new(r.right() - 180.0, r.y + 9.0, 164.0, 32.0),
        &format!("{:.0}%", settings.render_scale * 100.0),
        rgb(palette::TEXT, 1.0),
        true,
    );
    if step != 0 {
        let scales = crate::settings::RENDER_SCALES;
        let at = scales
            .iter()
            .position(|&s| s == settings.render_scale)
            .unwrap_or(2) as i32;
        settings.render_scale = scales[(at + step).clamp(0, scales.len() as i32 - 1) as usize];
        return true;
    }
    false
}

fn antialiasing(ui: &mut Ui, settings: &mut Settings, r: Rect) -> bool {
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    ui.text(
        r.x + 16.0,
        r.mid_y(),
        type_scale::BODY,
        rgb(palette::TEXT, 0.82),
        "Anti-aliasing",
    );
    let step = ui.stepper(
        id("antialiasing", 0),
        Rect::new(r.right() - 180.0, r.y + 9.0, 164.0, 32.0),
        settings.antialiasing.label(),
        rgb(palette::TEXT, 1.0),
        true,
    );
    if step != 0 {
        let all = crate::settings::Antialiasing::ALL;
        let at = all
            .iter()
            .position(|&a| a == settings.antialiasing)
            .unwrap_or(1) as i32;
        settings.antialiasing = all[(at + step).clamp(0, all.len() as i32 - 1) as usize];
        return true;
    }

    false
}

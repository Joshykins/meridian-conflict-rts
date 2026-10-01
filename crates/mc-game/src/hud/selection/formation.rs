//! The formation panel over the order card: together or free, the spacing, the
//! block's shape (wider or longer, the steps the wheel takes on a held move) and
//! forming up where the selection stands.

use super::super::{Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::formation_drag::{shape_label, step_shape};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_sim::formations::SHAPES;

/// Height of the panel, and how far above the order card its top stands.
const PANEL_H: f32 = 176.0;
const PANEL_RISE: f32 = PANEL_H + 10.0;

pub(super) fn formation_panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, r: Rect) {
    let cw = r.w - 24.0;
    let panel = Rect::new(r.x, r.y - PANEL_RISE, r.w, PANEL_H);
    hud.glass(ui, panel);
    ui.section(
        panel.x + 12.0,
        panel.y + 18.0,
        panel.w - 24.0,
        "Formation \u{b7} Selection",
    );
    let choices = [
        (
            "Together",
            HudAction::FormationTogether(true),
            s.view.formation_together,
        ),
        (
            "Free Move",
            HudAction::FormationTogether(false),
            !s.view.formation_together,
        ),
    ];
    for (i, (label, action, lit)) in choices.into_iter().enumerate() {
        let button = Rect::new(
            panel.x + 12.0 + i as f32 * (cw + 4.0) / 2.0,
            panel.y + 29.0,
            (cw - 4.0) / 2.0,
            27.0,
        );
        labelled(
            hud,
            ui,
            id("formation-mode", i),
            button,
            lit,
            true,
            label,
            action,
        );
    }
    for (i, label) in ["Compact", "Standard", "Wide"].into_iter().enumerate() {
        let button = Rect::new(
            panel.x + 12.0 + i as f32 * (cw + 4.0) / 3.0,
            panel.y + 62.0,
            (cw - 8.0) / 3.0,
            25.0,
        );
        let lit = s.view.formation_spacing == i as u8;
        let action = HudAction::FormationSpacing(i as u8);
        labelled(
            hud,
            ui,
            id("formation-spacing", i),
            button,
            lit,
            true,
            label,
            action,
        );
    }
    shape_row(
        hud,
        ui,
        s.view.formation_shape,
        panel.x + 12.0,
        panel.y + 93.0,
        cw,
    );
    let button = Rect::new(panel.x + 12.0, panel.y + 124.0, cw, 26.0);
    labelled(
        hud,
        ui,
        id("form-up", 0),
        button,
        false,
        true,
        "Form UP Here",
        HudAction::FormUp,
    );
    ui.text(
        panel.x + 12.0,
        panel.y + 164.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "AIR: REPEATING Vs    LAND: BLOCKS",
    );
}

/// Longer, the shape as it stands (a click squares it), wider.
fn shape_row(hud: &mut Hud, ui: &mut Ui, shape: i8, x: f32, y: f32, w: f32) {
    let side = (w - 8.0) * 0.3;
    let longer = Rect::new(x, y, side, 25.0);
    let middle = Rect::new(x + side + 4.0, y, w - 2.0 * side - 8.0, 25.0);
    let wider = Rect::new(x + w - side, y, side, 25.0);
    let step = |n| HudAction::FormationShape(step_shape(shape, n));
    labelled(
        hud,
        ui,
        id("formation-shape", 0),
        longer,
        false,
        shape > -SHAPES,
        "Longer",
        step(-1),
    );
    labelled(
        hud,
        ui,
        id("formation-shape", 2),
        wider,
        false,
        shape < SHAPES,
        "Wider",
        step(1),
    );
    let t = hud.tile(ui, id("formation-shape", 1), middle, shape == 0, true);
    // A dozen dots stood the way the block will stand.
    let aspect = 2f32.powf(shape as f32 / 2.0);
    let cols = (12.0 * aspect).sqrt().ceil().clamp(1.0, 12.0) as usize;
    let ranks = 12usize.div_ceil(cols);
    let gap = (26.0 / cols.max(ranks) as f32).min(4.5);
    let glyph = Vec2::new(middle.x + 16.0, middle.mid_y());
    for i in 0..12 {
        let (rank, col) = ((i / cols) as f32, (i % cols) as f32);
        let at = glyph
            + Vec2::new(
                (col - (cols as f32 - 1.0) / 2.0) * gap,
                (rank - (ranks as f32 - 1.0) / 2.0) * gap,
            );
        ui.disc(at, (gap * 0.36).max(1.0), rgb(palette::TEXT, 0.85));
    }
    ui.text_centred(
        middle.x + middle.w * 0.5 + 12.0,
        middle.mid_y(),
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        &shape_label(shape),
    );
    if t.clicked && shape != 0 {
        hud.actions.push(HudAction::FormationShape(0));
        ui.audio.play(Sfx::Select);
    }
}

/// A tile with a label centred on it, giving `action` when clicked.
fn labelled(
    hud: &mut Hud,
    ui: &mut Ui,
    tile: crate::ui::Id,
    r: Rect,
    lit: bool,
    enabled: bool,
    label: &str,
    action: HudAction,
) {
    let t = hud.tile(ui, tile, r, lit, enabled);
    ui.text_centred(
        r.x + r.w * 0.5,
        r.mid_y(),
        type_scale::MICRO,
        rgb(palette::TEXT, if enabled { 1.0 } else { 0.4 }),
        label,
    );
    if t.clicked && enabled {
        hud.actions.push(action);
        ui.audio.play(Sfx::Select);
    }
}

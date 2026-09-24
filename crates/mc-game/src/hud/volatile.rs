//! Volatile units: the chip that marks them wherever a unit is described (build
//! tooltip, selection panel, hover card, DETAILS), and the DETAILS card that says what
//! their destruction does to everything round them.

use super::selection::wrap_text;
use super::whole;
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::UnitBlueprint;

/// A warning, not a selection: the warning amber, not the accent.
const HAZARD: u32 = palette::WARN;

const NOTE: &str = "Goes up when destroyed. Every unit in reach takes the blast, its \
    owner's too, and a plant it kills can go up in turn. One still being built, or \
    being taken apart, does not.";

/// The "Volatile" chip, its left edge at `x` and centred on `y`, if the unit is
/// volatile. Returns the x after it (`x` itself when nothing was drawn).
pub fn chip(ui: &mut Ui, bp: &UnitBlueprint, x: f32, y: f32) -> f32 {
    if !bp.volatile() {
        return x;
    }
    let label = "Volatile";
    let w = ui.text_width(type_scale::MICRO, label) + 26.0;
    let r = Rect::new(x, y - 7.5, w, 15.0);
    ui.fill(r, rgb(HAZARD, 0.16));
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(HAZARD, 1.0));
    // A small warning triangle with its mark.
    let c = Vec2::new(r.x + 11.0, y + 0.5);
    let tone = rgb(HAZARD, 1.0);
    let (top, left, right) = (c + Vec2::new(0.0, -5.0), c + Vec2::new(-5.0, 4.0), c + Vec2::new(5.0, 4.0));
    ui.stroke(top, left, 1.2, tone);
    ui.stroke(left, right, 1.2, tone);
    ui.stroke(right, top, 1.2, tone);
    ui.stroke(c + Vec2::new(0.0, -2.0), c + Vec2::new(0.0, 1.0), 1.2, tone);
    ui.fill(Rect::new(c.x - 0.6, c.y + 2.0, 1.2, 1.2), tone);
    ui.text(r.x + 20.0, y, type_scale::MICRO, tone, label);
    r.right()
}

/// Height [`destruction`] takes in a card `cw` wide.
pub fn destruction_h(ui: &mut Ui, bp: &UnitBlueprint, cw: f32) -> f32 {
    if !bp.volatile() {
        return 0.0;
    }
    let lines = wrap_text(ui, type_scale::MICRO, NOTE, cw - 24.0).len();
    18.0 + 40.0 + lines as f32 * 15.0 + 2.0 * 22.0 + 12.0 + 10.0
}

/// What the unit's destruction does: its blast's damage and reach, and how it falls
/// off. Returns the y below it.
pub fn destruction(ui: &mut Ui, bp: &UnitBlueprint, x: f32, y: f32, cw: f32) -> f32 {
    let Some(db) = bp.death_blast else {
        return y;
    };
    ui.section(x, y, cw, "On Destruction");
    let top = y + 16.0;
    let h = destruction_h(ui, bp, cw) - 18.0 - 10.0;
    let r = Rect::new(x, top, cw, h);
    ui.fill_cut(r, 6.0, rgb(HAZARD, 0.06));
    ui.bevel(r, 6.0, 0.5);
    ui.fill(Rect::new(r.x + 1.0, r.y + 8.0, 3.0, r.h - 16.0), rgb(HAZARD, 1.0));
    let (ix, iw) = (r.x + 12.0, r.w - 24.0);
    ui.text(ix, r.y + 14.0, type_scale::CAPTION, rgb(HAZARD, 1.0), "Detonation");
    let mut ly = r.y + 34.0;
    for line in wrap_text(ui, type_scale::MICRO, NOTE, iw) {
        ui.text(ix, ly, type_scale::MICRO, rgb(palette::DIM, 1.0), &line);
        ly += 15.0;
    }
    let (damage, radius) = (db.damage.to_f32(), db.radius.to_f32());
    let half = (iw - 16.0) * 0.5;
    let rows = [
        [
            ("Damage", whole(damage), damage / 6000.0),
            ("Blast Radius", format!("{radius:.0} m"), radius / 150.0),
        ],
        [
            ("Full Damage To", format!("{:.0} m", radius * 0.5), 0.5 * radius / 150.0),
            ("At the Edge", whole(damage * 0.25), damage * 0.25 / 6000.0),
        ],
    ];
    ly += 6.0;
    for row in rows {
        for (i, (label, value, share)) in row.into_iter().enumerate() {
            super::selection::gauge(ui, ix + i as f32 * (half + 16.0), ly, half, label, &value, share, HAZARD);
        }
        ly += 22.0;
    }
    r.bottom() + 10.0
}

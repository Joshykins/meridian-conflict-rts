//! The Progenitor's hard-light bridge (`mc_map::PropKind::PrecursorBridge`), built to
//! `mc_map::landmark::HARDLIGHT_BRIDGE`: a road of hard light from the origin along +x
//! for the plan's span, the plan's half either side, its walking surface at the
//! origin's height (the map raises each landing to it). The map's deck
//! (`mc_map::deck`) is that road, so units cross on it and ships sail under it.
//! Nothing reaches down into the water: the light holds it.
//!
//! The road itself is not modelled: it is hard light, drawn by the renderer over the
//! map's deck (`progenitor_fx`'s deck pieces, progenitor_fx.wgsl), clear enough to see
//! the sea through, its sides thin rails of light. Here is what projects it: at each
//! landing a platform of pale alloy plates on a dark core, flush with the road, a
//! dark projector along its lip. Nothing stands over the span.
//!
//! Model space as for every prop: x along the heading, y left, z up, the origin on
//! the ground (the near landing).

use std::f32::consts::PI;

use glam::Affine3A;

use super::builder::{MeshBuilder, Section};
use super::library::ModelDef;
use super::precursor::{cut_rect, dark, pale, panel, v3};
use mc_map::landmark::HARDLIGHT_BRIDGE as PLAN;

pub(super) const MODELS: &[ModelDef] = &[ModelDef::new(
    "precursor_bridge",
    RADIUS,
    PLATFORM.2,
    bridge,
)];

/// The span and half its width, as the map lays the deck.
const SPAN: f32 = PLAN.span as f32;
const HALF: f32 = PLAN.half as f32;
/// The authored radius: the half diagonal, platforms included.
const RADIUS: f32 = 940.0;
/// A platform: how far inland of the deck's end it runs (inside the level landing
/// the map raises, `rimehold/shape.rs`, at any bridge's scale), how far past the
/// road either side, its top over the road (a step too low to see a unit sink) and
/// its foot.
const PLATFORM: (f32, f32, f32, f32) = (90.0, 8.0, 0.25, -30.0);
/// A platform's plates: the dark border left round them, and the joint between.
const PLATES: (f32, f32) = (5.0, 2.5);
/// The projector across a platform's lip, metres inland.
const LIP: f32 = 6.0;

/// A platform in its own frame: the road leaves its lip at x = 0 along +x, the
/// platform inland toward -x.
fn platform(b: &mut MeshBuilder) {
    let (depth, past, top, foot) = PLATFORM;
    let half_y = HALF + past;
    let lip = LIP;

    // The slab's dark core, its corners cut, buried well below the landing.
    dark(b);
    let slab = cut_rect(b, (depth - lip) * 0.5, half_y, 10.0);
    let back = -(depth + lip) * 0.5;
    b.loft_z(
        &slab,
        &[
            Section::new(foot, 1.0).shifted(back, 0.0),
            Section::new(top, 1.0).shifted(back, 0.0),
        ],
    );
    // Pale plates laid on it, a dark joint between each and round the edge, so the
    // platform reads on snow as well as on rock.
    pale(b);
    let (rows, cols) = match b.lod() {
        0 => (2, 6),
        1 => (1, 3),
        _ => (1, 2),
    };
    let (border, joint) = PLATES;
    let (x0, x1) = (-depth + border, -lip - joint);
    let (y0, y1) = (-half_y + border, half_y - border);
    let plate_x = (x1 - x0 - joint * (rows - 1) as f32) / rows as f32;
    let plate_y = (y1 - y0 - joint * (cols - 1) as f32) / cols as f32;
    for r in 0..rows {
        for c in 0..cols {
            let px = x0 + r as f32 * (plate_x + joint);
            let py = y0 + c as f32 * (plate_y + joint);
            panel(
                b,
                &[
                    v3(px, py, top),
                    v3(px + plate_x, py, top),
                    v3(px + plate_x, py + plate_y, top),
                    v3(px, py + plate_y, top),
                ],
                v3(0.0, 0.0, 1.0),
                0.12,
                0.5,
            );
        }
    }

    // The projector: a dark bar across the lip, flush with the slab. The light it
    // projects, the road's bright edge along it, is the renderer's (the deck's).
    dark(b);
    b.chamfered_box(
        v3(-lip * 0.5, 0.0, (top + foot) * 0.5),
        v3(lip, half_y * 2.0, top - foot),
        0.0,
    );
}

/// A platform at each landing, the far one turned back along the road.
fn bridge(b: &mut MeshBuilder, _tech: u8) {
    platform(b);
    let far = Affine3A::from_translation(v3(SPAN, 0.0, 0.0)) * Affine3A::from_rotation_z(PI);
    b.with(far, platform);
}

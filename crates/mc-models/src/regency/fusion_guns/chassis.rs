//! The body the tech 3 fusion guns ride, up to the deck their turret turns on, built to a
//! [`Hull`]'s size: a low faceted hull on two tracks hidden under lapped armour skirts swept
//! back into points, bronze road wheels showing under them.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates over bronze machinery, red
//! optic slits at the nose, no violet (they do not build).

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{red_slot, Course, Frame};

/// A body's size: the nose's x, the tail's (negative), half its width over everything,
/// and the deck its turret turns on.
#[derive(Clone, Copy, Debug)]
pub(super) struct Hull {
    pub(super) nose: f32,
    pub(super) tail: f32,
    pub(super) half: f32,
    pub(super) deck: f32,
}

impl Hull {
    /// How much bigger than the Spire's body this one is: plates thicken with it.
    fn k(&self) -> f32 {
        self.half / 3.4
    }
}

/// The core's outline in plan: a pointed nose, flanks drawn in toward a blunt tail.
fn core_plan(nose: f32, tail: f32, w: f32) -> Vec<[f32; 2]> {
    vec![
        [nose, 0.0],
        [nose * 0.55, w * 0.62],
        [nose * 0.05, w],
        [tail * 0.7, w * 0.92],
        [tail, w * 0.55],
        [tail, -w * 0.55],
        [tail * 0.7, -w * 0.92],
        [nose * 0.05, -w],
        [nose * 0.55, -w * 0.62],
    ]
}

/// Far off: a wedge of a body with its running gear under it as one face.
fn coarse(b: &mut MeshBuilder, h: &Hull, floor: f32) {
    dark_plate(b);
    let w = h.half * 0.9;
    let plan = [
        [h.nose, w * 0.45],
        [h.tail, w],
        [h.tail, -w],
        [h.nose, -w * 0.45],
    ];
    b.loft_z(
        &plan,
        &[Section::new(floor, 1.0), Section::new(h.deck, 0.85)],
    );
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[
            v3(h.nose * 0.7, 0.0, floor - 0.05),
            v3(h.tail * 0.9, -w * 0.8, floor - 0.05),
            v3(h.tail * 0.9, w * 0.8, floor - 0.05),
        ]);
    });
}

/// The nose's optics: two pairs of red slits on its flanks.
fn optics(b: &mut MeshBuilder, x: f32, y: f32, z: f32) {
    b.mirror_y(|b| {
        red_slot(b, v3(x, y, z), v3(0.7, 0.7, 0.0), Vec3::Y, 0.4, 0.14);
        if b.fine() {
            red_slot(
                b,
                v3(x - 0.9, y + 0.5, z + 0.1),
                v3(0.5, 0.86, 0.0),
                Vec3::Y,
                0.26,
                0.1,
            );
        }
    });
}

/// The owner's colour: a chevron on the back deck behind the turret.
fn chevron(b: &mut MeshBuilder, x: f32, w: f32, z: f32) {
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            v3(x, 0.0, z),
            v3(x - w, w, z),
            v3(x - w * 1.4, w * 0.8, z),
            v3(x - w * 0.4, 0.0, z),
        ])
    });
}

/// The body: a low hull on two tracks under lapped armour skirts, up to the deck.
pub(super) fn body(b: &mut MeshBuilder, h: &Hull) {
    let (inner, outer) = (h.half * 0.55, h.half * 0.95);
    let track_h = h.deck * 0.5;
    let (x0, x1) = (h.tail * 0.92, h.nose * 0.8);
    b.set_treads((inner + outer) * 0.5, outer - inner, x0);
    b.set_dust_line(track_h);
    if b.coarse() {
        coarse(b, h, track_h * 0.4);
        return;
    }
    b.mirror_y(|b| track(b, x0, x1, inner, outer, track_h));
    let core = core_plan(h.nose, h.tail, h.half * 0.6);
    seam(b);
    b.loft_z(
        &core,
        &[
            Section::scaled(track_h * 0.3, 0.9, 0.75),
            Section::scaled(track_h * 0.75, 0.95, 0.95),
        ],
    );
    dark_plate(b);
    b.loft_z(
        &core,
        &[
            Section::new(track_h * 0.75, 0.97),
            Section::scaled(h.deck - 0.4, 1.0, 1.25),
            Section::scaled(h.deck, 0.86, 1.1).shifted(-0.2, 0.0),
        ],
    );
    optics(b, h.nose * 0.66, h.half * 0.28, track_h * 1.15);
    chevron(b, h.tail * 0.55, h.half * 0.28, h.deck + 0.01);
    b.mirror_y(|b| skirt(b, h, outer, track_h));
}

/// One track on the +y side, from `x0` to `x1`, `inner` to `outer` across: a belt round
/// its ends, bronze road wheels showing under the skirt.
fn track(b: &mut MeshBuilder, x0: f32, x1: f32, inner: f32, outer: f32, th: f32) {
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        let lozenge = [
            [x0 + 0.5 * th, 0.0],
            [x1 - 0.5 * th, 0.0],
            [x1, 0.5 * th],
            [x1 - 0.25 * th, th],
            [x0 + 0.25 * th, th],
            [x0, 0.5 * th],
        ];
        b.extrude_y(&lozenge, inner, outer);
        if b.fine() {
            metal(b);
            let n = 5;
            for i in 0..n {
                let x = x0 + 0.6 * th + (x1 - x0 - 1.2 * th) * i as f32 / (n - 1) as f32;
                b.cylinder_between(
                    v3(x, outer, 0.3 * th),
                    v3(x, outer + 0.08, 0.3 * th),
                    0.24 * th,
                    0.2 * th,
                    8,
                );
            }
        }
    });
}

/// The skirt on the +y side: a course of armour plates hung down over the track's upper
/// run, lapped back like feathers, the last swept back past the tail into a point.
fn skirt(b: &mut MeshBuilder, h: &Hull, outer: f32, th: f32) {
    let len = (h.nose * 0.72 - h.tail) / 2.5;
    dark_plate(b);
    Course {
        count: if b.fine() { 3 } else { 2 },
        step: len * 0.78,
        len,
        half: th * 0.42,
        tip: 0.7,
        thick: 0.24 * h.k(),
        tail: len * 0.3,
    }
    .lay(
        b,
        &Frame::new(
            v3(h.nose * 0.72, outer + 0.06, th * 0.92),
            v3(-1.0, 0.0, 0.0),
            v3(0.0, 1.0, 0.2),
        ),
    );
    if b.fine() {
        red_slot(
            b,
            v3(h.nose * 0.55, outer + 0.32 * h.k(), th * 1.12),
            Vec3::Y,
            Vec3::X,
            1.0 * h.k(),
            0.1,
        );
    }
}

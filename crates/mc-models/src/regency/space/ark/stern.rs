//! The +y side's stern and lift: the canted fin, the drive cluster across the stern
//! (a big gravity drive and a small one under it, in a plated cowl swept back into
//! spikes), and the gravity lifts slung under the wing and the hull.
use glam::{Vec2, Vec3};

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::lift::bell;
use super::super::super::machine::{red_slot, Frame};
use super::shard;
use crate::builder::MeshBuilder;
use crate::material::GLOW_LASER;

/// The fin, canted out: three plates lapped up it, the top one swept highest, its
/// leading edge lit.
pub(super) fn fin(b: &mut MeshBuilder) {
    let f = Frame::new(
        v3(-62.0, 13.0, 87.0),
        v3(-1.0, 0.18, 0.5),
        v3(0.0, 1.0, -0.3),
    );
    shard(b, &f, 72.0, 8.0, 0.8, 2.4);
    if !b.mid() {
        return;
    }
    let g = Frame::new(
        v3(-90.0, 15.0, 86.0),
        v3(-1.0, 0.25, 0.32),
        v3(0.0, 1.0, -0.3),
    );
    shard(b, &g, 62.0, 6.0, -0.6, 1.8);
    if b.fine() {
        let h = Frame::new(
            v3(-74.0, 15.6, 87.0),
            v3(-1.0, 0.2, 0.62),
            v3(0.0, 1.0, -0.3),
        );
        shard(b, &h, 40.0, 4.0, 0.9, 1.2);
        // The leading edge: from the root up the fin's front to its spike.
        let root = f.at(0.0, 8.0, 1.4);
        let tip = f.at(72.0 * 0.35, 8.0, 1.4);
        b.paint(GLOW_LASER);
        b.cylinder_between(root, tip, 0.3, 0.3, 4);
    }
}

/// The drives: a big one high on the stern quarter and a small one outboard under it, in
/// one cowl, plates swept back off it into spikes past the mouths.
pub(super) fn drives(b: &mut MeshBuilder) {
    drive(b, v3(-150.0, 17.0, 63.0), 8.5, 20.0);
    drive(b, v3(-146.0, 31.0, 53.0), 5.0, 14.0);
    if !b.mid() {
        return;
    }
    let f = Frame::new(
        v3(-128.0, 22.0, 73.5),
        v3(-1.0, 0.08, 0.02),
        v3(0.0, 0.3, 1.0),
    );
    shard(b, &f, 34.0, 7.0, 0.5, 1.6);
    let g = Frame::new(
        v3(-130.0, 37.0, 54.0),
        v3(-1.0, 0.18, 0.0),
        v3(0.0, 1.0, 0.1),
    );
    shard(b, &g, 26.0, 4.0, -0.4, 1.2);
}

/// A gravity drive: a plated cowl round a graphite ring, its throat red, and a cone of
/// machinery in the throat.
fn drive(b: &mut MeshBuilder, mouth: Vec3, r: f32, len: f32) {
    let sides = b.sides(12);
    dark_plate(b);
    b.with_facets(|b| {
        b.cylinder_between(mouth + Vec3::X * len, mouth, r * 0.8, r * 1.1, sides);
    });
    metal(b);
    b.cylinder_between(mouth, mouth - Vec3::X * 1.2, r * 1.0, r * 0.95, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(
        mouth - Vec3::X * 0.4,
        mouth - Vec3::X * 0.5,
        r * 0.62,
        r * 0.62,
        sides,
    );
    if b.fine() {
        metal(b);
        b.cylinder_between(
            mouth - Vec3::X * 0.5,
            mouth - Vec3::X * 2.6,
            r * 0.32,
            r * 0.16,
            8,
        );
        // Ribs round the cowl.
        seam(b);
        for k in [0.3, 0.65] {
            let c = mouth + Vec3::X * (len * k);
            let rr = r * (1.1 - 0.3 * k) + 0.25;
            b.cylinder_between(c, c + Vec3::X * 1.2, rr, rr, sides);
        }
    }
}

/// The gravity lifts: two under the wing, two under the hull, each a plated nacelle
/// with a lit seam and the lift bell hung in it.
pub(super) fn lifts(b: &mut MeshBuilder) {
    for (at, r, drop) in [
        (v3(-40.0, 60.0, 50.5), 7.0, 8.0),
        (v3(-96.0, 72.0, 54.0), 5.0, 6.0),
        (v3(40.0, 33.0, 31.0), 6.0, 6.0),
        (v3(-124.0, 32.0, 40.0), 6.0, 8.0),
    ] {
        lift(b, at, r, drop);
    }
}

/// A lift slung under the hull: a plated nacelle hung from `at` (its top) and the bell
/// under it, mouth `drop` lower; a lit seam round the nacelle and two plates swept back
/// off its flanks.
fn lift(b: &mut MeshBuilder, at: Vec3, r: f32, drop: f32) {
    let mouth = at - Vec3::Z * drop;
    if b.mid() {
        let sides = b.sides(8);
        dark_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(mouth + Vec3::Z * (r * 0.5), at, r * 1.35, r * 0.9, sides)
        });
        if b.fine() {
            let seam_at = mouth + Vec3::Z * (r * 0.5 + drop * 0.35);
            b.paint(GLOW_LASER);
            b.cylinder_between(seam_at, seam_at + Vec3::Z * 0.3, r * 1.25, r * 1.25, sides);
            for side in [1.0, -1.0] {
                let o = mouth + v3(r * 0.4, side * r * 1.2, r * 0.5 + drop * 0.5);
                let f = Frame::new(o, v3(-1.0, side * 0.3, 0.1), v3(0.0, side, 0.0));
                shard(b, &f, r * 3.0, r * 0.45, 0.0, 0.6);
            }
            red_slot(
                b,
                mouth + v3(-r * 1.3, 0.0, r * 0.8),
                -Vec3::X,
                Vec3::Y,
                r * 0.9,
                0.4,
            );
        }
        metal(b);
        b.beam(
            mouth + v3(r * 1.1, 0.0, r * 0.3),
            mouth + v3(-r * 1.1, 0.0, r * 0.3),
            Vec2::new(0.8, 1.0),
            Vec2::new(0.8, 1.0),
        );
    }
    bell(b, mouth, r, r * 0.6);
}

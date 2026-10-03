//! Design A (`fabricator~a`), the drum: a stepped foundation with wedge feet off the
//! diagonals carrying heat sinks, an octagonal body, and on it the indexing drum, four
//! cassettes of matter round a squat spindle; a cross-head on the spindle stamps the
//! cassettes and the drum turns a quarter (`part::FAB_PRESS`, `part::FAB_INDEX`).
//! - Tech 2: all of that, the spindle capped, lamps under the cap.
//! - Tech 3: a finned condenser stage on the spindle under a higher cap, capacitor
//!   bastions on the grid sides (±x).

use glam::Vec3;

use super::super::parts::*;
use super::*;
use crate::builder::ngon;
use crate::part;

/// The body: foot, top, radius at foot and top.
const BODY: (f32, f32, f32, f32) = (2.3, 5.2, 7.2, 6.2);
/// The drum: foot, top, core radius; a cassette's middle out from the axis and its size.
const DRUM: (f32, f32, f32) = (5.6, 9.4, 5.0);
const CASSETTE: (f32, Vec3) = (5.6, Vec3::new(2.4, 3.0, 3.4));
/// The spindle's half width and the top of its column; the cross-head's height.
const SPINDLE: (f32, f32) = (2.4, 13.4);
const HEAD: f32 = 11.0;

pub(in crate::aster) fn build(b: &mut MeshBuilder, tech: u8) {
    plinth(b);
    step(b, 9.0, 4.4, DECK, BODY.0);
    if !b.coarse() {
        diagonals(b, foot);
    }
    body(b);
    b.with_part(part::FAB_INDEX, drum);
    spindle(b);
    b.with_part(part::FAB_PRESS, cross_head);

    tech_3(b, tech, 0.2, |b| {
        b.radial(2, |b| bastion(b, v3(10.3, 0.0, DECK), 5.0, 1.6))
    });
    tech_3(b, tech, 0.5, |b| condenser(b, 14.2, 2.0, 22.0));
}

/// A foot on the diagonal: a wedge splaying down off the body, a heat sink laid along
/// its back.
fn foot(b: &mut MeshBuilder) {
    let (x0, x1, h0, h1) = (6.0, 13.2, 4.6, 1.2);
    b.paint(PLATING);
    wedge(b, x0, x1, DECK, v2(3.4, 2.6), v2(h0, h1));
    let slope = (h0 - h1) / (x1 - x0);
    let at = 9.4;
    let z = DECK + h0 - slope * (at - x0);
    b.pitched(v3(at, 0.0, z), -slope.atan(), |b| {
        heat_sink(b, Vec3::ZERO, Vec3::X, (x1 - at - 0.3) * 1.1, 2.2)
    });
}

/// The octagonal body off the step, a dark band at its head.
fn body(b: &mut MeshBuilder) {
    let (z0, z1, r0, r1) = BODY;
    let sides = if b.coarse() { 4 } else { 8 };
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, z0), sides, r0, r1, z1 - z0);
    if b.coarse() {
        return;
    }
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, z1), sides, r1 * 1.02, r1 * 0.98, DRUM.0 - z1);
}

/// The indexing drum: a dark octagonal core, a cassette on each axis with a window onto
/// the matter in it. It repeats every quarter turn.
fn drum(b: &mut MeshBuilder) {
    let (z0, z1, r) = DRUM;
    if b.coarse() {
        return;
    }
    let plan = ngon(8, 1.0);
    b.paint(ACCENT);
    b.loft_z(&plan, &[Section::new(z0, r), Section::new(z1, r * 0.96)]);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(z1, r * 0.98), Section::new(z1 + 0.4, r * 0.9)],
    );
    let (out, size) = CASSETTE;
    let mid = z0 + size.z * 0.5 + 0.2;
    b.radial(4, |b| {
        b.paint(PLATING);
        if b.fine() {
            b.chamfered_box(v3(out, 0.0, mid), size, 0.35);
        } else {
            b.cuboid(v3(out, 0.0, mid), size);
        }
        if b.fine() {
            // A lid, and ribs either side of the window.
            b.paint(ACCENT);
            b.chamfered_box(
                v3(out, 0.0, mid + size.z * 0.5 + 0.1),
                v3(size.x * 0.8, size.y * 0.8, 0.2),
                0.15,
            );
            for y in [-0.85, 0.85] {
                b.cuboid(
                    v3(out + size.x * 0.5 + 0.06, y, mid),
                    v3(0.12, 0.3, size.z * 0.8),
                );
            }
        }
        matter(
            b,
            v3(out + size.x * 0.5 + 0.03, 0.0, mid + 0.4),
            v3(0.08, 1.1, 0.55),
        );
    });
}

/// The spindle up through the drum, its dark cap, the vent on it, lamps under the cap.
fn spindle(b: &mut MeshBuilder) {
    let (h, top) = SPINDLE;
    b.paint(PLATING);
    if b.coarse() {
        b.cuboid_open(
            v3(0.0, 0.0, (DRUM.1 + top) * 0.5),
            v3(h * 2.0, h * 2.0, top - DRUM.1),
        );
        return;
    }
    let plan = chamfered_rect(v2(h, h), 0.7);
    b.loft_z(&plan, &[Section::new(DRUM.0, 1.0), Section::new(top, 0.94)]);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(top, 1.12), Section::new(top + 0.8, 1.0)],
    );
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, top + 0.8), 8, 0.9, 0.55, 15.95 - top - 0.8);
    b.radial(4, |b| {
        lamp(b, v3(h * 1.06 + 0.03, 0.0, top + 0.4), v3(0.1, 1.2, 0.35))
    });
}

/// The cross-head on the spindle: a dark collar and an arm out over each cassette, a
/// stamp under each arm's end.
fn cross_head(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    let (out, _) = CASSETTE;
    b.paint(ACCENT);
    block(b, v3(0.0, 0.0, HEAD + 0.3), v3(6.0, 6.0, 1.6), 1.0);
    b.radial(4, |b| {
        b.paint(PLATING);
        b.beam(
            v3(2.8, 0.0, HEAD + 0.4),
            v3(out + 0.6, 0.0, HEAD + 0.4),
            v2(1.4, 1.2),
            v2(1.2, 1.0),
        );
        b.paint(ACCENT);
        b.cuboid(v3(out, 0.0, HEAD - 0.6), v3(1.7, 1.7, 1.0));
    });
}

/// Tech 3's condenser on the spindle's cap at `z`: a finned square column `half` across,
/// its own cap and vent up to `top`.
fn condenser(b: &mut MeshBuilder, z: f32, half: f32, top: f32) {
    let cap = top - 2.4;
    b.paint(PLATING);
    if b.coarse() {
        b.cuboid_open(
            v3(0.0, 0.0, (z + cap) * 0.5),
            v3(half * 2.0, half * 2.0, cap - z),
        );
        return;
    }
    let plan = chamfered_rect(v2(half, half), 0.6);
    b.loft_z(&plan, &[Section::new(z, 1.0), Section::new(cap, 0.92)]);
    b.radial(4, |b| {
        b.paint(ACCENT);
        b.beam(
            v3(half, 0.0, z + 0.4),
            v3(half * 0.92, 0.0, cap - 0.4),
            v2(0.4, 1.6),
            v2(0.4, 1.2),
        );
    });
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(cap, 1.15), Section::new(cap + 1.0, 1.0)],
    );
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, cap + 1.0), 8, 0.8, 0.5, top - cap - 1.05);
}

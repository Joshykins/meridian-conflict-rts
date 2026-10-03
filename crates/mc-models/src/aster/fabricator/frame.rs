//! Design B (`fabricator~b`), the frame: a heavy bed with heat sinks down the grid sides
//! (±x), four upright posts on its corners tied by a flat top frame, and between them a
//! cross-head with four rams stamping the molds on a turning table: the matter shows in
//! the molds (`part::FAB_PRESS`, `part::FAB_INDEX`).
//! - Tech 2: all of that, lamps on the post caps.
//! - Tech 3: the posts carried up to a second, fixed head with an accumulator on it.

use glam::Vec3;

use super::super::parts::*;
use super::*;
use crate::builder::ngon;
use crate::part;

/// The bed: half width, corner cut, top; the table on it.
const BED: (f32, f32, f32) = (8.4, 2.4, 4.6);
const TABLE: f32 = BED.2 + 0.5;
/// A post's place on its corner, half width, and the top of its column.
const POST: (f32, f32, f32) = (5.4, 0.95, 14.6);
/// The molds: how far out on the axes, half width, top.
const MOLD: (f32, f32, f32) = (3.0, 0.7, TABLE + 1.2);
/// The cross-head plate's foot and top; the rams reach down to `RAM`.
const HEAD: (f32, f32) = (11.0, 12.4);
const RAM: f32 = 7.0;

pub(in crate::aster) fn build(b: &mut MeshBuilder, tech: u8) {
    plinth(b);
    step(b, BED.0, BED.1, DECK, BED.2);
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, 10.25), v3(12.8, 12.8, 10.3));
        tech_3(b, tech, 0.4, |b| {
            b.cuboid_open(v3(0.0, 0.0, 18.4), v3(12.8, 12.8, 6.0))
        });
        return;
    }
    b.radial(2, |b| heat_sink(b, v3(9.9, -4.6, DECK), Vec3::Y, 9.2, 2.2));
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(v2(BED.0, BED.0), BED.1),
        &[Section::new(BED.2, 0.92), Section::new(TABLE, 0.9)],
    );
    b.with_part(part::FAB_INDEX, table);
    b.radial(4, post);
    top_frame(b, POST.2 + 0.4);
    b.with_part(part::FAB_PRESS, cross_head);

    tech_3(b, tech, 0.4, |b| b.radial(4, upper_post));
    tech_3(b, tech, 0.7, |b| {
        top_frame(b, 20.0);
        b.paint(PLATING);
        b.loft_z(
            &chamfered_rect(v2(4.4, 4.4), 1.2),
            &[Section::new(20.4, 1.0), Section::new(21.2, 0.96)],
        );
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, 21.2), 8, 2.4, 1.6, 0.75);
    });
}

/// The turning table: a dark octagonal disc, a mold on each axis with the matter in it.
/// It repeats every quarter turn.
fn table(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.loft_z(
        &ngon(8, 1.0),
        &[Section::new(TABLE, 4.5), Section::new(TABLE + 0.6, 4.3)],
    );
    let (out, h, top) = MOLD;
    b.radial(4, |b| {
        b.paint(PLATING);
        block(
            b,
            v3(out, 0.0, (TABLE + 0.6 + top) * 0.5),
            v3(h * 2.0, h * 2.0, top - TABLE - 0.6),
            0.2,
        );
        matter(b, v3(out, 0.0, top + 0.03), v3(h * 1.4, h * 1.4, 0.06));
    });
}

/// A post on the corner (+x, +y): an upright plated column banded and bolted down, a dark
/// cap with a lamp on its outer face, a nub on top.
fn post(b: &mut MeshBuilder) {
    let (at, h, top) = POST;
    b.paint(PLATING);
    block(
        b,
        v3(at, at, (TABLE + top) * 0.5),
        v3(h * 2.0, h * 2.0, top - TABLE),
        0.3,
    );
    if b.fine() {
        // Clamp bands up the column, and the foot it is bolted down with.
        b.paint(ACCENT);
        for z in [TABLE + 0.5, 8.6, 12.9] {
            b.chamfered_box(v3(at, at, z), v3(h * 2.3, h * 2.3, 0.5), 0.3);
        }
        b.chamfered_box(v3(at, at, BED.2 + 0.25), v3(h * 3.0, h * 3.0, 0.5), 0.5);
    }
    b.paint(ACCENT);
    block(b, v3(at, at, top + 0.4), v3(h * 2.5, h * 2.5, 0.8), 0.3);
    b.paint(METAL);
    b.prism(v3(at, at, top + 0.8), 8, 0.5, 0.35, 15.95 - top - 0.8);
    lamp(
        b,
        v3(at + h * 1.25 + 0.04, at, top + 0.4),
        v3(0.1, 1.0, 0.4),
    );
}

/// Tech 3's post on the corner, from the cap up to the second head.
fn upper_post(b: &mut MeshBuilder) {
    let (at, h, top) = POST;
    b.paint(PLATING);
    block(
        b,
        v3(at, at, (top + 0.8 + 20.4) * 0.5),
        v3(h * 1.7, h * 1.7, 20.4 - top - 0.8),
        0.25,
    );
}

/// A flat top frame at `z`, tying the posts' heads: a beam along each side and a
/// cylinder housing in the middle on a cross of beams.
fn top_frame(b: &mut MeshBuilder, z: f32) {
    let at = POST.0;
    b.paint(ACCENT);
    b.radial(4, |b| {
        b.beam(
            v3(at, -at + 0.9, z),
            v3(at, at - 0.9, z),
            v2(1.0, 0.8),
            v2(1.0, 0.8),
        );
        b.beam(
            v3(at - 0.6, at - 0.6, z),
            v3(1.0, 1.0, z),
            v2(0.9, 0.7),
            v2(0.9, 0.7),
        );
    });
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, z - 1.4), 8, 1.5, 1.4, 1.9);
}

/// The cross-head: a plate sliding on the posts, a ram down over each mold, the piston
/// rod up into the housing in the top frame.
fn cross_head(b: &mut MeshBuilder) {
    let (z0, z1) = HEAD;
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(6.4, 6.4), 1.6),
        &[Section::new(z0, 1.0), Section::new(z1, 0.97)],
    );
    let (out, h, _) = MOLD;
    b.radial(4, |b| {
        b.paint(ACCENT);
        block(
            b,
            v3(out, 0.0, (RAM + z0) * 0.5),
            v3(h * 2.2, h * 2.2, z0 - RAM),
            0.25,
        );
    });
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, z1), 8, 0.7, 0.7, POST.2 - 0.2 - z1);
}

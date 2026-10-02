//! The Slipway, the Regency naval factory, on its 8 x 8 lot (96 m square): a one-sided
//! quay, like ARC's Wharf.
//!
//! It stands on the water (its origin is the surface, and nothing reaches under it). The
//! berth is open water at the lot origin, where the hull floats while it is printed;
//! everything the Slipway has stands behind its berth face (-y), so nothing is on the
//! far side or at either end, and a capital ship longer or wider than the lot still
//! fits. Ships leave toward +x, past the quay's swept bow.
//!
//! - The quay is an armoured pontoon: a bronze apron along the berth face (mooring
//!   blocks, bronze frames down the face, a lit seam, a violet edge at the bow end where
//!   ships leave), then plates lapped back like feathers into spikes over the rest.
//!   Red optics either side of the bow's point.
//! - Over the berth the fabricator heads hang from arms reaching out off the quay
//!   (their mounts are `mc_core::print_heads`, where the nanite streams pour from):
//!   each head under a plated crab at an arm's tip. The violet runs hot while it builds.
//! - A toothed bronze turntable turns on the quay (`part::SPINNER`).
//! - The owner's colour runs along the quay's ridge.
//!
//! Each tier is more elaborate, not more lit: tech 2 (Slipway II) puts up a second,
//! taller pair of arms at the quay's ends, tech 3 (Slipway III) a high boom over the
//! middle of the berth with three heads under it, for the capital ships.
//!
//! The quay is a plated ridge; the arms are cantilevers off plated pylons, braced off the
//! apron, counterweighted over the back; tech 2 adds a machine bank along the back, tech
//! 3 the boom along the berth on two pylons.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::*;

const MESH: &str = "regency_tidebrood";

/// The quay: its berth face, ridge and back (y); its apron's inner edge.
const FACE: f32 = -17.0;
const APRON: f32 = -24.0;
const RIDGE_Y: f32 = -33.0;
const BACK: f32 = -44.0;
/// Its bow's point (+x, where ships leave) and its stern.
const BOW: f32 = 46.0;
const STERN: f32 = -42.0;
/// Heights: the apron and the quay's sides, the ridge, the plates' thickness.
const SIDE_Z: f32 = 4.0;
const RIDGE_Z: f32 = 9.0;
const THICK: f32 = 0.85;
/// Tech 3's boom along the berth: its line (y), height, and ends (x).
const BOOM_Y: f32 = -3.5;
const BOOM_Z: f32 = 30.6;
const BOOM_END: f32 = 27.0;

/// The quay's deck height at `y`: flat over the apron, peaked at the ridge.
fn deck(y: f32) -> f32 {
    let t = if y > RIDGE_Y {
        (APRON - y.min(APRON)) / (APRON - RIDGE_Y)
    } else {
        (y - BACK) / (RIDGE_Y - BACK)
    };
    SIDE_Z + (RIDGE_Z - SIDE_Z) * t.clamp(0.0, 1.0)
}

/// Where the quay's bow has swept back to at `y`: the point is on the ridge.
fn bow_x(y: f32) -> f32 {
    BOW - (y - RIDGE_Y).abs() * 0.6
}

/// The heads tier `tier` fits: mount, scale, aim.
fn heads(tier: u8) -> impl Iterator<Item = (Vec3, f32, Vec3)> {
    tier_heads(MESH, tier)
}

// ---- Pieces ----------------------------------------------------------------------------

/// A plated crab riding a rail along `along` at `at` (the middle of its underside): bronze
/// wheels at full detail, a red optic looking out its `look` end.
fn crab(b: &mut MeshBuilder, at: Vec3, along: Vec3, look: Vec3) {
    let along = along.normalize();
    let across = Vec3::Z.cross(along);
    let half = along.abs() * 2.6 + across.abs() * 2.4;
    dark_plate(b);
    b.block(at - half, at + half + Vec3::Z * 1.6);
    if b.fine() {
        for k in [-1.6f32, 1.6] {
            collar(b, at + along * k + Vec3::Z * 0.1, across, 0.8, 5.0);
        }
    }
    let look = look.normalize();
    red_slot(
        b,
        at + look * (half.dot(look.abs()) + 0.05) + Vec3::Z * 0.8,
        look,
        Vec3::Z.cross(look),
        2.6,
        0.25,
    );
}

/// A head hung on a plated drop from `from_z` (the underside of what carries it).
fn hang(b: &mut MeshBuilder, mount: Vec3, s: f32, aim: Vec3, from_z: f32) {
    dark_plate(b);
    b.beam(
        v3(mount.x, mount.y, from_z),
        mount + Vec3::Z * 0.3,
        Vec2::new(1.4, 1.2),
        Vec2::new(1.0, 0.9),
    );
    fabricator(b, mount, aim, s);
}

/// The berth face's apron (`x0` to `x1` along it): mooring blocks, bronze frames down the
/// face, a lit seam, and a violet edge at its bow end where ships leave.
fn apron(b: &mut MeshBuilder, x0: f32, x1: f32) {
    let face = FACE - 0.3;
    let mut x = x0 + 6.0;
    while x < x1 - 3.0 {
        shaft(b, v3(x, face, 0.4), v3(x, face, SIDE_Z + 0.4), 0.55);
        x += 12.0;
    }
    let mut x = x0 + 12.0;
    while x < x1 - 6.0 {
        dark_plate(b);
        b.block(
            v3(x - 1.4, face - 2.2, SIDE_Z - 0.2),
            v3(x + 1.4, face + 0.6, SIDE_Z + 1.2),
        );
        x += 24.0;
    }
    red_slot(
        b,
        v3((x0 + x1) * 0.5, face + 0.1, 2.0),
        Vec3::Y,
        Vec3::X,
        x1 - x0 - 10.0,
        0.2,
    );
    b.paint(GLOW_VIOLET);
    b.beam(
        v3(x1 - 0.4, FACE + 0.2, 0.6),
        v3(x1 - 0.4, FACE + 0.2, SIDE_Z + 3.0),
        Vec2::new(0.35, 0.5),
        Vec2::new(0.35, 0.5),
    );
}

/// The toothed turntable turning at `at` (the spinner's pivot), on a plated drum.
fn turntable(b: &mut MeshBuilder, at: Vec3, r: f32) {
    let fine = b.fine();
    seam(b);
    b.prism(
        v3(at.x, at.y, at.z - 1.6),
        b.sides(12),
        r + 1.4,
        r + 0.8,
        1.0,
    );
    b.with_part(part::SPINNER, |b| {
        metal(b);
        let sides = b.sides(16);
        b.prism(at - Vec3::Z * 0.6, sides, r, r, 1.0);
        if fine {
            teeth(b, at - Vec3::Z * 0.1, r - 0.2, 18, v3(0.7, 0.8, 0.9));
        }
        dark_plate(b);
        b.beam(
            at + v3(-r + 0.8, 0.0, 0.5),
            at + v3(r - 0.8, 0.0, 0.5),
            Vec2::new(1.6, 1.0),
            Vec2::new(1.6, 1.0),
        );
    });
}

/// Rows of plates lapped back along the quay (toward -x) from `from` to `to`, across the
/// band `y0` to `y1` of the deck, `count` plates the last running `tail` past.
fn plate_row(b: &mut MeshBuilder, y0: f32, y1: f32, from: f32, to: f32, count: usize, tail: f32) {
    let (z0, z1) = (deck(y0) + 0.3, deck(y1) - 0.2);
    let y = (y0 + y1) * 0.5;
    let across = v3(0.0, y1 - y0, z1 - z0);
    let normal = v3(0.0, -across.z, across.y);
    let normal = if normal.z < 0.0 { -normal } else { normal };
    let len = (from - to) * 0.45;
    let f = Frame::new(v3(from, y, (z0 + z1) * 0.5), -Vec3::X, normal);
    dark_plate(b);
    let plates = Course {
        count,
        step: (from - to - len) / (count - 1) as f32,
        len,
        half: across.length() * 0.5,
        tip: if y1 > y0 { 1.0 } else { -1.0 },
        thick: THICK,
        tail,
    }
    .lay(b, &f);
    if b.fine() {
        metal(b);
        for &(g, long) in &plates {
            let top = g.at(long * 0.3, 0.0, 0.0);
            b.cylinder_between(top, top - g.n * 1.0, 0.45, 0.45, 4);
        }
    }
}

/// The quay's swept bow (red optics either side of its point).
fn bow_optics(b: &mut MeshBuilder) {
    let tip = v3(bow_x(RIDGE_Y), RIDGE_Y, SIDE_Z - 1.4);
    for side in [-1.0f32, 1.0] {
        let along = v3(-0.6, side, 0.0).normalize();
        let out = v3(1.0, side * 0.6, 0.0).normalize();
        red_slot(b, tip + along * 3.0 + out * 0.05, out, along, 3.0, 0.3);
    }
}

/// The owner's colour: a strip `x0` to `x1` along `y`, facing up at `z`.
fn team(b: &mut MeshBuilder, x0: f32, x1: f32, y: f32, w: f32, z: f32) {
    b.paint(TEAM);
    b.face(&[
        v3(x0, y - w, z),
        v3(x1, y - w, z),
        v3(x1, y + w, z),
        v3(x0, y + w, z),
    ]);
}

/// Lapped plates along the top of an arm or boom from `tip` back toward `root`, the last
/// running `tail` past into a spike.
fn arm_plates(b: &mut MeshBuilder, tip: Vec3, root: Vec3, half: f32, count: usize, tail: f32) {
    let span = (root - tip).length();
    let len = span * 0.42;
    let f = Frame::new(tip, root - tip, Vec3::Z);
    dark_plate(b);
    Course {
        count,
        step: (span - len) / (count - 1) as f32,
        len,
        half,
        tip: 0.0,
        thick: THICK * 0.8,
        tail,
    }
    .lay(b, &f);
}

// ---- The quay, cantilevers off plated pylons -------------------------------------------

/// The turntable, on the caisson at the quay's stern.
const TURNTABLE: Vec3 = Vec3::new(-34.0, -33.0, 8.6);
/// Its caisson: from the stern to `CAISSON` along the quay.
const CAISSON: f32 = -27.0;

pub(super) fn tidebrood(b: &mut MeshBuilder, tech: u8) {
    b.set_spinner_pivot(TURNTABLE);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    quay(b);
    apron(b, STERN, bow_x(FACE));
    bow_optics(b);
    caisson(b);
    for (mount, s, aim) in heads(1) {
        cantilever(b, mount, s, aim, 3.0, 6.0);
    }
    tier(b, tech, 2, 0.2, |b| {
        for (mount, s, aim) in heads(2) {
            cantilever(b, mount, s, aim, 3.2, 6.8);
        }
        bank(b);
    });
    tier(b, tech, 3, 0.25, boom);
}

/// Far off: the quay, its caisson, the arms of the highest tier as a tower and a top,
/// the owner's colour along the ridge.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    dark_plate(b);
    let ring = |z: f32, inset: f32| -> Vec<Vec3> {
        vec![
            v3(bow_x(RIDGE_Y) - inset, RIDGE_Y, z),
            v3(bow_x(FACE) - inset, FACE - inset * 0.3, z),
            v3(STERN + inset * 0.3, FACE - inset * 0.3, z),
            v3(STERN + inset * 0.3, BACK + inset * 0.3, z),
            v3(bow_x(BACK) - inset, BACK + inset * 0.3, z),
        ]
    };
    b.loft(&[ring(0.0, 0.0), ring(RIDGE_Z + 0.6, 6.0)], false, true);
    team(b, -20.0, 30.0, RIDGE_Y, 1.4, RIDGE_Z + 0.65);
    dark_plate(b);
    let tiers: &[u8] = match tech {
        1 => &[1],
        2 => &[1, 2],
        _ => &[2],
    };
    for &t in tiers {
        for (mount, _, _) in heads(t) {
            coarse_arm(b, mount.x, mount.y - 2.0, mount.z + 3.0 + 1.2);
        }
    }
    if tech >= 3 {
        for x in [-BOOM_END + 2.0, BOOM_END - 2.0] {
            b.prism(v3(x, RIDGE_Y + 1.0, RIDGE_Z), 3, 3.0, 2.2, BOOM_Z - RIDGE_Z);
        }
        b.face(&[
            v3(-BOOM_END, BOOM_Y - 2.0, BOOM_Z + 1.3),
            v3(BOOM_END, BOOM_Y - 2.0, BOOM_Z + 1.3),
            v3(BOOM_END, BOOM_Y + 2.0, BOOM_Z + 1.3),
            v3(-BOOM_END, BOOM_Y + 2.0, BOOM_Z + 1.3),
        ]);
    }
}

/// A coarse arm: a three-sided tower on the ridge and a top reaching out to `tip`.
fn coarse_arm(b: &mut MeshBuilder, x: f32, tip: f32, top: f32) {
    b.prism(v3(x, RIDGE_Y, RIDGE_Z), 3, 3.4, 2.4, top - RIDGE_Z);
    b.face(&[
        v3(x - 1.8, RIDGE_Y - 6.0, top),
        v3(x + 1.8, RIDGE_Y - 6.0, top),
        v3(x + 1.8, tip, top),
        v3(x - 1.8, tip, top),
    ]);
}

/// The quay: a pontoon swept to a point at the bow, square at the stern, its bronze deck
/// peaked at the ridge, plates lapped back over it.
fn quay(b: &mut MeshBuilder) {
    let hull = |z: f32| -> Vec<Vec3> {
        vec![
            v3(bow_x(RIDGE_Y), RIDGE_Y, z),
            v3(bow_x(FACE), FACE, z),
            v3(STERN, FACE, z),
            v3(STERN, BACK, z),
            v3(bow_x(BACK), BACK, z),
        ]
    };
    seam(b);
    b.loft(&[hull(0.0), hull(SIDE_Z - 0.6)], false, true);
    let section = [
        (FACE - 0.4, SIDE_Z - 0.6),
        (APRON, SIDE_Z - 0.4),
        (RIDGE_Y, RIDGE_Z - 0.9),
        (BACK + 0.6, SIDE_Z - 0.6),
    ];
    let deck_ring = |x: Option<f32>| -> Vec<Vec3> {
        section
            .iter()
            .map(|&(y, z)| v3(x.unwrap_or(bow_x(y) - 3.0), y, z))
            .collect()
    };
    metal(b);
    b.loft(&[deck_ring(Some(STERN + 0.4)), deck_ring(None)], true, true);
    // Plates: one row up the inner slope from the apron, two down the back.
    for (y0, y1, count) in [
        (RIDGE_Y + 0.4, APRON - 0.4, 5),
        (RIDGE_Y - 0.4, RIDGE_Y - 5.2, 5),
        (RIDGE_Y - 5.8, BACK - 0.8, 5),
    ] {
        plate_row(
            b,
            y0,
            y1,
            bow_x((y0 + y1) * 0.5) - 3.0,
            CAISSON + 2.0,
            count,
            3.0,
        );
    }
    team(b, -20.0, 30.0, RIDGE_Y, 0.6, RIDGE_Z + 1.5);
}

/// The caisson at the stern, the turntable on its roof, plates lapped back over its
/// sides into spikes past the stern.
fn caisson(b: &mut MeshBuilder) {
    seam(b);
    b.frustum_open(
        v3((STERN + CAISSON) * 0.5, (APRON + BACK) * 0.5, 0.0),
        Vec2::new(CAISSON - STERN, APRON - BACK),
        Vec2::new(CAISSON - STERN - 2.0, APRON - BACK - 3.0),
        TURNTABLE.z - 1.6,
        Vec2::ZERO,
    );
    for (y, tip) in [(APRON - 2.0, 1.0f32), (BACK + 2.0, -1.0)] {
        let f = Frame::new(
            v3(CAISSON + 1.0, y, TURNTABLE.z - 1.8),
            -Vec3::X,
            v3(0.0, -tip * 0.35, 1.0),
        );
        dark_plate(b);
        Course {
            count: 2,
            step: 6.0,
            len: 8.0,
            half: 3.4,
            tip: -tip,
            thick: THICK,
            tail: 3.0,
        }
        .lay(b, &f);
    }
    turntable(b, TURNTABLE, 5.0);
}

/// A cantilever over the berth for the head at `mount`: a plated pylon on the ridge
/// leaning toward the berth, an arm out over the water `lift` above the head with the
/// crab and head at its tip, counterweighted `back` behind the pylon, braced off the
/// apron by a plated strut.
fn cantilever(b: &mut MeshBuilder, mount: Vec3, s: f32, aim: Vec3, lift: f32, back: f32) {
    let x = mount.x;
    let arm_z = mount.z + lift;
    let tip = mount.y + 2.4;
    let root = RIDGE_Y - back;
    // Bearing block on the ridge, a bronze collar the pylon stands in.
    dark_plate(b);
    b.block(
        v3(x - 4.2, RIDGE_Y - 3.4, deck(RIDGE_Y) - 1.2),
        v3(x + 4.2, RIDGE_Y + 3.4, RIDGE_Z + 0.8),
    );
    collar(b, v3(x, RIDGE_Y + 0.6, RIDGE_Z + 1.2), Vec3::Z, 2.9, 0.8);
    // The pylon: faceted, leaning toward the berth.
    dark_plate(b);
    b.with_facets(|b| {
        b.frustum(
            v3(x, RIDGE_Y + 0.6, RIDGE_Z + 0.8),
            Vec2::new(4.6, 5.2),
            Vec2::new(3.2, 3.4),
            arm_z - 1.2 - (RIDGE_Z + 0.8),
            Vec2::new(0.0, 2.0),
        )
    });
    let top = v3(x, RIDGE_Y + 2.6, arm_z);
    red_slot(
        b,
        v3(x, RIDGE_Y + 3.6, arm_z - 3.0),
        v3(0.0, 1.0, 0.2),
        Vec3::X,
        2.4,
        0.3,
    );
    // The arm, out to the tip; the counterweight under its root.
    dark_plate(b);
    b.beam(
        v3(x, root, arm_z),
        v3(x, tip, arm_z),
        Vec2::new(3.6, 2.4),
        Vec2::new(3.0, 2.0),
    );
    arm_plates(
        b,
        v3(x, tip + 0.6, arm_z + 1.0),
        v3(x, root, arm_z + 1.0),
        1.9,
        3,
        3.0,
    );
    dark_plate(b);
    b.block(
        v3(x - 2.0, root - 0.4, arm_z - 4.6),
        v3(x + 2.0, root + 4.0, arm_z - 1.0),
    );
    // Braced off the apron.
    strut(
        b,
        v3(x, FACE - 2.4, SIDE_Z + 0.4),
        v3(x, tip - 6.0, arm_z - 1.1),
        0.6,
    );
    if b.fine() {
        // The crab's drive line under the arm, and its feed cable down the pylon.
        shaft(
            b,
            v3(x, top.y, arm_z - 1.5),
            v3(x, tip + 0.8, arm_z - 1.5),
            0.32,
        );
        metal(b);
        cable(
            b,
            &[
                v3(x + 1.9, RIDGE_Y - 1.6, RIDGE_Z + 1.0),
                v3(x + 1.9, RIDGE_Y + 0.6, arm_z - 1.4),
                v3(x + 1.9, mount.y + 4.0, arm_z - 1.4),
            ],
            0.28,
        );
    }
    crab(b, v3(x, mount.y, arm_z + 1.2), Vec3::Y, Vec3::Y);
    hang(b, mount, s, aim, arm_z - 1.0);
}

/// Tech 2: a machine bank along the quay's back: a low plated housing, plates lapped back
/// along its outer edge into a spike, feed lines up the quay's back.
fn bank(b: &mut MeshBuilder) {
    let (y0, y1, x0, x1, top) = (BACK - 2.8, BACK + 0.5, -24.0, 22.0, 4.8);
    seam(b);
    b.frustum_open(
        v3((x0 + x1) * 0.5, (y0 + y1) * 0.5, 0.0),
        Vec2::new(x1 - x0, y1 - y0),
        Vec2::new(x1 - x0 - 2.0, y1 - y0 - 1.0),
        top,
        Vec2::ZERO,
    );
    let f = Frame::new(
        v3(x1 + 1.0, y0 + 2.0, top + 0.3),
        -Vec3::X,
        v3(0.0, -0.35, 1.0),
    );
    dark_plate(b);
    Course {
        count: 4,
        step: 10.5,
        len: 13.0,
        half: 1.6,
        tip: 1.0,
        thick: THICK,
        tail: 3.0,
    }
    .lay(b, &f);
    red_slot(
        b,
        v3(x1 - 0.9, (y0 + y1) * 0.5, top * 0.5),
        Vec3::X,
        Vec3::Y,
        2.4,
        0.3,
    );
    if b.fine() {
        metal(b);
        for x in [-14.0, 0.0, 12.0] {
            cable(
                b,
                &[
                    v3(x, y0 + 1.2, top),
                    v3(x, BACK + 1.0, top + 0.6),
                    v3(x, BACK + 3.0, deck(BACK + 3.0) + 0.4),
                ],
                0.3,
            );
        }
    }
}

/// Tech 3: the boom along the berth on two tall pylons, its arms reaching out from them,
/// three crabs riding it with the tech 3 heads under them.
fn boom(b: &mut MeshBuilder) {
    for x in [-BOOM_END + 2.0, BOOM_END - 2.0] {
        // Pylon: a bearing block, a tall faceted tower, plates lapped down its back.
        dark_plate(b);
        b.block(
            v3(x - 4.6, RIDGE_Y - 3.8, deck(RIDGE_Y) - 1.2),
            v3(x + 4.6, RIDGE_Y + 3.8, RIDGE_Z + 1.0),
        );
        collar(b, v3(x, RIDGE_Y, RIDGE_Z + 1.4), Vec3::Z, 3.2, 0.8);
        dark_plate(b);
        b.with_facets(|b| {
            b.frustum(
                v3(x, RIDGE_Y, RIDGE_Z + 1.0),
                Vec2::new(5.4, 6.0),
                Vec2::new(3.6, 4.0),
                BOOM_Z - 1.0 - (RIDGE_Z + 1.0),
                Vec2::new(0.0, 1.0),
            )
        });
        let f = Frame::new(
            v3(x, RIDGE_Y - 2.6, BOOM_Z - 2.0),
            v3(0.0, -0.25, -1.0),
            -Vec3::Y,
        );
        dark_plate(b);
        Course {
            count: 3,
            step: 6.0,
            len: 8.0,
            half: 2.6,
            tip: 0.0,
            thick: THICK,
            tail: 3.0,
        }
        .lay(b, &f);
        red_slot(
            b,
            v3(x, RIDGE_Y + 3.1, BOOM_Z - 4.0),
            Vec3::Y,
            Vec3::X,
            2.6,
            0.3,
        );
        // The arm out to the boom, a plated strut under it off the pylon.
        dark_plate(b);
        b.beam(
            v3(x, RIDGE_Y - 4.0, BOOM_Z),
            v3(x, BOOM_Y - 1.6, BOOM_Z),
            Vec2::new(3.8, 2.6),
            Vec2::new(3.4, 2.4),
        );
        strut(
            b,
            v3(x, RIDGE_Y + 3.0, BOOM_Z - 9.0),
            v3(x, BOOM_Y - 4.0, BOOM_Z - 1.3),
            0.6,
        );
        arm_plates(
            b,
            v3(x, BOOM_Y - 1.0, BOOM_Z + 1.4),
            v3(x, RIDGE_Y - 4.0, BOOM_Z + 1.4),
            2.0,
            3,
            3.0,
        );
    }
    dark_plate(b);
    b.beam(
        v3(-BOOM_END, BOOM_Y, BOOM_Z),
        v3(BOOM_END, BOOM_Y, BOOM_Z),
        Vec2::new(3.4, 2.4),
        Vec2::new(3.4, 2.4),
    );
    // Plates lapped back along the boom (toward -x), the last a spike past its end.
    let f = Frame::new(v3(BOOM_END, BOOM_Y, BOOM_Z + 1.2), -Vec3::X, Vec3::Z);
    dark_plate(b);
    Course {
        count: 4,
        step: 13.0,
        len: 16.0,
        half: 1.6,
        tip: 0.0,
        thick: THICK * 0.8,
        tail: 3.0,
    }
    .lay(b, &f);
    if b.fine() {
        shaft(
            b,
            v3(-BOOM_END + 1.0, BOOM_Y + 1.5, BOOM_Z - 1.4),
            v3(BOOM_END - 1.0, BOOM_Y + 1.5, BOOM_Z - 1.4),
            0.32,
        );
    }
    for (mount, s, aim) in heads(3) {
        crab(b, v3(mount.x, BOOM_Y, BOOM_Z + 1.2), Vec3::X, Vec3::Y);
        hang(b, mount, s, aim, BOOM_Z - 1.0);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tidebrood_fits_its_lot_at_every_tier() {
        let key = "regency_tidebrood";
        for (tech, height) in [(1, 20.0), (2, 24.0), (3, 32.0)] {
            super::super::check_at(key, tech, 46.0, height, Some(8), &[]);
            super::super::check_heads_at(key, tech, 46.0, height);
        }
    }
}

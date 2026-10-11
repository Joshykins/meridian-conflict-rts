//! The Atelier's tower (`regency_atelier` and its crown designs, `crown`): everything
//! under the crown.
//!
//! - The plinth: an octagon of sloped plate, two courses of swept plates lapped down each
//!   face either side of a machinery bay or a red slot, a lit band round its shoulder
//!   between machined covers, the owner's colour round its lid.
//! - The vault on the lid: an octagonal drum, a slotted nanite panel let into each face.
//! - The shaft: four straight-sided octagonal stages, each stepped in from the one below,
//!   lit seams between them, their faces plated, slotted, lit and covered by turns; four
//!   plated fins raked from the lowest onto the vault.
//! - Tech 2 adds four slotted feed limbs from the lid's corners up to the shaft, plates
//!   down the second stage and lockers on the plinth. Tech 3 leans a spiked buttress on
//!   each of the plinth's broad faces, laps plates down the vault and the top stage, and
//!   floats a toothed ring round the shaft.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_8};

use glam::Vec3;

use crate::builder::MeshBuilder;

use super::super::kit::{blade, dark_plate, metal, nanite_plate, seam, v3};
use super::super::machine::{tier, Frame};
use super::super::parts::{bay, locker};
use super::{
    cover, deep, facets, lapped, nanite_limb, panel, red_slot, sleeve, spinner_ring, stage,
    team_band, Facet, HEX,
};

/// The plinth's stages: (foot, head, corner radius at each).
const SKIRT: (f32, f32, f32, f32) = (0.0, 0.6, 18.6, 18.6);
const BODY: (f32, f32, f32, f32) = (0.6, 4.6, 18.2, 15.8);
const SHOULDER: (f32, f32, f32, f32) = (4.6, 6.0, 15.8, 13.6);
/// The vault on the lid.
const VAULT: (f32, f32, f32, f32) = (6.0, 12.0, 11.0, 9.6);
/// The shaft's stages: straight sides, each stepped in from the one below.
const SPIRE: [(f32, f32, f32, f32); 4] = [
    (12.0, 21.0, 6.6, 6.4),
    (21.4, 29.0, 6.0, 5.8),
    (29.4, 35.0, 5.4, 5.2),
    (35.4, SHAFT_TOP, 4.8, 4.6),
];
/// Where the shaft ends and the crown begins.
pub(super) const SHAFT_TOP: f32 = 37.6;
/// The floating ring's height and radius.
const RING: (f32, f32) = (25.2, 7.6);

/// The tower up to [`SHAFT_TOP`] at tech `tech`; the crown goes on it. Far off, the
/// tower alone, for the crown's own far shape to stand on.
pub(super) fn tower(b: &mut MeshBuilder, tech: u8) {
    b.set_dust_line(0.5);
    b.set_spinner_pivot(Vec3::Z * SHAFT_TOP);
    if b.coarse() {
        coarse(b);
        return;
    }
    plinth(b);
    vault(b);
    spire(b);
    tier(b, tech, 2, 0.15, plinth_lockers);
    tier(b, tech, 2, 0.4, |b| {
        for k in 0..4 {
            b.yawed(Vec3::ZERO, FRAC_PI_4 + FRAC_PI_2 * k as f32, feed);
        }
    });
    tier(b, tech, 2, 0.6, |b| stage_plates(b, 1, 1));
    tier(b, tech, 3, 0.1, |b| {
        for k in 0..4 {
            b.yawed(Vec3::ZERO, FRAC_PI_2 * k as f32, buttress);
        }
    });
    tier(b, tech, 3, 0.3, vault_plates);
    tier(b, tech, 3, 0.5, |b| stage_plates(b, 2, 1));
    tier(b, tech, 3, 0.6, |b| spinner_ring(b, RING.0, RING.1, 28));
}

/// The four-number stage tuples as `stage` takes them, octagonal, flat faces on the axes.
fn oct(b: &mut MeshBuilder, (z0, z1, r0, r1): (f32, f32, f32, f32)) {
    stage(b, 8, FRAC_PI_8, z0, z1, r0, r1);
}

fn oct_faces((z0, z1, r0, r1): (f32, f32, f32, f32)) -> Vec<Facet> {
    facets(8, FRAC_PI_8, z0, z1, r0, r1)
}

/// Far off: the plinth as a six-sided drum with the owner's colour on it, the shaft as
/// a square block.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    stage(b, 6, 0.0, 0.0, SHOULDER.1, 17.5, 13.0);
    stage(b, 4, FRAC_PI_4, SHOULDER.1, SHAFT_TOP, 8.4, 6.4);
    b.paint(crate::material::TEAM);
    let z = SHOULDER.1 + 0.05;
    b.face(&[v3(10.0, -2.0, z), v3(12.0, 0.0, z), v3(10.0, 2.0, z)]);
}

/// The plinth: skirt, body and shoulder, each face of the body plated in two lapped
/// courses either side of a machinery bay (on the broad faces) or a red slot (on the
/// others), the shoulder lit and covered by turns, the owner's colour round the lid.
fn plinth(b: &mut MeshBuilder) {
    seam(b);
    oct(b, SKIRT);
    dark_plate(b);
    oct(b, BODY);
    deep(b);
    oct(b, SHOULDER);
    let fine = b.fine();
    for (k, f) in oct_faces(BODY).iter().enumerate() {
        let out = f.out();
        for t in [-0.5f32, 0.5] {
            lapped(
                b,
                f.at(0.97, t),
                f.at(0.02, t * 1.02),
                out,
                2.6,
                if fine { 2 } else { 1 },
            );
        }
        if !b.mid() {
            continue;
        }
        if k % 2 == 0 && fine {
            let g = Frame::new(
                f.at(0.9, 0.0) + out * 0.02,
                f.at(0.1, 0.0) - f.at(0.9, 0.0),
                out,
            );
            bay(b, &g, f.at(0.9, 0.0).distance(f.at(0.1, 0.0)), 0.9);
        } else {
            red_slot(b, f, [0.25, 0.8], [-0.04, 0.04]);
        }
    }
    for (k, f) in oct_faces(SHOULDER).iter().enumerate() {
        if k % 2 == 0 {
            red_slot(b, f, [0.38, 0.62], [-0.55, 0.55]);
        } else if b.mid() {
            cover(b, f, [0.2, 0.8], [-0.7, 0.7]);
        }
    }
    let bearings: Vec<f32> = (0..=8).map(|k| FRAC_PI_8 + FRAC_PI_4 * k as f32).collect();
    team_band(b, &bearings, 11.6, 12.8, SHOULDER.1 + 0.03);
}

/// The tech 2 lockers on the plinth's four diagonal faces, over the red slots.
fn plinth_lockers(b: &mut MeshBuilder) {
    for (k, f) in oct_faces(BODY).iter().enumerate() {
        if k % 2 == 1 {
            let g = Frame::new(f.at(0.1, 0.0), f.at(0.9, 0.0) - f.at(0.1, 0.0), f.out());
            locker(b, &g, v3(1.4, 1.6, 0.5));
        }
    }
}

/// The vault: an octagonal drum, corners on the axes, a slotted nanite panel let into
/// each face, its lid stepped in.
fn vault(b: &mut MeshBuilder) {
    let (z0, z1, r0, r1) = VAULT;
    dark_plate(b);
    stage(b, 8, 0.0, z0, z1, r0, r1);
    seam(b);
    stage(b, 8, 0.0, z1, z1 + 0.4, r1 + 0.2, r1);
    for f in facets(8, 0.0, z0, z1, r0, r1) {
        nanite_plate(b);
        panel(b, &f, [0.12, 0.88], [-0.3, 0.3], 0.18);
        if b.fine() {
            red_slot(b, &f, [0.12, 0.88], [0.55, 0.62]);
            red_slot(b, &f, [0.12, 0.88], [-0.62, -0.55]);
        }
    }
}

/// The tech 3 courses lapped down the vault's faces either side of their panels.
fn vault_plates(b: &mut MeshBuilder) {
    let (z0, z1, r0, r1) = VAULT;
    for f in facets(8, 0.0, z0, z1, r0, r1) {
        for t in [-0.78f32, 0.78] {
            lapped(b, f.at(0.98, t), f.at(0.05, t), f.out(), 0.45, 2);
        }
    }
}

/// The shaft: four straight stages with lit seams between, their faces plated, slotted,
/// lit and covered by turns, and four fins raked from the lowest onto the vault.
fn spire(b: &mut MeshBuilder) {
    let fine = b.fine();
    for (n, &s) in SPIRE.iter().enumerate() {
        dark_plate(b);
        oct(b, s);
        let (_, z1, _, r1) = s;
        if n + 1 < SPIRE.len() {
            seam(b);
            oct(b, (z1, z1 + 0.4, r1 + 0.25, r1 + 0.2));
        }
        for (k, f) in oct_faces(s).iter().enumerate() {
            match (n, k % 2) {
                (0, 0) | (2, 0) => lapped(
                    b,
                    f.at(0.97, 0.0),
                    f.at(0.03, 0.0),
                    f.out(),
                    1.5,
                    if fine { 3 } else { 2 },
                ),
                (0, _) | (1, 0) | (2, _) => {
                    nanite_plate(b);
                    panel(b, f, [0.06, 0.94], [-0.3, 0.3], 0.14);
                }
                (1, _) if b.mid() => red_slot(b, f, [0.2, 0.8], [-0.1, 0.1]),
                (3, _) if b.mid() => {
                    red_slot(b, f, [0.25, 0.75], [-0.62, -0.5]);
                    red_slot(b, f, [0.25, 0.75], [0.5, 0.62]);
                }
                _ => {}
            }
            if fine {
                // A machined band round the head of every stage, across its plates.
                cover(b, f, [0.9, 0.98], [-0.85, 0.85]);
                if n == 3 {
                    cover(b, f, [0.2, 0.8], [-0.35, 0.35]);
                }
            }
        }
    }
    dark_plate(b);
    for k in 0..4 {
        let a = FRAC_PI_2 * k as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        let side = v3(-a.sin(), a.cos(), 0.0);
        blade(
            b,
            d * 6.0 + Vec3::Z * 19.6,
            d * 9.8 + Vec3::Z * 12.3,
            1.5,
            side,
        );
    }
}

/// Lapped plates either side of the middle of stage `n`'s faces whose index is `parity`
/// mod 2: a later tier's armour.
fn stage_plates(b: &mut MeshBuilder, n: usize, parity: usize) {
    for (k, f) in oct_faces(SPIRE[n]).iter().enumerate() {
        if k % 2 == parity {
            for t in [-0.62f32, 0.62] {
                lapped(b, f.at(0.97, t), f.at(0.03, t), f.out(), 0.55, 2);
            }
        }
    }
}

/// One tech 2 feed limb on the +x diagonal (yawed round to each): slotted nanite plate
/// from the lid's corner up to the spire's second stage, a graphite collar at each end.
fn feed(b: &mut MeshBuilder) {
    let (foot, head) = (v3(12.2, 0.0, SHOULDER.1 - 0.2), v3(6.3, 0.0, 20.0));
    let d = (head - foot).normalize();
    let out = v3(-d.z, 0.0, d.x);
    let out = if out.x < 0.0 { -out } else { out };
    nanite_limb(
        b,
        &[
            (foot, 0.75, 0.55),
            (foot.lerp(head, 0.5), 0.7, 0.5),
            (head, 0.6, 0.45),
        ],
        out,
    );
    metal(b);
    for (p, r) in [(foot + d * 0.4, 0.95), (head - d * 0.4, 0.8)] {
        b.cylinder_between(p - d * 0.35, p + d * 0.35, r, r, b.sides(8));
    }
    if b.fine() {
        dark_plate(b);
        sleeve(
            b,
            &[
                (foot + d * 1.6 + out * 0.7, 0.55, 0.12),
                (head - d * 2.0 + out * 0.62, 0.4, 0.1),
            ],
            out,
            &HEX,
        );
    }
}

/// One tech 3 buttress against the plinth's broad face on +x (yawed round to each): a
/// plated fin from the ground up the vault's flank, its plate swept into a spike at its
/// foot, a red slot in its brow.
fn buttress(b: &mut MeshBuilder) {
    let r0 = BODY.2 * FRAC_PI_8.cos();
    dark_plate(b);
    b.with_facets(|b| {
        b.extrude_y(
            &[
                [r0 - 0.4, 0.6],
                [r0 + 0.6, 0.0],
                [r0 - 0.2, 2.2],
                [10.4, 11.4],
                [9.4, 11.6],
                [9.2, 6.0],
            ],
            -0.8,
            0.8,
        )
    });
    if b.fine() {
        let f = Facet {
            a0: v3(r0 - 0.2, -0.82, 2.2),
            b0: v3(r0 - 0.2, 0.82, 2.2),
            a1: v3(10.4, -0.82, 11.4),
            b1: v3(10.4, 0.82, 11.4),
        };
        red_slot(b, &f, [0.55, 0.85], [-0.3, 0.3]);
    }
}

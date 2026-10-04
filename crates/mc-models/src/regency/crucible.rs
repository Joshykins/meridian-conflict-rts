//! The Crucible, the Regency reclaim tower (`regency_crucible`, tech 1 to 3 on one upgrade
//! chain, a 3 x 3 lot): a sealed vault with a plated mast rising out of it and a nanite
//! head at the mast's crown. Its strands take a wreck apart and the matter rides home down
//! them, down glazed chutes and into the vault, where it is melted into stock.
//!
//! - The vault: an octagon of sloped plate, each face armoured by a course of swept plates
//!   lapped down it into a spike at the foot, a lit red slot in each face of its shoulder
//!   where the melt runs hot, the owner's colour round the lid.
//! - The mast: plated, three sleeves of plates lapped down its square faces, a glazed chute
//!   (`pattern::MASS_FLOW`) down each diagonal face from the head to the vault lid.
//! - The head is the Breaker's (`reclaimer::house`, `reclaimer::head`) at [`K`] times its
//!   size: a graphite yaw collar, armoured cheeks, a keeled cowl and the nozzle with the
//!   violet lens at its tip, where the stream leaves.
//! - Tech 2 raises a plated pylon on each flank, braced to the mast, with a head of its own
//!   at [`FLANK_K`] and a chute from it into the vault: three wrecks come apart at once.
//!   Tech 3 leans a plated buttress against every corner of the vault and floats a toothed
//!   ring round the mast (`part::SPINNER`).
//!
//! Rig: each head is a gun house bound to its reclaim head (head i = weapon slot i, like
//! ARC's tower): the mast's is 0, the +y pylon's 1, the -y pylon's 2. Each turns about its
//! pivot and what is inside `with_recoil` pitches about it. Authored at blueprint scale
//! (radius 16.9, height 28, the same at every tier): [`PIVOT`], [`EMIT_X`], [`FLANK`] and
//! [`FLANK_EMIT_X`] are the heads of `regency_t{1,2,3}_reclaimer` in
//! `data/factions/regency/units/structures.ron`.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_8, TAU};

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::{armour, hoop, red_slot, strut, swept, teeth, tier, Course, Frame};
use super::reclaimer::{head, house};

/// Collision radius at every tier: a 3 x 3 lot of 12 m cells.
pub(super) const RADIUS: f32 = 16.9;
/// Authored height, the same at every tier: upgrades build round the mast, not up it.
pub(super) const HEIGHT: f32 = 28.0;
/// The Breaker's head, trunnion to lens tip, in its own metres.
const HEAD_REACH: f32 = 2.25;
/// The mast head's scale against the Breaker's, its trunnion, and how far ahead of it the
/// stream leaves.
const K: f32 = 3.6;
pub(super) const PIVOT: Vec3 = Vec3::new(0.0, 0.0, 25.5);
pub(super) const EMIT_X: f32 = HEAD_REACH * K;
/// The flank heads' scale, the +y head's trunnion (the -y one's is its mirror), and its
/// stream's lead.
const FLANK_K: f32 = 2.6;
pub(super) const FLANK: Vec3 = Vec3::new(0.0, 12.6, 15.0);
pub(super) const FLANK_EMIT_X: f32 = HEAD_REACH * FLANK_K;
/// The vault: its octagon's corner radius at the foot, at the top of its faces, and at the
/// lid, and the heights of the last two.
const VAULT: [(f32, f32); 3] = [(11.2, 0.3), (9.6, 6.6), (8.2, 8.0)];
/// The mast: its radius at the lid and at the top, and the top, under the head's collar.
const MAST: (f32, f32) = (3.6, 2.6);
const MAST_TOP: f32 = PIVOT.z - 0.62 * K;
/// Where the tech 3 ring floats round the mast.
const RING_Z: f32 = 12.0;
/// Full-detail triangle budget: a 3 x 3 installation, three heads and its tech 3 kit.
#[cfg(test)]
pub(super) const TRIANGLES: usize = 7000;

pub(super) fn crucible(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    b.set_dust_line(0.5);
    b.set_spinner_pivot(Vec3::Z * RING_Z);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    vault(b);
    mast(b);
    mast_head(b);
    tier(b, tech, 2, 0.15, |b| b.mirror_y(pylon));
    if tech >= 2 {
        for side in [1.0, -1.0] {
            flank_head(b, side);
        }
    }
    tier(b, tech, 3, 0.1, |b| {
        for k in 0..8 {
            let a = FRAC_PI_8 + FRAC_PI_4 * k as f32;
            b.yawed(Vec3::ZERO, a, buttress);
        }
    });
    tier(b, tech, 3, 0.4, ring);
}

/// A bar from `a` to `c`, `w` across, without end caps: the far level's heads.
fn bar(b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32) {
    let (side, up) = super::kit::frame(c - a, Vec3::Z);
    let ring = |p: Vec3, s: f32| -> Vec<Vec3> {
        let (x, y) = (side * (w * s * 0.5), up * (w * s * 0.5));
        vec![p - x - y, p + x - y, p + x + y, p - x + y]
    };
    b.loft(&[ring(a, 1.0), ring(c, 0.6)], false, false);
}

/// Far off: the vault as a six-sided drum, the mast, the owner's colour on the lid, each
/// head a bar on its house.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    let [(r0, z0), _, (r1, z1)] = VAULT;
    dark_plate(b);
    let hex = |r: f32, z: f32| -> Vec<Vec3> {
        (0..6)
            .map(|i| {
                let a = TAU * i as f32 / 6.0;
                v3(a.cos() * r, a.sin() * r, z)
            })
            .collect()
    };
    b.loft(&[hex(r0 + 0.6, z0), hex(r1, z1)], false, true);
    b.frustum_open(
        Vec3::Z * z1,
        Vec2::splat(MAST.0 * 1.4),
        Vec2::splat(MAST.1 * 1.4),
        MAST_TOP - z1,
        Vec2::ZERO,
    );
    b.paint(TEAM);
    let z = z1 + 0.05;
    b.face(&[v3(5.5, 0.0, z), v3(-3.0, 4.6, z), v3(-3.0, -4.6, z)]);
    b.with_house(0, PIVOT, 0.0, |b| {
        b.with_recoil(|b| {
            dark_plate(b);
            bar(b, PIVOT - Vec3::X * 2.4, PIVOT + Vec3::X * EMIT_X, 2.4);
        });
    });
    if tech >= 2 {
        for (i, side) in [(1, 1.0), (2, -1.0)] {
            let p = FLANK * v3(1.0, side, 1.0);
            dark_plate(b);
            bar(b, p.with_z(0.0), p.with_z(p.z - 1.4), 3.4);
            b.with_house(i, p, 0.0, |b| {
                b.with_recoil(|b| {
                    dark_plate(b);
                    bar(b, p - Vec3::X * 1.8, p + Vec3::X * FLANK_EMIT_X, 1.8);
                });
            });
        }
    }
}

/// The octagon's corner at bearing `a`, `r` out, at height `z`.
fn corner(a: f32, r: f32, z: f32) -> Vec3 {
    v3(a.cos() * r, a.sin() * r, z)
}

/// The vault: the sloped octagon and its lid, a course of swept plates lapped down each
/// face, a red slot in each face of the shoulder, the owner's colour round the lid.
fn vault(b: &mut MeshBuilder) {
    let plan: Vec<[f32; 2]> = (0..8)
        .map(|k| {
            let a = FRAC_PI_8 + FRAC_PI_4 * k as f32;
            [a.cos(), a.sin()]
        })
        .collect();
    let [(r0, z0), (r1, z1), (r2, z2)] = VAULT;
    seam(b);
    b.loft_z(
        &plan,
        &[Section::new(0.0, r0 + 0.3), Section::new(z0, r0 + 0.3)],
    );
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(z0, r0),
                Section::new(z1, r1),
                Section::new(z2, r2),
            ],
        )
    });
    let flat = FRAC_PI_8.cos();
    for k in 0..8 {
        let a = FRAC_PI_4 * k as f32;
        let out = v3(a.cos(), a.sin(), 0.0);
        let along = v3(-a.sin(), a.cos(), 0.0);
        // Down the face, from the top of it to its foot.
        let top = out * (r1 * flat) + Vec3::Z * z1;
        let foot = out * (r0 * flat) + Vec3::Z * z0;
        let down = foot - top;
        let n = down.cross(along).normalize();
        let n = if n.dot(out) < 0.0 { -n } else { n };
        dark_plate(b);
        let f = Frame::new(top + n * 0.02 + down.normalize() * 0.2, down, n);
        let (count, step) = if b.fine() { (2, 2.4) } else { (1, 2.4) };
        Course {
            count,
            step,
            len: 2.8,
            half: r1 * FRAC_PI_8.sin() * 0.82,
            tip: 0.0,
            thick: 0.35,
            tail: 0.7,
        }
        .lay(b, &f);
        // The shoulder above the face.
        let lo = out * (r1 * flat) + Vec3::Z * z1;
        let hi = out * (r2 * flat) + Vec3::Z * z2;
        let rise = hi - lo;
        let sn = rise.cross(along).normalize();
        let sn = if sn.dot(out) < 0.0 { -sn } else { sn };
        red_slot(b, lo.lerp(hi, 0.5), sn, along, 3.2, 0.32);
    }
    // The owner's colour: a band round the lid between the mast and its rim.
    b.paint(TEAM);
    let z = z2 + 0.03;
    for k in 0..8 {
        let (a, c) = (
            FRAC_PI_8 + FRAC_PI_4 * k as f32,
            FRAC_PI_8 + FRAC_PI_4 * (k + 1) as f32,
        );
        b.face(&[
            corner(a, 6.0, z),
            corner(a, 7.0, z),
            corner(c, 7.0, z),
            corner(c, 6.0, z),
        ]);
    }
}

/// The mast: the plated octagon up from the lid, a graphite bearing at its top, three
/// sleeves of plates lapped down its square faces, a glazed chute down each diagonal
/// face into the lid.
fn mast(b: &mut MeshBuilder) {
    let z0 = VAULT[2].1;
    let (r0, r1) = MAST;
    let r_at = |z: f32| r0 + (r1 - r0) * (z - z0) / (MAST_TOP - z0);
    dark_plate(b);
    b.with_facets(|b| b.prism(Vec3::Z * (z0 - 0.2), 8, r0, r1, MAST_TOP - z0 + 0.2));
    metal(b);
    hoop(
        b,
        Vec3::Z * (MAST_TOP - 0.25),
        r1 * 0.95,
        0.5,
        0.5,
        b.sides(16),
    );
    let flat = FRAC_PI_8.cos();
    for q in 0..4 {
        let a = FRAC_PI_2 * q as f32;
        let out = v3(a.cos(), a.sin(), 0.0);
        dark_plate(b);
        for (k, &z) in [11.6f32, 15.6, 19.6].iter().enumerate() {
            let len = 4.2 - 0.3 * k as f32;
            let top = out * (r_at(z + len) * flat + 0.05) + Vec3::Z * (z + len);
            let foot = out * (r_at(z) * flat + 0.05) + Vec3::Z * z;
            let f = Frame::new(top, foot - top, out);
            let half = r_at(z + len) * FRAC_PI_8.sin() * 0.95;
            armour(b, &f, &swept(len + 0.6, half, 0.0, 0.35), 0.28);
        }
        if b.fine() {
            red_slot(
                b,
                out * (r_at(22.6) * flat + 0.1) + Vec3::Z * 22.6,
                out,
                Vec3::Z,
                0.9,
                0.22,
            );
        }
        // The chute down the diagonal face beside it.
        let d = a + FRAC_PI_4;
        let dir = v3(d.cos(), d.sin(), 0.0);
        chute(
            b,
            dir * (r_at(MAST_TOP - 1.2) * flat + 0.4) + Vec3::Z * (MAST_TOP - 1.2),
            dir * (r0 * flat + 0.4) + Vec3::Z * (z0 + 0.1),
            0.38,
        );
    }
}

/// A glazed chute carrying what the strands bring home from `from` down to `to`: dark
/// `MASS_FLOW` glazing that runs with the Materials red while the tower reclaims, a
/// graphite collar at each end at full detail.
fn chute(b: &mut MeshBuilder, from: Vec3, to: Vec3, r: f32) {
    let sides = if b.fine() { 8 } else { 4 };
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.cylinder_between(from, to, r, r, sides);
    if b.fine() {
        let dir = (to - from).normalize_or_zero();
        metal(b);
        for end in [from + dir * 0.35, to - dir * 0.35] {
            b.cylinder_between(end - dir * 0.3, end + dir * 0.3, r * 1.3, r * 1.3, sides);
        }
    }
}

/// The head `i` at `pivot`, the Breaker's at `k` times its size: its house turns about
/// the pivot and the head in it pitches.
fn nanite_head(b: &mut MeshBuilder, i: usize, pivot: Vec3, k: f32) {
    let frame = Affine3A::from_translation(pivot) * Affine3A::from_scale(Vec3::splat(k));
    b.with_house(i, pivot, 0.0, |b| {
        b.with(frame, |b| {
            house(b, Vec3::ZERO);
            b.with_recoil(|b| head(b, Vec3::ZERO, Vec3::X * HEAD_REACH));
        });
    });
}

fn mast_head(b: &mut MeshBuilder) {
    nanite_head(b, 0, PIVOT, K);
}

/// The +y flank's head (`side` 1) or the -y one's (-1): reclaim heads 1 and 2.
fn flank_head(b: &mut MeshBuilder, side: f32) {
    let i = if side > 0.0 { 1 } else { 2 };
    nanite_head(b, i, FLANK * v3(1.0, side, 1.0), FLANK_K);
}

/// The +y flank pylon (mirrored for -y): a plated column up to the flank head's collar,
/// swept plates lapped down its outer face, a strut to the mast, and a chute from its
/// head down into the vault.
fn pylon(b: &mut MeshBuilder) {
    let p = FLANK;
    let top = p.z - 0.62 * FLANK_K;
    dark_plate(b);
    b.with_facets(|b| {
        b.frustum(
            v3(p.x, p.y, 0.0),
            Vec2::new(4.4, 4.2),
            Vec2::new(2.8, 2.6),
            top,
            Vec2::ZERO,
        )
    });
    metal(b);
    hoop(b, p.with_z(top - 0.2), 1.25, 0.4, 0.4, b.sides(12));
    // The outer face's plates, lapped down it.
    let hi = v3(p.x, p.y + 1.3, top - 0.4);
    let lo = v3(p.x, p.y + 2.1, 0.6);
    let f = Frame::new(hi, lo - hi, Vec3::Y);
    dark_plate(b);
    let (count, step) = if b.fine() { (3, 3.6) } else { (2, 5.4) };
    Course {
        count,
        step,
        len: 4.0,
        half: 1.5,
        tip: 0.0,
        thick: 0.3,
        tail: 0.6,
    }
    .lay(b, &f);
    if b.fine() {
        red_slot(
            b,
            v3(p.x + 1.78, p.y, top - 2.0),
            Vec3::X,
            Vec3::Z,
            1.6,
            0.24,
        );
    }
    // Braced to the mast.
    strut(b, v3(p.x, p.y - 1.1, top - 1.2), v3(0.0, 3.0, 17.5), 0.7);
    chute(
        b,
        v3(p.x - 1.6, p.y - 1.0, top - 0.6),
        v3(p.x - 1.6, VAULT[2].0 * 0.95, VAULT[2].1 - 0.4),
        0.32,
    );
}

/// One tech 3 buttress against the vault's corner on +x (yawed round to each): a plated
/// fin from the ground up to the shoulder, its plate swept out into a spike at the foot.
fn buttress(b: &mut MeshBuilder) {
    let [(r0, z0), (r1, z1), _] = VAULT;
    dark_plate(b);
    b.with_facets(|b| {
        b.extrude_y(
            &[
                [r0 - 0.2, z0],
                [r0 + 2.6, 0.0],
                [r0 + 1.6, 1.4],
                [r1 + 0.8, z1 + 0.4],
                [r1 - 0.4, z1 + 0.6],
            ],
            -0.9,
            0.9,
        )
    });
    if b.fine() {
        metal(b);
        b.cylinder_between(
            v3(r0 + 0.3, -1.05, 1.6),
            v3(r0 + 0.3, 1.05, 1.6),
            0.42,
            0.42,
            8,
        );
        red_slot(
            b,
            v3(r0 + 1.15, 0.0, 2.0),
            v3(0.8, 0.0, 0.6),
            Vec3::Z,
            0.8,
            0.14,
        );
    }
}

/// The tech 3 ring floating round the mast: a graphite hoop with teeth round its rim,
/// turning (`part::SPINNER`), held off the mast by gravity like the Orrery's.
fn ring(b: &mut MeshBuilder) {
    b.with_part(part::SPINNER, |b| {
        metal(b);
        let segs = b.sides(24);
        hoop(b, Vec3::Z * RING_Z, 4.7, 1.1, 0.7, segs);
        if b.fine() {
            teeth(b, Vec3::Z * RING_Z, 5.25, 20, v3(0.45, 0.45, 0.5));
            seam(b);
            hoop(b, Vec3::Z * (RING_Z + 0.4), 4.7, 0.5, 0.1, segs);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::build_lod;
    use crate::rig;

    #[test]
    fn fits_the_librarys_checks_at_every_tier() {
        for tech in 1..=3 {
            super::super::check_at("regency_crucible", tech, RADIUS, HEIGHT, Some(3), &[]);
        }
    }

    /// Every head the blueprint has is a house at its pivot with the violet lens's tip at
    /// its emitter, and the model has no house the blueprint lacks.
    #[test]
    fn the_unit_files_heads_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        for tech in 1..=3u8 {
            let key = format!("regency_t{tech}_reclaimer");
            let bp = blueprints.unit(blueprints.id_of(&key).unwrap());
            assert_eq!(bp.visual.mesh, "regency_crucible");
            assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3, "{key}");
            assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3, "{key}");
            let heads = bp.reclaimer.expect("it reclaims").heads().to_vec();
            let model =
                crate::build_model_scaled("regency_crucible", RADIUS, HEIGHT, tech).unwrap();
            assert_eq!(model.houses.len(), heads.len(), "{key}: a house a head");
            let fine = build_lod("regency_crucible", 0, tech);
            let mesh = fine.mesh();
            for (i, head) in heads.iter().enumerate() {
                let house = model
                    .houses
                    .iter()
                    .position(|h| h.weapon as usize == i)
                    .unwrap_or_else(|| panic!("{key}: no house for head {i}"));
                let pivot = v(head.pivot.expect("a head turns about its pivot"));
                assert!(Vec3::from(model.houses[house].pivot).distance(pivot) < 1e-3);
                let emitter = v(head.emitter);
                let near = mesh
                    .vertices
                    .iter()
                    .filter(|v| v.material == GLOW_VIOLET && v.rig & rig::RECOIL != 0)
                    .map(|v| Vec3::from(v.pos).distance(emitter))
                    .fold(f32::MAX, f32::min);
                // The lens narrows to a ring 0.05 of the Breaker's metres across at its tip.
                assert!(
                    near < 0.06 * K,
                    "{key} head {i}: lens {near} m from the emitter"
                );
            }
        }
    }
}

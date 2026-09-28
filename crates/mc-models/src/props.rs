//! Map props: trees, rocks and city buildings. Authored at scale 1; the
//! renderer applies each prop's own scale and heading.

use glam::{Vec2, Vec3};

use super::builder::{chamfered_rect, hash_unit, MeshBuilder};
use super::library::ModelDef;
use super::material::*;
use crate::foliage::{BROADLEAF_REGIONS, CONIFER_REGIONS, TROPICAL_REGIONS};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("tree_conifer", 3.9, 16.2, tree_conifer).with_far(),
    ModelDef::new("tree_pine", 5.2, 20.2, tree_pine).with_far(),
    ModelDef::new("tree_broadleaf", 6.0, 14.6, tree_broadleaf).with_far(),
    ModelDef::new("tree_dead", 2.9, 10.1, tree_dead),
    ModelDef::new("tree_palm", 5.5, PALM_HEIGHT, tree_palm),
    ModelDef::new("tree_jungle", 9.0, JUNGLE_HEIGHT, tree_jungle),
    ModelDef::new("rock_small", 2.2, 1.8, rock_small),
    ModelDef::new("rock_large", 6.0, 5.0, rock_large),
    ModelDef::new("building_small", 12.0, 9.0, building_small),
    ModelDef::new("building_medium", 17.0, 20.0, building_medium),
    ModelDef::new("building_wide", 23.0, 14.0, building_wide),
    ModelDef::new("building_tower", 15.0, 64.0, building_tower),
];

pub(super) fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

// ---- trees -------------------------------------------------------------------
//
// Crowns are cutout leaf cards (`MeshBuilder::leaf_card`) on real branches.
// Each card carries the shape of the crown it belongs to, so the shader can
// light a crown as lumps of foliage rather than as flat cards, and darken its
// heart and underside. Budgets: maps carry hundreds of thousands of trees, so
// the reduced level is ~100 triangles, the coarse one a handful of cards, and
// the far one (a tree a few pixels across) two cards and no trunk.

/// `leaf_card` tag bit: the card shows the conifer atlas.
const CONIFER_ATLAS: u32 = 0x80;
/// Bark pattern that asks the shader for pine bark instead of broadleaf bark.
/// (Patterns only mean panel detail on plated materials.)
pub(super) const PINE_BARK: u32 = super::pattern::PLAIN;

pub(super) fn card_tag(conifer: bool, seed: u32, i: u32) -> u32 {
    (hash_unit(seed, i) * 127.0) as u32 | if conifer { CONIFER_ATLAS } else { 0 }
}

/// Two unit axes across a card facing `normal`, turned `spin` radians about it.
pub(super) fn across(normal: Vec3, spin: f32) -> (Vec3, Vec3) {
    let n = normal.normalize();
    let helper = if n.z.abs() > 0.95 { Vec3::X } else { Vec3::Z };
    let r = helper.cross(n).normalize();
    let u = n.cross(r);
    let (s, c) = spin.sin_cos();
    (r * c + u * s, u * c - r * s)
}

/// Unit vector at `azimuth` (radians about z) raised `elevation` radians.
pub(super) fn heading(azimuth: f32, elevation: f32) -> Vec3 {
    v3(
        azimuth.cos() * elevation.cos(),
        azimuth.sin() * elevation.cos(),
        elevation.sin(),
    )
}

/// How a leaf at `p` sits in an ellipsoidal lobe (`lobe`, radii `lobe_r`) of
/// the whole crown (`crown`, radii `crown_r`): the lobe's own surface normal
/// bent toward the crown's, and how deep in the crown and under it the leaf is.
pub(super) fn lobe_shade(
    p: Vec3,
    lobe: Vec3,
    lobe_r: Vec3,
    crown: Vec3,
    crown_r: Vec3,
) -> [f32; 4] {
    let q = (p - lobe) / lobe_r;
    let g = (p - crown) / crown_r;
    let n = ((q / lobe_r).normalize_or(Vec3::Z) * 0.62
        + (g / crown_r).normalize_or(Vec3::Z) * 0.38)
        .normalize_or(Vec3::Z);
    let depth = (1.0 - g.length()).clamp(0.0, 1.0) * 0.8
        + (1.0 - q.length()).clamp(0.0, 1.0) * 0.3
        + (-g.z).clamp(0.0, 1.0) * 0.3;
    [n.x, n.y, n.z, depth.min(1.0)]
}

/// Bark from the ground (rooted a metre down, so trees on slopes never float) to `top`.
pub(super) fn trunk(b: &mut MeshBuilder, joints: &[(Vec3, f32)], fine_sides: usize, pine: bool) {
    b.paint(BARK);
    if pine {
        b.pattern(PINE_BARK);
    }
    let sides = if b.coarse() { 3 } else { b.sides(fine_sides) };
    for pair in joints.windows(2) {
        b.cylinder_between(pair[0].0, pair[1].0, pair[0].1, pair[1].1, sides);
    }
}

pub(super) fn limb(b: &mut MeshBuilder, from: Vec3, to: Vec3, r0: f32, r1: f32, pine: bool) {
    b.paint(BARK);
    if pine {
        b.pattern(PINE_BARK);
    }
    let sides = if b.fine() { 5 } else { 3 };
    b.cylinder_between(from, to, r0, r1, sides);
}

// Broadleaf: a flared trunk forks into scaffold limbs that carry an irregular
// crown of leafy lobes, each lobe a dome of round leaf-cluster cards.
const BROAD_CROWN: Vec3 = Vec3::new(0.2, 0.0, 9.3);
const BROAD_CROWN_R: Vec3 = Vec3::new(5.3, 5.3, 4.4);
/// Lobe centre, radius, and the scaffold limb (index into `BROAD_LIMBS`) it grows from.
const BROAD_LOBES: [(Vec3, f32, usize); 11] = [
    (Vec3::new(3.3, 1.0, 7.7), 2.7, 0),
    (Vec3::new(0.9, 3.4, 8.1), 2.6, 1),
    (Vec3::new(-2.9, 2.0, 7.9), 2.8, 1),
    (Vec3::new(-2.6, -2.4, 8.3), 2.6, 2),
    (Vec3::new(1.4, -3.3, 7.6), 2.7, 2),
    (Vec3::new(2.1, 1.9, 10.6), 2.5, 0),
    (Vec3::new(-1.6, 1.5, 10.9), 2.5, 3),
    (Vec3::new(-1.1, -1.9, 10.6), 2.5, 3),
    (Vec3::new(2.1, -1.5, 10.2), 2.4, 0),
    (Vec3::new(0.4, 0.2, 12.0), 2.3, 3),
    (Vec3::new(3.9, -0.8, 9.1), 2.0, 0),
];
const BROAD_FORK: Vec3 = Vec3::new(0.25, -0.1, 4.7);
const BROAD_LIMBS: [(Vec3, f32); 4] = [
    (Vec3::new(1.9, 0.6, 7.4), 0.24),
    (Vec3::new(-1.0, 1.6, 7.6), 0.23),
    (Vec3::new(-0.3, -1.8, 7.5), 0.22),
    (Vec3::new(0.2, 0.1, 10.0), 0.26),
];

fn tree_broadleaf(b: &mut MeshBuilder, _tech: u8) {
    let shade = |lobe: Vec3, r: f32| {
        move |p: Vec3| lobe_shade(p, lobe, v3(r, r, r * 0.8), BROAD_CROWN, BROAD_CROWN_R)
    };
    if b.far() {
        // The coarse lid for the view from above, and a clump standing across the
        // crown for the view from the side.
        let lid = BROAD_CROWN + Vec3::Z * 1.6;
        b.leaf_card(
            lid,
            v3(5.2, 0.0, 0.0),
            v3(0.0, 5.2, 0.0),
            BROADLEAF_REGIONS[3],
            card_tag(false, 5, 0),
            shade(lid, 4.5),
        );
        let (r, u) = across(heading(0.4, 0.0), 0.0);
        b.leaf_card(
            BROAD_CROWN - Vec3::Z * 0.6,
            r * 5.0,
            u * 4.2,
            BROADLEAF_REGIONS[3],
            card_tag(false, 5, 1),
            shade(BROAD_CROWN, 4.5),
        );
        return;
    }
    let base = if b.coarse() { -1.0 } else { 0.6 };
    if !b.coarse() {
        // Root flare.
        trunk(
            b,
            &[(v3(0.0, 0.0, -1.0), 0.95), (v3(0.03, -0.01, 0.6), 0.47)],
            8,
            false,
        );
    }
    trunk(
        b,
        &[(v3(0.03, -0.01, base), 0.47), (BROAD_FORK, 0.37)],
        8,
        false,
    );
    if b.coarse() {
        // Five dense clump cards: a lid and four tilted around it.
        let lid = BROAD_CROWN + Vec3::Z * 2.4;
        b.leaf_card(
            lid,
            v3(4.9, 0.0, 0.0),
            v3(0.0, 4.9, 0.0),
            BROADLEAF_REGIONS[3],
            card_tag(false, 5, 0),
            shade(lid, 4.5),
        );
        for k in 0..4 {
            let d = heading(k as f32 * 1.571 + 0.4, 0.55);
            let (r, u) = across(d, k as f32 * 0.9);
            let c = BROAD_CROWN + d * 2.9 - Vec3::Z * 1.1;
            b.leaf_card(
                c,
                r * 4.0,
                u * 4.0,
                BROADLEAF_REGIONS[3],
                card_tag(false, 5, k + 1),
                shade(c, 4.0),
            );
        }
        return;
    }
    for &(tip, radius) in &BROAD_LIMBS {
        limb(
            b,
            BROAD_FORK - Vec3::Z * 0.3,
            tip,
            radius,
            radius * 0.55,
            false,
        );
    }
    // Reduced: the lower ring, the top, and two between.
    let fine = b.fine();
    let keep = |i: usize| fine || matches!(i, 0..=4 | 6 | 8 | 9);
    for (i, &(lobe, r, from)) in BROAD_LOBES.iter().enumerate().filter(|(i, _)| keep(*i)) {
        let i = i as u32;
        let outward = (lobe - BROAD_CROWN).with_z(0.0).normalize_or(Vec3::X);
        let azimuth = outward.y.atan2(outward.x);
        if b.fine() {
            let (anchor, radius) = BROAD_LIMBS[from];
            limb(
                b,
                anchor,
                lobe - outward * r * 0.25,
                radius * 0.5,
                0.05,
                false,
            );
            // A dome of clusters: a lid, then five around it, tilted out.
            let lid = lobe + (Vec3::Z * 0.7 + outward * 0.3) * r * 0.45;
            let (x, y) = across(Vec3::Z + outward * 0.35, hash_unit(31, i) * 6.3);
            b.leaf_card(
                lid,
                x * r,
                y * r,
                BROADLEAF_REGIONS[(i % 2) as usize],
                card_tag(false, 31, i),
                shade(lobe, r),
            );
            for k in 0..5 {
                let az = azimuth + k as f32 * 1.2566 + hash_unit(37, i * 8 + k) * 0.6;
                let d = heading(az, 0.18 + hash_unit(41, i * 8 + k) * 0.35);
                let (x, y) = across(d, hash_unit(43, i * 8 + k) * 6.3);
                let size = r * (0.78 + hash_unit(47, i * 8 + k) * 0.2);
                b.leaf_card(
                    lobe + d * r * 0.42,
                    x * size,
                    y * size,
                    BROADLEAF_REGIONS[((i + k) % 2) as usize],
                    card_tag(false, 53, i * 8 + k),
                    shade(lobe, r),
                );
            }
            // A branch end reaching out of the lobe's lower side breaks the outline.
            if lobe.z < 9.0 {
                let grow = (outward - Vec3::Z * 0.25).normalize();
                let side = Vec3::Z.cross(outward).normalize();
                let tilt = (side + Vec3::Z * (hash_unit(57, i) - 0.5) * 0.6).normalize();
                let c = lobe + outward * r * 0.75 - Vec3::Z * r * 0.25 + grow * r * 0.45;
                b.leaf_card(
                    c,
                    tilt * r * 0.55,
                    grow * r * 0.6,
                    BROADLEAF_REGIONS[2],
                    card_tag(false, 59, i),
                    shade(lobe, r),
                );
            }
        } else {
            // Reduced: a lid and one card toward the outside, both dense clumps.
            let size = r * 1.12;
            let lid = lobe + Vec3::Z * r * 0.3;
            let (x, y) = across(Vec3::Z + outward * 0.4, hash_unit(31, i) * 6.3);
            b.leaf_card(
                lid,
                x * size,
                y * size,
                BROADLEAF_REGIONS[3],
                card_tag(false, 31, i),
                shade(lobe, r),
            );
            let d = heading(azimuth + 0.3, 0.3);
            let (x, y) = across(d, hash_unit(43, i) * 6.3);
            b.leaf_card(
                lobe + d * r * 0.35,
                x * size,
                y * size,
                BROADLEAF_REGIONS[(i % 2) as usize],
                card_tag(false, 53, i),
                shade(lobe, r),
            );
        }
    }
}

// Conifer (fir / spruce): a straight trunk to a narrow spire, whorls of flat
// fronds drooping more toward the bottom.
const FIR_TOP: f32 = 16.2;
const FIR_LOW: f32 = 1.7;
const FIR_REACH: f32 = 3.7;

fn fir_reach(z: f32) -> f32 {
    let t = ((z - FIR_LOW) / (FIR_TOP - 0.6 - FIR_LOW)).clamp(0.0, 1.0);
    0.35 + FIR_REACH * (1.0 - t).powf(0.92)
}

fn fir_shade(p: Vec3) -> [f32; 4] {
    let radial = p.with_z(0.0);
    let out = radial.normalize_or(Vec3::X);
    let n = (out + Vec3::Z * 0.6).normalize();
    let t = ((p.z - FIR_LOW) / (FIR_TOP - FIR_LOW)).clamp(0.0, 1.0);
    let depth = (1.0 - radial.length() / fir_reach(p.z)).clamp(0.0, 1.0) * 0.75 + (1.0 - t) * 0.3;
    [n.x, n.y, n.z, depth.min(1.0)]
}

fn tree_conifer(b: &mut MeshBuilder, _tech: u8) {
    if b.far() {
        // Two of the coarse level's crossed silhouettes, a little fuller.
        for k in 0..2 {
            let d = heading(k as f32 * std::f32::consts::FRAC_PI_2 + 0.3, 0.0);
            b.leaf_card(
                v3(0.0, 0.0, FIR_TOP * 0.5),
                d * FIR_REACH * 1.15,
                Vec3::Z * FIR_TOP * 0.5,
                CONIFER_REGIONS[2],
                card_tag(true, 7, k),
                |p| {
                    let [x, y, z, w] = fir_shade(p);
                    [x, y, z, w * 0.5]
                },
            );
        }
        return;
    }
    if b.coarse() {
        trunk(
            b,
            &[(v3(0.0, 0.0, -1.0), 0.42), (v3(0.0, 0.0, 2.5), 0.34)],
            3,
            true,
        );
        // Three crossed silhouettes of the whole tree, and a tuft across the middle
        // for the view from above.
        for k in 0..3 {
            let d = heading(k as f32 * std::f32::consts::FRAC_PI_3 + 0.3, 0.0);
            b.leaf_card(
                v3(0.0, 0.0, FIR_TOP * 0.5),
                d * FIR_REACH * 1.05,
                Vec3::Z * FIR_TOP * 0.5,
                CONIFER_REGIONS[2],
                card_tag(true, 7, k),
                |p| {
                    let [x, y, z, w] = fir_shade(p);
                    [x, y, z, w * 0.5]
                },
            );
        }
        return;
    }
    if b.fine() {
        trunk(
            b,
            &[(v3(0.0, 0.0, -1.0), 0.72), (v3(0.0, 0.0, 0.5), 0.4)],
            7,
            true,
        );
    }
    trunk(
        b,
        &[
            (v3(0.0, 0.0, if b.fine() { 0.5 } else { -1.0 }), 0.4),
            (v3(0.05, 0.0, FIR_TOP - 0.3), 0.06),
        ],
        7,
        true,
    );
    // A dense dark core: two crossed silhouettes of the whole tree, so the
    // gaps between fronds show foliage behind rather than the trunk.
    for k in 0..2 {
        let d = heading(k as f32 * std::f32::consts::FRAC_PI_2 + 0.8, 0.0);
        b.leaf_card(
            v3(0.0, 0.0, FIR_TOP * 0.5 + 0.3),
            d * FIR_REACH * 0.78,
            Vec3::Z * FIR_TOP * 0.47,
            CONIFER_REGIONS[2],
            card_tag(true, 7, k),
            fir_shade,
        );
    }
    let (levels, arms) = if b.fine() { (15, 6) } else { (8, 4) };
    for level in 0..levels {
        let t = level as f32 / (levels - 1) as f32;
        let z = FIR_LOW + t * (FIR_TOP - 1.4 - FIR_LOW) + hash_unit(61, level) * 0.3;
        let reach = fir_reach(z) * if b.fine() { 1.0 } else { 1.12 };
        let droop = 0.12 + 0.34 * (1.0 - t);
        for arm in 0..arms {
            let id = level * 8 + arm;
            let az = level as f32 * 2.39996
                + arm as f32 * std::f32::consts::TAU / arms as f32
                + hash_unit(67, id) * 0.5;
            let out = heading(az, -droop);
            let side = Vec3::Z.cross(out).normalize();
            let roll = (hash_unit(71, id) - 0.5) * 0.5;
            let width = side * roll.cos() + out.cross(side) * roll.sin();
            let root = v3(0.05 * t, 0.0, z);
            let length = reach * (0.92 + hash_unit(73, id) * 0.16);
            b.leaf_card(
                root + out * length * 0.5,
                out * length * 0.5,
                width * length * 0.36,
                CONIFER_REGIONS[0],
                card_tag(true, 79, id),
                fir_shade,
            );
        }
        if b.fine() && level < 2 {
            // Dead lower twigs, shaded out.
            limb(
                b,
                v3(0.0, 0.0, z - 0.5),
                v3(0.0, 0.0, z - 0.5) + heading(level as f32 * 2.0, -0.2) * 1.3,
                0.05,
                0.015,
                true,
            );
        }
    }
    // Leader: two crossed fronds pointing up.
    for k in 0..2 {
        let d = heading(k as f32 * std::f32::consts::FRAC_PI_2 + 0.4, 0.0);
        b.leaf_card(
            v3(0.05, 0.0, FIR_TOP - 0.9),
            Vec3::Z * 0.95,
            d * 0.42,
            CONIFER_REGIONS[0],
            card_tag(true, 83, k),
            fir_shade,
        );
    }
}

// Pine: a tall, slightly leaning bare trunk under an umbrella of needle pads.
const PINE_CROWN: Vec3 = Vec3::new(0.6, 0.0, 17.0);
const PINE_CROWN_R: Vec3 = Vec3::new(4.6, 4.6, 2.8);
const PINE_BEND: Vec3 = Vec3::new(0.35, 0.1, 9.5);
const PINE_TOP: Vec3 = Vec3::new(0.8, 0.25, 18.6);
/// Pad centre and radius.
const PINE_PADS: [(Vec3, f32); 9] = [
    (Vec3::new(3.1, 0.9, 15.6), 2.2),
    (Vec3::new(-2.0, 2.2, 16.1), 2.1),
    (Vec3::new(0.6, -3.0, 15.4), 2.1),
    (Vec3::new(-1.7, -1.8, 17.4), 2.0),
    (Vec3::new(2.5, -1.4, 18.0), 1.9),
    (Vec3::new(1.1, 1.9, 18.5), 2.0),
    (Vec3::new(0.8, 0.2, 19.2), 1.8),
    (Vec3::new(-2.9, 0.2, 15.2), 1.8),
    (Vec3::new(3.0, 2.8, 16.9), 1.6),
];

fn pine_trunk_at(z: f32) -> Vec3 {
    if z < PINE_BEND.z {
        Vec3::ZERO.lerp(PINE_BEND, z / PINE_BEND.z)
    } else {
        PINE_BEND.lerp(PINE_TOP, (z - PINE_BEND.z) / (PINE_TOP.z - PINE_BEND.z))
    }
}

fn tree_pine(b: &mut MeshBuilder, _tech: u8) {
    let root = v3(-0.05, -0.01, -1.0);
    let shade = |pad: Vec3, r: f32| {
        move |p: Vec3| lobe_shade(p, pad, v3(r, r, r * 0.5), PINE_CROWN, PINE_CROWN_R)
    };
    if b.far() {
        // The flat crown seen from above, and across it seen from the side.
        let top = PINE_CROWN - Vec3::Z * 0.4;
        let (x, y) = across(Vec3::Z, 0.4);
        b.leaf_card(
            top,
            x * PINE_CROWN_R.x,
            y * PINE_CROWN_R.y,
            CONIFER_REGIONS[1],
            card_tag(true, 89, 0),
            shade(top, 4.0),
        );
        let (x, y) = across(heading(0.4, 0.0), 0.0);
        b.leaf_card(
            top,
            x * PINE_CROWN_R.x,
            y * PINE_CROWN_R.z,
            CONIFER_REGIONS[1],
            card_tag(true, 89, 1),
            shade(top, 4.0),
        );
        return;
    }
    if b.coarse() {
        trunk(b, &[(root, 0.5), (PINE_TOP, 0.14)], 3, true);
    } else {
        trunk(
            b,
            &[(root, 0.52), (PINE_BEND, 0.36), (PINE_TOP, 0.13)],
            8,
            true,
        );
    }
    if b.coarse() {
        for (k, &i) in [0usize, 1, 2, 5].iter().enumerate() {
            let (pad, r) = PINE_PADS[i];
            let c = (pad + PINE_CROWN * 0.35) / 1.35 + Vec3::Z * 0.3;
            let (x, y) = across(
                Vec3::Z + (pad - PINE_CROWN).with_z(0.0) * 0.12,
                k as f32 * 1.7,
            );
            b.leaf_card(
                c,
                x * r * 1.55,
                y * r * 1.55,
                CONIFER_REGIONS[1],
                card_tag(true, 89, k as u32),
                shade(c, r * 1.5),
            );
        }
        return;
    }
    let pads = if b.fine() {
        &PINE_PADS[..]
    } else {
        &PINE_PADS[..6]
    };
    for (i, &(pad, r)) in pads.iter().enumerate() {
        let i = i as u32;
        let from = pine_trunk_at(pad.z - 2.8);
        if b.fine() || i < 4 {
            limb(b, from, pad - Vec3::Z * 0.2, 0.17, 0.06, true);
        }
        let outward = (pad - PINE_CROWN).with_z(0.0).normalize_or(Vec3::X);
        let (x, y) = across(Vec3::Z + outward * 0.15, hash_unit(97, i) * 6.3);
        b.leaf_card(
            pad + Vec3::Z * 0.15,
            x * r,
            y * r,
            CONIFER_REGIONS[1],
            card_tag(true, 97, i),
            shade(pad, r),
        );
        let tufts = if b.fine() { 3 } else { 1 };
        for k in 0..tufts {
            let az = outward.y.atan2(outward.x)
                + (k as f32 - 1.0) * 2.1
                + hash_unit(101, i * 4 + k) * 0.5;
            let d = heading(az, 0.75);
            let (x, y) = across(d, hash_unit(103, i * 4 + k) * 6.3);
            let size = r * 0.72;
            b.leaf_card(
                pad + d * r * 0.45 - Vec3::Z * 0.25,
                x * size,
                y * size,
                CONIFER_REGIONS[1],
                card_tag(true, 107, i * 4 + k),
                shade(pad, r),
            );
        }
    }
    if b.fine() {
        // Stubs of shed lower branches.
        for (k, z) in [7.0f32, 9.5, 11.8].into_iter().enumerate() {
            let at = pine_trunk_at(z);
            limb(
                b,
                at,
                at + heading(k as f32 * 2.3 + 0.5, 0.2) * 0.9,
                0.08,
                0.03,
                true,
            );
        }
    }
}

// Dead: a snag with a splintered top and broken limbs.
fn tree_dead(b: &mut MeshBuilder, _tech: u8) {
    let bend = v3(0.3, -0.15, 5.5);
    let top = v3(0.55, -0.25, 9.0);
    if b.coarse() {
        trunk(b, &[(v3(0.0, 0.0, -1.0), 0.5), (top, 0.2)], 3, false);
    } else {
        trunk(
            b,
            &[(v3(0.0, 0.0, -1.0), 0.55), (bend, 0.36), (top, 0.2)],
            7,
            false,
        );
        // Splinters where the top broke off.
        b.paint(BARK);
        b.cylinder_between(top, top + v3(-0.05, 0.1, 1.1), 0.17, 0.01, 3);
        if b.fine() {
            b.cylinder_between(top, top + v3(0.15, -0.1, 0.6), 0.12, 0.01, 3);
        }
    }
    let limbs: &[(Vec3, Vec3, f32)] = &[
        (v3(0.15, -0.05, 3.4), v3(2.1, 0.9, 5.6), 0.2),
        (v3(0.3, -0.1, 4.8), v3(-1.8, -0.9, 6.9), 0.18),
        (v3(0.4, -0.2, 6.4), v3(0.9, 1.8, 8.0), 0.14),
        (v3(2.1, 0.9, 5.6), v3(2.5, 1.1, 6.9), 0.09),
        (v3(-1.8, -0.9, 6.9), v3(-2.3, -0.7, 8.2), 0.08),
        (v3(0.5, -0.2, 7.4), v3(1.9, -1.2, 8.6), 0.1),
        (v3(2.1, 0.9, 5.6), v3(2.9, 0.2, 6.0), 0.06),
        (v3(0.1, 0.0, 2.2), v3(-1.0, 0.8, 2.9), 0.1),
    ];
    let count = match b.lod() {
        0 => limbs.len(),
        1 => 4,
        _ => 2,
    };
    for &(from, to, radius) in limbs.iter().take(count) {
        limb(b, from, to, radius, radius * 0.4, false);
    }
}

// ---- tropical trees ------------------------------------------------------------
//
// Both show the tropical leaf atlas (`foliage::TROPICAL`, picked with
// `MeshBuilder::leaf_atlas`) and pale bark (entity.wgsl `BARK_PALE_PATTERN`,
// ringed on the palm).

/// Bark patterns the shader reads as tropical bark. (Patterns only mean panel
/// detail on plated materials.)
pub(super) const PALE_BARK: u32 = super::pattern::SHUTTER;
const RINGED_BARK: u32 = super::pattern::DECK;
/// Leaf-card pattern that asks for the tropical atlas.
const TROPICAL_LEAVES: u32 = super::pattern::PLAIN;

/// A bark tube through `joints` (centre, radius) with `sides` facets, capped
/// at its two ends only.
pub(super) fn stem(b: &mut MeshBuilder, joints: &[(Vec3, f32)], sides: usize, bark: u32) {
    b.paint(BARK);
    b.pattern(bark);
    let rings: Vec<Vec<Vec3>> = joints
        .iter()
        .enumerate()
        .map(|(k, &(c, r))| {
            // Each ring square to the stem there.
            let prev = joints[k.saturating_sub(1)].0;
            let next = joints[(k + 1).min(joints.len() - 1)].0;
            let axis = (next - prev).normalize_or(Vec3::Z);
            let (x, y) = across(axis, 0.0);
            (0..sides)
                .map(|i| {
                    let a = (i as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
                    c + (x * a.cos() + y * a.sin()) * r
                })
                .collect()
        })
        .collect();
    b.loft(&rings, true, true);
}

// Coconut palm: a slender ringed trunk curving up and over to one side, a
// crown of long fronds arching out and down, coconuts under it.
const PALM_HEIGHT: f32 = 15.2;
/// Where the fronds spring from: the top of the trunk.
const PALM_TOP: Vec3 = Vec3::new(2.4, 0.35, 13.7);
const PALM_CROWN: Vec3 = Vec3::new(2.4, 0.35, 13.2);
const PALM_CROWN_R: Vec3 = Vec3::new(5.0, 5.0, 2.6);

/// The trunk's centre and radius a share `t` of the way up: nearly upright at
/// the foot, leaning more toward the top, a slight swell under the crown.
fn palm_trunk_at(t: f32) -> (Vec3, f32) {
    let z = -1.0 + t * (PALM_TOP.z + 1.0);
    let lean = t.max(0.0).powf(1.8);
    let at = v3(
        PALM_TOP.x * lean,
        PALM_TOP.y * (t * std::f32::consts::PI).sin() * 0.6 + PALM_TOP.y * lean * 0.4,
        z,
    );
    let radius = 0.3 - 0.11 * t + 0.035 * (-((t - 0.93) / 0.06).powi(2)).exp();
    (at, radius)
}

fn palm_shade(p: Vec3) -> [f32; 4] {
    let g = (p - PALM_CROWN) / PALM_CROWN_R;
    let n = (g + Vec3::Z * 0.35).normalize_or(Vec3::Z);
    let depth = (1.0 - g.length()).clamp(0.0, 1.0) * 0.55 + (-g.z).clamp(0.0, 1.0) * 0.3;
    [n.x, n.y, n.z, depth.min(1.0)]
}

/// One frond from the crown along `azimuth`, leaving it `rise` radians up and
/// arching over by `droop` radians more at its tip, `length` metres along in
/// `segments` flat pieces.
/// Each segment shows its share of the frond picture, folded into a shallow V
/// down the rachis (`fold`, or flat and whole when `fold` is None).
fn palm_frond(
    b: &mut MeshBuilder,
    azimuth: f32,
    rise: f32,
    droop: f32,
    length: f32,
    segments: u32,
    fold: Option<f32>,
    seed: u32,
) {
    let [u0, v0, u1, v1] = TROPICAL_REGIONS[0];
    let vm = (v0 + v1) * 0.5;
    let half_width = length * 0.21;
    let step = length / segments as f32;
    let mut at = PALM_TOP + heading(azimuth, 0.0) * 0.25;
    for k in 0..segments {
        // Stiff near the crown, bowing more toward the tip.
        let elevation = rise - droop * ((k as f32 + 0.5) / segments as f32).powf(1.5);
        let dir = heading(azimuth, elevation);
        let side = Vec3::Z.cross(dir).normalize_or(Vec3::Y);
        let up = dir.cross(side);
        let mid = at + dir * step * 0.5;
        let (a, z) = (
            u0 + (u1 - u0) * k as f32 / segments as f32,
            u0 + (u1 - u0) * (k + 1) as f32 / segments as f32,
        );
        // A frond's first stretch is its bare stalk: keep the cards a little wider there.
        let w = half_width * if k == 0 { 0.9 } else { 1.0 };
        let tag = card_tag(false, seed, k);
        match fold {
            Some(f) => {
                // Pinnae hang down either side of the rachis.
                let (c, s) = (f.cos(), f.sin());
                let out_a = side * c - up * s;
                let out_b = -side * c - up * s;
                b.leaf_card(
                    mid + out_a * w * 0.5,
                    dir * step * 0.5,
                    out_a * w * 0.5,
                    [a, v0, z, vm],
                    tag,
                    palm_shade,
                );
                b.leaf_card(
                    mid + out_b * w * 0.5,
                    dir * step * 0.5,
                    -out_b * w * 0.5,
                    [a, vm, z, v1],
                    tag,
                    palm_shade,
                );
            }
            None => {
                b.leaf_card(
                    mid,
                    dir * step * 0.5,
                    side * w,
                    [a, v0, z, v1],
                    tag,
                    palm_shade,
                );
            }
        }
        at += dir * step;
    }
}

fn tree_palm(b: &mut MeshBuilder, _tech: u8) {
    // Trunk: rings of leaf scars (the shader's), a flared foot.
    let joints = match b.lod() {
        0 => 10,
        1 => 4,
        _ => 1,
    };
    let mut trunk: Vec<(Vec3, f32)> = (0..=joints)
        .map(|k| palm_trunk_at(k as f32 / joints as f32))
        .collect();
    if b.fine() {
        trunk.insert(1, (v3(0.0, 0.0, -0.4), 0.36));
        trunk[0].1 = 0.46;
    }
    let sides = match b.lod() {
        0 => 8,
        1 => 5,
        _ => 3,
    };
    stem(b, &trunk, sides, RINGED_BARK);
    b.leaf_atlas(TROPICAL_LEAVES);
    // Fronds: a lower ring hanging out and down, an upper one rising first,
    // and at full detail two young spears standing up in the middle.
    let (count, lower, segments, fold) = match b.lod() {
        0 => (11, 6, 3, Some(0.5)),
        1 => (7, 4, 2, None),
        _ => (5, 3, 1, None),
    };
    for i in 0..count {
        let id = i as u32;
        let upper = i >= lower;
        let azimuth = i as f32 * 2.39996 + hash_unit(211, id) * 0.3;
        let (rise, droop, length) = if upper {
            (0.72, 1.1, 5.3)
        } else {
            (0.26, 1.55, 6.0)
        };
        let rise = rise + (hash_unit(223, id) - 0.5) * 0.2;
        let length = length * (0.9 + hash_unit(227, id) * 0.18);
        palm_frond(b, azimuth, rise, droop, length, segments, fold, 229 + id);
    }
    if b.fine() {
        for k in 0..2 {
            palm_frond(
                b,
                0.8 + k as f32 * 3.0,
                1.2,
                0.4,
                3.2,
                2,
                Some(0.35),
                241 + k,
            );
        }
        // The crown's boot of old frond bases and a bunch of coconuts under it.
        b.paint(BARK);
        b.pattern(PALE_BARK);
        b.spheroid(PALM_TOP - Vec3::Z * 0.15, v3(0.42, 0.42, 0.55), 6, 3);
        b.paint(BARK);
        for k in 0..4 {
            let a = k as f32 * 1.7 + 0.4;
            let c = PALM_TOP
                + v3(
                    a.cos() * 0.42,
                    a.sin() * 0.42,
                    -0.55 - (k % 2) as f32 * 0.22,
                );
            b.spheroid(c, v3(0.2, 0.2, 0.23), 5, 3);
        }
    }
    b.leaf_atlas(super::pattern::NONE);
}

// Jungle hardwood: a pale straight bole on buttress fins, scaffold limbs
// spreading high up to a broad umbrella of leaf pads in three tiers.
const JUNGLE_HEIGHT: f32 = 22.5;
const JUNGLE_FORK: Vec3 = Vec3::new(0.2, 0.1, 13.2);
const JUNGLE_CROWN: Vec3 = Vec3::new(0.2, 0.1, 17.8);
const JUNGLE_CROWN_R: Vec3 = Vec3::new(9.0, 9.0, 4.4);
/// Pad centre, radius (flattened to 0.45 of it), and tier: 0 the wide lower
/// umbrella, 1 the middle, 2 the emergent top.
const JUNGLE_PADS: [(Vec3, f32, u32); 13] = [
    (Vec3::new(6.3, 0.9, 15.3), 3.3, 0),
    (Vec3::new(3.2, 5.6, 15.7), 3.2, 0),
    (Vec3::new(-2.5, 6.0, 15.1), 3.4, 0),
    (Vec3::new(-6.2, 1.4, 15.6), 3.2, 0),
    (Vec3::new(-4.6, -4.4, 15.2), 3.3, 0),
    (Vec3::new(0.9, -6.3, 15.8), 3.2, 0),
    (Vec3::new(5.4, -3.6, 15.4), 3.0, 0),
    (Vec3::new(3.0, 1.6, 18.6), 3.0, 1),
    (Vec3::new(-1.4, 3.0, 18.9), 2.9, 1),
    (Vec3::new(-2.8, -1.6, 18.4), 3.0, 1),
    (Vec3::new(1.4, -2.9, 18.8), 2.8, 1),
    (Vec3::new(0.4, 0.3, 21.2), 2.8, 2),
    (Vec3::new(-1.3, -0.9, 20.7), 2.0, 2),
];

fn tree_jungle(b: &mut MeshBuilder, _tech: u8) {
    let sides = match b.lod() {
        0 => 9,
        1 => 5,
        _ => 3,
    };
    let bole = [
        (v3(0.0, 0.0, -1.0), 0.95),
        (v3(0.0, 0.0, 1.2), 0.62),
        (v3(0.1, 0.05, 7.0), 0.5),
        (JUNGLE_FORK, 0.42),
    ];
    if b.coarse() {
        stem(b, &[bole[0], bole[3]], sides, PALE_BARK);
    } else {
        stem(b, &bole, sides, PALE_BARK);
    }
    if b.fine() {
        // Buttress fins: thin plates flaring from the bole into the ground.
        b.paint(BARK);
        b.pattern(PALE_BARK);
        for k in 0..5 {
            let a = k as f32 * 1.2566 + hash_unit(251, k) * 0.5;
            let out = heading(a, 0.0);
            let across = Vec3::Z.cross(out) * 0.5;
            let fin = |reach: f32, thick: f32, top: f32| {
                let c = out * reach;
                vec![
                    c - across * thick + Vec3::Z * -1.0,
                    c + across * thick + Vec3::Z * -1.0,
                    c + across * thick + Vec3::Z * top,
                    c - across * thick + Vec3::Z * top,
                ]
            };
            let tall = 2.2 + hash_unit(257, k) * 1.2;
            b.loft(
                &[
                    fin(0.35, 0.3, tall),
                    fin(1.2, 0.2, tall * 0.4),
                    fin(2.0 + hash_unit(263, k) * 0.6, 0.1, -0.35),
                ],
                true,
                true,
            );
        }
    }
    // Scaffold limbs from the fork out under each lower pad, and on up to the top ones.
    let limbs: &[usize] = match b.lod() {
        0 => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        1 => &[0, 2, 4, 6, 8, 11],
        _ => &[],
    };
    for &i in limbs {
        let (pad, r, tier) = JUNGLE_PADS[i];
        let from = JUNGLE_FORK - Vec3::Z * 0.4;
        let to = pad - Vec3::Z * r * 0.3;
        let radius = [0.26, 0.2, 0.17][tier as usize];
        b.paint(BARK);
        b.pattern(PALE_BARK);
        let fine_sides = if b.fine() { 5 } else { 3 };
        if b.fine() && tier == 0 {
            // A crook partway out: the limb rises then levels.
            let knee = from + (to - from) * 0.5 + Vec3::Z * 1.1;
            b.cylinder_between(from, knee, radius, radius * 0.75, fine_sides);
            b.cylinder_between(knee, to, radius * 0.75, radius * 0.4, fine_sides);
        } else {
            b.cylinder_between(from, to, radius, radius * 0.45, fine_sides);
        }
    }
    b.leaf_atlas(TROPICAL_LEAVES);
    let shade = |pad: Vec3, r: f32| {
        move |p: Vec3| lobe_shade(p, pad, v3(r, r, r * 0.45), JUNGLE_CROWN, JUNGLE_CROWN_R)
    };
    if b.coarse() {
        // A wide lid over the whole umbrella and four pads tilted out round it.
        let lid = JUNGLE_CROWN + Vec3::Z * 0.9;
        b.leaf_card(
            lid,
            v3(5.0, 0.0, 0.0),
            v3(0.0, 5.0, 0.0),
            TROPICAL_REGIONS[2],
            card_tag(false, 269, 0),
            shade(lid, 4.5),
        );
        for k in 0..4 {
            let d = heading(k as f32 * 1.571 + 0.5, 0.0);
            let (x, y) = across(Vec3::Z * 1.4 + d, k as f32 * 0.9);
            let c = JUNGLE_CROWN + d * 5.0 - Vec3::Z * 2.0;
            b.leaf_card(
                c,
                x * 4.6,
                y * 4.6,
                TROPICAL_REGIONS[2],
                card_tag(false, 269, k + 1),
                shade(c, 4.0),
            );
        }
        b.leaf_atlas(super::pattern::NONE);
        return;
    }
    let fine = b.fine();
    let keep = |i: usize| fine || matches!(i, 0..=6 | 7 | 9 | 11);
    for (i, &(pad, r, tier)) in JUNGLE_PADS.iter().enumerate().filter(|(i, _)| keep(*i)) {
        let i = i as u32;
        let outward = (pad - JUNGLE_CROWN).with_z(0.0).normalize_or(Vec3::X);
        let azimuth = outward.y.atan2(outward.x);
        if fine {
            // A flat pad: a broad, nearly level lid of big leaves, a ring of four
            // round its rim leaning out and a little down, and on the lower
            // tier a skirt hanging off the outer edge.
            let (x, y) = across(Vec3::Z + outward * 0.1, hash_unit(271, i) * 6.3);
            b.leaf_card(
                pad + Vec3::Z * r * 0.18,
                x * r * 1.1,
                y * r * 1.1,
                TROPICAL_REGIONS[1],
                card_tag(false, 271, i),
                shade(pad, r),
            );
            for k in 0..4 {
                let id = i * 8 + k;
                let az =
                    azimuth + k as f32 * std::f32::consts::FRAC_PI_2 + hash_unit(277, id) * 0.5;
                let d = heading(az, -0.12 + hash_unit(281, id) * 0.2);
                let (x, y) = across(Vec3::Z * 1.7 + d, hash_unit(283, id) * 6.3);
                let size = r * (0.7 + hash_unit(293, id) * 0.2);
                let region = TROPICAL_REGIONS[if (i + k).is_multiple_of(3) { 2 } else { 1 }];
                b.leaf_card(
                    pad + d * r * 0.6,
                    x * size,
                    y * size,
                    region,
                    card_tag(false, 307, id),
                    shade(pad, r),
                );
            }
            if tier == 0 {
                let c = pad + outward * r * 0.85 - Vec3::Z * r * 0.3;
                let (x, y) = across(outward + Vec3::Z * 0.7, hash_unit(311, i) * 6.3);
                b.leaf_card(
                    c,
                    x * r * 0.65,
                    y * r * 0.65,
                    TROPICAL_REGIONS[2],
                    card_tag(false, 313, i),
                    shade(pad, r),
                );
            }
        } else {
            // Reduced: a dense lid and one card leaning out.
            let size = r * 1.15;
            let (x, y) = across(Vec3::Z + outward * 0.3, hash_unit(271, i) * 6.3);
            b.leaf_card(
                pad + Vec3::Z * r * 0.15,
                x * size,
                y * size,
                TROPICAL_REGIONS[2],
                card_tag(false, 271, i),
                shade(pad, r),
            );
            let d = heading(azimuth + 0.3, 0.0);
            let (x, y) = across(Vec3::Z * 1.5 + d, hash_unit(283, i) * 6.3);
            b.leaf_card(
                pad + d * r * 0.55 - Vec3::Z * r * 0.1,
                x * size * 0.85,
                y * size * 0.85,
                TROPICAL_REGIONS[1],
                card_tag(false, 307, i),
                shade(pad, r),
            );
        }
    }
    b.leaf_atlas(super::pattern::NONE);
}

// ---- rocks ---------------------------------------------------------------------

fn boulder(b: &mut MeshBuilder, center: Vec3, radii: Vec3, seed: u32) {
    b.paint(ROCK);
    let (sides, rings) = match b.lod() {
        0 => (8, 5),
        1 => (6, 4),
        _ => (5, 3),
    };
    // Sunk a little so the jagged underside never shows.
    b.lumpy_spheroid(
        center - Vec3::Z * radii.z * 0.25,
        radii,
        sides,
        rings,
        0.22,
        seed,
    );
}

fn rock_small(b: &mut MeshBuilder, _tech: u8) {
    boulder(b, v3(0.0, 0.0, 0.75), v3(2.0, 1.5, 1.25), 5);
    if b.fine() {
        boulder(b, v3(1.3, -1.1, 0.35), v3(0.8, 0.7, 0.55), 6);
    }
}

fn rock_large(b: &mut MeshBuilder, _tech: u8) {
    boulder(b, v3(-0.4, 0.3, 2.2), v3(4.8, 3.9, 3.4), 7);
    if b.mid() {
        b.yawed(v3(3.3, -2.4, 0.0), 0.8, |b| {
            boulder(b, v3(0.0, 0.0, 1.0), v3(2.4, 1.7, 1.6), 8)
        });
    }
    if b.fine() {
        boulder(b, v3(-3.6, -3.0, 0.5), v3(1.3, 1.1, 0.9), 9);
        boulder(b, v3(1.0, 4.2, 0.6), v3(1.5, 1.1, 1.0), 10);
    }
}

// ---- city buildings --------------------------------------------------------------

/// Concrete block with lit window bands on every storey (every other storey
/// at reduced detail). `base` is the ground-level centre.
fn storeys(b: &mut MeshBuilder, base: Vec3, size: Vec3, chamfer: f32, storey: f32) {
    b.paint(CONCRETE);
    // Ground-level blocks reach 2 m down so buildings on slopes never float.
    let sunk = if base.z == 0.0 { 2.0 } else { 0.0 };
    if b.coarse() {
        b.cuboid_open(
            base + Vec3::Z * ((size.z - sunk) * 0.5),
            size + Vec3::Z * sunk,
        );
        return;
    }
    // Corner chamfers only read up close.
    let plan = chamfered_rect(
        Vec2::new(size.x, size.y) * 0.5,
        if b.fine() { chamfer } else { 0.0 },
    );
    b.at(base, |b| {
        b.extrude_z(&plan, -sunk, size.z);
        let floors = ((size.z - 1.5) / storey).floor() as usize;
        let (stride, band) = if b.fine() {
            (1, storey * 0.45)
        } else {
            (2, storey * 0.9)
        };
        // Bands stand 12 cm proud of the wall.
        let grow = Vec2::new(1.0 + 0.24 / size.x, 1.0 + 0.24 / size.y);
        b.paint(WINDOWS);
        for floor in (0..floors).step_by(stride) {
            let z = 1.6 + storey * floor as f32;
            let ring = |z: f32| {
                plan.iter()
                    .map(|p| v3(p[0] * grow.x, p[1] * grow.y, z))
                    .collect::<Vec<_>>()
            };
            b.loft(&[ring(z), ring(z + band)], false, false);
        }
    });
}

/// Rooftop clutter at full detail: a parapet around `size` and `units` air handlers.
fn rooftop(b: &mut MeshBuilder, center: Vec3, size: Vec2, units: u32, seed: u32) {
    if !b.fine() {
        return;
    }
    b.paint(CONCRETE);
    for side in [-1.0, 1.0] {
        b.cuboid_open(
            center + v3(side * (size.x - 0.5) * 0.5, 0.0, 0.4),
            v3(0.5, size.y, 0.8),
        );
        b.cuboid_open(
            center + v3(0.0, side * (size.y - 0.5) * 0.5, 0.4),
            v3(size.x - 1.0, 0.5, 0.8),
        );
    }
    b.paint(METAL);
    for i in 0..units {
        let p = Vec2::new(hash_unit(seed, i) - 0.5, hash_unit(seed, 10 + i) - 0.5) * size * 0.6;
        b.cuboid_open(
            center + v3(p.x, p.y, 0.6),
            v3(2.2 + hash_unit(seed, 20 + i) * 1.5, 1.8, 1.2),
        );
    }
}

fn building_small(b: &mut MeshBuilder, _tech: u8) {
    storeys(b, Vec3::ZERO, v3(18.0, 14.0, 7.5), 0.0, 3.2);
    // Pitched roof.
    b.paint(ACCENT);
    b.extrude_x(&[[-7.6, 7.5], [7.6, 7.5], [0.0, 9.6]], -9.4, 9.4);
    if b.mid() {
        b.paint(CONCRETE);
        b.cuboid_open(v3(11.5, -2.0, 1.1), v3(5.0, 8.0, 4.2));
    }
    if b.fine() {
        // Chimney, dormers, annex roof lights.
        b.paint(CONCRETE);
        b.block(v3(-5.5, 2.4, 8.0), v3(-4.3, 3.6, 10.6));
        b.paint(ACCENT);
        for x in [-2.0, 3.5] {
            b.extrude_x(
                &[
                    [-7.2, 7.6],
                    [-4.4, 7.6],
                    [-4.4, 8.4],
                    [-5.8, 9.0],
                    [-7.2, 8.4],
                ],
                x,
                x + 2.2,
            );
        }
        b.paint(WINDOWS);
        for x in [-2.0, 3.5] {
            b.block(v3(x + 0.5, -7.3, 7.7), v3(x + 1.7, -7.2, 8.4));
        }
        b.plate(v3(11.5, -2.0, 3.2), Vec2::new(2.0, 4.0), 0.3, 0.15);
    }
}

fn building_medium(b: &mut MeshBuilder, _tech: u8) {
    storeys(b, Vec3::ZERO, v3(26.0, 22.0, 16.0), 1.5, 3.4);
    if b.mid() {
        storeys(b, v3(-4.0, 0.0, 16.0), v3(14.0, 16.0, 4.0), 1.0, 3.4);
    }
    rooftop(b, v3(7.5, 0.0, 16.0), Vec2::new(9.0, 20.0), 3, 3);
}

fn building_wide(b: &mut MeshBuilder, _tech: u8) {
    // Low slab with two wings and a raised atrium.
    storeys(b, Vec3::ZERO, v3(38.0, 20.0, 9.5), 1.0, 3.6);
    storeys(b, v3(-11.0, 0.0, 0.0), v3(12.0, 30.0, 12.5), 1.0, 3.6);
    if b.mid() {
        storeys(b, v3(12.0, 0.0, 0.0), v3(10.0, 28.0, 7.0), 1.0, 3.6);
        b.paint(GLASS);
        b.extrude_x(&[[-5.0, 9.5], [5.0, 9.5], [0.0, 13.0]], -4.0, 6.0);
    }
    rooftop(b, v3(-11.0, 0.0, 12.5), Vec2::new(11.0, 28.0), 4, 5);
}

fn building_tower(b: &mut MeshBuilder, _tech: u8) {
    // Podium, setback shaft, crown and mast.
    storeys(b, Vec3::ZERO, v3(28.0, 28.0, 10.0), 3.0, 3.6);
    storeys(b, v3(0.0, 0.0, 10.0), v3(20.0, 20.0, 34.0), 3.0, 3.8);
    if b.mid() {
        storeys(b, v3(0.0, 0.0, 44.0), v3(14.0, 14.0, 12.0), 2.5, 3.8);
        b.paint(CONCRETE);
        b.prism(v3(0.0, 0.0, 56.0), 4, 6.0, 2.0, 3.0);
    } else {
        b.paint(CONCRETE);
        b.frustum_open(
            v3(0.0, 0.0, 44.0),
            Vec2::splat(14.0),
            Vec2::splat(9.0),
            15.0,
            Vec2::ZERO,
        );
    }
    if b.fine() {
        b.paint(METAL);
        b.cylinder_between(v3(0.0, 0.0, 59.0), v3(0.0, 0.0, 66.0), 0.4, 0.12, 5);
        b.paint(WINDOWS);
        b.cuboid(v3(0.0, 0.0, 66.0), Vec3::splat(0.6));
    }
    rooftop(b, v3(0.0, 0.0, 10.0), Vec2::new(27.0, 27.0), 0, 9);
    rooftop(b, v3(0.0, 0.0, 44.0), Vec2::new(19.0, 19.0), 0, 9);
}

/// A tree's far level is a real saving on its coarse one (the renderer draws it for
/// hundreds of thousands of trees at once), and still has a crown to show.
#[cfg(test)]
#[test]
fn far_levels_are_cheaper_than_coarse() {
    for def in MODELS.iter().filter(|def| def.far) {
        let model = super::build_model(def.key).unwrap();
        let far = model.far.as_ref().expect("a far level");
        let coarse = model.lods[super::LOD_COUNT - 1].indices.len() / 3;
        let tris = far.indices.len() / 3;
        assert!(
            tris > 0 && tris * 2 <= coarse,
            "{}: far {tris} of coarse {coarse}",
            def.key
        );
        assert!(
            far.vertices
                .iter()
                .all(|v| v.material == super::material::FOLIAGE),
            "{}: the far level is crown only",
            def.key
        );
    }
}

/// Writes each tree's levels of detail as raw `MeshVertex` / index arrays to
/// `$TREE_DUMP_DIR` (default `target/tree-dump`), for offline preview renders:
/// `cargo test -p mc-models -- --ignored dump_trees`
#[cfg(test)]
#[test]
#[ignore = "writes inspection files"]
fn dump_trees() {
    let dir = std::env::var_os("TREE_DUMP_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tree-dump")
        });
    std::fs::create_dir_all(&dir).unwrap();
    for def in &MODELS[..4] {
        let model = super::build_model(def.key).unwrap();
        for (lod, mesh) in model.lods.iter().enumerate() {
            let mut bytes = Vec::new();
            bytes.extend((mesh.vertices.len() as u32).to_le_bytes());
            bytes.extend((mesh.indices.len() as u32).to_le_bytes());
            bytes.extend(bytemuck::cast_slice::<_, u8>(&mesh.vertices));
            bytes.extend(bytemuck::cast_slice::<_, u8>(&mesh.indices));
            std::fs::write(dir.join(format!("{}_lod{lod}.bin", def.key)), bytes).unwrap();
            println!("{} lod{lod}: {} triangles", def.key, mesh.indices.len() / 3);
        }
    }
}

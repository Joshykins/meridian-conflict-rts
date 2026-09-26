//! Desert map props (canyon country): Utah juniper, pinyon pine, Fremont
//! cottonwood and a fallen block of bedded sandstone. Authored at scale 1, like
//! the other props (props.rs, whose tree helpers these share). The trees show
//! the desert leaf atlas (`foliage::DESERT`), picked by the leaf-card pattern
//! `gpu_consts::scenery::LEAF_DESERT`.

use glam::Vec3;

use super::builder::{hash_unit, MeshBuilder};
use super::library::ModelDef;
use super::material::*;
use super::props::{across, card_tag, heading, limb, lobe_shade, stem, trunk, v3, PALE_BARK};
use crate::foliage::DESERT_REGIONS;
use crate::gpu_consts::scenery;

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("tree_juniper", JUNIPER_REACH, JUNIPER_HEIGHT, tree_juniper),
    ModelDef::new("tree_pinyon", PINYON_REACH, PINYON_HEIGHT, tree_pinyon),
    ModelDef::new(
        "tree_cottonwood",
        COTTONWOOD_REACH,
        COTTONWOOD_HEIGHT,
        tree_cottonwood,
    ),
    ModelDef::new("rock_slab", 4.4, SLAB_HEIGHT, rock_slab),
];

/// Crown radius of each desert tree at scale 1, metres (ground cover, previews).
pub(crate) const JUNIPER_REACH: f32 = 3.0;
pub(crate) const PINYON_REACH: f32 = 3.7;
pub(crate) const COTTONWOOD_REACH: f32 = 8.5;

const JUNIPER_REGION: [f32; 4] = DESERT_REGIONS[0];
const PINYON_REGION: [f32; 4] = DESERT_REGIONS[1];
const COTTONWOOD_REGION: [f32; 4] = DESERT_REGIONS[2];
const COTTONWOOD_CLUMP: [f32; 4] = DESERT_REGIONS[3];

/// A dome of leaf cards over the lobe at `lobe` (radius `r`): a lid tipped
/// toward the outside and `ring` cards round it, leaning out. Reduced and
/// coarse levels take a lid and one card.
#[expect(
    clippy::too_many_arguments,
    reason = "one lobe's shape, look and seed, spelt out at each call"
)]
fn lobe_dome(
    b: &mut MeshBuilder,
    lobe: Vec3,
    r: f32,
    outward: Vec3,
    ring: u32,
    region: [f32; 4],
    seed: u32,
    shade: impl Fn(Vec3) -> [f32; 4] + Copy,
) {
    let azimuth = outward.y.atan2(outward.x);
    let lid = lobe + (Vec3::Z * 0.7 + outward * 0.3) * r * 0.45;
    let (x, y) = across(Vec3::Z + outward * 0.35, hash_unit(seed, 0) * 6.3);
    let size = if ring > 1 { r } else { r * 1.12 };
    b.leaf_card(
        lid,
        x * size,
        y * size,
        region,
        card_tag(false, seed, 0),
        shade,
    );
    for k in 0..ring {
        let az =
            azimuth + k as f32 * std::f32::consts::TAU / ring as f32 + hash_unit(seed + 1, k) * 0.6;
        let d = heading(az, 0.15 + hash_unit(seed + 2, k) * 0.35);
        let (x, y) = across(d, hash_unit(seed + 3, k) * 6.3);
        let size = size * (0.78 + hash_unit(seed + 4, k) * 0.2);
        b.leaf_card(
            lobe + d * r * 0.42,
            x * size,
            y * size,
            region,
            card_tag(false, seed + 5, k),
            shade,
        );
    }
}

// ---- Utah juniper ------------------------------------------------------------
//
// Squat and many-stemmed: three twisted stems from one swollen root, shaggy
// silver bark, a ragged crown of blue-grey lobes thrown to one side, and on
// the other a dead limb reaching out bare.

const JUNIPER_HEIGHT: f32 = 6.3;
const JUNIPER_CROWN: Vec3 = Vec3::new(0.5, 0.2, 4.2);
const JUNIPER_CROWN_R: Vec3 = Vec3::new(2.9, 2.7, 2.0);
/// Each stem's joints (centre, radius), twisting up out of the root.
const JUNIPER_STEMS: [[(Vec3, f32); 4]; 3] = [
    [
        (Vec3::new(0.12, 0.0, 0.2), 0.36),
        (Vec3::new(0.75, 0.4, 1.4), 0.29),
        (Vec3::new(1.05, 0.2, 2.6), 0.23),
        (Vec3::new(1.7, 0.8, 3.7), 0.14),
    ],
    [
        (Vec3::new(-0.1, 0.12, 0.2), 0.33),
        (Vec3::new(-0.65, 0.45, 1.3), 0.27),
        (Vec3::new(-0.75, 1.15, 2.4), 0.2),
        (Vec3::new(-1.35, 1.35, 3.4), 0.12),
    ],
    [
        (Vec3::new(0.0, -0.12, 0.2), 0.34),
        (Vec3::new(0.35, -0.65, 1.5), 0.26),
        (Vec3::new(0.05, -1.0, 2.9), 0.19),
        (Vec3::new(0.5, -1.4, 4.1), 0.12),
    ],
];
/// Lobe centre, radius, and the stem it grows from.
const JUNIPER_LOBES: [(Vec3, f32, usize); 8] = [
    (Vec3::new(1.9, 0.9, 4.1), 1.5, 0),
    (Vec3::new(1.2, 0.0, 5.1), 1.35, 0),
    (Vec3::new(2.4, -0.2, 3.4), 1.1, 0),
    (Vec3::new(-1.3, 1.5, 3.8), 1.35, 1),
    (Vec3::new(-0.3, 1.0, 5.0), 1.25, 1),
    (Vec3::new(0.6, -1.5, 4.5), 1.4, 2),
    (Vec3::new(0.35, -0.6, 5.55), 1.1, 2),
    (Vec3::new(0.9, 1.7, 3.1), 1.0, 0),
];
/// The dead limb: from the second stem out over the thin side.
const JUNIPER_SNAG: [(Vec3, f32); 3] = [
    (Vec3::new(-0.65, 0.45, 1.3), 0.14),
    (Vec3::new(-2.0, -0.4, 2.5), 0.09),
    (Vec3::new(-2.75, -0.75, 3.4), 0.03),
];

fn tree_juniper(b: &mut MeshBuilder, _tech: u8) {
    let bark = scenery::BARK_SHAGGY;
    let sides = match b.lod() {
        0 => 7,
        1 => 5,
        _ => 3,
    };
    if b.coarse() {
        stem(
            b,
            &[(v3(0.0, 0.0, -1.0), 0.6), (v3(0.6, 0.1, 3.0), 0.25)],
            sides,
            bark,
        );
    } else {
        // The swollen root the stems share.
        stem(
            b,
            &[
                (v3(0.0, 0.0, -1.0), 0.8),
                (v3(0.0, 0.0, 0.2), 0.62),
                (v3(0.05, 0.0, 0.7), 0.45),
            ],
            sides,
            bark,
        );
        for joints in &JUNIPER_STEMS {
            let joints = if b.fine() { &joints[..] } else { &joints[..3] };
            stem(b, joints, sides, bark);
        }
        stem(b, &JUNIPER_SNAG, if b.fine() { 5 } else { 3 }, bark);
        if b.fine() {
            // A forked twig off the snag, and a stub where a limb broke.
            b.paint(BARK).pattern(bark);
            b.cylinder_between(JUNIPER_SNAG[1].0, v3(-2.4, 0.3, 3.3), 0.05, 0.015, 3);
            b.cylinder_between(JUNIPER_STEMS[2][1].0, v3(0.9, -1.2, 1.9), 0.09, 0.05, 4);
        }
    }
    b.leaf_atlas(scenery::LEAF_DESERT);
    let shade = |lobe: Vec3, r: f32| {
        move |p: Vec3| lobe_shade(p, lobe, v3(r, r, r * 0.75), JUNIPER_CROWN, JUNIPER_CROWN_R)
    };
    if b.coarse() {
        let lid = JUNIPER_CROWN + Vec3::Z * 1.0;
        b.leaf_card(
            lid,
            v3(2.6, 0.3, 0.0),
            v3(-0.3, 2.4, 0.0),
            JUNIPER_REGION,
            card_tag(false, 401, 0),
            shade(lid, 2.4),
        );
        for k in 0..4 {
            let d = heading(k as f32 * 1.571 + 0.3, 0.5);
            let (r, u) = across(d, k as f32 * 0.9);
            let c = JUNIPER_CROWN + d * 1.7 - Vec3::Z * 0.6;
            b.leaf_card(
                c,
                r * 2.1,
                u * 2.1,
                JUNIPER_REGION,
                card_tag(false, 401, k + 1),
                shade(c, 2.0),
            );
        }
        b.leaf_atlas(super::pattern::NONE);
        return;
    }
    let fine = b.fine();
    for (i, &(lobe, r, from)) in JUNIPER_LOBES.iter().enumerate() {
        let i = i as u32;
        let outward = (lobe - JUNIPER_CROWN).with_z(0.0).normalize_or(Vec3::X);
        if fine {
            let tip = JUNIPER_STEMS[from][3].0;
            stem(b, &[(tip, 0.08), (lobe - outward * r * 0.3, 0.03)], 3, bark);
        }
        let ring = if fine { 5 } else { 1 };
        lobe_dome(
            b,
            lobe,
            r,
            outward,
            ring,
            JUNIPER_REGION,
            411 + i * 8,
            shade(lobe, r),
        );
    }
    b.leaf_atlas(super::pattern::NONE);
}

// ---- pinyon pine -------------------------------------------------------------
//
// A small, round, dense dark pine: a short trunk, a few stout limbs up into a
// crown of needle brushes from near the ground to a blunt top.

const PINYON_HEIGHT: f32 = 9.1;
const PINYON_CROWN: Vec3 = Vec3::new(0.1, 0.05, 4.7);
const PINYON_CROWN_R: Vec3 = Vec3::new(3.8, 3.8, 4.3);
const PINYON_FORK: Vec3 = Vec3::new(0.1, 0.05, 1.3);
/// Rings of lobes: (height, distance out, lobe radius, count). A bushy dome
/// from low down: the bare trunk shows only a metre or so.
const PINYON_RINGS: [(f32, f32, f32, u32); 5] = [
    (2.0, 2.9, 1.35, 8),
    (3.7, 2.8, 1.35, 7),
    (5.4, 2.2, 1.3, 6),
    (7.0, 1.3, 1.2, 4),
    (8.3, 0.2, 0.95, 1),
];

/// The pinyon's lobes, jittered off their rings: (centre, radius).
fn pinyon_lobes() -> Vec<(Vec3, f32)> {
    let mut lobes = Vec::new();
    for (ring, &(z, out, r, count)) in PINYON_RINGS.iter().enumerate() {
        for k in 0..count {
            let id = ring as u32 * 8 + k;
            let a = k as f32 * std::f32::consts::TAU / count as f32
                + ring as f32 * 0.6
                + hash_unit(503, id) * 0.5;
            let reach = out * (0.85 + hash_unit(509, id) * 0.3);
            lobes.push((
                v3(
                    a.cos() * reach,
                    a.sin() * reach,
                    z + hash_unit(521, id) * 0.5,
                ) + PINYON_CROWN.with_z(0.0),
                r * (0.9 + hash_unit(523, id) * 0.2),
            ));
        }
    }
    lobes
}

fn tree_pinyon(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        trunk(
            b,
            &[(v3(0.0, 0.0, -1.0), 0.45), (v3(0.1, 0.05, 3.0), 0.3)],
            3,
            true,
        );
    } else {
        trunk(
            b,
            &[
                (v3(0.0, 0.0, -1.0), 0.55),
                (v3(0.04, 0.0, 0.6), 0.38),
                (PINYON_FORK, 0.34),
            ],
            8,
            true,
        );
        // Stout limbs up into the crown.
        let tops = [
            v3(1.9, 0.6, 3.6),
            v3(-1.5, 1.2, 4.2),
            v3(0.2, -1.8, 4.8),
            v3(0.15, 0.1, 7.2),
        ];
        let count = if b.fine() { 4 } else { 2 };
        for &to in &tops[..count] {
            limb(b, PINYON_FORK - Vec3::Z * 0.2, to, 0.2, 0.08, true);
        }
    }
    b.leaf_atlas(scenery::LEAF_DESERT);
    let shade = |lobe: Vec3, r: f32| {
        move |p: Vec3| lobe_shade(p, lobe, v3(r, r, r * 0.8), PINYON_CROWN, PINYON_CROWN_R)
    };
    if b.coarse() {
        let lid = PINYON_CROWN + Vec3::Z * 2.6;
        b.leaf_card(
            lid,
            v3(2.8, 0.0, 0.0),
            v3(0.0, 2.8, 0.0),
            PINYON_REGION,
            card_tag(false, 531, 0),
            shade(lid, 2.6),
        );
        for k in 0..4 {
            let d = heading(k as f32 * 1.571 + 0.5, 0.35);
            let (r, u) = across(d, k as f32 * 0.9);
            let c = PINYON_CROWN + d * 2.3 - Vec3::Z * 0.9;
            b.leaf_card(
                c,
                r * 2.9,
                u * 2.9,
                PINYON_REGION,
                card_tag(false, 531, k + 1),
                shade(c, 2.5),
            );
        }
        b.leaf_atlas(super::pattern::NONE);
        return;
    }
    let fine = b.fine();
    for (i, (lobe, r)) in pinyon_lobes().into_iter().enumerate() {
        let i = i as u32;
        // Reduced: every other lobe, a little larger.
        if !fine && i % 2 == 1 && i + 1 < PINYON_RINGS.iter().map(|r| r.3).sum::<u32>() {
            continue;
        }
        let r = if fine { r } else { r * 1.3 };
        let outward = (lobe - PINYON_CROWN).with_z(0.0).normalize_or(Vec3::X);
        let ring = if fine { 4 } else { 1 };
        lobe_dome(
            b,
            lobe,
            r,
            outward,
            ring,
            PINYON_REGION,
            541 + i * 8,
            shade(lobe, r),
        );
    }
    b.leaf_atlas(super::pattern::NONE);
}

// ---- Fremont cottonwood -------------------------------------------------------
//
// The big tree by the water: a thick pale trunk forking low into great limbs
// that lean out wide, under a broad, open, bright crown.

const COTTONWOOD_HEIGHT: f32 = 17.2;
const COTTONWOOD_CROWN: Vec3 = Vec3::new(0.3, 0.3, 12.4);
const COTTONWOOD_CROWN_R: Vec3 = Vec3::new(8.0, 8.0, 4.6);
const COTTONWOOD_FORK: Vec3 = Vec3::new(0.2, 0.1, 3.6);
/// The great limbs: (elbow, tip), leaning out from the fork.
const COTTONWOOD_LIMBS: [(Vec3, Vec3, f32); 4] = [
    (Vec3::new(2.4, 0.9, 7.2), Vec3::new(5.0, 1.7, 10.4), 0.55),
    (Vec3::new(-1.9, 1.8, 7.6), Vec3::new(-4.4, 3.2, 10.8), 0.5),
    (Vec3::new(-0.3, -2.3, 7.4), Vec3::new(0.3, -5.0, 11.0), 0.5),
    (Vec3::new(0.5, 0.2, 8.6), Vec3::new(0.9, 0.6, 13.2), 0.45),
];
/// Lobe centre, radius, and the limb it grows from.
const COTTONWOOD_LOBES: [(Vec3, f32, usize); 15] = [
    (Vec3::new(6.0, 1.2, 10.6), 3.1, 0),
    (Vec3::new(4.2, 4.6, 11.0), 3.0, 1),
    (Vec3::new(-1.0, 6.0, 10.4), 3.2, 1),
    (Vec3::new(-5.4, 2.8, 10.9), 3.1, 1),
    (Vec3::new(-4.8, -2.8, 10.3), 2.9, 2),
    (Vec3::new(0.2, -6.2, 10.8), 3.2, 2),
    (Vec3::new(4.4, -4.0, 10.2), 2.9, 0),
    (Vec3::new(3.0, 1.2, 13.2), 3.0, 0),
    (Vec3::new(-1.6, 3.2, 13.4), 3.0, 3),
    (Vec3::new(-2.8, -1.6, 13.0), 2.9, 2),
    (Vec3::new(1.8, -2.6, 13.3), 2.8, 3),
    (Vec3::new(0.6, 0.4, 15.2), 2.6, 3),
    (Vec3::new(-0.8, -0.2, 14.6), 2.2, 3),
    (Vec3::new(2.4, 3.8, 9.0), 2.2, 0),
    (Vec3::new(-2.6, -4.6, 8.8), 2.1, 2),
];

fn tree_cottonwood(b: &mut MeshBuilder, _tech: u8) {
    let sides = match b.lod() {
        0 => 10,
        1 => 6,
        _ => 3,
    };
    if b.coarse() {
        stem(
            b,
            &[(v3(0.0, 0.0, -1.0), 1.1), (v3(0.4, 0.3, 9.0), 0.4)],
            sides,
            PALE_BARK,
        );
    } else {
        stem(
            b,
            &[
                (v3(0.0, 0.0, -1.0), 1.35),
                (v3(0.0, 0.0, 0.6), 0.95),
                (v3(0.1, 0.05, 2.2), 0.8),
                (COTTONWOOD_FORK, 0.72),
            ],
            sides,
            PALE_BARK,
        );
        let limb_sides = if b.fine() { 7 } else { 3 };
        for &(elbow, tip, radius) in &COTTONWOOD_LIMBS {
            stem(
                b,
                &[
                    (COTTONWOOD_FORK - Vec3::Z * 0.4, radius),
                    (elbow, radius * 0.72),
                    (tip, radius * 0.35),
                ],
                limb_sides,
                PALE_BARK,
            );
        }
    }
    b.leaf_atlas(scenery::LEAF_DESERT);
    let shade = |lobe: Vec3, r: f32| {
        move |p: Vec3| {
            lobe_shade(
                p,
                lobe,
                v3(r, r, r * 0.75),
                COTTONWOOD_CROWN,
                COTTONWOOD_CROWN_R,
            )
        }
    };
    if b.coarse() {
        let lid = COTTONWOOD_CROWN + Vec3::Z * 2.0;
        b.leaf_card(
            lid,
            v3(6.2, 0.0, 0.0),
            v3(0.0, 6.2, 0.0),
            COTTONWOOD_CLUMP,
            card_tag(false, 601, 0),
            shade(lid, 5.5),
        );
        for k in 0..4 {
            let d = heading(k as f32 * 1.571 + 0.4, 0.3);
            let (r, u) = across(Vec3::Z * 0.8 + d, k as f32 * 0.9);
            let c = COTTONWOOD_CROWN + d * 5.0 - Vec3::Z * 1.4;
            b.leaf_card(
                c,
                r * 4.6,
                u * 4.6,
                COTTONWOOD_CLUMP,
                card_tag(false, 601, k + 1),
                shade(c, 4.5),
            );
        }
        b.leaf_atlas(super::pattern::NONE);
        return;
    }
    let fine = b.fine();
    let keep = |i: usize| fine || matches!(i, 0..=7 | 9 | 11);
    for (i, &(lobe, r, from)) in COTTONWOOD_LOBES
        .iter()
        .enumerate()
        .filter(|(i, _)| keep(*i))
    {
        let i = i as u32;
        let outward = (lobe - COTTONWOOD_CROWN).with_z(0.0).normalize_or(Vec3::X);
        if fine {
            // A branch from the limb's tip out into the lobe.
            let (_, tip, radius) = COTTONWOOD_LIMBS[from];
            stem(
                b,
                &[(tip, radius * 0.35), (lobe - outward * r * 0.3, 0.06)],
                4,
                PALE_BARK,
            );
            lobe_dome(
                b,
                lobe,
                r,
                outward,
                5,
                COTTONWOOD_REGION,
                611 + i * 8,
                shade(lobe, r),
            );
            // A spray hanging off the lobe's outer side breaks the outline.
            if lobe.z < 12.0 {
                let c = lobe + outward * r * 0.8 - Vec3::Z * r * 0.35;
                let (x, y) = across(outward + Vec3::Z * 0.6, hash_unit(613, i) * 6.3);
                b.leaf_card(
                    c,
                    x * r * 0.6,
                    y * r * 0.6,
                    COTTONWOOD_REGION,
                    card_tag(false, 617, i),
                    shade(lobe, r),
                );
            }
        } else {
            lobe_dome(
                b,
                lobe,
                r * 1.05,
                outward,
                1,
                COTTONWOOD_CLUMP,
                611 + i * 8,
                shade(lobe, r),
            );
        }
    }
    b.leaf_atlas(super::pattern::NONE);
}

// ---- sandstone slab ------------------------------------------------------------
//
// A block of bedded sandstone fallen from a cliff: three or four beds stacked,
// each stepped back from the one below and rounded along its edges by the
// weather, an undercut at each parting. The beds are level in the model, so
// the shader's laminae run with them.

const SLAB_HEIGHT: f32 = 3.1;
/// Beds: (bottom, top, inset from the block's outline, shift in plan).
const SLAB_BEDS: [(f32, f32, f32, [f32; 2]); 4] = [
    (-1.0, 0.95, 0.0, [0.0, 0.0]),
    (0.95, 1.9, 0.35, [0.35, -0.2]),
    (1.9, 2.6, 0.95, [0.6, 0.1]),
    (2.6, SLAB_HEIGHT, 2.0, [1.0, 0.35]),
];

/// Bed `bed`'s outline at `a` radians round it: a squarish lozenge 8.4 m by
/// 6 m, chipped at the corners, each bed ragged in its own way and one corner
/// broken off the upper ones.
fn slab_outline(a: f32, inset: f32, bed: usize) -> [f32; 2] {
    let (s, c) = a.sin_cos();
    // Superellipse: flat sides, blunt corners.
    let k = 3.2;
    let r = (c.abs().powf(k) / 4.2f32.powf(k) + s.abs().powf(k) / 3.0f32.powf(k)).powf(-1.0 / k);
    let phase = bed as f32 * 1.9;
    let broken = if bed > 0 {
        0.22 * (1.0 - ((a - 0.8).rem_euclid(std::f32::consts::TAU) - 0.0).min(1.2) / 1.2).max(0.0)
    } else {
        0.0
    };
    let lump =
        1.0 + 0.07 * (a * 3.0 + 0.7 + phase).sin() + 0.05 * (a * 7.0 + 2.1 + phase * 1.7).sin()
            - broken;
    let r = (r * lump - inset).max(0.4);
    [c * r, s * r]
}

fn rock_slab(b: &mut MeshBuilder, _tech: u8) {
    b.paint(ROCK).pattern(scenery::ROCK_BEDDED);
    let (points, beds) = match b.lod() {
        0 => (16, 4),
        1 => (10, 3),
        _ => (6, 2),
    };
    for (k, &(z0, z1, inset, shift)) in SLAB_BEDS[..beds].iter().enumerate() {
        let ring = |z: f32, grow: f32| -> Vec<Vec3> {
            (0..points)
                .map(|i| {
                    let a =
                        (i as f32 + 0.5 * (k % 2) as f32) * std::f32::consts::TAU / points as f32;
                    let [x, y] = slab_outline(a, inset - grow, k);
                    v3(x + shift[0], y + shift[1], z)
                })
                .collect()
        };
        let rings = if b.coarse() {
            vec![ring(z0, -0.2), ring(z1, -0.3)]
        } else {
            // Undercut at the parting, full through the bed, rounded into its top.
            let h = z1 - z0;
            vec![
                ring(z0, -0.28),
                ring(z0 + h * 0.3, 0.0),
                ring(z1 - h * 0.22, 0.0),
                ring(z1 - h * 0.04, -0.22),
                ring(z1, -0.5),
            ]
        };
        b.loft(&rings, true, true);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build_model, material, MeshVertex};

    fn pattern_of(v: &MeshVertex) -> u32 {
        v.surface & 0xFF
    }

    /// Desert trees show the desert atlas, stand their height, and keep the
    /// budgets forests need (tree_canopies_are_cutout_sprays_with_bounded_lods).
    #[test]
    fn desert_trees_pick_the_desert_atlas_and_stand_their_height() {
        for (key, lo, hi) in [
            ("tree_juniper", 5.0, 7.5),
            ("tree_pinyon", 8.0, 10.0),
            ("tree_cottonwood", 15.5, 19.0),
        ] {
            let model = build_model(key).unwrap();
            let counts = model.lods.each_ref().map(|m| m.indices.len() / 3);
            assert!(
                counts[0] <= 1000 && counts[1] <= 200 && counts[2] <= 32,
                "{key}: {counts:?}"
            );
            assert!(
                counts[1] as f32 <= counts[0] as f32 * 0.45 + 20.0,
                "{key}: {counts:?}"
            );
            for mesh in &model.lods {
                let leaves: Vec<_> = mesh
                    .vertices
                    .iter()
                    .filter(|v| v.material == material::FOLIAGE)
                    .collect();
                assert!(!leaves.is_empty(), "{key}: no leaves");
                assert!(
                    leaves
                        .iter()
                        .all(|v| pattern_of(v) == super::scenery::LEAF_DESERT),
                    "{key}: leaves off the desert atlas"
                );
                assert!(
                    mesh.vertices.iter().any(|v| v.material == material::BARK),
                    "{key}: no bark"
                );
            }
            let top = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MIN, f32::max);
            assert!((lo..=hi).contains(&top), "{key}: top at {top}");
        }
        let juniper = build_model("tree_juniper").unwrap();
        assert!(juniper.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == material::BARK)
            .all(|v| pattern_of(v) == super::scenery::BARK_SHAGGY));
    }

    /// Writes a copy of a map with a line-up of the desert props added, for GPU
    /// shots of them on real ground: `LINEUP_MAP=maps/vermilion_gorge.mcmap
    /// LINEUP_OUT=/tmp/lineup.mcmap LINEUP_AT=x,y cargo test -p mc-render --lib
    /// desert_lineup -- --ignored`. Rows along +x from `LINEUP_AT`, 30 m apart:
    /// juniper, pinyon, cottonwood, slab, each at scale 1 in its first column,
    /// then a grove of eight at mixed scales and headings. Copy the map's `.ron`
    /// beside the output for its climate.
    #[test]
    #[ignore = "writes a map for inspection shots"]
    fn desert_lineup() {
        use mc_core::{Angle, Fx, FxVec2};
        use mc_map::{encode_tile, MapFile, MapWriter, Prop, PropKind};
        let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let file = MapFile::open(env("LINEUP_MAP")).unwrap();
        let at: Vec<f64> = env("LINEUP_AT")
            .split(',')
            .map(|v| v.parse().unwrap())
            .collect();
        let fx = |v: f64| Fx((v * 65536.0).round() as i64);
        let mut props = file.props().to_vec();
        let kinds = [
            PropKind::TreeJuniper,
            PropKind::TreePinyon,
            PropKind::TreeCottonwood,
            PropKind::RockSlab,
        ];
        for (row, kind) in kinds.into_iter().enumerate() {
            for col in 0..9u32 {
                let (x, y) = (at[0] + col as f64 * 24.0, at[1] + row as f64 * 30.0);
                let jitter = if col == 0 { 0.0 } else { 1.0 };
                props.push(Prop {
                    kind,
                    pos: FxVec2::new(
                        fx(x + jitter * (col as f64 * 7.3).sin() * 5.0),
                        fx(y + jitter * (col as f64 * 3.1).cos() * 6.0),
                    ),
                    heading: Angle((col * 7919 % 65536) as u16),
                    scale_milli: if col == 0 {
                        1000
                    } else {
                        700 + (col * 97 % 600) as u16
                    },
                });
            }
        }
        println!(
            "starts: {:?}",
            file.start_positions()
                .iter()
                .map(|s| s.to_f32())
                .collect::<Vec<_>>()
        );
        let out = env("LINEUP_OUT");
        let mut writer =
            MapWriter::create(std::path::Path::new(&out), file.info().clone()).unwrap();
        let (tw, th) = file.size_tiles();
        for ty in 0..th {
            for tx in 0..tw {
                writer
                    .push_tile(&encode_tile(&file.read_tile(tx, ty).unwrap()))
                    .unwrap();
            }
        }
        if let Some(snow) = file.snow() {
            writer.set_snow(snow.to_vec()).unwrap();
        }
        writer.set_wrecks(file.wrecks().to_vec()).unwrap();
        writer
            .finish(props, file.start_positions(), file.ore_regions())
            .unwrap();
    }

    /// The slab is bedded sandstone: flat-topped, wider than it is tall.
    #[test]
    fn slab_is_a_flat_bedded_block() {
        let model = build_model("rock_slab").unwrap();
        for mesh in &model.lods {
            assert!(mesh
                .vertices
                .iter()
                .all(|v| v.material == material::ROCK
                    && pattern_of(v) == super::scenery::ROCK_BEDDED));
        }
        let mesh = &model.lods[0];
        let (mut lo, mut hi) = (glam::Vec3::MAX, glam::Vec3::MIN);
        for v in &mesh.vertices {
            lo = lo.min(v.pos.into());
            hi = hi.max(v.pos.into());
        }
        let size = hi - lo;
        assert!(size.x > 7.0 && size.x < 10.0 && size.y > 5.0, "{size}");
        assert!((hi.z - super::SLAB_HEIGHT).abs() < 0.01, "{hi}");
    }
}

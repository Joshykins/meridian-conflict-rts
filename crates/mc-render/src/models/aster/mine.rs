//! The Aster core mine: a rig that drives pipe down into the ground for the ore
//! far under it, one section at a time, deeper with every blow.
//!
//! Authored on a 7x7 lot (+-42 m), opening toward +x, and shrunk evenly to the
//! blueprint's 3x3 lot: every length here is 3/7 of that in the game.
//! The middle of the lot is a hole: a pit cut into the rock in faces and
//! benches, ore seams glowing in its walls, and a narrow bore sunk from its
//! floor further than the eye can follow. The slab is a ring round it. A
//! headframe straddles the pit, and the mine works on a beat (see `pipe_offset`
//! in entity.wgsl): the pile driver (`part::RAM`) is hauled up its leader rails,
//! the next pipe section (`part::FEED`) rises out of its magazine and swings in
//! over the bore onto the string, and the driver drops and drives it and the
//! whole string (`part::STRING`) down one section. The string repeats every
//! section, so all that is seen is pipe going on down into the hole. The shader
//! draws what is down the pit over the terrain (`Model::pit`).
//!
//! On open water the same mine is an offshore rig: raised onto stilts
//! (`part::AFLOAT`), the pit and the broken ground left out (`part::ASHORE`), and
//! the pipe going down through a moon pool into the sea.
//!
//! Every piece does a job that can be read off it: nothing is there for menace.
//! The tiers start plain and grow into better machinery, each adding to the last:
//! - Tech 1, the pile rig (40 m): an open four-legged headframe with the winch in
//!   its head, the hoist cables down to the driver, the pipe magazine and a few
//!   spare sections, and one conveyor out of the pit to an ore bin.
//! - Tech 2, the mine works (66 m): the pit a bench deeper, conveyors and bins on
//!   all four sides, a tower stage with a sheave house on it and a winch house on
//!   the deck cabled up to it, more pipe in the stack.
//! - Tech 3, the induction rig (70 m): the tower clad in plate, and induction
//!   coils round the leader rails that throw the driver down, fed from two
//!   capacitor banks along conduits up the headframe; the pit a bench deeper.
//! - Tech 4, the deep core: a heavier driver on a longer stroke, fatter pipe down
//!   a wider bore, twice the coils and twice the capacitors.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use super::parts::*;
use super::structures::kit;
use crate::models::builder::{chamfered_rect, hash_unit, ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, Pit};

/// Half-width of the foundation slab.
const SLAB: f32 = 41.0;
/// Top of the slab, and the height of the pit's opening (`Model::pit`).
const DECK: f32 = 2.4;
/// Radius of the pit where it breaks through the slab.
const MOUTH: f32 = 17.0;
/// Outer edge of the spoil lip thrown up round the mouth, and its crest. On
/// water, the edge of the moon pool.
const LIP: f32 = 22.0;
const LIP_CREST: f32 = 3.3;
/// The bottom of the bore, far below anything the eye can make out down it.
const BORE_BOTTOM: f32 = -150.0;
/// Underside and top of the tech 1 winch head, where the tech 2 tower stands.
const HEAD_FOOT: f32 = 27.5;
const HEAD: f32 = 38.5;
/// Underside and top of the tech 2 sheave house.
const SHEAVE_FOOT: f32 = 55.5;
const SHEAVE: f32 = 64.5;
/// Where the headframe's legs stand, on each diagonal, clear of the lip.
const FOOT: f32 = 17.5;
/// Where the headframe's legs meet under the head.
const FRAME_TOP: f32 = 29.5;
/// Top of the pipe string when the driver has just struck.
const STRING_TOP: f32 = DECK + 1.0;
/// One pipe section: how far each blow drives the string.
const SECTION: f32 = 9.0;
/// Height of the pile driver.
const DRIVER: f32 = 4.2;
/// The leader rails the driver rides, either side of the bore (±y).
const RAIL: f32 = 3.6;
/// How far out the next section waits in its magazine, at an eighth of a turn.
const RACK_R: f32 = 24.0;
const RACK_ANGLE: f32 = TAU / 16.0;
/// Where the winch house stands, opposite the magazine.
const WINCH_ANGLE: f32 = RACK_ANGLE + TAU / 2.0;
/// The induction coils round the leader rails: inner and outer radius, and the
/// span of rail they cover, from over the magazine's trolley rail up to the head.
const COIL_IN: f32 = 4.7;
const COIL_OUT: f32 = 5.9;
const COILS_LOW: f32 = 14.5;
/// The struts that brace the coils to the headframe's legs.
const STRUT_Z: f32 = 20.0;
/// How high the rig stands on its stilts over the water.
const LIFT: f32 = 9.0;
/// How deep the stilts go: they stand on the seabed, which hides the rest (about
/// 60 m down at game scale).
const PILE_FOOT: f32 = -140.0;

pub fn core_mine(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.min(4);
    b.set_spinner_pivot(v3(0.0, 0.0, DECK));
    b.set_pit(Pit {
        open: DECK,
        radius: MOUTH * 1.07,
        stroke: SECTION + if tech >= 4 { 3.0 } else { 1.5 },
        section: SECTION,
        rack: rack().to_array(),
        afloat_lift: LIFT,
    });

    if b.coarse() {
        coarse(b, tech);
        return;
    }

    foundation(b, tech);
    b.with_part(part::AFLOAT, stilts);
    derrick(b);
    pipe(b, tech);
    ore_line(b);
    b.yawed(Vec3::ZERO, RACK_ANGLE, |b| pipe_stack(b, tech));

    kit(b, tech, 2, 0.1, |b| {
        for k in 1..4 {
            b.yawed(Vec3::ZERO, k as f32 * TAU / 4.0, ore_line);
        }
    });
    kit(b, tech, 2, 0.35, tower);
    kit(b, tech, 2, 0.6, sheave_house);
    kit(b, tech, 2, 0.8, |b| b.yawed(Vec3::ZERO, WINCH_ANGLE, winch_house));
    kit(b, tech, 2, 0.9, |b| b.yawed(Vec3::ZERO, RACK_ANGLE, |b| more_pipe(b, tech)));

    kit(b, tech, 3, 0.15, cladding);
    kit(b, tech, 3, 0.35, |b| {
        for k in [3.0, 7.0] {
            b.yawed(Vec3::ZERO, k * TAU / 8.0 - TAU / 16.0, |b| capacitors(b, 1.0));
        }
    });
    kit(b, tech, 3, 0.6, |b| coils(b, &[0.0, 3.0, 6.0, 9.0, 12.0]));

    kit(b, tech, 4, 0.2, |b| {
        for k in [3.0, 7.0] {
            b.yawed(Vec3::ZERO, k * TAU / 8.0 + TAU / 16.0, |b| capacitors(b, -1.0));
        }
    });
    kit(b, tech, 4, 0.45, |b| coils(b, &[1.5, 4.5, 7.5, 10.5]));
    kit(b, tech, 4, 0.7, weights);
}

/// Where the next pipe section waits in its magazine.
fn rack() -> Vec2 {
    Vec2::from_angle(RACK_ANGLE) * RACK_R
}

/// The pipe's radius by tier: the deep core drives a fatter string.
fn pipe_radius(tech: u8) -> f32 {
    1.5 + 0.15 * tech as f32
}

/// The pit's wall from the mouth down, as (height, radius) turns: a steep face
/// cut back into a bench, one more face and bench a tier. The last turn is the
/// floor's edge, where the bore goes down.
fn pit_profile(tech: u8) -> Vec<(f32, f32)> {
    let mut turns = vec![(DECK, MOUTH), (-5.0, 15.6), (-5.0, 13.4), (-14.0, 11.6)];
    if tech >= 2 {
        turns.extend([(-14.0, 10.2), (-22.0, 9.2)]);
    }
    if tech >= 3 {
        turns.extend([(-22.0, 8.2), (-30.0, 7.4)]);
    }
    turns
}

/// A block per tier: slab, headframe and tower stage, the pit's glow on land,
/// the stilts' bulk on water.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    b.paint(PLATING);
    b.cuboid_open(v3(0.0, 0.0, DECK * 0.5), v3(80.0, 80.0, DECK));
    b.with_part(part::ASHORE, |b| {
        b.paint(ROCK);
        b.decal(v3(0.0, 0.0, DECK + 0.05), v2(36.0, 36.0));
    });
    b.with_part(part::AFLOAT, |b| {
        b.paint(PLATING_DARK).pattern(pattern::PILE);
        b.frustum_open(v3(0.0, 0.0, -8.0), v2(70.0, 70.0), v2(62.0, 62.0), LIFT + 8.0, Vec2::ZERO);
    });
    team_panel(b, v3(36.0, 0.0, DECK), v2(5.0, 24.0));
    b.paint(PLATING_DARK);
    b.frustum_open(v3(0.0, 0.0, HEAD - 10.0), v2(19.0, 19.0), v2(17.0, 17.0), 10.0, Vec2::ZERO);
    let top = if tech >= 2 { SHEAVE } else { HEAD };
    if tech >= 2 {
        b.paint(if tech >= 3 { PLATING } else { PLATING_DARK });
        b.frustum_open(v3(0.0, 0.0, HEAD), v2(15.0, 15.0), v2(13.0, 13.0), SHEAVE - HEAD, Vec2::ZERO);
    }
    b.paint(PLATING);
    b.decal(v3(0.0, 0.0, top + 0.05), v2(10.0, 10.0));
}

// ---- Tech 1: the pile rig ------------------------------------------------------

/// One ring of the ground's profile: its height, how far out it is (`None`: the
/// slab's own chamfered edge), how ragged, and what the band down to it is made of.
struct Turn {
    z: f32,
    radius: Option<f32>,
    ragged: f32,
    material: u32,
}

/// The ground from the slab's edge in to the bottom of the bore, one surface:
/// the slab's bevelled sides and deck, the spoil lip round the mouth, the pit's
/// faces and benches with ore seams in the faces and rubble on the benches, the
/// floor, and the bore down from it. Everything past the deck is `part::ASHORE`;
/// on water the deck rings a moon pool instead.
fn foundation(b: &mut MeshBuilder, tech: u8) {
    let slab = |z: f32| Turn { z, radius: None, ragged: 0.0, material: PLATING };
    let rock = |z: f32, r: f32, ragged: f32| Turn { z, radius: Some(r), ragged, material: ROCK };
    let mut turns = vec![
        slab(0.0),
        slab(1.3),
        Turn { z: DECK, radius: Some(-0.035), ..slab(DECK) },
        Turn { z: DECK, radius: Some(LIP), ragged: 0.015, material: PLATING },
        rock(LIP_CREST, (LIP + MOUTH) * 0.5, 0.03),
    ];
    for (i, &(z, radius)) in pit_profile(tech).iter().enumerate() {
        // The mouth meets the lip cleanly enough; the faces below are torn rock.
        turns.push(rock(z, radius, if i == 0 { 0.03 } else { 0.055 }));
    }
    // The bore, a little wider than the pipe, down out of sight.
    let floor_z = turns.last().expect("the pit has a floor").z;
    let bore = pipe_radius(tech) + 1.5;
    // In short lengths: the shader decides ring by ring whether the eye sees it
    // through the opening, so no face may reach far past where that changes.
    turns.extend([rock(floor_z, bore * 1.25, 0.08), rock(floor_z - 1.5, bore, 0.06)]);
    let mut z = floor_z - 1.5;
    let step = if b.fine() { 1.0 } else { 1.6 };
    while z > BORE_BOTTOM {
        z = (z - step * if z > -80.0 { 10.0 } else { 25.0 }).max(BORE_BOTTOM);
        turns.push(rock(z, bore * (0.9 + 0.1 * (z - BORE_BOTTOM) / (floor_z - BORE_BOTTOM)), 0.06));
    }
    let deck = 3;

    let sides = if b.fine() { 24 } else { 16 };
    let rings: Vec<Vec<Vec3>> = turns
        .iter()
        .enumerate()
        .map(|(k, turn)| {
            (0..sides)
                .map(|i| {
                    let angle = i as f32 * TAU / sides as f32;
                    let dir = Vec2::from_angle(angle);
                    let r = match turn.radius {
                        // Negative: inset from the slab's edge by that share (its bevel).
                        Some(r) if r < 0.0 => slab_edge(angle) * (1.0 + r),
                        Some(r) => r * (1.0 + turn.ragged * (hash_unit(k as u32 * 7 + 3, i as u32) * 2.0 - 1.0)),
                        None => slab_edge(angle),
                    };
                    (dir * r).extend(turn.z)
                })
                .collect()
        })
        .collect();
    for k in 1..=deck {
        b.paint(turns[k].material);
        band(b, &rings[k - 1], &rings[k]);
    }
    b.with_part(part::ASHORE, |b| {
        for k in deck + 1..rings.len() {
            b.paint(turns[k].material);
            band(b, &rings[k - 1], &rings[k]);
        }
        if b.fine() {
            // Seams in the faces and rubble on the benches, the pit's own turns only.
            for k in deck + 3..rings.len() - 3 {
                let (upper, lower) = (&rings[k - 1], &rings[k]);
                if upper[0].z > lower[0].z + 1.0 {
                    seams(b, upper, lower, k as u32);
                } else {
                    rubble(b, upper, lower, k as u32);
                }
            }
        }
        // The bottom, where the last of the light gives out.
        let bottom = rings.last().expect("the bore has a bottom");
        b.paint(GLOW_RED).pattern(pattern::NONE);
        b.face(bottom);
    });

    // On water: the moon pool's coaming, and the deck's underside.
    b.with_part(part::AFLOAT, |b| {
        let lifted = |ring: &Vec<Vec3>, dz: f32| -> Vec<Vec3> { ring.iter().map(|p| *p + Vec3::Z * dz).collect() };
        let pool_top = lifted(&rings[deck], LIFT);
        let pool_foot = lifted(&rings[deck], LIFT - DECK - 1.2);
        b.paint(PLATING_DARK);
        band(b, &pool_top, &pool_foot);
        let under_edge = lifted(&rings[0], LIFT);
        let under_pool = lifted(&rings[deck], LIFT - DECK);
        b.paint(PLATING_DARK).pattern(pattern::PILE);
        band(b, &under_pool, &under_edge);
        if b.fine() {
            b.paint(PLATING).pattern(pattern::HAZARD);
            let rim_in = lifted(&rings[deck], LIFT + 0.05);
            let rim_out: Vec<Vec3> = rim_in.iter().map(|p| (p.truncate() * 1.08).extend(p.z)).collect();
            band(b, &rim_out, &rim_in);
        }
    });

    b.radial(4, |b| team_panel(b, v3(38.0, 0.0, DECK), v2(1.8, 12.0)));
}

/// The offshore rig's legs: eight stilts from the seabed up under the deck,
/// bracing between them just over the water, and caps under the deck. Authored
/// as they stand, raised (`LIFT`).
fn stilts(b: &mut MeshBuilder) {
    let feet: Vec<Vec2> = (0..8)
        .map(|i| {
            let angle = i as f32 * TAU / 8.0;
            Vec2::from_angle(angle) * if i % 2 == 0 { 34.0 } else { 46.0 }
        })
        .collect();
    for (i, &at) in feet.iter().enumerate() {
        let corner = i % 2 == 1;
        let r = if corner { 3.0 } else { 2.2 };
                b.paint(PLATING_DARK).pattern(pattern::PILE);
        b.cylinder_between(at.extend(PILE_FOOT), at.extend(LIFT), r, r, b.sides(10));
        if b.fine() {
            b.paint(ACCENT);
            b.prism(at.extend(LIFT - 1.4), 8, r * 1.5, r * 1.2, 1.4);
        }
        let next = feet[(i + 1) % feet.len()];
        b.paint(PLATING_DARK).pattern(pattern::PILE);
        b.beam(at.extend(1.6), next.extend(1.6), v2(1.0, 1.3), v2(1.0, 1.3));
        if b.fine() {
            // Cross-bracing down into the water.
            b.paint(METAL);
            b.beam(at.extend(1.2), next.extend(-9.0), v2(0.6, 0.6), v2(0.6, 0.6));
            b.beam(next.extend(1.2), at.extend(-9.0), v2(0.6, 0.6), v2(0.6, 0.6));
        }
    }
    // The magazine the pipe comes up through, down to the water.
    let rack = rack();
    b.paint(PLATING_DARK);
    b.cylinder_between(rack.extend(LIFT - SECTION - 1.5), rack.extend(LIFT), 2.9, 2.9, b.sides(10));
}


/// Distance from the middle to the slab's chamfered edge along `angle`.
fn slab_edge(angle: f32) -> f32 {
    let (c, s) = (angle.cos().abs(), angle.sin().abs());
    (SLAB / c.max(1e-4)).min(SLAB / s.max(1e-4)).min((2.0 * SLAB - 11.0) / (c + s))
}

/// Quads between two rings of the same count, each ring running anticlockwise
/// seen from above, from the ring further along the ground (`outer`: the slab's
/// foot, the rim) to the next (`inner`): the slab's sides face out, a bench up,
/// a face in toward the middle.
fn band(b: &mut MeshBuilder, outer: &[Vec3], inner: &[Vec3]) {
    let n = outer.len();
    for i in 0..n {
        let j = (i + 1) % n;
        b.face(&[inner[i], outer[i], outer[j], inner[j]]);
    }
}

/// Ore seams in a face: glowing streaks running along the rock and dipping,
/// each over a few of its segments.
fn seams(b: &mut MeshBuilder, upper: &[Vec3], lower: &[Vec3], seed: u32) {
    let n = upper.len();
    b.paint(GLOW_ORANGE).pattern(pattern::NONE);
    for s in 0..2 {
        let start = (hash_unit(seed, 40 + s) * n as f32) as usize;
        let span = 3 + (hash_unit(seed, 50 + s) * 4.0) as usize;
        let (height, dip) = (0.25 + hash_unit(seed, 60 + s) * 0.5, (hash_unit(seed, 70 + s) - 0.5) * 0.3);
        let at = |i: usize, v: f32| {
            let (top, bottom) = (upper[i % n], lower[i % n]);
            let p = bottom.lerp(top, v.clamp(0.05, 0.95));
            // Just proud of the rock, toward the middle.
            p - p.truncate().normalize_or_zero().extend(0.0) * 0.12
        };
        for t in 0..span {
            let (v0, v1) = (height + dip * t as f32 / span as f32, height + dip * (t + 1) as f32 / span as f32);
            let w = 0.07 * (1.0 - 0.6 * (t as f32 / span as f32 - 0.5).abs());
            let (i, j) = (start + t, start + t + 1);
            b.face(&[at(i, v0 - w), at(i, v0 + w), at(j, v1 + w), at(j, v1 - w)]);
        }
    }
}

/// Broken rock lying on a bench.
fn rubble(b: &mut MeshBuilder, outer: &[Vec3], inner: &[Vec3], seed: u32) {
    let n = outer.len();
    b.paint(ROCK);
    for s in 0..3 {
        let i = (hash_unit(seed, 80 + s) * n as f32) as usize % n;
        let p = outer[i].lerp(inner[i], 0.3 + hash_unit(seed, 90 + s) * 0.4);
        let size = 0.6 + hash_unit(seed, 100 + s) * 1.1;
        b.yawed(p, hash_unit(seed, 110 + s) * TAU, |b| {
            b.frustum(Vec3::ZERO, v2(size * 1.4, size), v2(size * 0.7, size * 0.5), size * 0.8, v2(size * 0.2, 0.0));
        });
    }
}

/// An ore line on the +x axis: the conveyor that brings ore up out of the pit
/// and the bin it tips into, the ore heaped in its open top.
fn ore_line(b: &mut MeshBuilder) {
    b.at(v3(31.0, 0.0, 0.0), |b| {
        b.paint(PLATING);
        b.loft_z(
            &chamfered_rect(v2(4.8, 6.5), 1.4),
            &[Section::new(DECK, 0.8), Section::new(4.5, 1.0), Section::new(12.0, 1.0)],
        );
        if b.fine() {
            // The rim of the open top, and the ore heaped inside it.
            b.paint(PLATING_DARK).pattern(pattern::PLAIN);
            b.cuboid_open(v3(0.0, 0.0, 12.4), v3(9.4, 12.8, 0.8));
            b.paint(ROCK);
            b.frustum(v3(0.0, 0.0, 12.0), v2(8.6, 12.0), v2(4.0, 6.5), 1.6, Vec2::ZERO);
            b.paint(GLOW_ORANGE).pattern(pattern::NONE);
            b.decal(v3(0.0, 0.0, 13.62), v2(2.4, 4.0));
            // The chute it is emptied through, low on the outer side.
            b.paint(PLATING_DARK);
            b.beam(v3(4.2, 0.0, 6.0), v3(7.2, 0.0, 3.6), v2(2.4, 1.6), v2(2.0, 1.2));
        } else {
            b.paint(ROCK);
            b.decal(v3(0.0, 0.0, 12.02), v2(8.0, 11.0));
        }
    });

    // The belt reaches out over the pit from a trestle on the first bench and
    // climbs over the lip to the bin.
    let (foot, head) = (v3(11.0, 0.0, -1.0), v3(26.0, 0.0, 13.0));
    b.paint(PLATING_DARK);
    b.beam(foot, head, v2(3.4, 2.0), v2(3.0, 1.8));
    if b.fine() {
        // Ore on the belt, the drums at either end, and the trestle.
        let up = (head - foot).cross(Vec3::Y).normalize() * 1.02;
        b.paint(GLOW_ORANGE);
        b.beam(foot.lerp(head, 0.04) + up, head.lerp(foot, 0.03) + up, v2(1.9, 0.2), v2(1.7, 0.2));
        b.paint(METAL);
        for end in [foot, head] {
            b.cylinder_between(end - v3(0.0, 1.9, 0.0), end + v3(0.0, 1.9, 0.0), 1.2, 1.2, b.sides(8));
        }
        b.with_part(part::ASHORE, |b| {
            b.beam(v3(14.4, 0.0, -5.0), v3(14.4, 0.0, 1.3), v2(1.2, 1.2), v2(1.0, 1.0));
        });
        b.beam(v3(23.5, 0.0, DECK), v3(23.5, 0.0, 9.8), v2(1.2, 1.2), v2(1.0, 1.0));
    }
}

/// The headframe: four legs standing clear of the pit and leaning in over it,
/// tied and braced, the winch head on top that hauls the driver, the leader
/// rails the driver rides, the spider over the bore that holds the string, and
/// the magazine and trolley rail that bring in the next section.
fn derrick(b: &mut MeshBuilder) {
    // Leg centre at height z, along each diagonal.
    let leg = |z: f32| FOOT - (FOOT - 7.0) * (z - DECK) / (FRAME_TOP - DECK);
    b.radial(4, |b| {
        b.paint(PLATING);
        b.frustum(v3(FOOT, FOOT, DECK), v2(6.0, 6.0), v2(4.2, 4.2), 1.6, Vec2::ZERO);
        b.paint(PLATING_DARK);
        b.beam(v3(FOOT, FOOT, DECK + 1.2), v3(7.0, 7.0, FRAME_TOP), v2(3.0, 3.0), v2(2.2, 2.2));
        b.paint(ACCENT);
        for z in [15.5, 25.5] {
            let k = leg(z);
            b.beam(v3(k, k, z), v3(k, -k, z), v2(1.2, 1.5), v2(1.2, 1.5));
        }
        if b.fine() {
            let (a, c) = (leg(15.5), leg(25.5));
            b.paint(METAL);
            b.beam(v3(a, a, 15.5), v3(c, -c, 25.5), v2(0.7, 0.7), v2(0.7, 0.7));
            b.beam(v3(a, -a, 15.5), v3(c, c, 25.5), v2(0.7, 0.7), v2(0.7, 0.7));
        }
    });

    // The winch head: a plain house on the legs, the hoist drum inside it.
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(9.5, 9.5), 2.8),
        &[
            Section::new(HEAD_FOOT, 0.8),
            Section::new(30.5, 1.0),
            Section::new(36.5, 1.0),
            Section::new(HEAD, 0.82),
        ],
    );
    team_panel(b, v3(0.0, 0.0, HEAD), v2(9.0, 9.0));

    // Leader rails from the spider up to the head, either side of the bore.
    b.mirror_y(|b| {
        b.paint(METAL);
        b.cylinder_between(v3(0.0, RAIL, DECK), v3(0.0, RAIL, HEAD_FOOT + 0.3), 0.5, 0.5, b.sides(6));
        // The spider: a ring round the string, arms out over the pit to the lip.
        b.paint(PLATING_DARK);
        b.beam(v3(0.0, 2.6, DECK + 0.1), v3(0.0, LIP + 0.8, DECK + 0.1), v2(1.4, 1.0), v2(1.8, 1.0));
    });
    b.paint(PLATING_DARK);
    b.prism(v3(0.0, 0.0, DECK - 0.5), b.sides(10), 3.1, 2.9, 1.2);

    // The magazine the next section rises out of, and the trolley rail it swings
    // in along, clear over the top of it.
    let rack = rack();
    let along = rack.normalize();
    b.paint(PLATING);
    b.prism(rack.extend(DECK), b.sides(10), 3.0, 2.7, 1.4);
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.prism(rack.extend(DECK + 1.4), b.sides(10), 2.7, 2.6, 0.3);
    let rail_z = STRING_TOP + SECTION + 0.9;
    b.paint(PLATING_DARK);
    b.beam((along * 4.2).extend(rail_z), (rack + along * 2.0).extend(rail_z), v2(1.0, 0.9), v2(1.0, 0.9));
    b.beam((rack + along * 2.0).extend(rail_z), (rack + along * 2.0).extend(DECK + 1.4), v2(1.1, 1.1), v2(1.3, 1.3));
}

/// Middle of the pipe stack, out behind the magazine (+x here).
const STACK: f32 = 34.5;

/// Spare pipe lying on sleepers behind the magazine (+x here), a section long
/// each, the same pipe the rig is driving: a row of three.
fn pipe_stack(b: &mut MeshBuilder, tech: u8) {
    let r = pipe_radius(tech);
    let spacing = 2.0 * r + 0.1;
    b.paint(ACCENT);
    for y in [-3.0, 3.0] {
        b.cuboid(v3(STACK, 0.0 + y, DECK + 0.3), v3(3.0 * spacing + 0.6, 1.0, 0.6));
    }
    for i in [-1.0, 0.0, 1.0] {
        stacked_pipe(b, v3(STACK + i * spacing, 0.0, DECK + 0.6 + r), r);
    }
}

/// The stack grows a second row in the grooves of the first, held in by posts
/// at each end.
fn more_pipe(b: &mut MeshBuilder, tech: u8) {
    let r = pipe_radius(tech);
    let spacing = 2.0 * r + 0.1;
    let z = DECK + 0.6 + r + spacing * 3f32.sqrt() * 0.5;
    for i in [-0.5, 0.5] {
        stacked_pipe(b, v3(STACK + i * spacing, 0.0, z), r);
    }
    b.paint(PLATING_DARK);
    let (out, height) = (1.5 * spacing + 0.4, z + r - DECK);
    for y in [-3.0, 3.0] {
        for x in [STACK - out, STACK + out] {
            b.cuboid(v3(x, y, DECK + height * 0.5), v3(0.6, 0.8, height));
        }
    }
}

/// One pipe section lying along y, its collar at one end.
fn stacked_pipe(b: &mut MeshBuilder, at: Vec3, r: f32) {
    let half = v3(0.0, SECTION * 0.5, 0.0);
    b.paint(METAL);
    b.cylinder_between(at - half, at + half, r, r, b.sides(10));
    if b.fine() {
        b.paint(PLATING_DARK);
        b.cylinder_between(at + half - v3(0.0, 1.0, 0.0), at + half, r * 1.3, r * 1.3, b.sides(10));
    }
}

/// The pile driver (`part::RAM`), the pipe string it drives (`part::STRING`) and
/// the next section waiting at the rack (`part::FEED`), all authored as the
/// driver has just struck. The string runs from the spider far down the bore,
/// a collar at the head of every section and a glowing band round each middle,
/// so the eye can follow it down as it goes. The next section is one of those.
fn pipe(b: &mut MeshBuilder, tech: u8) {
    let r = pipe_radius(tech);
    let section = |b: &mut MeshBuilder, top: f32, whole: bool| {
        if whole {
            b.paint(METAL);
            b.cylinder_between(v3(0.0, 0.0, top - SECTION), v3(0.0, 0.0, top), r, r, b.sides(10));
        }
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, top - 1.0), b.sides(10), r * 1.3, r * 1.3, 1.0);
        if b.fine() {
            b.paint(GLOW_ORANGE).pattern(pattern::NONE);
            b.prism(v3(0.0, 0.0, top - SECTION * 0.5 - 0.2), b.sides(10), r * 1.02, r * 1.02, 0.4);
        }
    };
    b.with_part(part::STRING, |b| {
        // One long pipe down past the bottom of the bore, with a section's slack so
        // its end is never seen as it goes down, and collars where they can be seen.
        // In section lengths, for the shader's sake as the bore is.
        b.paint(METAL);
        let ring = ngon(b.sides(10), r);
        let rings: Vec<Vec<Vec3>> = (0..=19)
            .map(|k| ring.iter().map(|p| v3(p[0], p[1], STRING_TOP - k as f32 * SECTION)).collect())
            .collect();
        b.loft(&rings, false, false);
        let seen = if b.fine() { 5 } else { 2 };
        for k in 0..seen {
            section(b, STRING_TOP - k as f32 * SECTION, false);
        }
    });
    b.with_part(part::FEED, |b| {
        b.at(rack().extend(SECTION), |b| section(b, STRING_TOP, true));
    });
    b.with_part(part::RAM, |b| {
        // The driver: a heavy drum with shoes on the rails and a cap for the cables.
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, STRING_TOP), b.sides(12), r * 1.35, 2.9, 1.2);
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, STRING_TOP + 1.2), b.sides(12), 2.9, 2.9, DRIVER - 2.0);
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.prism(v3(0.0, 0.0, STRING_TOP + DRIVER - 0.8), b.sides(12), 2.95, 2.95, 0.5);
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, STRING_TOP + DRIVER - 0.3), b.sides(12), 2.9, 1.6, 0.9);
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cuboid(v3(0.0, RAIL - 0.2, STRING_TOP + DRIVER * 0.5), v3(1.6, 1.3, DRIVER - 0.6));
        });
        if b.fine() {
            // The hoist cables up into the winch head. Long enough that their ends
            // stay inside it with the driver hauled all the way up.
            b.paint(METAL);
            for x in [-1.2, 1.2] {
                b.cylinder_between(v3(x, 0.0, STRING_TOP + DRIVER), v3(x, 0.0, HEAD_FOOT + 0.4), 0.22, 0.22, 5);
            }
        }
    });
}

// ---- Tech 2: the mine works ----------------------------------------------------

/// The tower stage over the winch head that carries the sheave house: four legs
/// tied and braced like the headframe's.
fn tower(b: &mut MeshBuilder) {
    let leg = |z: f32| 7.2 - 2.4 * (z - HEAD) / (SHEAVE_FOOT + 1.0 - HEAD);
    b.radial(4, |b| {
        b.paint(PLATING_DARK);
        b.beam(v3(7.2, 7.2, HEAD - 0.5), v3(4.8, 4.8, SHEAVE_FOOT + 1.0), v2(2.2, 2.2), v2(1.7, 1.7));
        b.paint(ACCENT);
        for z in [46.0, 53.0] {
            let k = leg(z);
            b.beam(v3(k, k, z), v3(k, -k, z), v2(0.9, 1.1), v2(0.9, 1.1));
        }
        if b.fine() {
            let (a, c) = (leg(46.0), leg(53.0));
            b.paint(METAL);
            b.beam(v3(a, a, 46.0), v3(c, -c, 53.0), v2(0.5, 0.5), v2(0.5, 0.5));
            b.beam(v3(a, -a, 46.0), v3(c, c, 53.0), v2(0.5, 0.5), v2(0.5, 0.5));
        }
    });
    if b.fine() {
        // The hoist cables go on up through the head to the sheaves.
        b.paint(METAL);
        for x in [-1.2, 1.2] {
            b.cylinder_between(v3(x, 0.0, HEAD), v3(x, 0.0, SHEAVE_FOOT + 0.4), 0.22, 0.22, 5);
        }
    }
}

/// The sheave house on top of the tower: the pulleys the hoist cables turn
/// over, from the winch house on the deck down to the driver.
fn sheave_house(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(7.2, 7.2), 2.2),
        &[
            Section::new(SHEAVE_FOOT, 0.7),
            Section::new(57.5, 1.0),
            Section::new(63.0, 1.0),
            Section::new(SHEAVE, 0.8),
        ],
    );
    team_panel(b, v3(0.0, 0.0, SHEAVE), v2(8.0, 8.0));
}

/// The winch house on the deck (+x here): a shed over the hoist drum, and the
/// cables off the drum up to the sheave house.
fn winch_house(b: &mut MeshBuilder) {
    const AT: Vec3 = Vec3::new(30.0, 0.0, 0.0);
    b.paint(PLATING);
    b.at(AT, |b| {
        b.loft_z(
            &chamfered_rect(v2(4.0, 3.2), 0.9),
            &[Section::new(DECK, 1.0), Section::new(8.6, 1.0), Section::new(9.6, 0.8)],
        );
    });
    team_panel(b, AT + v3(0.0, 0.0, 9.6), v2(4.4, 3.0));
    // Where the cables leave the roof, and where they reach the sheave house's
    // side, clear of the tower's legs.
    let (from, to) = (AT + v3(-2.0, 0.0, 9.6), v3(6.6, 0.0, 60.0));
    b.paint(PLATING_DARK);
    b.prism(from - v3(0.0, 0.0, 0.2), 8, 1.8, 1.4, 0.8);
    b.paint(METAL);
    for y in [-0.7, 0.7] {
        b.cylinder_between(from + v3(0.0, y, 0.4), to + v3(0.0, y, 0.0), 0.22, 0.22, 5);
    }
}

// ---- Tech 3: the induction rig -------------------------------------------------

/// Plate cladding round the tower stage, over its ties and braces; the legs
/// stand proud at its chamfered corners.
fn cladding(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(7.8, 7.8), 2.8),
        &[Section::new(HEAD, 1.0), Section::new(SHEAVE_FOOT, 0.72)],
    );
}

/// Rings of induction coils round the leader rails (their bottoms `at` metres up
/// from `COILS_LOW`), which throw the driver down; hung from the head on four
/// rods and braced to the headframe's legs.
fn coils(b: &mut MeshBuilder, at: &[f32]) {
    for &dz in at {
        coil(b, COILS_LOW + dz);
    }
    if at[0] == 0.0 {
        b.radial(4, |b| {
            b.paint(METAL);
            let rod = Vec2::splat(COIL_IN + 0.6) * std::f32::consts::FRAC_1_SQRT_2;
            b.cylinder_between(rod.extend(COILS_LOW), rod.extend(HEAD_FOOT + 0.3), 0.35, 0.35, 6);
            let k = FOOT - (FOOT - 7.0) * (STRUT_Z - DECK) / (FRAME_TOP - DECK);
            b.paint(PLATING_DARK);
            b.beam(v3(k, k, STRUT_Z), rod.extend(STRUT_Z), v2(1.0, 1.2), v2(0.9, 1.0));
        });
    }
}

/// One coil: a dark ring with its winding lit round the outside.
fn coil(b: &mut MeshBuilder, z: f32) {
    let sides = if b.fine() { 16 } else { 8 };
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    ring(b, z, COIL_IN, COIL_OUT, 1.0, sides);
    if b.fine() {
        b.paint(GLOW).pattern(pattern::NONE);
        ring(b, z + 0.3, COIL_OUT - 0.1, COIL_OUT + 0.08, 0.4, sides);
    }
}

/// An open ring about the z axis from `z` up `height`, between two radii.
fn ring(b: &mut MeshBuilder, z: f32, inner: f32, outer: f32, height: f32, sides: usize) {
    let at = |r: f32, i: usize, z: f32| (Vec2::from_angle(i as f32 * TAU / sides as f32) * r).extend(z);
    let top = z + height;
    for i in 0..sides {
        let j = (i + 1) % sides;
        let (ob, obj, ot, otj) = (at(outer, i, z), at(outer, j, z), at(outer, i, top), at(outer, j, top));
        let (ib, ibj, it, itj) = (at(inner, i, z), at(inner, j, z), at(inner, i, top), at(inner, j, top));
        b.face(&[ob, obj, otj, ot]);
        b.face(&[it, ot, otj, itj]);
        b.face(&[ibj, ib, it, itj]);
        b.face(&[ibj, obj, ob, ib]);
    }
}

/// A capacitor bank on the deck (+x here) that charges the coils, and its
/// conduit: along the deck to the nearest headframe leg (an eighth of a turn
/// to the `side`), up beside it and in along the strut to the coils.
fn capacitors(b: &mut MeshBuilder, side: f32) {
    const AT: f32 = 30.0;
    b.paint(PLATING_DARK);
    b.cuboid(v3(AT, 0.0, DECK + 0.5), v3(5.0, 7.4, 1.0));
    for y in [-2.4, 0.0, 2.4] {
        let base = v3(AT, y, DECK + 1.0);
        b.paint(PLATING);
        b.prism(base, b.sides(10), 1.05, 1.05, 5.4);
        b.paint(PLATING_DARK);
        b.prism(base + v3(0.0, 0.0, 5.4), b.sides(10), 1.05, 0.65, 0.6);
        if b.fine() {
            b.paint(GLOW).pattern(pattern::NONE);
            b.prism(base + v3(0.0, 0.0, 3.6), b.sides(10), 1.09, 1.09, 0.35);
        }
    }
    let leg = Vec2::from_angle(side * TAU / 16.0);
    // Beside the leg, on the bank's side of it.
    let beside = leg.perp() * -side;
    let radius_at = |z: f32| {
        std::f32::consts::SQRT_2 * (FOOT - (FOOT - 7.0) * (z - DECK - 1.2) / (FRAME_TOP - DECK - 1.2))
    };
    let path = [
        v3(AT - 2.6, 0.0, DECK + 0.5),
        (leg * radius_at(DECK + 0.5) + beside * 3.4).extend(DECK + 0.5),
        (leg * radius_at(STRUT_Z + 0.9) + beside * 3.4).extend(STRUT_Z + 0.9),
        (leg * (COIL_OUT + 0.4) + beside * 0.9).extend(STRUT_Z + 0.9),
    ];
    b.paint(PLATING_DARK).pattern(pattern::CONDUIT);
    for w in path.windows(2) {
        b.beam(w[0], w[1], v2(0.8, 0.8), v2(0.8, 0.8));
    }
}

// ---- Tech 4: the deep core -------------------------------------------------------

/// Drive weights stacked on the pile driver: the deep core's blow is a heavier
/// one, on a longer stroke. They ride with the driver, between its rails.
fn weights(b: &mut MeshBuilder) {
    let base = STRING_TOP + DRIVER + 0.6;
    b.with_part(part::RAM, |b| {
        for (i, z) in [0.0, 1.5, 3.0].into_iter().enumerate() {
            b.paint(if i == 1 { ACCENT } else { PLATING_DARK });
            b.cuboid(v3(0.0, 0.0, base + z + 0.7), v3(5.4, 5.4, 1.4));
        }
        b.paint(PLATING);
        b.frustum(v3(0.0, 0.0, base + 4.5), v2(5.4, 5.4), v2(2.6, 2.6), 1.0, Vec2::ZERO);
    });
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use crate::models::{build_model_scaled, part, LOD_COUNT};

    fn triangles(model: &crate::models::Model, lod: usize) -> usize {
        model.lods[lod].indices.len() / 3
    }

    /// Every tier has its driver, its string and the next section, a bore far down,
    /// stilts for the sea, the pit recorded for the shader, and stays in its budget.
    #[test]
    fn every_tier_drives_pipe_down_a_deep_bore_and_keeps_its_budget() {
        for tech in 1..=4 {
            let height = [40.0, 66.0, 70.0, 70.0][tech as usize - 1];
            let model = build_model_scaled("core_mine", 40.5, height, tech).unwrap();
            let pit = model.pit.expect("the mine records its pit");
            assert!(pit.open > 0.0 && pit.radius > 15.0, "tech {tech}: {pit:?}");
            // The driver clears the next section when hauled up, so it can swing in under it.
            assert!(pit.stroke > pit.section && pit.afloat_lift > 0.0, "tech {tech}: {pit:?}");
            assert!(Vec3::new(pit.rack[0], pit.rack[1], 0.0).length() > pit.radius, "tech {tech}: rack in the pit");
            let full = &model.lods[0];
            let deepest = full.vertices.iter().filter(|v| v.part == part::ASHORE).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(deepest < -140.0, "tech {tech}: bore only down to {deepest}");
            // Underground, only the pit's own pieces (pulled up by the shader) and the stilts.
            for v in &full.vertices {
                let p = Vec3::from(v.pos);
                if p.z < -0.01 && v.part != part::AFLOAT {
                    assert!(p.truncate().length() < pit.radius, "tech {tech}: {p} underground outside the pit");
                }
            }
            for lod in 0..2 {
                for kind in [part::RAM, part::STRING, part::FEED, part::AFLOAT, part::ASHORE] {
                    assert!(
                        model.lods[lod].vertices.iter().any(|v| v.part == kind),
                        "tech {tech} lod {lod}: no part {kind}"
                    );
                }
            }
            let counts: Vec<usize> = (0..LOD_COUNT).map(|lod| triangles(&model, lod)).collect();
            // A mine draws either its stilts or its pit, never both: the budget is for the
            // bigger of the two.
            let of = |kind| full.indices.chunks(3).filter(|t| full.vertices[t[0] as usize].part == kind).count();
            let drawn = counts[0] - of(part::AFLOAT).min(of(part::ASHORE));
            assert!(drawn <= 9000, "tech {tech}: {drawn} drawn of {counts:?}");
            assert!(counts[1] as f32 <= counts[0] as f32 * 0.45 + 20.0, "tech {tech}: {counts:?}");
            assert!(counts[2] < 60, "tech {tech}: {counts:?}");
            for lod in &model.lods {
                for t in lod.indices.chunks(3) {
                    let v = [0, 1, 2].map(|i| lod.vertices[t[i] as usize]);
                    assert!(v[0].part == v[1].part && v[1].part == v[2].part, "tech {tech}: mixed parts");
                    let [a, b, c] = v.map(|v| Vec3::from(v.pos));
                    assert!((b - a).cross(c - a).length() > 2e-7, "tech {tech}: degenerate at {a}");
                }
            }
        }
    }
}

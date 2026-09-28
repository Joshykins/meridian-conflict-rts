//! The Harrow, the Naga's tech 3 battle scorpion: a heavy armoured scorpion on eight
//! plated legs, a pair of great claws forward and a long jointed tail curled high over
//! its back, ending in a Pinched-plasmeric Beam projector (docs/STYLE.md "The Naga
//! look" and "The Naga suite"). Its claws charge Gravitic Bombs between their fingers
//! and throw them to land around what it fights.
//!
//! Shape: a wedge of a head under an overhanging brow, a pinched neck collar, then a
//! broad abdomen of overlapping arched plates that climbs to a raised socket where the
//! tail plugs in. The plates are dark gunmetal (`PLATING_DARK` under `pattern::EMBER`),
//! the seams between them darker still and lit red (`ACCENT` under `EMBER`), and the
//! working machinery under them (rams, ribs, cables, the projector's pinch rings) dark
//! bronze (`METAL` under `EMBER`).
//!
//! Rig: the body is `HULL`. Each leg is two bones posed by `entity.wgsl` `crawl_leg`
//! (`MeshBuilder::set_crawl_legs`), pair by pair. The claws ride `rig::TAIL` on the hull
//! (`MeshBuilder::with_claw`): the arm swings about its shoulder, the inner finger opens
//! about its hinge (`claw_pose`). The tail is the turret: segment `i` turns about joint
//! `i` of `TAIL` (`tail_pose`), writhing, leaning in to strike and taking a share of the
//! aim; the projector rides the last joint. The tail never swivels whole: its top four
//! joints bend round to the target (`AIM_SHARE`), within the unit file's `aim_arc`, and
//! past that the body turns. The sim turns the muzzle about the one point that best
//! matches that chain (`turret_at`). The projector pitches about its joint (`ARM_GUN`).
//!
//! Authored at the old commander's size (10.4 x 19 m) and built 1.3 times bigger: every
//! number in `data/factions/naga/units/land.ron` is this file's times `SCALE` (tests).

mod body;
mod limbs;
mod tail;
#[cfg(test)]
mod tests;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::kit::*;

/// Authored size (the unit file's is `SCALE` times it).
pub(super) const RADIUS: f32 = 10.4;
pub(super) const HEIGHT: f32 = 19.0;
/// How much bigger the unit file builds it.
#[cfg(test)]
const SCALE: f32 = 1.3;

/// The projector's joint on the tail's tip, and its muzzle.
const BEAM_PIVOT: Vec3 = Vec3::new(-1.6, 0.0, 18.6);
const BEAM_TIP: Vec3 = Vec3::new(4.4, 0.0, 17.6);
/// Where the left claw lets its bomb go: between its fingers. The right is the mirror.
const BOMB_AT: Vec3 = Vec3::new(17.5, 3.3, 5.3);

/// The tail's spine, root on the socket to where the venom bulb rides, and the middle of
/// its curl (for which way is "outside").
const TAIL: [Vec3; 10] = [
    Vec3::new(-6.4, 0.0, 6.9),
    Vec3::new(-8.8, 0.0, 8.0),
    Vec3::new(-10.7, 0.0, 9.7),
    Vec3::new(-11.9, 0.0, 11.8),
    Vec3::new(-12.2, 0.0, 14.1),
    Vec3::new(-11.6, 0.0, 16.3),
    Vec3::new(-10.3, 0.0, 18.0),
    Vec3::new(-8.6, 0.0, 19.1),
    Vec3::new(-6.8, 0.0, 19.6),
    Vec3::new(-5.1, 0.0, 19.5),
];
const CURL: Vec3 = Vec3::new(-7.6, 0.0, 13.2);
/// How the aim is shared over the tail's top joints, lowest first (`entity.wgsl`
/// `tail_pose`: the fourth joint from the top takes 1/10, the top one 4/10).
#[cfg(test)]
const AIM_SHARE: [f32; 4] = [0.1, 0.2, 0.3, 0.4];
/// The single point the stinger swings about that best matches that chain over the aim
/// arc: the unit file's `turret_at`, and the model's turret pivot.
const AIM_PIVOT: f32 = -6.8;
/// Half widths of the tail's armour rings at each joint, root to bulb.
const TAIL_WIDTH: [f32; 10] = [1.7, 1.62, 1.54, 1.46, 1.38, 1.3, 1.22, 1.14, 1.06, 1.0];

/// Left legs, front to back: hip, knee, the foot's tip, and where in the cycle it lifts.
/// Two alternating fours (front and third left with second and fourth right), a little
/// out of step so it scuttles rather than marches.
const LEGS: [(Vec3, Vec3, Vec3, f32); 4] = [
    (
        Vec3::new(4.8, 3.0, 4.9),
        Vec3::new(7.6, 5.5, 8.0),
        Vec3::new(10.8, 7.8, 0.0),
        0.0,
    ),
    (
        Vec3::new(2.2, 3.4, 4.9),
        Vec3::new(3.5, 6.5, 8.5),
        Vec3::new(5.1, 10.0, 0.0),
        0.5,
    ),
    (
        Vec3::new(-0.5, 3.4, 5.0),
        Vec3::new(-1.0, 6.6, 8.5),
        Vec3::new(-1.8, 10.2, 0.0),
        0.08,
    ),
    (
        Vec3::new(-3.2, 3.2, 5.1),
        Vec3::new(-4.8, 6.1, 8.2),
        Vec3::new(-7.9, 8.8, 0.0),
        0.58,
    ),
];

/// The left claw: shoulder, elbow, wrist, the palm's end, and the moving finger's hinge.
const SHOULDER: Vec3 = Vec3::new(9.0, 2.3, 4.9);
const ELBOW: Vec3 = Vec3::new(11.3, 4.9, 6.3);
const WRIST: Vec3 = Vec3::new(13.3, 4.7, 5.8);
const PALM: Vec3 = Vec3::new(16.2, 4.1, 5.4);
const JAW_HINGE: Vec3 = Vec3::new(16.0, 3.2, 5.3);

pub(super) fn scorpion(b: &mut MeshBuilder, _tech: u8) {
    b.set_crawl_legs(&LEGS, 8.0, 0.62, 1.6);
    b.set_tail(&TAIL, TAIL[TAIL.len() - 1].z - 0.4);
    b.set_claw(SHOULDER, JAW_HINGE);
    // The claws throw the unit file's weapons 1 (left) and 2 (right): the Gravitic Bombs.
    b.set_claw_throws(1, 2);
    // Where the sim turns the projector's muzzle (`turret_at` in the unit file). The
    // shader bends the tail's top joints instead of swivelling about it.
    b.set_turret_pivot(v3(AIM_PIVOT, 0.0, BEAM_PIVOT.z));
    b.set_arm_pivot(BEAM_PIVOT);
    b.set_dust_line(4.2);

    if b.coarse() {
        coarse(b);
        return;
    }
    body::body(b);
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in LEGS.iter().enumerate() {
            b.with_pair(i, |b| limbs::leg(b, hip, knee, foot, i));
        }
        limbs::claw(b);
    });
    tail::tail(b);
    tail::projector(b);
}

/// Far off: a slab of a body, flat legs that do not walk, the tail in two bars.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.frustum(
        v3(1.0, 0.0, 3.6),
        Vec2::new(16.0, 7.0),
        Vec2::new(12.0, 4.4),
        3.6,
        Vec2::new(0.6, 0.0),
    );
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            for &(hip, knee, foot, _) in &LEGS {
                b.face(&[hip, foot, knee]);
                b.face(&[hip, knee, foot]);
            }
        });
    });
    b.mirror_y(|b| {
        b.with_claw(false, |b| {
            dark_plate(b);
            b.face(&[SHOULDER, PALM + v3(2.2, -0.8, 0.0), ELBOW + Vec3::Z]);
            b.face(&[SHOULDER, ELBOW + Vec3::Z, PALM + v3(2.2, -0.8, 0.0)]);
        });
    });
    // The tail in two three-sided bars, keel out.
    let bar = |b: &mut MeshBuilder, a: Vec3, c: Vec3, wa: f32, wc: f32| {
        let (side, up) = frame(c - a, (a + c) * 0.5 - CURL);
        let tri = |p: Vec3, w: f32| {
            vec![
                p + up * w,
                p + side * w - up * (w * 0.6),
                p - side * w - up * (w * 0.6),
            ]
        };
        b.loft(&[tri(a, wa), tri(c, wc)], true, true);
    };
    b.with_tail(0, |b| {
        dark_plate(b);
        bar(b, TAIL[0], TAIL[4], 1.8, 1.5);
    });
    b.with_tail(4, |b| {
        dark_plate(b);
        bar(b, TAIL[4], TAIL[9], 1.5, 1.1);
    });
    // The projector as a spike off the tail's tip, its muzzle lit.
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        let root = [
            v3(-5.4, 1.4, 19.2),
            v3(-5.4, -1.4, 19.2),
            v3(-5.4, 0.0, 21.0),
        ];
        b.loft(&[root.to_vec(), vec![BEAM_TIP; 3]], true, false);
    });
}

//! The Lens's legs: four great shards standing on their boots round it and leaning in to
//! meet over its middle. Each is a shin; it meets a plated thigh at a knee high over the
//! body and runs on past the knee into its point, so the four points close over the core.
//! Posed by entity.wgsl `crawl_leg` (`set_crawl_legs`), and braced by the deploy
//! (`set_crawl_brace`): packed, the feet stand in and the body rides high; planting, each
//! leg steps out in turn and the body settles to the ground between them.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::{part, rig};

use super::super::commander::form::{ring, sleeve, OCT};
use super::super::kit::{dark_plate, metal};
use super::super::shard::Shard;

/// The left legs, front and back: hip, knee, the foot's middle on the ground, and where in
/// the cycle it lands. A heavy walk, one foot at a time and three always down: back left,
/// front left, back right, front right, a quarter of a cycle apart (the right legs land
/// half a cycle after their left).
pub(super) const CRAWL: [(Vec3, Vec3, Vec3, f32); 2] = [
    (
        Vec3::new(2.0, 2.0, 2.0),
        Vec3::new(1.28, 0.62, 6.6),
        Vec3::new(4.6, 7.0, 0.0),
        0.125,
    ),
    (
        Vec3::new(-2.0, 2.0, 2.0),
        Vec3::new(-1.28, 0.62, 6.6),
        Vec3::new(-4.6, 7.0, 0.0),
        -0.125,
    ),
];
/// The walk: how far the body goes in one cycle, the share of it each foot stands, and
/// how high a shard is lifted to step: high enough that each one is seen to step.
pub(super) const STRIDE: f32 = 7.0;
pub(super) const STANCE: f32 = 0.75;
pub(super) const LIFT: f32 = 1.5;
/// The body's heft (`set_walk_sway`): its roll over the side whose feet are down, its dip
/// toward each foot as it lands, and how far it settles onto it.
const SWAY: (f32, f32, f32) = (0.035, 0.03, 0.22);
/// The shards' size against the Fallen's.
const SHARD_SCALE: f32 = 0.075;
/// How far past its knee a shin runs on into its point.
const POINT: f32 = 1.0;

/// A shin: the shard from its foot up past its knee to its point.
fn shin(knee: Vec3, foot: Vec3, seed: u32) -> Shard {
    let up = (knee - foot).normalize();
    Shard {
        root: foot,
        tip: knee + up * POINT,
        back: (foot - knee).with_z(0.0).normalize(),
        half: 1.15,
        depth: 0.62,
        skew: if seed.is_multiple_of(2) { 0.5 } else { -0.5 },
        plates: 3,
        seed,
        scale: SHARD_SCALE,
    }
}

/// Packed, how much higher the body rides, and what share nearer its hips the feet stand.
pub(super) const SINK: f32 = 1.2;
pub(super) const WIDEN: f32 = 0.05;

/// The rig and every leg, at the builder's level of detail.
pub(super) fn legs(b: &mut MeshBuilder) {
    b.set_crawl_legs(&CRAWL, STRIDE, STANCE, LIFT);
    b.set_walk_sway(SWAY.0, SWAY.1, SWAY.2);
    b.set_crawl_brace(SINK, WIDEN);
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in CRAWL.iter().enumerate() {
            b.with_pair(i, |b| {
                if b.coarse() {
                    b.with_part(part::LOCOMOTION, |b| {
                        b.with_limb(rig::SHIN, |b| shin(knee, foot, i as u32).build(b))
                    });
                } else {
                    leg(b, hip, knee, foot, i as u32);
                }
            });
        }
    });
}

/// One leg: a plated thigh from the hip drum up to the knee, and the shard below and past
/// it, which is the shin.
fn leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, foot: Vec3, seed: u32) {
    let out = (foot - hip).with_z(0.0).normalize();
    let axis = out.cross(Vec3::Z);
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            metal(b);
            let sides = b.sides(8);
            b.cylinder_between(hip - out * 0.5, hip + out * 0.3, 0.5, 0.45, sides);
            if b.fine() {
                b.cylinder_between(hip, knee, 0.2, 0.17, 6);
            }
            let thigh = knee - hip;
            let n = (out - thigh.normalize() * out.dot(thigh.normalize())).normalize();
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(hip + thigh * 0.08 + n * 0.12, n, 0.42, 0.3),
                    ring(hip + thigh * 0.55 + n * 0.18, n, 0.46, 0.34),
                    ring(knee - thigh * 0.1 + n * 0.1, n, 0.3, 0.24),
                ],
                &OCT,
            );
        });
        b.with_limb(rig::SHIN, |b| {
            metal(b);
            let sides = b.sides(8);
            b.cylinder_between(knee - axis * 0.4, knee + axis * 0.4, 0.38, 0.38, sides);
            shin(knee, foot, seed).build(b);
        });
    });
}

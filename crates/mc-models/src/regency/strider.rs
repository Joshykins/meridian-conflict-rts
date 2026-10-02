//! The Strider, the Regency's tech 3 assault tripod: a fighting machine on three long legs,
//! a tall keeled head carried high over the field, a Pinched-plasmeric Cannon slung under
//! a plated outrigger either side of it. Tall and lean: the head stands 25 m up on legs as thick as a
//! tank's hull is high, so it wades out to sea and still fires over the water.
//!
//! Under the head the legs meet at a faceted crown, bronze hip drums round it and plates
//! hanging between the legs to a point. Each leg is a plated thigh out to a bronze knee
//! under a guard swept up into a spike, then a long keeled shin down to an armoured hoof,
//! a bronze tendon showing behind the shin's plate. The head is a narrow prow, flat-faceted
//! and swept back to a point, a keel fin rising off its back, three red optics low on its
//! prow. Each cannon gathers its charge between two flat prongs past its bore, lit red
//! inside; the unit file's `muzzle` is the middle of that charge.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, seam-dark joints, dark bronze on
//! the machinery, red optics. No violet: it does not build.
//!
//! Rig: three legs posed by `entity.wgsl` `crawl_leg` (`MeshBuilder::set_crawl_legs`): the
//! front two a mirrored pair, the rear one a lone leg on the centreline (`set_lone_leg`),
//! so they step one at a time, front left, rear, front right, then all three stand a beat.
//! The head is the turret and turns about the unit's middle; the cannons pitch about their
//! common trunnion (`rig::ARM_GUN`). The numbers match `regency_t3_strider` in
//! `data/factions/regency/units/land_t3.ron`.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::commander::form::{ball, blade, ring, sleeve, KEEL, OCT};
use super::kit::{dark_plate, metal, seam, v3};
use super::machine::{collar, red_slot, shaft};

/// The cannons' trunnion and the middles of their charges: the unit file's weapon `pivot`
/// and `muzzle`s, left then right.
const PIVOT: Vec3 = Vec3::new(1.2, 0.0, 24.4);
const MUZZLES: [Vec3; 2] = [Vec3::new(9.0, 3.8, 24.4), Vec3::new(9.0, -3.8, 24.4)];
/// How far round each charge its prongs stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 2.0;
/// Where the head turns on the crown.
const RACE: f32 = 24.0;
/// The hips' height and how far out from the middle they sit.
const HIP_Z: f32 = 20.0;
const HIP_R: f32 = 2.2;
/// How far out the feet stand.
const FOOT_R: f32 = 10.5;
/// The ground one walking cycle covers, the share of it a foot stands, and how high a foot
/// lifts: three steps and a beat on every foot standing (`crawl_leg`).
const STRIDE: f32 = 16.0;
const STANCE: f32 = 0.75;
const LIFT: f32 = 3.0;

/// A leg at rest from its hip (at `hip_az` round the crown) to its foot's tip (at `foot_az`),
/// the knee out and a little up from the hip, under the head's sweep, and the cycle's share
/// at which it lifts.
fn leg_joints(hip_az: f32, foot_az: f32, phase: f32) -> (Vec3, Vec3, Vec3, f32) {
    let at = |az: f32, r: f32, z: f32| {
        let a = az.to_radians();
        v3(a.cos() * r, a.sin() * r, z)
    };
    let hip = at(hip_az, HIP_R, HIP_Z);
    let foot = at(foot_az, FOOT_R, 0.0);
    let out = (foot - hip).with_z(0.0).normalize();
    let knee = hip + out * 5.3 + Vec3::Z * 2.4;
    let snap = |p: Vec3| if p.y.abs() < 1e-4 { p.with_y(0.0) } else { p };
    (snap(hip), snap(knee), snap(foot), phase)
}

/// The front left leg (the front right is its mirror) and the lone rear leg.
fn legs() -> [(Vec3, Vec3, Vec3, f32); 2] {
    [leg_joints(50.0, 60.0, 0.0), leg_joints(180.0, 180.0, 0.25)]
}

pub(super) fn strider(b: &mut MeshBuilder, _tech: u8) {
    let legs = legs();
    b.set_crawl_legs(&legs, STRIDE, STANCE, LIFT);
    b.set_lone_leg(1);
    b.set_turret_pivot(v3(0.0, 0.0, RACE));
    b.set_arm_pivot(PIVOT);
    b.set_recoil(PIVOT, MUZZLES[0].with_y(0.0), 0.4);
    b.set_dust_line(1.5);
    if b.coarse() {
        coarse(b, &legs);
        return;
    }
    crown(b, &legs);
    let (front, rear) = (legs[0], legs[1]);
    b.mirror_y(|b| b.with_pair(0, |b| leg(b, front)));
    b.with_pair(1, |b| leg(b, rear));
    b.with_part(part::TURRET, |b| {
        prow(b);
        b.with_limb(rig::ARM_GUN, |b| b.mirror_y(cannon));
    });
}

/// Far off: the crown a spindle, flat legs that do not walk, the head a wedge with the team
/// colour on it, the cannons bars that still pitch.
fn coarse(b: &mut MeshBuilder, legs: &[(Vec3, Vec3, Vec3, f32); 2]) {
    dark_plate(b);
    b.loft_z(
        &[[2.4, 0.0], [-1.2, 2.1], [-1.2, -2.1]],
        &[Section::new(17.0, 0.3), Section::new(RACE, 1.0)],
    );
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING_DARK);
        let flat = |b: &mut MeshBuilder, (hip, knee, foot, _): (Vec3, Vec3, Vec3, f32)| {
            let side = (foot - hip).with_z(0.0).normalize().cross(Vec3::Z) * 0.6;
            b.face(&[hip - side, foot, knee]);
            b.face(&[hip + side, knee, foot]);
            b.face(&[hip + side, foot, knee]);
            b.face(&[hip - side, knee, foot]);
        };
        let front = legs[0];
        b.mirror_y(|b| flat(b, front));
        flat(b, legs[1]);
    });
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        let (plan, top) = ([[7.4, 0.0], [-6.8, 2.2], [-6.8, -2.2]], 30.6);
        b.loft_z(
            &plan,
            &[
                Section::new(25.3, 1.0),
                Section::new(top, 0.5).shifted(-1.0, 0.0),
            ],
        );
        b.paint(TEAM);
        b.face(&[
            v3(1.0, 0.0, top + 0.02),
            v3(-2.6, 1.2, top + 0.02),
            v3(-2.6, -1.2, top + 0.02),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            for m in MUZZLES {
                b.beam(
                    m.with_x(PIVOT.x),
                    m - Vec3::X * 1.0,
                    Vec2::splat(1.4),
                    Vec2::splat(0.9),
                );
            }
        });
    });
}

/// The crown the legs meet at: a faceted spindle from a point under the hips up to the
/// head's race, a bronze drum at each hip, a plate hanging between each two legs down to a
/// point, the race on top.
fn crown(b: &mut MeshBuilder, legs: &[(Vec3, Vec3, Vec3, f32); 2]) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(0.0, 0.0, 15.6), Vec3::X, 0.25, 0.25),
            ring(v3(0.0, 0.0, 17.6), Vec3::X, 1.5, 1.5),
            ring(v3(0.0, 0.0, 20.6), Vec3::X, 2.5, 2.5),
            ring(v3(0.0, 0.0, 23.2), Vec3::X, 2.6, 2.6),
            ring(v3(0.0, 0.0, RACE), Vec3::X, 2.1, 2.1),
        ],
        &OCT,
    );
    // The hip drums, each across its leg's plane.
    let drum = |b: &mut MeshBuilder, (hip, _, foot, _): (Vec3, Vec3, Vec3, f32)| {
        let out = (foot - hip).with_z(0.0).normalize();
        let across = out.cross(Vec3::Z);
        collar(b, hip, across, 1.0, 2.0);
        if b.fine() {
            seam(b);
            let sides = b.sides(10);
            b.cylinder_between(hip + across * 1.0, hip + across * 1.15, 0.6, 0.45, sides);
            b.cylinder_between(hip - across * 1.0, hip - across * 1.15, 0.6, 0.45, sides);
        }
    };
    let front = legs[0];
    b.mirror_y(|b| drum(b, front));
    drum(b, legs[1]);
    // Plates hanging between the legs: one between the front two, one either side between
    // a front leg and the rear, each swept down to a point.
    dark_plate(b);
    for az in [0.0f32, 115.0, -115.0] {
        let a = az.to_radians();
        let out = v3(a.cos(), a.sin(), 0.0);
        blade(
            b,
            out * 2.75 + Vec3::Z * 23.4,
            (out * 0.28 - Vec3::Z).normalize(),
            out,
            6.4,
            1.25,
            0.0,
            0.34,
        );
    }
    // The race the head turns on.
    metal(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, RACE - 0.1), sides, 2.4, 2.35, 0.35);
}

/// One leg from `hip` through `knee` to the foot's tip at `foot`: a plated thigh, the
/// bronze knee under its guard, a long keeled shin with a bronze tendon behind it, and
/// an armoured hoof. Its bones ride `rig::THIGH` and `rig::SHIN`.
fn leg(b: &mut MeshBuilder, (hip, knee, foot, _): (Vec3, Vec3, Vec3, f32)) {
    let out = (foot - hip).with_z(0.0).normalize();
    let across = out.cross(Vec3::Z);
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            let up = Vec3::Z - (knee - hip).normalize() * (knee - hip).normalize().z;
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(hip.lerp(knee, 0.08), up, 0.8, 0.8),
                    ring(hip.lerp(knee, 0.45), up, 0.95, 1.05),
                    ring(hip.lerp(knee, 0.92), up, 0.72, 0.78),
                ],
                &KEEL,
            );
            // A plate along its top, its spike back over the hip.
            let along = (hip - knee).normalize();
            blade(
                b,
                knee.lerp(hip, 0.15) + up.normalize() * 0.95,
                along,
                up,
                (knee - hip).length() * 0.85,
                0.75,
                0.0,
                0.28,
            );
        });
        b.with_limb(rig::SHIN, |b| {
            metal(b);
            ball(b, knee, 0.95);
            // The knee guard: a plate swept up and out past the knee into a spike.
            dark_plate(b);
            let rise = (out + Vec3::Z * 0.35).normalize();
            blade(
                b,
                knee - rise * 1.2 + out * 0.5,
                rise,
                out,
                3.2,
                0.85,
                0.0,
                0.32,
            );
            let ankle = foot + Vec3::Z * 2.6;
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(knee, out, 0.85, 0.85),
                    ring(knee.lerp(ankle, 0.25), out, 1.0, 1.05),
                    ring(knee.lerp(ankle, 0.7), out, 0.66, 0.7),
                    ring(ankle, out, 0.48, 0.5),
                ],
                &KEEL,
            );
            // A plate down the shin's outside, lapped over the keel, its spike up at the knee.
            blade(
                b,
                knee.lerp(ankle, 0.55) + out * 0.85,
                (knee - ankle).normalize(),
                out,
                (ankle - knee).length() * 0.5,
                0.7,
                0.0,
                0.26,
            );
            if b.fine() {
                // The tendon behind the shin's plate: bare bronze working in the gap.
                let back = -out * 0.95;
                shaft(
                    b,
                    knee.lerp(ankle, 0.08) + back,
                    knee.lerp(ankle, 0.62) + back,
                    0.2,
                );
                for t in [0.08f32, 0.62] {
                    collar(b, knee.lerp(ankle, t) + back, ankle - knee, 0.3, 0.35);
                }
            }
            // The hoof: a bronze ankle collar, then an armoured pad down to the ground.
            collar(b, ankle, Vec3::Z, 0.55, 0.4);
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(ankle - Vec3::Z * 0.2, across, 0.55, 0.6),
                    ring(foot + Vec3::Z * 0.9, across, 0.95, 1.1),
                    ring(foot + Vec3::Z * 0.15, across, 0.85, 1.0),
                    ring(foot, across, 0.6, 0.75),
                ],
                &OCT,
            );
        });
    });
}

/// The head's turntable and neck: a bronze ring on the race and a plated block over it,
/// with the cannons' trunnion through it.
fn neck(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, RACE + 0.2), sides, 2.25, 2.2, 0.25);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [2.4, 0.0],
                [1.7, 1.7],
                [-1.7, 1.9],
                [-2.6, 0.0],
                [-1.7, -1.9],
                [1.7, -1.7],
            ],
            &[Section::new(RACE + 0.4, 1.0), Section::new(25.5, 0.92)],
        );
    });
}

/// The prow head: a tall narrow keeled head swept back to a point, a plated outrigger
/// either side over its gun, three red optics low on its prow, the team colour on its back.
fn prow(b: &mut MeshBuilder) {
    neck(b);
    let plan = [
        [7.4, 0.0],
        [4.6, 1.5],
        [0.0, 2.3],
        [-4.0, 2.0],
        [-6.8, 0.8],
        [-7.4, 0.0],
        [-6.8, -0.8],
        [-4.0, -2.0],
        [0.0, -2.3],
        [4.6, -1.5],
    ];
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(25.3, 0.86),
                Section::new(26.8, 1.0),
                Section::new(29.0, 0.78),
                Section::new(30.8, 0.36).shifted(-1.0, 0.0),
            ],
        );
    });
    // The outriggers: a thick plate either side over the gun, swept back into a spike.
    b.mirror_y(|b| {
        dark_plate(b);
        blade(
            b,
            v3(3.0, 1.8, 25.9),
            v3(-1.0, 0.55, -0.05),
            v3(0.0, 0.15, 1.0),
            6.6,
            1.0,
            0.3,
            0.4,
        );
    });
    // The keel fin rising off its back.
    blade(
        b,
        v3(1.0, 0.0, 30.6),
        v3(-1.0, 0.0, 0.3),
        Vec3::Y,
        5.4,
        0.7,
        0.8,
        0.3,
    );
    eyes(b, 6.2, 26.6, 0.7);
    b.paint(TEAM);
    b.face(&[
        v3(-0.6, 0.0, 30.82),
        v3(-2.8, 0.7, 30.82),
        v3(-2.8, -0.7, 30.82),
    ]);
}

/// Three red optics under the brow at `x`, `z`: one in the middle, one either side `wide`
/// out, each on the face it sits on.
fn eyes(b: &mut MeshBuilder, x: f32, z: f32, wide: f32) {
    red_slot(b, v3(x + 0.45, 0.0, z + 0.3), Vec3::X, Vec3::Y, 0.55, 0.18);
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(x - 0.25, wide, z),
            v3(1.0, 0.55, 0.0),
            v3(0.55, -1.0, 0.0),
            0.6,
            0.16,
        );
    });
}

/// The left cannon (the right is its mirror), in the head's frame: it pitches about
/// `PIVOT`. A bronze trunnion out from the neck, a plated breech, a keeled barrel banded in
/// bronze, and two flat prongs either side past its mouth, lit red inside, holding the
/// charge between them.
fn cannon(b: &mut MeshBuilder) {
    let m = MUZZLES[0];
    let (y, z) = (m.y, m.z);
    shaft(b, v3(PIVOT.x, 1.6, z), v3(PIVOT.x, y - 0.6, z), 0.36);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(PIVOT.x - 1.1, y, z), Vec3::Z, 0.55, 0.5),
            ring(v3(PIVOT.x - 0.6, y, z), Vec3::Z, 0.8, 0.7),
            ring(v3(PIVOT.x + 1.6, y, z), Vec3::Z, 0.8, 0.68),
            ring(v3(PIVOT.x + 2.2, y, z), Vec3::Z, 0.52, 0.48),
        ],
        &OCT,
    );
    sleeve(
        b,
        &[
            ring(v3(PIVOT.x + 2.1, y, z), Vec3::Z, 0.48, 0.45),
            ring(v3(7.2, y, z), Vec3::Z, 0.42, 0.4),
        ],
        &KEEL,
    );
    if b.fine() {
        for x in [4.4f32, 5.9] {
            collar(b, v3(x, y, z), Vec3::X, 0.5, 0.3);
        }
    }
    // The prongs, either side of the charge, red on their inner faces.
    for side in [1.0f32, -1.0] {
        let py = y + side * 1.15;
        let ring = |dy: f32| {
            vec![
                v3(6.2, py + dy, z - 0.32),
                v3(9.3, py + dy, z - 0.18),
                v3(9.3, py + dy, z + 0.18),
                v3(6.2, py + dy, z + 0.32),
            ]
        };
        dark_plate(b);
        b.loft(&[ring(0.0), ring(side * 0.26)], true, true);
        // The root tying the prong to the barrel.
        b.beam(
            v3(6.4, y, z),
            v3(6.4, py + side * 0.13, z),
            Vec2::new(0.6, 0.5),
            Vec2::new(0.6, 0.5),
        );
        b.paint(GLOW_LASER);
        let inner = py - side * 0.02;
        let face = [
            v3(6.9, inner, z - 0.16),
            v3(9.0, inner, z - 0.1),
            v3(9.0, inner, z + 0.1),
            v3(6.9, inner, z + 0.16),
        ];
        if side > 0.0 {
            b.face(&face);
        } else {
            b.face(&[face[3], face[2], face[1], face[0]]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model, Model};

    fn model() -> Model {
        build_model("regency_strider").unwrap()
    }

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check_charge(
            "regency_strider",
            12.0,
            32.0,
            None,
            &MUZZLES.map(|m| m.to_array()),
            HOLD,
        );
    }

    #[test]
    fn the_unit_files_cannons_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t3_strider").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, "regency_strider");
        assert_eq!(bp.weapons.len(), 2);
        for (w, m) in bp.weapons.iter().zip(MUZZLES) {
            assert!(v(w.muzzle).distance(m) < 1e-3, "{}", w.name);
            assert!(v(w.pivot.unwrap()).distance(PIVOT) < 1e-3);
        }
        assert!(bp.turret_at.is_none(), "the head turns about the middle");
        let model = model();
        assert_eq!(Vec3::from(model.turret_pivot), v3(0.0, 0.0, RACE));
        assert_eq!(Vec3::from(model.arm_pivot.unwrap()), PIVOT);
        for lod in &model.lods {
            assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
        }
    }

    /// Three legs: a front pair, and a lone rear leg on the centreline that walks as one.
    /// Each reaches the ground where its rest pose stands it, and the head's sweep never
    /// meets a knee.
    #[test]
    fn stands_on_three_legs() {
        {
            let key = "regency_strider";
            let model = build_model(key).unwrap();
            let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
            assert_eq!(crawl.pairs, 2);
            assert_eq!(crawl.lone, Some(1));
            assert_eq!(crawl.gpu()[5][3], 1.0, "{key}: the rear leg is marked lone");
            for lod in &model.lods[..2] {
                for pair in 0..2u32 {
                    let bones = |limb: u32| {
                        lod.vertices.iter().filter(move |v| {
                            v.part == part::LOCOMOTION
                                && v.rig & rig::LIMB_MASK == limb
                                && (v.rig & rig::PAIR_MASK) >> rig::PAIR_SHIFT == pair
                        })
                    };
                    for limb in [rig::THIGH, rig::SHIN] {
                        assert!(bones(limb).count() > 0, "{key}: pair {pair} bone {limb}");
                    }
                    let [_, _, foot] = crawl.joints[pair as usize];
                    let foot = Vec2::new(foot[0], foot[1]);
                    let low = bones(rig::SHIN).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
                    assert!(low < 0.08, "{key}: pair {pair} foot at {low}");
                    let reach = bones(rig::SHIN)
                        .map(|v| Vec2::new(v.pos[0], v.pos[1]).dot(foot))
                        .fold(f32::MIN, f32::max)
                        / foot.length();
                    assert!(reach >= FOOT_R - 0.1, "{key}: pair {pair} reaches {reach}");
                    if pair == 1 {
                        // A lone leg is its own mirror.
                        let ys: Vec<f32> = bones(rig::SHIN).map(|v| v.pos[1]).collect();
                        let (lo, hi) = ys
                            .iter()
                            .fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y)));
                        assert!((lo + hi).abs() < 1e-3, "{key}: lone leg off the centreline");
                    }
                }
                // The head sweeps round over the knees: everything that turns stays above
                // every leg vertex within its reach.
                let turret_low = lod
                    .vertices
                    .iter()
                    .filter(|v| v.part == part::TURRET)
                    .map(|v| v.pos[2])
                    .fold(f32::MAX, f32::min);
                let reach = lod
                    .vertices
                    .iter()
                    .filter(|v| v.part == part::TURRET)
                    .map(|v| Vec2::new(v.pos[0], v.pos[1]).length())
                    .fold(0.0f32, f32::max);
                let leg_high = lod
                    .vertices
                    .iter()
                    .filter(|v| {
                        v.part == part::LOCOMOTION
                            && Vec2::new(v.pos[0], v.pos[1]).length() > HIP_R + 1.3
                            && Vec2::new(v.pos[0], v.pos[1]).length() < reach
                    })
                    .map(|v| v.pos[2])
                    .fold(f32::MIN, f32::max);
                assert!(
                    leg_high < turret_low - 0.3,
                    "{key}: a leg at {leg_high} m under the head's sweep at {turret_low} m"
                );
            }
        }
    }
}

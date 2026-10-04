//! The Strider, the Regency's tech 4 assault tripod: a fighting machine on three long
//! legs, a tall keeled head carried high over the field, a Pinch-fusion Cannon slung
//! under a plated outrigger either side of it and a battery of Gravitic Seekers in a
//! launcher on the back of its head. Tall and lean: the head stands 75 m up on legs as
//! thick as a tank is long, so it wades out to sea and still fires over the water.
//!
//! Under the head the legs meet at a faceted crown, drums at the hips round it and plates
//! hanging between the legs to a point. Each leg is a plated thigh out to a knee under a
//! guard swept up into a spike, then a long keeled shin down to an armoured hoof, a tendon
//! showing behind the shin's plate. The head is a narrow prow, flat-faceted and swept back
//! to a point, a keel fin rising off its brow, three red optics low on its prow, the
//! seeker launcher a plated hump on its back, its cells open to the sky. Each cannon
//! gathers its charge past its mouth, held between the gun's lit faces
//! (`cannon`); the unit file's `muzzle` is the middle of that charge.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, seam-dark joints, graphite
//! machinery, red optics, the cannons' fusion lit in the prism. No violet: it does not
//! build.
//!
//! Rig: three legs posed by `entity.wgsl` `crawl_leg` (`MeshBuilder::set_crawl_legs`): the
//! front two a mirrored pair, the rear one a lone leg on the centreline (`set_lone_leg`),
//! so they step one at a time, front left, rear, front right, then all three stand a beat.
//! The head is the turret and turns about the unit's middle, the launcher with it; the
//! cannons pitch about their common trunnion (`rig::ARM_GUN`).
//!
//! Authored at the old tech 3 size and built [`SCALE`] times bigger: every number for it in
//! `data/factions/regency/units/experimental.ron` (`regency_t4_strider`) is this file's
//! times `SCALE` (tests).

mod cannon;
#[cfg(test)]
mod tests;

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::commander::form::{ball, blade, ring, sleeve, KEEL, OCT};
use super::kit::{dark_plate, metal, seam, v3};
use super::machine::{collar, hoop_on, red_slot, shaft};

/// Authored size (the unit file's is `SCALE` times it).
pub(super) const RADIUS: f32 = 12.0;
pub(super) const HEIGHT: f32 = 32.0;
/// How much bigger the unit file builds it.
#[cfg(test)]
const SCALE: f32 = 3.0;

/// The cannons' trunnion and the middles of their charges: the unit file's weapon `pivot`
/// and `muzzle`s, left then right.
const PIVOT: Vec3 = Vec3::new(1.2, 0.0, 24.9);
const MUZZLES: [Vec3; 2] = [
    Vec3::new(PIVOT.x + cannon::LEN, 3.8, PIVOT.z),
    Vec3::new(PIVOT.x + cannon::LEN, -3.8, PIVOT.z),
];
/// How far round each charge the gun stands: none of it closer than 0.4 of this.
#[cfg(test)]
const HOLD: f32 = 2.0;
/// The seeker launcher on the back of the head: its deck, clear over the head's ridge, and
/// its cells (x along the head, y across) in the order they fire, front pair first, left
/// before right.
const DECK: f32 = 31.0;
const CELLS: [[f32; 2]; 8] = [
    [-2.8, 0.62],
    [-2.8, -0.62],
    [-3.9, 0.62],
    [-3.9, -0.62],
    [-5.0, 0.62],
    [-5.0, -0.62],
    [-6.1, 0.62],
    [-6.1, -0.62],
];
/// A cell's mouth across.
const CELL_R: f32 = 0.42;
/// Where the head turns on the crown.
const RACE: f32 = 24.0;
/// The hips' height and how far out from the middle they sit.
const HIP_Z: f32 = 20.0;
const HIP_R: f32 = 2.2;
/// How far out the feet stand.
const FOOT_R: f32 = 10.5;
/// The ground one walking cycle covers at the authored size, the share of it a foot
/// stands, and how high a foot lifts: three steps and a beat on every foot standing
/// (`crawl_leg`). The builder scales the joints and the lift but not the stride, so
/// `strider` scales it to the build (`rig_legs`).
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
    rig_legs(b, &legs);
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
        launcher(b);
        b.with_limb(rig::ARM_GUN, |b| b.mirror_y(mount));
    });
}

/// The walking rig, its stride grown with the build to the nearest power of two (64 m at
/// the unit file's size): a stride left at the authored 16 m has the long legs patter
/// through tiny quick steps.
fn rig_legs(b: &mut MeshBuilder, legs: &[(Vec3, Vec3, Vec3, f32); 2]) {
    b.set_crawl_legs(legs, STRIDE, STANCE, LIFT);
    let scale = b.legs().map_or(1.0, |l| l.hip[2] / HIP_Z);
    b.set_crawl_legs(legs, STRIDE * scale.log2().round().exp2(), STANCE, LIFT);
    b.set_lone_leg(1);
}

/// Far off: the crown a spindle, flat legs that do not walk, the head a wedge with the
/// team colour on it, the cannons bars that still pitch.
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
/// either side over its gun, a keel fin on its brow, three red optics low on its prow.
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
    // The keel fin rising off its brow, swept back into the launcher.
    blade(
        b,
        v3(3.4, 0.0, 30.0),
        v3(-1.0, 0.0, 0.3),
        Vec3::Y,
        3.8,
        0.7,
        0.8,
        0.3,
    );
    eyes(b, 6.2, 26.6, 0.7);
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

/// The seeker launcher: a plated hump on the back of the head, swept down at its tail,
/// its deck open in two rows of cells, a lit red rim round each mouth, the team colour
/// down the deck between the rows.
fn launcher(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [-1.4, 0.0],
                [-1.8, 1.45],
                [-6.4, 1.45],
                [-7.2, 0.9],
                [-7.2, -0.9],
                [-6.4, -1.45],
                [-1.8, -1.45],
            ],
            &[
                Section::new(27.6, 0.86),
                Section::new(29.4, 1.0),
                Section::new(DECK, 0.98),
            ],
        );
    });
    for [x, y] in CELLS {
        let mouth = v3(x, y, DECK);
        // The cell's collar standing proud of the deck, dark inside.
        dark_plate(b);
        let sides = b.sides(8);
        b.cylinder_between(
            mouth - Vec3::Z * 0.2,
            mouth + Vec3::Z * 0.14,
            CELL_R + 0.1,
            CELL_R + 0.06,
            sides,
        );
        seam(b);
        b.cylinder_between(
            mouth + Vec3::Z * 0.1,
            mouth + Vec3::Z * 0.15,
            CELL_R * 0.86,
            CELL_R * 0.86,
            sides,
        );
        if fine {
            b.paint(GLOW_LASER);
            hoop_on(
                b,
                mouth + Vec3::Z * 0.16,
                Vec3::Z,
                CELL_R * 0.78,
                0.07,
                0.03,
                sides,
            );
        }
    }
    b.paint(TEAM);
    b.face(&[
        v3(-1.9, 0.0, DECK + 0.01),
        v3(-6.6, 0.16, DECK + 0.01),
        v3(-6.6, -0.16, DECK + 0.01),
    ]);
    if fine {
        // Red lines down the hump's flanks.
        b.mirror_y(|b| {
            red_slot(b, v3(-4.2, 1.46, 28.9), Vec3::Y, Vec3::X, 3.6, 0.12);
        });
    }
}

/// The left cannon (the right is its mirror), in the head's frame: it pitches about
/// `PIVOT`. A trunnion out from the neck to the gun, which is drawn in its own frame
/// (`cannon`).
fn mount(b: &mut MeshBuilder) {
    let m = MUZZLES[0];
    shaft(b, v3(PIVOT.x, 1.6, m.z), v3(PIVOT.x, m.y - 0.6, m.z), 0.36);
    collar(b, v3(PIVOT.x, m.y - 0.7, m.z), Vec3::Y, 0.55, 0.3);
    b.at(v3(PIVOT.x, m.y, m.z), cannon::cannon);
}

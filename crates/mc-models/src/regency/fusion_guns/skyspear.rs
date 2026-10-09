//! The tech 3 mobile anti-spaceship gun: a siege carriage that plants to fire. A long
//! faceted body rides four armoured tread pods, one at each corner on a short rigid arm
//! out of a housing in its flank. Packed, the pods run beside the body like a four-tracked
//! hull, the arms lying nearly flat, and the gun lies along the deck with its rails slid
//! back down the turret's flanks. Planting (`Model::siege`, `gpu_consts::siege`), each pod
//! turns in place out into an X, then the arms pivot down about their hips and push the
//! body up over the pods; the gun rises to the sky and its rails slide out along the bore.
//!
//! The gun is the Sunspear's own at [`S`] times its size: two tall rails either side of
//! a short coiled bore, lens heads at their tips, the rails parting and the heads sliding
//! out into the charge as it builds. Its fusion is housed in a plated drum across the
//! turret's back, lit only through slits (no caged core).
//!
//! Authored planted. The unit file's pivot and muzzle are [`LINE`]'s
//! (`regency_t3_mobile_aa`, `data/factions/regency/units/land_t3.ron`).

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::gpu_consts::siege::{ARM, POD};
use crate::material::*;
use crate::{part, Siege};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, collar, hoop_on, red_slot, Course, Frame};
use super::super::turrets::sunspear;
use super::{team_patch, turret_hull, yoke, Line};

/// The blueprint's radius and height.
pub(in crate::regency) const RADIUS: f32 = 22.0;
pub(in crate::regency) const HEIGHT: f32 = 42.0;

/// The Sunspear's gun at this size.
const S: f32 = 0.9;
/// The body, planted: nose and tail x, half its width, its underside and its deck.
const NOSE: f32 = 15.0;
const TAIL: f32 = -13.0;
const HALF: f32 = 7.5;
const BELLY: f32 = 8.0;
const DECK: f32 = 12.5;
/// The turret's roof.
const ROOF: f32 = DECK + 3.2;
/// The gun's line: raised 60 degrees at rest, the Sunspear's bore at [`S`].
pub(super) const LINE: Line = Line {
    pivot: Vec3::new(1.0, 0.0, DECK + 4.6),
    pitch_deg: 60.0,
    len: 26.6 * S,
};
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
pub(super) const HOLD: f32 = 4.3 * S;
/// How far back along the bore the rails lie packed, down the turret's flanks.
const RETRACT: f32 = 12.0 * S;

/// The front left arm, planted: its hip in the body's flank, its knee on the pod. Packed
/// it lies [`PACKED_PITCH`] below level, planted 40 degrees.
const HIP: Vec3 = Vec3::new(9.5, 6.0, 9.8);
const ARM_LEN: f32 = 6.5;
const ARM_WAY: Vec2 = Vec2::new(0.55, 0.835);
const ARM_PITCH: f32 = 0.698;
const PACKED_PITCH: f32 = 0.14;
/// How far the front left pod turns out to the left, planted: the X.
const SPLAY: f32 = 0.6;
/// A pod: half its length, half its width, its height.
const POD_SIZE: Vec3 = Vec3::new(6.2, 2.8, 5.0);
/// The arm's width and depth.
const ARM_THICK: Vec2 = Vec2::new(3.0, 2.6);

/// The front left arm's knee, planted.
fn knee() -> Vec3 {
    HIP + ARM_WAY.normalize().extend(0.0) * ARM_LEN * ARM_PITCH.cos()
        - Vec3::Z * ARM_LEN * ARM_PITCH.sin()
}

/// The front left pod's middle, packed.
fn packed_knee() -> Vec2 {
    HIP.truncate() + ARM_WAY.normalize() * ARM_LEN * PACKED_PITCH.cos()
}

pub(super) fn draw(b: &mut MeshBuilder) {
    LINE.rig(b, DECK, 0.6);
    let knee = knee();
    b.set_siege(Siege {
        hip: HIP.to_array(),
        knee: knee.to_array(),
        packed_pitch: PACKED_PITCH,
        splay: SPLAY,
        // Level along the deck, a hair down.
        stow: -LINE.pitch() - 0.03,
        retract: RETRACT,
    });
    // Packed, its tracks run beside the body.
    let packed = packed_knee();
    b.set_treads(packed.y, POD_SIZE.y * 2.0, -packed.x - POD_SIZE.x);
    b.set_dust_line(POD_SIZE.z);
    if b.coarse() {
        coarse(b, knee);
        return;
    }
    legs(b, knee);
    body(b);
    b.with_part(part::TURRET, |b| {
        turret_hull(
            b,
            &[[4.6, 2.4], [2.0, 4.4], [-4.6, 4.4], [-6.6, 2.6]],
            DECK,
            ROOF,
        );
        team_patch(b, -3.6, -1.0, 1.1, ROOF + 0.01);
        yoke(
            b,
            LINE.pivot,
            3.3,
            0.7,
            [-1.6, 3.0],
            [ROOF - 0.2, LINE.pivot.z + 1.2],
        );
        fusion_drum(b, v3(-4.4, 0.0, ROOF + 1.3));
        b.set_charge_gear(LINE.pivot, S);
        LINE.gun(b, |b| {
            b.with(Affine3A::from_scale(Vec3::splat(S)), |b| {
                sunspear::rails(b, 25.0, true);
                sunspear::pinch_coils(b);
            });
        });
    });
}

/// The four corners: front left, front right, rear left, rear right.
const CORNERS: [(f32, f32); 4] = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)];

/// Far off: the body a wedge, each pod a ridge (no arms: they are lost at this range),
/// and the gun a bar with its rails either side.
fn coarse(b: &mut MeshBuilder, knee: Vec3) {
    dark_plate(b);
    b.loft_z(
        &[[NOSE, 0.0], [0.0, HALF], [TAIL, 0.0], [0.0, -HALF]],
        &[Section::new(BELLY, 0.9), Section::new(ROOF, 0.6)],
    );
    team_patch(b, -3.0, 0.0, 1.0, ROOF + 0.01);
    for (sx, sy) in CORNERS {
        b.with_siege(POD, |b| {
            let frame = Affine3A::from_translation(v3(knee.x * sx, knee.y * sy, 0.0))
                * Affine3A::from_rotation_z(SPLAY * sx * sy);
            b.with(frame, |b| {
                b.with_part(part::LOCOMOTION, |b| {
                    b.paint(TREAD);
                    let Vec3 { x: l, y: w, z: h } = POD_SIZE;
                    for y in [-w, w] {
                        b.face(&[v3(-l, y, 0.2), v3(l, y, 0.2), v3(l, 0.0, h), v3(-l, 0.0, h)]);
                    }
                })
            });
        });
    }
    b.with_part(part::TURRET, |b| {
        LINE.gun(b, |b| {
            dark_plate(b);
            b.cylinder_between(Vec3::ZERO, v3(LINE.len * 0.7, 0.0, 0.0), 1.4, 1.1, 3);
            b.with_siege(0, |b| {
                b.mirror_y(|b| {
                    b.face(&[
                        v3(1.0, 3.4 * S, -1.2),
                        v3(LINE.len - 1.5, 3.6 * S, -0.8),
                        v3(LINE.len - 1.5, 3.6 * S, 0.8),
                        v3(1.0, 3.4 * S, 1.2),
                    ])
                });
            });
        });
    });
}

/// Each corner: a rigid armoured arm out of the body's flank at the hip, pivoting down to
/// plant, and the tread pod at its knee, turned out into the X.
fn legs(b: &mut MeshBuilder, knee: Vec3) {
    for (sx, sy) in CORNERS {
        let side = Vec3::new(sx, sy, 1.0);
        let (hip, knee) = (HIP * side, knee * side);
        let along = (knee - hip).normalize();
        b.with_siege(ARM, |b| {
            dark_plate(b);
            b.with_facets(|b| {
                b.beam(
                    hip - along * 0.8,
                    knee + along * 0.4,
                    ARM_THICK,
                    ARM_THICK * Vec2::new(0.85, 0.8),
                )
            });
            // A plate over the arm's top, swept into a point past the knee.
            let up = along.cross(Vec3::Z).cross(along).normalize();
            let len = (knee - hip).length();
            armour(
                b,
                &Frame::new(hip + up * ARM_THICK.y * 0.5, along, up),
                &[
                    [0.0, -ARM_THICK.x * 0.62],
                    [0.0, ARM_THICK.x * 0.62],
                    [len * 0.75, ARM_THICK.x * 0.56],
                    [len + 1.0, 0.0],
                    [len * 0.75, -ARM_THICK.x * 0.56],
                ],
                0.3,
            );
            collar(b, hip, Vec3::Y, ARM_THICK.y * 0.55, ARM_THICK.x + 0.4);
        });
        b.with_siege(POD, |b| {
            let frame = Affine3A::from_translation(v3(knee.x, knee.y, 0.0))
                * Affine3A::from_rotation_z(SPLAY * sx * sy);
            b.with(frame, |b| pod(b, knee.z, sy));
        });
    }
}

/// A tread pod in its own frame (its middle on the ground at the origin, running along
/// x), the knee's bracket on top at `knee_z`; `sy` the side of the body it is on.
fn pod(b: &mut MeshBuilder, knee_z: f32, sy: f32) {
    let Vec3 { x: l, y: w, z: h } = POD_SIZE;
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        let lozenge = [
            [-l + 0.5 * h, 0.0],
            [l - 0.5 * h, 0.0],
            [l, 0.5 * h],
            [l - 0.25 * h, 0.8 * h],
            [-l + 0.25 * h, 0.8 * h],
            [-l, 0.5 * h],
        ];
        b.extrude_y(&lozenge, -w, w);
        if b.fine() {
            metal(b);
            for i in 0..4 {
                let x = -l + 0.6 * h + (2.0 * l - 1.2 * h) * i as f32 / 3.0;
                for y in [-w, w] {
                    let out = y.signum() * 0.08;
                    b.cylinder_between(
                        v3(x, y, 0.3 * h),
                        v3(x, y + out, 0.3 * h),
                        0.24 * h,
                        0.2 * h,
                        8,
                    );
                }
            }
        }
    });
    // A plated cowl over the belt's top run, swept to points fore and aft.
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [l + 0.6, 0.0],
                [l - 0.6, w + 0.3],
                [-l + 0.6, w + 0.3],
                [-l - 0.6, 0.0],
                [-l + 0.6, -w - 0.3],
                [l - 0.6, -w - 0.3],
            ],
            &[
                Section::new(0.62 * h, 1.0),
                Section::new(0.9 * h, 0.96),
                Section::new(h, 0.7),
            ],
        )
    });
    // A heavy slab down the outer face, nearly to the ground, the owner's colour on it.
    let face = sy * (w + 0.35);
    b.with_facets(|b| {
        b.extrude_y(
            &[
                [-l * 0.8, 0.12 * h],
                [l * 0.8, 0.12 * h],
                [l * 0.95, 0.5 * h],
                [l * 0.75, 1.02 * h],
                [-l * 0.75, 1.02 * h],
                [-l * 0.95, 0.5 * h],
            ],
            face.min(face + sy * 0.5),
            face.max(face + sy * 0.5),
        )
    });
    b.paint(TEAM);
    let out = face + sy * 0.52;
    let quad = [
        v3(-l * 0.3, out, 0.4 * h),
        v3(l * 0.3, out, 0.4 * h),
        v3(l * 0.3, out, 0.75 * h),
        v3(-l * 0.3, out, 0.75 * h),
    ];
    if sy > 0.0 {
        b.face(&quad);
    } else {
        b.face(&[quad[1], quad[0], quad[3], quad[2]]);
    }
    // The knee's bracket: two plated cheeks and a bronze pin.
    let cheek = ARM_THICK.x * 0.5 + 0.25;
    b.mirror_y(|b| {
        dark_plate(b);
        b.beam(
            v3(0.0, cheek, h - 0.1),
            v3(0.0, cheek, knee_z + 0.6),
            Vec2::new(1.8, 0.4),
            Vec2::new(1.3, 0.4),
        );
    });
    collar(b, v3(0.0, 0.0, knee_z), Vec3::Y, 0.7, cheek * 2.0 + 0.5);
    if b.fine() {
        red_slot(b, v3(l + 0.2, 0.0, 0.8 * h), Vec3::X, Vec3::Y, 1.2, 0.14);
    }
}

/// The body, planted: a long faceted hull, drawn in toward its tail, its sides lapped in
/// plates swept back into points, optics at the nose and the owner's chevron aft.
fn body(b: &mut MeshBuilder) {
    let (half, belly, deck) = (HALF, BELLY, DECK);
    let plan = [
        [NOSE, 0.0],
        [NOSE * 0.6, half * 0.7],
        [NOSE * 0.1, half],
        [TAIL * 0.6, half * 0.95],
        [TAIL, half * 0.55],
        [TAIL, -half * 0.55],
        [TAIL * 0.6, -half * 0.95],
        [NOSE * 0.1, -half],
        [NOSE * 0.6, -half * 0.7],
    ];
    let h = deck - belly;
    seam(b);
    b.loft_z(
        &plan,
        &[
            Section::new(belly, 0.7),
            Section::new(belly + h * 0.35, 0.92),
        ],
    );
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(belly + h * 0.35, 0.94),
                Section::scaled(deck - 0.6, 1.0, 1.08),
                Section::scaled(deck, 0.88, 0.96).shifted(-0.3, 0.0),
            ],
        )
    });
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: if b.fine() { 4 } else { 2 },
            step: 5.6,
            len: 6.8,
            half: h * 0.32,
            tip: 0.7,
            thick: 0.3,
            tail: 2.4,
        }
        .lay(
            b,
            &Frame::new(
                v3(NOSE * 0.62, half * 0.78, belly + h * 0.62),
                v3(-1.0, -0.06, 0.0),
                v3(0.0, 1.0, 0.25),
            ),
        );
        red_slot(
            b,
            v3(NOSE * 0.78, half * 0.38, belly + h * 0.62),
            v3(0.6, 0.8, 0.0),
            Vec3::Y,
            0.9,
            0.16,
        );
    });
    // A plated housing at each corner that the arm pivots in.
    for (sx, sy) in CORNERS {
        let at = HIP * Vec3::new(sx, sy, 1.0);
        dark_plate(b);
        b.with_facets(|b| {
            b.cuboid(
                at - Vec3::Y * sy * 0.9,
                Vec3::new(ARM_THICK.x + 1.4, 2.4, ARM_THICK.y + 1.2),
            )
        });
    }
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            v3(TAIL * 0.55, 0.0, deck + 0.01),
            v3(TAIL * 0.55 - 1.6, 1.6, deck + 0.01),
            v3(TAIL * 0.55 - 2.3, 1.3, deck + 0.01),
            v3(TAIL * 0.55 - 0.7, 0.0, deck + 0.01),
        ])
    });
}

/// The fusion drum lying across the turret's back at `c`, short enough for the rails to lie
/// either side of it packed: a plated cylinder in a cradle of plates, bronze end caps, its
/// light only through slits along its top.
fn fusion_drum(b: &mut MeshBuilder, c: Vec3) {
    let sides = b.sides(14);
    dark_plate(b);
    b.cylinder_between(c - Vec3::Y * 1.9, c + Vec3::Y * 1.9, 1.7, 1.7, sides);
    metal(b);
    for y in [-2.0f32, 2.0] {
        hoop_on(b, c + Vec3::Y * y, Vec3::Y, 1.5, 0.35, 0.4, sides);
    }
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(c + v3(-1.9, 0.4, -1.2), Vec3::Y, -Vec3::X),
            &[[0.0, -0.4], [1.4, -0.4], [1.4, 2.6], [0.0, 2.0]],
            0.3,
        );
        if b.fine() {
            red_slot(b, c + v3(0.0, 0.9, 1.72), Vec3::Z, Vec3::Y, 1.2, 0.14);
        }
    });
}

//! The tech 3 mobile anti-spaceship gun: a siege carriage that plants to fire. A long
//! faceted body rides four tread pods, one at each corner on a plated leg; packed, the
//! pods run beside the body like a four-tracked hull and the gun lies along the deck, its
//! rails folded back over its top. Planting (`Model::siege`, `gpu_consts::siege`), the
//! pods splay out into an X, the legs stand and lift the body high over them, then the
//! gun rises to the sky and its rails swing up and over into place either side of the bore.
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
use crate::gpu_consts::siege::{LOWER, POD, UPPER};
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
/// How much higher the body stands planted than packed.
const LIFT: f32 = 7.5;
/// The body, planted: nose and tail x, half its width, its underside and its deck.
const NOSE: f32 = 15.0;
const TAIL: f32 = -13.0;
const HALF: f32 = 6.5;
const BELLY: f32 = 9.0;
const DECK: f32 = 14.0;
/// The turret's roof.
const ROOF: f32 = 17.2;
/// The gun's line: raised 60 degrees at rest, the Sunspear's bore at [`S`].
pub(super) const LINE: Line = Line {
    pivot: Vec3::new(1.0, 0.0, 18.6),
    pitch_deg: 60.0,
    len: 26.6 * S,
};
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
pub(super) const HOLD: f32 = 4.3 * S;

/// The front left leg, planted: hip on the body, knee on the pod. Packed, the knee stands
/// at `KNEE_PACKED` beside the body.
const HIP: Vec3 = Vec3::new(9.0, 2.5, 10.0);
const KNEE: Vec3 = Vec3::new(16.5, 13.5, 4.4);
const KNEE_PACKED: Vec2 = Vec2::new(9.5, 9.0);
/// How far the front left pod turns out to the left, planted: the X.
const SPLAY: f32 = 0.6;
/// A pod: half its length, half its width, its height.
const POD_HALF: f32 = 5.5;
const POD_WIDE: f32 = 2.2;
const POD_H: f32 = 4.0;
/// The leg's sleeve from the hip and blade from the knee: long enough to overlap planted.
const SLEEVE: f32 = 6.5;
const BLADE: f32 = 9.0;
/// The rails' hinge in the gun's frame (+x down the bore), and how far they fold packed:
/// right up and over, to lie back along the gun's top.
const FOLD_AT: Vec2 = Vec2::new(6.0, 2.2);
const FOLD: f32 = std::f32::consts::PI;

pub(super) fn draw(b: &mut MeshBuilder) {
    LINE.rig(b, DECK, 0.6);
    let a = LINE.pitch();
    let hinge = LINE.pivot
        + Vec3::new(a.cos(), 0.0, a.sin()) * FOLD_AT.x * S
        + Vec3::new(-a.sin(), 0.0, a.cos()) * FOLD_AT.y * S;
    b.set_siege(Siege {
        hip: HIP.to_array(),
        knee: KNEE.to_array(),
        packed: KNEE_PACKED.to_array(),
        lift: LIFT,
        splay: SPLAY,
        // Level along the deck, a hair down.
        stow: -a - 0.03,
        fold_hinge: [hinge.x, hinge.z],
        fold: FOLD,
    });
    // Packed, its tracks run beside the body.
    b.set_treads(KNEE_PACKED.y, POD_WIDE * 2.0, -KNEE_PACKED.x - POD_HALF);
    b.set_dust_line(POD_H);
    if b.coarse() {
        coarse(b);
        return;
    }
    legs(b);
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

/// Far off: the body a wedge, each pod a ridge (no legs: they are lost at this range),
/// and the gun a bar with its rails either side.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.loft_z(
        &[[NOSE, 0.0], [0.0, HALF], [TAIL, 0.0], [0.0, -HALF]],
        &[Section::new(BELLY, 0.9), Section::new(ROOF, 0.6)],
    );
    team_patch(b, -3.0, 0.0, 1.0, ROOF + 0.01);
    for (sx, sy) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        b.with_siege(POD, |b| {
            let frame = Affine3A::from_translation(v3(KNEE.x * sx, KNEE.y * sy, 0.0))
                * Affine3A::from_rotation_z(SPLAY * sx * sy);
            b.with(frame, |b| {
                b.with_part(part::LOCOMOTION, |b| {
                    b.paint(TREAD);
                    let (l, w, h) = (POD_HALF, POD_WIDE, POD_H);
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

/// Each leg: a plated sleeve out of the body's flank at the hip and a blade up off the pod
/// that slide along each other, and the tread pod, turned out into the X.
fn legs(b: &mut MeshBuilder) {
    for (sx, sy) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        let side = Vec3::new(sx, sy, 1.0);
        let (hip, knee) = (HIP * side, KNEE * side);
        let along = (knee - hip).normalize();
        b.with_siege(UPPER, |b| {
            dark_plate(b);
            b.beam(
                hip,
                hip + along * SLEEVE,
                Vec2::new(1.6, 2.2),
                Vec2::new(1.4, 1.9),
            );
            collar(b, hip, Vec3::Y, 1.3, 2.0);
        });
        b.with_siege(LOWER, |b| {
            metal(b);
            b.beam(
                knee,
                knee - along * BLADE,
                Vec2::new(1.0, 1.3),
                Vec2::new(0.9, 1.2),
            );
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
    let (l, w, h) = (POD_HALF, POD_WIDE, POD_H);
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
    // Plates lapped down its outer face, the side away from the body.
    Course {
        count: if b.fine() { 3 } else { 2 },
        step: 2.9,
        len: 3.6,
        half: 0.75,
        tip: 0.6,
        thick: 0.22,
        tail: 1.0,
    }
    .lay(
        b,
        &Frame::new(
            v3(l - 0.8, sy * (w + 0.32), 0.66 * h),
            v3(-1.0, 0.0, 0.0),
            v3(0.0, sy, 0.25),
        ),
    );
    // The knee's bracket: two plated cheeks and a bronze pin.
    b.mirror_y(|b| {
        dark_plate(b);
        b.beam(
            v3(0.0, 0.75, h - 0.1),
            v3(0.0, 0.75, knee_z + 0.6),
            Vec2::new(1.8, 0.4),
            Vec2::new(1.3, 0.4),
        );
    });
    collar(b, v3(0.0, 0.0, knee_z), Vec3::Y, 0.55, 2.0);
    if b.fine() {
        red_slot(b, v3(l + 0.2, 0.0, 0.8 * h), Vec3::X, Vec3::Y, 1.2, 0.14);
    }
}

/// The body, planted: a long faceted hull, drawn in toward its tail, its sides lapped in
/// plates swept back into points, optics at the nose and the owner's chevron aft.
fn body(b: &mut MeshBuilder) {
    let plan = [
        [NOSE, 0.0],
        [NOSE * 0.6, HALF * 0.7],
        [NOSE * 0.1, HALF],
        [TAIL * 0.6, HALF * 0.95],
        [TAIL, HALF * 0.55],
        [TAIL, -HALF * 0.55],
        [TAIL * 0.6, -HALF * 0.95],
        [NOSE * 0.1, -HALF],
        [NOSE * 0.6, -HALF * 0.7],
    ];
    let h = DECK - BELLY;
    seam(b);
    b.loft_z(
        &plan,
        &[
            Section::new(BELLY, 0.7),
            Section::new(BELLY + h * 0.35, 0.92),
        ],
    );
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(BELLY + h * 0.35, 0.94),
                Section::scaled(DECK - 0.6, 1.0, 1.08),
                Section::scaled(DECK, 0.88, 0.96).shifted(-0.3, 0.0),
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
                v3(NOSE * 0.62, HALF * 0.78, BELLY + h * 0.62),
                v3(-1.0, -0.06, 0.0),
                v3(0.0, 1.0, 0.25),
            ),
        );
        red_slot(
            b,
            v3(NOSE * 0.78, HALF * 0.38, BELLY + h * 0.62),
            v3(0.6, 0.8, 0.0),
            Vec3::Y,
            0.9,
            0.16,
        );
    });
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            v3(TAIL * 0.55, 0.0, DECK + 0.01),
            v3(TAIL * 0.55 - 1.6, 1.6, DECK + 0.01),
            v3(TAIL * 0.55 - 2.3, 1.3, DECK + 0.01),
            v3(TAIL * 0.55 - 0.7, 0.0, DECK + 0.01),
        ])
    });
}

/// The fusion drum lying across the turret's back at `c`: a plated cylinder in a cradle of
/// plates, bronze end caps, its light only through slits along its top.
fn fusion_drum(b: &mut MeshBuilder, c: Vec3) {
    let sides = b.sides(14);
    dark_plate(b);
    b.cylinder_between(c - Vec3::Y * 3.4, c + Vec3::Y * 3.4, 1.7, 1.7, sides);
    metal(b);
    for y in [-3.6f32, 3.6] {
        hoop_on(b, c + Vec3::Y * y, Vec3::Y, 1.5, 0.35, 0.4, sides);
    }
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(c + v3(-1.9, 1.2, -1.2), Vec3::Y, -Vec3::X),
            &[[0.0, -0.4], [2.6, -0.4], [2.6, 2.6], [0.0, 2.0]],
            0.3,
        );
        if b.fine() {
            red_slot(b, c + v3(0.0, 1.6, 1.72), Vec3::Z, Vec3::Y, 1.6, 0.14);
        }
    });
}

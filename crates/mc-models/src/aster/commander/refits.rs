//! The commander's back and shoulder refits: the formation engine and the shield packs,
//! the flak cannon and the howitzer on the right shoulder, the auxiliary projector on the
//! left.

use glam::Vec3;

use super::*;
use crate::aster::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// The back: bare, or one of the two packs. The formation engine is a wide,
/// low block of furnace and drums; the shield pack a narrow core under two
/// tall projector fins that stand clear of the shoulders.
pub(super) fn back(b: &mut MeshBuilder) {
    b.module("mfe", 0.0, |b| {
        // Shell: black, two white lids with a furnace slot between them.
        b.paint(ACCENT);
        b.block(v3(-3.2, -1.55, 10.4), v3(-2.0, 1.55, 13.3));
        b.extrude_y_chamfered(
            &[
                [-3.75, 10.2],
                [-3.1, 10.2],
                [-3.1, 13.6],
                [-2.1, 13.6],
                [-2.1, 13.9],
                [-3.75, 13.2],
            ],
            1.75,
            0.3,
        );
        b.paint(PLATING);
        b.mirror_y(|b| b.plate(v3(-2.9, 0.95, 13.62), v2(1.2, 1.0), 0.1, 0.04));
        b.paint(GLOW_ORANGE).pattern(pattern::FURNACE);
        b.block(v3(-3.3, -0.32, 13.55), v3(-2.3, 0.32, 13.72));
    });
    b.module("mfe", 0.35, |b| {
        // Formation drums, one each side, banded in the furnace's light.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(
                v3(-3.3, 2.0, 10.4),
                v3(-3.3, 2.0, 13.2),
                0.62,
                0.62,
                b.sides(10),
            );
            b.paint(PLATING);
            b.cylinder_between(
                v3(-3.3, 2.0, 13.2),
                v3(-3.3, 2.0, 13.45),
                0.5,
                0.42,
                b.sides(10),
            );
            b.paint(GLOW_ORANGE);
            for z in [10.9, 11.7, 12.5] {
                b.cylinder_between(
                    v3(-3.3, 2.0, z),
                    v3(-3.3, 2.0, z + 0.16),
                    0.66,
                    0.66,
                    b.sides(10),
                );
            }
        });
    });
    b.module("mfe", 0.7, |b| {
        if b.fine() {
            b.mirror_y(|b| {
                b.paint(ACCENT).pattern(pattern::FURNACE);
                b.plate(v3(-2.6, 0.95, 13.72), v2(0.6, 0.7), 0.08, 0.04)
            });
            b.paint(METAL);
            b.mirror_y(|b| {
                b.cylinder_between(v3(-2.3, 1.3, 13.0), v3(-2.3, 2.55, 12.6), 0.12, 0.12, 6)
            });
        }
    });

    b.module("shield", 0.0, |b| {
        // Generator core: a narrow white housing with a metal-ringed lens, set close to the spine.
        // The personal field is thrown from the lens (`entity.wgsl` `hull_emitter`).
        b.set_shield_emitter(v3(-3.4, 0.0, 11.9));
        b.paint(ACCENT);
        b.block(v3(-2.85, -0.85, 10.6), v3(-2.05, 0.85, 13.1));
        b.paint(PLATING);
        b.extrude_y_chamfered(
            &[[-3.2, 10.9], [-2.8, 10.9], [-2.8, 13.0], [-3.2, 12.7]],
            0.95,
            0.2,
        );
        b.paint(METAL);
        b.cylinder_between(
            v3(-3.2, 0.0, 11.9),
            v3(-3.35, 0.0, 11.9),
            0.55,
            0.5,
            b.sides(10),
        );
        b.paint(GLOW_SHIELD);
        b.cylinder_between(
            v3(-3.34, 0.0, 11.9),
            v3(-3.4, 0.0, 11.9),
            0.32,
            0.3,
            b.sides(10),
        );
    });
    b.module("shield", 0.35, |b| {
        // Projector fins: tall, raked back, lit along the leading edge in the field's gold.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [-2.9, 12.2],
                    [-2.3, 12.4],
                    [-2.7, 16.1],
                    [-3.2, 16.3],
                    [-3.3, 12.4],
                ],
                1.05,
                1.3,
            );
            b.paint(ACCENT);
            b.extrude_y(
                &[[-3.3, 11.2], [-2.4, 11.2], [-2.3, 12.4], [-3.3, 12.4]],
                0.95,
                1.4,
            );
            b.paint(GLOW_SHIELD);
            b.extrude_y(
                &[[-2.32, 12.5], [-2.2, 12.5], [-2.62, 16.0], [-2.72, 16.0]],
                1.1,
                1.25,
            );
        });
    });
    b.module("shield", 0.7, |b| {
        if b.fine() {
            b.paint(GLOW_SHIELD);
            b.mirror_y(|b| b.cuboid(v3(-2.95, 1.18, 16.35), Vec3::splat(0.22)));
            b.paint(METAL);
            b.cylinder_between(v3(-2.6, -1.1, 14.0), v3(-2.6, 1.1, 14.0), 0.1, 0.1, 6);
        }
    });
    b.module("shield_2", 0.5, |b| {
        // The heavy field: a wider lens over the generator's, ringed in metal, and a
        // crossbar tying the fin tips together.
        b.paint(METAL);
        b.cylinder_between(
            v3(-3.3, 0.0, 11.9),
            v3(-3.5, 0.0, 11.9),
            0.95,
            0.9,
            b.sides(12),
        );
        b.paint(GLOW_SHIELD);
        b.cylinder_between(
            v3(-3.48, 0.0, 11.9),
            v3(-3.56, 0.0, 11.9),
            0.72,
            0.68,
            b.sides(12),
        );
        b.paint(ACCENT);
        b.cylinder_between(v3(-2.9, -1.3, 16.2), v3(-2.9, 1.3, 16.2), 0.16, 0.16, 8);
    });
}

/// The shoulders' refits. On the right, a turret of its own that aims at what it shoots,
/// whatever the torso is doing: the AA flak cannon (`flak`), an open mount with
/// a gun shield and two slim barrels, or the howitzer, one long tube over the shoulder. On the
/// left, a second projector that lies back behind the shoulder and folds out over it
/// to build.
pub(super) fn shoulders(b: &mut MeshBuilder) {
    b.module("aa", 0.0, flak::house);
    b.module("aa", 0.4, flak::guns);

    let t = TRUNNION;
    b.module("artillery", 0.0, |b| {
        flak::turntable(b, t);
        b.with_mount(t, 0.7, |b| {
            // Cradle: two white cheeks the tube swings between, standing still on the ring
            // as it pitches, and a black counterweight behind that pitches with it.
            // Mirrored about the trunnion, not the body's centreline, or one cheek would sit
            // on the other shoulder.
            b.with_yoke(|b| {
                b.paint(PLATING);
                b.at(v3(0.0, t.y, 0.0), |b| {
                    b.mirror_y(|b| {
                        b.extrude_y(
                            &[
                                [t.x - 1.3, t.z - 0.75],
                                [t.x + 0.5, t.z - 0.75],
                                [t.x + 0.3, t.z + 0.4],
                                [t.x - 1.0, t.z + 0.55],
                            ],
                            0.36,
                            0.52,
                        )
                    })
                });
            });
            b.paint(ACCENT);
            b.chamfered_box(t + v3(-1.6, 0.0, 0.0), v3(1.1, 0.8, 0.9), 0.2);
            b.paint(METAL);
            b.cylinder_between(
                t - Vec3::Y * 0.56,
                t + Vec3::Y * 0.56,
                0.24,
                0.24,
                b.sides(8),
            );
        });
    });
    b.module("artillery", 0.4, |b| {
        b.with_mount(t, 0.7, |b| {
            b.with_recoil(|b| howitzer(b, t - Vec3::X * 1.1, HOWITZER_MUZZLE, 0.3));
            if b.fine() {
                b.paint(METAL);
                b.cylinder_between(
                    t + v3(0.1, 0.0, -0.32),
                    t + v3(1.8, 0.0, -0.32),
                    0.1,
                    0.1,
                    6,
                );
                b.paint(GLOW_ORANGE);
                b.block(t + v3(-2.1, -0.3, 0.1), t + v3(-2.14, 0.3, 0.3));
            }
        });
    });

    b.module("aux_eng", 0.0, |b| {
        // The mount on the back behind the left pauldron: a black saddle and the base knuckle.
        b.paint(ACCENT);
        b.chamfered_box(v3(-2.35, 2.55, 12.45), v3(0.9, 1.0, 1.3), 0.18);
        b.paint(PLATING);
        b.block(v3(-2.85, 2.1, 12.2), v3(-2.78, 3.0, 13.0));
        b.paint(METAL);
        b.cylinder_between(
            AUX_HINGE - Vec3::Y * 0.5,
            AUX_HINGE + Vec3::Y * 0.5,
            0.36,
            0.36,
            b.sides(10),
        );
    });
    b.module("aux_eng", 0.3, |b| {
        // The arm, authored out: black, a white guard down its back, a piston under it and
        // a lit feed line; knuckles at both ends.
        b.with_fold(AUX_HINGE, AUX_STOWED, |b| {
            let along = (AUX_ELBOW - AUX_HINGE).normalize();
            let over = Vec3::new(-along.z, 0.0, along.x);
            b.paint(ACCENT);
            b.beam(AUX_HINGE, AUX_ELBOW, v2(0.5, 0.55), v2(0.4, 0.45));
            b.paint(PLATING);
            b.beam(
                AUX_HINGE + along * 0.6 + over * 0.3,
                AUX_ELBOW - along * 0.5 + over * 0.26,
                v2(0.56, 0.1),
                v2(0.46, 0.09),
            );
            b.paint(METAL);
            b.cylinder_between(
                AUX_ELBOW - Vec3::Y * 0.2,
                AUX_ELBOW + Vec3::Y * 0.75,
                0.3,
                0.3,
                b.sides(8),
            );
            if b.fine() {
                b.cylinder_between(
                    AUX_HINGE + along * 0.5 - over * 0.36,
                    AUX_HINGE + along * 3.2 - over * 0.3,
                    0.1,
                    0.1,
                    6,
                );
                b.paint(ACCENT).pattern(pattern::CONDUIT);
                b.beam(
                    AUX_HINGE + along * 0.9 + Vec3::Y * 0.22,
                    AUX_ELBOW - along * 0.7 + Vec3::Y * 0.22,
                    v2(0.08, 0.3),
                    v2(0.08, 0.26),
                );
            }
        });
        // The head, outboard of the arm on the wrist knuckle: a short housing, a white cowl
        // and an amber projector with three prongs, pointing at the work.
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, |b| {
            let d = (AUX_EMITTER - AUX_WRIST).normalize();
            let up = Vec3::new(-d.z, 0.0, d.x);
            let at = |t: f32| AUX_WRIST + d * t;
            b.paint(ACCENT);
            b.beam(at(-0.3), at(1.5), v2(0.5, 0.55), v2(0.42, 0.42));
            b.paint(PLATING);
            b.beam(
                at(0.0) + up * 0.3,
                at(1.35) + up * 0.24,
                v2(0.5, 0.08),
                v2(0.4, 0.07),
            );
            b.paint(METAL);
            b.cylinder_between(at(1.5), at(2.2), 0.24, 0.2, b.sides(8));
            b.paint(GLOW_AMBER);
            b.cylinder_between(at(2.2), at(2.55), 0.16, 0.05, 6);
            if b.fine() {
                let side = Vec3::Y;
                b.paint(METAL);
                for (s, u) in [(0.28, 0.16), (-0.28, 0.16), (0.0, -0.3)] {
                    b.beam(
                        at(1.7) + side * s + up * u,
                        at(2.45) + side * s * 0.55 + up * u * 0.55,
                        v2(0.08, 0.08),
                        v2(0.05, 0.05),
                    );
                }
            }
        });
    });
}

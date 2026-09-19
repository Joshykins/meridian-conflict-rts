//! Aster walkers: the commander, the Paladin assault bot and the Lancer light bot.
//!
//! Legs and feet are `LOCOMOTION`, the pelvis is `HULL`, and everything above
//! the waist ring (torso, arms, head) is the `TURRET`, twisting about the
//! model's z axis so arm-mounted muzzles track the sim's muzzle offsets.

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::part;

/// One leg on the +y side, hip first. Each segment is a tapering armoured
/// beam (`girth`: start and end cross-section) between two joints.
struct Leg<'a> {
    joints: &'a [Vec3],
    girth: &'a [(Vec2, Vec2)],
    /// Foot outline relative to the last joint: (x rear, x front, height, width).
    foot: (f32, f32, f32, f32),
    /// Segments from this index on are bare frame rather than plating.
    bare_from: usize,
}

fn leg(b: &mut MeshBuilder, leg: &Leg) {
    let (hip, ankle) = (leg.joints[0], leg.joints[leg.joints.len() - 1]);
    let (rear, front, height, width) = leg.foot;
    b.with_part(part::LOCOMOTION, |b| {
        if b.coarse() {
            b.paint(PLATING);
            let top = leg.girth[0].0;
            b.frustum_open(v3(ankle.x + (rear + front) * 0.5, ankle.y, 0.0), v2(front - rear, width), v2(top.y, top.x), hip.z, v2(hip.x - ankle.x - (rear + front) * 0.5, hip.y - ankle.y));
            return;
        }
        for (i, pair) in leg.joints.windows(2).enumerate() {
            b.paint(if i >= leg.bare_from { METAL } else { PLATING });
            b.beam(pair[0], pair[1], leg.girth[i].0, leg.girth[i].1);
        }
        // Joint drums across the limb.
        b.paint(ACCENT);
        let drums = if b.fine() { leg.joints.len() - 1 } else { 1 };
        for (i, &joint) in leg.joints.iter().enumerate().skip(1).take(drums) {
            let end = leg.girth[i - 1].1;
            let half = Vec3::Y * (end.x * 0.56);
            b.cylinder_between(joint - half, joint + half, end.y * 0.62, end.y * 0.62, if b.fine() { 8 } else { 4 });
        }
        // Foot: dark sole, white toe cap.
        let (x0, x1, h) = (ankle.x + rear, ankle.x + front, height);
        b.paint(TREAD);
        b.extrude_y(&[[x0, 0.0], [x1, 0.0], [x1, 0.4 * h], [x1 - 0.3 * (x1 - x0), h], [x0 + 0.15 * (x1 - x0), h], [x0, 0.55 * h]], ankle.y - width * 0.5, ankle.y + width * 0.5);
        if b.fine() {
            b.paint(PLATING);
            on_slope(b, [x1, 0.4 * h], [x1 - 0.3 * (x1 - x0), h], 0.5, |b| b.plate(v3(0.0, ankle.y, 0.0), v2(0.3 * (x1 - x0), width * 0.8), 0.1 * h, 0.04 * h));
            // Knee guard on the first joint below the hip.
            let knee = leg.joints[1];
            let size = leg.girth[0].1;
            b.frustum(knee + v3(size.y * 0.35, 0.0, -size.y * 0.5), v2(size.y * 0.5, size.x * 1.05), v2(size.y * 0.3, size.x * 0.7), size.y * 1.1, v2(size.y * 0.2, 0.0));
        }
    });
}

// ---- ARC Commander -----------------------------------------------------------

pub fn commander(b: &mut MeshBuilder, _tech: u8) {
    let stance = Leg {
        joints: &[v3(0.0, 1.75, 7.9), v3(0.6, 1.85, 4.5), v3(-0.25, 1.85, 1.3)],
        girth: &[(v2(1.5, 1.9), v2(1.25, 1.4)), (v2(1.2, 1.3), v2(1.5, 1.7))],
        foot: (-1.3, 2.3, 1.3, 1.9),
        bare_from: usize::MAX,
    };
    b.mirror_y(|b| leg(b, &stance));

    let gun_y = -3.2;
    b.set_turret_pivot(v3(0.0, 0.0, 8.7));
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.frustum_open(v3(-0.2, 0.0, 7.6), v2(3.2, 4.4), v2(3.0, 5.4), 5.4, v2(0.0, 0.0));
            team_panel(b, v3(-0.9, 0.0, 13.0), v2(1.2, 5.0));
            b.paint(PLATING);
            b.beam(v3(-2.4, gun_y, 10.5), v3(4.0, gun_y, 10.5), v2(1.9, 1.8), v2(1.0, 0.8));
            b.beam(v3(-1.6, 3.2, 10.8), v3(2.6, 3.2, 10.2), v2(1.5, 1.4), v2(1.1, 1.0));
        });
        return;
    }

    // Pelvis and hip joints.
    b.paint(ACCENT);
    b.chamfered_box(v3(0.0, 0.0, 7.85), v3(2.2, 2.6, 1.5), 0.5);
    b.cylinder_between(v3(0.0, -2.5, 7.9), v3(0.0, 2.5, 7.9), 0.75, 0.75, b.sides(8));
    b.paint(PLATING);
    b.frustum(v3(0.95, 0.0, 7.0), v2(0.5, 1.3), v2(0.7, 1.9), 1.5, v2(0.15, 0.0));

    b.with_part(part::TURRET, |b| {
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 8.5), 8, 1.35, 1.35, 0.6);
        // Torso: wedge chest, tapering to the collar.
        b.paint(PLATING);
        b.extrude_y_chamfered(&[[-1.7, 9.0], [1.3, 9.0], [2.3, 10.4], [2.0, 12.0], [0.6, 13.0], [-1.8, 12.7], [-2.4, 10.6]], 2.4, 0.6);
        // Head.
        b.at(v3(0.35, 0.0, 0.0), |b| {
            b.paint(PLATING);
            b.loft_z(&turret_plan(1.9, 1.6), &[Section::new(12.85, 0.85), Section::new(13.5, 1.0), Section::scaled(14.3, 0.55, 0.6).shifted(-0.2, 0.0)]);
            b.paint(GLOW);
            b.block(v3(0.7, -0.4, 13.42), v3(0.97, 0.4, 13.64));
        });
        // Backpack reactor.
        b.paint(ACCENT);
        b.block(v3(-3.5, -1.7, 9.9), v3(-2.0, 1.7, 12.7));
        b.paint(PLATING);
        b.extrude_y_chamfered(&[[-3.75, 10.2], [-3.4, 10.2], [-3.4, 13.1], [-2.2, 13.1], [-2.2, 12.7], [-3.75, 12.2]], 1.85, 0.25);

        // Shoulders with team-colour pauldrons.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(0.0, 2.2, 11.7), v3(0.0, 3.0, 11.7), 0.75, 0.75, b.sides(8));
            b.paint(PLATING);
            b.at(v3(0.0, 3.3, 0.0), |b| {
                b.loft_z(&chamfered_rect(v2(1.45, 1.05), 0.45), &[Section::new(11.5, 0.9), Section::new(12.3, 1.0), Section::scaled(12.85, 0.7, 0.6).shifted(0.0, -0.2)]);
            });
            team_panel(b, v3(0.0, 3.1, 12.85), v2(1.7, 1.0));
        });

        // Right arm: the Meridian rail rifle.
        b.paint(ACCENT);
        b.beam(v3(0.0, gun_y, 11.4), v3(-0.5, gun_y, 10.7), v2(0.9, 0.9), v2(0.8, 0.8));
        b.at(v3(0.0, gun_y, 0.0), |b| {
            b.paint(PLATING);
            b.extrude_y_chamfered(&[[-2.6, 9.6], [0.9, 9.6], [1.5, 10.05], [1.5, 10.95], [0.9, 11.4], [-2.6, 11.4]], 1.0, 0.3);
        });
        rail_gun(b, v3(1.1, gun_y, 10.5), v3(4.0, gun_y, 10.5), v2(0.3, 0.64), 0.3, Emitter::Blue);

        // Left arm: construction projector.
        let (elbow, wrist) = (v3(-0.5, 3.2, 10.7), v3(1.9, 3.2, 10.15));
        b.paint(ACCENT);
        b.beam(v3(0.0, 3.2, 11.4), elbow, v2(0.9, 0.9), v2(0.8, 0.8));
        b.paint(PLATING);
        b.beam(elbow + v3(-0.5, 0.0, 0.1), wrist, v2(1.3, 1.3), v2(1.1, 1.1));
        b.paint(GLOW);
        b.cylinder_between(wrist, wrist + v3(0.75, 0.0, -0.17), 0.42, 0.15, 6);

        if b.fine() {
            // Chest: cockpit glass, collar sensors, reactor vents under the wedge.
            on_slope(b, [2.0, 12.0], [0.6, 13.0], 0.62, |b| {
                b.paint(GLASS);
                b.plate(Vec3::ZERO, v2(0.8, 1.7), 0.08, 0.04);
            });
            on_slope(b, [2.3, 10.4], [2.0, 12.0], 0.5, |b| {
                b.mirror_y(|b| glow_strip(b, v3(0.0, 1.0, 0.0), v2(1.0, 0.16), GLOW));
                b.paint(ACCENT);
                b.plate(Vec3::ZERO, v2(0.9, 1.0), 0.07, 0.03);
            });
            on_slope(b, [1.3, 9.0], [2.3, 10.4], 0.5, |b| b.mirror_y(|b| glow_strip(b, v3(0.0, 0.9, 0.0), v2(1.1, 0.5), GLOW)));
            // Prongs around the construction emitter.
            b.paint(METAL);
            for (dy, dz) in [(0.45, 0.3), (-0.45, 0.3), (0.0, -0.5)] {
                b.beam(wrist + v3(-0.2, dy, dz), wrist + v3(1.1, dy * 0.6, dz * 0.6 - 0.25), v2(0.16, 0.16), v2(0.1, 0.1));
            }
            // Rifle furniture: energy cell with status lights, sight.
            b.paint(ACCENT);
            b.block(v3(-2.0, gun_y - 0.5, 11.4), v3(-0.4, gun_y + 0.5, 11.75));
            glow_strip(b, v3(-1.2, gun_y, 11.75), v2(1.2, 0.24), GLOW);
            b.paint(GLASS);
            b.block(v3(0.1, gun_y - 0.2, 11.4), v3(0.7, gun_y + 0.2, 11.7));
            // Backpack vents and antennae.
            b.mirror_y(|b| vent(b, v3(-2.8, 0.85, 13.1), v2(0.9, 1.0), 3, GLOW));
            antenna(b, v3(-3.2, -1.5, 13.0), 2.6, 0.12);
            antenna(b, v3(-3.2, 1.5, 13.0), 1.7, 0.12);
            b.mirror_y(|b| {
                b.paint(GLOW);
                b.block(v3(-3.79, 0.5, 10.6), v3(-3.75, 1.2, 11.8));
            });
        }
    });

    if b.fine() {
        // Thigh and shin light bars.
        b.mirror_y(|b| {
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(GLOW);
                b.beam(v3(1.22, 1.8, 6.6), v3(1.45, 1.84, 5.4), v2(0.5, 0.08), v2(0.4, 0.08));
                b.paint(TEAM);
                b.beam(v3(0.75, 1.85, 3.6), v3(0.55, 1.85, 2.4), v2(0.9, 0.1), v2(1.0, 0.1));
            });
        });
    }
}

// ---- Paladin: assault bot ----------------------------------------------------

pub fn assault_bot(b: &mut MeshBuilder, _tech: u8) {
    let stance = Leg {
        joints: &[v3(-0.2, 2.3, 6.5), v3(1.5, 2.5, 4.3), v3(-1.3, 2.5, 2.3), v3(0.1, 2.5, 0.9)],
        girth: &[(v2(1.5, 2.0), v2(1.2, 1.4)), (v2(1.1, 1.3), v2(0.9, 1.0)), (v2(0.7, 0.8), v2(0.9, 1.0))],
        foot: (-1.5, 2.2, 0.9, 2.1),
        bare_from: 2,
    };
    b.mirror_y(|b| leg(b, &stance));

    b.set_turret_pivot(v3(0.0, 0.0, 7.2));
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.frustum_open(v3(-0.2, 0.0, 6.3), v2(4.6, 5.0), v2(3.4, 5.2), 4.2, v2(-0.6, 0.0));
            team_panel(b, v3(-1.4, 0.0, 10.5), v2(1.4, 4.8));
            b.mirror_y(|b| b.paint(PLATING).beam(v3(-2.6, 3.6, 9.0), v3(3.4, 3.6, 9.0), v2(1.9, 1.7), v2(1.2, 0.9)));
        });
        return;
    }

    b.paint(ACCENT);
    b.chamfered_box(v3(-0.2, 0.0, 6.4), v3(2.6, 3.4, 1.5), 0.6);
    b.cylinder_between(v3(-0.2, -3.0, 6.5), v3(-0.2, 3.0, 6.5), 0.85, 0.85, b.sides(8));

    b.with_part(part::TURRET, |b| {
        b.paint(ACCENT);
        b.prism(v3(-0.2, 0.0, 7.0), 8, 1.7, 1.7, 0.5);
        // Low, broad hull-torso with a sunk sensor band instead of a head.
        b.paint(PLATING);
        b.extrude_y_chamfered(&[[-2.6, 7.4], [1.6, 7.4], [3.0, 8.4], [2.6, 9.6], [0.8, 10.6], [-2.4, 10.4], [-3.2, 9.0]], 2.6, 0.7);
        on_slope(b, [3.0, 8.4], [2.6, 9.6], 0.5, |b| {
            b.paint(ACCENT);
            b.plate(Vec3::ZERO, v2(0.8, 3.0), 0.1, 0.04);
            glow_strip(b, v3(0.0, 0.0, 0.1), v2(0.3, 2.4), GLOW);
        });
        on_slope(b, [0.8, 10.6], [-2.4, 10.4], 0.35, |b| team_panel(b, Vec3::ZERO, v2(1.5, 3.0)));

        // Arms: heavy arc projectors slung from the shoulders.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(-0.4, 2.3, 9.2), v3(-0.4, 3.0, 9.2), 0.8, 0.8, b.sides(8));
            b.at(v3(0.0, 3.6, 0.0), |b| {
                b.paint(PLATING);
                b.extrude_y_chamfered(&[[-2.7, 8.1], [0.5, 8.1], [1.2, 8.5], [1.2, 9.5], [0.4, 9.95], [-2.7, 9.95]], 0.98, 0.28);
            });
            rail_gun(b, v3(0.9, 3.6, 9.0), v3(3.4, 3.6, 9.0), v2(0.28, 0.62), 0.36, Emitter::Blue);
            team_panel(b, v3(-1.9, 3.6, 9.95), v2(0.8, 1.2));
            if b.fine() {
                glow_strip(b, v3(-0.5, 3.6, 9.95), v2(1.5, 0.22), GLOW);
                // Capacitor coils behind each projector.
                for i in 0..3 {
                    let x = -2.5 + 0.55 * i as f32;
                    b.paint(GLOW);
                    b.block(v3(x, 4.58, 8.4), v3(x + 0.25, 4.62, 9.6));
                }
                b.paint(ACCENT);
                b.block(v3(-3.0, 3.0, 8.4), v3(-2.7, 4.2, 9.6));
            }
        });

        // Heat-sink fins on the back.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(&[[-3.0, 9.6], [-1.6, 10.45], [-2.2, 11.6], [-3.9, 10.6]], 0.7, 1.0);
            b.extrude_y(&[[-3.0, 9.6], [-1.6, 10.45], [-2.0, 11.3], [-3.7, 10.4]], 1.6, 1.9);
        });
        if b.fine() {
            b.mirror_y(|b| {
                b.paint(GLOW);
                b.extrude_y(&[[-2.9, 9.8], [-1.9, 10.4], [-2.3, 11.1], [-3.5, 10.4]], 1.0, 1.6);
            });
            on_slope(b, [2.6, 9.6], [0.8, 10.6], 0.5, |b| {
                b.paint(GLASS);
                b.plate(Vec3::ZERO, v2(1.1, 2.2), 0.08, 0.04);
                b.mirror_y(|b| glow_strip(b, v3(0.0, 1.6, 0.0), v2(1.4, 0.16), GLOW));
            });
            on_slope(b, [1.6, 7.4], [3.0, 8.4], 0.5, |b| b.mirror_y(|b| glow_strip(b, v3(0.0, 1.0, 0.0), v2(0.9, 0.6), GLOW)));
            antenna(b, v3(-2.6, 0.0, 10.4), 2.0, 0.1);
        }
    });

    if b.fine() {
        b.mirror_y(|b| {
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(GLOW);
                b.beam(v3(1.0, 2.38, 5.75), v3(1.9, 2.48, 4.6), v2(0.5, 0.08), v2(0.4, 0.08));
                b.paint(TEAM);
                b.beam(v3(0.15, 2.3, 6.95), v3(1.0, 2.4, 5.85), v2(1.1, 0.1), v2(1.0, 0.1));
            });
        });
    }
}

// ---- Lancer: light assault bot ---------------------------------------------

pub fn bot_light(b: &mut MeshBuilder, _tech: u8) {
    let stance = Leg {
        joints: &[v3(-0.1, 1.1, 2.95), v3(0.8, 1.15, 1.95), v3(-0.6, 1.15, 1.05), v3(0.1, 1.15, 0.4)],
        girth: &[(v2(0.6, 0.8), v2(0.48, 0.55)), (v2(0.42, 0.48), v2(0.36, 0.4)), (v2(0.3, 0.34), v2(0.38, 0.42))],
        foot: (-0.7, 1.15, 0.42, 0.85),
        bare_from: 2,
    };
    b.mirror_y(|b| leg(b, &stance));

    b.set_turret_pivot(v3(0.0, 0.0, 3.2));
    let (breech, muzzle) = (v3(0.3, 0.0, 4.2), v3(1.4, 0.0, 4.2));
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.frustum_open(v3(-0.2, 0.0, 2.9), v2(1.7, 2.6), v2(1.2, 2.2), 1.85, v2(-0.1, 0.0));
            team_panel(b, v3(-0.5, 0.0, 4.75), v2(0.6, 1.8));
            b.paint(METAL);
            b.beam(breech, muzzle, v2(0.7, 0.26), v2(0.6, 0.2));
        });
        return;
    }

    b.paint(ACCENT);
    if b.fine() {
        b.chamfered_box(v3(-0.1, 0.0, 2.95), v3(0.9, 1.5, 0.6), 0.2);
        b.cylinder_between(v3(-0.1, -1.35, 2.95), v3(-0.1, 1.35, 2.95), 0.3, 0.3, 6);
    } else {
        b.cuboid(v3(-0.1, 0.0, 2.95), v3(0.9, 2.2, 0.6));
    }

    b.with_part(part::TURRET, |b| {
        b.paint(ACCENT);
        b.prism(v3(-0.1, 0.0, 3.2), b.sides(6), 0.55, 0.55, 0.2);
        b.paint(PLATING);
        b.extrude_y_chamfered(&[[-1.1, 3.35], [0.4, 3.35], [0.78, 3.75], [0.72, 4.45], [0.1, 4.8], [-0.9, 4.7], [-1.3, 4.0]], 0.8, 0.24);
        // Twin autocannon through the brow.
        b.mirror_y(|b| {
            b.paint(METAL);
            b.cylinder_between(breech + Vec3::Y * 0.26, muzzle + Vec3::Y * 0.26, 0.11, 0.09, b.sides(6));
            if b.fine() {
                b.paint(GLOW_ORANGE);
                b.cylinder_between(muzzle + v3(-0.04, 0.26, 0.0), muzzle + v3(0.02, 0.26, 0.0), 0.065, 0.065, 6);
            }
        });
        b.paint(if b.fine() { ACCENT } else { GLOW_ORANGE });
        b.block(v3(0.55, -0.5, 4.0), v3(0.95, 0.5, 4.4));
        // Shoulder sensor pods give the bot its hunched silhouette.
        b.mirror_y(|b| {
            b.paint(PLATING);
            if b.fine() {
                b.at(v3(-0.25, 1.22, 0.0), |b| b.loft_z(&chamfered_rect(v2(0.7, 0.42), 0.18), &[Section::new(3.8, 0.85), Section::new(4.35, 1.0), Section::scaled(4.65, 0.7, 0.6)]));
            } else {
                b.frustum(v3(-0.25, 1.22, 3.8), v2(1.3, 0.8), v2(0.9, 0.5), 0.85, v2(0.0, 0.0));
            }
        });
        on_slope(b, [0.1, 4.8], [-0.9, 4.7], 0.45, |b| team_panel(b, Vec3::ZERO, v2(0.6, 1.1)));
        if b.fine() {
            b.paint(ACCENT);
            b.mirror_y(|b| b.block(v3(1.15, 0.1, 4.05), v3(1.32, 0.42, 4.35)));
            on_slope(b, [0.78, 3.75], [0.72, 4.45], 0.3, |b| glow_strip(b, Vec3::ZERO, v2(0.12, 0.9), GLOW));
            b.mirror_y(|b| {
                b.paint(GLASS);
                b.block(v3(0.45, 1.0, 4.05), v3(0.5, 1.45, 4.25));
                glow_strip(b, v3(-0.25, 1.22, 4.65), v2(0.6, 0.12), GLOW);
            });
            // Ammunition drum and exhaust on the back.
            b.paint(ACCENT);
            b.cylinder_between(v3(-1.25, -0.55, 4.05), v3(-1.25, 0.55, 4.05), 0.36, 0.36, 6);
            b.paint(GLOW_ORANGE);
            b.block(v3(-1.64, -0.25, 3.98), v3(-1.6, 0.25, 4.1));
            antenna(b, v3(-0.8, -0.5, 4.65), 0.8, 0.2);
        }
    });
}

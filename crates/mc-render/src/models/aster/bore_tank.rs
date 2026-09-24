//! Arbalest: tech 3 lightning sniper. A low, wide wedge on long tracks under a
//! flat turret carrying a compact Argon Electric Bore: a dark structural spine
//! under segmented ceramic armor, recessed blue coil windows and a broad protected
//! emitter. Capacitor banks
//! hang off the turret's back, dark frames with blue slits. Two spades hinge
//! off the tail and plant when it deploys to fire (`rig::DEPLOY`, folded by the
//! shader about the Trebuchet's rear hinge, so the tail is authored at the same
//! place: x −5.2 at a 1.88 m deck).

use glam::Vec3;

use super::parts::*;
use crate::models::builder::{MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

/// Where the bore ends, as the unit file has it.
const MUZZLE: Vec3 = Vec3::new(11.5, 0.0, 3.6);
/// Where the barrel leaves the breech shroud.
const BREECH: Vec3 = Vec3::new(3.0, 0.0, 3.6);
/// The deck, and the turret race on it. The shader's deploy hinges are the
/// Trebuchet's at this height (`entity.wgsl`, `RIG_DEPLOY`).
const DECK: f32 = 1.88;
const TAIL: f32 = -5.2;

pub fn bore_tank(b: &mut MeshBuilder, _tech: u8) {
    let (rear, front) = (-5.0, 5.3);
    let (inner, outer, track_h) = (2.45, 3.95, 1.25);
    b.set_treads((inner + outer) * 0.5, outer - inner, rear);
    b.set_dust_line(1.5);

    // Long single tracks, low and wide apart.
    b.mirror_y(|b| track(b, rear, front, inner, outer, track_h));

    // Hull: a dark tub between the tracks under a white wedge that runs from a
    // sharp low nose back to a flat deck, overhanging the treads.
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(0.2, 0.0, 0.7),
            v2(11.0, 7.6),
            v2(8.4, 7.2),
            DECK - 0.7,
            v2(-0.9, 0.0),
        );
    } else {
        b.paint(ACCENT);
        b.extrude_y(
            &[[-4.9, 0.35], [4.2, 0.35], [5.6, 0.95], [5.6, 1.15], [-4.9, 1.15]],
            -inner,
            inner,
        );
        // Faceted upper hull: a pointed plan drawn in to a narrow deck, so every side slopes.
        b.paint(PLATING);
        b.loft_z(
            &hull_plan(TAIL, 6.8, outer + 0.1, 2.8),
            &[
                Section::new(1.12, 1.0),
                Section::new(1.4, 1.0),
                Section::scaled(DECK, 0.86, 0.72).shifted(-0.7, 0.0),
            ],
        );
    }
    if b.fine() {
        // Skirts over the upper run, a sensor slit and a team chevron on the glacis.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.block(v3(-4.4, outer + 0.02, 0.62), v3(4.1, outer + 0.14, 1.14));
            b.paint(ACCENT);
            for x in [-1.6, 1.3] {
                b.block(v3(x, outer + 0.1, 0.62), v3(x + 0.12, outer + 0.17, 1.14));
            }
        });
        let glacis = ([6.1, 1.4], [6.1 * 0.86 - 0.7, DECK]);
        on_slope(b, glacis.0, glacis.1, 0.62, |b| {
            glow_strip(b, Vec3::ZERO, v2(0.14, 2.2), GLOW);
        });
        on_slope(b, glacis.0, glacis.1, 0.25, |b| {
            b.paint(PLATING).pattern(pattern::TEAM_BAND);
            b.plate(Vec3::ZERO, v2(0.4, 3.0), 0.05, 0.02);
        });
        // Engine deck: louvres either side of the tail, glowing under load.
        b.mirror_y(|b| vent(b, v3(-3.9, 1.3, DECK), v2(0.9, 1.1), 4, GLOW));
        b.paint(ACCENT);
        b.plate(v3(-3.9, 0.0, DECK), v2(0.9, 0.9), 0.1, 0.04);
    }
    if !b.coarse() {
        // Team flashes on the front fenders, read from above.
        b.mirror_y(|b| team_panel(b, v3(1.6, 2.35, DECK), v2(1.4, 0.8)));
        // Hinge blocks for the spades: hull, so they stay put.
        b.paint(ACCENT);
        b.mirror_y(|b| b.block(v3(TAIL - 0.04, 1.2, 0.95), v3(TAIL + 0.4, 2.2, 1.5)));
    }

    spades(b);

    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_recoil(BREECH, MUZZLE, 0.45);
    b.set_arm_pivot(BREECH);
    b.with_part(part::TURRET, |b| {
        if b.coarse() {
            b.paint(PLATING);
            b.frustum_open(v3(-0.1, 0.0, DECK), v2(6.5, 4.3), v2(3.8, 1.5), 4.2 - DECK, v2(0.4, 0.0));
            team_panel(b, v3(0.3, 0.0, 4.2), v2(2.4, 1.1));
            b.with_limb(rig::ARM_GUN | rig::RECOIL, bore);
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, DECK - 0.03), b.sides(10), 2.3, 2.25, 0.2);
        b.paint(PLATING);
        b.loft_z(&turret_plan(6.8, 4.3), &[
            Section::new(DECK + 0.12, 1.0),
            Section::scaled(3.05, 0.8, 0.7).shifted(-0.4, 0.0),
        ]);
        b.paint(PLATING);
        b.at(v3(0.0, 0.0, 0.0), |b| {
            b.extrude_y_chamfered(&[
                [-2.4, 2.8], [3.1, 2.8], [3.35, 3.45],
                [2.9, 4.05], [-1.4, 3.95], [-2.4, 3.2],
            ], 0.74, 0.2);
        });
        team_panel(b, v3(-0.8, -0.9, 3.08), v2(2.0, 0.6));
        if b.fine() {
            b.paint(GLASS);
            b.chamfered_box(v3(1.5, -1.0, 3.2), v3(0.8, 0.55, 0.4), 0.12);
            antenna(b, v3(-2.5, -1.1, 2.9), 1.3, 0.2);
        }
        if !b.coarse() { capacitors(b); }
        bore_socket(b, BREECH, 0.6, 0.45);
        b.with_limb(rig::ARM_GUN | rig::RECOIL, bore);
    });
}

/// Two capacitor banks on the turret's back: dark frames on a cross-member,
/// blue slits across their rear faces and down their tops.
fn capacitors(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.block(v3(-3.0, -1.7, 2.2), v3(-2.2, 1.7, 2.5));
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.chamfered_box(v3(-2.75, 1.0, 3.0), v3(1.3, 1.25, 1.05), 0.14);
        if b.mid() {
            b.paint(GLOW);
            for i in 0..3 {
                let z = 2.68 + 0.26 * i as f32;
                b.block(v3(-3.43, 0.55, z), v3(-3.38, 1.45, z + 0.08));
            }
        }
        if b.fine() {
            b.paint(GLOW);
            b.block(v3(-3.25, 0.95, 3.52), v3(-2.25, 1.05, 3.555));
            // Feed from the bank forward to the breech.
            b.paint(METAL);
            b.cylinder_between(v3(-2.1, 0.75, 3.15), v3(-1.45, 0.72, 3.15), 0.09, 0.09, 5);
        }
    });
}

/// The compact armored Argon emitter, recoiling as one protected assembly.
fn bore(b: &mut MeshBuilder) {
    armored_bore(b, BREECH, MUZZLE, 0.6);
}

/// The spades: a boom off each hinge block back and down to a toothed blade,
/// authored planted; the shader folds everything behind x −5.25 up against the
/// tail while the unit is packed.
fn spades(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    b.with_deploy(|b| {
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.beam(v3(TAIL - 0.25, 1.7, 1.25), v3(-7.2, 1.75, 0.45), v2(0.55, 0.42), v2(0.5, 0.3));
            b.paint(ACCENT);
            b.extrude_y(
                &[[-6.9, 0.0], [-7.7, 0.0], [-7.95, 0.95], [-7.35, 1.0], [-6.95, 0.55]],
                1.2,
                2.3,
            );
            if b.fine() {
                b.paint(METAL);
                b.cylinder_between(v3(TAIL - 0.25, 1.7, 0.95), v3(-6.8, 1.72, 0.4), 0.08, 0.07, 5);
                b.paint(PLATING);
                b.plate(v3(-7.62, 1.75, 0.98), v2(0.5, 0.9), 0.05, 0.02);
            }
        });
    });
}


/// An exposed hexagonal induction spine between tapered ceramic blades.
/// Every LOD retains the open silhouette and the true, possibly offset muzzle.
pub(super) fn armored_bore(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, radius: f32) {
    if b.coarse() {
        b.paint(PLATING_DARK);
        b.beam(breech, muzzle, v2(radius * 1.9, radius * 1.3), v2(radius * 0.8, radius * 0.7));
        return;
    }
    let length = muzzle.x - breech.x;
    let at = |t: f32| breech.lerp(muzzle, t);
    b.paint(ACCENT);
    b.cylinder_between(at(0.0), at(0.99), radius * 0.48, radius * 0.32, b.sides(6));
    let count = if b.fine() { if radius < 0.5 { 3 } else { 4 } } else { 1 };
    for i in 0..count {
        let t = 0.06 + i as f32 * 0.74 / count as f32;
        b.paint(METAL);
        b.cylinder_between(at(t), at(t + 0.035), radius * 0.9, radius * 0.78, b.sides(6));
        b.paint(GLOW);
        b.cylinder_between(at(t + 0.036), at(t + 0.056), radius * 0.66, radius * 0.66, b.sides(6));
    }
    // Four swept ceramic blades have open gaps, with the energized core visible inside.
    for side in [-1.0, 1.0] {
        for level in [-1.0, 1.0] {
            let start = at(0.01) + v3(0.0, side * radius * 0.75, level * radius * 0.65);
            let end = at(0.9) + v3(0.0, side * radius * 0.5, level * radius * 0.42);
            b.paint(PLATING);
            b.beam(start, end, v2(radius * 0.5, radius * 0.43), v2(radius * 0.23, radius * 0.2));
        }
        // Separated prongs surround a recessed aperture, instead of a square muzzle block.
        b.paint(PLATING_DARK);
        b.beam(at(0.82) + v3(0.0, side * radius * 0.52, 0.0),
               muzzle + v3(0.0, side * radius * 0.7, 0.0),
               v2(radius * 0.45, radius * 1.15), v2(radius * 0.25, radius * 0.7));
    }
    b.paint(ACCENT);
    b.cylinder_between(muzzle - Vec3::X * (length * 0.04), muzzle, radius * 0.4, radius * 0.36, b.sides(6));
    b.paint(GLOW);
    b.cylinder_between(muzzle, muzzle + Vec3::X * 0.025, radius * 0.21, radius * 0.21, b.sides(6));
}

/// A trunnion drum and overlapping receiver sleeve stay seated during recoil.
/// The socket pitches with the barrel, while the barrel slides inside it.
pub(super) fn bore_socket(b: &mut MeshBuilder, pivot: Vec3, radius: f32, recoil: f32) {
    if b.coarse() { return; }
    b.with_limb(rig::ARM_GUN, |b| {
        b.paint(PLATING_DARK);
        b.cylinder_between(pivot - Vec3::Y * radius * 1.05,
            pivot + Vec3::Y * radius * 1.05, radius * 1.05, radius * 1.05, b.sides(6));
        b.paint(ACCENT);
        b.cylinder_between(pivot - Vec3::X * (recoil + radius),
            pivot + Vec3::X * radius * 1.25, radius * 0.8, radius * 0.66, b.sides(6));
    });
}

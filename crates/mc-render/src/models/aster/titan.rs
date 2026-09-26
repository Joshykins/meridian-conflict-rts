//! Behemoth (`aster_t5_titan`, mesh "titan"): the tech 5 siege walker, authored at
//! a quarter of blueprint scale (metres: radius 40, height 120; the unit file builds it at
//! 4x). Two legs that step over buildings, the Tempest rotary cannon on the right arm
//! (six barrels turning out of a fixed shroud; a barrel at the top fires, `barrels`), the AEB-3 Cataclysm
//! Bore on the left, rocket pods on the shoulders with a twin flak turret beside each (not
//! the best air defence it could carry: it leans on escorts), and a hull field thrown from
//! a dorsal projector.
//!
//! Rig (the unit file is authored 1:1 with these numbers; keep them in step):
//! - `part::LOCOMOTION` legs posed by the shader's two-bone IK (`HIP`, `KNEE`, `ANKLE`,
//!   `STRIDE`, `STANCE`, `LIFT`, `CROUCH`, `FOOT`). The sim's stomp and the renderer's
//!   footfalls key off them.
//! - `part::HULL` pelvis and hip caps.
//! - `part::TURRET` waist ring, torso, pauldrons, rocket pods (weapon 0, `POD_MOUTHS`: the
//!   torso's own weapon),
//!   head (`with_head`, idles about `NECK`) and the shield projector (`SHIELD`), turning
//!   about the waist (`WAIST`). The two flak turrets on the pauldrons (weapons 3 and 4,
//!   `FLAK`) are gun houses on it: they turn off the torso and are carried round with it.
//! - The arms are gun houses too (weapons 1 and 2), turning about `SHOULDER`, a point on
//!   the torso's axis at shoulder height: they orbit with the torso, each swinging a
//!   little off it on its own (`sway`), and each pitches on its own about the line
//!   through the shoulders. The gatling's
//!   cluster turns about its bore (`with_spin`); the bore's core kicks back inside its
//!   sleeve on a second house of weapon 2 (`BORE_SLIDE`).
//!
//! Everything above the hips is authored as it was for the first, shorter cut and raised
//! `RAISE` metres onto the longer legs (`b.at`); the `pub` constants ending `_AT` are in
//! model space, for the unit file and the tests.
//!
//! The AEB's light is alive (`pattern::COIL`): each coil band carries its stage along the
//! bore, and the shader breathes it idle, climbs it stage by stage through the 6 s charge,
//! blinds at the shot and lets it cool; the capacitor rings' lugs (`pattern::COIL_TURN`)
//! turn about the bore's axis, faster as it charges. The bore's axis is the gatling's
//! (`Model::spins`) mirrored across the centreline: keep the arms' bores at one height
//! and mirrored in y.
//!
//! Style: light plates over a dark gunmetal frame, grey metal for joints and rams only,
//! nothing lit but the AEB (its coils, capacitor slits and aperture are blue), the orange
//! visor glass round the head (the commander's), and a small gold ring on the shield projector.

use std::f32::consts::TAU;

use glam::{Affine3A, Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

// ---- rig ----------------------------------------------------------------------------

/// How far everything above the hips is raised over where it was first authored.
pub(crate) const RAISE: f32 = 20.0;
#[cfg(test)]
const fn up(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.y, p.z + RAISE)
}

/// Left leg joints at rest (the right is the mirror): hip, knee, hock, ankle. Model space.
/// A reverse-kneed leg: the thigh runs forward and down to the knee, the long shin back
/// and down to the hock, the tarsus forward and down again to the ankle over the foot.
pub(crate) const HIP: Vec3 = Vec3::new(0.0, 17.0, 50.0 + RAISE);
pub(crate) const KNEE: Vec3 = Vec3::new(14.0, 19.0, 47.0);
pub(crate) const HOCK: Vec3 = Vec3::new(-11.0, 20.5, 25.0);
/// The feet stand wider than the hips: the legs splay a little, planted like a crane's.
pub(crate) const ANKLE: Vec3 = Vec3::new(0.0, 22.0, 8.0);
/// How much of the leg's swing the tarsus leans with (`MeshBuilder::set_hock`).
const HOCK_FOLLOW: f32 = 0.55;
/// Ground to a full cycle (a power of two), share of it a foot is down, how high a
/// foot lifts, how far the hips settle in full stride.
pub(crate) const STRIDE: f32 = 64.0;
pub(crate) const STANCE: f32 = 0.6;
pub(crate) const LIFT: f32 = 18.0;
pub(crate) const CROUCH: f32 = 6.0;
/// The sole: metres behind the ankle, ahead of it, across (`FOOT_PLAN` is its outline).
pub(crate) const FOOT: (f32, f32, f32) = (-12.0, 15.0, 16.0);
/// The sole's outline round the ankle (x ahead, y out), its corners cut at 45 degrees.
pub(crate) const FOOT_PLAN: [[f32; 2]; 8] = [
    [15.0, -4.5],
    [15.0, 4.5],
    [11.5, 8.0],
    [-8.5, 8.0],
    [-12.0, 4.5],
    [-12.0, -4.5],
    [-8.5, -8.0],
    [11.5, -8.0],
];
/// The waist ring the torso turns on.
pub(crate) const WAIST: f32 = 57.0;
/// Both arm houses' pivot: on the torso axis, at shoulder height (the weapons' `pivot`).
pub(crate) const SHOULDER: Vec3 = Vec3::new(0.0, 0.0, 78.0);
/// The bore core's own house: weapon 1 again, a hair off `SHOULDER` so it takes a slot
/// of its own and can kick back (`BORE_RECOIL`) while the arm stays put.
const BORE_SLIDE: Vec3 = Vec3::new(0.0, 0.0, 78.01);
const BORE_RECOIL: f32 = 3.0;
/// Arms: centre line off the axis, and the height of the guns' bores: slung under the
/// shoulders on upper arms (`shoulder`), clear of the hip cowls.
const ARM_Y: f32 = 36.0;
const ARM_Z: f32 = 54.0;
/// Where the Tempest's spent cases leave it (authored, in the raised frame): the port in the
/// body's outboard flank, 54 m behind the muzzle. The sim throws them from here: the unit
/// file's `sabot.port` is this point at the built size.
pub(crate) const EJECT: Vec3 = Vec3::new(10.0, -ARM_Y - 8.6, ARM_Z + 1.0);
/// The unit file's muzzles: the rail cluster's hub (right arm) and the bore's aperture (left).
pub(crate) const GATLING_MUZZLE: Vec3 = Vec3::new(64.0, -ARM_Y, ARM_Z);
pub(crate) const BORE_MUZZLE: Vec3 = Vec3::new(70.0, ARM_Y, ARM_Z);
/// The left rocket pod's face: the middle of its cell face, and how far the pod is raked
/// nose-up about it (radians). The right pod is the mirror.
pub(crate) const POD_FACE: Vec3 = Vec3::new(0.0, 20.0, 97.5);
pub(crate) const POD_PITCH: f32 = 0.30;
/// Its cells about the face's middle before the rake: across (y) and up the face.
const POD_COLS: [f32; 3] = [-4.4, 0.0, 4.4];
const POD_ROWS: [f32; 2] = [-2.2, 2.2];
/// The six cell mouths of the left pod after the rake (x, y, z), all the unit file's
/// muzzles: `POD_FACE` + (-sin, cos) of the rake times the row, + the column.
#[cfg(test)]
pub(crate) const POD_MOUTHS: [(f32, f32, f32); 6] = [
    (0.6501, 15.6, 95.3983),
    (0.6501, 20.0, 95.3983),
    (0.6501, 24.4, 95.3983),
    (-0.6501, 15.6, 99.6017),
    (-0.6501, 20.0, 99.6017),
    (-0.6501, 24.4, 99.6017),
];
/// In model space: the arm pivot, the muzzles, the neck and the shield projector.
#[cfg(test)]
pub(crate) const SHOULDER_AT: Vec3 = up(SHOULDER);
#[cfg(test)]
pub(crate) const GATLING_MUZZLE_AT: Vec3 = up(GATLING_MUZZLE);
#[cfg(test)]
pub(crate) const EJECT_AT: Vec3 = up(EJECT);
#[cfg(test)]
pub(crate) const BORE_MUZZLE_AT: Vec3 = up(BORE_MUZZLE);
/// Flak turret pivots on the pauldrons, weapons 3 and 4 (left, right); each gun's muzzle
/// is its pivot plus `FLAK_REACH` of x, its two barrels `FLAK_GAP` either side.
pub(crate) const FLAK: [Vec3; 2] = [Vec3::new(-6.0, 33.0, 94.0), Vec3::new(-6.0, -33.0, 94.0)];
pub(crate) const FLAK_REACH: f32 = 10.0;
pub(crate) const FLAK_GAP: f32 = 0.85;
/// Where the head turns (x, centreline, z).
pub(crate) const NECK: Vec3 = Vec3::new(9.0, 0.0, 87.0);
/// The shield projector's tip, on the back of the torso.
pub(crate) const SHIELD: Vec3 = Vec3::new(-20.0, 0.0, 97.5);

const X: Vec3 = Vec3::X;
const Y: Vec3 = Vec3::Y;

pub(crate) fn titan(b: &mut MeshBuilder, tech: u8) {
    legs_rig(b);
    b.set_dust_line(16.0);
    if b.coarse() {
        b.mirror_y(coarse_leg);
    } else {
        b.mirror_y(leg);
    }
    b.at(Vec3::Z * RAISE, |b| upper(b, tech));
}

/// The walking rig. The builder scales the joints to the blueprint but takes the stride
/// as given: built at 4x the stride must be 4x too (still a power of two), or the feet
/// take tiny steps under long legs and the sim's footfall pace no longer matches.
fn legs_rig(b: &mut MeshBuilder) {
    b.set_legs(HIP, KNEE, ANKLE, STRIDE, STANCE, LIFT);
    let scale = b.legs().map_or(1.0, |l| l.hip[1] / HIP.y);
    let stride = STRIDE * scale.log2().round().exp2();
    b.set_legs(HIP, KNEE, ANKLE, stride, STANCE, LIFT);
    b.set_hock(HOCK, HOCK_FOLLOW);
    b.set_walk_crouch(CROUCH);
    b.set_foot(FOOT.0, FOOT.1, FOOT.2);
}

/// Everything above the hips, in the raised frame.
fn upper(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, WAIST));
    b.set_shield_emitter(SHIELD);
    if b.coarse() {
        coarse(b);
        return;
    }
    // The arms first, at every level, so their houses take the same slots (0 and 1) in
    // each: the coarse level draws no others.
    gatling_arm(b);
    bore_arm(b);
    pelvis(b);
    b.with_part(part::TURRET, torso);
}

/// From far off: a bar of a torso, the pods' cell faces and the two arms as bars tapering
/// out to their muzzles. (The legs are drawn by `titan`.)
fn coarse(b: &mut MeshBuilder) {
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        tri_bar(b, v3(0.0, 0.0, 56.0), v3(0.0, 0.0, 92.0), 13.0, 17.0);
        // The pods' cell faces, raked up as the pods are.
        b.mirror_y(|b| {
            let f = POD_FACE;
            b.pitched(f, POD_PITCH, |b| {
                b.face(&[
                    v3(0.0, -7.0, -4.0),
                    v3(0.0, 7.0, -4.0),
                    v3(0.0, 7.0, 4.0),
                    v3(0.0, -7.0, 4.0),
                ])
            })
        });
    });
    b.with_house(1, SHOULDER, 0.0, |b| {
        b.with_recoil(|b| {
            b.paint(METAL);
            tri_bar(b, v3(-8.0, -ARM_Y, ARM_Z + 3.0), GATLING_MUZZLE, 7.0, 0.3);
        })
    });
    b.with_house(2, SHOULDER, 0.0, |b| {
        b.with_recoil(|b| {
            b.paint(PLATING_DARK);
            tri_bar(b, v3(-14.0, ARM_Y, ARM_Z + 3.0), BORE_MUZZLE, 7.5, 0.3);
        })
    });
}

// ---- legs ---------------------------------------------------------------------------

/// The left leg: a heavy reverse-kneed leg, each bone a dark armoured frame under light
/// plates, rammed down its back, every piece riding its bone so it moves with the IK.
fn leg(b: &mut MeshBuilder) {
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, thigh);
        b.with_limb(rig::SHIN, shin);
        b.with_limb(rig::TARSUS, tarsus);
        b.with_limb(rig::FOOT, foot);
    });
}

/// From far off: each bone an open three-sided bar, still posed by the rig (the foot is
/// the tarsus's end: a speck at this range).
fn coarse_leg(b: &mut MeshBuilder) {
    let bar = |b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32| tri_bar(b, a, c, w, w * 0.8);
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING);
        b.with_limb(rig::THIGH, |b| bar(b, HIP + Vec3::Z * 6.0, KNEE, 9.0));
        b.with_limb(rig::SHIN, |b| bar(b, KNEE, HOCK, 8.0));
        b.with_limb(rig::TARSUS, |b| bar(b, HOCK, ANKLE, 7.0));
    });
}

/// A far-off bar from `a` to `c`: three sides, open ends, `w0` and `w1` its size at each.
fn tri_bar(b: &mut MeshBuilder, a: Vec3, c: Vec3, w0: f32, w1: f32) {
    let (_, across, face) = bone_frame(a, c);
    let ring = |p: Vec3, w: f32| {
        vec![
            p + face * w,
            p + across * w - face * (w * 0.6),
            p - across * w - face * (w * 0.6),
        ]
    };
    b.loft(&[ring(a, w0), ring(c, w1)], false, false);
}

/// A bone's frame from `a` to `c`: along it, across it (as near y as it can be), and its
/// face (square to both, the side toward +x).
fn bone_frame(a: Vec3, c: Vec3) -> (Vec3, Vec3, Vec3) {
    let along = (c - a).normalize();
    let across = (Y - along * Y.dot(along)).normalize();
    let face = along.cross(across);
    (along, across, if face.x < 0.0 { -face } else { face })
}

/// A limb's cross-section at `p`: `w` either side across, `front` out along the face and
/// `back` behind it, corners cut `cut`.
fn section(
    p: Vec3,
    across: Vec3,
    face: Vec3,
    w: f32,
    front: f32,
    back: f32,
    cut: f32,
) -> Vec<Vec3> {
    let cut = cut.max(0.01);
    [
        (w - cut, front),
        (w, front - cut),
        (w, -back + cut),
        (w - cut, -back),
        (-w + cut, -back),
        (-w, -back + cut),
        (-w, front - cut),
        (-w + cut, front),
    ]
    .iter()
    .map(|&(u, v)| p + across * u + face * v)
    .collect()
}

/// A limb lofted down the bone from `a` to `c` through sections `(t along it, w, front,
/// back, cut)`.
fn limb(b: &mut MeshBuilder, a: Vec3, c: Vec3, sections: &[(f32, f32, f32, f32, f32)]) {
    let (_, across, face) = bone_frame(a, c);
    let rings: Vec<Vec<Vec3>> = sections
        .iter()
        .map(|&(t, w, f, k, cut)| section(a.lerp(c, t), across, face, w, f, k, cut))
        .collect();
    b.loft(&rings, true, true);
}

/// A joint drum across the leg at `at`: a dark drum, metal hubs either end, bolt heads round
/// the outer hub up close.
fn joint(b: &mut MeshBuilder, at: Vec3, half: f32, r: f32) {
    let n = b.sides(16);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(at - Y * half, at + Y * half, r, r, n);
    b.paint(METAL);
    b.cylinder_between(at + Y * half, at + Y * (half + 0.9), r * 0.62, r * 0.55, n);
    b.cylinder_between(at - Y * (half + 0.7), at - Y * half, r * 0.5, r * 0.62, n);
    if b.fine() {
        for i in 0..10 {
            let a = i as f32 * TAU / 10.0;
            b.cuboid(
                at + v3(a.cos() * r * 0.8, half + 0.2, a.sin() * r * 0.8),
                v3(0.9, 0.6, 0.9),
            );
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            at + Y * (half + 0.9),
            at + Y * (half + 1.5),
            r * 0.3,
            r * 0.3,
            10,
        );
    }
}

/// A plate laid on a limb from `t0` to `t1` of the bone `a`-`c`, on the side `out` of it
/// (`face`, `-face`, `across` in the bone's frame), `lift` off the bone's line and `shift`
/// across it, `(half width, thickness)` at each end.
fn limb_plate(
    b: &mut MeshBuilder,
    (a, c): (Vec3, Vec3),
    (t0, t1): (f32, f32),
    out: Vec3,
    (lift0, lift1): (f32, f32),
    shift: Vec3,
    at_a: (f32, f32),
    at_c: (f32, f32),
) {
    armour(
        b,
        a.lerp(c, t0) + out * lift0 + shift,
        a.lerp(c, t1) + out * lift1 + shift,
        out,
        at_a,
        at_c,
    );
}

/// The thigh: the great hip actuator, a deep armoured thigh running forward and down to
/// the knee, twin rams along its back, the knee cop over the joint's front.
fn thigh(b: &mut MeshBuilder) {
    let (h, k) = (HIP, KNEE);
    let (along, across, face) = bone_frame(h, k);
    let fine = b.fine();
    let bone = (h, k);
    joint(b, h - Y * 0.5, 9.2, 9.6);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    limb(
        b,
        h,
        k,
        &[
            (0.0, 7.4, 8.4, 8.4, 3.0),
            (0.3, 8.2, 9.2, 7.6, 3.2),
            (0.78, 7.4, 8.0, 6.4, 3.0),
            (1.02, 6.2, 6.4, 5.4, 2.6),
        ],
    );
    // Light armour over its front and flanks, the dark frame showing in the seams.
    b.paint(PLATING);
    limb_plate(
        b,
        bone,
        (0.06, 0.52),
        face,
        (9.2, 9.1),
        Vec3::ZERO,
        (7.4, 1.8),
        (7.6, 1.8),
    );
    limb_plate(
        b,
        bone,
        (0.56, 0.94),
        face,
        (8.9, 7.3),
        Vec3::ZERO,
        (7.2, 1.7),
        (6.0, 1.5),
    );
    for s in [1.0f32, -1.0] {
        let side = across * s;
        let (w, t) = if s > 0.0 { (7.0, 1.6) } else { (6.0, 1.2) };
        limb_plate(
            b,
            bone,
            (0.1, 0.9),
            side,
            (8.2, 7.2),
            face * 1.2,
            (w, t),
            (w - 1.6, t * 0.8),
        );
    }
    // The rams: from the hip housing down the thigh's back to the knee.
    for s in [-1.0f32, 1.0] {
        let off = across * (s * 3.8);
        ram(
            b,
            h.lerp(k, 0.1) - face * 8.6 + off,
            h.lerp(k, 0.86) - face * 6.6 + off,
            2.2,
            1.4,
        );
    }
    // The knee cop: a heavy plate curled over the joint's front.
    b.paint(PLATING);
    armour(
        b,
        k + v3(6.0, 0.0, 6.5),
        k + v3(9.0, 0.0, -1.0),
        X,
        (7.6, 2.4),
        (6.6, 2.0),
    );
    armour(
        b,
        k + v3(9.0, 0.0, -1.0),
        k + v3(7.2, 0.0, -7.0),
        X + Vec3::NEG_Z * 0.6,
        (6.6, 2.0),
        (5.2, 1.6),
    );
    if fine {
        // Lames over the upper plate, a ridge down its face, the owner's colour down the
        // outer plate.
        b.paint(PLATING).pattern(pattern::PLAIN);
        for t in [0.12, 0.3] {
            limb_plate(
                b,
                bone,
                (t, t + 0.14),
                face,
                (11.0, 10.8),
                Vec3::ZERO,
                (6.4, 0.9),
                (6.4, 0.9),
            );
        }
        b.beam(
            h.lerp(k, 0.58) + face * 10.7,
            h.lerp(k, 0.9) + face * 8.9,
            v2(1.6, 0.8),
            v2(1.4, 0.7),
        );
        b.paint(TEAM);
        limb_plate(
            b,
            bone,
            (0.2, 0.8),
            across,
            (9.8, 9.0),
            face * 3.0,
            (1.2, 0.3),
            (1.1, 0.3),
        );
        // Cable runs down the inside, clamped.
        b.paint(METAL).pattern(pattern::PLAIN);
        for dz in [-1.4f32, 1.4] {
            let off = -across * 7.6 - face * (3.0 + dz);
            b.cylinder_between(h.lerp(k, 0.02) + off, h.lerp(k, 0.9) + off, 0.55, 0.55, 6);
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for t in [0.3, 0.6] {
            b.cuboid(h.lerp(k, t) - across * 7.7 - face * 3.0, v3(1.4, 1.2, 4.6));
        }
        // A service hatch low on the outer flank, and a vent grille in the rear seam.
        let at = h.lerp(k, 0.55) + across * 9.1 - face * 3.6;
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.beam(
            at - along * 2.2,
            at + along * 2.2,
            v2(0.5, 4.0),
            v2(0.5, 4.0),
        );
        b.paint(METAL);
        for i in 0..4 {
            let p = h.lerp(k, 0.28 + 0.1 * i as f32) - face * 8.7;
            b.beam(
                p - across * 1.2,
                p + across * 1.2,
                v2(0.4, 0.8),
                v2(0.4, 0.8),
            );
        }
    }
}

/// The shin: the long heavy bone from the knee back and down to the hock. A greave under
/// its front, a calf plate over its back with a pair of great rams on it, ribs round the frame.
fn shin(b: &mut MeshBuilder) {
    let (k, hk) = (KNEE, HOCK);
    let (along, across, face) = bone_frame(k, hk);
    let fine = b.fine();
    let bone = (k, hk);
    joint(b, k, 8.2, 7.6);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    limb(
        b,
        k,
        hk,
        &[
            (-0.02, 6.6, 6.8, 7.2, 2.6),
            (0.16, 7.2, 7.4, 7.8, 2.8),
            (0.6, 6.4, 6.2, 6.6, 2.6),
            (1.0, 5.6, 5.4, 5.8, 2.2),
        ],
    );
    b.paint(PLATING);
    // The greave under its front, the calf plate over its back.
    limb_plate(
        b,
        bone,
        (0.1, 0.92),
        face,
        (7.4, 5.5),
        Vec3::ZERO,
        (6.8, 1.6),
        (5.2, 1.3),
    );
    limb_plate(
        b,
        bone,
        (0.08, 0.5),
        -face,
        (7.8, 6.8),
        Vec3::ZERO,
        (7.2, 1.8),
        (6.6, 1.6),
    );
    limb_plate(
        b,
        bone,
        (0.54, 0.95),
        -face,
        (6.7, 5.9),
        Vec3::ZERO,
        (6.4, 1.5),
        (5.6, 1.3),
    );
    for s in [1.0f32, -1.0] {
        limb_plate(
            b,
            bone,
            (0.12, 0.88),
            across * s,
            (7.2, 5.8),
            face * 1.0,
            (5.2, 1.3),
            (4.0, 1.0),
        );
    }
    // The great rams over the calf, knee to hock.
    for s in [-1.0f32, 1.0] {
        let off = across * (s * 3.6);
        ram(
            b,
            k.lerp(hk, 0.14) - face * 9.9 + off,
            k.lerp(hk, 0.9) - face * 8.0 + off,
            2.1,
            1.35,
        );
    }
    if fine {
        // Ribs round the frame between the plates, the owner's colour down the outside,
        // a vent in the calf, cable runs down the inside.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        {
            let t = 0.52f32;
            let p = k.lerp(hk, t);
            b.loft(
                &[
                    section(p - along * 0.6, across, face, 7.1, 7.0, 7.4, 2.8),
                    section(p + along * 0.6, across, face, 7.1, 7.0, 7.4, 2.8),
                ],
                true,
                true,
            );
        }
        b.paint(TEAM);
        limb_plate(
            b,
            bone,
            (0.2, 0.8),
            across,
            (8.5, 6.9),
            face * 1.0,
            (1.1, 0.3),
            (1.0, 0.3),
        );
        b.paint(PLATING).pattern(pattern::PLAIN);
        for t in [0.2, 0.36] {
            limb_plate(
                b,
                bone,
                (t, t + 0.12),
                -face,
                (9.6, 9.3),
                Vec3::ZERO,
                (2.4, 0.8),
                (2.4, 0.8),
            );
        }
        b.paint(METAL).pattern(pattern::PLAIN);
        for dz in [-1.2f32, 1.2] {
            let off = -across * 6.9 + face * (1.0 + dz);
            b.cylinder_between(k.lerp(hk, 0.05) + off, k.lerp(hk, 0.95) + off, 0.5, 0.5, 6);
        }
        // Rod-end clevises where the rams bear on the frame.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for t in [0.12, 0.92] {
            b.cuboid(
                k.lerp(hk, t) - face * (8.0 + 1.6 * (1.0 - t)),
                v3(3.0, 11.0, 2.6),
            );
        }
    }
}

/// The tarsus: from the hock forward and down to the ankle. The hock's drum and its heel
/// cap, a shin plate down the front, a shock ram down the back.
fn tarsus(b: &mut MeshBuilder) {
    let (hk, a) = (HOCK, ANKLE);
    let (along, across, face) = bone_frame(hk, a);
    let fine = b.fine();
    let bone = (hk, a);
    joint(b, hk, 7.4, 6.4);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    limb(
        b,
        hk,
        a,
        &[
            (-0.06, 5.8, 5.4, 5.6, 2.2),
            (0.45, 5.2, 4.6, 4.8, 2.0),
            (0.92, 5.6, 5.0, 5.0, 2.0),
        ],
    );
    // The heel cap: an armoured hood over the back of the hock.
    b.paint(PLATING);
    armour(
        b,
        hk + v3(1.0, 0.0, 6.5),
        hk + v3(-7.0, 0.0, 2.0),
        v3(-0.3, 0.0, 1.0).normalize(),
        (7.8, 2.2),
        (7.0, 2.0),
    );
    armour(
        b,
        hk + v3(-7.0, 0.0, 2.0),
        hk + v3(-6.0, 0.0, -5.5),
        -X,
        (7.0, 2.0),
        (5.6, 1.6),
    );
    // The shin plate down its front, a flank plate each side.
    limb_plate(
        b,
        bone,
        (0.12, 0.84),
        face,
        (5.0, 4.8),
        Vec3::ZERO,
        (5.6, 1.5),
        (5.0, 1.3),
    );
    for s in [1.0f32, -1.0] {
        limb_plate(
            b,
            bone,
            (0.2, 0.8),
            across * s,
            (5.6, 5.4),
            face * 0.5,
            (3.6, 1.0),
            (3.2, 0.9),
        );
    }
    ram(
        b,
        hk.lerp(a, 0.08) - face * 6.6,
        hk.lerp(a, 0.84) - face * 5.4,
        1.9,
        1.2,
    );
    if fine {
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.beam(
            hk.lerp(a, 0.2) + face * 6.4,
            hk.lerp(a, 0.75) + face * 6.2,
            v2(1.4, 0.7),
            v2(1.2, 0.6),
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        let p = hk.lerp(a, 0.5);
        b.loft(
            &[
                section(p - along * 0.5, across, face, 5.6, 5.1, 5.2, 2.0),
                section(p + along * 0.5, across, face, 5.6, 5.1, 5.2, 2.0),
            ],
            true,
            true,
        );
        b.paint(METAL);
        for i in 0..6 {
            let a = i as f32 * TAU / 6.0;
            b.cuboid(
                hk + v3(-7.0 + a.cos() * 1.6, 0.0, 2.0 + a.sin() * 1.6) + Y * 7.6,
                v3(0.7, 0.4, 0.7),
            );
        }
    }
}

/// A great flat-planted foot: a dark armoured sole pad the shape of `FOOT_PLAN`, a light
/// plated shell over it rising to the ankle, layered prow plates over the front, a heel
/// block with its ram, guards either side of the ankle drum. No toes.
fn foot(b: &mut MeshBuilder) {
    let a = ANKLE;
    let fine = b.fine();
    let plan: Vec<[f32; 2]> = FOOT_PLAN.to_vec();
    b.at(v3(a.x, a.y, 0.0), |b| {
        b.paint(TREAD);
        b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(2.2, 1.0)]);
        // The shell: sloped all round, drawn in toward the ankle.
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(2.2, 0.97),
                Section::new(4.6, 0.95),
                Section::scaled(9.2, 0.6, 0.74).shifted(-0.4, 0.0),
            ],
        );
        if fine {
            // A dark band of plate round the shell's foot, bolted to the sole.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(&plan, &[Section::new(2.2, 1.005), Section::new(3.2, 1.0)]);
        }
    });
    // The prow: two thick armour plates lapped over the front of the shell.
    for (front, rear, w, z) in [
        ([a.x + 15.0, 3.4], [a.x + 7.4, 7.6], 13.2, 0.0),
        ([a.x + 9.4, 6.8], [a.x + 3.2, 9.6], 11.0, 0.0),
    ] {
        on_slope(b, front, rear, 0.5, |b| {
            b.paint(PLATING);
            let len = (front[0] - rear[0]).hypot(front[1] - rear[1]) + 1.2;
            b.plate(v3(0.0, a.y, z), v2(len, w), 1.3, 0.5);
        });
    }
    // The ankle block the drum sits in, and the drum.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(v3(a.x - 0.5, a.y, 8.4), v3(10.0, 11.0, 4.0), 1.8);
    joint(b, a, 6.0, 4.6);
    // The heel: a block behind, a spur of armour off it, a ram up from it toward the tarsus.
    b.paint(PLATING);
    b.chamfered_box(v3(a.x - 9.8, a.y, 4.6), v3(5.0, 11.0, 4.8), 1.5);
    b.beam(
        v3(a.x - 11.0, a.y, 6.4),
        v3(a.x - 15.0, a.y, 3.0),
        v2(8.0, 3.6),
        v2(5.0, 1.6),
    );
    ram(
        b,
        v3(a.x - 9.0, a.y, 7.0),
        v3(a.x - 4.8, a.y, 13.0),
        1.9,
        1.2,
    );
    if fine {
        // Ankle guards either side of the drum, bolted to the shell.
        for s in [-1.0f32, 1.0] {
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [a.x - 6.0, 4.0],
                    [a.x + 6.2, 4.0],
                    [a.x + 4.0, 11.8],
                    [a.x - 4.2, 11.8],
                ],
                a.y + s * 6.9,
                a.y + s * 8.1,
            );
        }
        b.paint(METAL);
        for (x, y) in [
            (-8.5, 6.2),
            (-4.0, 7.6),
            (4.0, 7.6),
            (9.0, 7.6),
            (12.8, 5.0),
        ] {
            for s in [-1.0f32, 1.0] {
                b.cuboid(v3(a.x + x, a.y + s * (y + 0.1), 1.3), v3(1.1, 0.5, 1.1));
            }
        }
        // Grip bars across the sole's rim, front and back, and hydraulic lines down the
        // shell's sides into the ankle block.
        b.paint(TREAD);
        for x in [-11.4, 14.4] {
            b.block(
                v3(a.x + x - 0.5, a.y - 4.0, 0.0),
                v3(a.x + x + 0.5, a.y + 4.0, 2.8),
            );
        }
        b.paint(METAL);
        for s in [-1.0f32, 1.0] {
            b.cylinder_between(
                v3(a.x + 7.0, a.y + s * 6.2, 4.6),
                v3(a.x + 2.0, a.y + s * 5.4, 8.6),
                0.45,
                0.45,
                6,
            );
            b.cylinder_between(
                v3(a.x - 7.0, a.y + s * 6.2, 4.8),
                v3(a.x - 3.0, a.y + s * 5.4, 8.6),
                0.45,
                0.45,
                6,
            );
        }
    }
}

/// A sculpted armour plate from `a` to `c`, facing `out`: flat on the limb, bevelled to a
/// ridged face, `(half width, thickness)` at each end.
fn armour(b: &mut MeshBuilder, a: Vec3, c: Vec3, out: Vec3, at_a: (f32, f32), at_c: (f32, f32)) {
    let axis = (c - a).normalize();
    let side = out.cross(axis).normalize();
    let face = axis.cross(side).normalize() * out.dot(axis.cross(side)).signum();
    let ring = |p: Vec3, (w, t): (f32, f32)| -> Vec<Vec3> {
        [
            (-w, 0.0),
            (w, 0.0),
            (w, 0.45 * t),
            (0.5 * w, t),
            (-0.5 * w, t),
            (-w, 0.45 * t),
        ]
        .iter()
        .map(|&(s, u)| p + side * s + face * u)
        .collect()
    };
    b.loft(&[ring(a, at_a), ring(c, at_c)], true, true);
}

/// A hydraulic ram from `top` to `bottom`: a dark cylinder over the upper half, a bright
/// rod out of it.
fn ram(b: &mut MeshBuilder, top: Vec3, bottom: Vec3, barrel: f32, rod: f32) {
    let sides = b.sides(8);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(top, top.lerp(bottom, 0.56), barrel, barrel * 0.94, sides);
    b.paint(METAL);
    b.cylinder_between(top.lerp(bottom, 0.5), bottom, rod, rod, sides);
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            top - (bottom - top).normalize() * 0.6,
            top + (bottom - top).normalize() * 0.9,
            barrel * 1.2,
            barrel * 1.2,
            8,
        );
    }
}

// ---- pelvis and flak ----------------------------------------------------------------

/// The pelvis: a deep dark armoured block between the hips, light tassets fore and aft,
/// a keel under it, and a heavy cowl over each hip actuator for the thigh to swing in.
fn pelvis(b: &mut MeshBuilder) {
    let fine = b.fine();
    let hip = HIP.z - RAISE;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(v3(0.0, 0.0, hip), v3(24.0, 18.0, 16.0), 5.5);
    b.frustum(
        v3(0.0, 0.0, hip - 13.0),
        v2(9.0, 8.0),
        v2(14.0, 12.0),
        5.2,
        Vec2::ZERO,
    );
    // Tassets fore and aft, narrow enough for the thighs to swing past.
    b.paint(PLATING);
    for s in [1.0f32, -1.0] {
        b.with(Affine3A::from_scale(v3(s, 1.0, 1.0)), |b| {
            b.extrude_y_chamfered(
                &[
                    [9.0, hip - 10.5],
                    [13.8, hip - 6.5],
                    [13.8, hip + 4.0],
                    [10.4, hip + 7.5],
                    [7.0, hip + 7.5],
                    [7.0, hip - 10.5],
                ],
                7.8,
                if fine { 1.5 } else { 0.0 },
            );
        });
    }
    b.mirror_y(|b| {
        let fine = b.fine();
        // The cowl: an armoured hood over the top of the hip drum, sloped fore and aft.
        let y = HIP.y;
        b.paint(PLATING);
        b.at(v3(0.0, y, 0.0), |b| {
            b.extrude_y_chamfered(
                &[
                    [-13.4, hip - 1.5],
                    [-12.6, hip + 5.5],
                    [-7.4, hip + 11.2],
                    [6.6, hip + 11.6],
                    [12.8, hip + 6.4],
                    [13.6, hip - 1.0],
                    [0.0, hip + 4.0],
                ],
                11.0,
                if fine { 1.8 } else { 0.0 },
            )
        });
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(-11.0, y + 10.9, hip - 0.5),
            v3(10.5, y + 11.6, hip + 7.0),
        );
        if fine {
            // A second plate lapped over the cowl's crown, a railed walkway along it, bolt
            // heads down its outer lip.
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.at(v3(0.0, y, 0.0), |b| {
                b.extrude_y_chamfered(
                    &[
                        [-6.0, hip + 11.0],
                        [5.6, hip + 11.4],
                        [4.6, hip + 12.4],
                        [-5.2, hip + 12.1],
                    ],
                    8.4,
                    1.0,
                )
            });
            b.paint(PLATING).pattern(pattern::WALKWAY);
            b.plate(v3(-0.5, y - 5.0, hip + 11.3), v2(10.0, 2.4), 0.12, 0.04);
            railing(
                b,
                v3(-6.0, y - 6.4, hip + 11.4),
                v3(5.0, y - 6.4, hip + 11.4),
                1.3,
            );
            b.paint(METAL);
            for i in 0..6 {
                b.cuboid(
                    v3(-10.0 + 4.0 * i as f32, y + 11.7, hip + 1.0),
                    v3(0.9, 0.4, 0.9),
                );
            }
            // Hazard band on the cowl's front edge.
            b.paint(PLATING).pattern(pattern::HAZARD);
            b.block(v3(12.9, y - 8.0, hip + 1.0), v3(13.3, y + 8.0, hip + 3.0));
        }
    });
    if fine {
        pelvis_detail(b);
    }
}

/// A twin flak turret on a pauldron (`weapon`, turning about `pivot`): a low faceted
/// turret on a ring, two plain gun barrels with flash hiders out of a dark mantlet.
/// Authored facing the nose.
fn flak(b: &mut MeshBuilder, weapon: usize, pivot: Vec3) {
    b.with_house(weapon, pivot, 0.6, |b| {
        b.at(pivot, |b| {
            let fine = b.fine();
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.prism(v3(0.0, 0.0, -3.0), b.sides(10), 4.1, 3.9, 0.9);
            b.paint(PLATING);
            b.at(v3(-1.0, 0.0, 0.0), |b| {
                b.loft_z(
                    &turret_plan(8.4, 7.4),
                    &[
                        Section::new(-2.2, 0.9),
                        Section::new(0.9, 1.0),
                        Section::scaled(2.8, 0.66, 0.7).shifted(-0.8, 0.0),
                    ],
                )
            });
            if fine {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.cuboid(v3(-2.6, 0.0, 3.0), v3(2.4, 2.2, 0.5));
                b.paint(METAL);
                b.cuboid(v3(-0.6, 2.2, 2.5), v3(1.4, 0.9, 0.9));
            }
            b.with_recoil(|b| {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.chamfered_box(v3(2.4, 0.0, 0.0), v3(3.4, 3.8, 2.6), 0.6);
                if fine {
                    for s in [-1.0f32, 1.0] {
                        let y = s * FLAK_GAP;
                        b.paint(METAL);
                        b.cylinder_between(
                            v3(2.8, y, 0.0),
                            v3(FLAK_REACH - 1.2, y, 0.0),
                            0.34,
                            0.3,
                            8,
                        );
                        b.paint(ACCENT).pattern(pattern::PLAIN);
                        b.cylinder_between(v3(4.2, y, 0.0), v3(5.4, y, 0.0), 0.46, 0.46, 8);
                        b.cylinder_between(
                            v3(FLAK_REACH - 1.2, y, 0.0),
                            v3(FLAK_REACH, y, 0.0),
                            0.44,
                            0.4,
                            8,
                        );
                    }
                } else {
                    b.paint(METAL);
                    b.cylinder_between(v3(2.8, 0.0, 0.0), v3(FLAK_REACH, 0.0, 0.0), 0.7, 0.6, 6);
                }
            });
        });
    });
}

// ---- torso --------------------------------------------------------------------------

/// The chest's dark core under its armour: plan (half length, half width, corner cut) and
/// sections up the body (z, scale along x, across, shift forward).
const CHEST_PLAN: (f32, f32, f32) = (13.5, 17.0, 6.5);
const CHEST_SECTIONS: [(f32, f32, f32, f32); 5] = [
    (61.0, 0.62, 0.6, 0.5),
    (67.0, 0.9, 0.86, 0.6),
    (77.0, 1.0, 1.0, 0.0),
    (85.0, 0.86, 0.9, -0.8),
    (90.0, 0.64, 0.66, -2.2),
];
/// The pauldrons' deck: the plan's middle (x, y), half extents, and the deck's height (the
/// flak turrets and the pods' pylons stand on it).
const DECK: (f32, f32, f32, f32, f32) = (-3.0, 29.0, 11.0, 14.0, 90.4);

/// One section of a V-fronted armour plate at height `z`: its ridge on the centreline at
/// `ridge` (x), its edges `w` either side and `back` behind the ridge, `t` thick. `side` +1
/// faces forward, -1 back.
fn v_ring(z: f32, ridge: f32, back: f32, w: f32, t: f32, side: f32) -> Vec<Vec3> {
    let x = |d: f32| side * d;
    vec![
        v3(x(ridge - back), -w, z),
        v3(x(ridge), 0.0, z),
        v3(x(ridge - back), w, z),
        v3(x(ridge - back - t), w - 0.3, z),
        v3(x(ridge - t), 0.0, z),
        v3(x(ridge - back - t), -w + 0.3, z),
    ]
}

/// A V-fronted plate lofted through `(z, ridge, back, w)` sections.
fn v_plate(b: &mut MeshBuilder, sections: &[(f32, f32, f32, f32)], t: f32, side: f32) {
    let rings: Vec<Vec<Vec3>> = sections
        .iter()
        .map(|&(z, r, k, w)| v_ring(z, r, k, w, t, side))
        .collect();
    b.loft(&rings, true, true);
}

/// A plate bent along the polyline `pts` (y, z), `t` thick to the right of the way the
/// polyline runs (list it top-down on the outside), run from `x0` to `x1` along x with its
/// ends drawn in by `bevel`.
fn bent_plate(b: &mut MeshBuilder, pts: &[[f32; 2]], t: f32, x0: f32, x1: f32, bevel: f32) {
    let n = pts.len();
    // The inner face: each point pushed in along the bend's normal (toward -y, down).
    let inner: Vec<[f32; 2]> = (0..n)
        .map(|i| {
            let a = pts[i.saturating_sub(1)];
            let c = pts[(i + 1).min(n - 1)];
            let (dy, dz) = (c[0] - a[0], c[1] - a[1]);
            let l = dy.hypot(dz).max(1e-4);
            // Normal to the right of travel: (dz, -dy), pointing in.
            [pts[i][0] + dz / l * t, pts[i][1] - dy / l * t]
        })
        .collect();
    let profile: Vec<[f32; 2]> = pts
        .iter()
        .copied()
        .chain(inner.iter().rev().copied())
        .collect();
    let mid = profile.iter().fold([0.0, 0.0], |m, p| {
        [
            m[0] + p[0] / profile.len() as f32,
            m[1] + p[1] / profile.len() as f32,
        ]
    });
    let ring = |x: f32, k: f32| -> Vec<Vec3> {
        profile
            .iter()
            .map(|p| {
                v3(
                    x,
                    mid[0] + (p[0] - mid[0]) * k,
                    mid[1] + (p[1] - mid[1]) * k,
                )
            })
            .collect()
    };
    let k = 1.0 - bevel;
    b.loft(
        &[
            ring(x0, k),
            ring(x0 + 1.0, 1.0),
            ring(x1 - 1.0, 1.0),
            ring(x1, k),
        ],
        true,
        true,
    );
}

fn torso(b: &mut MeshBuilder) {
    let fine = b.fine();
    let n = b.sides(12);
    // Waist ring, and a ribbed bare-metal abdomen between the hips and the chest.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(0.0, 0.0, 55.5), n, 9.6, 9.4, 3.5);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 58.8), n, 9.2, 10.4, 3.6);
    if fine {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for z in [59.4, 61.0] {
            b.prism(v3(0.0, 0.0, z), 12, 10.0, 10.1, 0.7);
        }
    }
    // The chest's dark core: what shows in the seams between the plates.
    let (hx, hy, cut) = CHEST_PLAN;
    let plan = chamfered_rect(v2(hx, hy), cut);
    let sections: Vec<Section> = CHEST_SECTIONS
        .iter()
        .map(|&(z, sx, sy, shift)| Section::scaled(z, sx, sy).shifted(shift, 0.0))
        .collect();
    b.paint(PLATING_DARK);
    b.loft_z(&plan, &sections);

    // The armour, lapped like a knight's: two belly lames under a lower glacis, the great
    // breastplate over that, its ridge down the middle; a flatter V over the back.
    b.paint(PLATING);
    v_plate(
        b,
        &[(59.8, 10.6, 5.0, 8.4), (63.6, 11.6, 5.4, 9.6)],
        1.6,
        1.0,
    );
    v_plate(
        b,
        &[(62.4, 12.2, 5.4, 10.4), (66.6, 13.4, 6.0, 12.6)],
        1.8,
        1.0,
    );
    v_plate(
        b,
        &[
            (65.2, 13.6, 6.0, 12.0),
            (68.0, 15.4, 6.6, 15.2),
            (73.4, 16.6, 7.0, 16.6),
        ],
        2.0,
        1.0,
    );
    v_plate(
        b,
        &[
            (72.0, 17.2, 7.2, 16.4),
            (77.0, 17.8, 7.4, 17.8),
            (82.0, 16.8, 7.0, 16.4),
            (85.6, 14.4, 6.2, 13.0),
        ],
        2.2,
        1.0,
    );
    v_plate(
        b,
        &[
            (63.0, 11.6, 3.0, 13.0),
            (72.0, 15.0, 3.4, 17.0),
            (84.0, 13.6, 3.2, 16.0),
            (88.4, 11.6, 2.8, 12.0),
        ],
        2.0,
        -1.0,
    );
    // The flanks under the shoulders: a heavy plate each side, bent in at the top; and the
    // upper chest's sides, a plate raked in from the shoulder to the deck.
    b.mirror_y(|b| {
        b.paint(PLATING);
        bent_plate(
            b,
            &[[17.6, 76.0], [20.6, 72.0], [20.4, 64.0], [18.4, 61.0]],
            1.8,
            -10.0,
            10.5,
            0.12,
        );
        bent_plate(
            b,
            &[[11.0, 90.6], [17.4, 87.4], [19.6, 80.0], [19.2, 75.6]],
            1.9,
            -12.5,
            12.4,
            0.12,
        );
    });
    // Deck plates over the chest top beside the reactor, and the owner's colour down the
    // middle behind the head.
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.plate(v3(-6.0, 8.2, 89.7), v2(10.0, 5.2), 0.8, 0.3);
    });
    team_panel(b, v3(-9.0, 0.0, 90.0), v2(5.6, 7.6));
    if fine {
        chest_detail(b);
    }
    reactor(b);

    // Shoulder sockets, the pauldrons over them, the rocket pods on their pylons.
    b.mirror_y(|b| {
        // The shoulder housing the arm's drum turns in: an armoured block off the chest's
        // side, a dark band round it where it meets the drum.
        b.paint(PLATING);
        b.at(v3(0.0, 0.0, SHOULDER.z), |b| {
            b.loft(
                &[
                    x_ring(-9.5, 23.0, 0.0, 5.6, 6.4, 2.6),
                    x_ring(-7.5, 23.0, 0.0, 6.6, 7.6, 3.0),
                    x_ring(6.5, 23.0, 0.0, 6.6, 7.6, 3.0),
                    x_ring(9.0, 23.0, -0.6, 5.4, 6.2, 2.4),
                ],
                true,
                true,
            );
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.cylinder_between(
                v3(0.0, 29.4, 0.0),
                v3(0.0, ARM_Y - 6.9, 0.0),
                6.9,
                6.9,
                b.sides(12),
            );
        });
        pauldron(b);
        rocket_pod(b);
    });
    // A flak turret beside each pod, on the pauldron's outer deck.
    for (i, &pivot) in FLAK.iter().enumerate() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.prism(pivot - Vec3::Z * 3.6, b.sides(10), 4.4, 4.6, 1.2);
        flak(b, 3 + i, pivot);
    }

    head(b);
    back(b);
    if fine {
        torso_detail(b);
    }
}

/// Up close on the chest: a raised ridge down the breastplate, bolts along the plates'
/// lower edges, a dark recess with a hatch in each side of the breastplate, intake grilles
/// under it, a hazard band along the lower glacis.
fn chest_detail(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.beam(
        v3(17.4, 0.0, 72.6),
        v3(17.9, 0.0, 81.8),
        v2(1.6, 1.0),
        v2(1.4, 0.9),
    );
    b.beam(
        v3(15.5, 0.0, 66.0),
        v3(16.5, 0.0, 72.4),
        v2(1.4, 0.9),
        v2(1.4, 0.9),
    );
    b.paint(METAL);
    for i in 0..6 {
        let y = 2.2 + 2.3 * i as f32;
        let x = 17.1 - y * 7.0 / 16.4 + 0.2;
        for s in [-1.0f32, 1.0] {
            b.cuboid(v3(x, s * y, 72.5), v3(0.8, 0.8, 0.6));
        }
    }
    b.mirror_y(|b| {
        // A recessed panel in the breastplate's face, a hatch in it.
        let (a, c) = (v3(15.0, 5.8, 76.0), v3(12.0, 12.6, 76.0));
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.beam(a, c, v2(5.0, 0.5), v2(5.0, 0.5));
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.beam(
            a.lerp(c, 0.2) + v3(0.3, 0.1, 0.0),
            a.lerp(c, 0.8) + v3(0.3, 0.1, 0.0),
            v2(3.6, 0.4),
            v2(3.6, 0.4),
        );
        // Intake grilles under the lower glacis.
        b.paint(TREAD);
        b.beam(
            v3(12.8, 3.0, 63.4),
            v3(11.2, 8.0, 63.4),
            v2(1.6, 0.4),
            v2(1.6, 0.4),
        );
        b.paint(METAL);
        for k in 0..4 {
            let t = 0.12 + 0.25 * k as f32;
            let p = v3(12.9, 3.0, 63.4).lerp(v3(11.3, 8.0, 63.4), t);
            b.cuboid(p, v3(0.5, 0.4, 1.8));
        }
        // Louvres in the flank plate, between the waist and the pauldron.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(v3(-7.0, 20.5, 64.8), v3(5.0, 21.1, 70.6));
        b.paint(METAL);
        for i in 0..5 {
            let z = 65.3 + 1.15 * i as f32;
            b.block(v3(-6.5, 21.0, z), v3(4.5, 21.5, z + 0.5));
        }
    });
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.beam(
        v3(13.1, -8.0, 65.0),
        v3(15.0, 0.0, 65.0),
        v2(0.6, 0.8),
        v2(0.6, 0.8),
    );
    b.beam(
        v3(15.0, 0.0, 65.0),
        v3(13.1, 8.0, 65.0),
        v2(0.6, 0.8),
        v2(0.6, 0.8),
    );
}

/// The left pauldron: a thick angular deck over the shoulder socket (the pods' pylon and a
/// flak turret stand on it) and three armour lames stepping down its outer side, each
/// lapped under the one above, clear of the upper arm swinging beneath.
fn pauldron(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (cx, cy, hx, hy, top) = DECK;
    b.paint(PLATING);
    b.at(v3(cx, cy, 0.0), |b| {
        b.loft_z(
            &chamfered_rect(v2(hx, hy), 4.5),
            &[
                Section::scaled(top - 6.5, 0.84, 0.84).shifted(1.6, 0.0),
                Section::new(top - 2.0, 1.0),
                Section::scaled(top, 0.9, 0.94).shifted(-0.8, 0.0),
            ],
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &chamfered_rect(v2(hx, hy), 4.5),
            &[
                Section::scaled(top - 8.0, 0.8, 0.78),
                Section::scaled(top - 6.4, 0.87, 0.85),
            ],
        );
    });
    // The lames: bent plates down the outside, each shorter and lower than the last.
    let lames: [(&[[f32; 2]], f32, f32); 3] = [
        (
            &[[36.0, top - 0.5], [43.4, top - 3.4], [45.0, top - 10.0]],
            -14.0,
            8.5,
        ),
        (
            &[[41.0, top - 6.4], [46.2, top - 9.6], [47.4, top - 15.4]],
            -12.5,
            7.0,
        ),
        (
            &[[44.2, top - 12.0], [48.4, top - 14.8], [49.0, top - 19.6]],
            -11.0,
            5.5,
        ),
    ];
    for (pts, x0, x1) in lames {
        b.paint(PLATING);
        bent_plate(b, pts, 1.7, x0, x1, 0.1);
        if fine {
            // A dark lip under each lame's edge, bolts along it.
            let last = pts[pts.len() - 1];
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.block(
                v3(x0 + 1.5, last[0] - 1.9, last[1] - 0.6),
                v3(x1 - 1.5, last[0] - 0.4, last[1] + 0.6),
            );
            b.paint(METAL);
            let mut x = x0 + 2.5;
            while x < x1 - 1.5 {
                b.cuboid(v3(x, pts[1][0] + 0.3, pts[1][1] - 0.6), v3(0.8, 0.6, 0.8));
                x += 3.2;
            }
        }
    }
    team_panel(b, v3(cx + 4.0, cy + 8.0, top + 0.02), v2(5.0, 3.0));
    if fine {
        // A hazard band round the deck's front edge, a hatch in its back, a vent grille
        // on its crown.
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(
            v3(cx + hx - 1.2, cy - 8.0, top - 3.2),
            v3(cx + hx + 0.2, cy + 8.0, top - 2.2),
        );
        hatch(b, v3(cx - hx - 0.1, cy + 3.0, top - 4.5), 4.0, 3.0);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(cx - 8.0, cy + 9.0, top),
            v3(cx - 1.0, cy + 12.4, top + 0.3),
        );
        b.paint(METAL);
        for i in 0..5 {
            let x = cx - 7.6 + 1.3 * i as f32;
            b.block(
                v3(x, cy + 9.3, top + 0.2),
                v3(x + 0.5, cy + 12.1, top + 0.5),
            );
        }
    }
}

/// The dorsal reactor behind the head: a round housing lying fore and aft on the chest
/// top, banded, finned along its crown, vented at its sides, its feeds running out to the
/// pods and down the back.
fn reactor(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1, z, r) = (-12.0, 1.0, 92.4, 4.0);
    let axis = |x: f32| v3(x, 0.0, z);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(x0 + 1.0, -3.2, 89.6), v3(x1 - 1.0, 3.2, 90.8));
    b.paint(PLATING_DARK);
    b.cylinder_between(axis(x0), axis(x1), r, r, b.sides(14));
    b.paint(PLATING);
    b.cylinder_between(axis(x1), axis(x1 + 1.2), r * 0.92, r * 0.7, b.sides(14));
    b.cylinder_between(axis(x0 - 1.2), axis(x0), r * 0.7, r * 0.92, b.sides(14));
    if !fine {
        return;
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for x in [x0 + 1.4, -5.5, x1 - 2.2] {
        b.cylinder_between(axis(x), axis(x + 0.9), r + 0.25, r + 0.25, 14);
    }
    // Heat-sink fins across its crown, and grilles low on its flanks.
    b.paint(METAL);
    for i in 0..7 {
        let x = x0 + 2.8 + 1.15 * i as f32;
        if (x - (-5.1)).abs() < 0.9 {
            continue;
        }
        b.block(v3(x, -2.6, z + r - 0.8), v3(x + 0.35, 2.6, z + r + 1.1));
    }
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(x0 + 2.0, r - 0.6, z - 1.6),
            v3(x1 - 3.0, r + 0.1, z + 1.0),
        );
        b.paint(METAL);
        for i in 0..4 {
            let zz = z - 1.3 + 0.6 * i as f32;
            b.block(v3(x0 + 2.4, r, zz), v3(x1 - 3.4, r + 0.3, zz + 0.25));
        }
        // Feeds out to the pods, and down the back to the spine.
        b.cylinder_between(
            v3(-8.0, r * 0.8, z - 1.6),
            v3(-9.0, 13.0, 88.6),
            0.55,
            0.55,
            6,
        );
        b.cylinder_between(v3(-9.0, 13.0, 88.6), v3(-12.0, 17.0, 88.0), 0.55, 0.55, 6);
        b.cylinder_between(
            v3(-3.0, r * 0.8, z - 1.8),
            v3(-4.0, 11.0, 89.4),
            0.45,
            0.45,
            6,
        );
        b.cylinder_between(
            v3(x0 - 0.8, 1.6, z - 1.0),
            v3(-14.5, 2.6, 88.2),
            0.5,
            0.5,
            6,
        );
    });
    // An antenna cluster at its tail.
    antenna_unlit(b, v3(x0 - 0.4, 1.6, z + 1.6), 6.5, 0.15);
    antenna_unlit(b, v3(x0 - 0.2, -1.4, z + 2.0), 4.2, 0.25);
    antenna_unlit(b, v3(x0 + 0.6, 0.2, z + 3.2), 3.0, 0.1);
}

/// The left rocket pod: a low armoured launcher on a pylon over the pauldron, raked
/// nose-up (`POD_PITCH` about `POD_FACE`), six cells in two rows in its face (all six the
/// unit file's muzzles, `POD_MOUTHS`), a cheek shroud down its outside, reload hatches on
/// its lid and a blast grille in its tail.
fn rocket_pod(b: &mut MeshBuilder) {
    let fine = b.fine();
    let f = POD_FACE;
    let (hy, hz) = (7.4, 4.6);
    // The pylon from the deck up into the pod's belly, and a brace aft.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.extrude_y(
        &[
            [-10.0, DECK.4 - 0.5],
            [-1.0, DECK.4 - 0.5],
            [-3.0, f.z - 3.6],
            [-9.0, f.z - 5.2],
        ],
        f.y - 3.4,
        f.y + 3.4,
    );
    b.paint(METAL);
    b.cylinder_between(
        v3(-12.0, f.y, DECK.4),
        v3(-9.0, f.y, f.z - 4.6),
        1.1,
        1.1,
        8,
    );
    b.pitched(f, POD_PITCH, |b| {
        // In the pod's own frame: its face's middle is the origin.
        let (x, y, z) = (0.0f32, 0.0f32, 0.0f32);
        b.paint(PLATING);
        b.loft(
            &[
                x_ring(x - 0.3, y, z, hy, hz, 2.2),
                x_ring(x - 3.0, y, z + 0.3, hy + 0.4, hz + 0.4, 2.6),
                x_ring(x - 11.0, y, z - 0.2, hy - 0.4, hz - 0.2, 2.4),
                x_ring(x - 15.0, y, z - 0.8, hy - 1.6, hz - 1.4, 1.8),
            ],
            true,
            true,
        );
        team_panel(b, v3(x - 2.0, y, z + hz + 0.4), v2(2.0, 2.0 * hy - 5.0));
        if fine {
            // Six cells: a dark face, a metal rim round each, the mouth dark in it.
            b.paint(PLATING_DARK).pattern(pattern::PLAIN);
            b.loft(
                &[
                    x_ring(x - 0.4, y, z, hy - 0.5, hz - 0.5, 1.8),
                    x_ring(x - 0.1, y, z, hy - 0.5, hz - 0.5, 1.8),
                ],
                true,
                true,
            );
            for dz in POD_ROWS {
                for dy in POD_COLS {
                    let m = v3(x, y + dy, z + dz);
                    b.paint(METAL).pattern(pattern::PLAIN);
                    b.cylinder_between(m - X * 0.4, m + X * 0.3, 1.75, 1.65, 8);
                    b.paint(TREAD);
                    let mouth: Vec<Vec3> = (0..8)
                        .map(|k| {
                            let a = (k as f32 + 0.5) * TAU / 8.0;
                            m + v3(0.32, a.cos() * 1.2, a.sin() * 1.2)
                        })
                        .collect();
                    b.face(&mouth);
                }
            }
            // The cheek shroud: a raked armour plate down the outside, standing off it.
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [x + 0.6, z - hz + 0.6],
                    [x + 0.6, z + hz - 0.4],
                    [x - 4.0, z + hz + 0.8],
                    [x - 13.0, z + hz - 1.2],
                    [x - 11.0, z - hz - 0.2],
                ],
                y + hy + 0.5,
                y + hy + 1.7,
            );
            // Ribs round the body.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for rx in [x - 5.0, x - 9.5] {
                b.loft(
                    &[
                        x_ring(rx - 0.5, y, z, hy + 0.5, hz + 0.5, 2.6),
                        x_ring(rx + 0.5, y, z, hy + 0.5, hz + 0.5, 2.6),
                    ],
                    true,
                    true,
                );
            }
            // The lid: two reload hatches in a dark frame, latched at the edges.
            let lid = z + hz + 0.35;
            b.paint(PLATING_DARK).pattern(pattern::PLAIN);
            b.plate(v3(x - 6.0, y, lid - 0.1), v2(9.0, 2.0 * hy - 3.6), 0.3, 0.1);
            for s in [-1.0f32, 1.0] {
                b.paint(PLATING);
                b.plate(
                    v3(x - 6.0, y + s * (0.5 * hy - 0.5), lid + 0.15),
                    v2(8.0, hy - 2.4),
                    0.55,
                    0.2,
                );
                b.paint(METAL);
                for lx in [x - 9.0, x - 3.0] {
                    b.cuboid(v3(lx, y + s * (hy - 1.6), lid + 0.5), v3(1.0, 0.6, 0.5));
                }
            }
            // The tail: a dark blast grille where the back-blast vents.
            b.paint(TREAD);
            b.loft(
                &[
                    x_ring(x - 15.05, y, z - 0.8, hy - 2.4, hz - 2.2, 1.4),
                    x_ring(x - 14.9, y, z - 0.8, hy - 2.4, hz - 2.2, 1.4),
                ],
                true,
                true,
            );
            b.paint(METAL);
            for k in 0..4 {
                let gy = y - 3.6 + 2.4 * k as f32;
                b.block(
                    v3(x - 15.3, gy - 0.25, z - 2.8),
                    v3(x - 15.0, gy + 0.25, z + 1.2),
                );
            }
        } else {
            // From further off the cell face is one dark panel.
            b.paint(PLATING_DARK);
            b.block(
                v3(x - 0.6, y - hy + 0.6, z - hz + 0.6),
                v3(x + 0.3, y + hy - 0.6, z + hz - 0.6),
            );
        }
    });
}

/// The head: a low armoured sensor hood sunk between the pods over a heavy gorget, a
/// wedge brow over a wrap-round bridge of orange visor glass, sensor cans either side of it, a
/// crest down its crown. It looks about while the titan idles.
fn head(b: &mut MeshBuilder) {
    let z = NECK.z;
    // The gorget it turns in: fixed to the chest.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(8.0, 0.0, z - 2.6), b.sides(10), 8.4, 7.2, 2.4);
    b.with_head(NECK, |b| {
        let fine = b.fine();
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            v3(7.6, 0.0, z - 3.0),
            v3(9.4, 0.0, z + 2.0),
            4.6,
            4.2,
            b.sides(10),
        );
        b.at(v3(11.0, 0.0, 0.0), |b| {
            // The dark hood, low and long.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(
                &turret_plan(17.0, 12.6),
                &[
                    Section::scaled(z + 0.2, 0.8, 0.82),
                    Section::new(z + 3.2, 1.0),
                    Section::scaled(z + 6.0, 0.94, 0.92).shifted(-0.6, 0.0),
                    Section::scaled(z + 8.2, 0.7, 0.66).shifted(-2.2, 0.0),
                ],
            );
            // The brow: a light plate sloping down over the slit to a blunt prow.
            b.paint(PLATING);
            b.loft_z(
                &turret_plan(15.4, 12.0),
                &[
                    Section::scaled(z + 5.2, 1.04, 1.0).shifted(0.6, 0.0),
                    Section::scaled(z + 6.6, 0.98, 0.96).shifted(-0.2, 0.0),
                    Section::scaled(z + 8.6, 0.66, 0.6).shifted(-2.0, 0.0),
                ],
            );
            // Crest down the crown.
            b.extrude_y(
                &[
                    [-6.8, z + 8.2],
                    [3.0, z + 8.4],
                    [0.6, z + 10.0],
                    [-6.2, z + 9.6],
                ],
                -0.8,
                0.8,
            );
            // The bridge: a band of the commander's orange visor glass wrapped round the hood
            // under the brow, raked out toward the top, framed by dark mullions up close.
            b.paint(VISOR).pattern(pattern::PLAIN);
            b.loft_z(
                &turret_plan(17.0, 12.6),
                &[
                    Section::scaled(z + 2.6, 1.0, 1.0),
                    Section::scaled(z + 4.2, 1.03, 1.02).shifted(0.2, 0.0),
                    Section::scaled(z + 5.4, 1.05, 1.03).shifted(0.4, 0.0),
                ],
            );
            if fine {
                b.paint(TREAD);
                for y in [-2.6f32, 0.0, 2.6] {
                    b.beam(
                        v3(8.9, y, z + 2.7),
                        v3(9.3, y, z + 5.3),
                        v2(0.35, 0.3),
                        v2(0.35, 0.3),
                    );
                }
            }
            b.mirror_y(|b| {
                // A sensor can on each cheek, a dark lens in its face.
                b.paint(PLATING);
                b.cylinder_between(
                    v3(-3.0, 7.0, z + 1.6),
                    v3(3.6, 7.0, z + 1.6),
                    1.6,
                    1.6,
                    b.sides(10),
                );
                b.paint(TREAD);
                b.cylinder_between(
                    v3(3.6, 7.0, z + 1.6),
                    v3(3.75, 7.0, z + 1.6),
                    1.1,
                    1.1,
                    b.sides(10),
                );
            });
            if fine {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.mirror_y(|b| b.block(v3(-5.0, 5.4, z + 1.0), v3(2.0, 6.4, z + 2.0)));
                b.paint(METAL);
                for x in [-4.2, -2.6, -1.0] {
                    b.block(v3(x, -2.4, z + 8.0), v3(x + 0.5, 2.4, z + 8.5));
                }
                antenna_unlit(b, v3(-6.0, -3.2, z + 7.4), 6.0, 0.25);
                antenna_unlit(b, v3(-6.4, 3.4, z + 7.2), 3.6, 0.3);
            }
        });
    });
}

/// The back: a dark power spine with louvres, heat-sink fins either side, and the
/// shield projector on top.
fn back(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.paint(PLATING);
    b.extrude_y_chamfered(
        &[
            [-17.5, 67.0],
            [-13.0, 67.0],
            [-11.0, 90.6],
            [-18.6, 90.6],
            [-21.6, 86.0],
            [-21.6, 71.0],
        ],
        7.2,
        if fine { 1.5 } else { 0.0 },
    );
    // Projector: a dark base, a metal neck, a dome with a gold ring round its waist.
    let s = SHIELD;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(s.x, 0.0, 90.4), b.sides(10), 4.2, 3.4, 3.0);
    b.paint(METAL);
    b.cylinder_between(v3(s.x, 0.0, 93.2), v3(s.x, 0.0, 95.0), 1.9, 1.9, b.sides(8));
    b.paint(PLATING_DARK);
    b.spheroid(
        v3(s.x, 0.0, 95.6),
        v3(3.0, 3.0, 2.0),
        b.sides(10),
        if fine { 4 } else { 3 },
    );
    b.paint(GLOW_SHIELD);
    b.cylinder_between(
        v3(s.x, 0.0, 95.3),
        v3(s.x, 0.0, 96.0),
        3.12,
        3.1,
        b.sides(10),
    );
    if fine {
        // Louvres down the spine's back.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for i in 0..4 {
            let z = 72.5 + 3.3 * i as f32;
            b.block(v3(-22.4, -5.6, z), v3(-21.4, 1.8, z + 1.2));
        }
        // A service ladder up the spine to the projector deck.
        ladder(b, v3(-21.9, 4.2, 71.5), 90.2, 1.6);
        // Heat-sink fins either side of the spine.
        b.mirror_y(|b| {
            for y in [9.0, 12.5, 16.0] {
                b.paint(PLATING).pattern(pattern::PLAIN);
                b.extrude_y(
                    &[[-16.0, 73.0], [-14.0, 88.0], [-18.4, 89.0], [-20.0, 76.0]],
                    y,
                    y + 1.0,
                );
            }
        });
        antenna_unlit(b, v3(-16.0, 9.0, 90.4), 7.0, 0.2);
    }
}

// ---- arms ---------------------------------------------------------------------------

/// A ring of eight round a line along x at (`y`, `z`): an octagon `hy` by `hz` with its
/// corners cut `c`.
fn x_ring(x: f32, y: f32, z: f32, hy: f32, hz: f32, c: f32) -> Vec<Vec3> {
    chamfered_rect(v2(hy, hz), c)
        .iter()
        .map(|p| v3(x, y + p[0], z + p[1]))
        .collect()
}

/// A round ring of `n` about a line along x at (`y`, `z`), `ry` across and `rz` high.
fn o_ring(x: f32, y: f32, z: f32, ry: f32, rz: f32, n: usize) -> Vec<Vec3> {
    (0..n)
        .map(|i| {
            let a = (i as f32 + 0.5) * TAU / n as f32;
            v3(x, y + a.cos() * ry, z + a.sin() * rz)
        })
        .collect()
}

/// The shoulder and the upper arm the gun hangs from: a drum on the pivot line, an armoured
/// upper arm down to the elbow, a ram down its back, cables down its inside; the elbow a pin
/// through a clevis of two cheek plates rising from a saddle bolted on the gun's roof (`roof`
/// over the bore line, `half` the gun's half width there), a strut ram forward onto the gun.
fn shoulder(b: &mut MeshBuilder, y: f32, half: f32, roof: f32) {
    let s = y.signum();
    let z = SHOULDER.z;
    let fine = b.fine();
    let top = v3(0.0, y, z);
    let elbow = v3(-2.0, y, ARM_Z + roof + 3.4);
    let bone = (top, elbow);
    let (_, across, face) = bone_frame(top, elbow);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(
        v3(0.0, y - s * 7.4, z),
        v3(0.0, y + s * 6.2, z),
        6.6,
        6.6,
        b.sides(12),
    );
    // An armoured cap over the drum, so the shoulder reads as a joint, not a bare tube.
    b.paint(PLATING);
    b.at(v3(0.0, y + s * 0.4, z), |b| {
        b.loft(
            &[
                x_ring(-7.2, 0.0, 0.0, 6.2, 5.8, 2.6),
                x_ring(-5.6, 0.0, 0.4, 7.2, 7.4, 3.0),
                x_ring(5.4, 0.0, 0.4, 7.2, 7.4, 3.0),
                x_ring(7.4, 0.0, -0.4, 6.0, 5.8, 2.4),
            ],
            true,
            true,
        )
    });
    limb(
        b,
        top,
        elbow,
        &[
            (0.0, 5.4, 6.2, 6.2, 2.4),
            (0.45, 5.6, 6.8, 6.4, 2.6),
            (1.0, 4.8, 5.4, 5.2, 2.0),
        ],
    );
    // Armour over its front and outer flank, a ram down its back.
    b.paint(PLATING);
    limb_plate(
        b,
        bone,
        (0.2, 0.9),
        face,
        (6.6, 5.6),
        Vec3::ZERO,
        (5.8, 1.7),
        (5.0, 1.4),
    );
    limb_plate(
        b,
        bone,
        (0.25, 0.88),
        across * s,
        (5.5, 5.0),
        face * 0.6,
        (5.4, 1.5),
        (4.6, 1.3),
    );
    ram(
        b,
        top.lerp(elbow, 0.2) - face * 7.2,
        top.lerp(elbow, 0.92) - face * 6.2,
        1.7,
        1.05,
    );
    // The saddle on the gun's roof, and the clevis cheeks either side of the arm's foot.
    let r = ARM_Z + roof;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.at(v3(0.0, y, 0.0), |b| {
        b.extrude_y_chamfered(
            &[
                [-9.0, r - 0.6],
                [8.0, r - 0.6],
                [6.0, r + 2.2],
                [-7.4, r + 2.2],
            ],
            half - 0.4,
            if fine { 0.8 } else { 0.0 },
        )
    });
    b.paint(PLATING);
    for side in [-1.0f32, 1.0] {
        let (y0, y1) = (y + side * 5.4, y + side * 7.2);
        b.extrude_y(
            &[
                [-6.2, r + 1.6],
                [5.6, r + 1.6],
                [5.0, elbow.z + 1.6],
                [2.4, elbow.z + 5.2],
                [-2.8, elbow.z + 5.2],
                [-6.0, elbow.z + 2.0],
            ],
            y0.min(y1),
            y0.max(y1),
        );
    }
    // The pin through them, a nut at each end.
    b.paint(METAL);
    b.cylinder_between(elbow - Y * 7.9, elbow + Y * 7.9, 2.6, 2.6, b.sides(12));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for side in [-1.0f32, 1.0] {
        b.cylinder_between(
            elbow + Y * (side * 7.2),
            elbow + Y * (side * 8.3),
            3.6,
            3.6,
            6,
        );
    }
    // The strut: from the upper arm's front down onto the gun's roof ahead of the saddle.
    ram(
        b,
        top.lerp(elbow, 0.45) + face * 5.6,
        v3(11.0, y, r + 0.8),
        1.4,
        0.9,
    );
    if fine {
        // Bolts along the saddle, cables down the inside of the arm, a band on it.
        b.paint(METAL);
        for side in [-1.0f32, 1.0] {
            for x in [-6.5, -2.0, 2.5] {
                b.cuboid(v3(x, y + side * (half - 0.2), r + 1.2), v3(0.9, 0.5, 0.9));
            }
        }
        for dz in [-1.3f32, 1.3] {
            let off = -across * s * 5.8 + face * dz;
            b.cylinder_between(
                top.lerp(elbow, 0.05) + off,
                top.lerp(elbow, 0.9) + off,
                0.5,
                0.5,
                6,
            );
        }
        b.paint(PLATING).pattern(pattern::PLAIN);
        for t in [0.3, 0.55] {
            limb_plate(
                b,
                bone,
                (t, t + 0.16),
                face,
                (8.3, 8.0),
                Vec3::ZERO,
                (4.6, 0.8),
                (4.6, 0.8),
            );
        }
        b.paint(TEAM);
        limb_plate(
            b,
            bone,
            (0.3, 0.8),
            across * s,
            (7.0, 6.4),
            face * 0.6,
            (1.0, 0.3),
            (0.9, 0.3),
        );
    }
}

/// Right arm: the Tempest rotary cannon. Its bulk hangs straight under the elbow: a deep
/// rounded body holding the breeches, the ammunition drum slung behind it (the arm's
/// counterweight) feeding it by a belt chute underneath, the spin motor under its front,
/// the spent cases leaving by a small port in its outboard flank (`EJECT`). Out of its face
/// only the six long barrels, the drum plate they stand in and their collars turn about the
/// bore as the gun spins up.
fn gatling_arm(b: &mut MeshBuilder) {
    let y = -ARM_Y;
    let z = ARM_Z;
    let axis = v3(0.0, y, z);
    b.with_house(1, SHOULDER, 0.0, |b| {
        b.with_recoil(|b| {
            let fine = b.fine();
            let n = b.sides(16);
            shoulder(b, y, 8.6, 8.2);
            b.paint(PLATING);
            b.loft(
                &[
                    o_ring(-13.0, y, z, 5.0, 4.8, n),
                    o_ring(-10.0, y, z, 7.6, 7.4, n),
                    o_ring(8.0, y, z, 8.6, 8.4, n),
                    o_ring(11.5, y, z, 8.2, 8.0, n),
                    o_ring(14.5, y, z, 7.6, 7.4, n),
                    o_ring(16.0, y, z, 7.3, 7.2, n),
                ],
                true,
                true,
            );
            // A dorsal spine, bands round the body, a lip round its face.
            b.beam(
                v3(-9.0, y, z + 8.1),
                v3(9.0, y, z + 8.2),
                v2(3.6, 1.4),
                v2(3.2, 1.2),
            );
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for x in [-6.5, 3.5] {
                b.loft(
                    &[
                        o_ring(x, y, z, 8.55, 8.35, n),
                        o_ring(x + 1.3, y, z, 8.75, 8.55, n),
                    ],
                    true,
                    true,
                );
            }
            b.loft(
                &[
                    o_ring(14.2, y, z, 7.9, 7.7, n),
                    o_ring(15.6, y, z, 7.7, 7.5, n),
                ],
                true,
                true,
            );
            // The ammunition drum behind, its axis across the arm, and the belt chute from
            // it under the body.
            let drum = v3(-20.0, y, z + 0.5);
            b.paint(PLATING);
            b.cylinder_between(drum - Y * 5.8, drum + Y * 5.8, 7.4, 7.4, b.sides(18));
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for side in [-1.0f32, 1.0] {
                b.cylinder_between(
                    drum + Y * (side * 5.8),
                    drum + Y * (side * 6.5),
                    6.2,
                    5.6,
                    b.sides(18),
                );
            }
            b.paint(PLATING);
            b.beam(
                drum + v3(4.0, 0.0, -5.6),
                v3(-5.0, y, z - 7.4),
                v2(4.4, 2.6),
                v2(4.0, 2.4),
            );
            // The spin motor under the body's front.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.cylinder_between(
                v3(3.0, y, z - 8.2),
                v3(14.0, y, z - 7.4),
                2.6,
                2.4,
                b.sides(10),
            );
            b.paint(METAL);
            b.cylinder_between(
                v3(14.0, y, z - 7.4),
                v3(15.4, y, z - 7.2),
                1.8,
                1.4,
                b.sides(10),
            );
            // The ejection port: a small framed slot in the outboard flank.
            let port = v3(EJECT.x, y - 8.45, EJECT.z);
            b.paint(PLATING).pattern(pattern::HAZARD);
            b.block(port + v3(-3.0, -0.3, -1.9), port + v3(3.0, 0.4, 1.9));
            b.paint(TREAD);
            b.block(port + v3(-2.1, -0.5, -0.9), port + v3(2.1, -0.25, 0.9));
            b.paint(PLATING);
            b.block(port + v3(-2.6, -1.1, 1.3), port + v3(2.6, -0.2, 1.9));
            if fine {
                gatling_detail(b, y, z, drum);
            }
            b.with_spin(axis, rail_cluster);
        })
    });
}

/// What of the Tempest turns, about the arm's bore line: the six barrels out of the body's
/// face, the drum plate they stand in and the collars clamping them.
fn rail_cluster(b: &mut MeshBuilder) {
    let axis = v3(0.0, -ARM_Y, ARM_Z);
    let fine = b.fine();
    let n = b.sides(12);
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.cylinder_between(axis + X * 15.8, axis + X * 17.6, 7.1, 6.9, n);
    b.paint(METAL);
    b.cylinder_between(axis + X * 17.6, axis + X * 63.0, 1.35, 1.15, b.sides(8));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(axis + X * 63.0, GATLING_MUZZLE, 1.9, 1.7, b.sides(8));
    for (x0, x1, r) in [(29.0, 30.8, 7.2), (44.0, 45.6, 7.1), (58.8, 60.6, 7.1)] {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(axis + X * x0, axis + X * x1, r, r, n);
    }
    if fine {
        // Bolt lugs on the drum plate's face, between the barrels.
        b.paint(METAL);
        for k in 0..6 {
            let a = k as f32 * TAU / 6.0;
            let off = v3(0.0, a.cos(), a.sin()) * 5.6;
            b.cylinder_between(axis + off + X * 17.5, axis + off + X * 18.2, 0.7, 0.7, 6);
        }
    }
    // Six heavy gun barrels round the axis.
    for k in 0..6 {
        let a = (k as f32 + 0.5) * TAU / 6.0;
        b.with(
            Affine3A::from_translation(axis) * Affine3A::from_rotation_x(a),
            |b| gun_barrel(b, 4.9),
        );
    }
}

/// One of the Tempest's barrels, `r` off the cluster's axis (local +z) and running along
/// x: a heavy jacket out of the drum plate (the breech is inside the body), a long tube, a
/// bore evacuator part way, a slotted muzzle brake at the end.
fn gun_barrel(b: &mut MeshBuilder, r: f32) {
    let fine = b.fine();
    let n = b.sides(12);
    let at = |x: f32| v3(x, 0.0, r);
    let end = GATLING_MUZZLE.x;
    // The jacket, with a bolted band.
    b.paint(PLATING);
    b.cylinder_between(at(17.6), at(26.0), 1.9, 1.75, n);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(at(25.2), at(26.6), 2.0, 1.9, n);
    // The tube.
    b.paint(METAL);
    b.cylinder_between(at(26.0), at(end - 3.2), 1.35, 1.15, n);
    // The bore evacuator: a swelling round the tube.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(at(47.0), at(48.2), 1.25, 1.65, n);
    b.cylinder_between(at(48.2), at(51.8), 1.65, 1.65, n);
    b.cylinder_between(at(51.8), at(53.0), 1.65, 1.2, n);
    // The muzzle brake: a block with ports through each side, a dark bore at its face.
    b.paint(PLATING);
    b.chamfered_box(at(end - 1.6), v3(3.2, 2.9, 3.0), 0.8);
    if fine {
        b.paint(TREAD);
        for x in [end - 2.5, end - 1.2] {
            b.block(at(x) + v3(-0.4, -1.5, -0.8), at(x) + v3(0.4, 1.5, 0.8));
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(at(20.0), at(20.7), 2.05, 2.05, n);
    }
    b.paint(TREAD);
    b.cylinder_between(at(end - 0.02), at(end + 0.03), 0.75, 0.75, n);
}

/// Left arm: the AEB-3 Cataclysm Bore. A broad breech housing under the elbow running
/// straight back into an armoured capacitor housing (the arm's counterweight) that holds
/// three great cans, their capped ends showing at its back, their coils glowing through
/// slots in its flanks; a heavy sleeve ahead, and the bore itself out of the sleeve: a dark
/// core through six induction collars with blue coil windows between them, four ceramic
/// blades swept along it, two prongs either side of the aperture. The core kicks back into
/// the sleeve when it fires.
fn bore_arm(b: &mut MeshBuilder) {
    let y = ARM_Y;
    let z = ARM_Z;
    let axis = v3(0.0, y, z);
    b.with_house(2, SHOULDER, 0.0, |b| {
        b.with_recoil(|b| {
            let fine = b.fine();
            shoulder(b, y, 6.2, 4.8);
            // The breech housing and the capacitor housing behind it, one body.
            b.paint(PLATING);
            b.loft(
                &[
                    x_ring(-31.6, y, z + 3.8, 6.6, 7.0, 2.6),
                    x_ring(-30.2, y, z + 3.8, 7.6, 7.9, 3.2),
                    x_ring(-15.0, y, z + 3.6, 7.6, 7.9, 3.2),
                    x_ring(-9.0, y, z + 0.6, 6.4, 5.2, 3.0),
                    x_ring(19.0, y, z, 6.2, 4.8, 3.0),
                    x_ring(23.0, y, z, 5.4, 4.5, 2.6),
                ],
                true,
                true,
            );
            // The sleeve the core runs in, wound with a heavy coil stack.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.cylinder_between(axis + X * 21.0, axis + X * 30.5, 5.6, 5.3, b.sides(12));
            let ring = b.sides(16);
            for i in 0..4 {
                let x = 21.6 + 2.2 * i as f32;
                b.paint(METAL);
                b.cylinder_between(axis + X * x, axis + X * (x + 1.3), 6.8, 6.8, ring);
                coil(b, x);
                b.cylinder_between(axis + X * (x + 1.3), axis + X * (x + 2.2), 6.2, 6.2, ring);
            }
            // Three great capacitor rings round the breech, each with its blue band of
            // charge and a crown of lugs that turns as the charge builds, each ring the
            // other way to the last.
            for (k, x) in [-4.0f32, 4.0, 12.0].into_iter().enumerate() {
                b.paint(PLATING_DARK);
                b.cylinder_between(axis + X * x, axis + X * (x + 3.0), 7.35, 7.35, ring);
                coil(b, x);
                b.cylinder_between(axis + X * (x + 1.0), axis + X * (x + 2.0), 7.5, 7.5, ring);
                let turn = if k % 2 == 0 {
                    pattern::COIL_TURN
                } else {
                    pattern::COIL_TURN_BACK
                };
                b.paint(METAL).pattern(turn);
                for j in 0..if fine { 8 } else { 4 } {
                    let a = (j as f32 + 0.5) * TAU / if fine { 8.0 } else { 4.0 };
                    let at = axis + X * (x + 1.5) + v3(0.0, a.cos(), a.sin()) * 7.55;
                    b.with(
                        Affine3A::from_translation(at) * Affine3A::from_rotation_x(a),
                        |b| {
                            b.cuboid(Vec3::ZERO, v3(3.4, 1.6, 0.9));
                        },
                    );
                }
            }
            // The cans' ends out of the housing's back: a dark collar, a banded cap, the
            // coil's light round it.
            let cans = [(-2.9f32, 1.6f32, 3.1f32), (2.9, 1.6, 3.1), (0.0, 7.0, 2.7)];
            for (dy, dz, r) in cans {
                let at = |x: f32| v3(x, y + dy, z + dz);
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.cylinder_between(at(-32.4), at(-31.4), r * 0.95, r, b.sides(12));
                coil(b, -33.0);
                b.cylinder_between(at(-33.0), at(-32.4), r * 0.8, r * 0.8, b.sides(12));
                b.paint(METAL);
                b.cylinder_between(at(-33.6), at(-33.0), r * 0.55, r * 0.5, b.sides(12));
            }
            // Charge slots down the housing's flanks, lit by the coils inside.
            for side in [-1.0f32, 1.0] {
                let yy = y + side * 7.65;
                b.paint(TREAD);
                b.block(v3(-28.0, yy - 0.2, z + 2.0), v3(-17.0, yy + 0.2, z + 5.0));
                coil(b, -24.0);
                b.block(v3(-27.4, yy - 0.3, z + 3.0), v3(-17.6, yy + 0.3, z + 4.0));
            }
            if fine {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.block(v3(-8.5, y - 6.6, z - 4.8), v3(18.5, y + 6.6, z - 4.1));
                // Feeds from the breech into the sleeve.
                b.paint(METAL);
                for s in [-1.0, 1.0] {
                    b.cylinder_between(
                        v3(16.0, y + s * 4.2, z + 4.2),
                        v3(26.0, y + s * 3.6, z + 4.0),
                        0.7,
                        0.7,
                        6,
                    );
                }
                bore_detail(b, y, z);
            }
        })
    });
    b.with_house(2, BORE_SLIDE, BORE_RECOIL, |b| b.with_recoil(bore));
}

/// Paints what follows as the AEB's live light at `x` along the arm: its coil stage runs
/// from the capacitor bank's back (0) to the aperture (7).
fn coil(b: &mut MeshBuilder, x: f32) {
    let stage = (((x + 33.0) / (BORE_MUZZLE.x + 33.0)) * (pattern::COIL_STAGES - 1) as f32)
        .round()
        .clamp(0.0, (pattern::COIL_STAGES - 1) as f32) as u32;
    b.paint(GLOW).pattern(pattern::COIL + stage);
}

/// The bore itself, from inside the sleeve to the aperture.
fn bore(b: &mut MeshBuilder) {
    let axis = v3(0.0, ARM_Y, ARM_Z);
    let fine = b.fine();
    let hex = 6;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(axis + X * 14.0, axis + X * 67.4, 2.8, 2.1, hex);
    let collars: &[f32] = if fine {
        &[31.0, 37.5, 44.0, 50.5, 57.0, 63.0]
    } else {
        &[31.0, 44.0, 57.0]
    };
    for (k, &x) in collars.iter().enumerate() {
        let r = 7.0 - 1.6 * (x - 31.0) / 32.0;
        // The hex collars turn about the bore, one way then the other.
        b.paint(METAL).pattern(if k % 2 == 0 {
            pattern::COIL_TURN
        } else {
            pattern::COIL_TURN_BACK
        });
        b.cylinder_between(axis + X * x, axis + X * (x + 1.8), r, r * 0.9, hex);
        coil(b, x + 2.0);
        b.cylinder_between(
            axis + X * (x + 1.8),
            axis + X * (x + 2.6),
            r * 0.66,
            r * 0.66,
            hex,
        );
    }
    if fine {
        // The channel down the core, lit through the gaps between the blades: it is how
        // the charge is seen to climb the bore.
        for pair in collars.windows(2) {
            let (x0, x1) = (pair[0] + 2.8, pair[1] - 0.2);
            coil(b, (x0 + x1) * 0.5);
            let r = 2.75 - 0.6 * ((x0 + x1) * 0.5 - 31.0) / 32.0;
            for (dy, dz) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let off = v3(0.0, dy, dz) * r;
                b.beam(
                    axis + X * x0 + off,
                    axis + X * x1 + off,
                    v2(0.7, 0.35),
                    v2(0.7, 0.35),
                );
            }
        }
    }
    // Four swept ceramic blades round the core, open between them.
    for sy in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            b.paint(PLATING);
            b.beam(
                axis + v3(26.0, sy * 5.0, sz * 4.1),
                axis + v3(62.0, sy * 3.2, sz * 2.7),
                v2(3.2, 2.8),
                v2(1.5, 1.3),
            );
        }
        // Prongs either side of the aperture.
        b.paint(PLATING_DARK);
        b.beam(
            axis + v3(54.0, sy * 3.5, 0.0),
            BORE_MUZZLE + v3(0.0, sy * 4.3, 0.0),
            v2(2.4, 6.8),
            v2(1.4, 4.2),
        );
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(BORE_MUZZLE - X * 2.6, BORE_MUZZLE, 2.5, 2.3, hex);
    coil(b, BORE_MUZZLE.x);
    b.cylinder_between(BORE_MUZZLE, BORE_MUZZLE + X * 0.05, 1.3, 1.3, hex);
    if fine {
        // Clamps binding the blades at the collars.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for x in [34.0, 47.0, 60.0] {
            let r = 6.6 - 0.045 * (x - 34.0);
            b.cylinder_between(axis + X * x, axis + X * (x + 0.9), r, r, 12);
        }
    }
}

// ---- fine detail: what tells how big it is ------------------------------------------

/// Deck gear and sensors over the torso: walkways and railings on the pauldrons' decks
/// and along the back of the chest, a sensor mast behind the head.
fn torso_detail(b: &mut MeshBuilder) {
    let (cx, cy, hx, hy, top) = DECK;
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::WALKWAY);
        b.plate(
            v3(cx - 2.0, cy + hy - 2.6, top - 0.02),
            v2(14.0, 2.2),
            0.15,
            0.05,
        );
        railing(
            b,
            v3(cx - hx + 3.0, cy + hy - 1.2, top),
            v3(cx + hx - 5.0, cy + hy - 1.2, top),
            1.3,
        );
        // A hatch low on the chest's back.
        hatch(b, v3(-14.6, 10.5, 72.5), 4.0, 3.0);
    });
    // Railing along the back of the chest top, behind the owner's colour.
    railing(b, v3(-11.4, -10.6, 90.0), v3(-11.4, 10.6, 90.0), 1.3);
    // The sensor mast: a lattice spar off the chest top, arms, a small dish, sensor boxes.
    let base = v3(-8.5, -7.5, 89.8);
    mast(b, base, 13.0);
}

/// A lattice mast `height` tall from `base`: three corner rods braced across, two arms,
/// a dish on the upper arm and sensor boxes on the lower.
fn mast(b: &mut MeshBuilder, base: Vec3, height: f32) {
    let r = 0.55;
    let corners: Vec<Vec3> = (0..3)
        .map(|k| {
            let a = k as f32 * TAU / 3.0;
            v3(a.cos() * r, a.sin() * r, 0.0)
        })
        .collect();
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(base, 6, 1.4, 1.1, 0.8);
    for c in &corners {
        b.cylinder_between(base + *c, base + *c * 0.5 + Vec3::Z * height, 0.12, 0.09, 4);
    }
    let steps = 6;
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        for k in 0..3 {
            let (c0, c1) = (corners[k], corners[(k + 1) % 3]);
            let s = 1.0 - 0.5 * t;
            b.cylinder_between(
                base + c0 * s + Vec3::Z * (height * t),
                base + c1 * s + Vec3::Z * (height * t),
                0.07,
                0.07,
                4,
            );
        }
    }
    b.paint(METAL);
    let upper = base + Vec3::Z * (height * 0.82);
    let lower = base + Vec3::Z * (height * 0.55);
    b.beam(
        upper - Y * 2.2,
        upper + Y * 2.2,
        v2(0.25, 0.25),
        v2(0.25, 0.25),
    );
    b.beam(
        lower - X * 1.8,
        lower + X * 1.8,
        v2(0.25, 0.25),
        v2(0.25, 0.25),
    );
    b.paint(PLATING_DARK);
    b.cuboid(lower + X * 1.9, v3(0.8, 0.9, 1.1));
    b.cuboid(lower - X * 1.9, v3(0.8, 0.9, 1.1));
    // The dish, facing forward off the arm's end.
    b.paint(PLATING);
    b.cylinder_between(
        upper + Y * 2.2 + X * 0.2,
        upper + Y * 2.2 + X * 1.0,
        0.2,
        1.6,
        8,
    );
    b.paint(METAL);
    b.cylinder_between(
        base + Vec3::Z * height,
        base + Vec3::Z * (height + 1.4),
        0.06,
        0.03,
        4,
    );
}

/// Deck gear on the hips: railings along the hip caps, second-layer plates on the tassets,
/// louvres in the rear tasset.
fn pelvis_detail(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.block(v3(13.7, -5.6, 42.0), v3(14.7, 5.6, 51.8));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(-14.6, -5.4, 42.0), v3(-13.7, 5.4, 52.0));
    b.paint(METAL);
    for i in 0..6 {
        let z = 42.6 + 1.4 * i as f32;
        b.block(v3(-15.1, -4.9, z), v3(-14.5, 4.9, z + 0.5));
    }
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(v3(-6.0, -6.0, 36.8), v3(6.0, 6.0, 37.2));
}

/// The Tempest from near: ribs round the drum, a hatch low in the body's flank, bolts along
/// the spine, flutes along the body between its bands, cable runs from the motor.
fn gatling_detail(b: &mut MeshBuilder, y: f32, z: f32, drum: Vec3) {
    b.paint(METAL);
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        let off = v3(a.cos(), 0.0, a.sin()) * 7.45;
        b.beam(
            drum + off - Y * 5.2,
            drum + off + Y * 5.2,
            v2(0.9, 0.6),
            v2(0.9, 0.6),
        );
    }
    b.paint(PLATING).pattern(pattern::PLAIN);
    for k in 0..10 {
        let a = (k as f32 + 0.5) * TAU / 10.0;
        let off = v3(0.0, a.cos() * 8.65, a.sin() * 8.45);
        for (x0, x1) in [(-4.6, 2.8), (5.4, 7.6)] {
            b.beam(
                v3(x0, y, z) + off,
                v3(x1, y, z) + off,
                v2(0.8, 0.5),
                v2(0.8, 0.5),
            );
        }
    }
    hatch_side(b, v3(-2.0, y - 8.4, z - 3.6), 4.0, 3.0);
    b.paint(METAL);
    for x in [-6.0, -1.0, 4.0] {
        b.cuboid(v3(x, y, z + 9.0), v3(0.8, 2.4, 0.4));
    }
    for s in [-1.0f32, 1.0] {
        b.cylinder_between(
            v3(4.0, y + s * 1.8, z - 10.4),
            v3(-12.0, y + s * 1.8, z - 7.4),
            0.5,
            0.5,
            6,
        );
    }
}

/// The Cataclysm's breech from near: cable runs along the capacitor housing, hazard bands
/// round its back, latches on its lid.
fn bore_detail(b: &mut MeshBuilder, y: f32, z: f32) {
    for s in [-1.0f32, 1.0] {
        b.paint(METAL);
        b.cylinder_between(
            v3(-28.0, y + s * 5.2, z + 11.6),
            v3(-9.0, y + s * 4.6, z + 6.6),
            0.6,
            0.6,
            6,
        );
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(
            v3(-30.6, y + s * 7.6 - 0.2, z - 3.0),
            v3(-29.4, y + s * 7.6 + 0.2, z + 10.6),
        );
    }
    b.paint(METAL);
    for x in [-26.0, -21.0] {
        b.cuboid(v3(x, y, z + 11.8), v3(1.2, 4.0, 0.5));
    }
    hatch(b, v3(-31.7, y, z - 1.4), 3.0, 2.2);
}

/// A railing along a deck edge from `a` to `c`: posts about `spacing` apart, a hand rail
/// and a knee rail. Human height at the Behemoth's built size.
fn railing(b: &mut MeshBuilder, a: Vec3, c: Vec3, spacing: f32) {
    let (h, t) = (0.5, 0.16);
    let n = ((a.distance(c) / spacing).ceil() as usize).max(1);
    b.paint(METAL).pattern(pattern::PLAIN);
    for i in 0..=n {
        let p = a.lerp(c, i as f32 / n as f32);
        b.cuboid(p + Vec3::Z * (h * 0.5), v3(t, t, h));
    }
    for z in [h * 0.5, h] {
        b.beam(a + Vec3::Z * z, c + Vec3::Z * z, v2(t, t), v2(t, t));
    }
}

/// A ladder up a wall facing -x from `base` to `top`, `width` across.
fn ladder(b: &mut MeshBuilder, base: Vec3, top: f32, width: f32) {
    b.paint(METAL).pattern(pattern::PLAIN);
    for s in [-0.5, 0.5] {
        b.block(
            base + v3(-0.35, s * width - 0.06, 0.0),
            v3(base.x, base.y + s * width + 0.06, top),
        );
    }
    let mut z = base.z + 0.3;
    while z < top {
        b.block(
            v3(base.x - 0.3, base.y - width * 0.5, z),
            v3(base.x - 0.2, base.y + width * 0.5, z + 0.08),
        );
        z += 0.45;
    }
}

/// A hatch on a wall facing -y at `at`: a dark frame, a light lid, a handle.
fn hatch_side(b: &mut MeshBuilder, at: Vec3, w: f32, h: f32) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        at + v3(-w * 0.5, -0.3, -h * 0.5),
        at + v3(w * 0.5, 0.0, h * 0.5),
    );
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.block(
        at + v3(-w * 0.42, -0.5, -h * 0.4),
        at + v3(w * 0.42, -0.3, h * 0.4),
    );
}

/// A hatch on a wall facing -x at `at`: a dark frame, a light lid, a handle.
fn hatch(b: &mut MeshBuilder, at: Vec3, w: f32, h: f32) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        at + v3(-0.3, -w * 0.5, -h * 0.5),
        at + v3(0.0, w * 0.5, h * 0.5),
    );
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.block(
        at + v3(-0.5, -w * 0.42, -h * 0.4),
        at + v3(-0.3, w * 0.42, h * 0.4),
    );
    b.paint(METAL);
    b.block(at + v3(-0.7, -w * 0.2, -0.1), at + v3(-0.5, w * 0.2, 0.1));
}

// ---- the spent sabot ------------------------------------------------------------------

/// A spent Tempest shell case (mesh "titan_sabot", `aster_t5_titan_sabot`: tumbling as it
/// falls, then scrap): a big bottlenecked cartridge case lying on its side, dark steel,
/// a bright rim and an extractor groove at the base, the primer in its head, a dent in
/// its flank. Authored at 6 x 3 m: 11.6 m long along x, about 3 m across.
pub(crate) fn sabot(b: &mut MeshBuilder, tech: u8) {
    // Authored small (11.6 by 3 m) and stretched to the size of the Tempest's bore at
    // the Behemoth's built size: about 6 m across, 15 long.
    b.with(Affine3A::from_scale(v3(1.29, 2.0, 2.0)), |b| case(b, tech));
}

fn case(b: &mut MeshBuilder, _tech: u8) {
    let r = 1.4;
    // On its side, resting on the rim (the widest thing on it).
    let axis = v3(0.0, 0.0, 1.52);
    if b.coarse() {
        b.paint(METAL);
        b.cylinder_between(axis - X * 6.0, axis + X * 5.6, r, r * 0.75, 6);
        return;
    }
    let fine = b.fine();
    let n = b.sides(14);
    let at = |x: f32| axis + X * x;
    // The head: the rim, the extractor groove, the base, the primer.
    b.paint(PLATING);
    b.cylinder_between(at(-6.0), at(-5.5), 1.5, 1.5, n);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    if fine {
        b.cylinder_between(at(-5.5), at(-5.1), 1.22, 1.22, n);
    }
    b.paint(METAL);
    b.cylinder_between(at(-6.05), at(-6.0), 0.42, 0.42, n);
    // The body, dented on one flank, then the shoulder and the neck.
    let ring = |x: f32, radius: f32, dent: f32| -> Vec<Vec3> {
        (0..n)
            .map(|i| {
                let a = (i as f32 + 0.5) * TAU / n as f32;
                let d = 1.0 - dent * (0.5 + 0.5 * (a - 1.1).cos()).powi(4);
                at(x) + v3(0.0, a.cos(), a.sin()) * radius * d
            })
            .collect()
    };
    b.paint(PLATING_DARK);
    let mut rings = vec![
        ring(if fine { -5.1 } else { -5.5 }, r, 0.0),
        ring(-2.0, r * 0.99, 0.0),
    ];
    if fine {
        rings.push(ring(-0.6, r * 0.985, 0.18));
    }
    rings.extend([
        ring(1.4, r * 0.97, 0.0),
        ring(2.8, r * 0.96, 0.0),
        ring(4.0, r * 0.72, 0.0),
        ring(5.6, r * 0.7, 0.0),
    ]);
    b.loft(&rings, true, true);
    if fine {
        // The mouth, a dark ring at the neck's end, and a stencilled band.
        b.paint(TREAD);
        b.cylinder_between(at(5.6), at(5.65), r * 0.55, r * 0.55, n);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(at(-4.4), at(-3.9), r * 1.01, r * 1.01, n);
    }
}

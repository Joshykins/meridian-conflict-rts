//! Citadel (`aster_t3_point_defense`, mesh "citadel"): the tech 3 rail point defence,
//! authored at blueprint scale (metres, 4x4 lot, radius 20, height 28).
//!
//! A keep built round one heavy rail cannon: the Zenith's little sister, laid flat at
//! the ground instead of up at the sky. Nothing on it is lit (docs/STYLE.md, rail guns
//! are hardware). The armour is built round the gun, not a house the gun pokes out of:
//! the turret turns about a point a quarter of the way down the gun, the firing
//! mechanism behind it, and only a short length of bare rail runs out in front.
//! - Fixed (`part::HULL`): the lot slab; a sloped lower step with its corners cut, a
//!   squat capacitor tower on each cut corner with its conduit run into the plinth, an
//!   octagonal plinth, and on it the armoured gun tower that lifts the turret high
//!   enough for the gun to lay down onto ground close in.
//! - Turning (`part::TURRET`, about the lot's centre): the turntable on the tower's
//!   top, a low armoured carriage with a glacis falling away under the gun's nose and
//!   sponsons down its flanks, and two tall cheeks carrying the trunnion.
//! - Elevating (`rig::ARM_GUN`, about [`TRUNNION`]): the gun body, a long faceted
//!   armoured shell wrapped round the gun from the breech door to a square clamp at
//!   its nose, the capacitor pods strapped along its flanks behind the trunnion,
//!   heat-sink slats on its back. (`rig::ARM_GUN | rig::RECOIL`) the barrel: nothing
//!   but the two rails, straight out of the nose clamp with the slot open between
//!   them, held by square yokes, the last one flush with the muzzle. No round jacket,
//!   no brake.

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern, rig, TurretRail};

/// The trunnion (model space), over the turret's axis: keep `weapons[0].pivot` in
/// structures.ron equal to it, and `weapons[0].muzzle` equal to it plus [`MUZZLE`]
/// along x.
pub(crate) const TRUNNION: Vec3 = Vec3::new(0.0, 0.0, 26.0);

// ---- the gun (barrel frame: metres from the trunnion along the bore) -----------------

/// The gun body: its rear face (the breech door), where its full section starts and
/// ends, where the nose taper ends, and the clamp the rails leave by; half width and
/// half height of the full section, and the corner chamfer.
const BODY_BACK: f32 = -13.0;
const BODY_FULL_FROM: f32 = -11.4;
const BODY_FULL_TO: f32 = 4.5;
const BODY_NOSE: f32 = 11.5;
pub(crate) const BODY_FRONT: f32 = 13.5;
const BODY_HW: f32 = 3.45;
const BODY_HH: f32 = 2.8;
const BODY_CUT: f32 = 0.8;
/// The capacitor pods along the body's flanks behind the trunnion: from, to (x), radius,
/// and the heights (z) of the upper and lower pod.
const POD_FROM: f32 = -12.4;
const POD_TO: f32 = -5.8;
const POD_R: f32 = 0.85;
const PODS_Z: [f32; 2] = [0.95, -0.95];
/// The rails: each rail's centre off the bore (±y), its half width and half height, and
/// how far back inside the body they start.
const RAIL_Y: f32 = 1.1;
const RAIL_HW: f32 = 0.42;
const RAIL_HH: f32 = 1.15;
const RAILS_FROM: f32 = BODY_FRONT - 3.0;
/// The yokes clamping the bare rails; the last sits flush with the muzzle.
const YOKES: [f32; 3] = [17.9, 22.5, 27.1];
/// The muzzle face.
pub(crate) const MUZZLE: f32 = 34.0;
const RECOIL: f32 = 1.2;
// The turret turns about a point a quarter of the way down the gun.
const _: () =
    assert!(-BODY_BACK > 0.22 * (MUZZLE - BODY_BACK) && -BODY_BACK < 0.35 * (MUZZLE - BODY_BACK));
/// The breech mouth on the body's back face (half width, half height); the door's hinge,
/// across its top behind the face; and how far it swings open (up and back).
const BREECH_MOUTH: Vec2 = Vec2::new(1.75, 1.4);
const BREECH_HINGE: Vec3 = Vec3::new(BODY_BACK - 0.3, 0.0, 1.75);
const BREECH_OPEN: f32 = -1.35;

// ---- the carriage (model space) ---------------------------------------------------

/// The turret's collar: the deck height its shoulder rises to (plus 0.7), and the
/// shoulder's radius there.
const DECK: f32 = 19.6;
const COLLAR_SHOULDER: f32 = 7.6;
/// The collar's skirt radius, over the tower's lip.
const COLLAR_R: f32 = 10.3;
/// The carriage: the top of its flat between the cheeks, which runs from a glacis in
/// front down to a low back deck; its plan at its foot (front, rear, half width).
const CARRIAGE_TOP: f32 = 21.2;
const CARRIAGE_FLAT: [f32; 2] = [-4.0, 1.5];
const CARRIAGE_FRONT: f32 = 5.5;
const CARRIAGE_REAR: f32 = -9.0;
const CARRIAGE_HW: f32 = 7.0;
/// The cheeks carrying the trunnion: inner face (y), thickness, half length at the foot.
const CHEEK_IN: f32 = 3.7;
const CHEEK_T: f32 = 2.2;
const CHEEK_FOOT: f32 = 4.8;
/// How far below level the gun lays before it touches its own carriage or keep (a test
/// checks it): the tower is tall so the gun reaches down onto ground close in.
#[cfg(test)]
const DEPRESSION_DEG: f32 = 22.0;
// The gun body clears the carriage deck when level.
const _: () = assert!(TRUNNION.z - BODY_HH > CARRIAGE_TOP);

// ---- the base ----------------------------------------------------------------------

/// The slab's half width and top.
const SLAB: f32 = 23.2;
const SLAB_TOP: f32 = 0.5;
/// The lower step: half width, corner chamfer, top.
const FOUND: f32 = 20.0;
const FOUND_CUT: f32 = 7.0;
const FOUND_TOP: f32 = 4.2;
/// The upper plinth: radius (to its corners) at its foot, and its top.
const PLINTH_R: f32 = 14.5;
const PLINTH_TOP: f32 = 8.0;
/// The gun tower standing on the plinth: its top, under the turntable.
const KEEP_TOP: f32 = 19.0;
/// The capacitor towers on the cut corners: centre (±, ±), half width, top.
const TOWER_C: f32 = 17.2;
const TOWER_HW: f32 = 3.9;
const TOWER_TOP: f32 = 10.5;

pub(crate) fn citadel(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(TRUNNION);
    b.set_recoil(TRUNNION, TRUNNION + Vec3::X, RECOIL);
    if b.coarse() {
        coarse(b);
        return;
    }
    slab(b);
    plinth(b);
    keep(b);
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        tower(b, v2(sx, sy));
    }
    b.with_part(part::TURRET, |b| {
        collar(b);
        carriage(b);
        b.with_limb(rig::ARM_GUN, |b| b.at(TRUNNION, gun_body));
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| b.at(TRUNNION, barrel));
    });
}

/// Far off: the keep, the carriage, the gun as one bar from the breech door to the
/// muzzle, and the owner's colour on its back. Under 60 triangles.
fn coarse(b: &mut MeshBuilder) {
    // The keep and the gun tower as one block, open underneath.
    b.paint(PLATING);
    let ring = |half: f32, z: f32| {
        vec![
            v3(half, -half, z),
            v3(half, half, z),
            v3(-half, half, z),
            v3(-half, -half, z),
        ]
    };
    b.loft(
        &[
            ring(FOUND, 0.0),
            ring(12.0, PLINTH_TOP),
            ring(COLLAR_SHOULDER, DECK),
        ],
        false,
        true,
    );
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let length = CARRIAGE_FRONT - CARRIAGE_REAR;
        b.frustum_open(
            v3((CARRIAGE_FRONT + CARRIAGE_REAR) * 0.5, 0.0, DECK),
            v2(length, 2.0 * CARRIAGE_HW),
            v2(length - 5.0, 2.0 * CARRIAGE_HW - 2.0),
            CARRIAGE_TOP - DECK,
            Vec2::ZERO,
        );
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
            b.at(TRUNNION, |b| {
                b.paint(PLATING);
                b.loft(
                    &[
                        square(BODY_BACK, BODY_HW, BODY_HH),
                        square(BODY_FULL_TO, BODY_HW, BODY_HH),
                        square(BODY_FRONT, 1.5, 1.5),
                        square(MUZZLE, 1.3, 1.2),
                    ],
                    true,
                    true,
                );
                team_panel(b, v3(-5.0, 0.0, BODY_HH), v2(3.0, 3.6));
            });
        });
    });
}

/// A square ring across the bore at `x`, for the coarse bar.
fn square(x: f32, hw: f32, hh: f32) -> Vec<Vec3> {
    vec![
        v3(x, -hw, -hh),
        v3(x, hw, -hh),
        v3(x, hw, hh),
        v3(x, -hw, hh),
    ]
}

// ---- the base ----------------------------------------------------------------------

/// The lot's slab: a dark foot and a light deck.
fn slab(b: &mut MeshBuilder) {
    let plan = chamfered_rect(v2(SLAB, SLAB), 3.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(0.25, 1.0)]);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(0.25, 0.99), Section::new(SLAB_TOP, 0.98)],
    );
}

/// The fixed keep: a sloped lower step with its corners cut, a dark trim, the
/// octagonal plinth stepped in on it, and the race ring on top.
fn plinth(b: &mut MeshBuilder) {
    let fine = b.fine();
    let lower = chamfered_rect(v2(FOUND, FOUND), FOUND_CUT);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &lower,
        &[Section::new(SLAB_TOP - 0.1, 1.0), Section::new(1.3, 0.995)],
    );
    b.paint(PLATING);
    b.loft_z(
        &lower,
        &[Section::new(1.2, 0.985), Section::new(FOUND_TOP, 0.925)],
    );
    let upper = ngon(8, PLINTH_R);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &upper,
        &[
            Section::new(FOUND_TOP - 0.1, 1.0),
            Section::new(FOUND_TOP + 0.7, 0.99),
        ],
    );
    b.paint(PLATING);
    b.loft_z(
        &upper,
        &[
            Section::new(FOUND_TOP + 0.6, 0.975),
            Section::new(PLINTH_TOP, 0.9),
        ],
    );
    let sides = b.sides(16);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(0.0, 0.0, PLINTH_TOP - 0.15), sides, 12.9, 12.7, 0.45);
    if !fine {
        return;
    }
    // A dark trim round the lower step's top edge, and an armoured door in each face
    // between the towers.
    b.loft_z(
        &lower,
        &[
            Section::new(FOUND_TOP - 0.05, 0.93),
            Section::new(FOUND_TOP + 0.25, 0.925),
        ],
    );
    for k in 0..4 {
        let yaw = std::f32::consts::FRAC_PI_2 * k as f32;
        b.yawed(Vec3::ZERO, yaw, |b| {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.block(v3(19.2, -2.4, SLAB_TOP), v3(19.9, 2.4, 3.4));
            b.paint(PLATING_DARK);
            b.block(v3(19.85, -1.9, SLAB_TOP + 0.1), v3(20.1, 1.9, 3.0));
        });
    }
}

/// One capacitor tower on a cut corner (`corner` is (±1, ±1)): a squat sloped
/// blockhouse, dark roof with a raised can housing, louvres, and a conduit trunk over
/// the lower step into the plinth. The tower is built about its own centre, so its walls
/// slope in evenly and what sits on them stays on them.
fn tower(b: &mut MeshBuilder, corner: Vec2) {
    let c = corner * TOWER_C;
    b.at(c.extend(0.0), |b| tower_body(b, corner));
    // The conduit trunk into the plinth.
    let inward = -corner.normalize();
    let from = c + inward * (TOWER_HW * 0.8);
    let into = inward * -(PLINTH_R * 0.8);
    b.paint(PLATING);
    b.beam(
        from.extend(FOUND_TOP + 1.2),
        into.extend(FOUND_TOP + 1.8),
        v2(2.6, 2.2),
        v2(2.6, 2.2),
    );
    if !b.fine() {
        return;
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        from.extend(FOUND_TOP + 2.45),
        into.extend(FOUND_TOP + 3.05),
        v2(1.8, 0.3),
        v2(1.8, 0.3),
    );
    // Feeders laid along the trunk.
    let side = v2(-inward.y, inward.x);
    for k in [-1.0, 1.0] {
        let o = side * (k * 1.5);
        cable(
            b,
            &[
                (from + o).extend(FOUND_TOP + 0.3),
                (from + inward * 2.0 + o).extend(FOUND_TOP + 0.8),
                (into + o).extend(FOUND_TOP + 1.0),
            ],
            0.28,
        );
    }
}

/// The tower's walls (scale 0.99 at their foot to 0.86 near the roof, then 0.8 at the
/// roof), at the given height.
fn tower_wall_scale(z: f32) -> f32 {
    let t = ((z - 1.3) / (TOWER_TOP - 0.7 - 1.3)).clamp(0.0, 1.0);
    0.99 + (0.86 - 0.99) * t
}

/// The tower itself, in a frame at its centre.
fn tower_body(b: &mut MeshBuilder, corner: Vec2) {
    let fine = b.fine();
    let plan = chamfered_rect(v2(TOWER_HW, TOWER_HW), 1.1);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &plan,
        &[Section::new(SLAB_TOP - 0.1, 1.0), Section::new(1.4, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(1.3, 0.99),
            Section::new(TOWER_TOP - 0.7, 0.86),
            Section::new(TOWER_TOP, 0.8),
        ],
    );
    b.paint(PLATING_DARK);
    b.decal(
        v3(0.0, 0.0, TOWER_TOP + 0.02),
        v2(TOWER_HW * 1.3, TOWER_HW * 1.3),
    );
    let inward = -corner.normalize();
    team_panel(b, (-inward * 0.9).extend(TOWER_TOP), v2(1.8, 1.8));
    if !fine {
        return;
    }
    // A low hatch on the roof's inner half with dark slats across it.
    let hatch = inward * 1.2;
    b.paint(PLATING);
    b.chamfered_box(hatch.extend(TOWER_TOP + 0.35), v3(3.2, 3.2, 0.7), 0.25);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for k in -1..=1 {
        let o = hatch + v2(0.0, k as f32 * 0.9);
        b.block(
            v3(o.x - 1.3, o.y - 0.18, TOWER_TOP + 0.7),
            v3(o.x + 1.3, o.y + 0.18, TOWER_TOP + 0.85),
        );
    }
    // Louvres down each outward face, each slat's back set into the sloping wall.
    for (axis, sign) in [(0usize, corner.x), (1usize, corner.y)] {
        for k in 0..3 {
            let z = 3.0 + 1.3 * k as f32;
            let back = TOWER_HW * tower_wall_scale(z + 0.45) - 0.1;
            let front = TOWER_HW * tower_wall_scale(z) + 0.3;
            let (lo, hi) = (sign * back, sign * front);
            let (lo, hi) = (lo.min(hi), lo.max(hi));
            let (a, e) = if axis == 0 {
                (v3(lo, -1.8, z), v3(hi, 1.8, z + 0.45))
            } else {
                (v3(-1.8, lo, z), v3(1.8, hi, z + 0.45))
            };
            b.block(a, e);
        }
    }
}

// ---- the turret ---------------------------------------------------------------------

/// The turret's collar on the tower's top: a round armoured skirt dropped over the
/// tower's lip, and a sloped shoulder up from it to the deck the carriage stands on, low
/// enough in front for the gun's nose to dip past it.
fn collar(b: &mut MeshBuilder) {
    let sides = b.sides(16);
    let r = COLLAR_R;
    let ring = ngon(sides, r);
    b.paint(PLATING);
    b.loft_z(
        &ring,
        &[
            Section::new(KEEP_TOP - 1.2, 1.0),
            Section::new(KEEP_TOP + 0.2, 1.0),
            Section::new(DECK + 0.7, COLLAR_SHOULDER / r),
        ],
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &ring,
        &[
            Section::new(KEEP_TOP - 1.25, 1.008),
            Section::new(KEEP_TOP - 0.85, 1.008),
        ],
    );
    if b.fine() {
        b.paint(METAL).pattern(pattern::PLAIN);
        b.loft_z(
            &ring,
            &[
                Section::new(KEEP_TOP + 0.1, 1.006),
                Section::new(KEEP_TOP + 0.3, 1.006),
            ],
        );
    }
}

/// The carriage: a low armoured hull under the gun, a glacis in front falling away from
/// the gun's nose so it can lay down, a low back deck under the breech, sponsons down
/// its flanks, and the two tall cheeks that carry the gun.
fn carriage(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (f, r, hw) = (CARRIAGE_FRONT, CARRIAGE_REAR, CARRIAGE_HW);
    let low = DECK + 0.7;
    b.paint(PLATING);
    // The hull over it: glacis, flat, back deck.
    let [flat_back, flat_front] = CARRIAGE_FLAT;
    b.extrude_y(
        &[
            [r + 0.6, low - 0.1],
            [f - 0.4, low - 0.1],
            [flat_front, CARRIAGE_TOP],
            [flat_back, CARRIAGE_TOP],
            [r + 1.6, low + 0.7],
        ],
        -(hw - 1.4),
        hw - 1.4,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.decal(
        v3((flat_back + flat_front) * 0.5, 0.0, CARRIAGE_TOP + 0.02),
        v2(flat_front - flat_back - 0.6, 2.0 * CHEEK_IN - 0.4),
    );
    let sponson_top = low + 0.9;
    team_panel(b, v3(-4.5, hw - 0.6, sponson_top), v2(2.4, 1.2));
    team_panel(b, v3(-4.5, -(hw - 0.6), sponson_top), v2(2.4, 1.2));
    b.mirror_y(|b| {
        // A low sponson down the flank, a light lid on it.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.extrude_y(
            &[
                [r + 1.8, KEEP_TOP + 0.3],
                [f - 3.4, KEEP_TOP + 0.3],
                [f - 4.2, sponson_top],
                [r + 2.4, sponson_top],
            ],
            hw - 1.5,
            hw + 0.4,
        );
        b.paint(PLATING);
        b.plate(
            v3((f + r) * 0.5 - 1.5, hw - 0.6, sponson_top),
            v2(f - r - 9.0, 1.4),
            0.14,
            0.04,
        );
        // The cheek: a tall sloped armoured wall up to the trunnion, a dark boss on it.
        b.paint(PLATING);
        b.extrude_y(
            &[
                [-CHEEK_FOOT, low - 0.1],
                [CHEEK_FOOT, low - 0.1],
                [2.6, TRUNNION.z + 1.7],
                [-2.6, TRUNNION.z + 1.7],
            ],
            CHEEK_IN,
            CHEEK_IN + CHEEK_T,
        );
        let out = CHEEK_IN + CHEEK_T;
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            v3(0.0, out - 0.1, TRUNNION.z),
            v3(0.0, out + 0.7, TRUNNION.z),
            1.5,
            1.3,
            b.sides(12),
        );
        if fine {
            b.paint(METAL).pattern(pattern::PLAIN);
            b.cylinder_between(
                v3(0.0, out + 0.7, TRUNNION.z),
                v3(0.0, out + 1.0, TRUNNION.z),
                0.8,
                0.6,
                8,
            );
            // A dark rib up the cheek's face, and vision slits along the sponson.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.block(
                v3(-0.4, out - 0.05, low + 0.4),
                v3(0.4, out + 0.15, TRUNNION.z - 1.6),
            );
            b.paint(TREAD).pattern(pattern::NONE);
            for x in [-5.5, -1.5] {
                b.block(
                    v3(x - 1.2, hw + 0.38, low + 0.15),
                    v3(x + 1.2, hw + 0.46, low + 0.5),
                );
            }
        }
    });
    if fine {
        antenna_unlit(b, v3(-7.0, 4.6, low + 0.5), 3.8, 0.12);
    }
}

// ---- the gun tower ------------------------------------------------------------------

/// A frame on a sloping face of the tower: `yaw` is the face's outward direction, the
/// face runs from `foot` out from the axis at `z0` to `top` out at `z1`, and `f` draws at
/// height `z` with +x out of the face, y across it and +z up its slope.
#[expect(
    clippy::too_many_arguments,
    reason = "a face is its bearing, two stations and the height drawn at"
)]
fn on_face(
    b: &mut MeshBuilder,
    yaw: f32,
    foot: f32,
    top: f32,
    z0: f32,
    z1: f32,
    z: f32,
    f: impl FnOnce(&mut MeshBuilder),
) {
    let t = (z - z0) / (z1 - z0);
    let out = foot + (top - foot) * t;
    let lean = ((foot - top) / (z1 - z0)).atan();
    b.with(
        glam::Affine3A::from_rotation_z(yaw)
            * glam::Affine3A::from_translation(v3(out, 0.0, z))
            * glam::Affine3A::from_rotation_y(-lean),
        f,
    );
}

/// The tower the gun stands on, from the plinth's top to the turret's collar: an
/// octagonal drum in two slopes, a steep glacis up from the plinth and a near-sheer
/// wall above, a dark band where they meet, ribs up its corners and slits in its faces.
fn keep(b: &mut MeshBuilder) {
    let fine = b.fine();
    const R: f32 = 12.6;
    const BAND: f32 = 12.6;
    let ring = ngon(8, R);
    let (foot, waist, top) = (1.0, 0.9, 0.8);
    b.paint(PLATING);
    b.loft_z(
        &ring,
        &[
            Section::new(PLINTH_TOP - 0.1, foot),
            Section::new(BAND, waist),
            Section::new(KEEP_TOP - 0.6, top),
        ],
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &ring,
        &[
            Section::new(BAND - 0.4, waist + 0.026),
            Section::new(BAND + 0.4, waist + 0.016),
        ],
    );
    b.loft_z(
        &ring,
        &[
            Section::new(KEEP_TOP - 0.7, top + 0.01),
            Section::new(KEEP_TOP, top - 0.02),
        ],
    );
    if !fine {
        return;
    }
    // Ribs up the eight corners, on the upper wall.
    let apothem = |s: f32| R * s * (std::f32::consts::PI / 8.0).cos();
    for k in 0..8 {
        let a = (k as f32 + 0.5) * std::f32::consts::FRAC_PI_4;
        let dir = v2(a.cos(), a.sin());
        b.paint(PLATING_DARK);
        b.beam(
            (dir * (R * waist + 0.1)).extend(BAND + 0.4),
            (dir * (R * top + 0.1)).extend(KEEP_TOP - 0.7),
            v2(0.7, 0.7),
            v2(0.7, 0.7),
        );
    }
    // A slit and an armour plate on each face of the upper wall.
    for k in 0..8 {
        let yaw = k as f32 * std::f32::consts::FRAC_PI_4;
        let (lo, hi) = (BAND, KEEP_TOP - 0.6);
        on_face(b, yaw, apothem(waist), apothem(top), lo, hi, 15.8, |b| {
            if k % 2 == 0 {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.block(v3(-0.1, -2.2, -0.9), v3(0.12, 2.2, 0.9));
                b.paint(TREAD).pattern(pattern::NONE);
                b.block(v3(0.1, -1.6, -0.2), v3(0.16, 1.6, 0.2));
            } else {
                b.paint(PLATING_DARK);
                b.block(v3(-0.1, -1.4, -1.6), v3(0.15, 1.4, 1.6));
            }
        });
    }
}

/// A chamfered rectangle across the bore at `x` (half width, half height, corner cut),
/// wound like [`square`].
fn section(x: f32, hw: f32, hh: f32, cut: f32) -> Vec<Vec3> {
    vec![
        v3(x, -hw + cut, -hh),
        v3(x, hw - cut, -hh),
        v3(x, hw, -hh + cut),
        v3(x, hw, hh - cut),
        v3(x, hw - cut, hh),
        v3(x, -hw + cut, hh),
        v3(x, -hw, hh - cut),
        v3(x, -hw, -hh + cut),
    ]
}

// ---- the gun (barrel frame: origin at the trunnion, +x down the bore) ----------------

/// The gun body: the armour wrapped round the gun, the firing mechanism behind the
/// trunnion (breech door, capacitor pods, heat-sink slats), a dark band at the
/// trunnion and a square clamp round the rails at its nose.
fn gun_body(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (hw, hh, cut) = (BODY_HW, BODY_HH, BODY_CUT);
    b.paint(PLATING);
    b.loft(
        &[
            section(BODY_BACK, hw - 0.5, hh - 0.45, cut),
            section(BODY_FULL_FROM, hw, hh, cut),
            section(BODY_FULL_TO, hw, hh, cut),
            section(BODY_NOSE, hw - 0.7, hh - 0.55, cut),
            section(BODY_FRONT, RAIL_Y + RAIL_HW + 0.5, RAIL_HH + 0.5, 0.4),
        ],
        true,
        true,
    );
    // A dark band round the body at the trunnion, and the clamp the rails leave by.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[
            section(-1.3, hw + 0.06, hh + 0.06, cut),
            section(1.3, hw + 0.06, hh + 0.06, cut),
        ],
        false,
        false,
    );
    b.chamfered_box(
        v3(BODY_FRONT, 0.0, 0.0),
        v3(0.9, 2.0 * (RAIL_Y + RAIL_HW + 0.45), 2.0 * (RAIL_HH + 0.45)),
        0.15,
    );
    breech(b);
    team_panel(b, v3(-1.5, 0.0, hh), v2(2.2, 3.0));
    // The capacitor pods, two a side, strapped along the flanks behind the trunnion.
    let sides = if fine { 10 } else { 6 };
    b.mirror_y(|b| {
        let y = hw + POD_R - 0.25;
        for z in PODS_Z {
            b.paint(PLATING);
            b.cylinder_between(v3(POD_FROM, y, z), v3(POD_TO, y, z), POD_R, POD_R, sides);
            if fine {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                for (x0, x1) in [
                    (POD_FROM - 0.1, POD_FROM + 0.35),
                    (POD_TO - 0.35, POD_TO + 0.1),
                ] {
                    b.cylinder_between(v3(x0, y, z), v3(x1, y, z), POD_R + 0.07, POD_R + 0.07, 10);
                }
            }
        }
        if fine {
            // Straps over both pods, and the bus from each pod's front into the body.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for x in [-10.6, -7.6] {
                b.block(
                    v3(x - 0.25, hw - 0.1, PODS_Z[1] - POD_R - 0.1),
                    v3(x + 0.25, y + POD_R + 0.1, PODS_Z[0] + POD_R + 0.1),
                );
            }
            for z in PODS_Z {
                cable(
                    b,
                    &[
                        v3(POD_TO + 0.05, y, z),
                        v3(POD_TO + 1.0, y - 0.1, z * 0.8),
                        v3(-3.2, hw + 0.05, z * 0.6),
                    ],
                    0.22,
                );
            }
        }
    });
    if !fine {
        return;
    }
    // Heat-sink slats across the back behind the trunnion.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for k in 0..6 {
        let x = -10.4 + 1.15 * k as f32;
        b.block(
            v3(x, -(hw - 1.0), hh - 0.05),
            v3(x + 0.45, hw - 1.0, hh + 0.25),
        );
    }
    // The rangefinder: a low dark box on the fore back, off the axis, its slit darker.
    let at = v3(6.2, -1.4, hh - 0.35);
    b.chamfered_box(at + v3(0.0, 0.0, 0.55), v3(2.6, 1.4, 1.1), 0.25);
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(at + v3(1.28, -0.5, 0.6), at + v3(1.36, 0.5, 0.9));
    // Bolted plates down the flanks ahead of the cheeks.
    b.paint(PLATING_DARK);
    b.mirror_y(|b| b.block(v3(5.2, hw - 0.3, -1.2), v3(9.0, hw - 0.1, 1.0)));
}

/// The breech on the body's back face: the dark mouth the spent cartridge comes out of, a
/// steel frame round it, the hinge knuckles, and the door, which swings up and back on its
/// hinge as the gun kicks and shuts as it runs out (`rig::BREECH`).
fn breech(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (mw, mh) = (BREECH_MOUTH.x, BREECH_MOUTH.y);
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(v3(BODY_BACK - 0.02, -mw, -mh), v3(BODY_BACK + 0.02, mw, mh));
    if fine {
        b.paint(METAL).pattern(pattern::PLAIN);
        for (y0, y1, z0, z1) in [
            (-mw - 0.2, mw + 0.2, mh, mh + 0.2),
            (-mw - 0.2, mw + 0.2, -mh - 0.2, -mh),
            (mw, mw + 0.2, -mh, mh),
            (-mw - 0.2, -mw, -mh, mh),
        ] {
            b.block(v3(BODY_BACK - 0.06, y0, z0), v3(BODY_BACK + 0.02, y1, z1));
        }
    }
    // The knuckles on the body either side of the door's own.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for (y0, y1) in [(-mw - 0.3, -mw + 0.35), (mw - 0.35, mw + 0.3)] {
        b.cylinder_between(
            v3(BREECH_HINGE.x, y0, BREECH_HINGE.z),
            v3(BREECH_HINGE.x, y1, BREECH_HINGE.z),
            0.26,
            0.26,
            8,
        );
    }
    b.with_breech(BREECH_HINGE, BREECH_OPEN, |b| {
        // The door: a thick armoured plate hanging from the hinge over the mouth.
        let (top, bottom) = (BREECH_HINGE.z, -mh - 0.3);
        b.paint(PLATING);
        b.chamfered_box(
            v3(BREECH_HINGE.x, 0.0, (top + bottom) * 0.5),
            v3(0.5, 2.0 * mw + 0.3, top - bottom),
            0.12,
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            v3(BREECH_HINGE.x, -mw + 0.4, top),
            v3(BREECH_HINGE.x, mw - 0.4, top),
            0.24,
            0.24,
            8,
        );
        if fine {
            // A locking bar across it and dark bolt strips down it.
            b.block(
                v3(BREECH_HINGE.x - 0.33, -mw + 0.2, -0.5),
                v3(BREECH_HINGE.x - 0.22, mw - 0.2, -0.1),
            );
            b.paint(PLATING_DARK);
            b.mirror_y(|b| {
                b.block(
                    v3(BREECH_HINGE.x - 0.3, mw - 0.6, bottom + 0.3),
                    v3(BREECH_HINGE.x - 0.24, mw - 0.35, top - 0.4),
                )
            });
        }
    });
}

/// The barrel: see the module notes. The rails start inside the body, so the kick
/// shows no gap.
fn barrel(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (outer, inner) = (RAIL_Y + RAIL_HW, RAIL_Y - RAIL_HW);
    // The rails: two light bars bevelled on their outer edges, the slot open.
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let (h, bevel) = (RAIL_HH, 0.14);
        let profile = [
            [inner, -h],
            [outer - bevel, -h],
            [outer, -h + bevel],
            [outer, h - bevel],
            [outer - bevel, h],
            [inner, h],
        ];
        b.extrude_x(&profile, RAILS_FROM, MUZZLE - 0.05);
    });
    // The yokes: square dark clamps across both rails, the last flush with the muzzle.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let size = v3(0.7, 2.0 * (outer + 0.18), 2.0 * (RAIL_HH + 0.18));
    for (i, x) in YOKES.into_iter().enumerate() {
        if fine || i == 1 {
            b.chamfered_box(v3(x, 0.0, 0.0), size, 0.1);
        }
    }
    b.chamfered_box(v3(MUZZLE - 0.35, 0.0, 0.0), size, 0.1);
    // The bore dark in the last yoke.
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(
        v3(MUZZLE - 0.02, -inner, -(RAIL_HH - 0.1)),
        v3(MUZZLE + 0.02, inner, RAIL_HH - 0.1),
    );
}

// ---- the spent cartridge ----------------------------------------------------------

/// The Citadel's spent rail cartridge (mesh "citadel_casing", `aster_t3_citadel_casing`):
/// tumbling out of the breech, then lying where it fell as scrap. The squared case the
/// armature rode in, on its side: a bright base rim, a dark body with the contact strips
/// down its flanks where the rails bit, a stepped nose with the mouth dark in it.
/// Authored at blueprint scale (radius 2.2, height 1.3): 3.8 m long, 1.3 m over the rim.
pub(crate) fn casing(b: &mut MeshBuilder, _tech: u8) {
    b.at(v3(0.0, 0.0, 0.66), |b| {
        if b.coarse() {
            b.paint(PLATING_DARK);
            b.loft(
                &[square(-1.9, 0.62, 0.62), square(1.9, 0.5, 0.5)],
                true,
                true,
            );
            return;
        }
        if !b.fine() {
            // Rim, body and nose as one piece.
            b.paint(PLATING_DARK);
            b.loft(
                &[
                    section(-1.9, 0.64, 0.64, 0.18),
                    section(1.1, 0.6, 0.6, 0.2),
                    section(1.9, 0.44, 0.44, 0.14),
                ],
                true,
                true,
            );
            return;
        }
        b.paint(PLATING_DARK);
        b.loft(
            &[
                section(-1.6, 0.6, 0.6, 0.2),
                section(1.1, 0.6, 0.6, 0.2),
                section(1.5, 0.48, 0.48, 0.16),
                section(1.9, 0.44, 0.44, 0.14),
            ],
            false,
            true,
        );
        b.paint(PLATING);
        b.loft(
            &[
                section(-1.9, 0.66, 0.66, 0.16),
                section(-1.6, 0.66, 0.66, 0.16),
            ],
            true,
            true,
        );
        // The contact strips, rail-scorched steel, and a dark band by the rim.
        b.paint(METAL).pattern(pattern::PLAIN);
        b.mirror_y(|b| b.block(v3(-1.4, 0.58, -0.28), v3(1.0, 0.64, 0.28)));
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft(
            &[
                section(-1.35, 0.62, 0.62, 0.2),
                section(-1.15, 0.62, 0.62, 0.2),
            ],
            false,
            false,
        );
        b.paint(TREAD).pattern(pattern::NONE);
        b.block(v3(1.9, -0.25, -0.25), v3(1.92, 0.25, 0.25));
    });
}

// ---- kit ---------------------------------------------------------------------------

/// A heavy cable through `points`, a sleeve at each bend.
fn cable(b: &mut MeshBuilder, points: &[Vec3], radius: f32) {
    for pair in points.windows(2) {
        b.cylinder_between(pair[0], pair[1], radius, radius, 6);
    }
    for &p in &points[1..points.len() - 1] {
        b.spheroid(p, Vec3::splat(radius * 1.2), 6, 2);
    }
}

/// Where the charge's arcs crawl (`TurretRail`): along the rail tops in the open lengths
/// between the clamps, the last stretch (the longest) three times over.
pub(crate) const RAIL: TurretRail = TurretRail {
    breech: BODY_BACK - TRUNNION.x,
    muzzle: MUZZLE - TRUNNION.x,
    rail_y: RAIL_Y,
    rail_top: RAIL_HH,
    arcs: [
        (BODY_FRONT + YOKES[0]) * 0.5 - TRUNNION.x,
        (YOKES[0] + YOKES[1]) * 0.5 - TRUNNION.x,
        (YOKES[1] + YOKES[2]) * 0.5 - TRUNNION.x,
        YOKES[2] + (MUZZLE - YOKES[2]) * 0.2 - TRUNNION.x,
        YOKES[2] + (MUZZLE - YOKES[2]) * 0.5 - TRUNNION.x,
        YOKES[2] + (MUZZLE - YOKES[2]) * 0.8 - TRUNNION.x,
    ],
    arc_half: 2.0,
};

#[cfg(test)]
mod tests;

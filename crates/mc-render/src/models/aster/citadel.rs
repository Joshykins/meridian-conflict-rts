//! Citadel (`aster_t3_point_defense`, mesh "citadel"): the tech 3 rail point defence,
//! authored at blueprint scale (metres, 4x4 lot, radius 20, height 17).
//!
//! A keep built round one heavy rail cannon: the Zenith's little sister, laid flat at
//! the ground instead of up at the sky. Nothing on it is lit (docs/STYLE.md, rail guns
//! are hardware). The armour is built round the gun, not a house the gun pokes out of:
//! the turret turns about a point a quarter of the way down the gun, the firing
//! mechanism behind it, and only a short length of bare rail runs out in front.
//! - Fixed (`part::HULL`): the lot slab; a sloped lower step with its corners cut, a
//!   squat capacitor tower on each cut corner with its conduit run into the plinth, an
//!   octagonal plinth, and the race ring the turret turns on.
//! - Turning (`part::TURRET`, about the lot's centre): the turntable, a low armoured
//!   carriage with sponsons down its flanks, and two cheeks carrying the trunnion.
//! - Elevating (`rig::ARM_GUN`, about [`TRUNNION`]): the gun body, a long faceted
//!   armoured shell wrapped round the gun from the breech door to a square clamp at
//!   its nose, the capacitor pods strapped along its flanks behind the trunnion,
//!   heat-sink slats on its back. (`rig::ARM_GUN | rig::RECOIL`) the barrel: nothing
//!   but the two rails, straight out of the nose clamp with the slot open between
//!   them, held by square yokes, the last one flush with the muzzle. No round jacket,
//!   no brake.

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

/// The trunnion (model space), over the turret's axis: keep `weapons[0].pivot` in
/// structures.ron equal to it, and `weapons[0].muzzle` equal to it plus [`MUZZLE`]
/// along x.
pub(crate) const TRUNNION: Vec3 = Vec3::new(0.0, 0.0, 13.0);

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

// ---- the carriage (model space) ---------------------------------------------------

/// The turntable's top, where the carriage stands, and the carriage's deck.
const DECK: f32 = 8.6;
const CARRIAGE_TOP: f32 = 10.0;
/// The carriage's plan at its foot: front, rear, half width.
const CARRIAGE_FRONT: f32 = 9.5;
const CARRIAGE_REAR: f32 = -13.5;
const CARRIAGE_HW: f32 = 8.8;
/// The cheeks carrying the trunnion: inner face (y), thickness, half length at the foot.
const CHEEK_IN: f32 = 3.7;
const CHEEK_T: f32 = 2.2;
const CHEEK_FOOT: f32 = 4.6;
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
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        tower(b, v2(sx, sy));
    }
    b.with_part(part::TURRET, |b| {
        turntable(b);
        carriage(b);
        b.with_limb(rig::ARM_GUN, |b| b.at(TRUNNION, gun_body));
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| b.at(TRUNNION, barrel));
    });
}

/// Far off: the keep, the carriage, the gun as one bar from the breech door to the
/// muzzle, and the owner's colour on its back. Under 60 triangles.
fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.frustum_open(
        Vec3::ZERO,
        v2(2.0 * FOUND, 2.0 * FOUND),
        v2(2.0 * PLINTH_R, 2.0 * PLINTH_R),
        PLINTH_TOP,
        Vec2::ZERO,
    );
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let length = CARRIAGE_FRONT - CARRIAGE_REAR;
        b.frustum_open(
            v3((CARRIAGE_FRONT + CARRIAGE_REAR) * 0.5, 0.0, DECK),
            v2(length, 2.0 * CARRIAGE_HW),
            v2(length - 2.0, 2.0 * CARRIAGE_HW - 2.0),
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
/// the lower step into the plinth.
fn tower(b: &mut MeshBuilder, corner: Vec2) {
    let fine = b.fine();
    let c = corner * TOWER_C;
    let plan: Vec<[f32; 2]> = chamfered_rect(v2(TOWER_HW, TOWER_HW), 1.1)
        .iter()
        .map(|p| [p[0] + c.x, p[1] + c.y])
        .collect();
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
        c.extend(TOWER_TOP + 0.02),
        v2(TOWER_HW * 1.3, TOWER_HW * 1.3),
    );
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
    team_panel(b, (c - inward * 0.9).extend(TOWER_TOP), v2(1.8, 1.8));
    if !fine {
        return;
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        from.extend(FOUND_TOP + 2.45),
        into.extend(FOUND_TOP + 3.05),
        v2(1.8, 0.3),
        v2(1.8, 0.3),
    );
    // A low hatch on the roof's inner half with dark slats across it.
    let hatch = c + inward * 1.2;
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
    // Louvres down each outward face.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for (axis, sign) in [(0usize, corner.x), (1usize, corner.y)] {
        let face = c[axis] + sign * (TOWER_HW * 0.93);
        for k in 0..3 {
            let z = 3.0 + 1.3 * k as f32;
            let (lo, hi) = (face.min(face + sign * 0.3), face.max(face + sign * 0.3));
            let (a, e) = if axis == 0 {
                (v3(lo, c.y - 1.8, z), v3(hi, c.y + 1.8, z + 0.45))
            } else {
                (v3(c.x - 1.8, lo, z), v3(c.x + 1.8, hi, z + 0.45))
            };
            b.block(a, e);
        }
    }
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

// ---- the turret ---------------------------------------------------------------------

/// The turntable ring the carriage stands on.
fn turntable(b: &mut MeshBuilder) {
    let sides = b.sides(16);
    b.paint(PLATING_DARK);
    b.prism(
        v3(0.0, 0.0, PLINTH_TOP - 0.05),
        sides,
        12.5,
        12.2,
        DECK - PLINTH_TOP + 0.05,
    );
    if b.fine() {
        b.paint(METAL).pattern(pattern::PLAIN);
        b.prism(v3(0.0, 0.0, PLINTH_TOP + 0.3), sides, 12.6, 12.6, 0.2);
    }
}

/// The carriage: a low armoured deck under the gun, sponsons down its flanks, a dark
/// well under the gun for its breech to dip into, and the two cheeks that carry it.
fn carriage(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (f, r, hw) = (CARRIAGE_FRONT, CARRIAGE_REAR, CARRIAGE_HW);
    let plan: Vec<[f32; 2]> = vec![
        [f, -5.0],
        [f, 5.0],
        [f - 3.5, hw],
        [r + 2.5, hw],
        [r, hw - 2.5],
        [r, -(hw - 2.5)],
        [r + 2.5, -hw],
        [f - 3.5, -hw],
    ];
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(DECK - 0.05, 1.0),
            Section::new(CARRIAGE_TOP - 0.7, 1.0),
            Section::scaled(CARRIAGE_TOP, 0.95, 0.93),
        ],
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.decal(
        v3(-2.0, 0.0, CARRIAGE_TOP + 0.02),
        v2(19.0, 2.0 * CHEEK_IN - 0.4),
    );
    team_panel(b, v3(-8.0, 5.6, CARRIAGE_TOP), v2(2.4, 1.8));
    team_panel(b, v3(-8.0, -5.6, CARRIAGE_TOP), v2(2.4, 1.8));
    b.mirror_y(|b| {
        // A low sponson down the flank, a light lid on it.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.extrude_y(
            &[
                [r + 1.8, DECK + 0.3],
                [f - 4.2, DECK + 0.3],
                [f - 5.0, CARRIAGE_TOP - 0.2],
                [r + 2.4, CARRIAGE_TOP - 0.2],
            ],
            hw - 0.6,
            hw + 0.9,
        );
        b.paint(PLATING);
        b.plate(
            v3((f + r) * 0.5 - 0.5, hw + 0.15, CARRIAGE_TOP - 0.2),
            v2(f - r - 7.6, 1.5),
            0.14,
            0.04,
        );
        // The cheek: a sloped armoured wall up to the trunnion, a dark boss on it.
        b.paint(PLATING);
        b.extrude_y(
            &[
                [-CHEEK_FOOT, CARRIAGE_TOP - 0.1],
                [CHEEK_FOOT, CARRIAGE_TOP - 0.1],
                [2.4, TRUNNION.z + 1.7],
                [-2.4, TRUNNION.z + 1.7],
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
            // Vision slits along the sponson.
            b.paint(TREAD).pattern(pattern::NONE);
            for x in [-8.0, -3.0, 2.0] {
                b.block(
                    v3(x - 1.3, hw + 0.88, DECK + 0.7),
                    v3(x + 1.3, hw + 0.96, DECK + 1.05),
                );
            }
        }
    });
    if fine {
        antenna_unlit(b, v3(-10.6, 5.2, CARRIAGE_TOP - 0.3), 3.8, 0.12);
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
    // The breech door on the rear face.
    b.block(
        v3(BODY_BACK - 0.12, -(hw - 1.4), -(hh - 1.2)),
        v3(BODY_BACK + 0.05, hw - 1.4, hh - 1.2),
    );
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

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::{BODY_FRONT, MUZZLE, TRUNNION};
    use crate::models::{build_model_scaled, material, part, rig, MeshLod, Model};

    /// The unit file's size (`aster_t3_point_defense`): radius, height, tech; 4x4 lot.
    const SIZE: (f32, f32, u8) = (20.0, 17.0, 3);
    const HALF_LOT: f32 = 24.0;

    fn built() -> Model {
        build_model_scaled("citadel", SIZE.0, SIZE.1, SIZE.2).unwrap()
    }

    fn tris(mesh: &MeshLod) -> usize {
        mesh.indices.len() / 3
    }

    fn weapon() -> (Vec3, Vec3) {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&data).unwrap();
        let bp = blueprints.unit(blueprints.id_of("aster_t3_point_defense").unwrap());
        let w = &bp.weapons[0];
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        (v(w.pivot.expect("the Citadel's gun elevates")), v(w.muzzle))
    }

    /// The unit file's pivot and muzzle are the model's trunnion and barrel tip, and the
    /// barrel reaches the muzzle at every level of detail.
    #[test]
    fn citadel_barrel_ends_at_the_muzzle_and_pitches_about_the_trunnion() {
        let (pivot, muzzle) = weapon();
        assert!(pivot.distance(TRUNNION) < 1e-2, "data pivot {pivot}");
        let tip = TRUNNION + Vec3::X * MUZZLE;
        assert!(muzzle.distance(tip) < 1e-2, "data muzzle {muzzle} vs {tip}");
        let model = built();
        assert_eq!(model.arm_pivot, Some(TRUNNION.to_array()));
        assert!(model.recoil.is_some());
        for (l, lod) in model.lods.iter().enumerate() {
            let barrel: Vec<Vec3> = lod
                .vertices
                .iter()
                .filter(|v| {
                    v.part == part::TURRET
                        && v.rig & rig::LIMB_MASK == rig::ARM_GUN
                        && v.rig & rig::RECOIL != 0
                })
                .map(|v| Vec3::from(v.pos))
                .collect();
            let front = barrel.iter().map(|p| p.x).fold(f32::MIN, f32::max);
            assert!(
                (front - tip.x).abs() < 0.2,
                "lod{l}: barrel ends at {front}"
            );
            // Out of the gun body, the barrel is five times longer than its widest clamp.
            let wide = barrel
                .iter()
                .filter(|p| p.x > TRUNNION.x + BODY_FRONT)
                .map(|p| p.y.abs().max((p.z - TRUNNION.z).abs()))
                .fold(0.0, f32::max);
            assert!(
                MUZZLE - BODY_FRONT > 5.0 * 2.0 * wide,
                "lod{l}: {wide} m half-wide"
            );
        }
    }

    /// Stands in its 4x4 lot (only the barrel overhangs), to its height, bigger than the
    /// Bastion's 2x2 keep; wears team colour, and nothing on it glows.
    #[test]
    fn citadel_fits_its_lot_and_is_unlit_hardware() {
        let model = built();
        for (l, lod) in model.lods.iter().enumerate() {
            let fixed = lod.vertices.iter().filter(|v| v.part != part::TURRET);
            let (x, y) = fixed.fold((0.0f32, 0.0f32), |(x, y), v| {
                (x.max(v.pos[0].abs()), y.max(v.pos[1].abs()))
            });
            assert!(x <= HALF_LOT && y <= HALF_LOT, "lod{l}: base {x} x {y}");
            assert!(x >= 19.0 && y >= 19.0, "lod{l}: base {x} x {y}");
            let top = lod.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
            assert!(
                top >= SIZE.1 * 0.8 && top <= SIZE.1 * 1.1,
                "lod{l}: top {top}"
            );
            assert!(
                lod.vertices.iter().any(|v| v.material == material::TEAM),
                "lod{l}: team colour"
            );
            let lit = lod
                .vertices
                .iter()
                .filter(|v| matches!(v.material, material::GLOW | material::GLOW_ORANGE))
                .count();
            assert_eq!(lit, 0, "lod{l}: lit");
            assert!(
                lod.vertices.iter().all(|v| v.pos[2] >= -1e-3),
                "lod{l}: below ground"
            );
        }
        let [full, mid, coarse] = [0, 1, 2].map(|l| tris(&model.lods[l]));
        println!("citadel: {full}/{mid}/{coarse}");
    }

    /// Sound meshes: no degenerate triangles, winding agreeing with the normals, one
    /// material and part per triangle.
    #[test]
    fn citadel_is_a_sound_mesh() {
        for (l, mesh) in built().lods.iter().enumerate() {
            for t in mesh.indices.chunks(3) {
                let v = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize]);
                let p = v.map(|v| Vec3::from(v.pos));
                let n = (p[1] - p[0]).cross(p[2] - p[0]);
                assert!(n.length() * 0.5 > 1e-7, "lod{l}: degenerate at {}", p[0]);
                for v in &v {
                    assert!(
                        n.normalize().dot(Vec3::from(v.normal)) > 0.5,
                        "lod{l}: winding at {}",
                        p[0]
                    );
                }
                assert!(v[0].material == v[1].material && v[1].material == v[2].material);
                assert!(v[0].part == v[1].part && v[1].part == v[2].part);
            }
        }
    }

    /// Previews at rest and elevated: `MODEL_DUMP_DIR=... cargo test -p mc-render --lib
    /// -- --ignored citadel_previews`.
    #[test]
    #[ignore = "writes preview images"]
    fn citadel_previews() {
        let dir =
            std::path::PathBuf::from(std::env::var_os("MODEL_DUMP_DIR").expect("MODEL_DUMP_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        let model = built();
        for (l, lod) in model.lods.iter().enumerate() {
            for pitch in [0.0f32, 0.2] {
                let mut posed = lod.clone();
                for v in &mut posed.vertices {
                    if v.rig & rig::LIMB_MASK == rig::ARM_GUN {
                        let r = Vec3::from(v.pos) - TRUNNION;
                        let (s, c) = pitch.sin_cos();
                        v.pos = (TRUNNION + Vec3::new(r.x * c - r.z * s, r.y, r.x * s + r.z * c))
                            .to_array();
                    }
                }
                let res = if l == 0 { 900 } else { 300 };
                for az in [-38.0f32, 52.0, 142.0] {
                    if l > 0 && az != -38.0 {
                        continue;
                    }
                    crate::models::preview::render(&posed, res, az)
                        .write_ppm(&dir.join(format!(
                            "citadel_l{l}_p{}_{}.ppm",
                            (pitch * 100.0) as i32,
                            az as i32
                        )))
                        .unwrap();
                }
            }
        }
    }
}

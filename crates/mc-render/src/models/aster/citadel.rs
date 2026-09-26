//! Citadel (`aster_t3_point_defense`, mesh "citadel"): the tech 3 rail point defence,
//! authored at blueprint scale (metres, 4x4 lot, radius 20, height 17).
//!
//! A keep built round one heavy rail cannon: the Zenith's little sister, laid flat at
//! the ground instead of up at the sky. Nothing on it is lit (docs/STYLE.md, rail guns
//! are hardware). Read it as a coastal gun: a low, wide house and a long gun, never a
//! tall box with a gun stuck in its face.
//! - Fixed (`part::HULL`): the lot slab; a sloped lower step with its corners cut, a
//!   squat capacitor tower on each cut corner with its conduit run into the plinth, an
//!   octagonal plinth, and the race ring the house turns on.
//! - Turning (`part::TURRET`, about the lot's centre): the turntable and a long, low
//!   casemate, a vertical skirt and then a glacis sloped back hard at the front and
//!   in on every side, armour sponsons down its cheeks, a bank of capacitor tubes
//!   half sunk in the rear deck with bus cables run forward, and a rangefinder.
//! - Elevating (`rig::ARM_GUN`, about [`TRUNNION`] inside the house): the mantlet, a
//!   wide, low armoured slab over the gun slot in the glacis. (`rig::ARM_GUN |
//!   rig::RECOIL`) the barrel, which kicks back through the mantlet when it fires.
//!
//! The barrel: one long jacket tapering all the way from the mantlet, then the two
//! rails bare with the slot open between them, held by square dark yokes, into a
//! squared muzzle brace with the bore dark in it. The breech is inside the house and
//! never seen.

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

/// The trunnion (model space): keep `weapons[0].pivot` in structures.ron equal to it,
/// and `weapons[0].muzzle` equal to it plus [`MUZZLE`] along x.
pub(crate) const TRUNNION: Vec3 = Vec3::new(4.0, 0.0, 12.4);

// ---- the gun (barrel frame: metres from the trunnion along the bore) -----------------

/// The breech block, hidden in the house.
const BREECH: f32 = -8.0;
const BREECH_R: f32 = 2.0;
/// The mantlet over the gun slot: rear and front faces, half width and height.
const MANTLET_BACK: f32 = 3.0;
const MANTLET_FRONT: f32 = 7.8;
const MANTLET_HW: f32 = 4.4;
const MANTLET_HH: f32 = 1.9;
/// The jacket, tapering from its root to where the rails come out bare.
const JACKET_R: f32 = 1.55;
const JACKET_END_R: f32 = 1.35;
const JACKET_TO: f32 = 27.0;
/// The bare rails, into the muzzle brace: each rail's centre off the bore (±y), its
/// half width and half height.
const RAILS_TO: f32 = 45.5;
const RAIL_Y: f32 = 1.0;
const RAIL_HW: f32 = 0.38;
const RAIL_HH: f32 = 1.05;
/// The first and last of the yokes clamping the bare rails.
const YOKE_FROM: f32 = 30.5;
const YOKE_TO: f32 = 42.5;
/// The muzzle face, and the brace round the rails' ends: half width and height.
pub(crate) const MUZZLE: f32 = 50.0;
const BRACE_HW: f32 = 1.75;
const BRACE_HH: f32 = 1.45;
const RECOIL: f32 = 1.6;

// ---- the house (model space) ------------------------------------------------------

/// The turntable's top, where the house stands; the top of the vertical skirt, where
/// the glacis starts; and the roof.
const DECK: f32 = 8.6;
const SKIRT: f32 = 10.2;
const ROOF: f32 = 14.2;
/// The house's plan at the deck: front face, rear, half width.
const FRONT: f32 = 11.5;
const REAR: f32 = -15.0;
const HOUSE_HW: f32 = 10.0;
/// The roof's plan against the deck's: scale along and across, and how far back it sits.
const ROOF_SX: f32 = 0.84;
const ROOF_SY: f32 = 0.86;
const ROOF_BACK: f32 = -1.45;
// The breech stays inside the house when the gun is level.
const _: () = assert!(TRUNNION.x + BREECH > REAR);

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
        house(b);
        b.with_limb(rig::ARM_GUN, |b| b.at(TRUNNION, mantlet));
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| b.at(TRUNNION, barrel));
    });
}

/// Far off: the keep, the house, the mantlet and the barrel as a bar, and the owner's
/// colour on the roof. Under 60 triangles.
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
        let length = FRONT - REAR;
        b.frustum_open(
            v3((FRONT + REAR) * 0.5, 0.0, DECK),
            v2(length, 2.0 * HOUSE_HW),
            v2(length * ROOF_SX, 2.0 * HOUSE_HW * ROOF_SY),
            ROOF - DECK,
            v2((FRONT + REAR) * 0.5 * (ROOF_SX - 1.0) + ROOF_BACK, 0.0),
        );
        team_panel(b, v3(-7.5, 0.0, ROOF), v2(4.0, 9.0));
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
            b.at(TRUNNION, |b| {
                // A square bar from the mantlet to the muzzle, capped at the muzzle.
                b.paint(PLATING_DARK);
                b.loft(
                    &[
                        square(MANTLET_BACK, MANTLET_HW, MANTLET_HH),
                        square(MANTLET_FRONT, 1.5, 1.5),
                        square(MUZZLE, BRACE_HW, BRACE_HH),
                    ],
                    false,
                    true,
                );
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

// ---- the house ----------------------------------------------------------------------

/// The turntable ring the house stands on.
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

/// The casemate: long and low, a short vertical skirt and then a glacis sloped back
/// hard at the front and in on every side, the mantlet in the glacis, armour sponsons
/// down its cheeks, the capacitor bank on its rear deck.
fn house(b: &mut MeshBuilder) {
    let fine = b.fine();
    let plan: Vec<[f32; 2]> = vec![
        [FRONT, -6.5],
        [FRONT, 6.5],
        [7.0, HOUSE_HW],
        [-11.5, HOUSE_HW],
        [REAR, 7.0],
        [REAR, -7.0],
        [-11.5, -HOUSE_HW],
        [7.0, -HOUSE_HW],
    ];
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(DECK - 0.05, 1.0),
            Section::new(SKIRT, 1.0),
            Section::scaled(ROOF, ROOF_SX, ROOF_SY).shifted(ROOF_BACK, 0.0),
        ],
    );
    // A dark band round the skirt's top, where the glacis starts.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &plan,
        &[
            Section::scaled(SKIRT - 0.55, 1.006, 1.006),
            Section::scaled(SKIRT, 1.006, 1.006),
        ],
    );
    // The owner's colour on the rear roof.
    team_panel(b, v3(-9.0, 5.0, ROOF), v2(2.6, 2.0));
    team_panel(b, v3(-9.0, -5.0, ROOF), v2(2.6, 2.0));
    // Cheek sponsons: low armoured banks down each flank, a light lid on each.
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.extrude_y(
            &[
                [-12.5, DECK + 0.4],
                [6.0, DECK + 0.4],
                [5.0, SKIRT + 1.2],
                [-11.5, SKIRT + 1.2],
            ],
            HOUSE_HW - 1.0,
            HOUSE_HW + 1.2,
        );
        b.paint(PLATING);
        b.plate(
            v3(-3.25, HOUSE_HW + 0.1, SKIRT + 1.2),
            v2(16.0, 2.0),
            0.14,
            0.04,
        );
    });
    capacitors(b);
    if !fine {
        return;
    }
    // Vision slits down the sponsons, and a hatch either side of the bank.
    b.paint(TREAD).pattern(pattern::NONE);
    b.mirror_y(|b| {
        for x in [-8.5, -3.5, 1.5] {
            b.block(
                v3(x - 1.4, HOUSE_HW + 1.18, DECK + 1.6),
                v3(x + 1.4, HOUSE_HW + 1.26, DECK + 2.0),
            );
        }
    });
    b.paint(PLATING_DARK);
    b.mirror_y(|b| b.plate(v3(-2.5, 5.2, ROOF - 0.1), v2(3.0, 1.9), 0.3, 0.1));
    // The rangefinder: a low dark box on the fore roof, off the axis, its slit darker
    // still; a short unlit mast at the rear.
    let at = v3(2.4, -5.2, ROOF - 0.1);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(at + v3(0.0, 0.0, 0.7), v3(3.2, 2.0, 1.4), 0.3);
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(at + v3(1.55, -0.75, 0.8), at + v3(1.68, 0.75, 1.15));
    antenna_unlit(b, v3(-12.4, 4.4, ROOF - 0.4), 3.6, 0.12);
}

/// The capacitor bank on the rear deck: three heavy tubes lying across the house, half
/// sunk in a dark saddle, banded, and the bus cables laid forward along the roof from
/// it to a junction box over the breech.
fn capacitors(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1, hw, r) = (-13.0, -6.2, 3.4, 0.9);
    let axis = ROOF + 0.25 + r * 0.6;
    b.paint(PLATING_DARK);
    b.chamfered_box(
        v3((x0 + x1) * 0.5, 0.0, ROOF + 0.3),
        v3(x1 - x0, 2.0 * hw - 0.4, 0.8),
        0.25,
    );
    let sides = if fine { 12 } else { 6 };
    for i in 0..3 {
        let x = x0 + r + 0.1 + i as f32 * (x1 - x0 - 2.0 * r - 0.2) / 2.0;
        b.paint(PLATING);
        b.cylinder_between(v3(x, -hw, axis), v3(x, hw, axis), r, r, sides);
        if fine {
            // Dark end caps and a band at the middle.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for (y0, y1) in [
                (-hw - 0.15, -hw + 0.35),
                (-0.3, 0.3),
                (hw - 0.35, hw + 0.15),
            ] {
                b.cylinder_between(v3(x, y0, axis), v3(x, y1, axis), r + 0.08, r + 0.08, 12);
            }
        }
    }
    if !fine {
        return;
    }
    // The bus: cables out of the saddle's front, laid flat along the roof to a
    // junction box on the fore roof.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let lie = ROOF + 0.22;
    for y in [-1.8, -0.6, 0.6, 1.8] {
        cable(
            b,
            &[
                v3(x1 - 0.2, y, ROOF + 0.5),
                v3(x1 + 0.9, y, lie),
                v3(-1.2, y * 0.7, lie),
            ],
            0.2,
        );
    }
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(-0.5, 0.0, ROOF + 0.4), v3(1.6, 3.2, 0.8), 0.2);
}

// ---- the gun (barrel frame: origin at the trunnion, +x down the bore) ----------------

/// The mantlet: a wide, low slab over the slot with a sloped brow, a short collar where
/// the jacket leaves it.
fn mantlet(b: &mut MeshBuilder) {
    let (hw, hh) = (MANTLET_HW, MANTLET_HH);
    let profile = [
        [MANTLET_BACK, -hh],
        [MANTLET_FRONT - 0.5, -hh],
        [MANTLET_FRONT, -hh + 0.5],
        [MANTLET_FRONT, hh - 0.6],
        [MANTLET_FRONT - 1.7, hh],
        [MANTLET_BACK, hh],
    ];
    b.paint(PLATING);
    b.extrude_y_chamfered(&profile, hw, 0.25);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(
        v3(MANTLET_FRONT - 0.05, 0.0, 0.0),
        v3(MANTLET_FRONT + 0.45, 0.0, 0.0),
        JACKET_R + 0.25,
        JACKET_R + 0.2,
        b.sides(16),
    );
    if !b.fine() {
        return;
    }
    // Dark bolt strips down the face either side of the gun.
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.block(
            v3(MANTLET_FRONT - 0.02, 2.4, -hh + 0.7),
            v3(MANTLET_FRONT + 0.1, 3.3, hh - 0.8),
        )
    });
}

/// The barrel: see the module notes. The breech block and the jacket's root sit inside
/// the house and the mantlet, so the kick shows no gap.
fn barrel(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = if fine { 14 } else { 8 };
    let (outer, inner) = (RAIL_Y + RAIL_HW, RAIL_Y - RAIL_HW);

    // The breech, in the house: only its block, round.
    b.paint(PLATING_DARK);
    b.cylinder_between(
        v3(BREECH, 0.0, 0.0),
        v3(BREECH + 4.0, 0.0, 0.0),
        BREECH_R,
        BREECH_R,
        8,
    );
    // The jacket, one long taper from inside the mantlet to the rails.
    b.paint(PLATING);
    b.cylinder_between(
        v3(BREECH + 4.0, 0.0, 0.0),
        v3(JACKET_TO, 0.0, 0.0),
        JACKET_R,
        JACKET_END_R,
        sides,
    );
    let r_at = |x: f32| {
        JACKET_R + (JACKET_END_R - JACKET_R) * (x - BREECH - 4.0) / (JACKET_TO - BREECH - 4.0)
    };
    if fine {
        // Two thin dark bands, and the rails showing down its flanks as steel spines.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for x in [14.0, 21.0] {
            let r = r_at(x) + 0.08;
            b.cylinder_between(v3(x - 0.2, 0.0, 0.0), v3(x + 0.2, 0.0, 0.0), r, r, 12);
        }
        b.paint(METAL).pattern(pattern::PLAIN);
        b.mirror_y(|b| {
            b.beam(
                v3(12.0, r_at(12.0) * 0.9, 0.0),
                v3(JACKET_TO - 0.4, r_at(JACKET_TO) * 0.9, 0.0),
                v2(0.3, 0.7),
                v2(0.3, 0.7),
            )
        });
    }
    // Where the rails come out: a square dark block, not a ring.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(
        v3(JACKET_TO + 0.5, 0.0, 0.0),
        v3(1.4, 2.0 * (outer + 0.25), 2.0 * (RAIL_HH + 0.3)),
        0.15,
    );

    // The bare rails: two light bars bevelled on their outer edges, the slot open.
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
        b.extrude_x(&profile, JACKET_TO, RAILS_TO + 0.3);
    });
    // The yokes: square dark clamps across both rails, evenly down the bare length.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let yokes = 4;
    for i in 0..yokes {
        if !fine && i % 3 != 0 {
            continue;
        }
        let x = YOKE_FROM + (YOKE_TO - YOKE_FROM) * i as f32 / (yokes - 1) as f32;
        b.chamfered_box(
            v3(x, 0.0, 0.0),
            v3(0.7, 2.0 * (outer + 0.18), 2.0 * (RAIL_HH + 0.18)),
            0.1,
        );
    }

    // The muzzle brace: a squared housing over the rails' ends, flaring a little, a dark
    // face plate and the bore dark in it.
    b.paint(PLATING_DARK);
    b.loft(
        &[
            square(RAILS_TO - 0.5, BRACE_HW - 0.15, BRACE_HH - 0.1),
            square(MUZZLE - 0.6, BRACE_HW, BRACE_HH),
        ],
        true,
        false,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(
        v3(MUZZLE - 0.3, 0.0, 0.0),
        v3(0.6, 2.0 * BRACE_HW + 0.2, 2.0 * BRACE_HH + 0.2),
        0.1,
    );
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(
        v3(MUZZLE - 0.02, -inner - 0.1, -RAIL_HH),
        v3(MUZZLE + 0.02, inner + 0.1, RAIL_HH),
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

    use super::{MUZZLE, TRUNNION};
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
            // Long and slim: out of the mantlet, eight times longer than its widest collar.
            let wide = barrel
                .iter()
                .filter(|p| p.x > TRUNNION.x + 12.0)
                .map(|p| p.y.abs().max((p.z - TRUNNION.z).abs()))
                .fold(0.0, f32::max);
            assert!(
                MUZZLE - 8.0 > 8.0 * 2.0 * wide,
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

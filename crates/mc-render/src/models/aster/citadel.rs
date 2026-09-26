//! Citadel (`aster_t3_point_defense`, mesh "citadel"): the tech 3 rail point defence,
//! authored at blueprint scale (metres, 4x4 lot, radius 20, height 22).
//!
//! A keep built round one heavy rail cannon: the Zenith's little sister, laid flat at
//! the ground instead of up at the sky. Nothing on it is lit (docs/STYLE.md, rail guns
//! are hardware).
//! - Fixed (`part::HULL`): the lot slab; a sloped lower step with its corners cut, a
//!   squat capacitor tower on each cut corner with its conduit run into the plinth, an
//!   octagonal plinth, and the race ring the house turns on.
//! - Turning (`part::TURRET`, about the lot's centre): the turntable and a long, low
//!   armoured casemate with sloped walls, armour sponsons down its cheeks, a bank of
//!   capacitor tubes on the rear deck with bus cables run forward, and a rangefinder
//!   with a dark slit.
//! - Elevating (`rig::ARM_GUN`, about [`TRUNNION`] inside the house): the mantlet, a
//!   heavy armoured block over the gun slot in the house's face. (`rig::ARM_GUN |
//!   rig::RECOIL`) the barrel, which kicks back through the mantlet when it fires.
//!
//! The barrel: a heavy round jacket out of the mantlet, a step collar down to a slim
//! jacket, then the two rails bare with the slot open between them through a ladder of
//! dark clamps, and a flared muzzle ring with the square bore dark in it. The breech is
//! inside the house and never seen.

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

/// The trunnion (model space): keep `weapons[0].pivot` in structures.ron equal to it,
/// and `weapons[0].muzzle` equal to it plus [`MUZZLE`] along x.
pub(crate) const TRUNNION: Vec3 = Vec3::new(3.0, 0.0, 15.0);

// ---- the gun (barrel frame: metres from the trunnion along the bore) -----------------

/// The breech block, hidden in the house.
const BREECH: f32 = -9.0;
const BREECH_R: f32 = 2.3;
/// The mantlet over the gun slot: rear and front faces, half width and height.
const MANTLET_BACK: f32 = 3.6;
const MANTLET_FRONT: f32 = 8.4;
const MANTLET_HW: f32 = 3.4;
const MANTLET_HH: f32 = 2.9;
/// The heavy jacket out of the mantlet to the step collar, and its radius.
const JACKET_R: f32 = 1.8;
const STEP: f32 = 18.0;
/// The slim jacket after the step, to where the rails come out bare.
const SLIM_R: f32 = 1.5;
const JACKET_TO: f32 = 24.0;
/// The bare rails, to the muzzle ring: each rail's centre off the bore (±y), its half
/// width and half height.
const RAILS_TO: f32 = 39.0;
const RAIL_Y: f32 = 0.9;
const RAIL_HW: f32 = 0.33;
const RAIL_HH: f32 = 0.9;
/// The last clamp before the muzzle ring.
const LAST_CLAMP: f32 = 36.5;
/// The muzzle face, and the flare of the ring there.
pub(crate) const MUZZLE: f32 = 43.0;
const MUZZLE_R: f32 = 2.0;
const RECOIL: f32 = 1.4;

// ---- the house (model space) ------------------------------------------------------

/// The turntable's top, where the house stands, and the house's roof.
const DECK: f32 = 9.0;
const ROOF: f32 = 19.0;
/// The house's plan at the deck: front face, rear, half width, and where the cheeks
/// start to slope in toward the face.
const FRONT: f32 = 9.6;
const REAR: f32 = -14.0;
const HOUSE_HW: f32 = 9.0;

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
        b.frustum_open(
            v3((FRONT + REAR) * 0.5, 0.0, DECK),
            v2(FRONT - REAR, 2.0 * HOUSE_HW),
            v2(FRONT - REAR - 3.0, 2.0 * HOUSE_HW - 3.0),
            ROOF - DECK,
            v2(-1.0, 0.0),
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
                        square(MUZZLE, 1.1, 1.1),
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

/// The casemate: long and low, sloped in on every side, its face blunt behind the
/// mantlet, armour sponsons down its cheeks, the capacitor rack on its rear deck.
fn house(b: &mut MeshBuilder) {
    let fine = b.fine();
    let plan: Vec<[f32; 2]> = vec![
        [FRONT, -5.2],
        [FRONT, 5.2],
        [5.4, HOUSE_HW],
        [-10.6, HOUSE_HW],
        [REAR, 6.0],
        [REAR, -6.0],
        [-10.6, -HOUSE_HW],
        [5.4, -HOUSE_HW],
    ];
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(DECK - 0.05, 1.0),
            Section::scaled(ROOF - 3.0, 0.985, 0.975),
            Section::scaled(ROOF, 0.9, 0.86).shifted(-0.9, 0.0),
        ],
    );
    // A dark band round the house's waist.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &plan,
        &[
            Section::scaled(DECK + 1.8, 1.0, 1.0),
            Section::scaled(DECK + 2.5, 0.998, 0.996),
        ],
    );
    // The recess the mantlet sits in: a dark frame round the gun slot on the face.
    b.block(
        v3(
            FRONT - 0.6,
            -MANTLET_HW - 0.7,
            TRUNNION.z - MANTLET_HH - 0.7,
        ),
        v3(
            FRONT + 0.15,
            MANTLET_HW + 0.7,
            TRUNNION.z + MANTLET_HH + 0.7,
        ),
    );
    // The owner's colour on the rear roof.
    team_panel(b, v3(-9.5, 5.2, ROOF), v2(2.6, 2.0));
    team_panel(b, v3(-9.5, -5.2, ROOF), v2(2.6, 2.0));
    // Cheek sponsons: armoured banks down each flank, a light lid on each.
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.extrude_y(
            &[
                [-11.0, DECK + 0.6],
                [4.6, DECK + 0.6],
                [3.4, 15.2],
                [-10.0, 15.2],
            ],
            HOUSE_HW - 0.2,
            HOUSE_HW + 1.3,
        );
        b.paint(PLATING);
        b.plate(v3(-3.3, HOUSE_HW + 0.55, 15.2), v2(12.6, 1.3), 0.14, 0.04);
    });
    capacitors(b);
    if !fine {
        return;
    }
    // Vision slits down the flanks, and a hatch either side of the rack.
    b.paint(TREAD).pattern(pattern::NONE);
    b.mirror_y(|b| {
        for x in [-8.0, -3.0] {
            b.block(
                v3(x - 1.4, HOUSE_HW + 1.28, 12.6),
                v3(x + 1.4, HOUSE_HW + 1.36, 13.1),
            );
        }
    });
    b.paint(PLATING_DARK);
    b.mirror_y(|b| b.plate(v3(-1.0, 4.9, ROOF - 0.1), v2(3.0, 1.9), 0.3, 0.1));
    // The rangefinder: a dark box on the fore roof, off the axis, its slit darker still;
    // a short unlit mast behind it.
    let at = v3(3.2, -4.1, ROOF - 0.2);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(at + v3(0.0, 0.0, 1.0), v3(3.4, 2.2, 2.0), 0.3);
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(at + v3(1.65, -0.8, 1.2), at + v3(1.78, 0.8, 1.6));
    antenna_unlit(b, v3(-12.2, 3.8, ROOF - 0.9), 4.6, 0.12);
}

/// The capacitor bank on the rear deck: three heavy tubes lying across the house on a
/// dark saddle, banded, and the bus cables laid forward along the roof from it to a
/// junction box over the breech.
fn capacitors(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1, hw, r) = (-13.0, -5.2, 3.5, 1.05);
    let axis = ROOF + 0.55 + r;
    b.paint(PLATING_DARK);
    b.chamfered_box(
        v3((x0 + x1) * 0.5, 0.0, ROOF + 0.3),
        v3(x1 - x0, 2.0 * hw - 0.6, 0.9),
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
                v3(x1 - 0.2, y, ROOF + 0.55),
                v3(x1 + 0.9, y, lie),
                v3(-0.8, y * 0.7, lie),
            ],
            0.2,
        );
    }
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(-0.1, 0.0, ROOF + 0.45), v3(1.6, 3.2, 0.9), 0.2);
}

// ---- the gun (barrel frame: origin at the trunnion, +x down the bore) ----------------

/// The mantlet: a heavy block over the slot with a sloped brow, its nose ring round the
/// jacket, and the trunnion pin inside.
fn mantlet(b: &mut MeshBuilder) {
    let (hw, hh) = (MANTLET_HW, MANTLET_HH);
    let profile = [
        [MANTLET_BACK, -hh],
        [MANTLET_FRONT - 1.0, -hh],
        [MANTLET_FRONT, -hh + 0.9],
        [MANTLET_FRONT, hh - 1.2],
        [MANTLET_FRONT - 1.8, hh],
        [MANTLET_BACK, hh],
    ];
    b.paint(PLATING);
    b.extrude_y_chamfered(&profile, hw, 0.3);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(
        v3(MANTLET_FRONT - 0.1, 0.0, 0.0),
        v3(MANTLET_FRONT + 0.8, 0.0, 0.0),
        JACKET_R + 0.55,
        JACKET_R + 0.35,
        b.sides(16),
    );
    if !b.fine() {
        return;
    }
    // A dark band across the block and bolted cheek plates.
    b.cuboid(
        v3(MANTLET_BACK + 1.4, 0.0, 0.0),
        v3(0.7, 2.0 * hw + 0.2, 2.0 * hh + 0.2),
    );
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.block(
            v3(MANTLET_BACK + 2.4, hw - 0.05, -1.6),
            v3(MANTLET_FRONT - 1.4, hw + 0.2, 1.4),
        )
    });
}

/// The barrel: see the module notes. The breech block and the jacket's root sit inside
/// the house and the mantlet, so the kick shows no gap.
fn barrel(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = if fine { 14 } else { 8 };
    let band = if fine { 12 } else { 8 };
    let ring = |b: &mut MeshBuilder, x0: f32, x1: f32, r: f32| {
        b.cylinder_between(v3(x0, 0.0, 0.0), v3(x1, 0.0, 0.0), r, r, band);
    };

    // The breech, in the house: only its block and the jacket's root, round.
    b.paint(PLATING_DARK);
    b.cylinder_between(
        v3(BREECH, 0.0, 0.0),
        v3(BREECH + 5.0, 0.0, 0.0),
        BREECH_R,
        BREECH_R,
        8,
    );
    // The heavy jacket, from inside the mantlet to the step collar.
    b.paint(PLATING);
    b.cylinder_between(
        v3(BREECH + 5.0, 0.0, 0.0),
        v3(STEP, 0.0, 0.0),
        JACKET_R,
        JACKET_R * 0.97,
        sides,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    if fine {
        ring(b, 12.6, 13.3, JACKET_R + 0.15);
    }
    // The step collar down to the slim jacket.
    b.cylinder_between(
        v3(STEP - 0.7, 0.0, 0.0),
        v3(STEP + 0.7, 0.0, 0.0),
        JACKET_R + 0.3,
        SLIM_R + 0.25,
        sides,
    );
    b.paint(PLATING);
    b.cylinder_between(
        v3(STEP + 0.7, 0.0, 0.0),
        v3(JACKET_TO, 0.0, 0.0),
        SLIM_R,
        SLIM_R * 0.96,
        sides,
    );
    // The slim jacket's end: a heavy stepped ring where the rails come out.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    ring(b, JACKET_TO - 0.6, JACKET_TO + 0.3, SLIM_R + 0.3);
    // The rails showing down the slim jacket's flanks as bare steel spines.
    b.paint(METAL).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        b.beam(
            v3(STEP + 0.8, SLIM_R * 0.92, 0.0),
            v3(JACKET_TO - 0.6, SLIM_R * 0.88, 0.0),
            v2(0.36, 0.86),
            v2(0.36, 0.8),
        )
    });

    // The bare rails: two light bars bevelled on their outer edges, the slot open.
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let (inner, outer, h, bevel) = (RAIL_Y - RAIL_HW, RAIL_Y + RAIL_HW, RAIL_HH, 0.14);
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
    // The clamp ladder: dark collars round both rails, a touch smaller toward the muzzle.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let clamps = 5;
    for i in 0..clamps {
        if !fine && i % 2 != 0 {
            continue;
        }
        let f = i as f32 / (clamps - 1) as f32;
        let x = JACKET_TO + 3.2 + (LAST_CLAMP - JACKET_TO - 3.2) * f;
        let r = 1.62 - 0.1 * f;
        ring(b, x - 0.3, x + 0.3, r);
    }

    // The muzzle: a flared ring over the rails' ends and the square bore dark in it.
    b.paint(PLATING_DARK);
    b.cylinder_between(
        v3(RAILS_TO, 0.0, 0.0),
        v3(MUZZLE - 0.5, 0.0, 0.0),
        1.55,
        MUZZLE_R,
        sides,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(
        v3(MUZZLE - 0.5, 0.0, 0.0),
        v3(MUZZLE, 0.0, 0.0),
        MUZZLE_R,
        MUZZLE_R * 0.96,
        sides,
    );
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(
        v3(MUZZLE - 0.02, -(RAIL_Y - RAIL_HW) - 0.1, -RAIL_HH),
        v3(MUZZLE + 0.02, RAIL_Y - RAIL_HW + 0.1, RAIL_HH),
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

    use super::{BREECH, MUZZLE, TRUNNION};
    use crate::models::{build_model_scaled, material, part, rig, MeshLod, Model};

    /// The unit file's size (`aster_t3_point_defense`): radius, height, tech; 4x4 lot.
    const SIZE: (f32, f32, u8) = (20.0, 22.0, 3);
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
        // The breech stays inside the house when the gun is level.
        assert!(TRUNNION.x + BREECH > super::REAR);
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

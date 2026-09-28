//! Culverin (`aster_t4_artillery`, mesh "culverin"): the tech 4 map gun, authored at
//! blueprint scale (metres, 6x6 lot, radius 30, height 27).
//!
//! A supergun, not a bigger howitzer: one very long plain tube, so long it would sag
//! under its own weight, held straight by a king-post and a pair of cables over its
//! back (the Paris gun's truss). Nothing on it is lit: a conventional gun, a dark bore,
//! no brake (docs/STYLE.md).
//! - Fixed (`part::HULL`): the lot slab, a sloped concrete emplacement, the race ring,
//!   and two low magazine bunkers on the corners behind the lot's middle.
//! - Turning (`part::TURRET`, about the lot's centre): the turntable, the carriage (two
//!   long armoured side hulls with an open well between them for the breech to dip
//!   into, the loading house and its shell hoist at the back, a rangefinder), and the
//!   two cheeks carrying the trunnion.
//! - Elevating (`rig::ARM_GUN`, about [`TRUNNION`]): the cradle, an armoured sleeve round
//!   the tube with four recoil cylinders along it. (`rig::ARM_GUN | rig::RECOIL`) the
//!   gun: the breech ring behind the cradle, the tube, the king-post and its cables.

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern, rig};

/// The trunnion (model space), over the turret's axis: keep `weapons[0].pivot` in
/// strategic.ron equal to it, and `weapons[0].muzzle` equal to it plus [`MUZZLE`]
/// along x.
pub(crate) const TRUNNION: Vec3 = Vec3::new(0.0, 0.0, 18.0);

// ---- the gun (barrel frame: metres from the trunnion along the bore) -----------------

/// The breech ring behind the cradle: its back face, front face, half width and half height.
const BREECH_BACK: f32 = -12.0;
const BREECH_FRONT: f32 = -7.6;
const BREECH_HW: f32 = 3.1;
const BREECH_HH: f32 = 3.0;
/// The cradle sleeve: back and front, half width and height, corner cut.
const CRADLE_BACK: f32 = -8.0;
const CRADLE_FRONT: f32 = 14.0;
const CRADLE_HW: f32 = 3.6;
const CRADLE_HH: f32 = 3.4;
const CRADLE_CUT: f32 = 1.1;
/// The tube: its radius where it leaves the cradle, where the taper ends, and at the
/// muzzle; the hoops shrunk on along it.
const TUBE_R: f32 = 2.3;
const TUBE_WAIST: f32 = 36.0;
const TUBE_WAIST_R: f32 = 1.75;
const TUBE_MUZZLE_R: f32 = 1.4;
const HOOPS: [f32; 4] = [22.0, 29.0, 36.0, 54.0];
/// The muzzle face.
pub(crate) const MUZZLE: f32 = 74.0;
/// How far the gun runs back in its cradle when it fires.
const RECOIL: f32 = 3.0;
/// The king-post over the tube, how high its head stands over the bore, and where the
/// two cables from its head are made fast along the tube.
const POST: f32 = 18.0;
const POST_HEAD: f32 = 8.5;
const STAY_BACK: f32 = 2.0;
const STAY_FRONT: f32 = 60.0;

// ---- the carriage (model space) ---------------------------------------------------

/// The turntable's top, where the carriage stands, and the side hulls' deck.
const DECK: f32 = 4.4;
const HULL_TOP: f32 = 10.0;
/// The carriage's plan: front and rear, outer half width, and the well between the side
/// hulls (half width, its front and rear) that the breech dips into at high elevation.
const CARRIAGE_FRONT: f32 = 12.0;
const CARRIAGE_REAR: f32 = -28.0;
const CARRIAGE_HW: f32 = 12.0;
const WELL_HW: f32 = 4.2;
const WELL_FRONT: f32 = 7.0;
const WELL_REAR: f32 = -16.5;
/// The cheeks carrying the trunnion: inner face (y), thickness, half length at the foot.
const CHEEK_IN: f32 = CRADLE_HW + 0.4;
const CHEEK_T: f32 = 3.2;
const CHEEK_FOOT: f32 = 8.5;
/// The loading house at the carriage's back: front, rear, half width, roof.
const HOUSE_FRONT: f32 = -18.5;
const HOUSE_REAR: f32 = -27.5;
const HOUSE_HW: f32 = 7.5;
const HOUSE_TOP: f32 = 21.0;
// The gun is level at rest: its cradle clears the side hulls, and the breech ring
// clears the loading house when level.
const _: () = assert!(TRUNNION.z - CRADLE_HH > HULL_TOP);
const _: () = assert!(TRUNNION.x + BREECH_BACK - RECOIL > HOUSE_FRONT + 1.0);

// ---- the base ----------------------------------------------------------------------

/// The slab's half width and top.
const SLAB: f32 = 29.0;
const SLAB_TOP: f32 = 0.5;
/// The emplacement: radius to its corners at the foot and the top, and its top.
const BERM_FOOT: f32 = 26.5;
const BERM_TOP_R: f32 = 23.0;
const BERM_TOP: f32 = 3.2;
/// The turntable's radius, and the dark hub in the middle of its deck.
const TABLE_R: f32 = 21.0;
const HUB_R: f32 = 13.0;
/// The magazine bunkers on the corners: centre (x, ±y), half size, roof.
const BUNKER: Vec2 = Vec2::new(-23.5, 23.5);
const BUNKER_HW: f32 = 4.2;
const BUNKER_TOP: f32 = 2.9;

pub(crate) fn culverin(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(TRUNNION);
    b.set_recoil(TRUNNION, TRUNNION + Vec3::X, RECOIL);
    if b.coarse() {
        coarse(b);
        return;
    }
    slab(b);
    emplacement(b);
    b.mirror_y(bunker);
    b.with_part(part::TURRET, |b| {
        turntable(b);
        carriage(b);
        house(b);
        b.with_limb(rig::ARM_GUN, |b| b.at(TRUNNION, cradle));
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| b.at(TRUNNION, gun));
    });
}

/// Far off: the emplacement, the carriage as a block, the gun as one tapering bar, and
/// the owner's colour on the carriage.
fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.frustum_open(
        Vec3::ZERO,
        v2(2.0 * BERM_FOOT, 2.0 * BERM_FOOT),
        v2(2.0 * BERM_TOP_R, 2.0 * BERM_TOP_R),
        BERM_TOP,
        Vec2::ZERO,
    );
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let length = CARRIAGE_FRONT - CARRIAGE_REAR;
        b.frustum_open(
            v3((CARRIAGE_FRONT + CARRIAGE_REAR) * 0.5, 0.0, DECK),
            v2(length, 2.0 * CARRIAGE_HW),
            v2(length - 2.0, 2.0 * CARRIAGE_HW - 2.0),
            HULL_TOP - DECK,
            Vec2::ZERO,
        );
        team_panel(b, v3(-8.0, 0.0, HULL_TOP), v2(6.0, 8.0));
        b.paint(PLATING);
        b.frustum_open(
            v3((HOUSE_FRONT + HOUSE_REAR) * 0.5, 0.0, HULL_TOP),
            v2(HOUSE_FRONT - HOUSE_REAR, 2.0 * HOUSE_HW),
            v2(HOUSE_FRONT - HOUSE_REAR - 1.5, 2.0 * HOUSE_HW - 1.5),
            HOUSE_TOP - HULL_TOP,
            Vec2::ZERO,
        );
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
            b.at(TRUNNION, |b| {
                b.paint(PLATING_DARK);
                b.beam(
                    v3(BREECH_BACK, 0.0, 0.0),
                    v3(MUZZLE, 0.0, 0.0),
                    v2(2.0 * CRADLE_HW, 2.0 * CRADLE_HH),
                    v2(2.0 * TUBE_MUZZLE_R, 2.0 * TUBE_MUZZLE_R),
                );
                b.paint(PLATING);
                b.beam(
                    v3(POST, 0.0, 0.0),
                    v3(POST, 0.0, POST_HEAD),
                    v2(1.2, 0.8),
                    v2(0.6, 0.4),
                );
            });
        });
    });
}

// ---- the base ----------------------------------------------------------------------

/// The lot's slab: a dark foot and a light deck.
fn slab(b: &mut MeshBuilder) {
    let plan = chamfered_rect(v2(SLAB, SLAB), 4.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(0.25, 1.0)]);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(0.25, 0.99), Section::new(SLAB_TOP, 0.98)],
    );
}

/// The sloped concrete emplacement the turntable sits in, and the race ring.
fn emplacement(b: &mut MeshBuilder) {
    let berm = ngon(12, BERM_FOOT);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &berm,
        &[
            Section::new(SLAB_TOP - 0.1, 1.0),
            Section::new(SLAB_TOP + 0.6, 0.995),
        ],
    );
    b.paint(PLATING);
    b.loft_z(
        &berm,
        &[
            Section::new(SLAB_TOP + 0.5, 0.99),
            Section::new(BERM_TOP, BERM_TOP_R / BERM_FOOT),
        ],
    );
    let sides = b.sides(24);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(
        v3(0.0, 0.0, BERM_TOP - 0.15),
        sides,
        TABLE_R + 0.9,
        TABLE_R + 0.7,
        0.45,
    );
}

/// One magazine bunker on a back corner (+y; mirrored): a low sloped blockhouse with a
/// dark blast door facing the turntable, a vent can on its roof, and the owner's colour.
fn bunker(b: &mut MeshBuilder) {
    let c = BUNKER;
    let plan: Vec<[f32; 2]> = chamfered_rect(v2(BUNKER_HW, BUNKER_HW), 1.2)
        .iter()
        .map(|p| [p[0] + c.x, p[1] + c.y])
        .collect();
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &plan,
        &[Section::new(SLAB_TOP - 0.1, 1.0), Section::new(1.1, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(1.0, 0.99), Section::new(BUNKER_TOP, 0.82)],
    );
    team_panel(b, (c + v2(-0.8, 0.8)).extend(BUNKER_TOP), v2(2.4, 2.4));
    if !b.fine() {
        return;
    }
    // The blast door, on the face toward the lot's middle.
    let inward = -c.normalize();
    let door = c + inward * (BUNKER_HW * 0.86);
    b.paint(PLATING_DARK);
    b.yawed(door.extend(0.0), inward.y.atan2(inward.x), |b| {
        b.block(v3(-0.2, -1.6, SLAB_TOP), v3(0.35, 1.6, 2.2));
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(v3(0.3, -1.9, 2.2), v3(0.6, 1.9, 2.45));
    });
    let sides = b.sides(8);
    b.paint(METAL).pattern(pattern::PLAIN);
    b.prism(
        (c + v2(1.4, -1.4)).extend(BUNKER_TOP - 0.1),
        sides,
        0.8,
        0.7,
        1.1,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(
        (c + v2(1.4, -1.4)).extend(BUNKER_TOP + 1.0),
        sides,
        1.0,
        1.0,
        0.25,
    );
}

// ---- the turret ---------------------------------------------------------------------

/// The turntable the carriage stands on: a dark drum with a bright race lip, its deck
/// a ring of light plates round a darker middle, ribbed and bolted, with the traverse
/// racks, hatches down into the magazine run and a walkway rail round its edge.
fn turntable(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(32);
    b.paint(PLATING_DARK);
    b.prism(
        v3(0.0, 0.0, BERM_TOP - 0.05),
        sides,
        TABLE_R,
        TABLE_R - 0.4,
        DECK - BERM_TOP - 0.25,
    );
    // The deck: light plate on the outer ring, the darker hub the carriage stands on.
    b.paint(PLATING);
    b.prism(
        v3(0.0, 0.0, DECK - 0.35),
        sides,
        TABLE_R - 0.4,
        TABLE_R - 0.7,
        0.35,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, HUB_R, HUB_R - 0.3, 0.15);
    if !fine {
        return;
    }
    b.paint(METAL).pattern(pattern::PLAIN);
    b.prism(
        v3(0.0, 0.0, BERM_TOP + 0.35),
        sides,
        TABLE_R + 0.1,
        TABLE_R + 0.1,
        0.25,
    );
    // Radial ribs across the plate ring, a bolt head between each pair.
    const RIBS: usize = 24;
    for k in 0..RIBS {
        let yaw = std::f32::consts::TAU * k as f32 / RIBS as f32;
        b.yawed(Vec3::ZERO, yaw, |b| {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.block(
                v3(HUB_R, -0.22, DECK - 0.05),
                v3(TABLE_R - 1.0, 0.22, DECK + 0.18),
            );
            b.paint(METAL).pattern(pattern::PLAIN);
            b.yawed(Vec3::ZERO, std::f32::consts::PI / RIBS as f32, |b| {
                b.block(
                    v3(TABLE_R - 1.9, -0.3, DECK - 0.05),
                    v3(TABLE_R - 1.3, 0.3, DECK + 0.22),
                );
            });
        });
    }
    // Walkway rail posts round the rim, and two hatches down to the magazine run on the
    // flanks of the carriage.
    for k in 0..16 {
        let yaw = std::f32::consts::TAU * (k as f32 + 0.5) / 16.0;
        b.yawed(Vec3::ZERO, yaw, |b| {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.block(
                v3(TABLE_R - 0.9, -0.12, DECK),
                v3(TABLE_R - 0.65, 0.12, DECK + 1.1),
            );
        });
    }
    b.mirror_y(|b| {
        b.paint(PLATING_DARK);
        b.chamfered_box(v3(-4.0, HUB_R + 2.6, DECK + 0.15), v3(3.6, 2.6, 0.4), 0.4);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for x in [-5.0, -4.0, -3.0] {
            b.block(
                v3(x - 0.12, HUB_R + 1.5, DECK + 0.35),
                v3(x + 0.12, HUB_R + 3.7, DECK + 0.45),
            );
        }
    });
}

/// The carriage: two long armoured side hulls with the well open between them, a
/// bridge ahead of the well and the loading deck behind it, the cheeks on the side
/// hulls, and a rangefinder on the left hull.
fn carriage(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (f, r, hw) = (CARRIAGE_FRONT, CARRIAGE_REAR, CARRIAGE_HW);
    // The dark floor of the well.
    b.paint(TREAD).pattern(pattern::NONE);
    b.decal(
        v3((WELL_FRONT + WELL_REAR) * 0.5, 0.0, DECK + 0.02),
        v2(WELL_FRONT - WELL_REAR, 2.0 * WELL_HW),
    );
    b.mirror_y(|b| {
        // A side hull: sloped glacis at the front, sloped flank, flat top.
        b.paint(PLATING);
        let plan = [
            [f, WELL_HW],
            [f, hw - 3.0],
            [f - 3.5, hw],
            [r + 2.5, hw],
            [r, hw - 2.5],
            [r, WELL_HW],
        ];
        b.loft_z(
            &plan,
            &[
                Section::new(DECK - 0.05, 1.0),
                Section::new(HULL_TOP - 1.0, 1.0),
                Section::scaled(HULL_TOP, 0.97, 0.94),
            ],
        );
        team_panel(b, v3(-5.0, 8.4, HULL_TOP), v2(4.4, 2.4));
        // The cheek: a sloped armoured wall up to the trunnion, a boss over it.
        b.paint(PLATING);
        b.extrude_y(
            &[
                [-CHEEK_FOOT, HULL_TOP - 0.1],
                [CHEEK_FOOT, HULL_TOP - 0.1],
                [3.2, TRUNNION.z + 1.9],
                [-3.2, TRUNNION.z + 1.9],
            ],
            CHEEK_IN,
            CHEEK_IN + CHEEK_T,
        );
        let sides = b.sides(12);
        b.paint(PLATING_DARK);
        b.cylinder_between(
            v3(0.0, CHEEK_IN + CHEEK_T - 0.1, TRUNNION.z),
            v3(0.0, CHEEK_IN + CHEEK_T + 0.7, TRUNNION.z),
            1.9,
            1.6,
            sides,
        );
        if !fine {
            return;
        }
        // A buttress down the cheek's outer face onto the hull.
        b.paint(PLATING);
        b.extrude_x(
            &[
                [CHEEK_IN + CHEEK_T - 0.1, HULL_TOP - 0.1],
                [CHEEK_IN + CHEEK_T + 3.0, HULL_TOP - 0.1],
                [CHEEK_IN + CHEEK_T - 0.1, TRUNNION.z - 2.2],
            ],
            -1.4,
            1.4,
        );
        // Walkway rails along the hull top, and the dark hatches in it.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(r + 3.0, hw - 1.2, HULL_TOP),
            v3(f - 4.5, hw - 0.9, HULL_TOP + 1.0),
        );
        b.paint(PLATING_DARK);
        for x in [-12.0, 3.5] {
            b.block(
                v3(x - 1.2, 7.4, HULL_TOP - 0.02),
                v3(x + 1.2, 9.6, HULL_TOP + 0.1),
            );
        }
        // The side skirts over the turntable's edge.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(r + 3.0, hw - 0.1, DECK + 0.4),
            v3(f - 4.0, hw + 0.3, DECK + 1.6),
        );
    });
    // The bridge across the front of the well and the loading deck behind it.
    b.paint(PLATING);
    b.block(
        v3(WELL_FRONT, -WELL_HW - 0.1, DECK),
        v3(f, WELL_HW + 0.1, HULL_TOP - 1.2),
    );
    b.block(
        v3(r, -WELL_HW - 0.1, DECK),
        v3(WELL_REAR, WELL_HW + 0.1, HULL_TOP - 0.6),
    );
    if fine {
        rangefinder(b);
    }
}

/// The optical rangefinder on the left hull: a pedestal, a long bar across it with a
/// dark window at each end.
fn rangefinder(b: &mut MeshBuilder) {
    let at = v3(-9.0, -8.6, HULL_TOP);
    let sides = b.sides(8);
    b.paint(PLATING);
    b.prism(at, sides, 1.1, 0.9, 2.2);
    b.chamfered_box(at + v3(0.0, 0.0, 2.8), v3(1.6, 6.0, 1.2), 0.3);
    b.paint(TREAD).pattern(pattern::NONE);
    for y in [-3.02, 3.02] {
        b.block(at + v3(0.2, y - 0.02, 2.5), at + v3(0.85, y + 0.02, 3.1));
    }
}

/// The loading house at the carriage's back: a sloped armoured box straddling the
/// loading deck, the shell hoist's tower on its back, a rammer tray out of its front
/// face lined up on the breech, and a jib crane on its roof.
fn house(b: &mut MeshBuilder) {
    let fine = b.fine();
    let plan = chamfered_rect(v2((HOUSE_FRONT - HOUSE_REAR) * 0.5, HOUSE_HW), 1.4);
    let mid = (HOUSE_FRONT + HOUSE_REAR) * 0.5;
    b.paint(PLATING);
    b.at(v3(mid, 0.0, 0.0), |b| {
        b.loft_z(
            &plan,
            &[
                Section::new(HULL_TOP - 1.0, 1.0),
                Section::new(HOUSE_TOP - 1.2, 0.97),
                Section::scaled(HOUSE_TOP, 0.9, 0.86),
            ],
        );
    });
    // The hoist tower on the back, taller than the house.
    b.paint(PLATING);
    b.chamfered_box(
        v3(HOUSE_REAR + 1.6, 0.0, (DECK + HOUSE_TOP + 2.5) * 0.5),
        v3(3.6, 5.0, HOUSE_TOP + 2.5 - DECK),
        0.6,
    );
    team_panel(b, v3(mid + 1.0, 0.0, HOUSE_TOP), v2(4.0, 5.0));
    // The loading port in the front face, and the rammer tray out of it at the
    // breech's height when level.
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(
        v3(HOUSE_FRONT - 0.1, -1.8, TRUNNION.z - 2.0),
        v3(HOUSE_FRONT + 0.05, 1.8, TRUNNION.z + 1.4),
    );
    b.paint(METAL).pattern(pattern::PLAIN);
    b.block(
        v3(HOUSE_FRONT, -1.1, TRUNNION.z - 2.3),
        v3(HOUSE_FRONT + 2.4, 1.1, TRUNNION.z - 1.9),
    );
    if !fine {
        return;
    }
    // The jib crane on the roof: a mast and a boom out over the side.
    let sides = b.sides(8);
    b.paint(PLATING);
    b.prism(v3(mid - 2.0, 4.0, HOUSE_TOP - 0.1), sides, 0.6, 0.5, 4.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        v3(mid - 2.0, 4.0, HOUSE_TOP + 3.6),
        v3(mid - 2.0, 11.5, HOUSE_TOP + 2.2),
        v2(0.6, 0.8),
        v2(0.4, 0.5),
    );
    cable(
        b,
        &[
            v3(mid - 2.0, 11.3, HOUSE_TOP + 2.1),
            v3(mid - 2.0, 11.3, HOUSE_TOP - 2.0),
        ],
        0.08,
    );
    // Vents down the house's flanks and a stack of shell racks on the loading deck.
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for k in 0..3 {
            let x = HOUSE_REAR + 3.0 + 1.6 * k as f32;
            b.block(
                v3(x, HOUSE_HW - 0.1, HOUSE_TOP - 3.5),
                v3(x + 0.9, HOUSE_HW + 0.2, HOUSE_TOP - 1.8),
            );
        }
    });
}

// ---- the gun ----------------------------------------------------------------------

/// The cradle: a long armoured sleeve round the tube, the four recoil cylinders along
/// it, and the trunnion pins out to the cheeks.
fn cradle(b: &mut MeshBuilder) {
    let fine = b.fine();
    let profile = chamfered_rect(v2(CRADLE_HW, CRADLE_HH), CRADLE_CUT);
    let ring = |x: f32, s: f32| -> Vec<Vec3> {
        profile.iter().map(|p| v3(x, p[0] * s, p[1] * s)).collect()
    };
    b.paint(PLATING);
    b.loft(
        &[
            ring(CRADLE_BACK, 1.0),
            ring(CRADLE_FRONT - 3.0, 1.0),
            ring(CRADLE_FRONT, 0.8),
        ],
        true,
        true,
    );
    let sides = b.sides(12);
    b.paint(PLATING_DARK);
    b.cylinder_between(
        v3(0.0, -CHEEK_IN - 0.2, 0.0),
        v3(0.0, CHEEK_IN + 0.2, 0.0),
        1.2,
        1.2,
        sides,
    );
    // The recoil cylinders: two over the tube, two under it, on the cradle's corners.
    let sides = b.sides(10);
    let (cy, cz) = (CRADLE_HW * 0.78, CRADLE_HH * 0.9);
    for (y, z) in [(cy, cz), (-cy, cz), (cy, -cz), (-cy, -cz)] {
        b.paint(METAL).pattern(pattern::PLAIN);
        b.cylinder_between(
            v3(CRADLE_BACK - 1.4, y, z),
            v3(CRADLE_FRONT - 1.5, y, z),
            0.85,
            0.85,
            sides,
        );
        if fine {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.cylinder_between(
                v3(CRADLE_BACK - 1.6, y, z),
                v3(CRADLE_BACK - 0.7, y, z),
                1.05,
                1.05,
                sides,
            );
        }
    }
    if !fine {
        return;
    }
    // Armour bands across the sleeve's back.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for x in [-3.0, 3.0, 8.5] {
        b.loft(&[ring(x, 1.03), ring(x + 0.6, 1.03)], true, true);
    }
}

/// The gun, riding back on recoil: the breech ring, the tube with its hoops, the
/// dark bore at the muzzle, the king-post and its two cables.
fn gun(b: &mut MeshBuilder) {
    let fine = b.fine();
    // The breech ring: a heavy square block behind the cradle, the dark breech block
    // face on its back.
    b.paint(PLATING_DARK);
    b.chamfered_box(
        v3((BREECH_BACK + BREECH_FRONT) * 0.5, 0.0, 0.0),
        v3(BREECH_FRONT - BREECH_BACK, 2.0 * BREECH_HW, 2.0 * BREECH_HH),
        0.5,
    );
    b.paint(METAL).pattern(pattern::PLAIN);
    b.block(
        v3(BREECH_BACK - 0.35, -1.3, -1.3),
        v3(BREECH_BACK + 0.05, 1.3, 1.3),
    );
    // The tube: tapering out of the cradle to its waist, then long and slim to the muzzle.
    let sides = b.sides(16);
    b.paint(PLATING_DARK);
    b.cylinder_between(
        v3(BREECH_FRONT, 0.0, 0.0),
        v3(TUBE_WAIST, 0.0, 0.0),
        TUBE_R,
        TUBE_WAIST_R,
        sides,
    );
    b.cylinder_between(
        v3(TUBE_WAIST, 0.0, 0.0),
        v3(MUZZLE, 0.0, 0.0),
        TUBE_WAIST_R,
        TUBE_MUZZLE_R,
        sides,
    );
    // The muzzle: a plain swell and the dark bore.
    b.paint(PLATING);
    b.cylinder_between(
        v3(MUZZLE - 2.2, 0.0, 0.0),
        v3(MUZZLE, 0.0, 0.0),
        TUBE_MUZZLE_R * 1.12,
        TUBE_MUZZLE_R * 1.12,
        sides,
    );
    b.paint(TREAD).pattern(pattern::NONE);
    b.cylinder_between(
        v3(MUZZLE - 0.05, 0.0, 0.0),
        v3(MUZZLE + 0.02, 0.0, 0.0),
        TUBE_MUZZLE_R * 0.62,
        TUBE_MUZZLE_R * 0.62,
        sides,
    );
    // The king-post and its cables: the tube hangs straight from them.
    let tube_r = |x: f32| {
        if x < TUBE_WAIST {
            TUBE_R + (TUBE_WAIST_R - TUBE_R) * (x - BREECH_FRONT) / (TUBE_WAIST - BREECH_FRONT)
        } else {
            TUBE_WAIST_R + (TUBE_MUZZLE_R - TUBE_WAIST_R) * (x - TUBE_WAIST) / (MUZZLE - TUBE_WAIST)
        }
    };
    let head = v3(POST, 0.0, POST_HEAD);
    b.paint(PLATING);
    b.beam(
        v3(POST, 0.0, tube_r(POST) - 0.2),
        head,
        v2(1.4, 0.9),
        v2(0.7, 0.5),
    );
    b.paint(METAL).pattern(pattern::PLAIN);
    for x in [STAY_BACK, STAY_FRONT] {
        b.cylinder_between(head, v3(x, 0.0, tube_r(x) * 0.8), 0.14, 0.14, 6);
    }
    if !fine {
        return;
    }
    // Clamp bands where the post and the cables hold the tube, and the hoops.
    b.paint(PLATING);
    for (x, w) in [(POST, 1.6), (STAY_FRONT, 1.0)] {
        let r = tube_r(x) * 1.14;
        b.cylinder_between(
            v3(x - w * 0.5, 0.0, 0.0),
            v3(x + w * 0.5, 0.0, 0.0),
            r,
            r,
            sides,
        );
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for x in HOOPS {
        let r = tube_r(x) * 1.07;
        b.cylinder_between(v3(x - 0.3, 0.0, 0.0), v3(x + 0.3, 0.0, 0.0), r, r, sides);
    }
    // The cable saddle on the post's head.
    b.paint(PLATING_DARK);
    b.block(head + v3(-0.6, -0.4, -0.2), head + v3(0.6, 0.4, 0.35));
}

// ---- kit ---------------------------------------------------------------------------

/// A cable through `points`.
fn cable(b: &mut MeshBuilder, points: &[Vec3], radius: f32) {
    for pair in points.windows(2) {
        b.cylinder_between(pair[0], pair[1], radius, radius, 6);
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::{MUZZLE, TRUNNION};
    use crate::{build_model_scaled, part, rig};

    /// The unit file's size (`aster_t4_artillery`): radius, height, tech.
    const SIZE: (f32, f32, u8) = (30.0, 27.0, 4);

    /// The unit file's pivot and muzzle are the model's trunnion and barrel tip, and the
    /// gun reaches the muzzle at every level of detail.
    #[test]
    fn culverin_gun_ends_at_the_muzzle_and_pitches_about_the_trunnion() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&data).unwrap();
        let bp = blueprints.unit(blueprints.id_of("aster_t4_artillery").unwrap());
        let w = &bp.weapons[0];
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let pivot = v(w.pivot.expect("the Culverin's gun elevates"));
        assert!(pivot.distance(TRUNNION) < 1e-2, "data pivot {pivot}");
        let tip = TRUNNION + Vec3::X * MUZZLE;
        assert!(v(w.muzzle).distance(tip) < 1e-2, "data muzzle vs {tip}");
        let model = build_model_scaled("culverin", SIZE.0, SIZE.1, SIZE.2).unwrap();
        assert_eq!(model.arm_pivot, Some(TRUNNION.to_array()));
        for (l, lod) in model.lods.iter().enumerate() {
            let far = lod
                .vertices
                .iter()
                .filter(|v| {
                    v.part == part::TURRET
                        && v.rig & rig::LIMB_MASK == rig::ARM_GUN
                        && v.rig & rig::RECOIL != 0
                })
                .map(|v| v.pos[0])
                .fold(f32::MIN, f32::max);
            assert!(
                (far - tip.x).abs() < 0.2,
                "lod {l}: the gun ends at x {far}, the muzzle is at {}",
                tip.x
            );
        }
    }
}

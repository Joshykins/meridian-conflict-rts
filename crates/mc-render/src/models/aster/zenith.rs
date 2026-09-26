//! Zenith (`aster_t4_anti_ship`, mesh "anti_ship_rail"): the tech 4 anti-ship rail
//! cannon, authored at blueprint scale (metres, 8x8 lot, radius 46, height 86).
//!
//! One enormous rail gun that throws a slug clean out of the atmosphere, at warships
//! hanging in the clouds and above them. Big, plain slabs, no sensors of its own: it
//! shoots on what the rest of the army sees.
//! - Fixed (`part::HULL`): the lot slab; a broad two-step foundation (a sloped lower
//!   step with cut corners, an octagonal plinth on it) with heavy buttress walls down
//!   the axes and the race ring the turret rides; a power bunker in each corner, its
//!   conduit trunk run across the lower step into the plinth.
//! - Turning (`part::TURRET`, about the lot's centre): the turntable, a low armoured
//!   house, and two A-frame yokes, their slanted legs webbed and tied, meeting high up
//!   in the trunnion bearings.
//! - Elevating (`rig::ARM_GUN`, about [`TRUNNION`]): the great cradle block the gun is
//!   sunk into, the trunnion pin, and the open counterweight frame behind it that
//!   swings down between the yokes as the gun comes up. (`rig::ARM_GUN | rig::RECOIL`)
//!   the barrel itself, which kicks back through the cradle when it fires.
//!
//! The barrel is the hero: 164 m from breech to muzzle. Its breech sits in the open
//! frame behind the cradle, the cradle swallows its first 30 m, and it comes out the
//! cradle's nose as a heavy round jacket, steps down at a collar to a slimmer one with
//! the two rails showing down its flanks as spines, then the rails leave the jacket and
//! run bare, an open slot between them, through a ladder of round dark clamps to a
//! flared muzzle ring with the square bore dark in it. Nothing on it is lit
//! (docs/STYLE.md, rail guns are hardware).

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

/// The anchors of the Zenith's barrel, for effects (charge, muzzle blast, rail wakes).
///
/// Points marked "barrel frame" are in the barrel's own frame: origin at the trunnion
/// ([`ZenithRail::pivot`]), +x down the bore, +z up when the barrel is level, y to the
/// left. To place one in the world: pitch it up about y by the unit's gun pitch
/// (`arm_pitch`, the sim's `weapons[0]` slot 0), add `pivot`, yaw it by the unit's
/// heading + turret yaw about the model's z axis (the turret turns about the lot's
/// centre, (0, 0)), then scale/translate like any model point. The barrel geometry is
/// `part` with limb `limb`; `recoil` metres of kick along -x on firing.
#[derive(Clone, Copy, Debug)]
pub struct ZenithRail {
    /// The trunnion, model space (rest pose): the elevation axis, along y through it.
    /// The unit file's weapon `pivot`.
    pub pivot: [f32; 3],
    /// Barrel frame: the centre of the muzzle face. `pivot + muzzle` is the unit file's
    /// weapon `muzzle`.
    pub muzzle: [f32; 3],
    /// Barrel frame: the rear face of the breech block.
    pub breech: [f32; 3],
    /// Barrel frame: points on the bore axis from the breech to the muzzle: the breech
    /// block's front, the jacket's rear (where it leaves the cradle), mid-jacket, the
    /// jacket's end (where the rails come out bare), the middle of the bare rails, the
    /// last clamp collar.
    pub along: [[f32; 3]; 6],
    /// Barrel frame: where the bare rails run (x from, x to), and each rail's centre
    /// line offset (±y); the slot between them is open from above and below.
    pub rails: [f32; 3],
    /// Radius of the containment jacket, and of the muzzle ring's flare.
    pub jacket_radius: f32,
    pub muzzle_radius: f32,
    /// How far the barrel kicks back when it fires (`Model::recoil`).
    pub recoil: f32,
    /// The model part and rig limb the barrel is drawn with.
    pub part: u32,
    pub limb: u32,
}

/// The trunnion (model space): keep `weapons[0].pivot` in `space.ron` equal to it.
pub const TRUNNION: Vec3 = Vec3::new(0.0, 0.0, 78.0);

/// The Zenith's barrel anchors: see [`ZenithRail`].
pub const ZENITH_RAIL: ZenithRail = ZenithRail {
    pivot: [TRUNNION.x, TRUNNION.y, TRUNNION.z],
    muzzle: [MUZZLE, 0.0, 0.0],
    breech: [BREECH, 0.0, 0.0],
    along: [
        [BREECH_FRONT, 0.0, 0.0],
        [JACKET_FROM, 0.0, 0.0],
        [(JACKET_FROM + JACKET_TO) * 0.5, 0.0, 0.0],
        [JACKET_TO, 0.0, 0.0],
        [(JACKET_TO + RAILS_TO) * 0.5, 0.0, 0.0],
        [LAST_COLLAR, 0.0, 0.0],
    ],
    rails: [JACKET_TO, RAILS_TO, RAIL_Y],
    jacket_radius: JACKET_R,
    muzzle_radius: MUZZLE_R,
    recoil: RECOIL,
    part: part::TURRET,
    limb: rig::ARM_GUN | rig::RECOIL,
};

// ---- the gun (barrel frame: metres from the trunnion along the bore) -----------------

/// The breech block's rear face and front, and its radius. It sits in the open frame
/// behind the cradle, its front inside the cradle.
const BREECH: f32 = -29.0;
const BREECH_FRONT: f32 = -19.0;
const BREECH_R: f32 = 4.4;
/// The cradle: its rear face, its nose (where the jacket comes out), half its width
/// and height.
const CRADLE_BACK: f32 = -21.0;
const CRADLE_NOSE: f32 = 19.0;
const CRADLE_HW: f32 = 7.0;
const CRADLE_HH: f32 = 7.5;
/// The counterweight frame behind the cradle: its rails' offset (±y) and height (±z),
/// and the counterweight block at its end (from, to).
const FRAME_Y: f32 = 6.2;
const FRAME_Z: f32 = 5.8;
const WEIGHT_FROM: f32 = -33.5;
const WEIGHT_TO: f32 = -36.5;
/// The heavy jacket, from inside the cradle to the step collar, and its radius.
const JACKET_FROM: f32 = CRADLE_NOSE;
const JACKET_R: f32 = 4.1;
const STEP: f32 = 50.0;
/// The slim jacket after the step, to where the rails come out.
const JACKET_TO: f32 = 90.0;
const SLIM_R: f32 = 3.4;
/// The bare rails: to the muzzle ring; each rail's centre line off the bore.
const RAILS_TO: f32 = 129.0;
const RAIL_Y: f32 = 1.9;
/// Half a rail's width (y) and height (z).
const RAIL_HW: f32 = 0.65;
const RAIL_HH: f32 = 1.7;
/// The last round clamp before the muzzle ring.
const LAST_COLLAR: f32 = 124.0;
/// The muzzle face, and the flare of the ring there.
const MUZZLE: f32 = 135.0;
const MUZZLE_R: f32 = 4.0;
const RECOIL: f32 = 3.0;

// ---- the turret (model space) -----------------------------------------------------

/// The turntable's top, where the house stands, and the house's roof.
const DECK: f32 = 16.5;
const ROOF: f32 = 26.5;
/// The yokes: inner face, thickness (y).
const CHEEK_IN: f32 = 8.0;
const CHEEK_T: f32 = 7.0;
/// Where each yoke's front and rear legs stand on the house (x), and where they meet
/// the bearing block (±x).
const LEG_FRONT: f32 = 24.0;
const LEG_REAR: f32 = -29.0;
const LEG_HEAD: f32 = 5.5;
/// The tie across each yoke's legs; the web plate fills the yoke below it.
const TIE: f32 = 48.0;

// ---- the base ----------------------------------------------------------------------

/// The slab's half width and top.
const SLAB: f32 = 47.6;
const SLAB_TOP: f32 = 1.4;
/// The lower foundation: half width, corner chamfer, top.
const FOUND: f32 = 40.0;
const FOUND_TOP: f32 = 8.0;
/// The upper plinth: radius (to its corners) at its foot, and its top.
const PLINTH_R: f32 = 34.0;
const PLINTH_TOP: f32 = 15.0;
/// The power bunkers in the lot's corners: centre (±, ±), half width at the foot, top.
const BUNKER_C: f32 = 36.5;
const BUNKER_HW: f32 = 10.0;
const BUNKER_TOP: f32 = 17.0;

pub fn zenith(b: &mut MeshBuilder, _tech: u8) {
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
        bunker(b, v2(sx, sy));
    }
    b.with_part(part::TURRET, |b| {
        turntable(b);
        house(b);
        b.mirror_y(yoke);
        b.with_limb(rig::ARM_GUN, |b| b.at(TRUNNION, cradle));
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| b.at(TRUNNION, barrel));
    });
}

/// Far off: the foundation, the house, the yokes as one block, the cradle and barrel
/// as two long bars, and the owner's colour on the house. Under 60 triangles.
fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.frustum_open(v3(0.0, 0.0, 0.0), v2(2.0 * FOUND, 2.0 * FOUND), v2(2.0 * FOUND - 12.0, 2.0 * FOUND - 12.0), PLINTH_TOP, Vec2::ZERO);
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        b.cuboid_open(v3(-3.5, 0.0, (DECK + ROOF) * 0.5), v3(59.0, 42.0, ROOF - DECK));
        b.cuboid_open(
            v3(-2.0, 0.0, (ROOF + TRUNNION.z + 4.0) * 0.5),
            v3(22.0, 2.0 * (CHEEK_IN + CHEEK_T), TRUNNION.z + 4.0 - ROOF),
        );
        team_panel(b, v3(-18.0, 0.0, ROOF), v2(7.0, 24.0));
        b.with_limb(rig::ARM_GUN, |b| {
            b.at(TRUNNION, |b| {
                b.paint(PLATING);
                b.loft(&[square(WEIGHT_TO, CRADLE_HW, CRADLE_HH), square(CRADLE_NOSE, CRADLE_HW, CRADLE_HH)], false, false);
            });
        });
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
            b.at(TRUNNION, |b| {
                // An uncapped square bar: its ends are never seen from this far.
                b.paint(PLATING_DARK);
                b.loft(&[square(BREECH, 3.8, 3.8), square(MUZZLE, 3.0, 3.0)], false, false);
            });
        });
    });
}

/// A square ring across the bore at `x`, for the coarse bars.
fn square(x: f32, hw: f32, hh: f32) -> Vec<Vec3> {
    vec![v3(x, -hw, -hh), v3(x, hw, -hh), v3(x, hw, hh), v3(x, -hw, hh)]
}

// ---- the base ----------------------------------------------------------------------

/// The lot's slab: a dark foot and a light deck.
fn slab(b: &mut MeshBuilder) {
    let plan = chamfered_rect(v2(SLAB, SLAB), 5.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(0.6, 1.0)]);
    b.paint(PLATING);
    b.loft_z(&plan, &[Section::new(0.6, 0.99), Section::new(SLAB_TOP, 0.98)]);
}

/// The fixed foundation the turret turns on: a broad sloped lower step with its
/// corners cut, an octagonal upper plinth stepped in on it, four heavy buttress walls
/// down the axes onto the slab, and the race ring on top.
fn plinth(b: &mut MeshBuilder) {
    let fine = b.fine();
    let lower = chamfered_rect(v2(FOUND, FOUND), 13.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&lower, &[Section::new(SLAB_TOP - 0.1, 1.0), Section::new(2.6, 0.995)]);
    b.paint(PLATING);
    b.loft_z(&lower, &[Section::new(2.5, 0.985), Section::new(FOUND_TOP, 0.935)]);
    let upper = ngon(8, PLINTH_R);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&upper, &[Section::new(FOUND_TOP - 0.1, 1.0), Section::new(FOUND_TOP + 1.2, 0.99)]);
    b.paint(PLATING);
    b.loft_z(
        &upper,
        &[Section::new(FOUND_TOP + 1.1, 0.975), Section::new(FOUND_TOP + 3.8, 0.93), Section::new(PLINTH_TOP, 0.88)],
    );
    // The fixed race the turntable rides.
    let sides = b.sides(16);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(0.0, 0.0, PLINTH_TOP - 0.2), sides, 28.6, 28.2, 0.7);
    // Buttress walls down the four axes, from the plinth's shoulder to the slab's edge.
    for k in 0..4 {
        let yaw = std::f32::consts::FRAC_PI_2 * k as f32;
        b.yawed(Vec3::ZERO, yaw, |b| {
            b.paint(PLATING);
            b.extrude_y(
                &[[27.0, SLAB_TOP], [45.5, SLAB_TOP], [45.5, 4.0], [32.0, PLINTH_TOP - 0.6], [27.0, PLINTH_TOP - 0.6]],
                -3.4,
                3.4,
            );
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.block(v3(42.0, -4.2, SLAB_TOP), v3(46.2, 4.2, SLAB_TOP + 1.1));
            if fine {
                // A dark rib down the wall's back and bolted plates either side of it.
                b.beam(v3(44.8, 0.0, 4.2), v3(32.4, 0.0, PLINTH_TOP - 0.3), v2(1.6, 0.5), v2(1.6, 0.5));
                b.paint(PLATING_DARK);
                b.mirror_y(|b| b.block(v3(33.0, 3.35, 3.4), v3(40.0, 3.6, 8.4)));
            }
        });
    }
    if !fine {
        return;
    }
    // A dark trim round the lower step's top edge, and a door in its -x face.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&lower, &[Section::new(FOUND_TOP - 0.05, 0.94), Section::new(FOUND_TOP + 0.35, 0.935)]);
    b.mirror_y(|b| {
        b.block(v3(-38.9, 8.0, SLAB_TOP), v3(-37.6, 13.0, 5.4));
        b.paint(PLATING_DARK);
        b.block(v3(-39.1, 8.6, SLAB_TOP + 0.1), v3(-38.7, 12.4, 5.0));
        b.paint(ACCENT).pattern(pattern::PLAIN);
    });
}

/// One power bunker in a corner (`corner` is (±1, ±1)): a squat sloped blockhouse
/// with a dark roof, a raised switch deck and louvres on it, an armoured door facing
/// out, and a heavy conduit trunk across the lower step into the plinth.
fn bunker(b: &mut MeshBuilder, corner: Vec2) {
    let fine = b.fine();
    let c = corner * BUNKER_C;
    let plan: Vec<[f32; 2]> = chamfered_rect(v2(BUNKER_HW, BUNKER_HW), 2.4).iter().map(|p| [p[0] + c.x, p[1] + c.y]).collect();
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&plan, &[Section::new(SLAB_TOP - 0.1, 1.0), Section::new(2.4, 1.0)]);
    b.paint(PLATING);
    b.loft_z(&plan, &[Section::new(2.3, 0.99), Section::new(BUNKER_TOP - 1.0, 0.86), Section::new(BUNKER_TOP, 0.8)]);
    b.paint(PLATING_DARK);
    b.decal(c.extend(BUNKER_TOP + 0.02), v2(BUNKER_HW * 1.5, BUNKER_HW * 1.5));
    // The conduit trunk: from the bunker's inner face over the lower step to the plinth.
    let inward = -corner.normalize();
    let from = c + inward * (BUNKER_HW * 0.9);
    let into = inward * -(PLINTH_R * 0.86);
    b.paint(PLATING);
    b.beam(from.extend(FOUND_TOP + 2.4), into.extend(FOUND_TOP + 3.4), v2(6.0, 5.0), v2(6.0, 5.0));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(from.extend(FOUND_TOP + 5.1), into.extend(FOUND_TOP + 6.1), v2(4.2, 0.5), v2(4.2, 0.5));
    team_panel(b, (c + inward * 3.0).extend(BUNKER_TOP), v2(4.0, 4.0));
    if !fine {
        return;
    }
    // A raised switch deck with louvres on the roof's outer half.
    let out = c - inward * 2.6;
    b.paint(PLATING);
    b.chamfered_box(out.extend(BUNKER_TOP + 1.2), v3(8.0, 8.0, 2.4), 0.9);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for k in -2..=2 {
        let o = out + v2(k as f32 * 1.4, 0.0);
        b.block(v3(o.x - 0.3, o.y - 3.0, BUNKER_TOP + 2.4), v3(o.x + 0.3, o.y + 3.0, BUNKER_TOP + 2.8));
    }
    // An armoured door in each outward face.
    for (axis, sign) in [(0usize, corner.x), (1usize, corner.y)] {
        let face = c[axis] + sign * (BUNKER_HW * 0.965);
        let (lo, hi) = (face.min(face + sign * 0.5), face.max(face + sign * 0.5));
        let (a, z) = if axis == 0 {
            (v3(lo, c.y - 2.6, SLAB_TOP + 1.0), v3(hi, c.y + 2.6, 8.0))
        } else {
            (v3(c.x - 2.6, lo, SLAB_TOP + 1.0), v3(c.x + 2.6, hi, 8.0))
        };
        b.paint(PLATING_DARK);
        b.block(a, z);
    }
    // Fat feeders laid along the trunk.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let side = v2(-inward.y, inward.x);
    for k in [-1.0, 1.0] {
        let o = side * (k * 3.1);
        cable(b, &[(from + o).extend(FOUND_TOP + 0.6), (from + inward * 4.0 + o).extend(FOUND_TOP + 1.6), (into + o).extend(FOUND_TOP + 2.2)], 0.6);
    }
}

// ---- the turret ---------------------------------------------------------------------

/// The turning race and the turntable ring the house stands on.
fn turntable(b: &mut MeshBuilder) {
    let sides = b.sides(16);
    b.paint(PLATING_DARK);
    b.prism(v3(0.0, 0.0, PLINTH_TOP - 0.05), sides, 28.0, 27.6, DECK - PLINTH_TOP + 0.05);
    if b.fine() {
        b.paint(METAL).pattern(pattern::PLAIN);
        b.prism(v3(0.0, 0.0, PLINTH_TOP + 0.4), sides, 28.15, 28.15, 0.3);
    }
}

/// The house: a long low armoured shell sloped in on every side, a glacis at the
/// front, a dark well between the yokes for the counterweight to swing through, and
/// the owner's colour either side.
fn house(b: &mut MeshBuilder) {
    let plan: Vec<[f32; 2]> = vec![
        [26.0, -13.0],
        [26.0, 13.0],
        [19.0, 21.0],
        [-27.0, 21.0],
        [-33.0, 16.0],
        [-33.0, -16.0],
        [-27.0, -21.0],
        [19.0, -21.0],
    ];
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(DECK - 0.05, 1.0),
            Section::scaled(ROOF - 3.4, 0.985, 0.98),
            Section::scaled(ROOF, 0.9, 0.93).shifted(-1.5, 0.0),
        ],
    );
    // The well between the yokes, and the owner's colour on the rear roof.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.decal(v3(-2.5, 0.0, ROOF + 0.02), v2(44.0, 2.0 * CHEEK_IN - 0.6));
    team_panel(b, v3(-24.0, 13.5, ROOF), v2(5.6, 4.4));
    team_panel(b, v3(-24.0, -13.5, ROOF), v2(5.6, 4.4));
    if !b.fine() {
        return;
    }
    // A dark band round the house's waist, hatches on the roof and slits down the flanks.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&plan, &[Section::scaled(DECK + 3.7, 1.0, 1.0), Section::scaled(DECK + 4.4, 0.996, 0.994)]);
    b.paint(PLATING_DARK);
    for y in [-18.2, 18.2] {
        b.plate(v3(-5.0, y, ROOF - 0.4), v2(7.0, 2.6), 0.45, 0.15);
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        for x in [-22.0, -12.0, 6.0] {
            b.block(v3(x - 2.0, 20.55, DECK + 5.4), v3(x + 2.0, 20.8, DECK + 6.2));
        }
    });
}

/// One yoke (+y; mirrored): a front and a rear leg, slanted in to meet at the trunnion
/// bearing, tied across, a web plate filling the yoke below the tie, a heavy foot beam
/// along the house, and a bus of feed cables up the rear leg into the bearing.
fn yoke(b: &mut MeshBuilder) {
    let fine = b.fine();
    let y = CHEEK_IN + CHEEK_T * 0.5;
    let top = TRUNNION.z - 4.0;
    let t = CHEEK_T;
    // Where a leg standing at `foot` runs at height `h`.
    let at = |foot: f32, h: f32| {
        let head = if foot > 0.0 { LEG_HEAD } else { -LEG_HEAD };
        foot + (head - foot) * (h - ROOF) / (top - ROOF)
    };
    b.paint(PLATING);
    // The foot beam along the house the legs stand on.
    b.chamfered_box(v3((LEG_FRONT + LEG_REAR) * 0.5, y, ROOF + 1.6), v3(LEG_FRONT - LEG_REAR + 12.0, t + 0.8, 4.4), 1.0);
    // The legs.
    b.beam(v3(LEG_FRONT, y, ROOF), v3(LEG_HEAD, y, top), v2(t, 11.0), v2(t, 8.0));
    b.beam(v3(LEG_REAR, y, ROOF), v3(-LEG_HEAD, y, top), v2(t, 11.0), v2(t, 8.0));
    // The web between them below the tie, set in from both faces.
    let web = [
        [at(LEG_REAR, ROOF + 2.0), ROOF + 2.0],
        [at(LEG_FRONT, ROOF + 2.0), ROOF + 2.0],
        [at(LEG_FRONT, TIE), TIE],
        [at(LEG_REAR, TIE), TIE],
    ];
    b.extrude_y(&web, CHEEK_IN + 1.4, CHEEK_IN + t - 1.4);
    // The bearing block where they meet.
    b.chamfered_box(v3(TRUNNION.x, y, TRUNNION.z - 0.5), v3(18.0, t, 16.0), 3.0);
    // The tie across the legs over the web, dark.
    b.paint(PLATING_DARK);
    b.beam(v3(at(LEG_REAR, TIE) - 1.0, y, TIE), v3(at(LEG_FRONT, TIE) + 1.0, y, TIE), v2(t + 0.6, 3.4), v2(t + 0.6, 3.4));
    // The bearing's boss and steel cap on the outer face.
    let out = CHEEK_IN + CHEEK_T;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(
        v3(TRUNNION.x, out - 0.1, TRUNNION.z),
        v3(TRUNNION.x, out + 1.4, TRUNNION.z),
        5.4,
        4.9,
        b.sides(12),
    );
    if !fine {
        return;
    }
    b.paint(METAL);
    b.cylinder_between(
        v3(TRUNNION.x, out + 1.4, TRUNNION.z),
        v3(TRUNNION.x, out + 2.0, TRUNNION.z),
        2.9,
        2.3,
        8,
    );
    // Dark armour plates down the legs' outer faces, and a rib across the web.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for (foot, head) in [(LEG_FRONT, LEG_HEAD), (LEG_REAR, -LEG_HEAD)] {
        let a = v3(foot + (head - foot) * 0.1, out + 0.1, ROOF + (top - ROOF) * 0.1);
        let z = v3(foot + (head - foot) * 0.6, out + 0.1, ROOF + (top - ROOF) * 0.6);
        b.beam(a, z, v2(0.4, 6.0), v2(0.4, 5.0));
    }
    let mid = (ROOF + TIE) * 0.5;
    b.beam(v3(at(LEG_REAR, mid) + 4.0, out - 1.2, mid), v3(at(LEG_FRONT, mid) - 4.0, out - 1.2, mid), v2(0.5, 1.6), v2(0.5, 1.6));
    // Feed cables up the rear leg's outer face into the bearing.
    for (k, d) in [(0.0, -1.4), (1.0, 1.4)] {
        let yy = out + 0.9 + 0.2 * k;
        let foot = v3(LEG_REAR + 4.0 + d, yy, ROOF + 2.0);
        let mid = v3(at(LEG_REAR, 50.0) + 3.0 + d, yy, 50.0);
        let into = v3(TRUNNION.x - 6.4 + d * 0.4, yy, TRUNNION.z - 4.4);
        cable(b, &[foot, mid, into], 0.65);
    }
}

// ---- the gun (barrel frame: origin at the trunnion, +x down the bore) ----------------

/// The cradle the gun is sunk into: a great angular block with a sloped nose, dark
/// bands round it, the trunnion pin through it, and the open counterweight frame
/// behind it round the breech.
fn cradle(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (hh, hw) = (CRADLE_HH, CRADLE_HW);
    let profile = [
        [CRADLE_BACK, -hh],
        [CRADLE_NOSE - 6.0, -hh],
        [CRADLE_NOSE, -hh + 2.7],
        [CRADLE_NOSE, hh - 2.7],
        [CRADLE_NOSE - 6.0, hh],
        [CRADLE_BACK, hh],
    ];
    b.paint(PLATING);
    b.extrude_y_chamfered(&profile, hw, 0.8);
    // Dark bands round the block, either side of the trunnion.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for x in [-12.0, 8.0] {
        b.cuboid(v3(x, 0.0, 0.0), v3(1.6, 2.0 * hw + 0.5, 2.0 * hh + 0.5));
    }
    // The nose ring the jacket comes out of.
    b.cylinder_between(
        v3(CRADLE_NOSE - 0.2, 0.0, 0.0),
        v3(CRADLE_NOSE + 1.0, 0.0, 0.0),
        JACKET_R + 0.9,
        JACKET_R + 0.6,
        b.sides(16),
    );
    // The trunnion pin, through the yokes.
    b.paint(METAL);
    b.cylinder_between(v3(0.0, -CHEEK_IN - CHEEK_T, 0.0), v3(0.0, CHEEK_IN + CHEEK_T, 0.0), 2.6, 2.6, b.sides(10));
    team_panel(b, v3(-4.0, 0.0, hh), v2(4.0, 8.0));

    // The counterweight frame: top and bottom rails either side, and the weight.
    b.paint(PLATING);
    b.mirror_y(|b| {
        for z in [-FRAME_Z, FRAME_Z] {
            b.beam(v3(CRADLE_BACK + 0.5, FRAME_Y, z), v3(WEIGHT_FROM, FRAME_Y, z), v2(1.6, 1.8), v2(1.6, 1.8));
        }
        if fine {
            // A diagonal brace across each side of the frame.
            b.paint(PLATING_DARK);
            b.beam(
                v3(CRADLE_BACK + 0.5, FRAME_Y + 0.2, -FRAME_Z),
                v3(WEIGHT_FROM, FRAME_Y + 0.2, FRAME_Z),
                v2(1.0, 1.2),
                v2(1.0, 1.2),
            );
            b.paint(PLATING);
        }
    });
    b.chamfered_box(v3((WEIGHT_FROM + WEIGHT_TO) * 0.5, 0.0, 0.0), v3(WEIGHT_FROM - WEIGHT_TO, 2.0 * hw, 2.0 * hh - 1.0), 0.8);
    if !fine {
        return;
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(WEIGHT_TO - 0.3, -hw + 1.2, -hh + 1.6), v3(WEIGHT_TO + 0.05, hw - 1.2, hh - 1.6));
    // Access hatches and a vent grille on the cradle's flanks.
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.block(v3(-8.0, hw - 0.05, -3.2), v3(-2.5, hw + 0.25, 3.2));
        for k in 0..4 {
            let x = 11.0 + 1.4 * k as f32;
            b.block(v3(x, hw - 0.05, -3.8), v3(x + 0.6, hw + 0.3, 3.8));
        }
    });
    // Bus cables over the cradle's back into the breech.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for y in [-3.2, -1.1, 1.1, 3.2] {
        cable(b, &[v3(-9.0, y, hh + 0.4), v3(CRADLE_BACK + 1.0, y, hh + 0.4), v3(BREECH + 5.0, y * 0.8, BREECH_R + 0.3)], 0.42);
    }
}

/// The barrel: see the module notes. Drawn along +x from [`BREECH`] to [`MUZZLE`];
/// between the breech block and the cradle's nose it is hidden in the cradle, so it
/// is not drawn there (6 m of jacket start inside, so the kick shows no gap).
fn barrel(b: &mut MeshBuilder) {
    let fine = b.fine();
    // Round, but only just at the middle distance, where the barrel is a thin line.
    let sides = if fine { 16 } else { 8 };
    // Collars and bands: rings round the round parts, a little coarser.
    let band = if fine { 12 } else { 8 };
    let ring = |b: &mut MeshBuilder, x0: f32, x1: f32, r: f32| {
        b.cylinder_between(v3(x0, 0.0, 0.0), v3(x1, 0.0, 0.0), r, r, band);
    };

    // The breech: a heavy round block, a dark end plate and boss, bands, fins.
    b.paint(PLATING);
    b.cylinder_between(v3(BREECH, 0.0, 0.0), v3(BREECH_FRONT, 0.0, 0.0), BREECH_R, BREECH_R * 0.95, sides);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(v3(BREECH - 0.6, 0.0, 0.0), v3(BREECH, 0.0, 0.0), BREECH_R * 0.72, BREECH_R * 0.86, sides);
    if fine {
        ring(b, BREECH + 1.4, BREECH + 2.4, BREECH_R + 0.2);
        b.paint(METAL);
        b.cylinder_between(v3(BREECH - 1.1, 0.0, 0.0), v3(BREECH - 0.6, 0.0, 0.0), 1.2, 1.4, 8);
        // Heat-sink fins down each flank of the block, inside the frame's rails.
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            for k in 0..4 {
                let x = BREECH + 3.4 + 1.3 * k as f32;
                b.block(v3(x, BREECH_R * 0.8, -2.0), v3(x + 0.5, BREECH_R + 0.5, 2.0));
            }
        });
    }

    // The heavy jacket, from inside the cradle to the step collar, dark bands on it.
    b.paint(PLATING);
    b.cylinder_between(v3(JACKET_FROM - 6.0, 0.0, 0.0), v3(STEP, 0.0, 0.0), JACKET_R, JACKET_R * 0.97, sides);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    if fine {
        ring(b, 34.0, 35.4, JACKET_R + 0.3);
    }
    // The step collar down to the slim jacket.
    b.cylinder_between(v3(STEP - 1.5, 0.0, 0.0), v3(STEP + 1.5, 0.0, 0.0), JACKET_R + 0.6, SLIM_R + 0.5, sides);
    b.paint(PLATING);
    b.cylinder_between(v3(STEP + 1.5, 0.0, 0.0), v3(JACKET_TO, 0.0, 0.0), SLIM_R, SLIM_R * 0.95, sides);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    if fine {
        ring(b, 70.0, 71.0, SLIM_R + 0.3);
    }
    // The slim jacket's end: a heavy stepped ring where the rails come out.
    ring(b, JACKET_TO - 1.2, JACKET_TO + 0.6, SLIM_R + 0.6);
    // The rails showing down the slim jacket's flanks as bright spines: bare conductor steel.
    b.paint(METAL).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        b.beam(v3(STEP + 1.6, SLIM_R * 0.93, 0.0), v3(JACKET_TO - 1.2, SLIM_R * 0.88, 0.0), v2(0.7, 1.6), v2(0.7, 1.5))
    });

    // The bare rails: two light bars, bevelled on their outer edges, the slot open.
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let (inner, outer, h, bevel) = (RAIL_Y - RAIL_HW, RAIL_Y + RAIL_HW, RAIL_HH, 0.35);
        let profile = [
            [inner, -h],
            [outer - bevel, -h],
            [outer, -h + bevel],
            [outer, h - bevel],
            [outer - bevel, h],
            [inner, h],
        ];
        b.extrude_x(&profile, JACKET_TO, RAILS_TO + 0.5);
    });
    // The clamp ladder: round dark collars, a touch smaller toward the muzzle.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let clamps = 4;
    for i in 0..clamps {
        if !fine && i % 3 != 0 {
            continue;
        }
        let f = i as f32 / (clamps - 1) as f32;
        let x = JACKET_TO + 9.0 + (LAST_COLLAR - JACKET_TO - 9.0) * f;
        let r = 3.1 - 0.15 * f;
        ring(b, x - 0.5, x + 0.5, r);
    }

    // The muzzle: a flared ring over the rails' ends and the square bore dark in it.
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(RAILS_TO, 0.0, 0.0), v3(MUZZLE - 0.9, 0.0, 0.0), 3.0, MUZZLE_R, sides);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(v3(MUZZLE - 0.9, 0.0, 0.0), v3(MUZZLE, 0.0, 0.0), MUZZLE_R, MUZZLE_R * 0.96, sides);
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(v3(MUZZLE - 0.02, -(RAIL_Y - RAIL_HW) - 0.2, -RAIL_HH), v3(MUZZLE + 0.03, RAIL_Y - RAIL_HW + 0.2, RAIL_HH));
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

    use super::{TRUNNION, ZENITH_RAIL};
    use crate::models::{build_model_scaled, material, part, rig, MeshLod, Model};

    /// The unit file's size (`aster_t4_anti_ship`): radius, height, tech; 8x8 lot.
    const SIZE: (f32, f32, u8) = (46.0, 86.0, 4);
    const HALF_LOT: f32 = 48.0;
    /// A tech 4 hero structure: the factory budget (`models::tests::FACTORY_TRIANGLES`).
    const TRIANGLES: usize = 6000;

    fn built() -> Model {
        build_model_scaled("anti_ship_rail", SIZE.0, SIZE.1, SIZE.2).unwrap()
    }

    fn tris(mesh: &MeshLod) -> usize {
        mesh.indices.len() / 3
    }

    fn weapon() -> (Vec3, Vec3) {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&data).unwrap();
        let bp = blueprints.unit(blueprints.id_of("aster_t4_anti_ship").unwrap());
        let w = &bp.weapons[0];
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        (v(w.pivot.expect("the Zenith's gun elevates")), v(w.muzzle))
    }

    #[test]
    fn zenith_meets_its_budgets() {
        let model = built();
        let [full, mid, coarse] = [0, 1, 2].map(|l| tris(&model.lods[l]));
        println!("anti_ship_rail: {full}/{mid}/{coarse}");
        assert!(full <= TRIANGLES, "full LOD {full}");
        assert!(coarse < 60, "coarse LOD {coarse}");
        assert!(mid as f32 <= full as f32 * 0.45 + 20.0, "reduced {mid} of {full}");
    }

    /// The unit file's pivot and muzzle are the model's trunnion and barrel tip, the
    /// anchors agree with both, and the barrel reaches the muzzle at every level.
    #[test]
    fn zenith_barrel_ends_at_the_muzzle_and_pitches_about_the_trunnion() {
        let (pivot, muzzle) = weapon();
        assert!(pivot.distance(TRUNNION) < 1e-2, "data pivot {pivot} vs trunnion {TRUNNION}");
        let tip = TRUNNION + Vec3::from(ZENITH_RAIL.muzzle);
        assert!(muzzle.distance(tip) < 1e-2, "data muzzle {muzzle} vs barrel tip {tip}");
        let model = built();
        assert_eq!(model.arm_pivot, Some(TRUNNION.to_array()));
        assert!(model.recoil.is_some());
        for (l, lod) in model.lods.iter().enumerate() {
            let barrel: Vec<Vec3> = lod
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::ARM_GUN && v.rig & rig::RECOIL != 0)
                .map(|v| Vec3::from(v.pos))
                .collect();
            let front = barrel.iter().map(|p| p.x).fold(f32::MIN, f32::max);
            let back = barrel.iter().map(|p| p.x).fold(f32::MAX, f32::min);
            assert!((front - tip.x).abs() < 0.2, "lod{l}: barrel ends at {front}, muzzle {}", tip.x);
            assert!((back - (TRUNNION.x + ZENITH_RAIL.breech[0])).abs() < 1.3, "lod{l}: breech at {back}");
            // Nothing else on the turret reaches past the muzzle.
            let reach = lod.vertices.iter().filter(|v| v.part == part::TURRET).map(|v| v.pos[0]).fold(f32::MIN, f32::max);
            assert!(reach < tip.x + 0.5, "lod{l}: {reach}");
            // Long and slim: the barrel is round and well over ten times longer than wide.
            let wide = barrel.iter().filter(|p| p.x > TRUNNION.x + 12.0).map(|p| p.y.abs().max((p.z - TRUNNION.z).abs())).fold(0.0, f32::max);
            assert!((front - back) > 12.0 * 2.0 * wide, "lod{l}: {} m long, {wide} m half-wide", front - back);
            // Every anchor along the bore lies inside the barrel's extent.
            for p in ZENITH_RAIL.along {
                assert!(p[0] > ZENITH_RAIL.breech[0] && p[0] < ZENITH_RAIL.muzzle[0]);
            }
        }
    }

    /// Stands in its 8x8 lot (only the barrel overhangs), to its height, turns on a
    /// turret and elevates; wears team colour and nothing on it glows.
    #[test]
    fn zenith_fits_its_lot_and_is_unlit_hardware() {
        let model = built();
        for (l, lod) in model.lods.iter().enumerate() {
            let fixed = lod.vertices.iter().filter(|v| v.part != part::TURRET);
            let (x, y) = fixed.fold((0.0f32, 0.0f32), |(x, y), v| (x.max(v.pos[0].abs()), y.max(v.pos[1].abs())));
            assert!(x <= HALF_LOT && y <= HALF_LOT, "lod{l}: base {x} x {y}");
            let top = lod.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
            assert!(top >= SIZE.1 * 0.8 && top <= SIZE.1 * 1.25, "lod{l}: top {top}");
            assert!(lod.vertices.iter().any(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::ARM_GUN), "lod{l}: gun arm");
            assert!(lod.vertices.iter().any(|v| v.material == material::TEAM), "lod{l}: team colour");
            let lit = lod.vertices.iter().filter(|v| matches!(v.material, material::GLOW | material::GLOW_ORANGE)).count();
            assert_eq!(lit, 0, "lod{l}: lit");
            assert!(lod.vertices.iter().all(|v| v.pos[2] >= -1e-3), "lod{l}: below ground");
        }
        assert_eq!(model.turret_pivot[..2], [0.0, 0.0], "the turret turns about the lot's centre");
    }

    /// Sound meshes: no degenerate triangles, winding agreeing with the normals, one
    /// material and part per triangle.
    #[test]
    fn zenith_is_a_sound_mesh() {
        for (l, mesh) in built().lods.iter().enumerate() {
            for t in mesh.indices.chunks(3) {
                let v = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize]);
                let p = v.map(|v| Vec3::from(v.pos));
                let n = (p[1] - p[0]).cross(p[2] - p[0]);
                assert!(n.length() * 0.5 > 1e-7, "lod{l}: degenerate at {}", p[0]);
                for v in &v {
                    assert!(n.normalize().dot(Vec3::from(v.normal)) > 0.5, "lod{l}: winding at {}", p[0]);
                }
                assert!(v[0].material == v[1].material && v[1].material == v[2].material);
                assert!(v[0].part == v[1].part && v[1].part == v[2].part);
            }
        }
    }

    /// Previews at rest and elevated: `MODEL_DUMP_DIR=... cargo test -p mc-render --lib
    /// -- --ignored zenith_previews`.
    #[test]
    #[ignore = "writes preview images"]
    fn zenith_previews() {
        let dir = std::path::PathBuf::from(std::env::var_os("MODEL_DUMP_DIR").expect("MODEL_DUMP_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        let model = built();
        for (l, lod) in model.lods.iter().enumerate() {
            for pitch in [0.0f32, 0.5] {
                let mut posed = lod.clone();
                for v in &mut posed.vertices {
                    if v.rig & rig::LIMB_MASK == rig::ARM_GUN {
                        let r = Vec3::from(v.pos) - TRUNNION;
                        let (s, c) = pitch.sin_cos();
                        v.pos = (TRUNNION + Vec3::new(r.x * c - r.z * s, r.y, r.x * s + r.z * c)).to_array();
                    }
                }
                let res = if l == 0 { 900 } else { 300 };
                for az in [-38.0f32, 52.0, 142.0] {
                    if l > 0 && az != -38.0 {
                        continue;
                    }
                    crate::models::preview::render(&posed, res, az)
                        .write_ppm(&dir.join(format!("zenith_l{l}_p{}_{}.ppm", (pitch * 100.0) as i32, az as i32)))
                        .unwrap();
                }
            }
        }
    }
}

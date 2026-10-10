//! Nimbus (`aster_t4_storm_bomber`, mesh "storm_bomber"): the tech 4 storm bomber, an
//! unmanned "arsenal bird" about 220 m across a slender swept wing and 95 m long,
//! authored in metres at its unit's size (radius 110), +x forward, +y left, z up from
//! its belly.
//!
//! A dense layered body (a chined hull drawn out into a long pointed nose, a graphite deck
//! on its back, engine sponsons along its flanks, root extensions and armour cheeks
//! laid over its shoulders) carries a broad swept wing many times its length, hard
//! drooped outboard, a canted winglet at each tip, fences on it and pods under its
//! leading edge; over the tail, two fins canted out on booms off the sponsons, a long
//! swept wing bridging their tips and running on past them ([`tail`]).
//!
//! It sows storms: the Storm Seeder lets a seed of charged cloud go from a dispenser in
//! its tail ([`SEED`], the weapon's muzzle and pivot), and the seed grows into a
//! thunderhead over the target. Four small twin AA mounts guard it, each turning on its
//! own (weapons 1 to 4: dorsal fore and aft on the deck, one out on each wing, [`TURRETS`]),
//! their two barrels firing together ([`aa_mount`]).
//!
//! Style: hard facets, scratched light steel over graphite, panels cut on the facet
//! lines, the team colour in surface marks; no glazing, nothing on it is lit.

mod body;
#[cfg(test)]
pub(crate) use body::LASERS;
mod dispenser;
mod kit;
mod tail;
mod wing;

use dispenser::Dispenser;
use kit::Kit;
use tail::Tail;
use wing::Wing;

use super::*;

/// Full-detail triangle budget (`tests.rs`): one or two a match, the biggest aircraft but
/// the Bastion.
#[cfg(test)]
pub(crate) const TRIANGLES: usize = 12000;

/// The four AA mounts' pivots, weapons 1 to 4: dorsal fore and aft on the deck, port and
/// starboard out on the wing.
pub(crate) const TURRETS: [Vec3; 4] = [
    Vec3::new(9.0, 0.0, 13.8),
    Vec3::new(-20.0, 0.0, 13.8),
    Vec3::new(-9.0, 48.0, 8.5),
    Vec3::new(-9.0, -48.0, 8.5),
];

/// The middle of the seed dispenser's mouth: weapon 0's muzzle and pivot.
pub(crate) const SEED: Vec3 = Vec3::new(-46.5, 0.0, 7.2);

const DISPENSER: Dispenser = Dispenser {
    mouth: SEED,
    w: 3.6,
    h: 2.4,
    root_x: -35.0,
    root: 0.9,
    depth: 3.0,
};

/// The wing: broad, swept, hard drooped outboard.
const WING: Wing = Wing {
    stations: &[
        (6.0, 7.5, 14.0, -12.0, 2.6),
        (24.0, 7.5, 6.8, -11.0, 1.9),
        (54.0, 7.0, -5.2, -17.0, 1.2),
        (78.0, 4.0, -14.8, -23.5, 0.8),
        (108.0, 0.6, -26.8, -31.0, 0.4),
    ],
};

const KIT: Kit = Kit {
    fences: &[36.0, 64.0, 90.0],
    pods: &[26.0, 52.0, 76.0],
    winglet: (7.0, 1.2),
};

const TAIL: Tail = Tail {
    cant: 0.3,
    front: -25.0,
    top: 18.0,
    lead: -31.0,
    sweep: 0.45,
    chord: 7.0,
    overhang: 7.0,
};

pub(crate) fn build(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        body::coarse(b);
        b.mirror_y(|b| WING.coarse(b, &[0, 2, 4], PLATING));
        TAIL.coarse(b);
        return;
    }
    body::build(b);
    b.mirror_y(|b| {
        WING.build(b, PLATING, PLATING_DARK);
        KIT.build(b, &WING);
    });
    TAIL.build(b);
    DISPENSER.build(b);
    for (w, pivot) in TURRETS.into_iter().enumerate() {
        let skin = if pivot.y == 0.0 {
            body::DECK_TOP
        } else {
            WING.top(pivot.x, pivot.y).z
        };
        aa_mount(b, w + 1, pivot, skin);
    }
    let tip = WING.stations[WING.stations.len() - 2].0 + 4.0;
    for side in [1.0, -1.0] {
        let (lead, trail) = WING.edges(tip);
        team_panel(b, WING.top((lead + trail) * 0.5, tip * side), v2(5.0, 6.0));
    }
}

/// How far ahead of its pivot a mount's barrels end, the mount facing the nose, and how
/// far either side of the bore line each barrel lies: the unit file's muzzles are the
/// pivot plus `(REACH, ±GAP, 0)`, its `muzzle` the point between them.
pub(crate) const REACH: f32 = 3.6;
pub(crate) const GAP: f32 = 0.4;
/// How high a mount's bores stand over the skin it sits in.
pub(crate) const BORE: f32 = 0.8;

/// One twin AA mount, bound to `weapon` and turning about `pivot` (the bores' height,
/// [`BORE`] over the skin at `skin`): a flush faceted collar let into the skin, a low
/// teardrop cupola on it, and two slim barrels side by side out of its nose, pitching with
/// the cupola's trunnion, their ends [`REACH`] ahead of the pivot. The coarse level draws
/// none of it.
pub(crate) fn aa_mount(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, skin: f32) {
    if b.coarse() {
        return;
    }
    let deck = pivot.z - BORE + 0.2;
    if b.fine() {
        // The flush collar the cupola turns in.
        b.paint(ACCENT);
        b.prism(v3(pivot.x, pivot.y, skin - 0.6), 12, 1.9, 1.8, 0.8);
    }
    b.with_house(weapon, pivot, 0.0, |b| {
        // The cupola: a low faceted teardrop, pointed aft, its nose cut square for the guns.
        let ring = |x: f32, w: f32, h: f32| {
            vec![
                v3(pivot.x + x, pivot.y - w, deck - 0.1),
                v3(pivot.x + x, pivot.y + w, deck - 0.1),
                v3(pivot.x + x, pivot.y + w * 0.75, deck + h * 0.65),
                v3(pivot.x + x, pivot.y + w * 0.3, deck + h),
                v3(pivot.x + x, pivot.y - w * 0.3, deck + h),
                v3(pivot.x + x, pivot.y - w * 0.75, deck + h * 0.65),
            ]
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.with_facets(|b| {
            if b.fine() {
                b.loft(
                    &[
                        ring(-2.9, 0.15, 0.15),
                        ring(-1.2, 1.3, 0.95),
                        ring(0.6, 1.4, 1.0),
                        ring(1.3, 1.0, 0.75),
                    ],
                    true,
                    true,
                );
            } else {
                b.loft(
                    &[
                        ring(-2.9, 0.15, 0.15),
                        ring(0.6, 1.4, 1.0),
                        ring(1.3, 1.0, 0.75),
                    ],
                    true,
                    true,
                );
            }
        });
        b.with_recoil(|b| twin_guns(b, pivot));
    });
}

/// The two slim barrels and the mantlet that links them, on the bores through
/// `pivot` ± `GAP`.
fn twin_guns(b: &mut MeshBuilder, pivot: Vec3) {
    b.paint(PLATING_DARK);
    let mantlet = v3(0.5, 2.0 * GAP + 0.5, 0.55);
    if b.fine() {
        b.chamfered_box(pivot + v3(1.3, 0.0, 0.0), mantlet, 0.12);
    } else {
        b.cuboid(pivot + v3(1.3, 0.0, 0.0), mantlet);
    }
    let n = if b.fine() { 8 } else { 4 };
    for side in [1.0, -1.0] {
        let bore = pivot + Vec3::Y * GAP * side;
        b.paint(METAL);
        b.cylinder_between(bore + Vec3::X * 1.4, bore + Vec3::X * REACH, 0.11, 0.09, n);
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(
                bore + Vec3::X * (REACH - 0.3),
                bore + Vec3::X * REACH,
                0.13,
                0.13,
                6,
            );
        }
    }
}

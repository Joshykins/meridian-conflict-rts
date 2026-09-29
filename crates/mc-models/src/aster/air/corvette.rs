//! Valiant: the ARC's tech 2 rail corvette (`aster_t2_corvette`, mesh `rail_corvette`), a
//! small warship of the upper air carried round one long rail cannon turret on its back,
//! the Resolute's turret house with its rails drawn out, and a rotary AA cannon slung
//! under the bow. +X is forward, +Y port, the ground at z 0 with the legs down.
//!
//! The `~` keys are hull directions for picking; every hull carries the guns at the same
//! places and its belly at `KEEL`, so the unit file's pivots and muzzles hold for all.
//!
//! Contracts:
//! - `RAIL` is weapon 0's pivot and `RAIL_REACH` how far ahead of it the rails end (the
//!   unit file's `muzzle` - `pivot`); `RAIL_CHARGE` is where its charge crawls.
//! - `GATLING` is weapon 1's pivot, a slung `capital::rotary_house` of `GATLING_SIZE`,
//!   its muzzles `12 * GATLING_SIZE` ahead of the pivot as authored.
//! - `fit(mesh)`: each hull's drives, lift jets, lamps and rig (`capital.rs`), legs
//!   included: two pairs that stow into belly bays.
use super::capital::{self, CapitalRig, Leg};
use super::resolute::{self, HOUSE_SINK};
use super::*;
use crate::{CapitalLamps, TurretRail};

/// The long rail cannon's pivot, and how far ahead of it the rails end.
pub(super) const RAIL: [f32; 3] = [22.0, 0.0, 35.0];
pub(super) const RAIL_REACH: f32 = 40.0;
/// The rail house is the Resolute's at this size (its rails drawn out to `RAIL_REACH`).
const RAIL_SCALE: f32 = 1.0;
pub(crate) const RAIL_CHARGE: TurretRail = resolute::turret_rail(RAIL_SCALE, RAIL_REACH);
/// The rotary AA cannon's pivot and size (`capital::rotary_house`, slung).
pub(super) const GATLING: [f32; 3] = [48.0, 0.0, 7.5];
pub(super) const GATLING_SIZE: f32 = 0.9;
/// Where the rail house's ring ends below its pivot: every hull raises a pedestal from
/// its deck to here.
const RAIL_RING: f32 = RAIL[2] - (HOUSE_SINK + 1.0) * RAIL_SCALE;
/// The flat belly every hull sits on its legs at, and the deck under the rail house.
const KEEL: f32 = 12.0;
const DECK: f32 = 29.0;

/// Leg size against the Bastion's (hinge 36 m over the foot), and the legs themselves:
/// the fore pair stows aft, the aft pair forward, each into a bay under the belly.
const LEG: f32 = 0.45;
const fn legs(y: f32) -> [Leg; 2] {
    let (hz, bay) = (36.0 * LEG, 2.6);
    [
        Leg {
            hinge: [28.0, y, hz],
            stow: 1.0,
            bay: [13.2, 30.0, y - bay, y + bay],
            size: LEG,
        },
        Leg {
            hinge: [-26.0, y, hz],
            stow: -1.0,
            bay: [-27.8, -11.2, y - bay, y + bay],
            size: LEG,
        },
    ]
}

/// A hull's anchors: where its drives, lift jets and lamps are, and the rig built on them.
pub(crate) struct Fit {
    pub(crate) nozzles: &'static [[f32; 3]],
    /// `[aft -y, aft +y, fore -y, fore +y]`.
    pub(crate) lift_jets: [[f32; 3]; 4],
    pub(crate) lamps: CapitalLamps,
    pub(crate) rig: CapitalRig,
    /// The drives' size against the Bastion's 12 m bells.
    drive: f32,
}

const fn rig(nozzles: &[[f32; 3]], drive: f32, leg_y: f32) -> CapitalRig {
    let [x, y, z] = nozzles[0];
    CapitalRig {
        legs: Some(legs(leg_y)),
        door_hinge: KEEL - 0.3,
        drives: Some(([x, z, y.abs(), y.abs()], drive)),
        lift_jets: Some(([JETS[3][0], JETS[3][1], JETS[1][0], JETS[1][1]], JETS[3][2])),
        ramp: None,
    }
}

/// Lift jets under the belly, clear of the leg bays: a pair amidships, a pair aft.
const JETS: [[f32; 3]; 4] = [
    [-40.0, -4.0, KEEL - 0.4],
    [-40.0, 4.0, KEEL - 0.4],
    [2.0, -4.0, KEEL - 0.4],
    [2.0, 4.0, KEEL - 0.4],
];

const KEEL_NOZZLES: [[f32; 3]; 2] = [[-58.0, 13.0, 20.0], [-58.0, -13.0, 20.0]];
const KEEL_FIT: Fit = Fit {
    nozzles: &KEEL_NOZZLES,
    lift_jets: JETS,
    lamps: CapitalLamps {
        floods: &[
            [38.0, 5.0, 12.4],
            [38.0, -5.0, 12.4],
            [-48.0, 5.0, 13.4],
            [-48.0, -5.0, 13.4],
        ],
        nav_port: [-40.0, 19.2, 20.0],
        nav_starboard: [-40.0, -19.2, 20.0],
        strobes: &[[61.0, 0.0, 21.0], [-18.0, 0.0, 41.6]],
        beacons: &[[-48.0, 0.0, 27.8]],
        hold: None,
    },
    rig: rig(&KEEL_NOZZLES, 0.45, 6.0),
    drive: 0.45,
};

const OUTRIGGER_NOZZLES: [[f32; 3]; 2] = [[-56.0, 25.0, 19.0], [-56.0, -25.0, 19.0]];
const OUTRIGGER_FIT: Fit = Fit {
    nozzles: &OUTRIGGER_NOZZLES,
    lift_jets: JETS,
    lamps: CapitalLamps {
        floods: &[
            [38.0, 5.0, 12.4],
            [38.0, -5.0, 12.4],
            [-48.0, 5.0, 13.4],
            [-48.0, -5.0, 13.4],
        ],
        nav_port: [4.0, 31.4, 19.0],
        nav_starboard: [4.0, -31.4, 19.0],
        strobes: &[[61.0, 0.0, 21.0], [-18.0, 0.0, 41.6]],
        beacons: &[[-48.0, 0.0, 27.8]],
        hold: None,
    },
    rig: rig(&OUTRIGGER_NOZZLES, 0.45, 6.0),
    drive: 0.45,
};

const BLADE_NOZZLES: [[f32; 3]; 2] = [[-54.0, 22.0, 17.0], [-54.0, -22.0, 17.0]];
const BLADE_FIT: Fit = Fit {
    nozzles: &BLADE_NOZZLES,
    lift_jets: JETS,
    lamps: CapitalLamps {
        floods: &[
            [40.0, 3.0, 12.4],
            [40.0, -3.0, 12.4],
            [-10.0, 22.0, 11.6],
            [-10.0, -22.0, 11.6],
        ],
        nav_port: [8.0, 29.0, 20.0],
        nav_starboard: [8.0, -29.0, 20.0],
        strobes: &[[64.0, 0.0, 23.4], [-18.0, 0.0, 41.6]],
        beacons: &[[-44.0, 0.0, 28.4]],
        hold: None,
    },
    rig: rig(&BLADE_NOZZLES, 0.45, 4.2),
    drive: 0.45,
};

/// The hull directions, by mesh key.
#[derive(Clone, Copy)]
pub(super) enum Hull {
    Keel,
    Outrigger,
    Blade,
}

pub(crate) fn fit(mesh: &str) -> Option<&'static Fit> {
    Some(match mesh {
        "rail_corvette" => &KEEL_FIT,
        "rail_corvette~outrigger" => &OUTRIGGER_FIT,
        "rail_corvette~blade" => &BLADE_FIT,
        _ => return None,
    })
}

/// A chamfered box section across x: half width `w`, from `lo` to `hi`, corners cut `c`.
fn section(x: f32, w: f32, lo: f32, hi: f32, c: f32) -> Vec<Vec3> {
    [
        (-w + c, lo),
        (w - c, lo),
        (w, lo + c),
        (w, hi - c),
        (w - c, hi),
        (-w + c, hi),
        (-w, hi - c),
        (-w, lo + c),
    ]
    .iter()
    .map(|&(y, z)| v3(x, y, z))
    .collect()
}

/// A hull lofted through `stations` (x, half width, bottom, top, chamfer), capped.
fn hull(b: &mut MeshBuilder, stations: &[[f32; 5]]) {
    b.loft(
        &stations
            .iter()
            .map(|s| section(s[0], s[1], s[2], s[3], s[4]))
            .collect::<Vec<_>>(),
        true,
        true,
    );
}

pub(super) fn build(b: &mut MeshBuilder, kind: Hull) {
    let fit = match kind {
        Hull::Keel => &KEEL_FIT,
        Hull::Outrigger => &OUTRIGGER_FIT,
        Hull::Blade => &BLADE_FIT,
    };
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid(v3(2.0, 0.0, 20.5), v3(112.0, 22.0, 17.0));
        b.paint(PLATING_DARK);
        let beam = 2.0 * fit.nozzles[0][1].abs() + 10.0;
        b.cuboid(v3(-48.0, 0.0, 19.0), v3(20.0, beam, 10.0));
        b.paint(ACCENT);
        b.cuboid(v3(40.0, 0.0, 35.0), v3(40.0, 3.0, 3.0));
        return;
    }
    let bow_belly = match kind {
        Hull::Keel | Hull::Outrigger => armoured_hull(b),
        Hull::Blade => blade_hull(b),
    };
    match kind {
        Hull::Keel => {
            stubs(b);
            for &n in fit.nozzles {
                pod(b, n, -14.0, 5.0, fit.drive);
            }
        }
        Hull::Outrigger => {
            for &n in fit.nozzles {
                pylon(b, n, 11.0);
                pod(b, n, 2.0, 5.2, fit.drive);
            }
        }
        Hull::Blade => {
            for &n in fit.nozzles {
                pylon(b, n, 8.0);
                pod(b, n, 6.0, 5.2, fit.drive);
            }
        }
    }
    guns(b, bow_belly);
    capital::gear(b, &fit.rig, KEEL - 0.7);
    for jet in &fit.lift_jets {
        capital::lift_jet(b, Vec3::from(*jet), 0.5);
    }
    lamp_fittings(b, &fit.lamps);
}

/// The guns and what carries them: a pedestal from the deck up under the rail house, and
/// a post from the bow's belly (`bow_belly`, its height over the gun) down to the slung
/// rotary house.
fn guns(b: &mut MeshBuilder, bow_belly: f32) {
    let [rx, _, _] = RAIL;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(
        v3(rx, 0.0, DECK - 1.0),
        b.sides(12),
        9.6,
        8.6,
        RAIL_RING - DECK + 0.2,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(rx, 0.0, RAIL_RING - 0.8), b.sides(12), 8.9, 8.9, 0.8);
    resolute::rail_turret_sized(b, 0, Vec3::from(RAIL), RAIL_SCALE, RAIL_REACH);
    let [gx, _, gz] = GATLING;
    let post = gz + 4.9 * GATLING_SIZE;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(
        v3(gx, 0.0, post - 0.2),
        b.sides(10),
        2.6,
        3.8,
        bow_belly - post + 1.5,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(gx, 0.0, bow_belly - 0.6), b.sides(10), 5.2, 5.6, 0.8);
    capital::rotary_house(b, 1, Vec3::from(GATLING), true, GATLING_SIZE);
}

/// A drive pod: an octagonal nacelle from `fore` back onto a drive of `size` at `nozzle`.
fn pod(b: &mut MeshBuilder, nozzle: [f32; 3], fore: f32, r: f32, size: f32) {
    let [nx, y, z] = nozzle;
    let aft = nx + 27.6 * size;
    b.at(v3(0.0, y, 0.0), |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        hull(
            b,
            &[
                [fore, r * 0.5, z - r * 0.5, z + r * 0.5, r * 0.2],
                [fore - r * 1.6, r, z - r, z + r, r * 0.35],
                [aft + 1.0, r * 1.02, z - r * 1.02, z + r * 1.02, r * 0.35],
            ],
        );
        b.paint(ACCENT);
        hull(
            b,
            &[
                [aft + 1.2, r * 1.06, z - r * 1.06, z + r * 1.06, r * 0.37],
                [aft - 0.8, r * 0.96, z - r * 0.96, z + r * 0.96, r * 0.33],
            ],
        );
        if b.fine() {
            b.paint(TEAM);
            hull(
                b,
                &[
                    [aft + 6.0, r * 1.05, z - r * 1.05, z + r * 1.05, r * 0.37],
                    [aft + 4.4, r * 1.05, z - r * 1.05, z + r * 1.05, r * 0.37],
                ],
            );
        }
    });
    capital::drive(b, Vec3::from(nozzle), size);
}

/// Short stubs from the keel hull's quarters out to its drive pods.
fn stubs(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.mirror_y(|b| {
        b.frustum(
            v3(-40.0, 10.0, 17.5),
            v2(18.0, 1.0),
            v2(12.0, 1.0),
            4.0,
            v2(-3.0, 0.0),
        );
    });
}

/// A swept pylon from the hull's side (`root` half width) out to the nacelle at `nozzle`.
fn pylon(b: &mut MeshBuilder, nozzle: [f32; 3], root: f32) {
    let [nx, y, z] = nozzle;
    let s = y.signum();
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            vec![
                v3(nx + 58.0, s * root, z + 1.0),
                v3(nx + 58.0, s * root, z + 4.0),
                v3(nx + 30.0, s * root, z + 4.0),
                v3(nx + 30.0, s * root, z + 1.0),
            ],
            vec![
                v3(nx + 44.0, y, z - 1.0),
                v3(nx + 44.0, y, z + 1.0),
                v3(nx + 24.0, y, z + 1.0),
                v3(nx + 24.0, y, z - 1.0),
            ],
        ],
        true,
        true,
    );
    if b.fine() {
        b.paint(TEAM);
        b.beam(
            v3(nx + 55.0, s * (root + 1.0), z + 4.1),
            v3(nx + 43.0, y - s * 3.0, z + 1.6),
            v2(1.4, 0.2),
            v2(1.4, 0.2),
        );
    }
}

/// A bridge block: a stepped house from `deck` up to `top` aft of the rail house, a band
/// of glass across its face.
fn bridge(b: &mut MeshBuilder, x: f32, deck: f32, top: f32, w: f32) {
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    hull(
        b,
        &[
            [x + 6.0, w * 0.6, deck, top - 2.5, 1.0],
            [x + 3.5, w, deck, top, 1.4],
            [x - 10.0, w, deck, top, 1.4],
            [x - 12.0, w * 0.8, deck, top - 1.5, 1.2],
        ],
    );
    b.paint(GLASS);
    b.beam(
        v3(x + 4.6, w * 0.7, top - 2.3),
        v3(x + 4.6, -w * 0.7, top - 2.3),
        v2(0.6, 1.3),
        v2(0.6, 1.3),
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid(v3(x - 4.0, 0.0, top + 0.6), v3(6.0, w * 1.2, 1.2));
    if b.fine() {
        // A mast on the roof with the strobe on its tip.
        b.paint(METAL);
        b.cylinder_between(
            v3(x - 4.0, 0.0, top + 1.2),
            v3(x - 4.0, 0.0, top + 4.6),
            0.5,
            0.3,
            6,
        );
        team_panel(b, v3(x - 9.0, 0.0, top), v2(3.0, w));
    }
}

/// The faceted armoured hull (keel and outrigger): a raked prow, a dark belt low on the
/// sides, the bridge aft of the rail house. Returns the belly's height over the chin gun.
fn armoured_hull(b: &mut MeshBuilder) -> f32 {
    const STATIONS: [[f32; 5]; 7] = [
        [61.0, 2.0, 19.0, 22.0, 0.8],
        [54.0, 6.5, 15.5, 26.0, 2.6],
        [40.0, 11.0, KEEL + 0.5, DECK, 4.0],
        [0.0, 12.0, KEEL, DECK, 4.2],
        [-30.0, 12.0, KEEL, DECK, 4.2],
        [-44.0, 10.0, 13.5, 27.0, 3.6],
        [-50.0, 8.0, 15.0, 25.0, 3.0],
    ];
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    hull(b, &STATIONS);
    b.paint(PLATING_DARK);
    hull(
        b,
        &[
            [52.0, 7.2, 15.0, 20.0, 2.0],
            [40.0, 11.6, 12.2, 20.0, 3.6],
            [-32.0, 12.6, 11.6, 20.0, 3.6],
            [-46.0, 10.4, 13.2, 20.0, 3.0],
        ],
    );
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.beam(
            v3(52.0, 6.8, 25.5),
            v3(42.0, 11.3, 28.4),
            v2(1.2, 0.3),
            v2(1.2, 0.3),
        );
    });
    bridge(b, -14.0, DECK, 37.0, 6.0);
    14.0
}

/// Blade: a long, narrow armoured spine that is mostly gun mount, armoured cheeks along
/// it, a canard each side forward; its drives hang out on swept pylons like outriggers.
fn blade_hull(b: &mut MeshBuilder) -> f32 {
    const SPINE: [[f32; 5]; 7] = [
        [64.0, 1.2, 20.0, 24.0, 0.5],
        [54.0, 4.5, 16.0, 27.0, 1.8],
        [36.0, 8.0, KEEL, DECK, 2.6],
        [-10.0, 8.0, KEEL, DECK, 2.8],
        [-36.0, 8.0, KEEL, DECK, 2.8],
        [-46.0, 6.0, 14.0, 28.0, 2.4],
        [-50.0, 4.0, 17.0, 26.0, 1.8],
    ];
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    hull(b, &SPINE);
    b.mirror_y(|b| {
        b.at(v3(0.0, 8.0, 0.0), |b| {
            b.paint(PLATING_DARK);
            hull(
                b,
                &[
                    [44.0, 0.6, 18.0, 24.0, 0.3],
                    [32.0, 2.0, 17.0, 26.0, 0.8],
                    [-30.0, 2.0, 17.0, 26.0, 0.8],
                    [-40.0, 0.8, 18.5, 25.0, 0.4],
                ],
            );
        });
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                vec![
                    v3(42.0, 9.6, 20.0),
                    v3(42.0, 9.6, 21.6),
                    v3(30.0, 9.6, 21.6),
                    v3(30.0, 9.6, 20.0),
                ],
                vec![
                    v3(28.0, 20.0, 19.0),
                    v3(28.0, 20.0, 19.8),
                    v3(22.0, 20.0, 19.8),
                    v3(22.0, 20.0, 19.0),
                ],
            ],
            true,
            true,
        );
        b.paint(TEAM);
        b.cuboid(v3(25.0, 19.6, 19.4), v3(6.0, 1.0, 1.0));
    });
    bridge(b, -14.0, DECK, 37.0, 5.0);
    14.6
}

/// Fittings for the lamps the renderer lights: flood housings, nav pods, strobes, beacons.
fn lamp_fittings(b: &mut MeshBuilder, lamps: &CapitalLamps) {
    for at in lamps.floods {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.cuboid(c + v3(0.0, 0.0, 0.5), v3(1.8, 1.4, 0.8));
        b.paint(GLOW);
        b.cuboid(c + v3(0.0, 0.0, 0.1), v3(1.1, 0.9, 0.1));
    }
    for (at, glow) in [(lamps.nav_port, GLOW_RED), (lamps.nav_starboard, GLOW)] {
        b.paint(glow);
        b.cuboid(Vec3::from(at), v3(1.0, 0.7, 0.7));
    }
    for at in lamps.strobes {
        b.paint(GLOW);
        b.cuboid(Vec3::from(*at), v3(0.6, 0.6, 0.6));
    }
    for at in lamps.beacons {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.prism(c - v3(0.0, 0.0, 0.9), b.sides(8), 0.8, 0.7, 0.7);
        b.paint(GLOW_AMBER);
        b.prism(c - v3(0.0, 0.0, 0.2), b.sides(8), 0.5, 0.45, 0.5);
    }
}

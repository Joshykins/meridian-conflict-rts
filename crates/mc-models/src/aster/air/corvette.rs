//! Valiant: the ARC's tech 2 rail corvette (`aster_t2_corvette`, mesh `rail_corvette`), a
//! small warship of the upper air carried round one long rail cannon turret on its back,
//! the Resolute's turret house with its rails drawn out, and a rotary AA cannon astern.
//! +X is forward, +Y port, the ground at z 0 when landed (it sits on skids).
//!
//! The `~` keys are hull directions for picking; every hull carries the guns at the same
//! places, so the unit file's pivots and muzzles hold for all of them.
//!
//! Contracts:
//! - `RAIL` is weapon 0's pivot and `RAIL_REACH` how far ahead of it the rails end (the
//!   unit file's `muzzle` - `pivot`); `RAIL_CHARGE` is where its charge crawls.
//! - `GATLING` is weapon 1's pivot, a `capital::rotary_house` of `GATLING_SIZE` resting
//!   astern, its muzzles `12 * GATLING_SIZE` ahead of the pivot as authored.
//! - `fit(mesh)`: each hull's drives, lift jets, lamps and rig (`capital.rs`).
use super::capital::{self, CapitalRig};
use super::resolute::{self, HOUSE_SINK};
use super::*;
use crate::{CapitalLamps, TurretRail};

/// The long rail cannon's pivot, and how far ahead of it the rails end.
pub(super) const RAIL: [f32; 3] = [22.0, 0.0, 30.0];
pub(super) const RAIL_REACH: f32 = 40.0;
/// The rail house is the Resolute's at this size (its rails drawn out to `RAIL_REACH`).
const RAIL_SCALE: f32 = 1.0;
pub(crate) const RAIL_CHARGE: TurretRail = resolute::turret_rail(RAIL_SCALE, RAIL_REACH);
/// The rotary AA cannon's pivot and size (`capital::rotary_house`).
pub(super) const GATLING: [f32; 3] = [-30.0, 0.0, 30.0];
pub(super) const GATLING_SIZE: f32 = 0.7;
/// Where the rail house's ring and the rotary gun's post end below their pivots: every
/// hull raises a pedestal from its deck to here.
const RAIL_RING: f32 = RAIL[2] - (HOUSE_SINK + 1.0) * RAIL_SCALE;
const GATLING_POST: f32 = GATLING[2] - 6.6 * GATLING_SIZE;

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

const fn rig(nozzles: &[[f32; 3]], drive: f32, jets: &[[f32; 3]; 4]) -> CapitalRig {
    let [x, y, z] = nozzles[0];
    CapitalRig {
        legs: None,
        door_hinge: 0.0,
        drives: Some(([x, z, y.abs(), y.abs()], drive)),
        lift_jets: Some(([jets[3][0], jets[3][1], jets[1][0], jets[1][1]], jets[3][2])),
        ramp: None,
    }
}

const KEEL_NOZZLES: [[f32; 3]; 2] = [[-58.0, 13.0, 15.0], [-58.0, -13.0, 15.0]];
const KEEL_JETS: [[f32; 3]; 4] = [
    [-26.0, -6.0, 7.0],
    [-26.0, 6.0, 7.0],
    [30.0, -5.0, 7.5],
    [30.0, 5.0, 7.5],
];
const KEEL: Fit = Fit {
    nozzles: &KEEL_NOZZLES,
    lift_jets: KEEL_JETS,
    lamps: CapitalLamps {
        floods: &[
            [36.0, 4.0, 7.2],
            [36.0, -4.0, 7.2],
            [-14.0, 6.0, 6.8],
            [-14.0, -6.0, 6.8],
        ],
        nav_port: [-40.0, 19.2, 15.0],
        nav_starboard: [-40.0, -19.2, 15.0],
        strobes: &[[61.0, 0.0, 16.0], [-4.0, 0.0, 36.6]],
        beacons: &[[-44.0, 0.0, 22.8]],
        hold: None,
    },
    rig: rig(&KEEL_NOZZLES, 0.45, &KEEL_JETS),
    drive: 0.45,
};

const DART_NOZZLES: [[f32; 3]; 1] = [[-58.0, 0.0, 16.0]];
const DART_JETS: [[f32; 3]; 4] = [
    [-22.0, -14.0, 9.0],
    [-22.0, 14.0, 9.0],
    [26.0, -5.0, 8.6],
    [26.0, 5.0, 8.6],
];
const DART: Fit = Fit {
    nozzles: &DART_NOZZLES,
    lift_jets: DART_JETS,
    lamps: CapitalLamps {
        floods: &[
            [34.0, 3.0, 8.4],
            [34.0, -3.0, 8.4],
            [-14.0, 10.0, 8.4],
            [-14.0, -10.0, 8.4],
        ],
        nav_port: [-40.0, 31.0, 16.0],
        nav_starboard: [-40.0, -31.0, 16.0],
        strobes: &[[62.5, 0.0, 15.6], [-50.0, 0.0, 33.4]],
        beacons: &[[-4.0, 0.0, 29.6]],
        hold: None,
    },
    rig: rig(&DART_NOZZLES, 0.62, &DART_JETS),
    drive: 0.62,
};

const BLADE_NOZZLES: [[f32; 3]; 2] = [[-54.0, 22.0, 11.0], [-54.0, -22.0, 11.0]];
const BLADE_JETS: [[f32; 3]; 4] = [
    [-26.0, -22.0, 6.0],
    [-26.0, 22.0, 6.0],
    [34.0, -4.0, 9.6],
    [34.0, 4.0, 9.6],
];
const BLADE: Fit = Fit {
    nozzles: &BLADE_NOZZLES,
    lift_jets: BLADE_JETS,
    lamps: CapitalLamps {
        floods: &[
            [40.0, 3.0, 9.4],
            [40.0, -3.0, 9.4],
            [-12.0, 22.0, 5.6],
            [-12.0, -22.0, 5.6],
        ],
        nav_port: [8.0, 29.0, 14.0],
        nav_starboard: [8.0, -29.0, 14.0],
        strobes: &[[64.0, 0.0, 18.4], [-44.0, 0.0, 29.4]],
        beacons: &[[-4.0, 0.0, 33.6]],
        hold: None,
    },
    rig: rig(&BLADE_NOZZLES, 0.45, &BLADE_JETS),
    drive: 0.45,
};

/// The hull directions, by mesh key.
#[derive(Clone, Copy)]
pub(super) enum Hull {
    Keel,
    Dart,
    Blade,
}

pub(crate) fn fit(mesh: &str) -> Option<&'static Fit> {
    Some(match mesh {
        "rail_corvette" => &KEEL,
        "rail_corvette~dart" => &DART,
        "rail_corvette~blade" => &BLADE,
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

/// The same hull `stations` offset to `y`.
fn hull_at(b: &mut MeshBuilder, y: f32, stations: &[[f32; 5]]) {
    b.at(v3(0.0, y, 0.0), |b| hull(b, stations));
}

pub(super) fn build(b: &mut MeshBuilder, kind: Hull) {
    let fit = match kind {
        Hull::Keel => &KEEL,
        Hull::Dart => &DART,
        Hull::Blade => &BLADE,
    };
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid(v3(2.0, 0.0, 16.0), v3(112.0, 22.0, 16.0));
        b.paint(PLATING_DARK);
        b.cuboid(v3(-48.0, 0.0, 15.0), v3(20.0, 30.0, 10.0));
        b.paint(ACCENT);
        b.cuboid(v3(40.0, 0.0, 30.0), v3(40.0, 3.0, 3.0));
        return;
    }
    let deck = match kind {
        Hull::Keel => keel_hull(b, fit),
        Hull::Dart => dart_hull(b, fit),
        Hull::Blade => blade_hull(b, fit),
    };
    guns(b, deck);
    for jet in &fit.lift_jets {
        capital::lift_jet(b, Vec3::from(*jet), 0.5);
    }
    lamp_fittings(b, &fit.lamps);
}

/// The guns and the pedestals that carry them up off `deck` (the hull's top under each
/// gun: rail, rotary).
fn guns(b: &mut MeshBuilder, deck: [f32; 2]) {
    let [rx, _, _] = RAIL;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(
        v3(rx, 0.0, deck[0] - 1.0),
        b.sides(12),
        9.6,
        8.6,
        RAIL_RING - deck[0] + 0.2,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(rx, 0.0, RAIL_RING - 0.8), b.sides(12), 8.9, 8.9, 0.8);
    resolute::rail_turret_sized(b, 0, Vec3::from(RAIL), RAIL_SCALE, RAIL_REACH);
    let [gx, _, _] = GATLING;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(
        v3(gx, 0.0, deck[1] - 1.0),
        b.sides(10),
        5.2,
        3.2,
        GATLING_POST - deck[1] + 1.2,
    );
    capital::rotary_house(b, 1, Vec3::from(GATLING), false, GATLING_SIZE);
}

/// A drive pod: an octagonal nacelle from `fore` back onto a drive of `size` at `nozzle`.
fn pod(b: &mut MeshBuilder, nozzle: [f32; 3], fore: f32, r: f32, size: f32) {
    let [nx, y, z] = nozzle;
    let aft = nx + 27.6 * size;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            section(fore, r * 0.5, z - r * 0.5, z + r * 0.5, r * 0.2),
            section(fore - r * 1.6, r, z - r, z + r, r * 0.35),
            section(aft + 1.0, r * 1.02, z - r * 1.02, z + r * 1.02, r * 0.35),
        ]
        .into_iter()
        .map(|s| s.into_iter().map(|p| p + Vec3::Y * y).collect())
        .collect::<Vec<_>>(),
        true,
        true,
    );
    b.paint(ACCENT);
    b.at(v3(0.0, y, 0.0), |b| {
        b.loft(
            &[
                section(aft + 1.2, r * 1.06, z - r * 1.06, z + r * 1.06, r * 0.37),
                section(aft - 0.8, r * 0.96, z - r * 0.96, z + r * 0.96, r * 0.33),
            ],
            true,
            true,
        );
    });
    if b.fine() {
        b.paint(TEAM);
        b.at(v3(0.0, y, 0.0), |b| {
            b.loft(
                &[
                    section(aft + 6.0, r * 1.05, z - r * 1.05, z + r * 1.05, r * 0.37),
                    section(aft + 4.4, r * 1.05, z - r * 1.05, z + r * 1.05, r * 0.37),
                ],
                true,
                true,
            );
        });
    }
    capital::drive(b, Vec3::from(nozzle), size);
}

/// A skid on the ground at `y` from `x0` to `x1`, on struts up to the belly at `keel`
/// (half width `keel_y`). Mirror it.
fn skid(b: &mut MeshBuilder, x0: f32, x1: f32, y: f32, keel: f32, keel_y: f32) {
    b.paint(ACCENT).pattern(pattern::AIRFRAME);
    b.frustum(
        v3((x0 + x1) * 0.5, y, 0.0),
        v2(x1 - x0, 2.8),
        v2(x1 - x0 - 3.0, 2.0),
        1.3,
        v2(0.0, 0.0),
    );
    b.paint(METAL);
    for x in [x0 + 3.0, x1 - 3.0] {
        b.beam(
            v3(x, y, 1.2),
            v3(x + 1.0, keel_y, keel + 0.5),
            v2(1.5, 1.2),
            v2(1.3, 1.0),
        );
    }
}

/// A bridge block: a stepped house from `deck` up to `top` between the guns, a band of
/// glass across its face.
fn bridge(b: &mut MeshBuilder, x: f32, deck: f32, top: f32, w: f32) {
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            section(x + 6.0, w * 0.6, deck, top - 2.5, 1.0),
            section(x + 3.5, w, deck, top, 1.4),
            section(x - 10.0, w, deck, top, 1.4),
            section(x - 12.0, w * 0.8, deck, top - 1.5, 1.2),
        ],
        true,
        true,
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
            v3(x - 4.0, 0.0, top + 6.0),
            0.5,
            0.3,
            6,
        );
        team_panel(b, v3(x - 9.0, 0.0, top), v2(3.0, w));
    }
}

/// Keel: a faceted armoured hull with a raked prow, the rail house forward on its back, a
/// small bridge amidships and two drive pods on stubs astern. Returns the deck under the
/// guns.
fn keel_hull(b: &mut MeshBuilder, fit: &Fit) -> [f32; 2] {
    const STATIONS: [[f32; 5]; 7] = [
        [61.0, 2.0, 14.0, 17.0, 0.8],
        [54.0, 6.5, 10.5, 21.0, 2.6],
        [40.0, 11.0, 7.5, 24.0, 4.0],
        [0.0, 12.0, 7.0, 24.0, 4.2],
        [-30.0, 12.0, 7.0, 24.0, 4.2],
        [-44.0, 10.0, 8.5, 22.0, 3.6],
        [-50.0, 8.0, 10.0, 20.0, 3.0],
    ];
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    hull(b, &STATIONS);
    // A dark armoured belt low on each side and a keel plate under the belly.
    b.paint(PLATING_DARK);
    hull(
        b,
        &[
            [52.0, 7.2, 10.0, 15.0, 2.0],
            [40.0, 11.6, 7.2, 15.0, 3.6],
            [-32.0, 12.6, 6.6, 15.0, 3.6],
            [-46.0, 10.4, 8.2, 15.0, 3.0],
        ],
    );
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.beam(
            v3(52.0, 6.8, 20.5),
            v3(42.0, 11.3, 23.4),
            v2(1.2, 0.3),
            v2(1.2, 0.3),
        );
    });
    // Stubs out to the pods.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.mirror_y(|b| {
        b.frustum(
            v3(-40.0, 10.0, 12.5),
            v2(18.0, 1.0),
            v2(12.0, 1.0),
            4.0,
            v2(-3.0, 0.0),
        );
    });
    for &n in fit.nozzles {
        pod(b, n, -14.0, 5.0, fit.drive);
    }
    bridge(b, -4.0, 24.0, 33.0, 6.0);
    b.mirror_y(|b| skid(b, -30.0, 30.0, 8.0, 7.0, 6.0));
    [24.0, 24.0]
}

/// A lens section: half span `w`, thickness `t`, centred at `zc`; pale above, dark below.
fn lens(x: f32, w: f32, t: f32, zc: f32, up: bool) -> Vec<Vec3> {
    let s = if up { 1.0 } else { -0.85 };
    vec![
        v3(x, w, zc),
        v3(x, 0.78 * w, zc + 0.42 * t * s),
        v3(x, 0.32 * w, zc + 0.6 * t * s),
        v3(x, -0.32 * w, zc + 0.6 * t * s),
        v3(x, -0.78 * w, zc + 0.42 * t * s),
        v3(x, -w, zc),
    ]
}

/// Dart: one blended arrowhead lifting body, pale above and dark below, a raised spine
/// carrying the guns, canted fins at the trailing corners and one big drive astern.
fn dart_hull(b: &mut MeshBuilder, fit: &Fit) -> [f32; 2] {
    const STATIONS: [[f32; 4]; 7] = [
        [63.0, 1.5, 1.4, 15.5],
        [52.0, 7.0, 7.0, 15.5],
        [30.0, 15.0, 12.0, 16.0],
        [0.0, 24.0, 14.0, 16.0],
        [-28.0, 31.0, 13.0, 16.0],
        [-38.0, 31.0, 10.0, 16.0],
        [-44.0, 12.0, 11.0, 16.0],
    ];
    for (up, paint) in [(true, PLATING), (false, PLATING_DARK)] {
        b.paint(paint).pattern(pattern::AIRFRAME);
        b.loft(
            &STATIONS
                .iter()
                .map(|s| lens(s[0], s[1], s[2], s[3], up))
                .collect::<Vec<_>>(),
            true,
            true,
        );
    }
    // The spine: a narrow armoured ridge the length of the back.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    hull(
        b,
        &[
            [50.0, 1.5, 17.0, 20.0, 0.6],
            [36.0, 5.0, 17.0, 24.0, 1.8],
            [-36.0, 6.0, 17.0, 25.0, 2.2],
            [-46.0, 5.0, 14.0, 22.0, 2.0],
        ],
    );
    let [nx, _, nz] = fit.nozzles[0];
    pod(b, [nx, 0.0, nz], -30.0, 7.0, fit.drive);
    b.mirror_y(|b| {
        // Canted fins at the trailing corners, the owner's colour on their tips.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.beam(
            v3(-34.0, 28.0, 16.0),
            v3(-44.0, 32.0, 28.0),
            v2(12.0, 1.0),
            v2(6.0, 0.8),
        );
        b.paint(TEAM);
        b.beam(
            v3(-43.0, 31.6, 27.0),
            v3(-45.0, 32.1, 29.5),
            v2(6.0, 0.9),
            v2(5.4, 0.9),
        );
        if b.fine() {
            b.beam(
                v3(50.0, 5.4, 19.2),
                v3(34.0, 11.4, 22.4),
                v2(1.2, 0.2),
                v2(1.2, 0.2),
            );
        }
    });
    bridge(b, -4.0, 25.0, 31.0, 5.0);
    b.mirror_y(|b| skid(b, -24.0, 26.0, 9.0, 8.4, 6.0));
    [24.0, 25.0]
}

/// Blade: a long, narrow armoured spine that is mostly gun mount, a keel fin under it, and
/// two drive nacelles out on swept pylons like outriggers.
fn blade_hull(b: &mut MeshBuilder, fit: &Fit) -> [f32; 2] {
    const SPINE: [[f32; 5]; 7] = [
        [64.0, 1.2, 15.0, 19.0, 0.5],
        [54.0, 4.5, 12.0, 22.0, 1.8],
        [36.0, 7.0, 10.0, 24.0, 2.6],
        [-10.0, 7.5, 10.0, 24.0, 2.8],
        [-36.0, 7.5, 10.0, 24.0, 2.8],
        [-46.0, 6.0, 12.0, 23.0, 2.4],
        [-50.0, 4.0, 14.0, 21.0, 1.8],
    ];
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    hull(b, &SPINE);
    // The keel fin under the spine, and armoured cheeks along it.
    b.paint(PLATING_DARK);
    b.loft(
        &[
            vec![
                v3(46.0, 0.0, 10.5),
                v3(46.0, 2.0, 11.0),
                v3(46.0, -2.0, 11.0),
            ],
            vec![
                v3(20.0, 0.0, 6.0),
                v3(20.0, 2.6, 10.5),
                v3(20.0, -2.6, 10.5),
            ],
            vec![
                v3(-30.0, 0.0, 6.0),
                v3(-30.0, 2.6, 10.5),
                v3(-30.0, -2.6, 10.5),
            ],
            vec![
                v3(-44.0, 0.0, 11.0),
                v3(-44.0, 2.0, 12.0),
                v3(-44.0, -2.0, 12.0),
            ],
        ],
        true,
        true,
    );
    b.mirror_y(|b| {
        hull_at(
            b,
            7.0,
            &[
                [44.0, 0.6, 13.0, 19.0, 0.3],
                [32.0, 2.0, 12.0, 21.0, 0.8],
                [-30.0, 2.0, 12.0, 21.0, 0.8],
                [-40.0, 0.8, 13.5, 20.0, 0.4],
            ],
        );
        // Swept pylon out to the nacelle, and a canard forward.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                vec![
                    v3(-4.0, 6.0, 15.0),
                    v3(-4.0, 6.0, 18.0),
                    v3(-26.0, 6.0, 18.0),
                    v3(-26.0, 6.0, 15.0),
                ],
                vec![
                    v3(-18.0, 21.0, 11.0),
                    v3(-18.0, 21.0, 13.0),
                    v3(-32.0, 21.0, 13.0),
                    v3(-32.0, 21.0, 11.0),
                ],
            ],
            true,
            true,
        );
        b.loft(
            &[
                vec![
                    v3(42.0, 6.0, 15.0),
                    v3(42.0, 6.0, 16.6),
                    v3(30.0, 6.0, 16.6),
                    v3(30.0, 6.0, 15.0),
                ],
                vec![
                    v3(28.0, 18.0, 14.0),
                    v3(28.0, 18.0, 14.8),
                    v3(22.0, 18.0, 14.8),
                    v3(22.0, 18.0, 14.0),
                ],
            ],
            true,
            true,
        );
        b.paint(TEAM);
        b.cuboid(v3(25.0, 17.6, 14.4), v3(6.0, 1.0, 1.0));
    });
    for &n in fit.nozzles {
        pod(b, n, 6.0, 5.2, fit.drive);
    }
    // Skids under the nacelles and a nose skid under the fin.
    b.mirror_y(|b| skid(b, -36.0, -6.0, 22.0, 5.8, 22.0));
    skid(b, 26.0, 44.0, 0.0, 8.0, 0.0);
    bridge(b, -4.0, 24.0, 33.0, 5.0);
    [24.0, 24.0]
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

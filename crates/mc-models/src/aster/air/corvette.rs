//! Valiant: the ARC's tech 2 rail corvette (`aster_t2_corvette`, mesh `rail_corvette`), a
//! small warship of the upper air. Its long rail cannon hangs under the belly on a post,
//! the Resolute's turret house with its rails drawn out, free to turn right round and
//! lay onto the ground below; a rotary AA cannon stands on a raised barbette on its back
//! with the whole sky over it. The hull is faceted with a hard chine, a wedge bridge
//! forward, a raised engineering block aft, and two drive pods out on swept stubs.
//! +X is forward, +Y port, the ground at z 0 with the legs down.
//!
//! Contracts:
//! - `RAIL` is weapon 0's pivot and `RAIL_REACH` how far ahead of it the rails end (the
//!   unit file's `muzzle` - `pivot`); `RAIL_CHARGE` is where its charge crawls.
//! - `GATLING` is weapon 1's pivot, a `capital::rotary_house` of `GATLING_SIZE`, its
//!   muzzles `12 * GATLING_SIZE` ahead of the pivot as authored.
//! - `NOZZLES`, `LIFT_JETS`, `LAMPS`, `RIG`: the shared spacecraft rig (`capital.rs`),
//!   two pairs of legs that stow into belly bays included.
use super::capital::{self, CapitalRig, Leg};
use super::resolute;
use super::*;
use crate::{CapitalLamps, TurretRail};

/// The long rail cannon's pivot, under the belly, and how far ahead of it the rails end.
pub(super) const RAIL: [f32; 3] = [10.0, 0.0, 9.0];
pub(super) const RAIL_REACH: f32 = 40.0;
/// The rail house is the Resolute's at this size (its rails drawn out to `RAIL_REACH`).
const RAIL_SCALE: f32 = 1.0;
pub(crate) const RAIL_CHARGE: TurretRail = resolute::turret_rail(RAIL_SCALE, RAIL_REACH);
/// The house's roof over its pivot (`resolute::HOUSE_ROOF`, authored size).
const RAIL_ROOF: f32 = RAIL[2] + 4.2 * RAIL_SCALE;
/// The rotary AA cannon's pivot and size (`capital::rotary_house`, standing).
pub(super) const GATLING: [f32; 3] = [-4.0, 0.0, 42.0];
pub(super) const GATLING_SIZE: f32 = 0.9;
/// The top of the rotary gun's post: its barbette rises from the deck to here.
const GATLING_POST: f32 = GATLING[2] - 6.6 * GATLING_SIZE;

/// The flat belly the ship sits over on its legs, the chine line, and the main deck.
const KEEL: f32 = 16.0;
const CHINE: f32 = 23.0;
const DECK: f32 = 33.0;

/// Leg size against the Bastion's (hinge 36 m over the foot).
const LEG: f32 = 0.55;
const LEG_Y: f32 = 5.5;
const BAY: f32 = 3.2;

/// The drive mouths (model space): the exhaust trail and drive effects start here.
pub(crate) const NOZZLES: [[f32; 3]; 2] = [[-60.0, 15.0, 23.0], [-60.0, -15.0, 23.0]];
const DRIVE: f32 = 0.48;
/// Lift jets under the belly, clear of the leg bays and the rail's post:
/// `[aft -y, aft +y, fore -y, fore +y]`.
pub(crate) const LIFT_JETS: [[f32; 3]; 4] = [
    [-36.0, -4.0, KEEL - 0.4],
    [-36.0, 4.0, KEEL - 0.4],
    [-6.0, -4.0, KEEL - 0.4],
    [-6.0, 4.0, KEEL - 0.4],
];

/// Lamp fittings on the hull: floods under the bow and the stern, nav lights on the pods'
/// flanks, strobes at the bow and on the mast, a beacon on the engineering block.
pub(crate) const LAMPS: CapitalLamps = CapitalLamps {
    floods: &[
        [46.0, 4.0, 16.4],
        [46.0, -4.0, 16.4],
        [-44.0, 4.5, 17.6],
        [-44.0, -4.5, 17.6],
    ],
    nav_port: [-44.0, 21.0, 23.0],
    nav_starboard: [-44.0, -21.0, 23.0],
    strobes: &[[66.4, 0.0, 25.0], [-32.0, 0.0, 45.4]],
    beacons: &[[-44.0, 0.0, 38.8]],
    hold: None,
};

/// What `entity.wgsl` animates (`models::capital_rig`): two pairs of legs (the fore pair
/// stows aft, the aft pair forward), the drives and the lift jets. No ramp.
pub(crate) const RIG: CapitalRig = CapitalRig {
    legs: Some([
        Leg {
            hinge: [38.0, LEG_Y, 36.0 * LEG],
            stow: 1.0,
            bay: [19.6, 40.4, LEG_Y - BAY, LEG_Y + BAY],
            size: LEG,
        },
        Leg {
            hinge: [-30.0, LEG_Y, 36.0 * LEG],
            stow: -1.0,
            bay: [-32.4, -11.6, LEG_Y - BAY, LEG_Y + BAY],
            size: LEG,
        },
    ]),
    door_hinge: KEEL - 0.3,
    drives: Some((
        [NOZZLES[0][0], NOZZLES[0][2], NOZZLES[0][1], NOZZLES[0][1]],
        DRIVE,
    )),
    lift_jets: Some((
        [
            LIFT_JETS[3][0],
            LIFT_JETS[3][1],
            LIFT_JETS[1][0],
            LIFT_JETS[1][1],
        ],
        LIFT_JETS[3][2],
    )),
    ramp: None,
};

/// A hull section across x with a hard chine: the belly's half width `wb` at `lo`, the
/// chine's `wc` at `zc`, the deck edge's `wd` at `hi`.
fn chined(x: f32, [wb, lo, wc, zc, wd, hi]: [f32; 6]) -> Vec<Vec3> {
    [
        (-wb, lo),
        (wb, lo),
        (wc, zc),
        (wd, hi),
        (-wd, hi),
        (-wc, zc),
    ]
    .iter()
    .map(|&(y, z)| v3(x, y, z))
    .collect()
}

/// A hull lofted through chined stations (x, then `chined`'s six numbers), capped.
fn chined_hull(b: &mut MeshBuilder, stations: &[(f32, [f32; 6])]) {
    b.loft(
        &stations
            .iter()
            .map(|&(x, s)| chined(x, s))
            .collect::<Vec<_>>(),
        true,
        true,
    );
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

/// A block lofted through `stations` (x, half width, bottom, top, chamfer), capped.
fn block(b: &mut MeshBuilder, stations: &[[f32; 5]]) {
    b.loft(
        &stations
            .iter()
            .map(|s| section(s[0], s[1], s[2], s[3], s[4]))
            .collect::<Vec<_>>(),
        true,
        true,
    );
}

/// The main hull: x, then belly half width and height, chine half width and height, deck
/// edge half width and height. A sharp raked prow, the full section amidships, drawn in
/// and lifted toward the stern.
const HULL: [(f32, [f32; 6]); 9] = [
    (66.0, [0.6, 24.0, 1.2, 25.0, 0.6, 26.0]),
    (60.0, [2.6, 21.0, 5.6, CHINE + 1.0, 3.4, 29.0]),
    (50.0, [5.4, 17.6, 10.6, CHINE, 7.0, 31.6]),
    (40.0, [7.0, KEEL, 13.0, CHINE, 8.8, DECK]),
    (-12.0, [7.4, KEEL, 14.0, CHINE, 10.0, DECK]),
    (-38.0, [7.0, KEEL, 13.4, CHINE, 9.6, DECK]),
    (-46.0, [6.0, 18.0, 11.6, 23.6, 8.4, 31.6]),
    (-52.0, [4.6, 20.0, 9.0, 24.0, 6.4, 29.6]),
    (-54.0, [3.6, 21.0, 7.0, 24.0, 5.0, 28.4]),
];

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid(v3(4.0, 0.0, 24.5), v3(116.0, 26.0, 17.0));
        b.paint(PLATING_DARK);
        b.cuboid(v3(-50.0, 0.0, 23.0), v3(20.0, 40.0, 10.0));
        b.cuboid(v3(30.0, 0.0, 9.0), v3(40.0, 4.0, 3.0));
        b.paint(ACCENT);
        b.cuboid(v3(-4.0, 0.0, 38.0), v3(8.0, 8.0, 10.0));
        return;
    }
    hull(b);
    bridge(b);
    engineering(b);
    b.mirror_y(stub);
    for n in NOZZLES {
        pod(b, n);
    }
    rail_mount(b);
    gatling_mount(b);
    capital::gear(b, &RIG, KEEL - 0.7);
    for jet in &LIFT_JETS {
        capital::lift_jet(b, Vec3::from(*jet), 0.5);
    }
    lamp_fittings(b, &LAMPS);
}

/// The hull with its chine strakes, deck edges, ribs and seams.
fn hull(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    chined_hull(b, &HULL);
    // The lower hull under the chine in dark armour, stood a little proud of the plating.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    chined_hull(
        b,
        &[
            (58.0, [3.1, 20.4, 6.5, CHINE + 1.1, 5.3, 24.6]),
            (50.0, [5.8, 17.2, 11.2, CHINE, 9.4, 25.4]),
            (40.0, [7.4, KEEL - 0.2, 13.5, CHINE, 11.6, 26.0]),
            (-38.0, [7.4, KEEL - 0.2, 13.9, CHINE, 12.4, 26.0]),
            (-46.0, [6.4, 17.8, 12.1, 23.6, 10.8, 26.0]),
            (-51.0, [5.0, 19.8, 9.6, 24.0, 8.2, 25.8]),
        ],
    );
    b.mirror_y(|b| {
        // The chine strake: an armoured rubbing band along the hull's widest line, the
        // owner's colour on its forward run.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.beam(
            v3(50.0, 11.3, CHINE),
            v3(-44.0, 12.8, CHINE + 0.3),
            v2(0.8, 1.4),
            v2(0.8, 1.4),
        );
        b.beam(
            v3(50.0, 11.3, CHINE),
            v3(60.0, 6.1, CHINE + 1.0),
            v2(0.8, 1.2),
            v2(0.6, 1.0),
        );
        b.paint(TEAM);
        b.beam(
            v3(54.0, 9.4, 27.4),
            v3(42.0, 11.9, 29.8),
            v2(1.4, 0.3),
            v2(1.4, 0.3),
        );
        // Raised deck edges (coamings) along the main deck.
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.beam(
            v3(40.0, 8.4, DECK + 0.4),
            v3(-38.0, 9.2, DECK + 0.4),
            v2(1.2, 0.8),
            v2(1.2, 0.8),
        );
        if b.mid() {
            // Frames down the flanks between the chine and the deck edge: they read the
            // hull's length from the RTS camera.
            b.paint(ACCENT);
            for x in [30.0, 16.0, 2.0, -12.0, -26.0] {
                let wc = 13.0 + (40.0 - x) * 0.012;
                b.beam(
                    v3(x, wc + 0.1, CHINE + 1.0),
                    v3(x, 9.6, DECK - 0.4),
                    v2(1.4, 0.5),
                    v2(1.4, 0.5),
                );
            }
        }
        if b.fine() {
            // Armoured hatches and vents on the deck, seams on the flanks.
            b.paint(PLATING_DARK);
            b.plate(v3(34.0, 5.0, DECK), v2(6.0, 3.0), 0.3, 0.1);
            b.plate(v3(-18.0, 5.4, DECK), v2(8.0, 3.2), 0.3, 0.1);
            vent(b, v3(8.0, 6.2, DECK), v2(6.0, 2.4), 5, METAL);
            b.paint(PLATING_DARK);
            b.beam(
                v3(40.0, 13.2, CHINE - 2.5),
                v3(-38.0, 13.6, CHINE - 2.5),
                v2(0.3, 0.3),
                v2(0.3, 0.3),
            );
        }
    });
    // A dark keel plate under the belly between the leg bays and the posts.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(-24.0, 0.0, KEEL - 0.9),
        v2(18.0, 3.4),
        v2(20.0, 4.0),
        0.9,
        v2(0.0, 0.0),
    );
    // A sensor prong under the prow, its tip glassed.
    b.paint(PLATING_DARK);
    b.beam(
        v3(58.0, 0.0, 21.8),
        v3(69.0, 0.0, 22.6),
        v2(2.2, 2.0),
        v2(0.9, 0.9),
    );
    b.paint(VISOR);
    b.beam(
        v3(68.4, 0.0, 22.56),
        v3(69.4, 0.0, 22.62),
        v2(0.9, 0.9),
        v2(0.5, 0.5),
    );
}

/// The wedge bridge forward on the deck: a low raked house with a band of glass.
fn bridge(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    block(
        b,
        &[
            [48.0, 3.0, DECK - 1.0, DECK + 0.4, 0.4],
            [44.0, 6.2, DECK - 0.5, DECK + 3.2, 1.0],
            [30.0, 7.2, DECK - 0.5, DECK + 4.2, 1.4],
            [22.0, 6.4, DECK - 0.5, DECK + 3.6, 1.2],
        ],
    );
    b.paint(GLASS);
    b.loft(
        &[
            vec![
                v3(44.6, 5.4, DECK + 1.6),
                v3(44.6, -5.4, DECK + 1.6),
                v3(43.6, -5.6, DECK + 2.9),
                v3(43.6, 5.6, DECK + 2.9),
            ],
            vec![
                v3(40.6, 6.6, DECK + 2.3),
                v3(40.6, -6.6, DECK + 2.3),
                v3(39.6, -6.7, DECK + 3.8),
                v3(39.6, 6.7, DECK + 3.8),
            ],
        ],
        true,
        true,
    );
    team_panel(b, v3(28.0, 0.0, DECK + 4.2), v2(5.0, 6.0));
    if b.fine() {
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.chamfered_box(v3(32.0, 6.9, DECK + 2.0), v3(4.0, 1.0, 2.4), 0.3);
        });
    }
}

/// The raised engineering block aft: radiator fins on its roof, a mast with a turning
/// search array, a beacon.
fn engineering(b: &mut MeshBuilder) {
    let top = DECK + 5.0;
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    block(
        b,
        &[
            [-22.0, 6.0, DECK - 0.5, top - 2.0, 1.2],
            [-26.0, 8.4, DECK - 0.5, top, 1.6],
            [-44.0, 8.4, DECK - 0.5, top, 1.6],
            [-50.0, 6.0, 29.0, top - 1.4, 1.4],
        ],
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let fins = if b.fine() { 5 } else { 3 };
        for i in 0..fins {
            let x = -28.0 - i as f32 * 14.0 / (fins - 1) as f32;
            b.plate(v3(x, 4.8, top), v2(1.0, 4.0), 1.6, 0.1);
        }
    });
    // The mast and its search array, which sweeps back and forth.
    b.paint(METAL);
    b.cylinder_between(
        v3(-32.0, 0.0, top),
        v3(-32.0, 0.0, top + 7.0),
        0.9,
        0.6,
        b.sides(8),
    );
    let pivot = v3(-32.0, 0.0, top + 5.0);
    b.set_spinner_pivot(pivot);
    b.set_spinner_scan();
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.beam(
            pivot + v3(0.4, 4.2, 0.0),
            pivot + v3(0.4, -4.2, 0.0),
            v2(0.5, 1.6),
            v2(0.5, 1.6),
        );
        b.paint(ACCENT);
        b.cuboid(pivot + v3(-0.4, 0.0, 0.0), v3(1.0, 1.4, 1.4));
    });
}

/// A swept stub from the hull's quarter out to the port pod.
fn stub(b: &mut MeshBuilder) {
    let [nx, y, z] = NOZZLES[0];
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            vec![
                v3(nx + 44.0, 12.0, z - 2.0),
                v3(nx + 44.0, 12.0, z + 2.4),
                v3(nx + 20.0, 12.0, z + 2.4),
                v3(nx + 20.0, 12.0, z - 2.0),
            ],
            vec![
                v3(nx + 32.0, y, z - 1.4),
                v3(nx + 32.0, y, z + 1.6),
                v3(nx + 16.0, y, z + 1.6),
                v3(nx + 16.0, y, z - 1.4),
            ],
        ],
        true,
        true,
    );
    if b.fine() {
        b.paint(TEAM);
        b.beam(
            v3(nx + 41.0, 12.2, z + 2.5),
            v3(nx + 30.0, y - 3.0, z + 1.7),
            v2(1.4, 0.2),
            v2(1.4, 0.2),
        );
    }
}

/// A drive pod: an intake ring at its nose, an armoured octagonal nacelle, the drive.
fn pod(b: &mut MeshBuilder, nozzle: [f32; 3]) {
    let [nx, y, z] = nozzle;
    let aft = nx + 27.6 * DRIVE;
    let (fore, r) = (-18.0, 5.4);
    b.at(v3(0.0, y, 0.0), |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        block(
            b,
            &[
                [fore, r * 0.8, z - r * 0.8, z + r * 0.8, r * 0.3],
                [fore - 4.0, r, z - r, z + r, r * 0.35],
                [aft + 1.0, r * 1.02, z - r * 1.02, z + r * 1.02, r * 0.35],
            ],
        );
        // The intake: a dark ring stood proud of the nose, its mouth darker still.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        block(
            b,
            &[
                [fore + 1.6, r * 0.9, z - r * 0.9, z + r * 0.9, r * 0.33],
                [fore - 0.4, r * 0.9, z - r * 0.9, z + r * 0.9, r * 0.33],
            ],
        );
        b.paint(METAL);
        block(
            b,
            &[
                [fore + 1.7, r * 0.6, z - r * 0.6, z + r * 0.6, r * 0.2],
                [fore + 1.0, r * 0.6, z - r * 0.6, z + r * 0.6, r * 0.2],
            ],
        );
        // An armour saddle over the pod, and a collar where it meets the drive.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        block(
            b,
            &[
                [fore - 6.0, r * 0.7, z + r * 0.6, z + r * 1.18, 0.6],
                [aft + 4.0, r * 0.8, z + r * 0.6, z + r * 1.22, 0.6],
            ],
        );
        b.paint(ACCENT);
        block(
            b,
            &[
                [aft + 1.2, r * 1.06, z - r * 1.06, z + r * 1.06, r * 0.37],
                [aft - 0.8, r * 0.96, z - r * 0.96, z + r * 0.96, r * 0.33],
            ],
        );
        if b.fine() {
            b.paint(TEAM);
            block(
                b,
                &[
                    [aft + 6.0, r * 1.05, z - r * 1.05, z + r * 1.05, r * 0.37],
                    [aft + 4.4, r * 1.05, z - r * 1.05, z + r * 1.05, r * 0.37],
                ],
            );
        }
    });
    capital::drive(b, Vec3::from(nozzle), DRIVE);
}

/// The rail cannon's post: an armoured drum from the belly down to the house's roof, so
/// the house hangs clear under the hull, turns right round and its rails depress freely.
fn rail_mount(b: &mut MeshBuilder) {
    let [x, _, _] = RAIL;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(
        v3(x - 2.0, 0.0, RAIL_ROOF - 0.2),
        b.sides(10),
        5.6,
        7.6,
        KEEL - RAIL_ROOF + 0.8,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(
        v3(x - 2.0, 0.0, RAIL_ROOF - 0.6),
        b.sides(10),
        6.2,
        6.2,
        1.0,
    );
    b.prism(v3(x - 2.0, 0.0, KEEL - 1.2), b.sides(10), 8.4, 8.0, 1.0);
    resolute::rail_turret_sized(b, 0, Vec3::from(RAIL), RAIL_SCALE, RAIL_REACH);
}

/// The rotary gun's barbette: an armoured drum from the deck up under its post, high
/// enough that the barrels clear the bridge and the engineering block all round.
fn gatling_mount(b: &mut MeshBuilder) {
    let [x, _, _] = GATLING;
    let plan = crate::builder::chamfered_rect(v2(7.0, 7.0), 2.6);
    b.at(v3(x, 0.0, 0.0), |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.loft_z(
            &plan,
            &[
                Section::new(DECK - 0.5, 1.15),
                Section::new(GATLING_POST - 1.4, 1.0),
            ],
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &plan,
            &[
                Section::new(GATLING_POST - 1.4, 0.9),
                Section::new(GATLING_POST, 0.9),
            ],
        );
    });
    capital::rotary_house(b, 1, Vec3::from(GATLING), false, GATLING_SIZE);
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

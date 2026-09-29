//! C: the crescent. A faceted, angular hull like a blade, a forward-swept crescent across
//! its bow whose horns reach round a big sensor lens in the nose, apertures down the
//! horns' inner edges looking into the bay. A V tail over the single drive and a scanning
//! head on the spine.
use super::*;

const NOZZLE: [f32; 3] = [-36.0, 0.0, 9.5];
const LIFT_JETS: [[f32; 3]; 4] = [
    [-12.0, -2.2, 4.1],
    [-12.0, 2.2, 4.1],
    [10.0, -2.2, 4.1],
    [10.0, 2.2, 4.1],
];

pub(super) static FIT: Fit = Fit {
    nozzles: [NOZZLE],
    lift_jets: LIFT_JETS,
    lamps: CapitalLamps {
        floods: &[
            [21.0, 2.0, 4.0],
            [21.0, -2.0, 4.0],
            [-17.0, 3.5, 4.0],
            [-17.0, -3.5, 4.0],
        ],
        nav_port: [34.2, 22.4, 8.7],
        nav_starboard: [34.2, -22.4, 8.7],
        strobes: &[[-20.0, 6.9, 18.8], [-20.0, -6.9, 18.8]],
        beacons: &[[-8.0, 0.0, 15.1]],
        hold: None,
    },
    rig: rig(NOZZLE, LIFT_JETS),
};

const KEEL: f32 = 4.5;
/// Hull stations: x, keel half width, chine half width and height, deck half width and height.
const STATIONS: [[f32; 6]; 5] = [
    [27.0, 2.5, 5.0, 8.0, 3.0, 11.5],
    [20.0, 3.5, 7.5, 7.5, 3.5, 13.0],
    [2.0, 4.0, 8.5, 7.5, 4.0, 14.5],
    [-14.0, 4.0, 8.5, 8.0, 4.0, 14.5],
    [-20.0, 4.0, 7.5, 8.5, 4.0, 13.5],
];
/// The crescent in plan (x, y): horns swept forward round the nose.
const CRESCENT: [[f32; 2]; 14] = [
    [25.0, 0.0],
    [25.0, 7.0],
    [28.0, 14.0],
    [34.0, 22.0],
    [27.0, 25.0],
    [19.0, 17.0],
    [15.0, 8.0],
    [14.0, 0.0],
    [15.0, -8.0],
    [19.0, -17.0],
    [27.0, -25.0],
    [34.0, -22.0],
    [28.0, -14.0],
    [25.0, -7.0],
];
const HEAD_LOW: f32 = 7.8;
const HEAD_EDGE: f32 = 9.4;
const HEAD_TOP: f32 = 10.6;

fn ring(s: &[f32; 6]) -> Vec<Vec3> {
    let [x, keel, chine, cz, deck, dz] = *s;
    vec![
        v3(x, keel, KEEL),
        v3(x, chine, cz),
        v3(x, deck, dz),
        v3(x, -deck, dz),
        v3(x, -chine, cz),
        v3(x, -keel, KEEL),
    ]
}

fn plan(points: &[[f32; 2]], z: f32) -> Vec<Vec3> {
    points.iter().map(|p| v3(p[0], p[1], z)).collect()
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING_DARK);
        b.cuboid(v3(-4.0, 0.0, 9.5), v3(56.0, 15.0, 10.0));
        b.cuboid(v3(-27.0, 0.0, 9.5), v3(10.0, 9.0, 9.0));
        b.paint(PLATING);
        b.cuboid(v3(24.0, 0.0, 9.2), v3(18.0, 50.0, 2.8));
        return;
    }
    hull(b);
    head(b);
    stern(b, NOZZLE, -20.0, 4.8);
    let c = v3(-13.0, 0.0, 13.0);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    for a in [40.0_f32, 140.0] {
        capital::fin(b, c, a.to_radians(), -7.0, 5.0, 1.0, [9.0, 4.0], 0.8);
    }
    b.paint(TEAM);
    for a in [40.0_f32, 140.0] {
        capital::fin(b, c, a.to_radians(), -7.0, -3.0, 7.0, [9.2, 7.5], 0.9);
    }
    scanner(b, v3(4.0, 0.0, 14.4), 2.6);
    b.mirror_y(|b| skid(b, -16.0, 12.0, 5.0, KEEL, 3.5));
    lift_jets(b, &LIFT_JETS);
    lamp_fittings(b, &FIT.lamps);
}

/// The blade hull: dark facets, a pale deck, a lit rail down each chine, the faceted
/// canopy, and the nose lens in its bezel.
fn hull(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&STATIONS.iter().map(ring).collect::<Vec<_>>(), true, true);
    let deck = |s: &[f32; 6]| {
        let [x, _, _, _, w, z] = *s;
        vec![
            v3(x, w + 0.3, z - 0.4),
            v3(x, w - 0.6, z + 0.5),
            v3(x, -(w - 0.6), z + 0.5),
            v3(x, -(w + 0.3), z - 0.4),
        ]
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &STATIONS[1..5].iter().map(deck).collect::<Vec<_>>(),
        true,
        true,
    );
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(GLOW_AMBER);
            let [x0, _, w0, z0, _, _] = STATIONS[1];
            let [x1, _, w1, z1, _, _] = STATIONS[3];
            b.beam(
                v3(x0 - 1.0, w0 + 0.1, z0),
                v3(x1 + 1.0, w1 + 0.1, z1),
                v2(0.25, 0.25),
                v2(0.25, 0.25),
            );
        });
    }
    b.paint(GLASS);
    b.frustum(
        v3(16.0, 0.0, 13.1),
        v2(7.0, 4.4),
        v2(3.6, 2.6),
        1.3,
        v2(-0.8, 0.0),
    );
    // The nose lens: a disc of visor glass in a dark bezel, looking forward.
    let z = 8.6;
    b.paint(ACCENT);
    b.cylinder_between(v3(26.6, 0.0, z), v3(27.4, 0.0, z), 3.3, 3.1, b.sides(16));
    b.paint(VISOR);
    b.cylinder_between(v3(27.3, 0.0, z), v3(27.8, 0.0, z), 2.6, 2.4, b.sides(16));
}

/// The crescent: a dark slab with a pale upper skin, apertures down each horn's inner
/// edge, the owner's colour at the horn tips.
fn head(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[plan(&CRESCENT, HEAD_LOW), plan(&CRESCENT, HEAD_EDGE)],
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            plan(&CRESCENT, HEAD_EDGE),
            plan(&inset(&CRESCENT, 0.95), HEAD_TOP),
        ],
        true,
        true,
    );
    b.mirror_y(|b| {
        let z = (HEAD_LOW + HEAD_EDGE) * 0.5;
        let p = |i: usize| v3(CRESCENT[i][0], CRESCENT[i][1], z);
        aperture(b, p(1), p(2), v2(0.4, 1.0), v3(7.0, -3.0, 0.0));
        aperture(b, p(2), p(3), v2(0.4, 1.0), v3(8.0, -6.0, 0.0));
        b.paint(TEAM);
        b.beam(
            v3(29.5, 21.0, HEAD_TOP - 0.2),
            v3(26.0, 23.2, HEAD_TOP - 0.2),
            v2(1.6, 0.35),
            v2(1.6, 0.35),
        );
    });
}

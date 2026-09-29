//! A: the hammerhead. The bow is the radar: a broad swept array wing across a narrow
//! armoured neck, its leading edges tiled with array faces, sensor pods drooping at the
//! tips and a scanning turret on its crown. Behind the neck the hull steps out to an
//! engineering block with the two drive nacelles.
use super::*;
use glam::Vec2;

const NOZZLES: [[f32; 3]; 2] = [[-56.0, -17.0, 12.0], [-56.0, 17.0, 12.0]];
const LIFT_JETS: [[f32; 3]; 4] = [
    [-30.0, -17.0, 3.6],
    [-30.0, 17.0, 3.6],
    [34.0, -3.0, 3.6],
    [34.0, 3.0, 3.6],
];

pub(super) static FIT: Fit = Fit {
    nozzles: NOZZLES,
    lift_jets: LIFT_JETS,
    lamps: CapitalLamps {
        floods: &[
            [44.0, 1.4, 4.4],
            [44.0, -1.4, 4.4],
            [-40.0, 17.0, 5.4],
            [-40.0, -17.0, 5.4],
        ],
        nav_port: [41.0, 44.2, 14.5],
        nav_starboard: [41.0, -44.2, 14.5],
        strobes: &[[55.2, 0.0, 16.0], [-45.0, 17.0, 19.0], [-45.0, -17.0, 19.0]],
        beacons: &[[-40.0, 0.0, 24.9]],
        hold: None,
    },
    rig: rig(NOZZLES, LIFT_JETS),
};

const KEEL: f32 = 4.0;
/// Hull stations: x, keel half width, chine half width and height, deck half width and
/// height. The neck steps out to the engineering block in one sharp bulkhead at x -8.
const STATIONS: [[f32; 6]; 7] = [
    [52.0, 1.0, 3.0, 11.0, 2.0, 15.0],
    [36.0, 3.5, 6.5, 9.5, 4.5, 20.0],
    [12.0, 4.0, 7.0, 9.5, 5.0, 21.0],
    [-8.0, 4.5, 8.0, 9.5, 5.5, 21.5],
    [-9.5, 5.5, 13.5, 9.0, 10.0, 25.0],
    [-30.0, 5.5, 13.5, 9.0, 9.5, 24.8],
    [-44.0, 5.0, 11.5, 10.0, 8.0, 23.6],
];
/// The array wing in plan (x, y): leading edge swept back to the tips.
const HEAD: [[f32; 2]; 8] = [
    [54.0, 0.0],
    [46.0, 40.0],
    [38.0, 42.0],
    [30.0, 8.0],
    [28.0, 0.0],
    [30.0, -8.0],
    [38.0, -42.0],
    [46.0, -40.0],
];
const HEAD_LOW: f32 = 13.0;
const HEAD_EDGE: f32 = 16.2;
const HEAD_TOP: f32 = 18.6;
/// The crown turret's pivot: it looks about rather than turning round.
const CROWN: Vec3 = Vec3::new(41.0, 0.0, 19.4);

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

/// A point on the +y upper flank of station `s`, `t` of the way from chine to deck edge,
/// stood `lift` off the plating.
fn flank(s: &[f32; 6], t: f32, lift: f32) -> Vec3 {
    let [x, _, chine, cz, deck, dz] = *s;
    let (a, c) = (v2(chine, cz), v2(deck, dz));
    let n = v2(c.y - a.y, -(c.x - a.x)).normalize();
    let q = a + (c - a) * t + n * lift;
    v3(x, q.x, q.y)
}

fn plan_ring(plan: &[[f32; 2]], z: f32) -> Vec<Vec3> {
    plan.iter().map(|p| v3(p[0], p[1], z)).collect()
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING_DARK);
        b.frustum(
            v3(-8.0, 0.0, KEEL),
            v2(96.0, 12.0),
            v2(88.0, 18.0),
            20.0,
            v2(0.0, 0.0),
        );
        b.cuboid(v3(-36.0, 0.0, 12.0), v3(18.0, 46.0, 12.0));
        b.paint(PLATING);
        b.frustum(
            v3(41.0, 0.0, HEAD_LOW),
            v2(16.0, 84.0),
            v2(10.0, 78.0),
            HEAD_TOP - HEAD_LOW,
            v2(0.0, 0.0),
        );
        return;
    }
    hull(b);
    head(b);
    crown(b);
    b.mirror_y(|b| {
        nacelle(b, NOZZLES[1], -14.0, LIFT_JETS[1]);
        skid(b, -34.0, 24.0, 7.5, KEEL, 4.5);
    });
    b.mirror_y(|b| capital::lift_jet(b, Vec3::from(LIFT_JETS[3]), 0.45));
    lamp_fittings(b, &FIT.lamps);
}

/// The neck and engineering block: dark plating, pale armour on the block's flanks with
/// the owner's stripe, a pale deck, the bridge house on the neck, radiators aft.
fn hull(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&STATIONS.iter().map(ring).collect::<Vec<_>>(), true, true);
    let deck = |s: &[f32; 6]| {
        let [x, _, _, _, w, z] = *s;
        vec![
            v3(x, w - 1.0, z - 0.3),
            v3(x, w - 2.0, z + 0.8),
            v3(x, -(w - 2.0), z + 0.8),
            v3(x, -(w - 1.0), z - 0.3),
        ]
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &STATIONS[4..7].iter().map(deck).collect::<Vec<_>>(),
        true,
        true,
    );
    b.paint(ACCENT);
    b.loft(
        &STATIONS[1..4].iter().map(deck).collect::<Vec<_>>(),
        true,
        true,
    );
    b.mirror_y(|b| {
        let plate = |s: &[f32; 6], t0: f32, t1: f32, lift: f32| {
            vec![
                flank(s, t0, -0.2),
                flank(s, t1, -0.2),
                flank(s, t1, lift),
                flank(s, t0, lift),
            ]
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &STATIONS[4..7]
                .iter()
                .map(|s| plate(s, 0.1, 0.9, 0.8))
                .collect::<Vec<_>>(),
            true,
            true,
        );
        b.paint(TEAM);
        b.loft(
            &STATIONS[4..6]
                .iter()
                .map(|s| plate(s, 0.22, 0.32, 0.95))
                .collect::<Vec<_>>(),
            true,
            true,
        );
        // The neck's flank: a lit rail down the chine.
        if b.fine() {
            b.paint(GLOW_AMBER);
            b.beam(
                flank(&STATIONS[1], 0.02, 0.1),
                flank(&STATIONS[3], 0.02, 0.1),
                v2(0.3, 0.3),
                v2(0.3, 0.3),
            );
            b.paint(ACCENT);
            for x in [-16.0, -24.0, -32.0] {
                let s = [x, 0.0, 13.5, 9.0, 9.5, 24.8];
                b.beam(
                    flank(&s, 0.45, 0.9),
                    flank(&s, 0.7, 0.9),
                    v2(4.0, 0.25),
                    v2(4.0, 0.25),
                );
            }
        }
    });
    // The bridge: a low house on the neck, glazed round, behind the head.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(20.0, 0.0, 20.8),
        v2(14.0, 8.0),
        v2(10.0, 6.4),
        3.6,
        v2(-1.0, 0.0),
    );
    b.paint(GLASS);
    b.frustum(
        v3(20.2, 0.0, 22.4),
        v2(13.2, 7.6),
        v2(12.4, 7.1),
        1.0,
        v2(-0.3, 0.0),
    );
    // The bulkhead where the neck meets the block: a pale collar.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            ring(&[-7.4, 4.9, 8.6, 9.5, 6.0, 22.1]),
            ring(&[-9.0, 4.9, 8.6, 9.5, 6.0, 22.1]),
        ],
        true,
        true,
    );
    radiators(b, v3(-27.0, 0.0, 25.4), v2(16.0, 12.0));
    team_panel(b, v3(-38.0, 0.0, 24.5), v2(5.0, 6.0));
}

/// The array wing: dark, with a pale upper skin, array faces tiled down both leading
/// edges, and a drooping sensor pod at each tip.
fn head(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            plan_ring(&HEAD, HEAD_LOW),
            plan_ring(&HEAD, HEAD_EDGE),
            plan_ring(&inset(&HEAD, 0.93), HEAD_TOP),
        ],
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            plan_ring(&inset(&HEAD, 0.86), HEAD_TOP - 0.2),
            plan_ring(&inset(&HEAD, 0.84), HEAD_TOP + 0.4),
        ],
        true,
        true,
    );
    b.mirror_y(|b| {
        // Array face down the leading edge: a dark tiled strip standing proud.
        let (p0, p1) = (v2(HEAD[0][0], 1.5), v2(HEAD[1][0], HEAD[1][1] - 1.0));
        let along = (p1 - p0).normalize();
        let out = v2(along.y, -along.x);
        let strip = |p: Vec2| {
            let (a, c) = (p + out * 0.4, p - out * 0.4);
            vec![
                v3(c.x, c.y, HEAD_LOW + 0.5),
                v3(a.x, a.y, HEAD_LOW + 0.5),
                v3(a.x, a.y, HEAD_EDGE - 0.3),
                v3(c.x, c.y, HEAD_EDGE - 0.3),
            ]
        };
        b.paint(ACCENT);
        b.loft(&[strip(p0), strip(p1)], true, true);
        if b.fine() {
            b.paint(METAL);
            for k in 1..10 {
                let p = p0 + (p1 - p0) * (k as f32 / 10.0) + out * 0.45;
                b.cuboid(
                    v3(p.x, p.y, (HEAD_LOW + HEAD_EDGE) * 0.5),
                    v3(0.25, 0.25, HEAD_EDGE - HEAD_LOW - 1.0),
                );
            }
            // The owner's stripe across the upper skin near each tip.
            b.paint(TEAM);
            b.beam(
                v3(42.8, 30.0, HEAD_TOP + 0.45),
                v3(37.2, 30.0, HEAD_TOP + 0.45),
                v2(2.0, 0.12),
                v2(2.0, 0.12),
            );
        }
        // The tip pod, drooping under the wing, and a fin down from it.
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.spheroid(
            v3(41.5, 41.8, HEAD_LOW + 0.6),
            v3(7.5, 2.4, 3.0),
            b.sides(12),
            4,
        );
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(
            v3(40.5, 41.8, HEAD_LOW - 7.0),
            v2(4.0, 0.8),
            v2(8.0, 1.2),
            5.6,
            v2(1.5, 0.0),
        );
    });
}

/// The crown turret: a squat drum on the head's crown with a dark window, turning to
/// look about (`set_spinner_scan`).
fn crown(b: &mut MeshBuilder) {
    b.paint(METAL);
    b.prism(CROWN - v3(0.0, 0.0, 1.2), b.sides(12), 5.4, 5.0, 1.4);
    b.set_spinner_pivot(CROWN);
    b.set_spinner_scan();
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.prism(CROWN, b.sides(12), 4.8, 4.4, 3.6);
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.prism(CROWN + v3(0.0, 0.0, 3.6), b.sides(12), 4.4, 3.2, 1.0);
        b.paint(ACCENT);
        b.cuboid(CROWN + v3(4.2, 0.0, 2.0), v3(1.4, 6.4, 1.8));
        b.paint(GLASS);
        b.cuboid(CROWN + v3(4.9, 0.0, 2.0), v3(0.2, 5.6, 1.2));
        if b.fine() {
            b.paint(METAL);
            b.beam(
                CROWN + v3(-1.5, 0.0, 4.6),
                CROWN + v3(-1.8, 0.0, 9.0),
                v2(0.3, 0.3),
                v2(0.15, 0.15),
            );
            b.paint(GLOW_RED);
            b.cuboid(CROWN + v3(-1.8, 0.0, 9.1), v3(0.4, 0.4, 0.4));
        }
    });
}

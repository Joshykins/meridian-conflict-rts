//! C: the truss ship. An open lattice spine between a heavy reactor block aft (radiator
//! wings, the two drive nacelles) and a forward sensor module carrying a big tracking
//! dish on a yoke, tilted up and swinging to look about. The crew pod hangs under the
//! truss. It reads as built for the upper air, not as an aircraft.
use super::*;

const NOZZLES: [[f32; 3]; 2] = [[-56.0, -17.0, 12.0], [-56.0, 17.0, 12.0]];
const LIFT_JETS: [[f32; 3]; 4] = [
    [-30.0, -17.0, 3.6],
    [-30.0, 17.0, 3.6],
    [38.0, -4.0, 3.6],
    [38.0, 4.0, 3.6],
];

pub(super) static FIT: Fit = Fit {
    nozzles: NOZZLES,
    lift_jets: LIFT_JETS,
    lamps: CapitalLamps {
        floods: &[
            [46.0, 3.0, 5.4],
            [46.0, -3.0, 5.4],
            [-40.0, 17.0, 5.4],
            [-40.0, -17.0, 5.4],
        ],
        nav_port: [-33.0, 31.4, 23.0],
        nav_starboard: [-33.0, -31.4, 23.0],
        strobes: &[[51.4, 0.0, 12.0], [-45.0, 17.0, 19.0], [-45.0, -17.0, 19.0]],
        beacons: &[[-44.0, 0.0, 23.2]],
        hold: None,
    },
    rig: rig(NOZZLES, LIFT_JETS),
};

/// The truss's chords: lower pair at (±y, z), the top chord at z.
const CHORD_Y: f32 = 5.5;
const CHORD_LOW: f32 = 12.0;
const CHORD_TOP: f32 = 21.0;
const TRUSS_AFT: f32 = -22.0;
const TRUSS_FORE: f32 = 30.0;
/// Where the dish turns (the yoke's foot), and its trunnions' height over it.
const YOKE: Vec3 = Vec3::new(40.0, 0.0, 20.0);
const TRUNNION: f32 = 9.5;
const DISH_R: f32 = 12.0;

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING_DARK);
        b.cuboid(v3(-36.0, 0.0, 13.0), v3(28.0, 40.0, 18.0));
        b.cuboid(v3(4.0, 0.0, 16.0), v3(52.0, 10.0, 9.0));
        b.cuboid(v3(40.0, 0.0, 12.0), v3(20.0, 14.0, 14.0));
        b.paint(PLATING);
        b.prism(YOKE + v3(0.0, 0.0, 3.0), 6, DISH_R, DISH_R, 3.0);
        return;
    }
    reactor(b);
    b.mirror_y(|b| {
        nacelle(b, NOZZLES[1], -22.0, LIFT_JETS[1]);
        skid(b, -50.0, -24.0, 9.5, 4.0, 8.0);
        skid(b, 30.0, 48.0, 6.0, 5.0, 4.0);
    });
    truss(b);
    pod(b);
    module(b);
    dish(b);
    lamp_fittings(b, &FIT.lamps);
}

/// The reactor block: an armoured, chamfered box with a pale crown, radiator wings
/// standing out either side, and a radiator bank on top.
fn reactor(b: &mut MeshBuilder) {
    let section = |x: f32, w: f32, top: f32| {
        vec![
            v3(x, w - 4.0, 4.0),
            v3(x, w, 9.0),
            v3(x, w, top - 4.0),
            v3(x, w - 4.0, top),
            v3(x, -(w - 4.0), top),
            v3(x, -w, top - 4.0),
            v3(x, -w, 9.0),
            v3(x, -(w - 4.0), 4.0),
        ]
    };
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            section(-46.0, 11.0, 21.0),
            section(-44.0, 12.5, 22.5),
            section(-26.0, 13.5, 23.5),
            section(-20.0, 10.5, 21.0),
        ],
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.mirror_y(|b| {
        b.loft(
            &[-43.0, -27.0]
                .map(|x| {
                    vec![
                        v3(x, 13.4, 10.0),
                        v3(x, 14.2, 10.0),
                        v3(x, 14.2, 19.2),
                        v3(x, 13.4, 19.6),
                    ]
                })
                .to_vec(),
            true,
            true,
        );
        b.paint(TEAM);
        b.cuboid(v3(-35.0, 14.25, 17.5), v3(14.0, 0.2, 1.2));
        // Radiator wing: a thin dark panel out from the block's shoulder, lit ribs on it.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(
            v3(-33.0, 22.5, 21.6),
            v2(16.0, 17.0),
            v2(14.0, 17.0),
            0.8,
            v2(-1.0, 0.0),
        );
        b.paint(METAL);
        b.beam(
            v3(-33.0, 13.0, 18.0),
            v3(-33.0, 30.0, 21.8),
            v2(1.0, 1.0),
            v2(0.6, 0.6),
        );
        if b.fine() {
            b.paint(GLOW_ORANGE);
            for k in 0..5 {
                let x = -39.0 + k as f32 * 3.0;
                b.cuboid(v3(x, 22.5, 22.5), v3(0.4, 15.0, 0.12));
            }
        }
    });
    radiators(b, v3(-34.0, 0.0, 23.5), v2(14.0, 14.0));
    team_panel(b, v3(-22.4, 0.0, 20.8), v2(3.0, 5.0));
}

/// The open truss: three chords, a frame every eight metres, diagonal braces, and a cable
/// run through its middle.
fn truss(b: &mut MeshBuilder) {
    let (a, f) = (TRUSS_AFT, TRUSS_FORE);
    b.paint(METAL);
    for (y, z) in [
        (CHORD_Y, CHORD_LOW),
        (-CHORD_Y, CHORD_LOW),
        (0.0, CHORD_TOP),
    ] {
        b.cylinder_between(v3(a, y, z), v3(f, y, z), 0.9, 0.9, b.sides(8));
    }
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.cylinder_between(v3(a, 0.0, 15.5), v3(f, 0.0, 15.5), 1.6, 1.6, b.sides(10));
    let frames = 7;
    for k in 0..=frames {
        let x = a + (f - a) * k as f32 / frames as f32;
        let (l, r, t) = (
            v3(x, CHORD_Y, CHORD_LOW),
            v3(x, -CHORD_Y, CHORD_LOW),
            v3(x, 0.0, CHORD_TOP),
        );
        b.paint(PLATING_DARK);
        for (p, q) in [(l, r), (r, t), (t, l)] {
            b.beam(p, q, v2(1.2, 1.2), v2(1.2, 1.2));
        }
        if b.fine() && k < frames {
            let n = a + (f - a) * (k + 1) as f32 / frames as f32;
            b.paint(METAL);
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            b.cylinder_between(l, v3(n, 0.0, CHORD_TOP), 0.35, 0.35, 5);
            b.cylinder_between(r, v3(n, 0.0, CHORD_TOP), 0.35, 0.35, 5);
            b.cylinder_between(
                v3(x, CHORD_Y * side, CHORD_LOW),
                v3(n, -CHORD_Y * side, CHORD_LOW),
                0.35,
                0.35,
                5,
            );
        }
    }
}

/// The crew pod slung under the truss's middle on two pylons, glazed round its bow.
fn pod(b: &mut MeshBuilder) {
    let z = 7.8;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    for x in [-2.0, 12.0] {
        b.beam(
            v3(x, 0.0, z + 3.0),
            v3(x, 0.0, CHORD_LOW),
            v2(3.0, 2.0),
            v2(3.0, 2.0),
        );
    }
    b.loft(
        &[
            octagon(20.0, 0.0, z, 2.6, 1.0),
            octagon(16.0, 0.0, z, 4.2, 1.6),
            octagon(-6.0, 0.0, z, 4.2, 1.6),
            octagon(-9.0, 0.0, z, 2.8, 1.0),
        ],
        true,
        true,
    );
    b.paint(GLASS);
    b.loft(
        &[
            octagon(18.6, 0.0, z, 3.6, 1.4),
            octagon(15.0, 0.0, z, 4.35, 1.6),
        ],
        true,
        true,
    );
    b.paint(TEAM);
    b.loft(
        &[
            octagon(4.0, 0.0, z, 4.35, 1.6),
            octagon(2.0, 0.0, z, 4.35, 1.6),
        ],
        true,
        true,
    );
}

/// The forward sensor module the dish stands on: an octagonal body with a chamfered nose,
/// the fore lift jets under it in fairings.
fn module(b: &mut MeshBuilder) {
    let z = 12.5;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            octagon(28.0, 0.0, z, 6.5, 2.4),
            octagon(46.0, 0.0, z, 7.5, 2.8),
            octagon(51.0, 0.0, z, 4.0, 1.6),
        ],
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(37.0, 0.0, z + 6.8),
        v2(18.0, 9.0),
        v2(16.0, 8.0),
        0.8,
        v2(0.0, 0.0),
    );
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.loft(
            &[46.2, 30.0]
                .map(|x| {
                    vec![
                        v3(x, 7.3, z - 2.0),
                        v3(x, 7.8, z - 2.0),
                        v3(x, 7.8, z + 2.0),
                        v3(x, 7.3, z + 2.0),
                    ]
                })
                .to_vec(),
            true,
            true,
        );
        let jet = Vec3::from(LIFT_JETS[3]);
        b.paint(PLATING_DARK);
        b.frustum(
            jet + v3(0.0, 0.0, 1.0),
            v2(6.0, 5.0),
            v2(8.0, 6.0),
            z - 5.5 - jet.z,
            v2(0.0, 0.0),
        );
        capital::lift_jet(b, jet, 0.45);
    });
}

/// The tracking dish: a turntable, a yoke of two arms, and a deep dish tilted up between
/// them with a feed on three struts. The yoke and dish swing together to look about.
fn dish(b: &mut MeshBuilder) {
    b.paint(METAL);
    b.prism(YOKE - v3(0.0, 0.0, 0.6), b.sides(12), 5.0, 4.6, 1.0);
    b.set_spinner_pivot(YOKE);
    b.set_spinner_scan();
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.prism(YOKE + v3(0.0, 0.0, 0.4), b.sides(12), 4.6, 4.0, 1.6);
        b.mirror_y(|b| {
            b.beam(
                YOKE + v3(0.0, 3.0, 1.6),
                YOKE + v3(-0.5, DISH_R + 1.4, TRUNNION),
                v2(3.0, 1.6),
                v2(2.2, 1.4),
            );
            b.paint(METAL);
            b.cylinder_between(
                YOKE + v3(-0.5, DISH_R + 2.2, TRUNNION),
                YOKE + v3(-0.5, DISH_R - 0.4, TRUNNION),
                1.3,
                1.3,
                b.sides(10),
            );
            b.paint(PLATING_DARK);
        });
        let n = if b.fine() { 28 } else { 14 };
        b.pitched(YOKE + v3(-0.5, 0.0, TRUNNION), 0.55, |b| {
            // The bowl opens toward +x; its back is dark and ribbed.
            b.paint(PLATING).pattern(pattern::PLAIN);
            capital::lathe(
                b,
                v3(-2.0, 0.0, 0.0),
                Vec3::X,
                &[
                    [0.0, 1.4],
                    [0.5, DISH_R * 0.42],
                    [1.5, DISH_R * 0.75],
                    [3.0, DISH_R],
                    [2.4, DISH_R + 0.4],
                    [1.0, DISH_R * 0.77],
                    [-0.1, DISH_R * 0.44],
                    [-0.6, 1.4],
                ],
                n,
            );
            b.paint(TEAM);
            capital::lathe(
                b,
                v3(-2.0, 0.0, 0.0),
                Vec3::X,
                &[
                    [2.7, DISH_R + 0.1],
                    [3.1, DISH_R + 0.1],
                    [2.6, DISH_R + 0.6],
                    [2.2, DISH_R + 0.6],
                ],
                n,
            );
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(-4.4, 0.0, 0.0),
                v3(-1.6, 0.0, 0.0),
                2.2,
                1.8,
                b.sides(10),
            );
            // The feed on its struts.
            b.paint(METAL);
            let feed = v3(7.5, 0.0, 0.0);
            for k in 0..3 {
                let a = k as f32 * std::f32::consts::TAU / 3.0 + 0.5;
                b.cylinder_between(
                    v3(0.8, a.cos() * DISH_R * 0.95, a.sin() * DISH_R * 0.95),
                    feed,
                    0.3,
                    0.25,
                    5,
                );
            }
            b.paint(ACCENT);
            b.cylinder_between(
                feed - v3(1.2, 0.0, 0.0),
                feed + v3(0.6, 0.0, 0.0),
                1.0,
                0.8,
                b.sides(8),
            );
            if b.fine() {
                b.paint(GLOW_RED);
                b.cuboid(feed + v3(0.8, 0.0, 0.0), v3(0.4, 0.4, 0.4));
            }
        });
    });
}

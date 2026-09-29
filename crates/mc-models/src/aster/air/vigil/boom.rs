//! B: the twin boom. Two slim hulls, a drive at each tail, joined by a ring wing; the
//! rotodome turns inside the ring, set into the wing rather than stood on it. A crew
//! gondola runs forward from the ring, and a tail bar carrying an array face joins the
//! booms between their fins.
use super::*;

/// Each boom's centre line: |y| and height.
const BOOM_Y: f32 = 22.0;
const BOOM_Z: f32 = 11.0;
const NOZZLES: [[f32; 3]; 2] = [[-56.0, -BOOM_Y, BOOM_Z], [-56.0, BOOM_Y, BOOM_Z]];
const LIFT_JETS: [[f32; 3]; 4] = [
    [-26.0, -BOOM_Y, 3.6],
    [-26.0, BOOM_Y, 3.6],
    [30.0, -BOOM_Y, 3.6],
    [30.0, BOOM_Y, 3.6],
];

pub(super) static FIT: Fit = Fit {
    nozzles: NOZZLES,
    lift_jets: LIFT_JETS,
    lamps: CapitalLamps {
        floods: &[
            [40.0, 2.0, 6.2],
            [40.0, -2.0, 6.2],
            [-40.0, BOOM_Y, 4.2],
            [-40.0, -BOOM_Y, 4.2],
        ],
        nav_port: [0.0, BOOM_Y + 7.4, BOOM_Z],
        nav_starboard: [0.0, -BOOM_Y - 7.4, BOOM_Z],
        strobes: &[
            [50.8, 0.0, 12.0],
            [-41.0, BOOM_Y, 26.8],
            [-41.0, -BOOM_Y, 26.8],
        ],
        beacons: &[[-23.5, 0.0, 19.6]],
        hold: None,
    },
    rig: rig(NOZZLES, LIFT_JETS),
};

/// The ring wing and the dome in it: centre, the ring's inner and outer radii, its top.
const RING: Vec3 = Vec3::new(-4.0, 0.0, 15.0);
const RING_IN: f32 = 17.0;
const RING_OUT: f32 = 23.0;
const RING_TOP: f32 = 19.0;
const DOME_R: f32 = 16.2;

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING_DARK);
        for y in [-BOOM_Y, BOOM_Y] {
            b.cuboid(v3(-2.0, y, BOOM_Z), v3(104.0, 12.0, 12.0));
        }
        b.paint(PLATING);
        b.prism(v3(RING.x, 0.0, 11.0), 6, RING_OUT, RING_OUT - 1.0, 8.0);
        b.cuboid(v3(32.0, 0.0, 12.0), v3(30.0, 10.0, 10.0));
        return;
    }
    b.mirror_y(|b| {
        boom(b);
        skid(b, -32.0, 36.0, BOOM_Y, BOOM_Z - 7.0, BOOM_Y);
        tail_fin(b);
    });
    ring(b);
    gondola(b);
    tail_bar(b);
    lamp_fittings(b, &FIT.lamps);
}

/// One boom (+y): a dark radome nose, the octagonal hull, a pale saddle with the owner's
/// band, the drive and both lift jets.
fn boom(b: &mut MeshBuilder) {
    let [nx, y, z] = NOZZLES[1];
    let aft = nx + 27.6 * DRIVE_SCALE + 0.4;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[
            octagon(53.0, y, z + 0.6, 1.0, 0.4),
            octagon(48.0, y, z + 0.3, 4.2, 1.5),
            octagon(44.0, y, z, 5.4, 2.0),
        ],
        true,
        false,
    );
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            octagon(44.0, y, z, 5.4, 2.0),
            octagon(34.0, y, z, 6.6, 2.4),
            octagon(-22.0, y, z, 7.0, 2.6),
            octagon(-38.0, y, z, 6.6, 2.4),
            octagon(aft + 1.5, y, z, 6.2, 2.2),
            octagon(aft, y, z, 5.7, 2.0),
        ],
        false,
        true,
    );
    b.paint(ACCENT);
    b.loft(
        &[octagon(44.2, y, z, 5.6, 2.0), octagon(43.4, y, z, 5.6, 2.0)],
        true,
        true,
    );
    const SADDLE: [[f32; 2]; 8] = [
        [-4.6, 7.4],
        [4.6, 7.4],
        [7.4, 4.6],
        [7.4, 0.0],
        [6.8, 0.0],
        [6.8, 4.2],
        [4.2, 6.8],
        [-4.2, 6.8],
    ];
    let saddle = |x: f32| {
        SADDLE
            .iter()
            .map(|&[dy, dz]| v3(x, y + dy, z + dz))
            .collect::<Vec<_>>()
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&[saddle(32.0), saddle(-36.0)], true, true);
    b.paint(TEAM);
    b.beam(
        v3(26.0, y - 4.4, z + 7.5),
        v3(26.0, y + 7.5, z + 4.4),
        v2(1.6, 0.2),
        v2(1.6, 0.2),
    );
    if b.fine() {
        b.paint(GLOW_AMBER);
        b.beam(
            v3(30.0, y + 7.1, z - 1.0),
            v3(-34.0, y + 7.1, z - 1.0),
            v2(0.3, 0.3),
            v2(0.3, 0.3),
        );
        b.paint(ACCENT);
        for x in [20.0, 8.0, -28.0] {
            b.cuboid(v3(x, y + 7.45, z + 2.0), v3(5.0, 0.2, 2.6));
        }
        b.paint(GLOW_RED);
        b.cuboid(v3(aft - 0.2, y + 5.0, z + 3.4), v3(0.6, 0.8, 0.8));
    }
    b.paint(ACCENT);
    b.loft(
        &[
            octagon(aft + 0.2, y, z, 5.9, 2.1),
            octagon(aft - 1.2, y, z, 5.4, 1.9),
        ],
        false,
        true,
    );
    capital::drive(b, Vec3::from(NOZZLES[1]), DRIVE_SCALE);
    for jet in [LIFT_JETS[1], LIFT_JETS[3]] {
        capital::lift_jet(b, Vec3::from(jet), 0.5);
    }
}

/// A swept fin standing on the boom's tail, the owner's colour at its tip.
fn tail_fin(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(-35.0, BOOM_Y, BOOM_Z + 5.8),
        v2(14.0, 1.8),
        v2(7.0, 1.2),
        9.2,
        v2(-4.5, 0.0),
    );
    b.paint(TEAM);
    b.frustum(
        v3(-39.5, BOOM_Y, BOOM_Z + 13.4),
        v2(7.6, 1.4),
        v2(7.0, 1.3),
        1.6,
        v2(-0.4, 0.0),
    );
}

/// The ring wing joining the booms, and the rotodome turning inside it.
fn ring(b: &mut MeshBuilder) {
    let n = if b.fine() { 32 } else { 16 };
    let top = v3(RING.x, 0.0, RING_TOP);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    capital::lathe(
        b,
        top,
        -Vec3::Z,
        &[
            [0.0, RING_IN],
            [0.0, RING_OUT - 1.5],
            [1.5, RING_OUT],
            [6.5, RING_OUT],
            [8.0, RING_OUT - 1.5],
            [8.0, RING_IN],
        ],
        n,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    capital::lathe(
        b,
        top,
        -Vec3::Z,
        &[
            [-0.4, RING_IN + 0.6],
            [-0.4, RING_OUT - 2.0],
            [0.1, RING_OUT - 1.8],
            [0.1, RING_IN + 0.4],
        ],
        n,
    );
    if b.fine() {
        // Lit seam round the ring's inner lip, where the dome turns.
        b.paint(GLOW_AMBER);
        capital::lathe(
            b,
            top,
            -Vec3::Z,
            &[
                [0.3, RING_IN - 0.15],
                [0.3, RING_IN + 0.15],
                [0.7, RING_IN + 0.15],
                [0.7, RING_IN - 0.15],
            ],
            n,
        );
    }
    let depth = 8.4;
    let dome_top = v3(RING.x, 0.0, RING_TOP + 1.8);
    let rim = depth * 0.45;
    b.set_spinner_pivot(RING);
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING).pattern(pattern::PLAIN);
        capital::lathe(
            b,
            dome_top,
            -Vec3::Z,
            &[
                [0.0, 1.2],
                [rim * 0.18, DOME_R * 0.5],
                [rim * 0.5, DOME_R * 0.8],
                [rim * 0.82, DOME_R * 0.97],
                [rim, DOME_R],
                [rim, 1.2],
            ],
            n,
        );
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        capital::lathe(
            b,
            dome_top,
            -Vec3::Z,
            &[
                [rim, 1.2],
                [rim, DOME_R],
                [depth * 0.7, DOME_R * 0.97],
                [depth * 0.9, DOME_R * 0.8],
                [depth, 1.2],
            ],
            n,
        );
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.spheroid(
                dome_top + v3(0.0, DOME_R * 0.6, -0.9),
                v3(2.8, 3.4, 1.0),
                b.sides(10),
                3,
            );
        });
        b.paint(TEAM);
        b.beam(
            dome_top + v3(-DOME_R * 0.55, 0.0, -0.8),
            dome_top + v3(-DOME_R * 0.15, 0.0, -0.1),
            v2(2.2, 0.2),
            v2(2.2, 0.2),
        );
    });
}

/// The crew gondola: an octagonal pod running forward from the ring, glazed at the front.
fn gondola(b: &mut MeshBuilder) {
    let z = 12.0;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            octagon(51.0, 0.0, z, 1.2, 0.4),
            octagon(45.0, 0.0, z, 4.4, 1.6),
            octagon(34.0, 0.0, z, 5.8, 2.2),
            octagon(18.0, 0.0, z, 6.0, 2.2),
            octagon(14.0, 0.0, z, 5.2, 2.0),
        ],
        true,
        true,
    );
    b.paint(GLASS);
    b.loft(
        &[
            vec![
                v3(47.0, 2.0, z + 2.9),
                v3(47.0, -2.0, z + 2.9),
                v3(47.0, -2.0, z + 3.3),
                v3(47.0, 2.0, z + 3.3),
            ],
            vec![
                v3(40.0, 4.2, z + 5.0),
                v3(40.0, -4.2, z + 5.0),
                v3(40.0, -3.6, z + 6.2),
                v3(40.0, 3.6, z + 6.2),
            ],
        ],
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(27.0, 0.0, z + 5.2),
        v2(22.0, 7.0),
        v2(19.0, 5.4),
        1.2,
        v2(0.0, 0.0),
    );
    team_panel(b, v3(27.0, 0.0, z + 6.45), v2(4.0, 3.0));
}

/// The tail bar between the booms: an array face along its trailing edge.
fn tail_bar(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(-36.0, 0.0, BOOM_Z + 4.0),
        v2(9.0, 2.0 * BOOM_Y),
        v2(6.0, 2.0 * BOOM_Y),
        2.4,
        v2(-1.0, 0.0),
    );
    b.paint(ACCENT);
    b.cuboid(
        v3(-40.6, 0.0, BOOM_Z + 5.2),
        v3(0.5, 2.0 * BOOM_Y - 12.0, 1.8),
    );
}

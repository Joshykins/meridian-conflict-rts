//! B: the eyes, the hammerhead shark. A slim round fuselage carries a thin blade across
//! its bow, and at each end of the blade a big sensor eye looks forward and out. An
//! aperture band runs along the blade's leading edge, a flush disc radome turns on the
//! spine, and a V tail and a keel fin stand over the single drive.
use super::*;

const NOZZLE: [f32; 3] = [-36.0, 0.0, 9.8];
const LIFT_JETS: [[f32; 3]; 4] = [
    [-12.0, -2.2, 5.0],
    [-12.0, 2.2, 5.0],
    [12.0, -2.2, 5.0],
    [12.0, 2.2, 5.0],
];

pub(super) static FIT: Fit = Fit {
    nozzles: [NOZZLE],
    lift_jets: LIFT_JETS,
    lamps: CapitalLamps {
        floods: &[
            [24.0, 2.0, 5.8],
            [24.0, -2.0, 5.8],
            [-17.0, 4.0, 5.0],
            [-17.0, -4.0, 5.0],
        ],
        nav_port: [21.0, 29.8, 10.4],
        nav_starboard: [21.0, -29.8, 10.4],
        strobes: &[[33.4, 0.0, 10.0], [-20.0, 7.0, 20.4], [-20.0, -7.0, 20.4]],
        beacons: &[[-8.5, 0.0, 16.4]],
        hold: None,
    },
    rig: rig(NOZZLE, LIFT_JETS),
};

/// The fuselage: x, half span, thickness, centre height.
const STATIONS: [[f32; 4]; 6] = [
    [33.0, 1.0, 1.0, 10.0],
    [29.0, 4.5, 5.5, 10.0],
    [20.0, 6.5, 8.5, 10.2],
    [4.0, 7.2, 10.0, 10.4],
    [-10.0, 6.8, 9.6, 10.2],
    [-20.0, 6.0, 9.4, 9.8],
];
/// The blade across the bow: y, then its chord (x0, x1) and thickness there.
const BLADE: [[f32; 4]; 5] = [
    [-24.0, 18.0, 25.0, 1.8],
    [-12.0, 19.0, 27.5, 2.4],
    [0.0, 20.0, 29.0, 3.0],
    [12.0, 19.0, 27.5, 2.4],
    [24.0, 18.0, 25.0, 1.8],
];
const BLADE_Z: f32 = 10.4;
/// Each eye's centre (+y), and where its lens faces.
const EYE: Vec3 = Vec3::new(22.0, 26.5, BLADE_Z);
/// The disc radome's centre on the spine.
const DISC: Vec3 = Vec3::new(0.0, 0.0, 16.0);

fn foil(s: &[f32; 4]) -> Vec<Vec3> {
    let [y, x0, x1, t] = *s;
    let c = x1 - x0;
    let z = BLADE_Z;
    vec![
        v3(x1, y, z),
        v3(x1 - 0.25 * c, y, z + 0.5 * t),
        v3(x0 + 0.35 * c, y, z + 0.5 * t),
        v3(x0, y, z + 0.1 * t),
        v3(x0 + 0.35 * c, y, z - 0.35 * t),
        v3(x1 - 0.25 * c, y, z - 0.35 * t),
    ]
}

/// On blade station `i`'s leading slope, a little above the edge.
fn lip(i: usize) -> Vec3 {
    let [y, x0, x1, t] = BLADE[i];
    v3(x1 - 0.12 * (x1 - x0) + 0.1, y, BLADE_Z + 0.26 * t + 0.08)
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid(v3(-4.0, 0.0, 10.2), v3(56.0, 13.0, 9.0));
        b.cuboid(v3(23.0, 0.0, BLADE_Z), v3(9.0, 60.0, 3.0));
        b.paint(PLATING_DARK);
        b.cuboid(v3(-26.0, 0.0, 9.8), v3(12.0, 10.0, 10.0));
        return;
    }
    body(b, &STATIONS);
    stern(b, NOZZLE, -20.0, 5.0);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&BLADE.iter().map(foil).collect::<Vec<_>>(), true, true);
    for k in 1..3 {
        aperture(b, lip(k), lip(k + 1), v2(0.4, 0.5), v3(1.0, 0.0, 0.6));
    }
    b.mirror_y(|b| {
        aperture(b, lip(3), lip(4), v2(0.4, 0.5), v3(1.0, 0.0, 0.6));
        eye(b);
        skid(b, -16.0, 12.0, 5.0, 5.4, 3.5);
    });
    aperture(b, lip(0), lip(1), v2(0.4, 0.5), v3(1.0, 0.0, 0.6));
    // Canopy.
    b.paint(PLATING_DARK);
    b.spheroid(v3(17.5, 0.0, 15.0), v3(5.6, 3.0, 1.7), b.sides(12), 4);
    b.paint(GLASS);
    b.spheroid(v3(17.9, 0.0, 15.3), v3(4.8, 2.5, 1.6), b.sides(12), 4);
    // V tail and keel fin.
    let c = v3(-14.0, 0.0, 12.0);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    for a in [50.0_f32, 130.0] {
        capital::fin(b, c, a.to_radians(), -6.0, 6.0, 3.0, [11.0, 5.0], 0.8);
    }
    capital::fin(
        b,
        c,
        -90.0_f32.to_radians(),
        -5.0,
        4.0,
        3.0,
        [7.4, 4.0],
        0.8,
    );
    b.paint(TEAM);
    for a in [50.0_f32, 130.0] {
        capital::fin(b, c, a.to_radians(), -6.0, -3.0, 9.0, [11.2, 9.6], 0.9);
    }
    disc(b);
    lift_jets(b, &LIFT_JETS);
    lamp_fittings(b, &FIT.lamps);
}

/// A sensor eye at the blade's tip (+y): a dark pod, a bezel, and a big lens of visor
/// glass looking forward and out.
fn eye(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.spheroid(EYE, v3(5.8, 3.1, 3.1), b.sides(14), 5);
    let look = v3(0.85, 0.5, 0.0).normalize();
    let face = EYE + v3(3.7, 1.1, 0.0);
    b.paint(ACCENT);
    b.cylinder_between(face - look * 0.6, face + look * 0.3, 2.5, 2.3, b.sides(14));
    b.paint(VISOR);
    b.spheroid(face + look * 0.4, v3(1.0, 2.1, 2.1), b.sides(14), 4);
    if b.fine() {
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.beam(
            EYE + v3(-5.0, 0.0, 2.2),
            EYE + v3(1.0, 0.0, 2.9),
            v2(1.4, 0.3),
            v2(1.4, 0.3),
        );
    }
}

/// The flush disc radome on the spine: a pale lens turning, two visor bars across it so
/// its turning shows.
fn disc(b: &mut MeshBuilder) {
    let n = if b.fine() { 24 } else { 12 };
    b.paint(METAL);
    b.prism(DISC - v3(0.0, 0.0, 1.0), b.sides(12), 3.0, 2.6, 1.2);
    b.set_spinner_pivot(DISC);
    b.with_part(part::SPINNER, |b| {
        let top = DISC + v3(0.0, 0.0, 1.4);
        b.paint(PLATING).pattern(pattern::PLAIN);
        capital::lathe(
            b,
            top,
            -Vec3::Z,
            &[
                [0.0, 0.8],
                [0.3, 3.4],
                [0.8, 5.2],
                [1.1, 5.4],
                [1.5, 4.6],
                [1.8, 0.8],
            ],
            n,
        );
        b.paint(VISOR);
        b.mirror_y(|b| {
            b.beam(
                top + v3(-1.2, 1.0, -0.2),
                top + v3(-1.2, 4.6, -0.75),
                v2(0.7, 0.2),
                v2(0.7, 0.2),
            );
        });
    });
}

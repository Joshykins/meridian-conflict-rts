//! Shrike: the cheap tech 1 gun drone. A graphite fuselage with an intake hump on its
//! back, a plain straight white wing set square across it, a V-tail, and twin cannon
//! run out of blisters on the nose cheeks. No canopy: it is unmanned. Tech 1, so
//! nothing on it is lit and the bores are dark.
use super::*;

/// The twin cannon's muzzles (the blueprint's): either side of the nose.
const MUZZLE: Vec3 = Vec3::new(3.55, 0.22, 1.12);
/// Where the exhausts end (`models::aircraft_exhausts`).
const NOZZLE: Vec3 = Vec3::new(-3.31, 0.2, 0.9);

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder, spine.
const HULL: [[f32; 9]; 6] = [
    [3.05, 0.04, 1.02, 0.08, 1.08, 0.06, 1.13, 0.02, 1.15],
    [2.2, 0.2, 0.72, 0.42, 0.92, 0.36, 1.24, 0.12, 1.34],
    [0.6, 0.3, 0.52, 0.56, 0.86, 0.48, 1.32, 0.18, 1.44],
    [-1.6, 0.3, 0.52, 0.54, 0.84, 0.46, 1.28, 0.18, 1.4],
    [-2.9, 0.22, 0.66, 0.42, 0.84, 0.36, 1.12, 0.14, 1.2],
    [-3.15, 0.2, 0.7, 0.4, 0.84, 0.34, 1.06, 0.12, 1.12],
];
/// Straight wing with cropped, slightly raked tips.
const WING: [[f32; 2]; 6] = [
    [0.75, 0.4],
    [0.52, 3.2],
    [0.3, 3.42],
    [-0.72, 3.42],
    [-0.95, 3.2],
    [-1.1, 0.4],
];
/// One V-tail fin in side view, standing on its root line.
const FIN: [[f32; 2]; 4] = [[-3.15, 0.0], [-1.85, 0.0], [-2.8, 1.05], [-3.35, 1.05]];
const FIN_ROOT: (f32, f32) = (0.16, 1.18);
const FIN_CANT: f32 = 0.72;

pub(super) fn build(b: &mut MeshBuilder) {
    b.set_turret_pivot(v3(0.0, 0.0, 1.05));
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    // Graphite fuselage: the dark frame the white wing is laid across.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 0, 3), true, true);

    // The intake hump on its back, mouth raked forward, running down into the tail.
    let hump = |x_top: f32, x_low: f32, w_low: f32, w_top: f32, z_low: f32, z_top: f32| {
        vec![
            v3(x_low, -w_low, z_low),
            v3(x_low, w_low, z_low),
            v3(x_top, w_top, z_top),
            v3(x_top, -w_top, z_top),
        ]
    };
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            hump(0.95, 0.62, 0.3, 0.34, 1.3, 1.64),
            hump(-0.2, -0.2, 0.36, 0.38, 1.3, 1.7),
            hump(-2.3, -2.3, 0.22, 0.18, 1.2, 1.42),
            hump(-2.95, -2.95, 0.14, 0.1, 1.12, 1.22),
        ],
        true,
        true,
    );
    // The mouth, black inside a thin white lip.
    b.paint(ACCENT);
    b.loft(&[hump(0.97, 0.66, 0.24, 0.28, 1.35, 1.59), hump(0.9, 0.59, 0.24, 0.28, 1.35, 1.59)], true, true);
    if fine {
        b.paint(PLATING);
        b.beam(v3(1.0, 0.0, 1.66), v3(0.93, 0.0, 1.62), v2(0.8, 0.06), v2(0.8, 0.06));
    }
    team_panel(b, v3(-1.05, 0.0, 1.6), v2(1.1, 0.34));
    if fine {
        // Access hatches on the hump, a blade aerial behind them and one under the belly.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.plate(v3(-0.1, 0.0, 1.69), v2(0.5, 0.5), 0.04, 0.02);
        b.plate(v3(-1.8, 0.0, 1.49), v2(0.6, 0.3), 0.04, 0.02);
        b.paint(ACCENT);
        b.extrude_y(&[[-2.2, 1.45], [-1.95, 1.45], [-2.3, 1.85], [-2.42, 1.85]], -0.02, 0.02);
        b.extrude_y(&[[-0.6, 0.54], [-0.3, 0.54], [-0.7, 0.2], [-0.82, 0.2]], -0.02, 0.02);
    }

    // The unmanned nose: a sensor window where a canopy would be, and a chin turret.
    if fine {
        b.paint(GLASS);
        b.plate(v3(2.2, 0.0, 1.3), v2(0.5, 0.22), 0.05, 0.02);
        b.paint(ACCENT);
        b.spheroid(v3(2.1, 0.0, 0.68), v3(0.3, 0.17, 0.12), 8, 3);
    }

    b.mirror_y(|b| {
        // The wing: plain white, square across the body.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_z(&WING, 0.98, 1.1);
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        b.plate(v3(-0.2, 2.72, 1.1), v2(1.25, 0.5), 0.03, 0.01);
        if b.fine() {
            // Dark leading edge and a fence down each tip.
            b.paint(PLATING_DARK);
            b.beam(v3(0.74, 0.5, 1.04), v3(0.52, 3.18, 1.04), v2(0.16, 0.14), v2(0.16, 0.12));
            b.beam(v3(0.36, 3.45, 1.02), v3(-0.8, 3.45, 1.02), v2(0.08, 0.22), v2(0.08, 0.16));
            // A hardpoint under each wing, empty.
            b.beam(v3(0.2, 1.7, 0.98), v3(-0.6, 1.7, 0.98), v2(0.12, 0.14), v2(0.12, 0.1));
            // Flaperons along the trailing edge, and a strake where the wing meets the nose.
            b.extrude_z(&[[-0.82, 0.62], [-0.64, 3.0], [-0.95, 3.0], [-1.08, 0.62]], 1.0, 1.12);
            b.extrude_z(&[[1.9, 0.28], [0.8, 0.62], [0.2, 0.62], [0.2, 0.3]], 1.0, 1.06);
        }

        // V-tail, each fin canted well out.
        let cant = Affine3A::from_translation(v3(0.0, FIN_ROOT.0, FIN_ROOT.1))
            * Affine3A::from_rotation_x(-FIN_CANT);
        b.with(cant, |b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.extrude_y(&FIN, -0.05, 0.05);
            if b.fine() {
                b.paint(PLATING_DARK);
                b.extrude_y(&[[-3.33, 0.82], [-2.62, 0.82], [-2.8, 1.05], [-3.35, 1.05]], -0.06, 0.06);
            }
        });

        // Twin nozzles side by side in the tail.
        b.paint(METAL);
        let tail = NOZZLE.with_y(0.2);
        b.cylinder_between(tail + Vec3::X * 0.4, tail, 0.19, 0.16, b.sides(8));
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(tail, tail + Vec3::X * 0.01, 0.12, 0.12, 8);
            b.paint(PLATING_DARK);
            b.cylinder_between(tail + Vec3::X * 0.5, tail + Vec3::X * 0.3, 0.2, 0.2, 10);
        }
    });

    // The cannon, in blisters on the nose cheeks. They yaw a little with the turret.
    b.with_part(part::TURRET, |b| {
        b.mirror_y(|b| {
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(1.45, MUZZLE.y + 0.1, MUZZLE.z - 0.04),
                v3(2.9, MUZZLE.y + 0.02, MUZZLE.z),
                0.16,
                0.11,
                b.sides(6),
            );
            b.paint(METAL);
            b.cylinder_between(v3(2.8, MUZZLE.y, MUZZLE.z), MUZZLE, 0.065, 0.06, b.sides(6));
            if b.fine() {
                // Muzzle brake, and a slotted cooling jacket where the barrel leaves its blister.
                b.cylinder_between(MUZZLE - Vec3::X * 0.28, MUZZLE, 0.085, 0.085, 6);
                b.paint(ACCENT);
                b.cylinder_between(v3(2.85, MUZZLE.y, MUZZLE.z), v3(3.15, MUZZLE.y, MUZZLE.z), 0.09, 0.09, 8);
            }
        });
    });
}

/// Far away: the white cross of the wing on a dark body, the tail's V, the gun block.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[4]];
    b.paint(PLATING_DARK);
    b.loft(&band(&stations, 1, 2), true, true);
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.face(&WING.map(|p| v3(p[0], p[1], 1.1)));
        let cant = Affine3A::from_translation(v3(0.0, FIN_ROOT.0, FIN_ROOT.1))
            * Affine3A::from_rotation_x(-FIN_CANT);
        // Wound to face outboard and up.
        let mut fin = FIN.map(|p| v3(p[0], 0.0, p[1]));
        fin.reverse();
        b.with(cant, |b| b.face(&fin));
    });
    b.paint(TEAM);
    b.decal(v3(-1.0, 0.0, 1.32), v2(1.1, 0.34));
    b.with_part(part::TURRET, |b| {
        b.paint(METAL);
        b.beam(v3(2.4, 0.0, MUZZLE.z), v3(MUZZLE.x, 0.0, MUZZLE.z), v2(0.6, 0.14), v2(0.56, 0.12));
    });
}

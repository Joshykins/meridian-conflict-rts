//! Gannet: the tech 2 torpedo bomber, a sea-patrol drone. A long, deep fuselage,
//! white over a graphite belly, carries an inverted gull wing: the inner panels
//! dip down and out to a knuckle where a ducted-fan pod hangs, and the outer
//! panels climb away from it to long straight tips. Under the chin is a black
//! sonar blister; the tail runs out into a sensor stinger under a T-tail. A
//! torpedo hangs under each inner wing, its white warhead well out ahead of the
//! leading edge, and a rack of sonobuoy tubes stands on the back. Head on it is a
//! flattened W; from above, a long cross with the pods and torpedoes on it.
use super::*;

/// Nose of the (left) torpedo: the blueprint's muzzle.
const TORPEDO: Vec3 = Vec3::new(2.7, 1.45, 0.78);
/// Length and radius of a torpedo.
const TORPEDO_LENGTH: f32 = 3.4;
const TORPEDO_RADIUS: f32 = 0.2;
/// Where the fan pods' exhausts end (`models::aircraft_exhausts`).
const NOZZLE: Vec3 = Vec3::new(-1.95, 2.55, 0.62);

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder, spine.
/// Deep through the bay, tapering hard into the tail boom.
const HULL: [[f32; 9]; 7] = [
    [5.5, 0.04, 1.0, 0.1, 1.06, 0.08, 1.16, 0.02, 1.2],
    [4.4, 0.24, 0.66, 0.44, 0.92, 0.38, 1.36, 0.14, 1.5],
    [2.4, 0.36, 0.46, 0.58, 0.86, 0.5, 1.5, 0.2, 1.72],
    [-1.2, 0.36, 0.46, 0.58, 0.86, 0.5, 1.5, 0.2, 1.72],
    [-2.8, 0.22, 0.8, 0.38, 0.98, 0.32, 1.38, 0.12, 1.5],
    [-4.8, 0.1, 1.02, 0.18, 1.1, 0.15, 1.3, 0.06, 1.36],
    [-5.5, 0.06, 1.08, 0.1, 1.12, 0.08, 1.24, 0.03, 1.28],
];

/// Gull wing sections, root to tip: (y, z of the chord line, leading x, trailing x,
/// thickness). The root sits high on the shoulder, the knuckle low, the tip high again.
const WING: [(f32, f32, f32, f32, f32); 4] = [
    (0.45, 1.46, 1.05, -1.25, 0.26),
    (2.55, 0.92, 0.95, -1.15, 0.22),
    (5.8, 1.48, 0.62, -0.72, 0.13),
    (6.8, 1.62, 0.44, -0.46, 0.1),
];
/// The fin in side view, on the tail boom.
const FIN: [[f32; 2]; 4] = [[-5.45, 1.25], [-3.7, 1.35], [-4.9, 3.0], [-5.55, 3.0]];
/// T-tail stabiliser in plan, on top of the fin.
const STABILISER: [[f32; 2]; 4] = [[-4.85, 0.0], [-5.2, 2.05], [-5.7, 2.05], [-5.62, 0.0]];

/// A wing section at `(y, z, lead, trail, thick)`: a slim lens round the chord.
fn section(s: (f32, f32, f32, f32, f32)) -> Vec<Vec3> {
    let (y, z, lead, trail, t) = s;
    let chord = lead - trail;
    vec![
        v3(lead, y, z),
        v3(lead - chord * 0.3, y, z + t * 0.5),
        v3(trail, y, z + t * 0.12),
        v3(trail, y, z - t * 0.08),
        v3(lead - chord * 0.3, y, z - t * 0.5),
    ]
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    // White upper hull over a graphite belly.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 1, 3), true, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 0, 1), true, true);
    team_panel(b, v3(-2.0, 0.0, 1.62), v2(1.4, 0.3));

    // The sonar blister under the chin: black, with a gunmetal collar.
    b.paint(ACCENT);
    b.spheroid(v3(3.5, 0.0, 0.5), v3(1.25, 0.46, 0.34), b.sides(12), if fine { 6 } else { 3 });
    if fine {
        b.paint(METAL);
        b.cylinder_between(v3(2.35, 0.0, 0.62), v3(2.25, 0.0, 0.62), 0.34, 0.34, 10);
        // Sensor window high on the nose, and a pitot probe out ahead.
        b.paint(GLASS);
        b.plate(v3(4.0, 0.0, 1.43), v2(0.8, 0.34), 0.05, 0.02);
        b.paint(METAL);
        b.cylinder_between(v3(5.45, 0.0, 1.08), v3(6.3, 0.0, 1.08), 0.04, 0.015, 5);
    }

    // The sensor stinger out of the tail: a long dark boom with a pale tip.
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(-5.5, 0.0, 1.17), v3(-7.1, 0.0, 1.17), 0.14, 0.1, b.sides(8));
    b.paint(PLATING);
    b.cylinder_between(v3(-7.1, 0.0, 1.17), v3(-7.5, 0.0, 1.17), 0.1, 0.03, b.sides(8));

    // Fin and T-tail.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_y(&FIN, -0.07, 0.07);
    b.paint(PLATING_DARK);
    b.extrude_y(&[[-5.52, 2.55], [-4.6, 2.55], [-4.9, 3.0], [-5.55, 3.0]], -0.08, 0.08);

    // The sonobuoy rack on the back: a block of launch tubes, their caps dark.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.plate(v3(-0.4, 0.0, 1.7), v2(1.6, 0.56), 0.1, 0.03);
    if fine {
        b.paint(ACCENT);
        for i in 0..4 {
            let x = -1.0 + i as f32 * 0.4;
            for y in [-0.14, 0.14] {
                b.cylinder_between(v3(x, y, 1.8), v3(x, y, 1.86), 0.09, 0.09, 6);
            }
        }
    }

    b.mirror_y(|b| {
        // Gull wing: the inner panel dips to the knuckle, the outer climbs to the tip.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(&WING.iter().map(|&s| section(s)).collect::<Vec<_>>(), true, true);
        // Dark leading edges and the owner's colour out on the tips.
        if b.fine() {
            b.paint(PLATING_DARK);
            b.beam(
                v3(WING[1].2 + 0.02, WING[1].0, WING[1].1),
                v3(WING[2].2 + 0.02, WING[2].0, WING[2].1),
                v2(0.16, 0.12),
                v2(0.12, 0.08),
            );
        }
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        b.beam(
            v3(0.1, 6.35, 1.57),
            v3(0.1, 6.75, 1.63),
            v2(0.95, 0.11),
            v2(0.85, 0.1),
        );
        // Stabiliser across the top of the fin.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_z(&STABILISER, 2.94, 3.04);

        // The ducted-fan pod under the knuckle: a white cowl ring round a dark fan.
        let pod = v3(0.0, WING[1].0, WING[1].1 - 0.3);
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.cylinder_between(pod + Vec3::X * 1.35, pod + Vec3::X * -1.6, 0.44, 0.34, b.sides(12));
        b.paint(METAL);
        b.cylinder_between(NOZZLE + Vec3::X * 0.4, NOZZLE, 0.33, 0.27, b.sides(10));
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(pod + Vec3::X * 1.37, pod + Vec3::X * 1.2, 0.34, 0.34, 10);
            b.paint(ACCENT);
            b.cylinder_between(NOZZLE, NOZZLE + Vec3::X * 0.01, 0.2, 0.2, 8);
            // Pylon from the knuckle down onto the cowl.
            b.paint(PLATING_DARK);
            b.beam(pod + v3(0.6, 0.0, 0.3), pod + v3(-0.8, 0.0, 0.3), v2(0.08, 0.36), v2(0.08, 0.36));
        }

        // A torpedo slung under the inner wing, nose well out ahead of it.
        let tail = TORPEDO - Vec3::X * TORPEDO_LENGTH;
        let head = TORPEDO - Vec3::X * 0.55;
        b.paint(PLATING_DARK);
        b.cylinder_between(tail + Vec3::X * 0.35, head, TORPEDO_RADIUS, TORPEDO_RADIUS, b.sides(8));
        b.paint(PLATING);
        b.cylinder_between(head, TORPEDO, TORPEDO_RADIUS, 0.06, b.sides(8));
        if b.fine() {
            b.paint(METAL);
            b.cylinder_between(tail, tail + Vec3::X * 0.35, 0.08, TORPEDO_RADIUS, 8);
            // Tail fins, and a band in the owner's colour behind the warhead.
            for k in 0..4 {
                let a = (k as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
                let out = v3(0.0, a.cos(), a.sin());
                let root = tail + Vec3::X * 0.2 + out * 0.1;
                b.beam(root, root + out * 0.2, v2(0.03, 0.3), v2(0.03, 0.2));
            }
            b.paint(TEAM);
            b.cylinder_between(head - Vec3::X * 0.3, head - Vec3::X * 0.15, 0.205, 0.205, 8);
        }
        // The pylon up into the wing.
        b.paint(PLATING_DARK);
        b.extrude_y(
            &[[0.5, TORPEDO.z + 0.1], [-0.7, TORPEDO.z + 0.1], [-0.9, 1.3], [0.8, 1.3]],
            TORPEDO.y - 0.05,
            TORPEDO.y + 0.05,
        );
    });
}

/// Far away: the hull, the W of the wing with its pods, the T-tail.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[4], HULL[6]];
    b.paint(PLATING);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        // The two wing panels, flat, and the tailplane.
        let edge = |s: (f32, f32, f32, f32, f32)| (v3(s.2, s.0, s.1), v3(s.3, s.0, s.1));
        let [(l0, t0), (l1, t1), (l3, t3)] = [edge(WING[0]), edge(WING[1]), edge(WING[3])];
        b.face(&[l0, t0, t1, l1]);
        b.face(&[l1, t1, t3, l3]);
        b.face(&STABILISER.map(|p| v3(p[0], p[1], 3.0)));
        // The fan pod, seen from above.
        b.paint(PLATING_DARK);
        let y = WING[1].0;
        b.face(&[v3(1.35, y - 0.4, 0.9), v3(1.35, y + 0.4, 0.9), v3(-1.9, y + 0.3, 0.9), v3(-1.9, y - 0.3, 0.9)]);
        // The torpedo, out ahead of the wing.
        let (x0, x1, t) = (TORPEDO.x - TORPEDO_LENGTH, TORPEDO.x, TORPEDO.y);
        b.face(&[v3(x1, t - 0.2, 1.0), v3(x1, t + 0.2, 1.0), v3(x0, t + 0.2, 1.0), v3(x0, t - 0.2, 1.0)]);
    });
    b.paint(PLATING);
    b.face(&FIN.map(|p| v3(p[0], 0.0, p[1])));
    b.face(&FIN.map(|p| v3(p[0], 0.0, p[1])).into_iter().rev().collect::<Vec<_>>());
    b.paint(TEAM);
    b.decal(v3(-2.0, 0.0, 1.62), v2(1.4, 0.3));
}

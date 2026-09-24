//! Peregrine: the tech 2 interceptor, built for speed. A long needle of a fuselage
//! behind a black radome and probe, two big box intakes feeding a wide engine block,
//! a small cropped delta set far aft, canards, twin canted fins, and an interceptor
//! missile on each wingtip rail, its seeker well out ahead of the wing: from above,
//! an arrow with a trident's prongs.
use super::*;

/// Seeker tip of the (left) wingtip missile: the blueprint's muzzle.
const MISSILE: Vec3 = Vec3::new(0.5, 3.33, 0.8);
/// Where the exhausts end (`models::aircraft_exhausts`).
const NOZZLE: Vec3 = Vec3::new(-5.11, 0.55, 0.9);

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder, spine.
/// The body swells from the needle into the engine block where the intakes join it.
const HULL: [[f32; 9]; 7] = [
    [5.7, 0.03, 1.02, 0.06, 1.06, 0.05, 1.1, 0.02, 1.12],
    [4.2, 0.2, 0.8, 0.34, 0.98, 0.3, 1.26, 0.12, 1.36],
    [2.2, 0.3, 0.66, 0.48, 0.92, 0.42, 1.4, 0.16, 1.54],
    [0.6, 0.34, 0.6, 0.52, 0.9, 0.46, 1.44, 0.18, 1.58],
    [-1.0, 0.95, 0.48, 1.15, 0.82, 1.0, 1.36, 0.36, 1.56],
    [-3.9, 0.95, 0.48, 1.12, 0.82, 1.0, 1.32, 0.36, 1.48],
    [-4.6, 0.85, 0.52, 1.0, 0.82, 0.9, 1.22, 0.34, 1.36],
];
/// Small cropped delta, well aft.
const WING: [[f32; 2]; 4] = [[0.4, 1.0], [-2.7, 3.3], [-3.75, 3.3], [-4.0, 1.0]];
const CANARD: [[f32; 2]; 4] = [[3.0, 0.36], [2.2, 1.28], [1.9, 1.28], [2.0, 0.36]];
const STABILATOR: [[f32; 2]; 4] = [[-3.9, 1.05], [-4.55, 2.2], [-5.2, 2.2], [-5.05, 1.05]];
/// One fin in side view, standing on its root line.
const FIN: [[f32; 2]; 4] = [[-4.75, 0.0], [-2.6, 0.0], [-4.05, 1.6], [-4.8, 1.6]];
const FIN_ROOT: (f32, f32) = (0.78, 1.42);
const FIN_CANT: f32 = 0.26;

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    // Black radome, white needle and engine block over a graphite belly.
    b.paint(PLATING_DARK);
    b.loft(&band(&HULL[..2], 0, 3), true, true);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[1..], 1, 3), true, true);
    b.paint(PLATING_DARK);
    b.loft(&band(&HULL[1..], 0, 1), true, true);
    // Heat-stained engine deck between the fins.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.plate(v3(-2.9, 0.0, 1.47), v2(2.6, 0.62), 0.05, 0.02);
    team_panel(b, v3(-0.2, 0.0, 1.55), v2(1.2, 0.26));
    if fine {
        // Pitot probe out of the radome, and the drone's sensor window behind it.
        b.paint(METAL);
        b.cylinder_between(v3(5.65, 0.0, 1.07), v3(6.6, 0.0, 1.07), 0.045, 0.015, 5);
        b.paint(GLASS);
        b.plate(v3(3.45, 0.0, 1.31), v2(0.9, 0.26), 0.05, 0.02);
    }

    b.mirror_y(|b| {
        // Box intake, its mouth raked back from the top lip; black inside.
        let (y0, y1) = (0.48, 1.14);
        let ring = |x_low: f32, x_top: f32, z_low: f32, z_top: f32, inset: f32| {
            vec![
                v3(x_low, y0 + inset, z_low + inset),
                v3(x_low, y1 - inset, z_low + inset),
                v3(x_top, y1 - inset, z_top - inset),
                v3(x_top, y0 + inset, z_top - inset),
            ]
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(&[ring(1.0, 1.55, 0.55, 1.3, 0.0), ring(-1.0, -1.0, 0.5, 1.32, 0.0)], false, true);
        b.paint(ACCENT);
        b.loft(&[ring(0.94, 1.49, 0.55, 1.3, 0.07), ring(0.5, 0.8, 0.55, 1.3, 0.07)], true, true);
        if b.fine() {
            // The splitter plate standing off the fuselage.
            b.paint(PLATING_DARK);
            b.extrude_y(&[[1.6, 1.32], [1.05, 0.52], [0.6, 0.52], [0.6, 1.32]], 0.42, 0.47);
        }

        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_z(&WING, 0.92, 1.04);
        b.extrude_z(&CANARD, 1.14, 1.2);
        b.extrude_z(&STABILATOR, 0.95, 1.03);
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        b.plate(v3(-2.9, 2.35, 1.04), v2(1.1, 0.55), 0.03, 0.01);
        if b.fine() {
            b.paint(PLATING_DARK);
            b.beam(v3(0.3, 1.1, 0.98), v3(-2.66, 3.22, 0.98), v2(0.2, 0.14), v2(0.18, 0.12));
        }

        let cant = Affine3A::from_translation(v3(0.0, FIN_ROOT.0, FIN_ROOT.1))
            * Affine3A::from_rotation_x(-FIN_CANT);
        b.with(cant, |b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.extrude_y(&FIN, -0.06, 0.06);
            if b.fine() {
                b.paint(PLATING_DARK);
                b.extrude_y(&[[-4.78, 1.3], [-3.8, 1.3], [-4.05, 1.6], [-4.8, 1.6]], -0.07, 0.07);
            }
        });
        // Ventral fin under each engine.
        b.paint(PLATING_DARK);
        b.extrude_y(&[[-4.45, 0.52], [-3.0, 0.52], [-3.95, 0.04], [-4.55, 0.04]], 0.82, 0.9);

        // Round nozzles: gunmetal cans, black inside.
        b.paint(METAL);
        b.cylinder_between(NOZZLE + Vec3::X * 0.6, NOZZLE, 0.42, 0.36, b.sides(10));
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(NOZZLE, NOZZLE + Vec3::X * 0.01, 0.28, 0.28, 8);
            b.paint(PLATING_DARK);
            b.cylinder_between(NOZZLE + Vec3::X * 0.75, NOZZLE + Vec3::X * 0.55, 0.45, 0.45, 10);
        }

        // Wingtip rail and its missile, nose far out ahead of the wing.
        b.paint(PLATING_DARK);
        b.beam(v3(-3.72, MISSILE.y, 0.98), v3(-2.2, MISSILE.y, 0.98), v2(0.1, 0.18), v2(0.1, 0.18));
        let (tail, body) = (MISSILE - Vec3::X * 3.8, MISSILE - Vec3::X * 0.42);
        b.paint(PLATING);
        b.cylinder_between(tail, body, 0.15, 0.15, b.sides(6));
        b.paint(ACCENT);
        b.cylinder_between(body, MISSILE, 0.15, 0.03, b.sides(6));
        if b.fine() {
            // Cruciform fins fore and aft, and a band in the owner's colour.
            for (x, chord, span) in [(tail.x + 0.1, 0.55, 0.3), (body.x - 0.7, 0.35, 0.18)] {
                for k in 0..4 {
                    let a = (k as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
                    let out = v3(0.0, a.cos(), a.sin());
                    let root = v3(x, MISSILE.y, MISSILE.z) + out * 0.12;
                    b.beam(root, root + out * span + Vec3::X * -0.15, v2(0.03, chord), v2(0.03, chord * 0.5));
                }
            }
            b.paint(TEAM);
            b.cylinder_between(body - Vec3::X * 0.35, body - Vec3::X * 0.2, 0.155, 0.155, 6);
        }
    });
}

/// Far away: the needle, the aft delta and the two long missiles either side.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[4], HULL[6]];
    b.paint(PLATING);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.face(&WING.map(|p| v3(p[0], p[1], 1.04)));
        b.paint(PLATING_DARK);
        b.beam(v3(-3.3, MISSILE.y, MISSILE.z), MISSILE, v2(0.26, 0.26), v2(0.1, 0.1));
    });
    b.paint(TEAM);
    b.decal(v3(-0.2, 0.0, 1.42), v2(1.2, 0.26));
}

//! Raptor: the tech 3 air superiority fighter. A wide, flat, hard-chined lifting body
//! in graphite with white armour let into it, forward-swept wings, big canards and
//! twin fins canted hard out. Its Plasma Lances run forward from the wing roots either
//! side of the nose like mandibles, ringed with blue field coils; a blue power run
//! down the spine feeds them. The one fighter that reads as an energy weapon.
use super::*;

/// Emitter tip of the (left) lance: the blueprint's muzzle.
const LANCE: Vec3 = Vec3::new(5.6, 2.15, 1.0);
/// Where the exhausts end (`models::aircraft_exhausts`).
const NOZZLE: Vec3 = Vec3::new(-6.52, 0.72, 1.02);

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder, spine.
const HULL: [[f32; 9]; 6] = [
    [6.3, 0.05, 1.12, 0.1, 1.18, 0.08, 1.24, 0.03, 1.27],
    [4.2, 0.35, 0.86, 0.95, 1.08, 0.55, 1.5, 0.2, 1.64],
    [1.5, 0.6, 0.64, 1.75, 0.98, 0.95, 1.72, 0.32, 1.92],
    [-2.4, 0.9, 0.56, 1.95, 0.96, 1.3, 1.7, 0.38, 1.86],
    [-5.0, 0.95, 0.66, 1.55, 0.96, 1.2, 1.5, 0.34, 1.58],
    [-5.7, 0.9, 0.7, 1.3, 0.96, 1.1, 1.42, 0.32, 1.48],
];
/// Forward-swept wing: the tip leads the root.
const WING: [[f32; 2]; 5] = [[-1.6, 1.7], [0.6, 6.2], [0.2, 6.7], [-1.1, 6.7], [-4.9, 1.9]];
const CANARD: [[f32; 2]; 4] = [[3.5, 0.95], [2.1, 3.25], [1.3, 3.25], [1.3, 1.4]];
const STABILATOR: [[f32; 2]; 4] = [[-4.5, 1.5], [-5.7, 3.3], [-6.6, 3.3], [-6.2, 1.5]];
/// One fin in side view, standing on its root line.
const FIN: [[f32; 2]; 4] = [[-6.0, 0.0], [-3.4, 0.0], [-5.0, 1.75], [-6.15, 1.75]];
const FIN_ROOT: (f32, f32) = (1.3, 1.5);
const FIN_CANT: f32 = 0.45;

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    // Graphite body; the forward deck is a white armour carapace, the aft deck carries
    // the power run.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 0, 2), true, true);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[..3], 2, 3), true, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[2..], 2, 3), true, true);
    glow_strip(b, v3(-1.9, 0.0, 1.86), v2(5.0, 0.1), GLOW);
    team_panel(b, v3(-3.1, 0.0, 1.78), v2(1.4, 0.44));
    if fine {
        // The sensor eye, a slit across the carapace's brow, lit from within.
        b.paint(GLASS);
        b.plate(v3(4.35, 0.0, 1.55), v2(0.35, 0.95), 0.05, 0.02);
        glow_strip(b, v3(4.35, 0.0, 1.58), v2(0.06, 0.7), GLOW);
        // Access panels either side of the power run.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        for (x, len) in [(-0.6, 1.6), (-3.0, 2.2)] {
            b.mirror_y(|b| b.plate(v3(x, 0.62, 1.74), v2(len, 0.4), 0.05, 0.02));
        }
    }

    b.mirror_y(|b| {
        // Dark wing and canard frames, each with a white armour plate let into it.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.extrude_z(&WING, 0.94, 1.12);
        b.extrude_z(&CANARD, 1.3, 1.4);
        b.extrude_z(&STABILATOR, 0.95, 1.06);
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_z(&inset(&WING, 0.78), 1.12, 1.16);
        b.extrude_z(&inset(&CANARD, 0.7), 1.4, 1.43);
        if b.fine() {
            b.extrude_z(&inset(&STABILATOR, 0.65), 1.06, 1.09);
            // A blue line down the wing's leading edge, and the owner's colour at the tip.
            b.paint(GLOW);
            b.beam(v3(-1.35, 2.05, 1.08), v3(0.4, 5.95, 1.08), v2(0.07, 0.06), v2(0.07, 0.06));
        }
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        b.plate(v3(-0.5, 5.6, 1.16), v2(1.3, 0.6), 0.03, 0.01);

        // Fins canted hard out, dark with white faces.
        let cant = Affine3A::from_translation(v3(0.0, FIN_ROOT.0, FIN_ROOT.1))
            * Affine3A::from_rotation_x(-FIN_CANT);
        b.with(cant, |b| {
            b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
            b.extrude_y(&FIN, -0.07, 0.07);
            if b.fine() {
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.extrude_y(&[[-5.75, 0.2], [-3.95, 0.2], [-5.05, 1.4], [-5.85, 1.4]], -0.09, 0.09);
            }
        });

        // Intake under each chine, black mouth.
        if b.fine() {
            b.paint(PLATING_DARK);
            b.loft(
                &[
                    vec![v3(3.0, 0.45, 0.62), v3(3.0, 1.35, 0.62), v3(3.3, 1.45, 0.98), v3(3.3, 0.45, 0.98)],
                    vec![v3(0.5, 0.6, 0.55), v3(0.5, 1.5, 0.55), v3(0.5, 1.6, 0.96), v3(0.5, 0.6, 0.96)],
                ],
                true,
                true,
            );
            b.paint(ACCENT);
            b.beam(v3(3.2, 0.93, 0.8), v3(3.06, 0.93, 0.8), v2(0.8, 0.28), v2(0.8, 0.28));
        }

        // Flat two-dimensional nozzles, blue-hot inside.
        b.paint(METAL);
        b.beam(NOZZLE + Vec3::X * 0.95, NOZZLE, v2(0.98, 0.6), v2(0.88, 0.48));
        b.paint(GLOW);
        b.beam(NOZZLE + Vec3::X * 0.02, NOZZLE - Vec3::X * 0.01, v2(0.66, 0.28), v2(0.66, 0.28));
        if b.fine() {
            // Nozzle flaps top and bottom, a ventral strake, a pod on the wingtip.
            b.paint(ACCENT);
            for s in [-1.0, 1.0] {
                b.beam(
                    NOZZLE + v3(0.35, 0.0, 0.3 * s),
                    NOZZLE + v3(-0.15, 0.0, 0.26 * s),
                    v2(1.0, 0.06),
                    v2(0.9, 0.05),
                );
            }
            b.paint(PLATING_DARK);
            b.extrude_y(&[[-5.4, 0.7], [-3.6, 0.7], [-4.7, 0.1], [-5.5, 0.1]], 1.02, 1.1);
            b.cylinder_between(v3(0.5, 6.72, 1.03), v3(-1.25, 6.72, 1.03), 0.1, 0.12, 8);
            // Canard pivot fairing.
            b.cylinder_between(v3(2.6, 1.28, 1.35), v3(1.3, 1.36, 1.35), 0.14, 0.1, 8);
        }

        lance(b);
    });
}

/// A Plasma Lance: a housing at the wing root, a long barrel out past the nose, field
/// coils along it and a forked emitter at its tip.
fn lance(b: &mut MeshBuilder) {
    let (y, z) = (LANCE.y, LANCE.z);
    let fine = b.fine();
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    if fine {
        b.chamfered_box(v3(-0.8, y, z), v3(3.2, 0.62, 0.56), 0.14);
    } else {
        b.cuboid(v3(-0.8, y, z), v3(3.2, 0.62, 0.56));
    }
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.plate(v3(-0.6, y, z + 0.28), v2(2.4, 0.46), 0.05, 0.02);
    glow_strip(b, v3(-0.6, y, z + 0.33), v2(1.8, 0.08), GLOW);
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(0.8, y, z), v3(2.3, y, z), 0.25, 0.19, b.sides(8));
    b.paint(METAL);
    b.cylinder_between(v3(2.3, y, z), LANCE - Vec3::X * 0.3, 0.12, 0.1, b.sides(8));
    b.paint(GLOW);
    for x in [2.9, 3.6, 4.3] {
        b.cylinder_between(v3(x, y, z), v3(x + 0.16, y, z), 0.2, 0.2, b.sides(8));
    }
    b.cylinder_between(LANCE - Vec3::X * 0.3, LANCE, 0.09, 0.06, b.sides(6));
    if fine {
        // Heat-sink fins down the housing's flank.
        b.paint(ACCENT);
        for k in 0..4 {
            let x = -1.9 + k as f32 * 0.42;
            b.beam(v3(x, y + 0.31, z - 0.18), v3(x, y + 0.31, z + 0.18), v2(0.1, 0.18), v2(0.1, 0.18));
        }
        // The fork: two dark tines above and below the emitter.
        b.paint(PLATING_DARK);
        for s in [-1.0, 1.0] {
            let off = Vec3::Z * 0.2 * s;
            b.beam(LANCE + off - Vec3::X * 1.0, LANCE + off + Vec3::X * 0.12, v2(0.1, 0.08), v2(0.06, 0.05));
        }
        b.paint(ACCENT);
        for x in [2.75, 3.45, 4.15, 4.62] {
            b.cylinder_between(v3(x, y, z), v3(x + 0.05, y, z), 0.16, 0.16, 8);
        }
    }
}

/// Far away: the dark lifting body, the forward-swept wing and the two lances.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[4]];
    b.paint(PLATING_DARK);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.face(&WING.map(|p| v3(p[0], p[1], 1.12)));
        b.face(&CANARD.map(|p| v3(p[0], p[1], 1.4)));
        b.paint(GLOW);
        b.beam(v3(-1.5, LANCE.y, LANCE.z), LANCE, v2(0.5, 0.4), v2(0.16, 0.16));
    });
    b.paint(TEAM);
    b.decal(v3(-3.1, 0.0, 1.66), v2(1.4, 0.44));
}

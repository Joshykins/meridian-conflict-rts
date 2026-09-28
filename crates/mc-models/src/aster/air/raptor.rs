//! Raptor: the tech 3 air superiority fighter. A slim, needle-nosed, hard-chined body
//! in graphite with white armour let into it, carried on a big forward-swept wing that
//! grows out of long knife-edge strakes, with canards and twin fins canted hard out: far
//! more wing than body. Its two light electric bores run forward from under the wing
//! roots, tight along the chines like mandibles: slim AEB barrels (`armored_bore`) whose
//! induction rings are the only light on it; the airframe itself does not glow. Drawn
//! 1.4x the size it is authored at (the blueprint's radius).
use super::*;

/// Muzzle of the (left) bore: the blueprint's muzzle over 1.4.
const LANCE: Vec3 = Vec3::new(5.6, 1.3, 0.8);
/// Where the exhausts end (`models::aircraft_exhausts`).
const NOZZLE: Vec3 = Vec3::new(-6.52, 0.72, 1.02);

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder, spine.
const HULL: [[f32; 9]; 7] = [
    [7.6, 0.02, 1.15, 0.04, 1.18, 0.03, 1.21, 0.01, 1.22],
    [5.6, 0.2, 0.92, 0.46, 1.07, 0.3, 1.36, 0.1, 1.45],
    [3.2, 0.36, 0.74, 0.86, 1.02, 0.5, 1.58, 0.17, 1.72],
    [0.4, 0.46, 0.64, 1.1, 0.98, 0.6, 1.62, 0.21, 1.78],
    [-2.8, 0.56, 0.6, 1.18, 0.96, 0.74, 1.56, 0.25, 1.7],
    [-5.0, 0.66, 0.66, 1.12, 0.96, 0.84, 1.44, 0.28, 1.54],
    [-5.7, 0.66, 0.7, 1.02, 0.96, 0.8, 1.38, 0.28, 1.46],
];
/// Forward-swept wing: the tip leads the root.
const WING: [[f32; 2]; 5] = [
    [-1.0, 1.0],
    [1.0, 6.8],
    [0.7, 7.2],
    [-0.4, 7.2],
    [-4.4, 1.1],
];
/// Knife-edge strake running from the nose into the wing root.
const STRAKE: [[f32; 2]; 4] = [[5.0, 0.45], [1.0, 1.4], [-1.2, 1.4], [-1.2, 0.8]];
const CANARD: [[f32; 2]; 4] = [[4.2, 0.75], [2.8, 3.0], [2.1, 3.0], [2.0, 0.95]];
const STABILATOR: [[f32; 2]; 4] = [[-4.3, 0.95], [-5.6, 3.0], [-6.5, 3.0], [-6.1, 0.95]];
/// One fin in side view, standing on its root line.
const FIN: [[f32; 2]; 4] = [[-6.0, 0.0], [-3.6, 0.0], [-5.2, 1.95], [-6.2, 1.95]];
const FIN_ROOT: (f32, f32) = (0.8, 1.45);
const FIN_CANT: f32 = 0.45;

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    // Graphite body; the forward deck is a white armour carapace, the aft deck carries
    // a dark cable run to the railguns.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 0, 2), true, true);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[..4], 2, 3), true, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[3..], 2, 3), true, true);
    glow_strip(b, v3(-1.9, 0.0, 1.76), v2(5.0, 0.08), ACCENT);
    team_panel(b, v3(-3.4, 0.0, 1.69), v2(1.4, 0.3));
    if fine {
        // The sensor eye, a dark slit across the carapace's brow.
        b.paint(TREAD);
        b.plate(v3(4.6, 0.0, 1.62), v2(0.35, 0.6), 0.05, 0.02);
        // Access panels either side of the power run.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        for (x, len) in [(-0.6, 1.6), (-3.0, 2.2)] {
            b.mirror_y(|b| b.plate(v3(x, 0.44, 1.66), v2(len, 0.26), 0.05, 0.02));
        }
    }

    b.mirror_y(|b| {
        // Dark wing and canard frames, each with a white armour plate let into it.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.extrude_z(&WING, 0.96, 1.08);
        b.extrude_z(&STRAKE, 0.98, 1.05);
        b.extrude_z(&CANARD, 1.3, 1.38);
        b.extrude_z(&STABILATOR, 0.95, 1.06);
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_z(&inset(&WING, 0.78), 1.08, 1.11);
        b.extrude_z(&inset(&CANARD, 0.7), 1.38, 1.41);
        if b.fine() {
            b.extrude_z(&inset(&STABILATOR, 0.65), 1.06, 1.09);
            // A dark line down the wing's leading edge, and the owner's colour at the tip.
            b.paint(ACCENT);
            b.beam(
                v3(-0.8, 1.6, 1.05),
                v3(0.85, 6.5, 1.05),
                v2(0.07, 0.05),
                v2(0.07, 0.05),
            );
        }
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        b.plate(v3(-0.33, 5.8, 1.11), v2(1.1, 0.5), 0.03, 0.01);

        // Fins canted hard out, dark with white faces.
        let cant = Affine3A::from_translation(v3(0.0, FIN_ROOT.0, FIN_ROOT.1))
            * Affine3A::from_rotation_x(-FIN_CANT);
        b.with(cant, |b| {
            b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
            b.extrude_y(&FIN, -0.07, 0.07);
            if b.fine() {
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.extrude_y(
                    &[[-5.8, 0.2], [-4.1, 0.2], [-5.25, 1.6], [-5.9, 1.6]],
                    -0.09,
                    0.09,
                );
            }
        });

        // Intake under each chine, black mouth.
        if b.fine() {
            b.paint(PLATING_DARK);
            b.loft(
                &[
                    vec![
                        v3(3.0, 0.3, 0.66),
                        v3(3.0, 0.85, 0.66),
                        v3(3.3, 0.92, 0.98),
                        v3(3.3, 0.3, 0.98),
                    ],
                    vec![
                        v3(0.5, 0.4, 0.58),
                        v3(0.5, 1.02, 0.58),
                        v3(0.5, 1.08, 0.96),
                        v3(0.5, 0.4, 0.96),
                    ],
                ],
                true,
                true,
            );
            b.paint(ACCENT);
            b.beam(
                v3(3.2, 0.61, 0.82),
                v3(3.06, 0.61, 0.82),
                v2(0.52, 0.26),
                v2(0.52, 0.26),
            );
        }

        // Flat two-dimensional nozzles, black inside.
        b.paint(METAL);
        b.beam(
            NOZZLE + Vec3::X * 0.95,
            NOZZLE,
            v2(0.8, 0.56),
            v2(0.7, 0.46),
        );
        b.paint(ACCENT);
        b.beam(
            NOZZLE + Vec3::X * 0.02,
            NOZZLE - Vec3::X * 0.01,
            v2(0.52, 0.26),
            v2(0.52, 0.26),
        );
        if b.fine() {
            // Nozzle flaps top and bottom, a ventral strake, a pod on the wingtip.
            b.paint(ACCENT);
            for s in [-1.0, 1.0] {
                b.beam(
                    NOZZLE + v3(0.35, 0.0, 0.3 * s),
                    NOZZLE + v3(-0.15, 0.0, 0.26 * s),
                    v2(0.82, 0.06),
                    v2(0.72, 0.05),
                );
            }
            b.paint(PLATING_DARK);
            b.extrude_y(
                &[[-5.4, 0.7], [-3.6, 0.7], [-4.7, 0.1], [-5.5, 0.1]],
                0.94,
                1.0,
            );
            b.cylinder_between(v3(1.0, 7.22, 1.02), v3(-0.6, 7.22, 1.02), 0.08, 0.1, 8);
            // Canard pivot fairing.
            b.cylinder_between(v3(3.3, 0.9, 1.34), v3(2.0, 0.98, 1.34), 0.12, 0.09, 8);
        }

        lance(b);
    });
}

/// A wing bore: a slim dark fairing under the wing root with heat-sink fins down its
/// flank, and a light AEB barrel running out from it past the nose.
fn lance(b: &mut MeshBuilder) {
    let (y, z) = (LANCE.y, LANCE.z);
    let fine = b.fine();
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    if fine {
        b.chamfered_box(v3(-0.8, y, z), v3(3.2, 0.36, 0.34), 0.1);
    } else {
        b.cuboid(v3(-0.8, y, z), v3(3.2, 0.36, 0.34));
    }
    if fine {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.plate(v3(-0.6, y + 0.18, z), v2(2.4, 0.2), 0.04, 0.02);
        b.paint(ACCENT);
        for k in 0..4 {
            let x = -1.9 + k as f32 * 0.42;
            b.beam(
                v3(x, y + 0.18, z - 0.13),
                v3(x, y + 0.18, z + 0.13),
                v2(0.08, 0.1),
                v2(0.08, 0.1),
            );
        }
    }
    super::super::bore_tank::armored_bore(b, v3(0.6, y, z), LANCE, 0.2);
}

/// Far away: the dark lifting body, the forward-swept wing and the two lances.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[5]];
    b.paint(PLATING_DARK);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.face(&WING.map(|p| v3(p[0], p[1], 1.08)));
        b.face(&CANARD.map(|p| v3(p[0], p[1], 1.4)));
        b.paint(PLATING);
        b.beam(
            v3(-1.5, LANCE.y, LANCE.z),
            LANCE,
            v2(0.34, 0.3),
            v2(0.22, 0.18),
        );
    });
    b.paint(TEAM);
    b.decal(v3(-3.4, 0.0, 1.6), v2(1.4, 0.3));
}

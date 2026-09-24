//! Osprey: the tech 2 reclaim carrier, a heavy-lift drone that hangs low over a
//! wreck field while its flock of Salvage Drones works it. A broad, flat lifting
//! body, white over graphite, deepens amidships into a salvage hold whose two door
//! leaves swing down to let the four drones drop out of their cradles. A thick stub
//! wing each side fore and aft carries a big ducted lift fan in a pod that stands
//! up to hover (so from above the camera looks down into the spinning fans) and
//! lies down to cruise. A generator spine with the sensor mast runs down the back,
//! and a small V-tail closes the silhouette. Amber, the colour of Aster's economy,
//! marks the hold and the spine. From above: a wide arrowhead with four fan discs.
//!
//! The Salvage Drone ([`drone`]) is here too: a ring-fan lifter with a claw arm.
use super::*;

/// Fan pod pivots, front and rear, for the left (+y) side; the right is the mirror.
/// Pods are authored along +x; the entity shader tilts them about these points
/// (hover stands them up: intake to the sky, wash to the ground) and
/// `renderer::aircraft_trails` tilts the nozzles the same way. `entity.wgsl` carries
/// the same numbers.
pub const NACELLES: [[f32; 3]; 2] = [[3.3, 6.7, 1.5], [-3.5, 6.7, 1.5]];
/// The duct: how far it runs ahead of and behind its pivot, and its radius.
const DUCT_AHEAD: f32 = 0.95;
const DUCT_BEHIND: f32 = 1.2;
const DUCT_R: f32 = 1.45;
/// Where the fans' wash leaves the ducts, in the rest pose (`models::aircraft_exhausts`).
pub const NOZZLES: [[f32; 3]; 4] = [
    [NACELLES[0][0] - DUCT_BEHIND, -NACELLES[0][1], NACELLES[0][2]],
    [NACELLES[0][0] - DUCT_BEHIND, NACELLES[0][1], NACELLES[0][2]],
    [NACELLES[1][0] - DUCT_BEHIND, -NACELLES[1][1], NACELLES[1][2]],
    [NACELLES[1][0] - DUCT_BEHIND, NACELLES[1][1], NACELLES[1][2]],
];

/// The salvage hold under the midbody: its reach fore and aft, its half width, its
/// floor (the closed doors) and its ceiling. The sim's drone sockets
/// (`air_support::drone_socket`) stow the flock in it, two abreast, two deep.
pub const HOLD_X: f32 = 3.6;
pub const HOLD_HALF_WIDTH: f32 = 2.55;
pub const HOLD_FLOOR: f32 = 0.5;
pub const HOLD_CEILING: f32 = 1.8;
/// The door hinge: at the hold's side, on its floor. `entity.wgsl` swings the
/// `part::HOLD_DOOR` leaves about it; the `part::CRADLE`s drop with the drones.
pub const DOOR_HINGE: [f32; 2] = [HOLD_HALF_WIDTH, HOLD_FLOOR + 0.06];
/// Where the four drones sit in the hold (x, y of each), matching the sim's sockets.
pub const CRADLES: [[f32; 2]; 4] = [[-1.5, 1.3], [-1.5, -1.3], [1.5, 1.3], [1.5, -1.3]];

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder, spine.
/// A shallow nose, then the body swells and its keel lifts over the hold.
const HULL: [[f32; 9]; 7] = [
    [7.8, 0.15, 1.55, 0.35, 1.65, 0.3, 1.95, 0.1, 2.05],
    [6.0, 1.0, 1.0, 1.9, 1.45, 1.75, 2.7, 0.7, 3.15],
    [3.9, 1.9, 0.75, 3.2, 1.45, 2.95, 3.15, 1.15, 3.75],
    [3.3, 2.5, HOLD_CEILING - 0.05, 3.5, 1.85, 3.1, 3.2, 1.2, 3.8],
    [-3.3, 2.5, HOLD_CEILING - 0.05, 3.5, 1.85, 3.1, 3.2, 1.2, 3.8],
    [-3.9, 1.8, 0.8, 3.1, 1.45, 2.8, 3.05, 1.05, 3.6],
    [-7.6, 0.4, 1.35, 0.9, 1.6, 0.8, 2.3, 0.3, 2.6],
];
/// Stub wings in plan, fore and aft, from the hull side out under the fan pods.
const WING_FORE: [[f32; 2]; 4] = [[4.7, 3.1], [4.3, 6.6], [2.3, 6.6], [1.9, 3.3]];
const WING_AFT: [[f32; 2]; 4] = [[-2.3, 3.3], [-2.7, 6.6], [-4.5, 6.6], [-4.9, 3.0]];
/// A V-tail fin in side view, standing on its root line.
const FIN: [[f32; 2]; 4] = [[-7.5, 0.0], [-5.4, 0.0], [-6.3, 1.9], [-7.5, 1.9]];
const FIN_ROOT: (f32, f32) = (0.32, 2.45);
const FIN_CANT: f32 = 0.8;

/// A faceted ring about the x axis at `x`, centred on `c`.
fn ring(c: Vec3, x: f32, r: f32, sides: usize) -> Vec<Vec3> {
    (0..sides)
        .map(|k| {
            let a = (k as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
            v3(x, c.y + a.cos() * r, c.z + a.sin() * r)
        })
        .collect()
}

/// One lift fan in its duct, about its pivot: a fat faceted drum, the fan turning
/// in a dark throat at the front, the wash leaving a gunmetal mouth at the back
/// with the lift field's blue in it.
fn fan_pod(b: &mut MeshBuilder, pivot: Vec3) {
    let sides = b.sides(10);
    let x = |d: f32| pivot.x + d;
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            ring(pivot, x(DUCT_AHEAD), DUCT_R * 0.92, sides),
            ring(pivot, x(DUCT_AHEAD - 0.35), DUCT_R, sides),
            ring(pivot, x(-0.7), DUCT_R, sides),
            ring(pivot, x(-1.0), DUCT_R * 0.9, sides),
        ],
        true,
        false,
    );
    b.paint(METAL);
    b.loft(
        &[ring(pivot, x(-1.0), DUCT_R * 0.86, sides), ring(pivot, x(-DUCT_BEHIND), DUCT_R * 0.74, sides)],
        false,
        false,
    );
    // The lift field's blue, a disc set just inside the mouth, facing the wash.
    b.paint(GLOW);
    b.face(&ring(pivot, x(-DUCT_BEHIND + 0.04), DUCT_R * 0.6, sides));
    // The throat, and the fan in it: a hub and broad blades that turn.
    b.paint(ACCENT);
    let mut throat = ring(pivot, x(DUCT_AHEAD + 0.02), DUCT_R * 0.8, sides);
    throat.reverse();
    b.face(&throat);
    let hub = v3(x(DUCT_AHEAD + 0.02), pivot.y, pivot.z);
    b.paint(METAL);
    b.cylinder_between(hub, hub + Vec3::X * 0.3, 0.32, 0.2, b.sides(6));
    if b.mid() {
        let blades = if b.fine() { 5 } else { 3 };
        b.with_spin(hub, |b| {
            b.paint(PLATING_DARK);
            for k in 0..blades {
                let a = k as f32 * std::f32::consts::TAU / blades as f32;
                let out = v3(0.0, a.cos(), a.sin());
                let along = v3(0.0, -a.sin(), a.cos());
                // Each blade a raked slab, its tip pitched a little.
                let root = hub + Vec3::X * 0.12 + out * 0.3;
                let tip = hub + Vec3::X * 0.06 + out * 1.16 + along * 0.14;
                b.beam(root, tip, v2(0.06, 0.26), v2(0.05, 0.4));
            }
        });
    }
    if b.fine() {
        // The mouth's lip, dark, and a strake down the outboard side.
        b.paint(ACCENT);
        b.loft(
            &[ring(pivot, x(DUCT_AHEAD - 0.02), DUCT_R * 0.95, sides), ring(pivot, x(DUCT_AHEAD - 0.16), DUCT_R * 0.97, sides)],
            false,
            false,
        );
        b.paint(PLATING_DARK);
        b.beam(
            v3(x(DUCT_AHEAD - 0.5), pivot.y + DUCT_R * 0.98, pivot.z),
            v3(x(-0.8), pivot.y + DUCT_R * 0.98, pivot.z),
            v2(0.1, 0.5),
            v2(0.1, 0.5),
        );
    }
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    // White armour above the chines over a graphite belly, with a graphite salvage
    // deck saddling the roof over the hold; the belly lifts over the hold.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 1, 2), true, true);
    b.loft(&band(&HULL[..3], 2, 3), true, false);
    b.loft(&band(&HULL[5..], 2, 3), false, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[2..6], 2, 3), false, false);
    b.loft(&band(&HULL, 0, 1), true, true);

    // The hold: a graphite box under the midbody, its sides hanging below the chines,
    // the door leaves closing its floor between them.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.mirror_y(|b| {
        b.block(
            v3(-HOLD_X, HOLD_HALF_WIDTH, HOLD_FLOOR),
            v3(HOLD_X, HOLD_HALF_WIDTH + 0.95, HOLD_CEILING + 0.1),
        );
    });
    b.block(v3(HOLD_X - 0.5, -HOLD_HALF_WIDTH, HOLD_FLOOR), v3(HOLD_X, HOLD_HALF_WIDTH, HOLD_CEILING + 0.1));
    b.block(v3(-HOLD_X, -HOLD_HALF_WIDTH, HOLD_FLOOR), v3(-HOLD_X + 0.5, HOLD_HALF_WIDTH, HOLD_CEILING + 0.1));
    b.mirror_y(|b| {
        // A door leaf: hinged at the hold's side, meeting its twin on the centre line.
        b.with_part(part::HOLD_DOOR, |b| {
            b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
            b.cuboid(
                v3(0.0, HOLD_HALF_WIDTH * 0.5, HOLD_FLOOR + 0.06),
                v3(HOLD_X * 2.0 - 1.0, HOLD_HALF_WIDTH, 0.12),
            );
            if b.fine() {
                b.paint(TEAM).pattern(pattern::TEAM_BAND);
                b.plate(v3(0.0, HOLD_HALF_WIDTH * 0.5, HOLD_FLOOR - 0.03), v2(1.0, HOLD_HALF_WIDTH * 0.8), 0.03, 0.0);
            }
        });
        // The salvage intake on the hold's side: louvres, lit amber, where the mass comes in.
        if b.fine() {
            let side = HOLD_HALF_WIDTH + 0.95;
            b.with(
                Affine3A::from_translation(v3(0.0, side, 1.15)) * Affine3A::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                |b| vent(b, v3(0.0, 0.0, 0.0), v2(3.4, 0.7), 6, GLOW_AMBER),
            );
        }
    });
    // The cradles the drones hang from: they drop with the flock. Seen only from
    // below, so full detail alone.
    if fine {
        b.with_part(part::CRADLE, |b| {
            b.paint(ACCENT);
            for c in CRADLES {
                b.cuboid(v3(c[0], c[1], HOLD_CEILING - 0.12), v3(1.0, 0.6, 0.24));
            }
        });
    }

    // The generator spine down the back, white on the dark deck, the owner's panel
    // on it and the mast behind.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            vec![v3(3.0, -0.35, 3.55), v3(3.0, 0.35, 3.55), v3(3.0, 0.3, 3.7), v3(3.0, -0.3, 3.7)],
            vec![v3(1.6, -0.75, 3.6), v3(1.6, 0.75, 3.6), v3(1.6, 0.6, 4.2), v3(1.6, -0.6, 4.2)],
            vec![v3(-2.6, -0.75, 3.6), v3(-2.6, 0.75, 3.6), v3(-2.6, 0.58, 4.2), v3(-2.6, -0.58, 4.2)],
            vec![v3(-4.0, -0.35, 3.3), v3(-4.0, 0.35, 3.3), v3(-4.0, 0.3, 3.55), v3(-4.0, -0.3, 3.55)],
        ],
        true,
        true,
    );
    team_panel(b, v3(-0.5, 0.0, 4.2), v2(2.6, 0.85));
    if fine {
        b.paint(METAL);
        b.cylinder_between(v3(-4.4, 0.0, 3.3), v3(-4.7, 0.0, 4.35), 0.09, 0.06, 6);
        b.paint(ACCENT);
        b.cuboid(v3(-4.7, 0.0, 4.4), v3(0.5, 0.3, 0.14));
        // Amber running strips down the spine; the drone's visor in the nose.
        b.mirror_y(|b| glow_strip(b, v3(-0.5, 0.68, 3.62), v2(3.6, 0.08), GLOW_AMBER));
        b.paint(GLASS);
        b.cuboid(v3(6.9, 0.0, 2.1), v3(0.6, 1.1, 0.18));
        // Salvage intake grille on the back, behind the spine, black with its lit slots.
        vent(b, v3(-5.4, 0.0, 2.65), v2(1.2, 0.8), 3, GLOW_AMBER);
    }

    b.mirror_y(|b| {
        // Stub wings: graphite under white, the owner's band out near the pods.
        for plan in [&WING_FORE, &WING_AFT] {
            if b.fine() {
                b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
                b.extrude_z(plan, 1.1, 1.4);
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.extrude_z(plan, 1.4, 1.85);
            } else {
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.extrude_z(plan, 1.1, 1.85);
            }
        }
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        for x in [3.3, -3.6] {
            if b.fine() {
                b.plate(v3(x, 4.6, 1.85), v2(1.5, 1.4), 0.04, 0.02);
            } else {
                b.decal(v3(x, 4.6, 1.87), v2(1.5, 1.4));
            }
        }
        if b.fine() {
            b.paint(PLATING_DARK);
            b.beam(v3(4.72, 3.3, 1.5), v3(4.33, 6.4, 1.5), v2(0.4, 0.4), v2(0.34, 0.34));
        }

        // The fan pods, on their pivots.
        for (i, at) in NACELLES.iter().enumerate() {
            let pivot = Vec3::from(*at);
            b.with_part([part::VTOL_FRONT, part::VTOL_REAR][i], |b| fan_pod(b, pivot));
        }

        // A V-tail fin on the tail's shoulder.
        let cant = Affine3A::from_translation(v3(0.0, FIN_ROOT.0, FIN_ROOT.1))
            * Affine3A::from_rotation_x(-FIN_CANT);
        b.with(cant, |b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.extrude_y(&FIN, -0.09, 0.09);
            if b.fine() {
                b.paint(PLATING_DARK);
                b.extrude_y(&[[-7.5, 1.45], [-6.5, 1.45], [-6.3, 1.9], [-7.5, 1.9]], -0.1, 0.1);
            }
        });
    });
}

/// Far away: the arrowhead of the hull and wings, four fan discs, the spine.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[4], HULL[6]];
    b.paint(PLATING);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        b.face(&WING_FORE.map(|p| v3(p[0], p[1], 1.85)));
        b.face(&WING_AFT.map(|p| v3(p[0], p[1], 1.85)));
        b.paint(PLATING_DARK);
        for (i, at) in NACELLES.iter().enumerate() {
            let c = Vec3::from(*at);
            b.with_part([part::VTOL_FRONT, part::VTOL_REAR][i], |b| {
                b.face(&[
                    v3(c.x + DUCT_AHEAD, c.y - DUCT_R, c.z + DUCT_R * 0.9),
                    v3(c.x + DUCT_AHEAD, c.y + DUCT_R, c.z + DUCT_R * 0.9),
                    v3(c.x - DUCT_BEHIND, c.y + DUCT_R, c.z + DUCT_R * 0.9),
                    v3(c.x - DUCT_BEHIND, c.y - DUCT_R, c.z + DUCT_R * 0.9),
                ]);
            });
        }
    });
    b.paint(PLATING_DARK);
    b.face(&[v3(1.6, -0.7, 3.85), v3(1.6, 0.7, 3.85), v3(-2.6, 0.7, 3.85), v3(-2.6, -0.7, 3.85)]);
    b.paint(TEAM);
    b.decal(v3(-0.5, 0.0, 3.88), v2(2.6, 0.85));
}

// ---- Salvage Drone -----------------------------------------------------------

/// The reclaim emitter at the tip of the claw arm: the blueprint's `emitter`.
const EMITTER: Vec3 = Vec3::new(1.15, 0.0, 0.35);
/// Where the two steering jets leave the drone (`models::aircraft_exhausts`).
pub const DRONE_NOZZLES: [[f32; 3]; 2] = [[-1.05, -0.5, 0.42], [-1.05, 0.5, 0.42]];
/// The lift fan's axis, up through the ring (`part::ROTOR` turns about it).
const DRONE_FAN: Vec3 = Vec3::new(-0.15, 0.0, 0.5);

/// The Salvage Drone: a lift fan in a ring, a claw arm out ahead with the reclaim
/// emitter at its tip, two little steering jets at the back. Lives in the Osprey's hold.
pub(super) fn drone(b: &mut MeshBuilder) {
    b.set_spinner_pivot(DRONE_FAN);
    if b.coarse() {
        b.paint(ACCENT);
        b.prism(DRONE_FAN - Vec3::Z * 0.2, 6, 0.95, 0.9, 0.42);
        b.paint(GLOW_AMBER);
        b.cuboid(EMITTER - Vec3::X * 0.15, v3(0.3, 0.3, 0.3));
        return;
    }
    let fine = b.fine();
    // The ring: a dark drum with a white top band, the throat black.
    b.paint(ACCENT);
    b.prism(DRONE_FAN - Vec3::Z * 0.25, b.sides(10), 0.95, 0.95, 0.4);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.prism(DRONE_FAN + Vec3::Z * 0.15, b.sides(10), 0.95, 0.88, 0.14);
    b.paint(ACCENT);
    let throat: Vec<Vec3> = (0..b.sides(10))
        .map(|k| {
            let a = k as f32 * std::f32::consts::TAU / b.sides(10) as f32;
            DRONE_FAN + v3(a.cos() * 0.72, a.sin() * 0.72, 0.3)
        })
        .collect();
    b.face(&throat);
    // The fan: a hub and four blades (a cross, far off), turning about the ring's axis.
    b.paint(METAL);
    b.prism(DRONE_FAN + Vec3::Z * 0.1, b.sides(6), 0.16, 0.12, 0.28);
    let blades = if fine { 4 } else { 2 };
    b.with_part(part::ROTOR, |b| {
        b.paint(PLATING_DARK);
        for k in 0..blades {
            let a = k as f32 * std::f32::consts::TAU / 4.0;
            let out = v3(a.cos(), a.sin(), 0.0);
            b.beam(
                DRONE_FAN + Vec3::Z * 0.32 + out * 0.15,
                DRONE_FAN + Vec3::Z * 0.28 + out * 0.68,
                v2(0.16, 0.04),
                v2(0.26, 0.04),
            );
        }
    });
    // The claw arm: a dark boom out of the ring's front, the emitter head on its end.
    b.paint(PLATING_DARK);
    b.beam(v3(0.6, 0.0, 0.45), v3(EMITTER.x - 0.3, 0.0, EMITTER.z + 0.05), v2(0.3, 0.26), v2(0.24, 0.2));
    b.paint(ACCENT);
    b.cuboid(EMITTER - Vec3::X * 0.18, v3(0.36, 0.44, 0.36));
    b.paint(GLOW_AMBER);
    b.cuboid(EMITTER - Vec3::X * 0.04, v3(0.1, 0.24, 0.2));
    if fine {
        // Claw tines either side of the emitter.
        b.paint(METAL);
        b.mirror_y(|b| {
            b.beam(v3(EMITTER.x - 0.3, 0.26, EMITTER.z - 0.05), v3(EMITTER.x + 0.15, 0.3, EMITTER.z - 0.22), v2(0.05, 0.1), v2(0.03, 0.06));
        });
    }
    // The steering jets at the back, and the owner's mark on the ring.
    b.mirror_y(|b| {
        let n = Vec3::from(DRONE_NOZZLES[1]);
        b.paint(METAL);
        b.cylinder_between(n + Vec3::X * 0.5, n, 0.14, 0.11, b.sides(6));
        if b.fine() {
            b.paint(GLOW);
            b.cylinder_between(n + Vec3::X * 0.02, n + Vec3::X * 0.01, 0.08, 0.08, 6);
        }
    });
    team_panel(b, v3(-0.75, 0.0, DRONE_FAN.z + 0.29), v2(0.36, 0.4));
}

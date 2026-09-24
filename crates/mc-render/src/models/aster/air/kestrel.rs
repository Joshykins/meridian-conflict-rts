//! Kestrel: the tech 2 heavy gunship, a vector-thrust drone built to hang over a
//! target and grind it down. A wide, hard-chined hull, white armour over a graphite
//! belly, with a squat sponson each side fore and aft; on the tip of each sponson a
//! faceted engine pod that stands up to hover and lies down to cruise, its intake
//! turbine spinning and its nozzle ring hot. Under the chin the twin assault cannon
//! in a yawing cradle, under the forward sponsons a seven-tube rocket pod each. On
//! the back a long generator hump; at the tail two fins canted outward. From above:
//! a broad arrow with four engine discs at its corners, the gun reaching out ahead.
use super::*;

/// Nacelle pivots, front and rear, for the left (+y) side; the right is the mirror.
/// Engines are authored along +x; the entity shader tilts them about these points
/// (hover stands them up so the nozzle points down) and `renderer::aircraft_trails`
/// tilts the nozzles the same way. `entity.wgsl` carries the same numbers.
pub const NACELLES: [[f32; 3]; 2] = [[2.55, 4.75, 1.55], [-3.2, 4.75, 1.55]];
/// Nacelle body: how far it runs ahead of and behind its pivot, and its radius.
const POD_AHEAD: f32 = 1.65;
const POD_BEHIND: f32 = 1.72;
const POD_R: f32 = 1.0;
/// Where the exhausts end, in the rest pose (`models::aircraft_exhausts`).
pub const NOZZLES: [[f32; 3]; 4] = [
    [NACELLES[0][0] - POD_BEHIND, -NACELLES[0][1], NACELLES[0][2]],
    [NACELLES[0][0] - POD_BEHIND, NACELLES[0][1], NACELLES[0][2]],
    [NACELLES[1][0] - POD_BEHIND, -NACELLES[1][1], NACELLES[1][2]],
    [NACELLES[1][0] - POD_BEHIND, NACELLES[1][1], NACELLES[1][2]],
];

/// The chin cannon: where it yaws and pitches, and the (left) bore's exit. The
/// blueprint's `pivot` and `muzzles` say the same.
const GUN_PIVOT: Vec3 = Vec3::new(3.4, 0.0, 0.95);
const MUZZLE: Vec3 = Vec3::new(6.2, 0.38, 0.95);
/// Nose of the (left) rocket pod: the blueprint's rocket muzzle.
const POD_MUZZLE: Vec3 = Vec3::new(2.5, 3.1, 0.75);
const POD_LENGTH: f32 = 2.0;

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder, spine.
/// Wide and flat through the middle, a chisel nose, the tail pinched into a boom.
const HULL: [[f32; 9]; 6] = [
    [6.4, 0.25, 1.05, 0.55, 1.25, 0.5, 1.7, 0.15, 1.85],
    [4.6, 0.9, 0.7, 1.6, 1.25, 1.45, 2.35, 0.55, 2.75],
    [1.8, 1.1, 0.55, 1.9, 1.3, 1.7, 2.6, 0.7, 3.1],
    [-2.2, 1.1, 0.6, 1.9, 1.35, 1.7, 2.6, 0.7, 3.1],
    [-4.6, 0.8, 0.9, 1.4, 1.4, 1.2, 2.4, 0.5, 2.8],
    [-6.4, 0.3, 1.3, 0.6, 1.5, 0.55, 2.0, 0.2, 2.2],
];
/// Sponsons in plan, fore and aft, from the hull side out to the nacelle.
const SPONSON_FORE: [[f32; 2]; 4] = [[3.9, 1.5], [3.55, 4.7], [1.55, 4.7], [1.2, 1.55]];
const SPONSON_AFT: [[f32; 2]; 4] = [[-1.7, 1.55], [-2.1, 4.7], [-4.3, 4.7], [-4.8, 1.4]];
/// A tail fin in side view, standing on its root line.
const FIN: [[f32; 2]; 4] = [[-6.3, 0.0], [-4.3, 0.0], [-4.9, 1.35], [-6.15, 1.35]];
const FIN_ROOT: (f32, f32) = (1.05, 2.15);
const FIN_CANT: f32 = 0.42;

/// An octagon about the x axis at `x`, centred on `c`, faceted like the hull.
fn ring(c: Vec3, x: f32, r: f32, sides: usize) -> Vec<Vec3> {
    (0..sides)
        .map(|k| {
            let a = (k as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
            v3(x, c.y + a.cos() * r, c.z + a.sin() * r)
        })
        .collect()
}

/// One engine pod about its pivot: faceted body, dark intake with a spinning
/// turbine face, a gunmetal nozzle with the hot ring inside it.
fn nacelle(b: &mut MeshBuilder, pivot: Vec3) {
    let sides = b.sides(8);
    let x = |d: f32| pivot.x + d;
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            ring(pivot, x(POD_AHEAD), POD_R * 0.9, sides),
            ring(pivot, x(POD_AHEAD - 0.5), POD_R, sides),
            ring(pivot, x(-0.85), POD_R, sides),
            ring(pivot, x(-1.4), POD_R * 0.84, sides),
        ],
        true,
        false,
    );
    // Nozzle: a short gunmetal cone with the hot ring set inside its mouth.
    b.paint(METAL);
    b.loft(
        &[ring(pivot, x(-1.4), POD_R * 0.8, sides), ring(pivot, x(-POD_BEHIND), POD_R * 0.66, sides)],
        false,
        false,
    );
    // The hot ring: a disc set just inside the mouth, facing the jet.
    b.paint(GLOW_ORANGE);
    b.face(&ring(pivot, x(-POD_BEHIND + 0.04), POD_R * 0.52, sides));
    // Intake: a dark throat, and the turbine's blades turning in it.
    b.paint(ACCENT);
    let mut throat = ring(pivot, x(POD_AHEAD + 0.02), POD_R * 0.74, sides);
    throat.reverse();
    b.face(&throat);
    if b.fine() {
        let hub = v3(x(POD_AHEAD + 0.02), pivot.y, pivot.z);
        b.paint(METAL);
        b.cylinder_between(hub, hub + Vec3::X * 0.28, 0.22, 0.12, 6);
        b.with_spin(hub, |b| {
            b.paint(PLATING_DARK);
            for k in 0..5 {
                let a = k as f32 * std::f32::consts::TAU / 5.0;
                let out = v3(0.0, a.cos(), a.sin());
                b.beam(
                    hub + Vec3::X * 0.1 + out * 0.2,
                    hub + Vec3::X * 0.06 + out * 0.66,
                    v2(0.05, 0.22),
                    v2(0.04, 0.3),
                );
            }
        });
        // A pale armour strake down the pod's outboard side, and the fastener bead round the mouth.
        b.paint(PLATING_DARK);
        b.beam(
            v3(x(POD_AHEAD - 0.6), pivot.y + POD_R * 0.98, pivot.z),
            v3(x(-0.9), pivot.y + POD_R * 0.98, pivot.z),
            v2(0.1, 0.42),
            v2(0.1, 0.42),
        );
        b.paint(ACCENT);
        b.loft(
            &[ring(pivot, x(POD_AHEAD - 0.02), POD_R * 0.93, sides), ring(pivot, x(POD_AHEAD - 0.12), POD_R * 0.95, sides)],
            false,
            false,
        );
    }
}

pub(super) fn build(b: &mut MeshBuilder) {
    b.set_turret_pivot(GUN_PIVOT);
    b.set_arm_pivot(GUN_PIVOT);
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    // White armour above the chines over a graphite belly; the nose is a dark cap.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[1..], 1, 3), false, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[..2], 1, 3), true, false);
    b.loft(&band(&HULL, 0, 1), true, true);
    if fine {
        // Dark strakes along the chines, either side of the hull.
        b.mirror_y(|b| {
            b.beam(v3(4.5, 1.62, 1.28), v3(-4.5, 1.42, 1.42), v2(0.12, 0.2), v2(0.12, 0.2));
        });
    }

    // The generator hump down the back: a dark spine with the owner's panel on it.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            vec![v3(2.2, -0.3, 2.9), v3(2.2, 0.3, 2.9), v3(2.2, 0.3, 3.05), v3(2.2, -0.3, 3.05)],
            vec![v3(0.8, -0.62, 2.95), v3(0.8, 0.62, 2.95), v3(0.8, 0.5, 3.5), v3(0.8, -0.5, 3.5)],
            vec![v3(-3.4, -0.62, 2.95), v3(-3.4, 0.62, 2.95), v3(-3.4, 0.48, 3.5), v3(-3.4, -0.48, 3.5)],
            vec![v3(-4.6, -0.3, 2.7), v3(-4.6, 0.3, 2.7), v3(-4.6, 0.25, 2.95), v3(-4.6, -0.25, 2.95)],
        ],
        true,
        true,
    );
    team_panel(b, v3(-1.2, 0.0, 3.5), v2(2.4, 0.7));
    if fine {
        // Cooling louvres either side of the hump, lit: a tech 2 hull.
        b.mirror_y(|b| vent(b, v3(-0.6, 0.92, 3.08), v2(2.2, 0.36), 5, GLOW_ORANGE));
        // The drone's eye: a visor let into the nose.
        b.paint(GLASS);
        b.cuboid(v3(5.6, 0.0, 2.02), v3(0.55, 1.05, 0.16));
        // The heavy chin plate the gun hangs under.
        b.paint(PLATING_DARK);
        b.cuboid(v3(4.4, 0.0, 0.72), v3(1.6, 1.5, 0.16));
    }

    // The twin assault cannon: a cradle under the chin that yaws, the tubes
    // pitching in it about the same point.
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING_DARK);
        b.chamfered_box(v3(GUN_PIVOT.x - 0.1, 0.0, GUN_PIVOT.z + 0.1), v3(1.5, 1.35, 0.7), 0.12);
        b.with_limb(rig::ARM_GUN, |b| {
            b.paint(ACCENT);
            b.cuboid(v3(GUN_PIVOT.x + 0.55, 0.0, GUN_PIVOT.z), v3(1.2, 1.15, 0.5));
            b.mirror_y(|b| {
                b.paint(METAL);
                b.cylinder_between(
                    v3(GUN_PIVOT.x + 1.0, MUZZLE.y, MUZZLE.z),
                    MUZZLE,
                    0.14,
                    0.1,
                    b.sides(6),
                );
                if b.fine() {
                    // Slotted cooling jacket at the breech, a brake at the muzzle.
                    b.paint(ACCENT);
                    b.cylinder_between(
                        v3(GUN_PIVOT.x + 1.1, MUZZLE.y, MUZZLE.z),
                        v3(GUN_PIVOT.x + 1.9, MUZZLE.y, MUZZLE.z),
                        0.2,
                        0.19,
                        6,
                    );
                    b.paint(METAL);
                    b.cylinder_between(MUZZLE - Vec3::X * 0.32, MUZZLE, 0.17, 0.16, 6);
                }
            });
        });
    });

    b.mirror_y(|b| {
        // Sponsons: thick stubs, graphite under white, the owner's band across the top.
        for plan in [&SPONSON_FORE, &SPONSON_AFT] {
            if b.fine() {
                b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
                b.extrude_z(plan, 1.2, 1.5);
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.extrude_z(plan, 1.5, 1.9);
            } else {
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.extrude_z(plan, 1.2, 1.9);
            }
        }
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        for x in [2.55, -3.2] {
            if b.fine() {
                b.plate(v3(x, 3.5, 1.9), v2(1.5, 1.1), 0.04, 0.02);
            } else {
                b.decal(v3(x, 3.5, 1.92), v2(1.5, 1.1));
            }
        }
        if b.fine() {
            // Leading-edge armour on the forward sponson.
            b.paint(PLATING_DARK);
            b.beam(v3(3.92, 1.7, 1.55), v3(3.58, 4.55, 1.55), v2(0.36, 0.36), v2(0.3, 0.3));
        }

        // The engine pods, on their pivots.
        for (i, at) in NACELLES.iter().enumerate() {
            let pivot = Vec3::from(*at);
            b.with_part([part::VTOL_FRONT, part::VTOL_REAR][i], |b| nacelle(b, pivot));
        }

        // The rocket pod under the forward sponson: a hexagonal cluster of seven
        // tubes, slung from a blade pylon.
        let tail = POD_MUZZLE - Vec3::X * POD_LENGTH;
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.cylinder_between(tail, POD_MUZZLE - Vec3::X * 0.12, 0.56, 0.56, b.sides(6));
        b.paint(ACCENT);
        b.cylinder_between(POD_MUZZLE - Vec3::X * 0.12, POD_MUZZLE, 0.5, 0.5, b.sides(6));
        b.paint(PLATING_DARK);
        b.extrude_y(
            &[[2.1, 0.95], [0.9, 0.95], [0.8, 1.25], [2.2, 1.25]],
            POD_MUZZLE.y - 0.1,
            POD_MUZZLE.y + 0.1,
        );
        if b.fine() {
            // The tube mouths: one in the middle, six round it.
            b.paint(METAL);
            for k in 0..7 {
                let (dy, dz) = if k == 0 {
                    (0.0, 0.0)
                } else {
                    let a = k as f32 * std::f32::consts::TAU / 6.0;
                    (a.cos() * 0.3, a.sin() * 0.3)
                };
                let mouth = POD_MUZZLE + v3(0.0, dy, dz);
                b.cylinder_between(mouth - Vec3::X * 0.06, mouth + Vec3::X * 0.02, 0.11, 0.11, 6);
            }
            b.paint(TEAM);
            b.cylinder_between(tail + Vec3::X * 0.3, tail + Vec3::X * 0.5, 0.57, 0.57, 6);
        }

        // A tail fin, canted outward, standing on the tail boom's shoulder.
        let cant = Affine3A::from_translation(v3(0.0, FIN_ROOT.0, FIN_ROOT.1))
            * Affine3A::from_rotation_x(-FIN_CANT);
        b.with(cant, |b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.extrude_y(&FIN, -0.09, 0.09);
            if b.fine() {
                b.paint(PLATING_DARK);
                b.extrude_y(&[[-6.15, 0.95], [-5.0, 0.95], [-4.9, 1.35], [-6.15, 1.35]], -0.1, 0.1);
            }
        });
    });
    if fine {
        // Formation strip along the tail boom, lit.
        glow_strip(b, v3(-5.4, 0.0, 2.6), v2(1.2, 0.08), GLOW_ORANGE);
    }
}

/// Far away: the hull, the four pods at the corners of the sponsons, the gun ahead.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[4], HULL[5]];
    b.paint(PLATING);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        b.face(&SPONSON_FORE.map(|p| v3(p[0], p[1], 1.9)));
        b.face(&SPONSON_AFT.map(|p| v3(p[0], p[1], 1.9)));
        b.paint(PLATING_DARK);
        for (i, at) in NACELLES.iter().enumerate() {
            let c = Vec3::from(*at);
            b.with_part([part::VTOL_FRONT, part::VTOL_REAR][i], |b| {
                b.face(&[
                    v3(c.x + POD_AHEAD, c.y - POD_R, c.z + POD_R * 0.9),
                    v3(c.x + POD_AHEAD, c.y + POD_R, c.z + POD_R * 0.9),
                    v3(c.x - POD_BEHIND, c.y + POD_R, c.z + POD_R * 0.9),
                    v3(c.x - POD_BEHIND, c.y - POD_R, c.z + POD_R * 0.9),
                ]);
            });
        }
    });
    b.paint(TEAM);
    b.decal(v3(-1.2, 0.0, 3.12), v2(2.4, 0.7));
    b.with_part(part::TURRET, |b| {
        b.paint(METAL);
        b.beam(v3(GUN_PIVOT.x, 0.0, MUZZLE.z), v3(MUZZLE.x, 0.0, MUZZLE.z), v2(1.0, 0.4), v2(0.9, 0.3));
    });
}

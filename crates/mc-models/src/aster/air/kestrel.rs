//! Kestrel: the tech 2 heavy gunship, a tilt-jet drone built to hang over a target
//! and grind it down. A chiselled nose over the chin gun steps out into armoured
//! cheeks, a hump rides the back under a high straight wing, and the body pinches
//! hard into a slim boom ending in an H tail. At each wing tip a faceted jet nacelle
//! with a raked inlet turns on its trunnion: up to hover, forward to fly, back to
//! brake, and one against the other to turn, so it needs no tail rotor. Under the
//! chin a ball turret carries one long autocannon; under each wing a six-tube rocket
//! launcher. From above: a long cross, a nacelle at each tip, the gun reaching out
//! ahead and the twin fins at the end of the boom.
use super::*;
use glam::Vec2;

/// The (left) nacelle's pivot on the wing tip. It turns about the trunnion (along y)
/// through here (`Model::vtol`).
const PIVOT: Vec3 = Vec3::new(0.05, 4.95, 2.4);
/// How far the nacelle runs ahead of its pivot and behind it (the nozzle), and its
/// half width and half height.
const NACELLE_AHEAD: f32 = 2.2;
const NACELLE_BEHIND: f32 = 2.05;
const NACELLE_W: f32 = 0.6;
const NACELLE_H: f32 = 0.66;

/// The chin gun: where the ball yaws and the barrel pitches, and the muzzle. The
/// blueprint's `pivot` and `muzzle` say the same.
const GUN_PIVOT: Vec3 = Vec3::new(4.75, 0.0, 0.56);
const MUZZLE: Vec3 = Vec3::new(7.3, 0.0, 0.56);

/// The (left) rocket launcher: the middle of its face, its tubes' spacing across and
/// up. The blueprint's rocket muzzles are the six tube mouths each side.
const LAUNCHER: Vec3 = Vec3::new(1.45, 2.7, 1.7);
const TUBE_STEP: Vec2 = Vec2::new(0.2, 0.12);
const LAUNCHER_LENGTH: f32 = 2.0;

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder,
/// spine. A narrow chisel nose, a step out into the armoured cheeks, a hard pinch into
/// the boom.
const HULL: [[f32; 9]; 9] = [
    [7.0, 0.04, 1.18, 0.08, 1.24, 0.06, 1.3, 0.02, 1.34],
    [5.9, 0.3, 0.9, 0.62, 1.18, 0.48, 1.62, 0.16, 1.8],
    [4.4, 0.5, 0.72, 0.86, 1.12, 0.7, 1.9, 0.26, 2.1],
    [3.2, 0.6, 0.62, 0.98, 1.08, 0.82, 2.05, 0.32, 2.3],
    [2.5, 0.92, 0.5, 1.38, 1.0, 1.12, 2.2, 0.44, 2.5],
    [-1.2, 0.95, 0.52, 1.42, 1.04, 1.16, 2.24, 0.46, 2.54],
    [-2.5, 0.58, 0.92, 0.9, 1.32, 0.76, 2.2, 0.3, 2.38],
    [-4.3, 0.28, 1.38, 0.44, 1.56, 0.38, 2.04, 0.15, 2.14],
    [-6.3, 0.18, 1.62, 0.28, 1.72, 0.25, 2.04, 0.1, 2.1],
];
/// The shoulder wing, root and tip: (y, z, leading x, trailing x, thickness).
const WING: [(f32, f32, f32, f32, f32); 2] =
    [(1.0, 2.45, 1.15, -1.25, 0.4), (4.2, 2.4, 0.95, -0.85, 0.3)];
/// The tailplane in plan, across the end of the boom, and a fin in side view standing
/// on its tip.
const TAILPLANE: [[f32; 2]; 4] = [[-5.7, 0.0], [-6.55, 1.75], [-7.25, 1.75], [-7.05, 0.0]];
const FIN: [[f32; 2]; 4] = [[-6.0, 0.0], [-6.75, 1.55], [-7.4, 1.55], [-7.2, 0.0]];

/// A wing section: a slim lens round the chord.
fn section(s: (f32, f32, f32, f32, f32)) -> Vec<Vec3> {
    let (y, z, lead, trail, t) = s;
    let chord = lead - trail;
    vec![
        v3(lead, y, z),
        v3(lead - chord * 0.3, y, z + t * 0.5),
        v3(trail, y, z + t * 0.1),
        v3(trail, y, z - t * 0.1),
        v3(lead - chord * 0.3, y, z - t * 0.5),
    ]
}

/// A ring about the x axis at `x`, centred on `c`.
fn ring(c: Vec3, x: f32, r: f32, sides: usize) -> Vec<Vec3> {
    (0..sides)
        .map(|k| {
            let a = (k as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
            v3(x, c.y + a.cos() * r, c.z + a.sin() * r)
        })
        .collect()
}

/// A faceted section about the x axis through `c`: half width `w`, half height `h`,
/// corners cut by `cut`. `rake` pushes each point forward by its height over `h`, so a
/// section can be scarfed (the top ahead of the bottom).
fn facet(c: Vec3, x: f32, w: f32, h: f32, cut: f32, rake: f32) -> Vec<Vec3> {
    [
        [w, h - cut],
        [w - cut, h],
        [-(w - cut), h],
        [-w, h - cut],
        [-w, -(h - cut)],
        [-(w - cut) * 0.8, -h],
        [(w - cut) * 0.8, -h],
        [w, -(h - cut)],
    ]
    .iter()
    .map(|p| v3(x + rake * p[1] / h, c.y + p[0], c.z + p[1]))
    .collect()
}

/// One nacelle about its pivot, lying along +x: a faceted body, the inlet, a flat
/// vectoring nozzle with its hot slot.
fn nacelle(b: &mut MeshBuilder, pivot: Vec3) {
    let (w, h) = (NACELLE_W, NACELLE_H);
    let x = |d: f32| pivot.x + d;
    let fine = b.fine();
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    // A raked inlet: the lip leans forward at the top, the throat goes dark
    // and deep with nothing in it.
    b.loft(
        &[
            facet(pivot, x(NACELLE_AHEAD - 0.25), w * 0.94, h * 0.9, 0.2, 0.3),
            facet(pivot, x(NACELLE_AHEAD - 0.9), w, h, 0.22, 0.0),
            facet(pivot, x(-1.25), w, h, 0.22, 0.0),
            facet(pivot, x(-1.65), w * 0.86, h * 0.72, 0.18, 0.0),
        ],
        false,
        false,
    );
    // The throat: a dark wall across the mouth, facing out of it (`face` keeps its
    // winding: counter-clockwise seen from +x faces +x), and a dark lip ring round it.
    b.paint(ACCENT);
    b.face(&facet(
        pivot,
        x(NACELLE_AHEAD - 0.3),
        w * 0.9,
        h * 0.86,
        0.19,
        0.3,
    ));
    if fine {
        b.paint(PLATING_DARK);
        b.loft(
            &[
                facet(
                    pivot,
                    x(NACELLE_AHEAD - 0.22),
                    w * 0.98,
                    h * 0.94,
                    0.21,
                    0.3,
                ),
                facet(pivot, x(NACELLE_AHEAD - 0.3), w * 0.98, h * 0.94, 0.21, 0.3),
            ],
            false,
            false,
        );
    }
    // The nozzle: the body drawn in to a flat gunmetal box, the hot slot inside it.
    b.paint(METAL);
    b.loft(
        &[
            facet(pivot, x(-1.65), w * 0.86, h * 0.72, 0.18, 0.0),
            facet(pivot, x(-NACELLE_BEHIND), w * 0.78, h * 0.46, 0.1, 0.0),
        ],
        false,
        false,
    );
    // The hot slot faces aft, out of the nozzle.
    b.paint(GLOW);
    let mut slot = facet(
        pivot,
        x(-NACELLE_BEHIND + 0.08),
        w * 0.66,
        h * 0.32,
        0.06,
        0.0,
    );
    slot.reverse();
    b.face(&slot);
    if fine {
        // Nozzle flaps top and bottom, a spine strake, the owner's band.
        b.paint(PLATING_DARK);
        for s in [1.0, -1.0] {
            b.beam(
                v3(x(-1.7), pivot.y, pivot.z + s * h * 0.62),
                v3(x(-NACELLE_BEHIND - 0.12), pivot.y, pivot.z + s * h * 0.44),
                v2(w * 1.5, 0.05),
                v2(w * 1.4, 0.05),
            );
        }
        b.beam(
            v3(x(NACELLE_AHEAD - 1.1), pivot.y, pivot.z + h),
            v3(x(-1.3), pivot.y, pivot.z + h),
            v2(0.16, 0.1),
            v2(0.16, 0.1),
        );
        b.paint(TEAM);
        b.loft(
            &[
                facet(pivot, x(-0.4), w * 1.02, h * 1.02, 0.22, 0.0),
                facet(pivot, x(-0.7), w * 1.02, h * 1.02, 0.22, 0.0),
            ],
            false,
            false,
        );
    }
    if fine {
        // The trunnion collar on its inboard side.
        b.paint(METAL);
        let collar = pivot - Vec3::Y * w;
        b.cylinder_between(
            collar - Vec3::Y * 0.14,
            collar + Vec3::Y * 0.06,
            0.34,
            0.34,
            8,
        );
    }
}

/// The H tail: a tailplane across the end of the boom with a fin standing on each tip,
/// canted a little outward and dark-capped.
fn h_tail(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_z(&TAILPLANE, 1.88, 2.02);
    b.mirror_y(|b| {
        let cant = Affine3A::from_translation(v3(0.0, TAILPLANE[1][1] - 0.1, 1.95))
            * Affine3A::from_rotation_x(-0.12);
        b.with(cant, |b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.extrude_y(&FIN, -0.09, 0.09);
            if b.fine() {
                // The fin runs below the tailplane too, a short ventral blade.
                b.extrude_y(
                    &[[-6.05, 0.0], [-7.05, 0.0], [-7.0, -0.55], [-6.45, -0.5]],
                    -0.08,
                    0.08,
                );
                b.paint(PLATING_DARK);
                b.extrude_y(
                    &[[-6.6, 1.3], [-7.35, 1.3], [-7.4, 1.55], [-6.75, 1.55]],
                    -0.1,
                    0.1,
                );
            }
        });
    });
}

pub(super) fn build(b: &mut MeshBuilder) {
    b.set_vtol(crate::Vtol {
        pivots: [PIVOT.to_array(), [0.0; 3]],
        pairs: 1,
        nozzle: [NACELLE_BEHIND, NACELLE_H * 0.4],
        fans: false,
    });
    b.set_turret_pivot(GUN_PIVOT);
    b.set_arm_pivot(GUN_PIVOT);
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    // White armour above the chines over a graphite belly; the nose is a dark chisel.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[1..], 1, 3), false, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[..2], 1, 3), true, false);
    b.loft(&band(&HULL, 0, 1), true, true);
    if fine {
        b.mirror_y(|b| {
            // The cheek armour's edge, a dark strake down the boom, and the sensor
            // slits either side of the nose.
            b.paint(PLATING_DARK);
            b.beam(
                v3(2.55, 1.3, 1.02),
                v3(-1.3, 1.36, 1.05),
                v2(0.16, 0.14),
                v2(0.16, 0.14),
            );
            b.beam(
                v3(-2.4, 0.84, 1.34),
                v3(-6.2, 0.26, 1.74),
                v2(0.1, 0.1),
                v2(0.08, 0.08),
            );
            b.paint(GLASS);
            b.beam(
                v3(5.7, 0.52, 1.38),
                v3(4.3, 0.76, 1.55),
                v2(0.05, 0.16),
                v2(0.05, 0.2),
            );
        });
        b.paint(GLASS);
        b.spheroid(v3(6.1, 0.0, 0.84), v3(0.24, 0.24, 0.2), 8, 4);
    }

    // The hump over the wing root: the engine bay, an intake at its front, lit
    // louvres either side, the owner's panel on top.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            facet(v3(0.0, 0.0, 2.3), 2.4, 0.3, 0.1, 0.04, 0.0),
            facet(v3(0.0, 0.0, 2.72), 1.5, 0.62, 0.36, 0.16, 0.0),
            facet(v3(0.0, 0.0, 2.72), -1.8, 0.6, 0.36, 0.16, 0.0),
            facet(v3(0.0, 0.0, 2.36), -3.3, 0.22, 0.1, 0.04, 0.0),
        ],
        true,
        true,
    );
    team_panel(b, v3(-0.3, 0.0, 3.08), v2(1.6, 0.5));
    if fine {
        b.paint(ACCENT);
        b.face(&facet(v3(0.0, 0.0, 2.74), 1.51, 0.46, 0.24, 0.1, 0.0));
        b.mirror_y(|b| vent(b, v3(-1.0, 0.58, 2.86), v2(1.4, 0.2), 4, GLOW_ORANGE));
        glow_strip(b, v3(-5.0, 0.0, 2.15), v2(1.0, 0.07), GLOW_ORANGE);
    }

    // The chin gun: a ball turret that yaws, its one long barrel pitching in it.
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING_DARK);
        b.spheroid(GUN_PIVOT, v3(0.56, 0.52, 0.42), b.sides(10), 5);
        b.with_limb(rig::ARM_GUN, |b| {
            b.paint(ACCENT);
            b.chamfered_box(GUN_PIVOT + v3(0.55, 0.0, 0.0), v3(1.1, 0.4, 0.34), 0.06);
            b.paint(METAL);
            let breech = GUN_PIVOT + Vec3::X * 1.05;
            b.cylinder_between(breech, MUZZLE, 0.085, 0.075, b.sides(8));
            if b.fine() {
                // A slotted cooling jacket and a muzzle brake.
                b.paint(PLATING_DARK);
                b.cylinder_between(breech, breech + Vec3::X * 0.95, 0.15, 0.14, 8);
                b.paint(ACCENT);
                for k in 0..4 {
                    let at = breech + Vec3::X * (0.12 + k as f32 * 0.22);
                    b.cylinder_between(at, at + Vec3::X * 0.06, 0.165, 0.165, 8);
                }
                b.paint(METAL);
                b.cylinder_between(MUZZLE - Vec3::X * 0.28, MUZZLE, 0.12, 0.12, 8);
            }
        });
    });

    b.mirror_y(|b| {
        // The shoulder wing, dark along its leading edge.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &WING.iter().map(|&s| section(s)).collect::<Vec<_>>(),
            true,
            true,
        );
        if b.fine() {
            let (a, z) = (WING[0], WING[1]);
            b.paint(PLATING_DARK);
            b.beam(
                v3(a.2 - 0.12, a.0 + 0.3, a.1),
                v3(z.2 - 0.12, z.0 - 0.05, z.1),
                v2(0.28, a.4 * 0.7),
                v2(0.24, z.4 * 0.7),
            );
            b.paint(TEAM).pattern(pattern::TEAM_BAND);
            b.plate(v3(-0.3, 3.3, 2.58), v2(1.0, 0.9), 0.03, 0.01);
        }
        // The trunnion shaft out of the tip, and the nacelle on it.
        b.paint(PLATING_DARK);
        b.cylinder_between(
            v3(PIVOT.x, WING[1].0 - 0.2, PIVOT.z),
            v3(PIVOT.x, PIVOT.y - NACELLE_W * 0.9, PIVOT.z),
            0.26,
            0.24,
            b.sides(8),
        );
        b.with_part(part::VTOL_FRONT, |b| nacelle(b, PIVOT));

        // The rocket launcher under the wing: six tubes in a box on a blade pylon.
        let tail = LAUNCHER - Vec3::X * LAUNCHER_LENGTH;
        let mid = (LAUNCHER + tail) * 0.5;
        let face = v3(0.0, TUBE_STEP.x * 3.0 + 0.14, TUBE_STEP.y * 2.0 + 0.2);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.chamfered_box(
            mid - Vec3::X * 0.05,
            face + Vec3::X * (LAUNCHER_LENGTH - 0.1),
            0.06,
        );
        b.extrude_y(
            &[
                [mid.x + 0.5, LAUNCHER.z + 0.1],
                [mid.x - 0.7, LAUNCHER.z + 0.1],
                [mid.x - 0.5, WING[0].1 - 0.05],
                [mid.x + 0.5, WING[0].1 - 0.05],
            ],
            LAUNCHER.y - 0.06,
            LAUNCHER.y + 0.06,
        );
        b.paint(TEAM);
        b.cuboid(tail + Vec3::X * 0.35, face + v3(0.18, 0.02, 0.02));
        if b.fine() {
            b.paint(ACCENT);
            b.cuboid(LAUNCHER - Vec3::X * 0.01, face + v3(0.04, -0.06, -0.08));
            b.paint(METAL);
            for dy in [-1.0, 0.0, 1.0] {
                for dz in [-1.0, 1.0] {
                    let mouth = LAUNCHER + v3(0.0, dy * TUBE_STEP.x, dz * TUBE_STEP.y);
                    b.face(&ring(mouth, mouth.x + 0.005, 0.075, 6));
                }
            }
        }
    });

    h_tail(b);
}

/// Far away: the hull, the wing, the two nacelles, the tail, the gun ahead.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[4], HULL[8]];
    b.paint(PLATING);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        b.face(&TAILPLANE.map(|p| v3(p[0], p[1], 2.0)));
        b.face(&FIN.map(|p| v3(p[0], TAILPLANE[1][1] - 0.1, 1.95 + p[1])));
        let (a, z) = (WING[0], WING[1]);
        b.face(&[
            v3(a.2, a.0, a.1),
            v3(z.2, z.0, z.1),
            v3(z.3, z.0, z.1),
            v3(a.3, a.0, a.1),
        ]);
        b.paint(PLATING_DARK);
        let c = PIVOT;
        b.with_part(part::VTOL_FRONT, |b| {
            b.face(&[
                v3(c.x + NACELLE_AHEAD, c.y - NACELLE_W, c.z + NACELLE_H),
                v3(c.x + NACELLE_AHEAD, c.y + NACELLE_W, c.z + NACELLE_H),
                v3(c.x - NACELLE_BEHIND, c.y + NACELLE_W, c.z + NACELLE_H),
                v3(c.x - NACELLE_BEHIND, c.y - NACELLE_W, c.z + NACELLE_H),
            ]);
        });
    });
    b.paint(TEAM);
    b.decal(v3(-0.3, 0.0, 2.6), v2(1.6, 0.5));
    b.with_part(part::TURRET, |b| {
        b.paint(METAL);
        b.beam(GUN_PIVOT, MUZZLE, v2(0.7, 0.4), v2(0.2, 0.2));
    });
}

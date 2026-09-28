//! Argus: the tech 2 radar and missile-defence picket, a high-loitering sensor drone.
//! Its wing is joined: a low forward wing sweeps back and a high rear wing, carried
//! on the fin, sweeps forward and down to meet it at a pod on each tip, so from
//! above it is a diamond round a long white body, a shape no other aircraft has.
//! A lens rotodome turns on a pylon over the back, the forward wings' leading edges
//! carry black conformal arrays, a sensor ball hangs under the chin and a radar
//! canoe runs along the belly. Its missile defence is a big red laser head on each
//! wingtip pod, where the beams leave (`anti_missile_mounts` in air.ron). It hunts
//! dived hulls with a hydrophone boom out of the tail, throws a small field from a
//! gold lens on its spine, and salvages wrecks from cruise height with a reclaim
//! turret hung under the belly ([`turret`]).
//!
//! The airframe is authored at its old size and drawn [`SCALE`] times bigger, [`LIFT`]
//! up, so what hangs under the belly stays above the model's origin.
use super::super::naval::pd_laser;
use super::*;
use crate::builder::ngon;

mod turret;
pub(super) use turret::Turret;

/// The airframe is drawn this much bigger than it is authored (`support_air` is 9.2 m).
const SCALE: f32 = 1.15;
/// And sits this far up (authored metres).
const LIFT: f32 = 1.2;
/// The (left) laser head's centre (authored): keep the Argus's `anti_missile_mounts`
/// (this times [`SCALE`], [`LIFT`] up) in step.
const LASER: Vec3 = Vec3::new(-2.2, 7.85, 2.08);
/// Where the (left) engine's exhaust ends, authored.
const NOZZLE: Vec3 = Vec3::new(-5.35, 1.05, 1.4);
/// Both exhausts as drawn, for the effect renderer (`models::aircraft_exhausts`).
pub(crate) const NOZZLES: [[f32; 3]; 2] = [
    [
        NOZZLE.x * SCALE,
        -NOZZLE.y * SCALE,
        (NOZZLE.z + LIFT) * SCALE,
    ],
    [
        NOZZLE.x * SCALE,
        NOZZLE.y * SCALE,
        (NOZZLE.z + LIFT) * SCALE,
    ],
];
/// The shield projector's lens (authored).
const SHIELD: Vec3 = Vec3::new(1.7, 0.0, 2.08);
/// The rotodome's centre, on its turning axis, and its radius.
const DOME: Vec3 = Vec3::new(-0.6, 0.0, 3.42);
const DOME_RADIUS: f32 = 2.1;
/// Centre line of the (left) wingtip pod the two wings meet in.
const POD: Vec3 = Vec3::new(0.0, 7.85, 1.22);

/// Hull stations nose to tail: x, then (half width, height) at keel, chine, shoulder, spine.
/// A long body with a sensor hump behind the nose, tapering into the fin.
const HULL: [[f32; 9]; 8] = [
    [6.6, 0.03, 1.0, 0.05, 1.05, 0.04, 1.12, 0.02, 1.16],
    [5.6, 0.24, 0.66, 0.44, 0.94, 0.38, 1.4, 0.16, 1.56],
    [4.2, 0.34, 0.5, 0.6, 0.88, 0.56, 1.7, 0.26, 2.02],
    [2.4, 0.38, 0.46, 0.66, 0.86, 0.58, 1.68, 0.24, 1.96],
    [0.0, 0.38, 0.46, 0.64, 0.86, 0.54, 1.56, 0.2, 1.78],
    [-2.8, 0.32, 0.52, 0.56, 0.9, 0.46, 1.48, 0.16, 1.66],
    [-5.0, 0.18, 0.84, 0.32, 1.0, 0.26, 1.36, 0.1, 1.46],
    [-6.4, 0.06, 1.06, 0.1, 1.1, 0.08, 1.24, 0.03, 1.28],
];

/// Wing sections, root then tip: (y, z of the chord line, leading x, trailing x,
/// thickness). The forward wing rises gently from the belly to the tip pod.
const FORE: [(f32, f32, f32, f32, f32); 2] =
    [(0.55, 0.78, 2.8, 0.2, 0.22), (7.7, 1.12, -1.1, -2.7, 0.12)];
/// The rear wing leaves the top of the fin and falls forward to the same pod.
const AFT: [(f32, f32, f32, f32, f32); 2] =
    [(0.1, 2.95, -4.5, -6.0, 0.15), (7.7, 1.3, -1.8, -3.1, 0.1)];
/// The fin in side view, on the tail; its top chord carries the rear wing's roots.
const FIN: [[f32; 2]; 4] = [[-6.35, 1.2], [-3.9, 1.5], [-4.65, 3.02], [-5.95, 3.02]];
/// The rotodome pylon in side view, from the spine up under the dome.
const PYLON: [[f32; 2]; 4] = [[1.0, 1.8], [-2.0, 1.62], [-1.4, 3.25], [0.1, 3.25]];

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

/// The Argus with its reclaim turret `turret` (the design variants `support_air~*`
/// differ only in the turret, until the user picks one).
pub(super) fn build(b: &mut MeshBuilder, turret: Turret) {
    let frame =
        Affine3A::from_scale(Vec3::splat(SCALE)) * Affine3A::from_translation(Vec3::Z * LIFT);
    b.with(frame, |b| body(b, turret));
}

fn body(b: &mut MeshBuilder, turret: Turret) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();
    let hull = if fine {
        HULL.to_vec()
    } else {
        [0, 1, 2, 4, 6, 7].map(|i| HULL[i]).to_vec()
    };

    // White upper hull over a graphite belly, the owner's panel behind the dome.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&hull, 1, 3), true, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&hull, 0, 1), true, true);
    team_panel(b, v3(-3.7, 0.0, 1.57), v2(1.2, 0.3));

    // The sensor ball under the chin, and the radar canoe along the belly.
    b.paint(ACCENT);
    b.spheroid(
        v3(4.6, 0.0, 0.34),
        v3(0.36, 0.36, 0.32),
        b.sides(10),
        if fine { 5 } else { 3 },
    );
    if fine {
        b.paint(PLATING_DARK);
        // The canoe stops short of the reclaim turret.
        b.beam(
            v3(3.2, 0.0, 0.4),
            v3(1.25, 0.0, 0.42),
            v2(0.52, 0.26),
            v2(0.46, 0.22),
        );
        // The sensor window on the hump, and a pitot probe out ahead.
        b.paint(GLASS);
        b.plate(v3(4.9, 0.0, 1.76), v2(0.55, 0.3), 0.05, 0.02);
        b.paint(METAL);
        b.cylinder_between(v3(6.55, 0.0, 1.08), v3(7.3, 0.0, 1.08), 0.04, 0.015, 5);
        b.cylinder_between(v3(4.6, 0.0, 0.66), v3(4.6, 0.0, 0.6), 0.2, 0.2, 8);
    }

    // The fin, its tip dark where the rear wings leave it.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_y(&FIN, -0.1, 0.1);
    if fine {
        b.paint(PLATING_DARK);
        b.extrude_y(
            &[[-6.0, 2.7], [-4.4, 2.7], [-4.65, 3.02], [-5.95, 3.02]],
            -0.11,
            0.11,
        );
    }

    rotodome(b);
    detail(b);
    turret::build(b, turret);
    sonar(b);
    shield(b);

    b.mirror_y(|b| {
        // The joined wing: forward panel low and swept back, rear panel high and
        // swept forward, both into the tip pod.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(&FORE.map(section), true, true);
        b.loft(&AFT.map(section), true, true);
        if b.fine() {
            // Black conformal arrays along the forward wing's leading edge.
            b.paint(ACCENT);
            let edge = |s: (f32, f32, f32, f32, f32), inboard: f32| {
                let chord = s.2 - s.3;
                v3(s.2 - chord * 0.16, s.0 + inboard, s.1 + s.4 * 0.36)
            };
            b.beam(
                edge(FORE[0], 0.4),
                edge(FORE[1], -0.5),
                v2(0.5, 0.05),
                v2(0.28, 0.04),
            );
            // The rear wing's leading edge dark, as on the fin.
            b.paint(PLATING_DARK);
            b.beam(
                v3(AFT[0].2 - 0.05, 0.3, AFT[0].1),
                v3(AFT[1].2 - 0.05, 7.4, AFT[1].1),
                v2(0.14, 0.1),
                v2(0.1, 0.07),
            );
        }

        // The tip pod: a white body, a black nose, a dark tail cone.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        if b.fine() {
            b.cylinder_between(POD + Vec3::X * -3.7, POD + Vec3::X * -0.7, 0.34, 0.34, 10);
            b.paint(ACCENT);
            b.cylinder_between(POD + Vec3::X * -0.7, POD + Vec3::X * 0.05, 0.34, 0.05, 10);
            b.paint(PLATING_DARK);
            b.cylinder_between(POD + Vec3::X * -3.7, POD + Vec3::X * -4.3, 0.34, 0.1, 10);
        } else {
            b.cylinder_between(POD + Vec3::X * -4.3, POD + Vec3::X * 0.05, 0.3, 0.2, 6);
        }
        // The big laser head, on a dark plinth on the pod.
        b.paint(PLATING_DARK);
        b.cuboid(v3(LASER.x, POD.y, POD.z + 0.34), v3(1.2, 0.46, 0.14));
        pd_laser(b, LASER, 0.56, Some(POD.z + 0.34));

        // Engine nacelle on the flank, under the rear wing, on a dark pylon.
        let intake = v3(-2.2, NOZZLE.y, NOZZLE.z);
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.cylinder_between(intake, NOZZLE + Vec3::X * 0.4, 0.44, 0.4, b.sides(12));
        b.paint(METAL);
        b.cylinder_between(NOZZLE + Vec3::X * 0.4, NOZZLE, 0.38, 0.32, b.sides(10));
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(intake + Vec3::X * 0.01, intake, 0.34, 0.34, 10);
            b.cylinder_between(NOZZLE, NOZZLE + Vec3::X * 0.01, 0.24, 0.24, 8);
            b.paint(PLATING_DARK);
            b.extrude_y(
                &[[-2.6, 1.2], [-4.6, 1.2], [-4.6, 1.6], [-2.8, 1.6]],
                0.3,
                NOZZLE.y - 0.3,
            );
        }
    });
}

/// Fences and a root fairing on the forward wing, blade antennae, a satcom blister,
/// the tip pods' fins and navigation lights, strobes, and the nacelles' intake cones.
fn detail(b: &mut MeshBuilder) {
    let fine = b.fine();
    // Navigation lights on the tip pods' outer flanks: red to port (+y), green to starboard.
    for (side, glow) in [(1.0, GLOW_NAV_RED), (-1.0, GLOW_NAV_GREEN)] {
        b.paint(glow);
        b.cuboid(v3(-0.9, side * (POD.y + 0.33), POD.z), v3(0.28, 0.06, 0.12));
    }
    b.paint(GLOW_RED);
    b.cuboid(v3(-2.45, 0.0, 0.45), v3(0.22, 0.16, 0.08));
    if !fine {
        return;
    }
    b.cuboid(v3(-5.3, 0.0, 3.07), v3(0.2, 0.14, 0.08));

    // Blade antennae on the spine and the belly, a satcom blister behind the dome.
    b.paint(ACCENT);
    b.extrude_y(
        &[[3.5, 1.85], [3.0, 1.85], [2.8, 2.38], [2.98, 2.38]],
        -0.03,
        0.03,
    );
    b.extrude_y(
        &[[-1.2, 0.52], [-1.6, 0.52], [-1.85, 0.12], [-1.68, 0.12]],
        -0.03,
        0.03,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.spheroid(v3(-2.55, 0.0, 1.6), v3(0.5, 0.34, 0.16), 8, 2);

    b.mirror_y(|b| {
        // Root fairing blending the forward wing into the body.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.spheroid(v3(1.3, 0.5, 0.74), v3(1.7, 0.34, 0.24), 8, 3);
        // Two stall fences over the forward wing.
        b.paint(PLATING_DARK);
        for y in [2.7, 5.1] {
            let [r, t] = FORE;
            let f = (y - r.0) / (t.0 - r.0);
            let lerp = |a: f32, c: f32| a + (c - a) * f;
            let (z, lead, trail, th) = (
                lerp(r.1, t.1),
                lerp(r.2, t.2),
                lerp(r.3, t.3),
                lerp(r.4, t.4),
            );
            b.beam(
                v3(lead + 0.05, y, z + th * 0.35),
                v3(trail + 0.1, y, z + th * 0.3),
                v2(0.04, 0.22),
                v2(0.04, 0.12),
            );
        }
        // Small fins over and under the tip pod's tail.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        for dir in [1.0, -1.0] {
            let z = |dz: f32| POD.z + dir * dz;
            b.extrude_y(
                &[
                    [-3.55, z(0.28)],
                    [-4.25, z(0.2)],
                    [-4.35, z(0.72)],
                    [-4.05, z(0.72)],
                ],
                POD.y - 0.03,
                POD.y + 0.03,
            );
        }
        // An intake cone in each nacelle.
        let intake = v3(-2.2, NOZZLE.y, NOZZLE.z);
        b.paint(METAL);
        b.cylinder_between(
            intake + Vec3::X * 0.28,
            intake - Vec3::X * 0.1,
            0.02,
            0.2,
            8,
        );
    });
}

/// Sonar: a boom out of the tail with a ringed hydrophone head on its end.
fn sonar(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.cylinder_between(
        v3(-6.2, 0.0, 1.17),
        v3(-8.3, 0.0, 1.17),
        0.14,
        0.11,
        b.sides(8),
    );
    b.paint(ACCENT);
    b.cylinder_between(
        v3(-8.3, 0.0, 1.17),
        v3(-9.3, 0.0, 1.17),
        0.2,
        0.2,
        b.sides(8),
    );
    b.spheroid(v3(-9.3, 0.0, 1.17), v3(0.28, 0.2, 0.2), b.sides(8), 2);
    if fine {
        b.paint(METAL);
        b.cylinder_between(v3(-8.3, 0.0, 1.17), v3(-8.4, 0.0, 1.17), 0.22, 0.22, 8);
        b.paint(PLATING);
        cruciform(
            b,
            1.17,
            &[[-7.6, 0.0], [-8.2, 0.0], [-8.2, 0.3], [-7.95, 0.3]],
        );
    }
}

/// Four fins round a boom along x at height `z`: `fin` is one fin in side view, x and
/// height off the boom's axis.
fn cruciform(b: &mut MeshBuilder, z: f32, fin: &[[f32; 2]]) {
    for k in 0..4 {
        let turn = Affine3A::from_translation(Vec3::Z * z)
            * Affine3A::from_rotation_x(k as f32 * std::f32::consts::FRAC_PI_2);
        b.with(turn, |b| b.extrude_y(fin, -0.02, 0.02));
    }
}

/// The personal shield's projector (`set_shield_emitter`): a gold lens on the spine
/// ahead of the dome pylon.
fn shield(b: &mut MeshBuilder) {
    b.set_shield_emitter(SHIELD);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.spheroid(v3(2.35, 0.0, 1.9), v3(0.7, 0.3, 0.14), b.sides(8), 2);
    b.paint(METAL);
    b.prism(v3(SHIELD.x, 0.0, 1.84), b.sides(10), 0.34, 0.27, 0.18);
    b.paint(GLOW_SHIELD);
    b.spheroid(SHIELD - Vec3::Z * 0.05, v3(0.23, 0.23, 0.1), b.sides(8), 3);
}

/// The rotodome on its pylon: a lens, graphite underneath and white on top, turning
/// (`part::SPINNER`) with a dark band and the owner's stripe across it.
fn rotodome(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.extrude_y(&PYLON, -0.13, 0.13);
    b.paint(METAL);
    b.prism(DOME - Vec3::Z * 0.35, b.sides(8), 0.32, 0.28, 0.2);
    b.set_spinner_pivot(DOME);
    b.with_part(part::SPINNER, |b| {
        let lens = ngon(if b.fine() { 18 } else { 10 }, DOME_RADIUS);
        b.at(v3(DOME.x, DOME.y, 0.0), |b| {
            b.paint(PLATING_DARK);
            b.loft_z(
                &lens,
                &[
                    Section::new(DOME.z - 0.26, 0.66),
                    Section::new(DOME.z - 0.08, 1.0),
                    Section::new(DOME.z + 0.04, 1.0),
                ],
            );
            b.paint(PLATING);
            b.loft_z(
                &lens,
                &[
                    Section::new(DOME.z + 0.04, 1.0),
                    Section::new(DOME.z + 0.24, 0.66),
                ],
            );
        });
        let top = DOME.z + 0.24;
        b.paint(PLATING_DARK);
        b.cuboid(DOME.with_z(top + 0.02), v3(2.7, 0.62, 0.04));
        b.paint(TEAM);
        b.cuboid(DOME.with_z(top + 0.04), v3(2.45, 0.18, 0.04));
    });
}

/// Far away: the body, the diamond of the joined wing, the dome and the fin.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[4], HULL[7]];
    b.paint(PLATING);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        let plan = |w: [(f32, f32, f32, f32, f32); 2]| {
            let [r, t] = w;
            [
                v3(r.2, r.0, r.1),
                v3(t.2, t.0, t.1),
                v3(t.3, t.0, t.1),
                v3(r.3, r.0, r.1),
            ]
        };
        b.face(&plan(FORE));
        b.face(&plan(AFT));
        // The tip pod.
        b.paint(PLATING_DARK);
        b.face(&[
            v3(0.0, POD.y - 0.3, 1.5),
            v3(0.0, POD.y + 0.3, 1.5),
            v3(-4.2, POD.y + 0.3, 1.5),
            v3(-4.2, POD.y - 0.3, 1.5),
        ]);
    });
    b.paint(PLATING);
    let fin: Vec<Vec3> = FIN.iter().map(|p| v3(p[0], 0.0, p[1])).collect();
    b.face(&fin);
    b.face(&fin.iter().rev().copied().collect::<Vec<_>>());
    b.set_spinner_pivot(DOME);
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING_DARK);
        b.face(
            &ngon(8, DOME_RADIUS)
                .iter()
                .map(|p| v3(DOME.x + p[0], p[1], DOME.z))
                .collect::<Vec<_>>(),
        );
        b.paint(TEAM);
        b.decal(DOME + Vec3::Z * 0.02, v2(2.45, 0.3));
    });
}

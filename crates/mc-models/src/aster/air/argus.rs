//! Argus: the tech 2 radar and missile-defence picket, a high-loitering sensor drone.
//! Its wing is joined: a low forward wing sweeps back and a high rear wing, carried
//! on the fin, sweeps forward and down to meet it at a pod on each tip, so from
//! above it is a diamond round a long white body, a shape no other aircraft has.
//! A lens rotodome turns on a pylon over the back, the forward wings' leading edges
//! carry black conformal arrays, a sensor ball hangs under the chin and a radar
//! canoe runs along the belly. Its missile defence is a red laser head on each
//! wingtip pod, where the beams leave (`anti_missile_mounts` in air.ron).
use super::super::naval::pd_laser;
use super::*;
use crate::builder::ngon;

/// The (left) laser head's centre: keep the Argus's `anti_missile_mounts` in step.
const LASER: Vec3 = Vec3::new(-2.2, 7.85, 1.95);
/// Where the (left) engine's exhaust ends (`models::aircraft_exhausts`).
const NOZZLE: Vec3 = Vec3::new(-5.35, 1.05, 1.4);
/// Both exhausts, for the effect renderer.
pub(crate) const NOZZLES: [[f32; 3]; 2] = [
    [NOZZLE.x, -NOZZLE.y, NOZZLE.z],
    [NOZZLE.x, NOZZLE.y, NOZZLE.z],
];
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

/// What sits under, on and behind the airframe: the base Argus has none of it; the
/// design variants (`support_air~a/b/c`) each try one reclaim head, one sonar, one
/// shield projector and one laser layout, on a hull drawn [`Kit::scale`] times bigger.
struct Kit {
    scale: f32,
    ray: Ray,
    sonar: Sonar,
    shield: Shield,
    lasers: Lasers,
    /// Fences, fairings, antennae, lights and the rest of the added detail.
    detail: bool,
}

/// The reclaim head: a gun house of its own under the belly (weapon slot 0), turning
/// with the weapon's yaw, and what is inside `with_recoil` pitching down at the work.
#[derive(Clone, Copy, PartialEq)]
enum Ray {
    None,
    /// A ball turret in a yoke, a short emitter snout out of the ball.
    Ball,
    /// A salvage dish, the Trawler's kin, under a ventral gondola.
    Dish,
    /// A long lance with coil rings on a fork, a counterweight behind the trunnion.
    Lance,
}

#[derive(Clone, Copy, PartialEq)]
enum Sonar {
    None,
    /// A boom out of the tail with a ringed hydrophone head on its end.
    Stinger,
    /// A dipping sonar body stowed in a well in the aft belly.
    Dipping,
    /// A reel under the tail and a towed sonar fish streaming behind on its cable.
    Towed,
}

#[derive(Clone, Copy, PartialEq)]
enum Shield {
    None,
    /// A gold lens on the spine ahead of the dome pylon.
    Spine,
    /// A projector pod on the fin tip, its gold lens looking forward.
    Fin,
    /// A gold band round the rotodome's rim: the lens throws the field.
    Rim,
}

#[derive(Clone, Copy, PartialEq)]
enum Lasers {
    /// The base Argus's head on each tip pod.
    Base,
    /// One bigger head on each tip pod, on a dark plinth.
    Big,
    /// A head over and a head under each tip pod.
    Stacked,
    /// Two heads along the top of each tip pod, fore and aft.
    Paired,
}

const BASE: Kit = Kit {
    scale: 1.0,
    ray: Ray::None,
    sonar: Sonar::None,
    shield: Shield::None,
    lasers: Lasers::Base,
    detail: false,
};
/// The variants are drawn this much bigger than the base Argus.
const SCALE: f32 = 1.15;
/// The variants' airframe sits this far up (authored metres) so the reclaim head and
/// the sonar hanging under the belly stay above the model's origin.
const LIFT: f32 = 1.2;
const KIT_A: Kit = Kit {
    scale: SCALE,
    ray: Ray::Ball,
    sonar: Sonar::Stinger,
    shield: Shield::Spine,
    lasers: Lasers::Big,
    detail: true,
};
const KIT_B: Kit = Kit {
    scale: SCALE,
    ray: Ray::Dish,
    sonar: Sonar::Dipping,
    shield: Shield::Fin,
    lasers: Lasers::Stacked,
    detail: true,
};
const KIT_C: Kit = Kit {
    scale: SCALE,
    ray: Ray::Lance,
    sonar: Sonar::Towed,
    shield: Shield::Rim,
    lasers: Lasers::Paired,
    detail: true,
};

/// Reclaim head pivots (authored, before [`Kit::scale`]) and the emitter each points
/// from when level and facing forward.
const BALL: Vec3 = Vec3::new(0.3, 0.0, -0.1);
const BALL_EMITTER: Vec3 = Vec3::new(1.36, 0.0, -0.1);
const DISH: Vec3 = Vec3::new(1.6, 0.0, -0.45);
const DISH_EMITTER: Vec3 = Vec3::new(2.65, 0.0, -0.45);
const LANCE: Vec3 = Vec3::new(0.5, 0.0, -0.15);
const LANCE_EMITTER: Vec3 = Vec3::new(3.0, 0.0, -0.15);
/// Laser heads (left side) for the variants' layouts.
const LASER_BIG: Vec3 = Vec3::new(-2.2, 7.85, 2.08);
const LASER_TOP: Vec3 = Vec3::new(-2.2, 7.85, 2.0);
const LASER_FORE: Vec3 = Vec3::new(-1.0, 7.85, 1.98);
const LASER_AFT: Vec3 = Vec3::new(-3.2, 7.85, 1.98);
/// Shield projector lenses.
const SHIELD_SPINE: Vec3 = Vec3::new(1.7, 0.0, 2.08);
const SHIELD_FIN: Vec3 = Vec3::new(-4.05, 0.0, 3.1);

pub(super) fn build(b: &mut MeshBuilder) {
    body(b, &BASE);
}

pub(super) fn build_a(b: &mut MeshBuilder) {
    build_kit(b, &KIT_A);
}

pub(super) fn build_b(b: &mut MeshBuilder) {
    build_kit(b, &KIT_B);
}

pub(super) fn build_c(b: &mut MeshBuilder) {
    build_kit(b, &KIT_C);
}

fn build_kit(b: &mut MeshBuilder, kit: &Kit) {
    let frame =
        Affine3A::from_scale(Vec3::splat(kit.scale)) * Affine3A::from_translation(Vec3::Z * LIFT);
    b.with(frame, |b| body(b, kit));
}

fn body(b: &mut MeshBuilder, kit: &Kit) {
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
        match kit.ray {
            Ray::None => b.beam(
                v3(3.2, 0.0, 0.4),
                v3(-2.2, 0.0, 0.44),
                v2(0.52, 0.26),
                v2(0.4, 0.2),
            ),
            Ray::Ball | Ray::Lance => b.beam(
                v3(3.2, 0.0, 0.4),
                v3(1.25, 0.0, 0.42),
                v2(0.52, 0.26),
                v2(0.46, 0.22),
            ),
            Ray::Dish => {}
        }
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

    rotodome(b, kit);
    if kit.detail {
        detail(b);
    }
    ray(b, kit.ray);
    sonar(b, kit.sonar);
    shield(b, kit.shield);

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
        lasers(b, kit.lasers);

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

/// The missile-defence heads on the (left) tip pod.
fn lasers(b: &mut MeshBuilder, lasers: Lasers) {
    match lasers {
        Lasers::Base => pd_laser(b, LASER, 0.4, Some(POD.z + 0.25)),
        Lasers::Big => {
            // A dark plinth on the pod under the bigger head.
            b.paint(PLATING_DARK);
            b.cuboid(v3(LASER_BIG.x, POD.y, POD.z + 0.34), v3(1.2, 0.46, 0.14));
            pd_laser(b, LASER_BIG, 0.56, Some(POD.z + 0.34));
        }
        Lasers::Stacked => {
            pd_laser(b, LASER_TOP, 0.48, Some(POD.z + 0.25));
            // The same head hung under the pod, for the lower hemisphere.
            let flip = Affine3A::from_translation(POD)
                * Affine3A::from_rotation_x(std::f32::consts::PI)
                * Affine3A::from_translation(-POD);
            b.with(flip, |b| pd_laser(b, LASER_TOP, 0.48, Some(POD.z + 0.25)));
        }
        Lasers::Paired => {
            b.paint(PLATING_DARK);
            b.cuboid(v3(-2.1, POD.y, POD.z + 0.32), v3(3.0, 0.3, 0.1));
            pd_laser(b, LASER_FORE, 0.46, Some(POD.z + 0.3));
            pd_laser(b, LASER_AFT, 0.46, Some(POD.z + 0.3));
        }
    }
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

/// The reclaim head under the belly: the part that only turns inside `with_house`, the
/// part that pitches down at the work inside `with_recoil`.
fn ray(b: &mut MeshBuilder, ray: Ray) {
    let fine = b.fine();
    match ray {
        Ray::None => {}
        Ray::Ball => {
            let p = BALL;
            b.paint(PLATING_DARK);
            b.prism(v3(p.x, 0.0, 0.2), b.sides(10), 0.5, 0.56, 0.3);
            // A glazed band round the collar: the salvage flowing up into the hull.
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.prism(v3(p.x, 0.0, 0.26), b.sides(10), 0.575, 0.585, 0.12);
            b.with_house(0, p, 0.0, |b| {
                b.paint(METAL);
                b.prism(v3(p.x, 0.0, 0.08), b.sides(10), 0.64, 0.62, 0.12);
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.mirror_y(|b| {
                    b.block(v3(p.x - 0.22, 0.52, p.z - 0.12), v3(p.x + 0.22, 0.64, 0.1));
                });
                b.with_recoil(|b| {
                    b.paint(ACCENT);
                    b.spheroid(p, Vec3::splat(0.47), b.sides(10), if fine { 5 } else { 3 });
                    b.paint(METAL);
                    b.cylinder_between(
                        p + Vec3::X * 0.3,
                        p + Vec3::X * 1.0,
                        0.2,
                        0.16,
                        b.sides(10),
                    );
                    b.paint(GLOW_MATERIALS);
                    b.cylinder_between(p + Vec3::X * 1.0, BALL_EMITTER, 0.14, 0.1, b.sides(8));
                    if fine {
                        b.paint(ACCENT);
                        b.cylinder_between(p + Vec3::X * 0.62, p + Vec3::X * 0.72, 0.24, 0.24, 8);
                        b.cylinder_between(p + Vec3::X * 0.9, p + Vec3::X * 1.0, 0.22, 0.2, 8);
                        // A sight box on the ball.
                        b.block(p + v3(0.05, 0.2, 0.3), p + v3(0.45, 0.36, 0.46));
                        b.paint(GLASS);
                        b.block(p + v3(0.45, 0.22, 0.32), p + v3(0.47, 0.34, 0.44));
                    }
                });
            });
        }
        Ray::Dish => {
            // The gondola the dish hangs under, faired into the belly, with a glazed
            // window down each flank where the salvage runs aft.
            b.paint(PLATING_DARK);
            b.spheroid(
                v3(0.5, 0.0, 0.2),
                v3(2.6, 0.5, 0.38),
                b.sides(10),
                if fine { 4 } else { 3 },
            );
            if fine {
                b.paint(ACCENT).pattern(pattern::MASS_FLOW);
                b.mirror_y(|b| {
                    b.beam(
                        v3(1.8, 0.4, 0.2),
                        v3(-0.9, 0.4, 0.22),
                        v2(0.08, 0.16),
                        v2(0.08, 0.12),
                    );
                });
            }
            let p = DISH;
            b.paint(METAL);
            b.prism(v3(p.x, 0.0, -0.18), b.sides(8), 0.36, 0.3, 0.1);
            b.with_house(0, p, 0.0, |b| {
                b.paint(METAL);
                b.prism(v3(p.x, 0.0, -0.26), b.sides(10), 0.5, 0.48, 0.1);
                b.with_recoil(|b| {
                    b.paint(ACCENT);
                    b.chamfered_box(p, v3(0.7, 0.72, 0.44), 0.12);
                    b.paint(PLATING);
                    b.plate(p + Vec3::Z * 0.22, v2(0.5, 0.5), 0.05, 0.02);
                    b.paint(ACCENT);
                    let (a, e) = (p + Vec3::X * 0.3, DISH_EMITTER);
                    b.cylinder_between(a, e, 0.2, 0.58, b.sides(10));
                    b.paint(GLOW_MATERIALS);
                    b.cylinder_between(a + Vec3::X * 0.35, a + Vec3::X * 0.4, 0.3, 0.3, b.sides(8));
                    if fine {
                        // Claw prongs round the dish's mouth.
                        b.paint(ACCENT);
                        for k in 0..3 {
                            let ang = k as f32 * std::f32::consts::TAU / 3.0
                                + std::f32::consts::FRAC_PI_2;
                            let r = Vec3::Y * ang.cos() + Vec3::Z * ang.sin();
                            b.beam(
                                e + r * 0.5,
                                e + r * 0.62 + Vec3::X * 0.4,
                                v2(0.1, 0.1),
                                v2(0.06, 0.06),
                            );
                        }
                    }
                });
            });
        }
        Ray::Lance => {
            let p = LANCE;
            b.paint(PLATING_DARK);
            b.prism(v3(p.x, 0.0, 0.3), b.sides(8), 0.38, 0.42, 0.2);
            b.with_house(0, p, 0.0, |b| {
                b.paint(METAL);
                b.prism(v3(p.x, 0.0, 0.2), b.sides(10), 0.46, 0.44, 0.1);
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.mirror_y(|b| {
                    b.block(v3(p.x - 0.18, 0.3, p.z - 0.14), v3(p.x + 0.18, 0.4, 0.2));
                });
                b.with_recoil(|b| {
                    b.paint(METAL);
                    b.cylinder_between(
                        p - Vec3::Y * 0.32,
                        p + Vec3::Y * 0.32,
                        0.13,
                        0.13,
                        b.sides(8),
                    );
                    b.paint(ACCENT);
                    b.beam(
                        p - Vec3::X * 0.34,
                        p + Vec3::X * 0.55,
                        v2(0.44, 0.36),
                        v2(0.36, 0.3),
                    );
                    if fine {
                        // Glazed flanks on the housing: the salvage coming in.
                        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
                        b.mirror_y(|b| {
                            b.block(p + v3(-0.25, 0.2, -0.1), p + v3(0.45, 0.23, 0.1));
                        });
                    }
                    b.paint(METAL);
                    b.cylinder_between(
                        p + Vec3::X * 0.55,
                        p + Vec3::X * 2.3,
                        0.12,
                        0.1,
                        b.sides(8),
                    );
                    b.paint(ACCENT);
                    b.cylinder_between(p + Vec3::X * 2.3, LANCE_EMITTER, 0.12, 0.2, b.sides(8));
                    b.paint(GLOW_MATERIALS);
                    b.cylinder_between(
                        LANCE_EMITTER - Vec3::X * 0.04,
                        LANCE_EMITTER,
                        0.13,
                        0.13,
                        b.sides(8),
                    );
                    if fine {
                        // Coil rings down the lance.
                        b.paint(PLATING_DARK);
                        for x in [1.0, 1.4, 1.8] {
                            b.cylinder_between(
                                p + Vec3::X * x,
                                p + Vec3::X * (x + 0.1),
                                0.18,
                                0.18,
                                8,
                            );
                        }
                        b.paint(GLOW_MATERIALS);
                        b.cylinder_between(p + Vec3::X * 2.1, p + Vec3::X * 2.14, 0.14, 0.14, 8);
                    }
                });
            });
        }
    }
}

/// Something that reads as sonar: the Argus hunts dived hulls too.
fn sonar(b: &mut MeshBuilder, sonar: Sonar) {
    let fine = b.fine();
    match sonar {
        Sonar::None => {}
        Sonar::Stinger => {
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
        Sonar::Dipping => {
            let (x, top) = (-3.3, 0.5);
            b.paint(ACCENT);
            b.prism(v3(x, 0.0, 0.22), b.sides(8), 0.46, 0.4, top - 0.22);
            b.paint(PLATING_DARK);
            b.cylinder_between(v3(x, 0.0, 0.22), v3(x, 0.0, -0.4), 0.3, 0.3, b.sides(10));
            b.spheroid(v3(x, 0.0, -0.4), v3(0.3, 0.3, 0.16), b.sides(8), 2);
            if fine {
                b.paint(METAL);
                for z in [0.02, -0.2] {
                    b.cylinder_between(v3(x, 0.0, z), v3(x, 0.0, z - 0.06), 0.33, 0.33, 8);
                }
                // The folded hydrophone arms along the body.
                b.paint(PLATING);
                b.at(v3(x, 0.0, 0.0), |b| {
                    b.radial(4, |b| {
                        b.beam(
                            v3(0.0, 0.31, 0.15),
                            v3(0.0, 0.31, -0.42),
                            v2(0.07, 0.05),
                            v2(0.07, 0.05),
                        );
                    });
                });
            }
        }
        Sonar::Towed => {
            b.paint(PLATING_DARK);
            b.spheroid(v3(-5.4, 0.0, 0.8), v3(0.65, 0.32, 0.26), b.sides(8), 2);
            let (reel, fish) = (v3(-5.95, 0.0, 0.7), v3(-10.3, 0.0, -0.3));
            b.paint(METAL);
            b.beam(reel, fish, v2(0.05, 0.05), v2(0.05, 0.05));
            let tail = fish - Vec3::X * 1.4;
            b.paint(ACCENT);
            b.cylinder_between(fish, tail, 0.24, 0.18, b.sides(10));
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.spheroid(fish, v3(0.34, 0.24, 0.24), b.sides(8), 2);
            if fine {
                b.paint(METAL);
                b.cylinder_between(fish - Vec3::X * 0.5, fish - Vec3::X * 0.58, 0.26, 0.25, 8);
                b.paint(PLATING);
                b.at(Vec3::X * tail.x, |b| {
                    cruciform(
                        b,
                        tail.z,
                        &[[0.55, 0.15], [0.0, 0.15], [-0.05, 0.5], [0.25, 0.5]],
                    );
                });
            }
        }
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

/// The personal shield's projector (`set_shield_emitter`); the rotodome's rim band is
/// drawn with the dome.
fn shield(b: &mut MeshBuilder, shield: Shield) {
    match shield {
        Shield::None => {}
        Shield::Spine => {
            b.set_shield_emitter(SHIELD_SPINE);
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.spheroid(v3(2.35, 0.0, 1.9), v3(0.7, 0.3, 0.14), b.sides(8), 2);
            b.paint(METAL);
            b.prism(v3(SHIELD_SPINE.x, 0.0, 1.84), b.sides(10), 0.34, 0.27, 0.18);
            b.paint(GLOW_SHIELD);
            b.spheroid(
                SHIELD_SPINE - Vec3::Z * 0.05,
                v3(0.23, 0.23, 0.1),
                b.sides(8),
                3,
            );
        }
        Shield::Fin => {
            b.set_shield_emitter(SHIELD_FIN);
            let at = SHIELD_FIN;
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.cylinder_between(
                at - Vec3::X * 0.12,
                at - Vec3::X * 1.5,
                0.17,
                0.12,
                b.sides(8),
            );
            b.paint(METAL);
            b.cylinder_between(
                at - Vec3::X * 0.04,
                at - Vec3::X * 0.14,
                0.15,
                0.18,
                b.sides(8),
            );
            b.paint(GLOW_SHIELD);
            b.spheroid(at, v3(0.08, 0.14, 0.14), b.sides(8), 2);
        }
        Shield::Rim => b.set_shield_emitter(DOME + Vec3::Z * 0.3),
    }
}

/// The rotodome on its pylon: a lens, graphite underneath and white on top, turning
/// (`part::SPINNER`) with a dark band and the owner's stripe across it.
fn rotodome(b: &mut MeshBuilder, kit: &Kit) {
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
            if kit.shield == Shield::Rim {
                // The shield band round the lens's rim.
                b.paint(GLOW_SHIELD);
                b.loft_z(
                    &lens,
                    &[
                        Section::new(DOME.z - 0.07, 1.015),
                        Section::new(DOME.z + 0.03, 1.015),
                    ],
                );
            }
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

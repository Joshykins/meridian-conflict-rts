//! The Naga engineer, the Tender: a small, low, six-legged crawler, a worker cousin of the
//! Sovereign. A pointed head under a short brow with a cluster of red eyes and a pair of
//! working mandibles, a pinched neck, then three arched back plates over a swollen
//! spinneret gland, and a short jointed tail curled up over its back whose tip carries a
//! fabricator prong reaching forward over the head: the build beam leaves its tip.
//!
//! Every plate is black hide (`PLATING_DARK` under `pattern::EMBER`), the soft hide between
//! them lit red at its seams (`ACCENT` under `EMBER`), bare metal at the joints and in the
//! bones the leg plates ride on.
//!
//! Rig (as the Sovereign's, `commander.rs`): the body is `HULL`. Each leg is two bones posed
//! by `entity.wgsl` `crawl_leg` (`MeshBuilder::set_crawl_legs`), pair by pair, the six
//! feet in two alternating tripods. The tail is the turret: segment `i` turns about joint
//! `i` of `TAIL` (`tail_pose`); the spinneret and its prong ride the last joint. Its top
//! four joints bend round to the work within the unit file's `aim_arc`, and past that the
//! body turns; the sim turns the prong's tip about the one point that best matches that
//! chain (`turret_at`, `AIM_PIVOT`). The prong pitches about its joint (`rig::ARM_TOOL`).
//! The numbers match `naga_t1_engineer` in `data/factions/naga/units/command.ron`.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, rig};

use super::kit::*;

/// The fabricator prong's joint and its tip, where the build beam leaves: the unit file's
/// `builder.arm.pivot` and `builder.arm.emitter`.
const PRONG_PIVOT: Vec3 = Vec3::new(-0.55, 0.0, 3.75);
const PRONG_TIP: Vec3 = Vec3::new(1.0, 0.0, 3.3);

/// The tail's spine, root on the socket to where the spinneret rides, and the middle of
/// its curl (for which way is "outside").
const TAIL: [Vec3; 6] = [
    Vec3::new(-1.5, 0.0, 1.6),
    Vec3::new(-2.15, 0.0, 2.0),
    Vec3::new(-2.6, 0.0, 2.6),
    Vec3::new(-2.5, 0.0, 3.25),
    Vec3::new(-2.0, 0.0, 3.65),
    Vec3::new(-1.35, 0.0, 3.8),
];
const CURL: Vec3 = Vec3::new(-1.9, 0.0, 2.75);
/// Half widths of the tail's armour at each joint, root to tip.
const TAIL_WIDTH: [f32; 6] = [0.42, 0.39, 0.36, 0.33, 0.3, 0.28];
/// How the aim is shared over the tail's top joints, lowest first (`entity.wgsl`
/// `tail_pose`).
#[cfg(test)]
const AIM_SHARE: [f32; 4] = [0.1, 0.2, 0.3, 0.4];
/// The single point the prong swings about that best matches that chain over the aim
/// arc: the unit file's `turret_at`, and the model's turret pivot.
const AIM_PIVOT: f32 = -1.9;

/// Left legs, front to back: hip, knee, the foot's tip, and where in the cycle it lifts.
/// Two alternating tripods (front and rear left with the middle right), a little out of
/// step so it skitters.
const LEGS: [(Vec3, Vec3, Vec3, f32); 3] = [
    (Vec3::new(1.05, 0.55, 1.2), Vec3::new(1.75, 1.35, 1.95), Vec3::new(2.55, 2.35, 0.0), 0.0),
    (Vec3::new(0.05, 0.7, 1.2), Vec3::new(0.2, 1.7, 2.0), Vec3::new(0.35, 2.95, 0.0), 0.5),
    (Vec3::new(-0.95, 0.65, 1.25), Vec3::new(-1.45, 1.5, 1.95), Vec3::new(-2.25, 2.55, 0.0), 0.06),
];

/// The back plates, front to back: middle x, half length, half width, base height and how
/// high its arch stands.
const TERGITES: [(f32, f32, f32, f32, f32); 3] = [
    (0.3, 0.3, 0.95, 1.55, 0.36),
    (-0.42, 0.3, 1.02, 1.6, 0.38),
    (-1.1, 0.26, 0.82, 1.64, 0.32),
];

/// A small plate's cross-section: an arch over a flat underside, fewer sides than kit's
/// `shell_ring` (a leg plate a hand wide needs no more).
fn plate_ring(b: &MeshBuilder, c: Vec3, side: Vec3, up: Vec3, w: f32, h: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() { &[[1.0, -0.6], [-1.0, -0.6], [-0.7, 0.55], [0.0, 1.0], [0.7, 0.55]] } else { &[[1.0, -0.6], [-1.0, -0.6], [0.0, 1.0]] };
    shape.iter().map(|[s, u]| c + side * (s * w) + up * (u * h)).collect()
}

/// A small run of hide's cross-section: a keeled five-sided tube (three-sided at mid).
fn core_ring(b: &MeshBuilder, c: Vec3, side: Vec3, up: Vec3, w: f32, h: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() { &[[1.0, -0.3], [0.5, -1.0], [-0.5, -1.0], [-1.0, -0.3], [0.0, 1.0]] } else { &[[1.0, -0.5], [-1.0, -0.5], [0.0, 1.0]] };
    shape.iter().map(|[s, u]| c + side * (s * w) + up * (u * h)).collect()
}

/// A small plate along `points` (`plate_ring`).
fn plate(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    let r = rings(b, points, hint, plate_ring);
    b.loft(&r, true, true);
}

/// A small run of hide along `points` (`core_ring`).
fn core(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    let r = rings(b, points, hint, core_ring);
    b.loft(&r, true, true);
}

pub fn tender(b: &mut MeshBuilder, _tech: u8) {
    b.set_crawl_legs(&LEGS, 4.0, 0.55, 0.5);
    b.set_tail(&TAIL, TAIL[TAIL.len() - 1].z - 0.1);
    // Where the sim turns the prong's tip (`turret_at` in the unit file). The shader bends
    // the tail's top joints instead of swivelling about it.
    b.set_turret_pivot(v3(AIM_PIVOT, 0.0, PRONG_PIVOT.z));
    b.set_arm_pivot(PRONG_PIVOT);
    b.set_dust_line(1.0);

    if b.coarse() {
        coarse(b);
        return;
    }
    body(b);
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in LEGS.iter().enumerate() {
            b.with_pair(i, |b| leg(b, hip, knee, foot));
        }
    });
    tail(b);
    spinneret(b);
}

/// Far off: a wedge of a body, flat legs that do not walk, the tail in two bars and the
/// prong as a spike.
fn coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum(v3(0.3, 0.0, 0.85), Vec2::new(4.2, 1.7), Vec2::new(3.0, 1.1), 0.95, Vec2::new(-0.1, 0.0));
    b.paint(TEAM);
    b.face(&[v3(0.6, 0.0, 1.81), v3(-0.2, 0.35, 1.81), v3(-0.2, -0.35, 1.81)]);
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            for &(hip, knee, foot, _) in &LEGS {
                b.face(&[hip, foot, knee]);
                b.face(&[hip, knee, foot]);
            }
        });
    });
    let bar = |b: &mut MeshBuilder, a: Vec3, c: Vec3, wa: f32, wc: f32| {
        let (side, up) = frame(c - a, (a + c) * 0.5 - CURL);
        let tri = |p: Vec3, w: f32| vec![p + up * w, p + side * w - up * (w * 0.6), p - side * w - up * (w * 0.6)];
        b.loft(&[tri(a, wa), tri(c, wc)], true, true);
    };
    b.with_tail(0, |b| {
        hide(b);
        bar(b, TAIL[0], TAIL[3], 0.45, 0.36);
    });
    b.with_tail(3, |b| {
        hide(b);
        bar(b, TAIL[3], TAIL[5], 0.36, 0.3);
    });
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_TOOL, |b| {
            hide(b);
            let root = [v3(-1.2, 0.3, 3.8), v3(-1.2, -0.3, 3.8), v3(-1.2, 0.0, 4.15)];
            b.loft(&[root.to_vec(), vec![PRONG_TIP; 3]], true, false);
        });
    });
}

fn body(b: &mut MeshBuilder) {
    // The soft hide the plates ride on, its seams glowing: mandibles to the tail's socket.
    under_hide(b);
    segment(
        b,
        &[
            (v3(2.55, 0.0, 1.12), 0.22, 0.14),
            (v3(2.15, 0.0, 1.25), 0.52, 0.28),
            (v3(1.55, 0.0, 1.3), 0.64, 0.33),
            (v3(0.95, 0.0, 1.3), 0.5, 0.28),
            (v3(0.55, 0.0, 1.34), 0.8, 0.4),
            (v3(-0.4, 0.0, 1.4), 0.95, 0.45),
            (v3(-1.2, 0.0, 1.46), 0.72, 0.4),
            (v3(-1.7, 0.0, 1.55), 0.42, 0.3),
        ],
        Vec3::Z,
    );
    head(b);
    abdomen(b);
    // The coxae: a drum at each hip the leg turns in. Close up only.
    if b.fine() {
        b.mirror_y(|b| {
            for &(hip, _, foot, _) in &LEGS {
                let out = (foot - hip).truncate().extend(0.0).normalize();
                under_hide(b);
                b.cylinder_between(hip - out * 0.28, hip + out * 0.08, 0.26, 0.22, 6);
            }
        });
    }
    // The socket the tail plugs into: an armoured collar.
    let root = TAIL[0];
    hide(b);
    let sides = b.sides(8);
    b.cylinder_between(root + v3(0.5, 0.0, -0.28), root + v3(-0.08, 0.0, 0.05), 0.55, 0.47, sides);
    // The belly: a flat plate under it all.
    under_hide(b);
    b.frustum(v3(-0.2, 0.0, 0.84), Vec2::new(2.4, 0.7), Vec2::new(2.8, 0.95), 0.3, Vec2::ZERO);
    if b.fine() {
        // A ram each side from the back to the socket, that heaves the tail up.
        b.mirror_y(|b| ram(b, v3(-1.0, 0.45, 1.85), root + v3(0.15, 0.36, 0.22), 0.06));
    }
}

/// The head: a pointed wedge under a short brow, red eyes in the dark beneath it, two
/// working mandibles below.
fn head(b: &mut MeshBuilder) {
    hide(b);
    shell(
        b,
        &[(v3(2.62, 0.0, 1.22), 0.2, 0.07), (v3(2.25, 0.0, 1.4), 0.5, 0.15), (v3(1.6, 0.0, 1.5), 0.66, 0.2), (v3(1.0, 0.0, 1.45), 0.52, 0.16)],
        Vec3::Z,
    );
    // The brow: a visor jutting over the eyes.
    shell(b, &[(v3(2.2, 0.0, 1.5), 0.38, 0.1), (v3(2.72, 0.0, 1.32), 0.2, 0.05)], Vec3::Z);
    // Eyes under the brow's lip, the middle pair biggest.
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        b.beam(v3(2.5, 0.07, 1.24), v3(2.56, 0.2, 1.21), Vec2::new(0.07, 0.05), Vec2::new(0.06, 0.04));
        if b.fine() {
            b.beam(v3(2.34, 0.28, 1.22), v3(2.36, 0.36, 1.2), Vec2::new(0.05, 0.04), Vec2::new(0.04, 0.03));
        }
    });
    b.mirror_y(|b| {
        // The mandibles: short hooked jaws under the brow, bare metal hooks at their tips.
        hide(b);
        core(b, &[(v3(2.3, 0.16, 1.02), 0.12, 0.1), (v3(2.72, 0.16, 0.96), 0.1, 0.09), (v3(2.9, 0.1, 0.86), 0.06, 0.06)], Vec3::Z);
        if b.fine() {
            metal(b);
            b.cylinder_between(v3(2.88, 0.1, 0.87), v3(3.06, 0.02, 0.76), 0.05, 0.01, 5);
            // A horn off the brow, swept back.
            under_hide(b);
            blade(b, v3(2.0, 0.34, 1.52), v3(1.45, 0.5, 1.88), 0.09, v3(0.0, 1.0, 0.2));
        }
    });
    // The neck: a ring of bare metal between head and back.
    if b.fine() {
        metal(b);
        b.cylinder_between(v3(0.98, 0.0, 1.32), v3(0.84, 0.0, 1.33), 0.46, 0.46, 8);
    }
}

/// Half of one back plate (the left), a cross-section at `x`: an arch from the spine's
/// edge out over the flank, its rim hanging past the hide, swept back at the spine.
fn tergite_ring(b: &MeshBuilder, x: f32, w: f32, z: f32, h: f32, sweep: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() {
        &[[1.0, -0.72], [0.78, -0.3], [0.2, 0.72], [0.17, 0.95], [0.4, 0.9], [0.72, 0.6], [0.97, 0.0]]
    } else {
        &[[1.0, -0.72], [0.2, 0.72], [0.2, 0.95], [0.75, 0.6]]
    };
    shape.iter().map(|&[s, u]| v3(x - sweep * (1.0 - s), s * w, z + u * h)).collect()
}

/// The back: three arched plates either side of a bare spine, apart so the working hide
/// shows between them; under the last two the spinneret gland swells out, lit.
fn abdomen(b: &mut MeshBuilder) {
    let sweep = 0.18;
    for (i, &(x, half, w, z, h)) in TERGITES.iter().enumerate() {
        b.mirror_y(|b| {
            hide(b);
            b.loft(
                &[
                    tergite_ring(b, x + half, w * 0.92, z - 0.03, h * 0.9, sweep),
                    tergite_ring(b, x, w, z, h, sweep),
                    tergite_ring(b, x - half, w * 0.96, z + 0.03, h * 1.02, sweep),
                ],
                true,
                true,
            );
        });
        // The vertebra on the spine under the plate, bare metal.
        let top = z + h;
        metal(b);
        b.beam(v3(x + half * 0.9 - sweep, 0.0, top - 0.14), v3(x - half * 0.9 - sweep, 0.0, top - 0.1), Vec2::new(0.16, 0.14), Vec2::new(0.16, 0.16));
        if b.fine() {
            // A lit seam each side of the spine.
            if i > 0 {
                b.paint(GLOW_LASER);
                b.mirror_y(|b| {
                    b.beam(v3(x + half * 0.6 - sweep, 0.14, top - 0.13), v3(x - half * 0.6 - sweep, 0.14, top - 0.11), Vec2::new(0.03, 0.03), Vec2::new(0.03, 0.03));
                });
            }
        }
    }
    // The owner's colour: a chevron across the front plate.
    b.paint(TEAM);
    let (x, _, w, z, h) = TERGITES[0];
    b.mirror_y(|b| b.beam(v3(x - 0.26, 0.14, z + h * 0.99), v3(x - 0.08, w * 0.45, z + h * 0.88), Vec2::new(0.12, 0.025), Vec2::new(0.12, 0.025)));
    // The spinneret gland, swollen between the rear plates' rims, two red slits on it.
    b.mirror_y(|b| {
        under_hide(b);
        let (x, _, w, z, _) = TERGITES[1];
        core(b, &[(v3(x + 0.35, w * 0.72, z - 0.2), 0.12, 0.1), (v3(x - 0.2, w * 0.86, z - 0.15), 0.2, 0.17), (v3(x - 0.8, w * 0.7, z - 0.08), 0.1, 0.08)], v3(0.0, 1.0, 1.0));
        if b.fine() {
            b.paint(GLOW_LASER);
            b.beam(v3(x - 0.02, w * 0.99, z - 0.12), v3(x - 0.34, w * 0.99, z - 0.1), Vec2::new(0.025, 0.03), Vec2::new(0.025, 0.03));
        }
    });
}

/// One left leg: a plated thigh on a bare bone up to a raised knee, a plated shin down to a
/// hooked foot.
fn leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, foot: Vec3) {
    let out = (foot - hip).truncate().extend(0.0).normalize();
    // Toward the outside of the leg's bend, for the keels.
    let outside = (Vec3::Z + out * 0.4).normalize();
    let thigh = knee - hip;
    let shin = foot - knee;
    let ankle = knee + shin * 0.7;
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            if b.fine() {
                metal(b);
                b.cylinder_between(hip, knee - thigh * 0.05, 0.07, 0.06, 5);
            }
            hide(b);
            plate(b, &[(hip + thigh * 0.1 + outside * 0.06, 0.19, 0.14), (knee - thigh * 0.08 + outside * 0.05, 0.16, 0.12)], outside);
            if b.fine() {
                // The ram under the thigh that lifts it.
                let (_, up) = frame(thigh, outside);
                ram(b, hip + thigh * 0.12 - up * 0.13, knee - thigh * 0.22 - up * 0.1, 0.045);
            }
        });
        b.with_limb(rig::SHIN, |b| {
            if b.fine() {
                // The knee: a bare drum, and the bare bone of the shin under its plate.
                metal(b);
                let axis = out.cross(Vec3::Z) * 0.13;
                b.cylinder_between(knee - axis, knee + axis, 0.13, 0.13, 6);
                b.cylinder_between(knee, ankle, 0.06, 0.045, 5);
            }
            hide(b);
            plate(b, &[(knee + shin * 0.06, 0.15, 0.12), (ankle - shin * 0.02, 0.1, 0.08)], out + Vec3::Z);
            // The foot: a hooked claw into the ground.
            under_hide(b);
            let sides = b.sides(5);
            b.cylinder_between(ankle, foot + Vec3::Z * 0.005, 0.08, 0.008, sides);
            if b.fine() {
                // A spur behind the ankle, and a red seam down the shin's plate.
                spike(b, ankle, ankle.lerp(foot, 0.6) - out * 0.2 + Vec3::Z * 0.05, 0.04);
                b.paint(GLOW_LASER);
                let (_, up) = frame(shin, out + Vec3::Z);
                b.beam(knee + shin * 0.15 + up * 0.11, knee + shin * 0.5 + up * 0.08, Vec2::new(0.03, 0.02), Vec2::new(0.025, 0.02));
            }
        });
    });
}

/// The tail: five segments from the socket to the spinneret, each a working joint: a bare
/// core on a drum, an arched plate over the outside of the curl, a belly plate, stopping
/// short of both joints so each joint shows as a gap with red in it.
fn tail(b: &mut MeshBuilder) {
    for i in 0..TAIL.len() - 1 {
        let (a, c) = (TAIL[i], TAIL[i + 1]);
        let (wa, wc) = (TAIL_WIDTH[i], TAIL_WIDTH[i + 1]);
        let dir = c - a;
        let along = dir.normalize();
        let mid = (a + c) * 0.5;
        let outward = (mid - CURL).normalize();
        let (side, up) = frame(dir, outward);
        let (p0, p1) = (a + dir * 0.18, c - dir * 0.1);
        b.with_tail(i, |b| {
            under_hide(b);
            core(b, &[(a, wa * 0.5, wa * 0.5), (c, wc * 0.5, wc * 0.5)], outward);
            if b.fine() {
                // The joint's drum, bare metal.
                metal(b);
                b.cylinder_between(a - Vec3::Y * (wa * 0.85), a + Vec3::Y * (wa * 0.85), wa * 0.6, wa * 0.6, 6);
            }
            hide(b);
            let arch = |p: Vec3, w: f32, k: f32| (p + up * (w * 0.4), w * k, w * 0.72 * k);
            if b.fine() {
                shell(b, &[arch(p0, wa, 0.9), arch(a.lerp(c, 0.55), wa, 1.02), arch(p1, wc, 1.1)], outward);
            } else {
                shell(b, &[arch(p0, wa, 0.9), arch(p1, wc, 1.05)], outward);
            }
            // Close up, past the socket's collar: the belly plate, the crest and the seams.
            if !b.fine() || i == 0 {
                return;
            }
            // The belly plate on the inside of the curl.
            let belly = |p: Vec3, w: f32| (p - up * (w * 0.6), w * 0.5, w * 0.28);
            plate(b, &[belly(a + dir * 0.26, wa), belly(c - dir * 0.2, wc)], -up);
            // The crest blade on the arch, raked back toward the root.
            under_hide(b);
            let top = mid + up * (wa * 1.1);
            blade(b, top + along * 0.12, top + up * (0.2 + wa * 0.3) - along * 0.3, 0.1, side);
            // A red seam each side, in the gap between the arch and the core.
            b.paint(GLOW_LASER);
            for s in [-1.0f32, 1.0] {
                let seam = a + dir * 0.3 + side * (s * wa * 0.82) + up * (wa * 0.12);
                b.beam(seam, seam + dir * 0.4, Vec2::new(0.03, 0.03), Vec2::new(0.03, 0.03));
            }
        });
    }
}

/// The spinneret on the tail's tip: a gland under a plate, and the fabricator prong
/// reaching forward over the head from its joint, two hooked feeders either side of it.
fn spinneret(b: &mut MeshBuilder) {
    b.with_part(part::TURRET, |b| {
        let root = TAIL[TAIL.len() - 1];
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(root - Vec3::Y * 0.24, root + Vec3::Y * 0.24, 0.18, 0.18, sides);
        under_hide(b);
        core(b, &[(root, 0.24, 0.22), (v3(-1.0, 0.0, 3.9), 0.34, 0.3), (v3(-0.66, 0.0, 3.8), 0.24, 0.22)], Vec3::Z);
        hide(b);
        shell(b, &[(v3(-1.32, 0.0, 4.02), 0.3, 0.2), (v3(-1.0, 0.0, 4.08), 0.36, 0.22), (v3(-0.74, 0.0, 3.98), 0.28, 0.16)], Vec3::Z);
        b.paint(GLOW_LASER);
        b.beam(v3(-1.05, 0.0, 4.28), v3(-0.85, 0.0, 4.22), Vec2::new(0.07, 0.03), Vec2::new(0.05, 0.03));

        b.with_limb(rig::ARM_TOOL, prong);
    });
}

/// The fabricator prong from its joint to its tip: a hide sheath, a bare needle lit at
/// the tip, and (close up) two hooked feeders flanking it and a ring round the needle.
fn prong(b: &mut MeshBuilder) {
    let (pivot, tip) = (PRONG_PIVOT, PRONG_TIP);
    let bend = v3(0.1, 0.0, 3.62);
    let reach = (tip - bend).normalize();
    metal(b);
    let sides = b.sides(8);
    b.cylinder_between(pivot - Vec3::Y * 0.17, pivot + Vec3::Y * 0.17, 0.16, 0.16, sides);
    hide(b);
    core(b, &[(pivot, 0.17, 0.18), (bend, 0.14, 0.14), (bend + reach * 0.25, 0.09, 0.09)], Vec3::Z);
    metal(b);
    let sides = b.sides(6);
    b.cylinder_between(bend, tip - reach * 0.2, 0.06, 0.04, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(tip - reach * 0.24, tip, 0.05, 0.01, sides);
    if !b.fine() {
        return;
    }
    // A ring round the needle where the thread is drawn.
    metal(b);
    b.cylinder_between(bend + reach * 0.45, bend + reach * 0.52, 0.08, 0.08, 6);
    b.mirror_y(|b| {
        // A feeder hook either side of the sheath, curving forward and in.
        under_hide(b);
        let root = pivot + v3(0.25, 0.16, -0.05);
        let knee = pivot + v3(0.55, 0.26, -0.12);
        b.cylinder_between(root, knee, 0.045, 0.035, 5);
        metal(b);
        b.cylinder_between(knee, pivot + v3(0.85, 0.12, -0.3), 0.035, 0.008, 5);
    });
    // A red line down the sheath.
    b.paint(GLOW_LASER);
    b.beam(pivot + v3(0.12, 0.0, 0.17), bend + v3(-0.05, 0.0, 0.13), Vec2::new(0.03, 0.03), Vec2::new(0.025, 0.025));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model, Model};

    fn model() -> Model {
        build_model("naga_tender").unwrap()
    }

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check("naga_tender", 3.8, 3.8, None, &[]);
    }

    #[test]
    fn stands_on_six_legs_each_rigged_to_its_pair() {
        let model = model();
        let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
        assert_eq!(crawl.pairs, 3);
        for lod in &model.lods[..2] {
            for pair in 0..3u32 {
                let bones = |limb: u32| {
                    lod.vertices.iter().filter(move |v| {
                        v.part == part::LOCOMOTION && v.rig & rig::LIMB_MASK == limb && (v.rig & rig::PAIR_MASK) >> rig::PAIR_SHIFT == pair
                    })
                };
                for limb in [rig::THIGH, rig::SHIN] {
                    assert!(bones(limb).any(|v| v.pos[1] > 0.0), "pair {pair} left bone {limb}");
                    assert!(bones(limb).any(|v| v.pos[1] < 0.0), "pair {pair} right bone {limb}");
                }
                // The foot's tip reaches the ground its pair's rest pose names.
                let [_, _, foot] = crawl.joints[pair as usize];
                let low = bones(rig::SHIN).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
                assert!(low < 0.08, "pair {pair} foot at {low}");
                let reach = bones(rig::SHIN).map(|v| v.pos[1]).fold(0.0f32, f32::max);
                assert!((reach - foot[1]).abs() < 0.2, "pair {pair} reaches y {reach} for {}", foot[1]);
            }
        }
    }

    #[test]
    fn crawl_data_fits_the_gpu_slots() {
        let crawl = model().legs.unwrap().crawl.unwrap();
        assert!(crawl.pairs <= crate::models::MAX_CRAWL_PAIRS);
        // The shader shares the aim over the top four joints and needs one under them.
        assert_eq!(crawl.tail_count, TAIL.len());
        assert!(crawl.tail_count >= 5 && crawl.tail_count <= crate::models::MAX_TAIL_JOINTS);
        assert_eq!(crawl.claw, None);
        let gpu = crawl.gpu();
        assert_eq!(gpu.len(), crate::models::CRAWL_SLOTS);
        assert_eq!(gpu[0], [3.0, TAIL[0].z, TAIL[5].z - 0.1, 6.0]);
        // As `entity.wgsl` `tail_joint` reads them, from slot 13.
        for (j, p) in TAIL.iter().enumerate() {
            let v = gpu[13 + j / 2];
            let got = if j % 2 == 1 { [v[2], v[3]] } else { [v[0], v[1]] };
            assert_eq!(got, [p.x, p.z], "joint {j}");
        }
        // No pincers: `claw_pose` stays off.
        assert_eq!(gpu[19][3], 0.0);
    }

    #[test]
    fn tail_segments_turn_about_their_own_joints() {
        let model = model();
        for lod in &model.lods[..2] {
            for seg in 0..TAIL.len() - 1 {
                let verts: Vec<Vec3> = lod
                    .vertices
                    .iter()
                    .filter(|v| {
                        v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::TAIL && ((v.rig & rig::TAIL_SEG_MASK) >> rig::TAIL_SEG_SHIFT) as usize == seg
                    })
                    .map(|v| Vec3::from(v.pos))
                    .collect();
                assert!(!verts.is_empty(), "segment {seg} has no geometry");
                let (a, c) = (TAIL[seg], TAIL[seg + 1]);
                let reach = TAIL_WIDTH[seg] * 2.2 + 0.4;
                for v in &verts {
                    let t = ((*v - a).dot(c - a) / (c - a).length_squared()).clamp(0.0, 1.0);
                    assert!(v.distance(a.lerp(c, t)) < reach, "segment {seg} vertex {v} strays");
                }
            }
        }
    }

    #[test]
    fn prong_ends_at_the_unit_files_emitter() {
        let model = model();
        for lod in &model.lods {
            let tip = lod
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::ARM_TOOL)
                .map(|v| Vec3::from(v.pos))
                .max_by(|a, b| a.x.total_cmp(&b.x))
                .unwrap();
            assert!(tip.distance(PRONG_TIP) < 0.1, "prong tip {tip}");
        }
        // `builder.arm` in the unit file: `pivot: (-0.55, 0, 3.75)`, `emitter: (1.0, 0, 3.3)`.
        assert_eq!(PRONG_PIVOT, Vec3::new(-0.55, 0.0, 3.75));
        assert_eq!(PRONG_TIP, Vec3::new(1.0, 0.0, 3.3));
        assert_eq!(model.arm_pivot, Some(PRONG_PIVOT.to_array()));
        // The sim swings the tip about the unit file's `turret_at: (-1.9, 0.0)`.
        assert_eq!((model.turret_pivot[0], model.turret_pivot[1]), (-1.9, 0.0));
    }

    #[test]
    fn the_sims_single_pivot_follows_the_bending_tail() {
        // The shader turns each of the top four joints by its share of the aim; the sim
        // turns the emitter about `AIM_PIVOT` by the whole aim. Over the unit file's
        // `aim_arc` (60 degrees either side) the two stay within 0.3 m.
        let turn = |p: Vec2, c: Vec2, a: f32| c + Vec2::from_angle(a).rotate(p - c);
        let joints = &TAIL[TAIL.len() - 4..];
        let worst = |pivot: f32| {
            let mut worst = 0.0f32;
            for deg in (5..=60).step_by(5) {
                let a = (deg as f32).to_radians();
                for at in [PRONG_TIP, PRONG_PIVOT] {
                    let mut bent = at.truncate();
                    for (j, share) in joints.iter().zip(AIM_SHARE).rev() {
                        bent = turn(bent, j.truncate(), a * share);
                    }
                    worst = worst.max(bent.distance(turn(at.truncate(), Vec2::new(pivot, 0.0), a)));
                }
            }
            worst
        };
        let error = worst(AIM_PIVOT);
        assert!(error < 0.3, "emitter off the drawn tip by {error} m");
        // It is the best fit: no pivot along the spine does better by more than a centimetre.
        for k in -60..=0 {
            let other = k as f32 * 0.05;
            assert!(worst(other) > error - 0.01, "pivot {other} fits better than {AIM_PIVOT}");
        }
        assert!((AIM_SHARE.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }
}

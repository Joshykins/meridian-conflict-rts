//! What every design of the yard shares: the plated apron it stands on with the owner's
//! colour round it, the four intakes helpers pour their work into, the crown of plates
//! the great beam leaves from, and the plated limbs its arms and buttresses are made of.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::{PLATING_DARK, TEAM};
use crate::pattern;

use super::super::gravitic_engine::rotors::Rotor;
use super::super::gravitic_engine::works::conduit;
use super::super::kit::{dark_plate, metal, nanite_plate, seam, v3};
use super::super::machine::{armour, collar, hoop, hoop_on, swept, Frame};
use super::{EMITTER, INTAKE_R, INTAKE_Z};

/// A level unit vector `deg` degrees round from +x.
pub(super) fn dir(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    v3(a.cos(), a.sin(), 0.0)
}

/// The apron: two plated steps, a course of swept plates round the outer one, and the
/// owner's colour round the inner. Far off, one low step and the colour.
pub(super) fn apron(b: &mut MeshBuilder) {
    dark_plate(b);
    if b.coarse() {
        b.prism(Vec3::ZERO, 6, 136.0, 132.0, 2.0);
        b.paint(TEAM);
        b.decal(Vec3::Z * 2.2, Vec2::splat(60.0));
        return;
    }
    let sides = if b.fine() { 24 } else { 12 };
    b.with_facets(|b| b.prism(Vec3::ZERO, sides, 136.0, 132.0, 1.8));
    seam(b);
    b.with_facets(|b| b.prism(Vec3::Z * 1.8, sides, 124.0, 120.0, 1.6));
    b.paint(TEAM);
    hoop(b, Vec3::Z * 3.5, 112.0, 5.0, 0.3, sides);
    if !b.fine() {
        return;
    }
    // Swept plates round the outer step, their points running round it: from above the
    // apron reads swept, not squared.
    dark_plate(b);
    let n = 28;
    for k in 0..n {
        let a = 360.0 * k as f32 / n as f32;
        let d = dir(a);
        let along = dir(a + 90.0);
        let f = Frame::new(d * 128.0 + Vec3::Z * 1.85, along, Vec3::Z + d * 0.1);
        armour(b, &f, &swept(24.0, 3.2, 0.6, 0.45), 0.6);
    }
}

/// The four intakes round the foot, on the axes: plated houses sloping down outward, the
/// mouth on each (the unit file's emitters 2 to 5) a graphite collar rimmed in violet,
/// a red optic under it, plates swept back over its flanks, and conduits in to the middle.
pub(super) fn intakes(b: &mut MeshBuilder, from: f32, inner: f32) {
    if b.coarse() {
        return;
    }
    for k in 0..4 {
        let a = 90.0 * k as f32;
        let d = dir(a);
        let side = d.cross(Vec3::Z);
        let section = |r: f32, h: f32, w: f32| {
            vec![
                d * r - side * w,
                d * r + side * w,
                d * r + side * (w * 0.7) + Vec3::Z * h,
                d * r - side * (w * 0.7) + Vec3::Z * h,
            ]
        };
        // Its top passes just under the mouth.
        dark_plate(b);
        b.with_facets(|b| {
            b.loft(
                &[
                    section(from, 60.0, 15.0),
                    section(INTAKE_R, INTAKE_Z - 3.0, 14.0),
                    section(80.0, 22.0, 12.0),
                ],
                true,
                true,
            );
        });
        // The mouth: a collar facing up and out, rimmed violet, working while it builds.
        let mouth = d * INTAKE_R + Vec3::Z * INTAKE_Z;
        let axis = (d * 0.55 + Vec3::Z).normalize();
        metal(b);
        let sides = b.sides(16);
        b.cylinder_between(mouth - axis * 4.0, mouth, 8.5, 7.5, sides);
        b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
        b.with_facets(|b| hoop_on(b, mouth + axis * 0.4, axis, 7.6, 2.4, 1.2, sides));
        if !b.fine() {
            continue;
        }
        // An optic looking out under the mouth, in a plate of its own.
        b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
        let f = Frame::new(
            d * 79.0 + Vec3::Z * 6.0,
            Vec3::Z + d * 0.4,
            d + Vec3::Z * 0.3,
        );
        armour(b, &f, &swept(14.0, 6.0, 0.0, 0.7), 0.6);
        // Plates swept back up its flanks toward the middle, points standing proud.
        dark_plate(b);
        for s in [-1.0f32, 1.0] {
            for (j, r) in [74.0f32, 62.0, from + 10.0].into_iter().enumerate() {
                let h = 22.0 + (r - 80.0) / (from - 80.0) * 38.0;
                let o = d * r + side * (s * 13.5) + Vec3::Z * (h * 0.35);
                let f = Frame::new(o, -d + Vec3::Z * 0.55, side * s);
                armour(
                    b,
                    &f,
                    &swept(18.0 + 3.0 * j as f32, h * 0.32, 0.5, 0.4),
                    0.7,
                );
            }
        }
        conduit(b, d * from, d * inner, 3, 1.1);
    }
}

/// The crown the great beam leaves from: `petals` plates splayed up and out round the
/// emitter, glazed violet inside, on a graphite collar `r` round with a violet rim.
pub(super) fn emitter_crown(b: &mut MeshBuilder, petals: usize, r: f32) {
    let sides = if b.fine() { 24 } else { 12 };
    metal(b);
    hoop(b, EMITTER - Vec3::Z * 7.0, r, 3.0, 8.0, sides);
    b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
    b.with_facets(|b| hoop(b, EMITTER - Vec3::Z * 2.0, r * 0.72, 2.6, 2.4, sides));
    for k in 0..petals {
        let a = 360.0 * k as f32 / petals as f32 + 15.0;
        let d = dir(a);
        let up = Vec3::Z + d * 0.5;
        let foot = EMITTER - Vec3::Z * 26.0 + d * (r * 0.9);
        dark_plate(b);
        let f = Frame::new(foot, up, d);
        b.with_facets(|b| armour(b, &f, &swept(36.0, r * 0.42, 0.0, 0.55), 1.4));
        if b.fine() {
            // Its inner face, glazed: the violet of the work showing through.
            nanite_plate(b);
            let g = Frame::new(foot - d * 0.3 + up.normalize() * 3.0, up, -d);
            armour(b, &g, &swept(26.0, r * 0.3, 0.0, 0.6), 0.5);
        }
    }
}

/// A plated limb along `path` (points with their half width): a faceted core, keel turned
/// toward `out`, with a course of swept plates laid up its back, points standing proud
/// toward its end, and a graphite collar at each inner point.
pub(super) fn limb(b: &mut MeshBuilder, path: &[(Vec3, f32)], out: Vec3) {
    let frame_at = |i: usize| {
        let (p, _) = path[i];
        let t = if i + 1 < path.len() {
            path[i + 1].0 - p
        } else {
            p - path[i - 1].0
        }
        .normalize();
        let n = (out - t * out.dot(t)).normalize_or(Vec3::Z);
        (t, n, n.cross(t))
    };
    let rings: Vec<Vec<Vec3>> = (0..path.len())
        .map(|i| {
            let (p, w) = path[i];
            let (_, n, v) = frame_at(i);
            // Its foot is cut level where it stands.
            [
                p + n * (w * 0.9),
                p + v * w + n * (w * 0.3),
                p + v * (w * 0.8) - n * (w * 0.6),
                p - n * (w * 0.8),
                p - v * (w * 0.8) - n * (w * 0.6),
                p - v * w + n * (w * 0.3),
            ]
            .iter()
            .map(|q| q.with_z(q.z.max(0.0)))
            .collect()
        })
        .collect();
    dark_plate(b);
    if b.coarse() {
        let thin: Vec<Vec<Vec3>> = rings.iter().map(|r| vec![r[0], r[2], r[4]]).collect();
        b.loft(
            &[thin[0].clone(), thin[thin.len() - 1].clone()],
            false,
            false,
        );
        return;
    }
    b.with_facets(|b| b.loft(&rings, true, true));
    for (i, &(p, w)) in path.iter().enumerate().take(path.len() - 1).skip(1) {
        let (t, _, _) = frame_at(i);
        collar(b, p, t, w * 0.95, w * 0.5);
    }
    let every = if b.fine() { 1 } else { 2 };
    for i in (0..path.len() - 1).step_by(every) {
        let (p, w) = path[i];
        let (t, n, _) = frame_at(i);
        let len = p.distance(path[i + 1].0) * 1.25;
        dark_plate(b);
        let o = p + n * (w * 0.85);
        let f = Frame::new(o.with_z(o.z.max(p.z + w * 0.3)), t + n * 0.06, n);
        armour(
            b,
            &f,
            &swept(len, w * 0.75, 0.0, 0.4),
            (w * 0.12).clamp(0.6, 2.0),
        );
        if b.fine() {
            // Plates on its flanks, a step behind, their points flaring out.
            for s in [-1.0f32, 1.0] {
                let (_, _, v) = frame_at(i);
                let g = Frame::new(p + v * (s * w * 0.85) + t * (len * 0.2), t, v * s + n * 0.4);
                armour(b, &g, &swept(len * 0.8, w * 0.5, 0.4 * s, 0.45), 0.8);
            }
        }
    }
    if b.fine() {
        // A seam-dark keel strip along its belly.
        seam(b);
        for (i, w) in path.windows(2).enumerate() {
            let (p, q) = (w[0], w[1]);
            let (_, n, _) = frame_at(i);
            // Off the ground at the foot.
            let p = if i == 0 { (p.0.lerp(q.0, 0.2), p.1) } else { p };
            b.beam(
                p.0 - n * (p.1 * 0.82),
                q.0 - n * (q.1 * 0.82),
                Vec2::splat(p.1 * 0.3),
                Vec2::splat(q.1 * 0.3),
            );
        }
    }
}

/// Spokes from a turning ring in to its race on a mast of radius `hub` at the ring's
/// height, turning with it: plated, a graphite knuckle at the ring.
pub(super) fn spokes(b: &mut MeshBuilder, r: &Rotor, hub: f32) {
    let z = (r.z0 + r.z1) * 0.5;
    let n = if b.fine() { 6 } else { 3 };
    for k in 0..n {
        let d = dir(360.0 * k as f32 / n as f32 + 10.0);
        dark_plate(b);
        b.beam(
            d * (hub + 4.0) + Vec3::Z * z,
            d * (r.inner + 1.0) + Vec3::Z * z,
            Vec2::new(3.0, (r.z1 - r.z0) * 0.5),
            Vec2::new(2.2, (r.z1 - r.z0) * 0.6),
        );
        if b.fine() {
            collar(b, d * (r.inner - 1.5) + Vec3::Z * z, d, 2.2, 3.0);
        }
    }
}

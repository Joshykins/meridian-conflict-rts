//! The scorpion's tail and the beam projector on its tip.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, rig};

use super::super::kit::*;
use super::*;

/// The tail: nine segments from the socket to the projector, each a working joint of the
/// machine, not a sleeve: a bare ribbed core turning on a drum, an arched plate over the
/// outside of the curl, a separate pad on each flank, a belly plate, a ram along each side
/// and cables down the inside. Every plate stops short of both joints, so each joint shows
/// as a gap with the core and its red seams in it.
pub(super) fn tail(b: &mut MeshBuilder) {
    for i in 0..TAIL.len() - 1 {
        let (a, c) = (TAIL[i], TAIL[i + 1]);
        let (wa, wc) = (TAIL_WIDTH[i], TAIL_WIDTH[i + 1]);
        let dir = c - a;
        let along = dir.normalize();
        let mid = (a + c) * 0.5;
        let outward = (mid - CURL).normalize();
        let (side, up) = frame(dir, outward);
        let (p0, p1) = (a + dir * 0.17, c - dir * 0.1);
        b.with_tail(i, |b| {
            // The core: bare and narrow, lit at its seams, turning on a drum at its joint.
            under_hide(b);
            segment(
                b,
                &[(a, wa * 0.5, wa * 0.5), (c, wc * 0.5, wc * 0.5)],
                outward,
            );
            knuckle(b, a, Vec3::Y, wa * 0.64, wa * 1.9);
            // The arch over the outside of the curl, lifted at its back edge.
            hide(b);
            let arch = |p: Vec3, w: f32, k: f32| (p + up * (w * 0.4), w * 1.0 * k, w * 0.72 * k);
            if b.fine() {
                shell(
                    b,
                    &[
                        arch(p0, wa, 0.88),
                        arch(a.lerp(c, 0.55), wa, 1.0),
                        arch(p1, wc, 1.1),
                    ],
                    outward,
                );
            } else {
                shell(b, &[arch(p0, wa, 0.9), arch(p1, wc, 1.05)], outward);
            }
            if b.coarse() {
                return;
            }
            // The crest blade on the arch, raked back toward the root.
            let top = mid + up * (wa * 1.1);
            blade(
                b,
                top + along * 0.5,
                top + up * (1.4 + wa * 0.4) - along * 1.2,
                0.5,
                side,
            );
            // The belly plate on the inside of the curl.
            let belly = |p: Vec3, w: f32| (p - up * (w * 0.62), w * 0.55, w * 0.3);
            shell(
                b,
                &[belly(a + dir * 0.26, wa), belly(c - dir * 0.2, wc)],
                -up,
            );
            if !b.fine() {
                return;
            }
            for s in [-1.0f32, 1.0] {
                // A pad on each flank, standing off the core under the arch's rim.
                hide(b);
                let at = |p: Vec3, w: f32| p + side * (s * w * 0.98) - up * (w * 0.2);
                let (f0, f1) = (at(a + dir * 0.28, wa), at(c - dir * 0.22, wc));
                slab(
                    b,
                    [
                        f0 - up * (wa * 0.34),
                        f0 + up * (wa * 0.3),
                        f1 + up * (wc * 0.34),
                        f1 - up * (wc * 0.3),
                    ],
                    side * (s * 0.22),
                );
                // A side blade off the pad, fanned out and back.
                let root = (f0 + f1) * 0.5 + up * (wa * 0.2);
                blade(
                    b,
                    root + along * 0.4,
                    root + (up * 0.7 + side * s).normalize() * 1.3 - along * 1.0,
                    0.34,
                    up,
                );
                // The ram that bends this joint, along the flank inside the pad.
                ram(
                    b,
                    a + dir * 0.1 + side * (s * wa * 0.62) + up * (wa * 0.05),
                    c - dir * 0.16 + side * (s * wc * 0.62) + up * (wc * 0.05),
                    0.15 + wa * 0.05,
                );
                // A red seam in the gap between the arch and the pad.
                b.paint(GLOW_LASER);
                let seam = a + dir * 0.34 + side * (s * wa * 0.86) + up * (wa * 0.2);
                b.beam(
                    seam,
                    seam + dir * 0.4,
                    Vec2::new(0.08, 0.08),
                    Vec2::new(0.08, 0.08),
                );
                // Cables down the inside of the curl.
                metal(b);
                cable(
                    b,
                    &[
                        a + dir * 0.02 + side * (s * wa * 0.26) - up * (wa * 0.44),
                        c - dir * 0.02 + side * (s * wc * 0.26) - up * (wc * 0.44),
                    ],
                    0.1,
                );
            }
            // A red line down the belly plate.
            b.paint(GLOW_LASER);
            b.beam(
                a + dir * 0.3 - up * (wa * 0.93),
                c - dir * 0.25 - up * (wc * 0.9),
                Vec2::new(0.18, 0.08),
                Vec2::new(0.14, 0.08),
            );
        });
    }
}

/// The Pinched-plasmeric Beam projector on the tail's tip: an armoured housing on the last
/// joint, flanked by bronze field drums, and the barrel it pitches (`ARM_GUN`).
pub(super) fn projector(b: &mut MeshBuilder) {
    b.with_part(part::TURRET, |b| {
        let root = TAIL[TAIL.len() - 1];
        knuckle(b, root, Vec3::Y, 0.85, 1.7);
        // The housing, where the tail ends: a dark core under an arched top plate.
        under_hide(b);
        segment(
            b,
            &[
                (root, 0.95, 0.9),
                (v3(-3.8, 0.0, 19.8), 1.6, 1.35),
                (v3(-2.3, 0.0, 19.5), 1.45, 1.2),
                (v3(-1.0, 0.0, 18.9), 0.95, 0.85),
            ],
            Vec3::Z,
        );
        hide(b);
        shell(
            b,
            &[
                (v3(-4.6, 0.0, 20.25), 1.55, 1.05),
                (v3(-3.2, 0.0, 20.35), 1.75, 1.1),
                (v3(-1.7, 0.0, 19.9), 1.3, 0.85),
            ],
            Vec3::Z,
        );
        if b.fine() {
            b.mirror_y(|b| {
                // A bronze field drum along each flank: what pinches the plasma.
                metal(b);
                b.cylinder_between(v3(-4.3, 1.45, 19.35), v3(-1.9, 1.35, 19.1), 0.52, 0.46, 8);
                under_hide(b);
                for x in [-3.7f32, -2.9, -2.1] {
                    b.cylinder_between(
                        v3(x + 0.12, 1.42, 19.3),
                        v3(x - 0.12, 1.42, 19.3),
                        0.6,
                        0.6,
                        8,
                    );
                }
                // The top plate's edge, swept back and out into a blade.
                hide(b);
                blade(
                    b,
                    v3(-3.6, 1.2, 20.6),
                    v3(-5.6, 2.3, 21.4),
                    0.34,
                    v3(0.0, 1.0, 0.3),
                );
            });
            hide(b);
            spike(b, v3(-3.9, 0.0, 21.2), v3(-6.2, 0.0, 22.6), 0.42);
            // Its eyes: a pair of red slits under the front of the top plate.
            b.paint(GLOW_LASER);
            b.mirror_y(|b| {
                b.beam(
                    v3(-1.5, 0.55, 19.95),
                    v3(-1.1, 0.5, 19.75),
                    Vec2::new(0.1, 0.12),
                    Vec2::new(0.08, 0.1),
                );
            });
        }
        b.with_limb(rig::ARM_GUN, barrel);
    });
}

/// The barrel from its joint to the muzzle: a plated breech, then the bore lit red through
/// three bronze pinch rings that squeeze the plasma into the stream, rails over and under
/// them, and a bronze muzzle ring.
fn barrel(b: &mut MeshBuilder) {
    let (pivot, tip) = (BEAM_PIVOT, BEAM_TIP);
    let axis = (tip - pivot).normalize();
    let length = tip.distance(pivot);
    let at = |t: f32| pivot + axis * (length * t);
    knuckle(b, pivot, Vec3::Y, 0.7, 1.3);
    // The breech: plated, wide at the joint, tapering onto the bore.
    hide(b);
    shell(
        b,
        &[
            (at(0.02), 0.85, 0.8),
            (at(0.2), 0.8, 0.7),
            (at(0.34), 0.55, 0.5),
        ],
        Vec3::Z,
    );
    // The bore, lit where the rings leave it bare.
    b.paint(GLOW_LASER);
    b.cylinder_between(at(0.3), at(0.97), 0.2, 0.17, 6);
    // The pinch rings, each tighter than the last.
    metal(b);
    let sides = b.sides(10).min(10);
    for (t, r) in [(0.42f32, 0.6f32), (0.62, 0.52), (0.8, 0.45)] {
        b.cylinder_between(at(t - 0.035), at(t + 0.035), r, r, sides);
    }
    b.cylinder_between(at(0.95), at(1.0), 0.36, 0.4, sides);
    if b.fine() {
        // Rails over and under the rings, holding them, their back ends swept into blades.
        hide(b);
        for s in [1.0f32, -1.0] {
            let off = Vec3::Z * (0.62 * s);
            b.beam(
                at(0.3) + off,
                at(0.86) + off,
                Vec2::new(0.22, 0.12),
                Vec2::new(0.16, 0.1),
            );
        }
        blade(
            b,
            at(0.34) + Vec3::Z * 0.7,
            at(0.12) + Vec3::Z * 1.5,
            0.2,
            Vec3::Y,
        );
    }
}

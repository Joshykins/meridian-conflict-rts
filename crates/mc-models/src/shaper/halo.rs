//! Every Shaper's anti-nuke, the halo: an open ring tilted round its Lens, a segment to
//! a slot and the gap astern, an interceptor blade hanging point down from a clamp
//! under each segment; from tier 3 a thin outer ring, from tier 5 a third over it.
//!
//! A blade to an interceptor, so the stock is counted at a glance: each slot's clamp and
//! blade carry `part::HALO` with the slot packed in (`gpu_consts::halo`), and the shader
//! draws a blade only while the Shaper holds it (entity.wgsl `halo_slot`), the next
//! printing in as it is assembled, a spent one lifting off.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::Affine3A;

use super::super::builder::MeshBuilder;
use super::super::gpu_consts::halo;
use super::super::part;
use super::super::precursor::{dark, pale, v3};
use super::super::precursor_sky::{arc_steps, rz_box, sweep};
use super::kit::{ring, Ring};
use super::Growth;

/// What the halo is laid out by: the Shaper's size and its Lens.
pub(super) struct Mount {
    pub(super) g: Growth,
    /// The Lens's middle, half height and great ring's radius.
    pub(super) lens_mid: f32,
    pub(super) lens_half: f32,
    pub(super) lens_r: f32,
    /// An interceptor blade's length.
    pub(super) len: f32,
}

/// An interceptor standing on its foot at the origin, `len` tall, `w` across x and `t`
/// thick in y: a dark blade cut on a slant at both ends, sheathed in pale at its head
/// and foot, so each reads apart from the pale work it hangs in.
fn interceptor(b: &mut MeshBuilder, len: f32, w: f32, t: f32) {
    let h = w * 0.5;
    let blade = [
        [-h, len * 0.1],
        [0.0, 0.0],
        [h, len * 0.16],
        [h, len * 0.8],
        [-h * 0.2, len],
        [-h, len * 0.86],
    ];
    dark(b);
    b.extrude_y_chamfered(&blade, t, t * 0.2);
    if b.coarse() {
        // Far off, the blade alone.
        return;
    }
    pale(b);
    let (sh, st) = (h * 1.08, t * 1.12);
    let head = [
        [-sh, len * 0.66],
        [sh, len * 0.7],
        [sh, len * 0.8],
        [-h * 0.2, len * 1.01],
        [-sh, len * 0.87],
    ];
    b.extrude_y_chamfered(&head, st, t * 0.2);
    let foot = [
        [-sh, len * 0.1],
        [0.0, -len * 0.01],
        [sh, len * 0.16],
        [sh, len * 0.24],
        [-sh, len * 0.2],
    ];
    b.extrude_y_chamfered(&foot, st, t * 0.2);
}

/// Runs `f` in a frame at `angle` round the z axis, `out` from it at height `z`, its x
/// along the circle and its y outward, so a blade drawn in x and z shows its face out.
fn facing_out(b: &mut MeshBuilder, angle: f32, out: f32, z: f32, f: impl FnOnce(&mut MeshBuilder)) {
    b.with(
        Affine3A::from_rotation_z(angle)
            * Affine3A::from_translation(v3(out, 0.0, z))
            * Affine3A::from_rotation_z(-FRAC_PI_2),
        f,
    );
}

/// The middle of slot `k` of `n`: round the circle from astern, but for the gap
/// (`halo::GAP`) there, where a printing Shaper's spine stands. entity.wgsl
/// `halo_slot` lifts a spent blade off along the same bearing.
fn slot_angle(k: usize, n: usize) -> f32 {
    let span = (TAU - halo::GAP) / n as f32;
    PI + halo::GAP * 0.5 + span * (k as f32 + 0.5)
}

/// The ring's radius, its depth and band, its tilt and its middle.
fn layout(m: &Mount) -> (f32, f32, f32, f32, glam::Vec3) {
    let r = m.lens_r * 1.32;
    (
        r,
        r * 0.1,
        m.lens_half * 0.09 + 0.6,
        0.14,
        v3(0.0, 0.0, m.lens_mid + m.lens_half * 0.15),
    )
}

/// The middle of the blade hanging in slot `k` (of the halo's own count), in model
/// space: where an interceptor leaves from (the renderer's hard light, `shaper::halo_slot`).
pub(super) fn slot_middle(m: &Mount, k: usize) -> glam::Vec3 {
    let n = m.g.slots.max(1);
    let (r, depth, band, tilt, centre) = layout(m);
    let a = slot_angle(k.min(n - 1), n);
    let local = v3(
        a.cos() * (r - depth * 0.5),
        a.sin() * (r - depth * 0.5),
        -band * 3.0 - m.len * 0.5,
    );
    centre + Affine3A::from_rotation_y(tilt).transform_vector3(local)
}

pub(super) fn draw(b: &mut MeshBuilder, m: &Mount) {
    let n = m.g.slots.max(1);
    let (r, depth, band, tilt, centre) = layout(m);
    let span = (TAU - halo::GAP) / n as f32;
    let len = m.len;
    let w = len * 0.2;
    b.with(
        Affine3A::from_translation(centre) * Affine3A::from_rotation_y(tilt),
        |b| {
            for k in 0..n {
                let a = slot_angle(k, n);
                let half = span * 0.5 - 0.04f32.min(span * 0.2);
                let (a0, a1) = (a - half, a + half);
                pale(b);
                let section = rz_box(b, r - depth, r, -band, band, band * 0.4);
                let steps = arc_steps(b, r, a1 - a0).max(2);
                sweep(b, &section, a0, a1, steps, true);
                // A clamp under the segment, the interceptor hanging from it.
                let slot =
                    part::HALO | (k as u32) << halo::SLOT_SHIFT | (n as u32) << halo::SLOTS_SHIFT;
                facing_out(b, a, r - depth * 0.5, -band, |b| {
                    // Far off, no clamp: a blade hangs from the ring alone.
                    if !b.coarse() {
                        b.with_part(slot, |b| {
                            dark(b);
                            b.chamfered_box(
                                v3(0.0, 0.0, -band),
                                v3(w * 0.9, depth * 0.9, band * 2.0),
                                0.0,
                            );
                        });
                    }
                    b.with_part(slot | halo::BLADE, |b| {
                        b.with(
                            Affine3A::from_translation(v3(0.0, 0.0, -band * 2.0))
                                * Affine3A::from_rotation_y(PI),
                            |b| interceptor(b, len, w, depth * 0.5),
                        );
                    });
                });
            }
            if m.g.tier >= 3 && !b.coarse() {
                for (k, (out, z)) in [(1.12f32, band * 1.6), (1.2, band * 4.0)]
                    .into_iter()
                    .enumerate()
                {
                    if k == 1 && m.g.tier < 5 {
                        break;
                    }
                    ring(
                        b,
                        Ring {
                            r0: r * out - depth * 0.35,
                            r1: r * out,
                            z,
                            h: band * 0.5,
                            pieces: 16,
                            gap: r * 0.05 + w,
                            turn: PI + halo::GAP * 0.5,
                        },
                    );
                }
            }
        },
    );
}

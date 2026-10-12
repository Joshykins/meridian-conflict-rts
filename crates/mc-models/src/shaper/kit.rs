//! The Shapers' kit: the pieces both spires are built of, in the Heart's language
//! (`precursor_sky.rs`): rings of hovering segments, battered courses, shaft stages
//! between pale piers, slab tips and the Lens. Every lit part is the plate's own surface
//! (`PRECURSOR` faces carry the slot pattern); the only lit solid is the Lens's core.

use std::f32::consts::{FRAC_PI_4, PI, TAU};

use glam::{Affine3A, Vec3};

use super::super::builder::{MeshBuilder, Section};
use super::super::precursor::{cut_rect, dark, fine_rect, key_light, pale, panel, v3};
use super::super::precursor_sky::{arc_steps, rz_box, sweep};

/// A ring of `pieces` segments hovering apart round the z axis, `r0..r1` across and `h`
/// either side of `z`: pale flanges over a recessed dark band at full detail.
#[derive(Clone, Copy)]
pub(super) struct Ring {
    pub(super) r0: f32,
    pub(super) r1: f32,
    pub(super) z: f32,
    pub(super) h: f32,
    pub(super) pieces: usize,
    pub(super) gap: f32,
    pub(super) turn: f32,
}

pub(super) fn ring(b: &mut MeshBuilder, o: Ring) {
    let span = TAU / o.pieces as f32;
    let rm = (o.r0 + o.r1) * 0.5;
    let half_gap = o.gap * 0.5 / rm;
    let depth = o.r1 - o.r0;
    let ch = (depth.min(o.h * 2.0) * 0.2).min(6.0);
    for k in 0..o.pieces {
        let a0 = o.turn + span * k as f32 + half_gap;
        let a1 = o.turn + span * (k + 1) as f32 - half_gap;
        let steps = arc_steps(b, o.r1, a1 - a0).max(((a1 - a0) / 0.4).ceil() as usize);
        ring_piece(b, &o, a0, a1, steps, ch);
    }
}

fn ring_piece(b: &mut MeshBuilder, o: &Ring, a0: f32, a1: f32, steps: usize, ch: f32) {
    let (r0, r1, z, h) = (o.r0, o.r1, o.z, o.h);
    if !b.fine() {
        pale(b);
        let whole = rz_box(b, r0, r1, z - h, z + h, ch);
        sweep(b, &whole, a0, a1, steps, true);
        return;
    }
    let band = h * 0.34;
    let recess = ((r1 - r0) * 0.1).max(0.2);
    dark(b);
    let core = rz_box(
        b,
        r0 + recess,
        r1 - recess,
        z - band - 0.2,
        z + band + 0.2,
        0.0,
    );
    sweep(b, &core, a0, a1, steps, true);
    let c = ch.min((h - band) * 0.9);
    let top = [
        [r1, z + band],
        [r1, z + h - c],
        [r1 - c, z + h],
        [r0 + c, z + h],
        [r0, z + h - c],
        [r0, z + band],
    ];
    let bottom: Vec<[f32; 2]> = top.iter().rev().map(|p| [p[0], 2.0 * z - p[1]]).collect();
    pale(b);
    sweep(b, &top, a0, a1, steps, true);
    sweep(b, &bottom, a0, a1, steps, true);
}

/// A battered course of a stepped foot, `hb` half across at `z0` to `ht` at `z1`, a
/// dark band round its faces with pale pilasters standing over it.
pub(super) fn course(b: &mut MeshBuilder, z0: f32, z1: f32, hb: f32, ht: f32) {
    let ch = hb * 0.28;
    pale(b);
    let plan = cut_rect(b, hb, hb, ch);
    b.loft_z(
        &plan,
        &[
            Section::new((z0 - 1.0).max(0.0), 1.0),
            Section::new(z1, ht / hb),
        ],
    );
    if b.coarse() {
        return;
    }
    let half = |z: f32| hb + (ht - hb) * (z - z0) / (z1 - z0);
    let out = v3(z1 - z0, 0.0, hb - ht).normalize();
    let p = |y: f32, z: f32| v3(half(z), y, z);
    let fine = b.fine();
    b.radial(4, |b| {
        let (za, zb) = (z0 + (z1 - z0) * 0.38, z0 + (z1 - z0) * 0.66);
        let w = half(zb) * (1.0 - 0.28) - hb * 0.04;
        dark(b);
        panel(
            b,
            &[p(-w, za), p(w, za), p(w, zb), p(-w, zb)],
            out,
            hb * 0.006,
            0.0,
        );
        if fine {
            pale(b);
            let n = 6;
            for i in 0..=n {
                let y = -w + 2.0 * w * i as f32 / n as f32;
                let (y0, y1) = (y - hb * 0.025, y + hb * 0.025);
                let (pa, pb) = (z0 + (z1 - z0) * 0.08, z1 - (z1 - z0) * 0.1);
                panel(
                    b,
                    &[p(y0, pa), p(y1, pa), p(y1, pb), p(y0, pb)],
                    out,
                    hb * 0.014,
                    hb * 0.004,
                );
            }
        }
    });
}

/// A dark joint between two courses or stages.
pub(super) fn joint(b: &mut MeshBuilder, z0: f32, z1: f32, half: f32) {
    dark(b);
    let plan = cut_rect(b, half, half, half * 0.28);
    b.loft_z(
        &plan,
        &[Section::new(z0 - 0.3, 1.0), Section::new(z1 + 0.3, 1.0)],
    );
}

/// One stage of a shaft: a dark core tapering from `h0` to `h1` half across between
/// pale corner piers, pale bands across the dark every `bands` metres.
pub(super) fn shaft(b: &mut MeshBuilder, z0: f32, z1: f32, h0: f32, h1: f32, bands: f32) {
    let shrink = h1 / h0;
    if b.coarse() {
        pale(b);
    } else {
        dark(b);
    }
    let core = cut_rect(b, h0, h0, h0 * 0.28);
    b.loft_z(&core, &[Section::new(z0, 1.0), Section::new(z1, shrink)]);
    if b.coarse() {
        return;
    }
    pale(b);
    let p = h0 * 0.24;
    let pier = fine_rect(b, p, p, p * 0.3);
    let c0 = h0 - p + h0 * 0.04;
    let c1 = c0 * shrink;
    for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        b.loft_z(
            &pier,
            &[
                Section::new(z0 - 0.2, 1.0).shifted(sx * c0, sy * c0),
                Section::new(z1 + 0.2, shrink).shifted(sx * c1, sy * c1),
            ],
        );
    }
    if !b.fine() || bands <= 0.0 {
        return;
    }
    let face = |z: f32| h0 + (h1 - h0) * (z - z0) / (z1 - z0);
    let out = v3(z1 - z0, 0.0, h0 - h1).normalize();
    b.radial(4, |b| {
        pale(b);
        let mut z = z0 + bands * 0.6;
        while z < z1 - bands * 0.4 {
            let (za, zb) = (z, z + h0 * 0.16);
            let (wa, wb) = (face(za) - p * 1.5, face(zb) - p * 1.5);
            let quad = [
                v3(face(za), -wa, za),
                v3(face(za), wa, za),
                v3(face(zb), wb, zb),
                v3(face(zb), -wb, zb),
            ];
            panel(b, &quad, out, h0 * 0.03, h0 * 0.008);
            z += bands;
        }
    });
}

/// A slab with both ends cut on a slant, `len` tall standing on z, `d` deep in x and
/// `w` thick in y: a Precursor interceptor in its rack.
pub(super) fn shard(b: &mut MeshBuilder, at: Vec3, len: f32, d: f32, w: f32) {
    if b.coarse() {
        return;
    }
    pale(b);
    let profile = [
        [at.x - d, at.z],
        [at.x + d, at.z + d * 1.2],
        [at.x + d, at.z + len - d * 1.6],
        [at.x - d, at.z + len],
    ];
    b.at(v3(0.0, at.y, 0.0), |b| {
        b.extrude_y_chamfered(&profile, w, w * 0.2);
    });
}

/// The Lens: the Lance's focus. A spindle of live light between a pale floor cap and
/// roof drum (each `cap` half across), shards standing round it, rings hovering round
/// it in the shape of a lens, more of them with `tier`. Centred at `c`, `half_h`
/// either side, its great ring `radius` out. `gap` widens the gaps of its rings (a
/// blade may stand through one).
pub(super) fn lens(
    b: &mut MeshBuilder,
    c: Vec3,
    half_h: f32,
    radius: f32,
    cap: f32,
    tier: usize,
    gap: f32,
    turn: f32,
) {
    let floor = cut_rect(b, cap, cap, cap * 0.28);
    pale(b);
    if cap > 0.0 {
        b.loft_z(
            &floor,
            &[
                Section::new(c.z - half_h - 1.0, 1.0),
                Section::new(c.z - half_h + cap * 0.12, 0.84),
                Section::new(c.z - half_h + cap * 0.2, 0.6),
            ],
        );
    }
    // The spindle of live light, flat at both ends.
    key_light(b);
    let core = cut_rect(b, radius * 0.16, radius * 0.16, radius * 0.05);
    let hz = half_h * 0.5;
    b.loft_z(
        &core,
        &[
            Section::new(c.z - hz, 0.35),
            Section::new(c.z - hz * 0.35, 1.0),
            Section::new(c.z + hz * 0.35, 1.0),
            Section::new(c.z + hz, 0.35),
        ],
    );
    // Shards standing round it, lit on no face: the core's light falls on them.
    b.radial(4, |b| {
        b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
            pale(b);
            let (ri, ro) = (radius * 0.28, radius * 0.36);
            let shard = [
                [ri, c.z],
                [ri + (ro - ri) * 0.4, c.z + half_h * 0.62],
                [ro, c.z + half_h * 0.52],
                [ro, c.z - half_h * 0.52],
                [ri + (ro - ri) * 0.4, c.z - half_h * 0.62],
            ];
            b.extrude_y_chamfered(&shard, radius * 0.035, radius * 0.01);
        });
    });
    // The great ring, the side rings over and under it from tier 2, keys hovering off
    // the great ring from tier 4.
    let depth = radius * 0.24;
    let rh = (half_h * 0.13).max(0.6);
    ring(
        b,
        Ring {
            r0: radius - depth,
            r1: radius,
            z: c.z,
            h: rh,
            pieces: 12,
            gap: radius * 0.06 + gap,
            turn,
        },
    );
    if tier >= 2 && !b.coarse() {
        for s in [-1.0f32, 1.0] {
            ring(
                b,
                Ring {
                    r0: radius * 0.56,
                    r1: radius * 0.72,
                    z: c.z + s * half_h * 0.42,
                    h: rh * 0.55,
                    pieces: 8,
                    gap: radius * 0.05 + gap,
                    turn: turn + PI / 8.0,
                },
            );
        }
    }
    if tier >= 4 && !b.coarse() {
        for k in 0..12 {
            let a = turn + k as f32 * TAU / 12.0;
            b.with(Affine3A::from_rotation_z(a), |b| {
                pale(b);
                let (r, kh) = (radius * 1.06, half_h * 0.4);
                let key = [
                    [r, c.z],
                    [r + radius * 0.05, c.z + kh],
                    [r + radius * 0.1, c.z + kh * 0.12],
                    [r + radius * 0.1, c.z - kh * 0.12],
                    [r + radius * 0.05, c.z - kh],
                ];
                b.extrude_y_chamfered(&key, radius * 0.024, radius * 0.006);
            });
        }
    }
}

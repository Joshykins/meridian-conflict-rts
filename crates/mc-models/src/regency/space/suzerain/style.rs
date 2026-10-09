//! The Ark's dress (`ark/`) laid on a faceted hull (`body.rs`): swept armour shards in
//! courses down the deck, shoulder and flank, a saw of plates run out past an edge into
//! spikes, twin armour ridges down the spine with the graphite workings in the trench
//! between, a horned wedge bridge, canted fins of lapped plates over the stern, a keel
//! blade under the prow, drives in plated cowls and lifts slung in nacelles. Every red
//! light is drawn by its plate (`pattern::EMBER_SLOT`, `EMBER_RIM`), never a lit piece.
//! All of it is the port side's, drawn under `mirror_y`, except where it says so.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::GLOW_VIOLET;
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::lift::bell;
use super::super::super::machine::{armour, collar, swept, Frame};
use super::super::body::{Body, SPINE};

/// What a shard is painted: charcoal, or charcoal with a red optic let into it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Finish {
    Dark,
    Lit,
}

fn finish(b: &mut MeshBuilder, finish: Finish) {
    match finish {
        Finish::Dark => dark_plate(b),
        Finish::Lit => {
            dark_plate(b);
            b.pattern(pattern::EMBER_SLOT);
        }
    }
}

/// A swept plate on frame `f`: `len` back from its head, `half` across, drawn into a
/// spike at `tip` across.
pub(super) fn shard(
    b: &mut MeshBuilder,
    f: &Frame,
    (len, half, tip): (f32, f32, f32),
    thick: f32,
    paint: Finish,
) {
    finish(b, paint);
    armour(b, f, &swept(len, half, tip, 0.35), thick);
}

/// A frame on the hull at `x`, `u` round the section, `lift` off it, running aft and
/// splayed `splay` outboard, its face the hull's.
pub(super) fn on(body: &Body, x: f32, u: f32, lift: f32, splay: f32) -> Frame {
    let p = body.out(x, u, lift);
    let q = body.out(x - 2.0, u, lift);
    let aft = (q - p).normalize();
    Frame::new(p, aft + v3(0.0, splay, 0.0), body.normal(x, u))
}

/// A course of shards from `from` (forward) back to `to` at `u`, each a little longer
/// than its run so its tail laps the next one's head. Every `lit`th is lit (0 none).
#[expect(clippy::too_many_arguments, reason = "one course of a plate table")]
pub(super) fn course(
    b: &mut MeshBuilder,
    body: &Body,
    (from, to): (f32, f32),
    u: f32,
    count: usize,
    (half, thick): (f32, f32),
    paint: Finish,
    lit: usize,
) {
    let step = (from - to) / count.max(1) as f32;
    for k in 0..count {
        let x = from - step * k as f32;
        let f = on(body, x, u, 0.1, 0.06);
        let paint = if lit > 0 && k % lit == lit / 2 {
            Finish::Lit
        } else {
            paint
        };
        shard(b, &f, (step * 1.3, half, 0.3), thick, paint);
    }
}

/// Teeth across the stern at `x`: shards from the deck at each `y` run back `over`
/// past the stern into spikes.
pub(super) fn teeth(b: &mut MeshBuilder, body: &Body, x: f32, ys: &[f32], half: f32, over: f32) {
    for (k, &y) in ys.iter().enumerate() {
        let u = body.u_up(x, y);
        let f = on(body, x, u, 0.3, 0.05 + 0.04 * k as f32);
        shard(b, &f, (over + 6.0, half, 0.1), 0.9, Finish::Dark);
    }
}

/// The port armour ridge `y` out from the spine from `from` back to `to`, `tall` over
/// the deck: a faceted beam, shards swept back off its outer face; and the trench's
/// workings inboard of it, a graphite shaft, drums and ribs, a red slot in its floor.
pub(super) fn ridge(b: &mut MeshBuilder, body: &Body, (from, to): (f32, f32), y: f32, tall: f32) {
    let n = 4;
    let line: Vec<Vec3> = (0..=n)
        .map(|i| {
            let x = from + (to - from) * i as f32 / n as f32;
            v3(x, y, body.point(x, SPINE).z + tall * 0.5 - 0.6)
        })
        .collect();
    dark_plate(b);
    for w in line.windows(2) {
        b.with_facets(|b| {
            b.beam(
                w[0],
                w[1],
                Vec2::new(tall * 0.45, tall * 0.5),
                Vec2::new(tall * 0.45, tall * 0.5),
            )
        });
    }
    if b.coarse() {
        return;
    }
    let per = if b.fine() { 2 } else { 1 };
    let run = (from - to) / n as f32;
    for w in line.windows(2) {
        for j in 0..per {
            let k = (j as f32 + 0.3) / per as f32;
            let at = w[0].lerp(w[1], k) + v3(0.0, tall * 0.3, tall * 0.35);
            let f = Frame::new(at, v3(-1.0, 0.3, 0.06), v3(0.0, 0.8, 1.0));
            shard(
                b,
                &f,
                (run / per as f32 + 5.0, tall * 0.5, 0.8),
                0.6,
                Finish::Dark,
            );
        }
    }
    if !b.fine() {
        return;
    }
    for w in line.windows(2) {
        let a = w[0] - v3(0.0, y, tall * 0.35);
        let c = w[1] - v3(0.0, y, tall * 0.35);
        collar(b, a, c - a, tall * 0.4, 2.4);
        metal(b);
        b.cylinder_between(
            a + v3(0.0, y * 0.45, 0.0),
            c + v3(0.0, y * 0.45, 0.0),
            0.5,
            0.5,
            6,
        );
        // Ribs across the trench, the spine to the ridge.
        dark_plate(b);
        let ribs = ((c - a).length() / 7.0) as usize;
        for r in 1..ribs.max(2) {
            let p = a.lerp(c, r as f32 / ribs.max(2) as f32);
            b.beam(
                p + v3(0.0, 0.0, tall * 0.4),
                p + v3(0.0, y * 0.8, tall * 0.4),
                Vec2::new(0.9, 0.6),
                Vec2::new(0.9, 0.6),
            );
        }
        // The trench's floor plate, its red slot let in.
        dark_plate(b);
        b.pattern(pattern::EMBER_SLOT);
        let mid = a.lerp(c, 0.5);
        let half = (c - a).length() * 0.3;
        b.face(&[
            mid + v3(half, 0.05, -tall * 0.12),
            mid + v3(half, y * 0.35, -tall * 0.12),
            mid + v3(-half, y * 0.35, -tall * 0.12),
            mid + v3(-half, 0.05, -tall * 0.12),
        ]);
    }
}

/// The bridge on the spine from `aft` forward to `fore`: a low armoured wedge `half`
/// wide and `tall`, its brow two plates swept up and back into horns, its forward faces
/// lit, plates swept down its flanks. Both sides.
pub(super) fn bridge(
    b: &mut MeshBuilder,
    body: &Body,
    (aft, fore): (f32, f32),
    half: f32,
    tall: f32,
) {
    let base = |x: f32| body.point(x, SPINE).z - 0.6;
    let station = |x: f32, w: f32, h: f32| -> Vec<Vec3> {
        let z = base(x);
        vec![
            v3(x, -w, z),
            v3(x, w, z),
            v3(x, w * 0.82, z + h * 0.55),
            v3(x, w * 0.5, z + h),
            v3(x, -w * 0.5, z + h),
            v3(x, -w * 0.82, z + h * 0.55),
        ]
    };
    let len = fore - aft;
    let ring = [
        station(aft, half, tall * 0.9),
        station(aft + len * 0.55, half * 0.85, tall),
        station(aft + len * 0.85, half * 0.5, tall * 0.6),
        station(fore, 0.4, 0.8),
    ];
    dark_plate(b);
    b.with_facets(|b| b.loft(&ring, true, true));
    if b.coarse() {
        return;
    }
    // The brow: the upper forward faces, lit.
    let (r1, r2) = (ring[1].clone(), ring[2].clone());
    b.mirror_y(|b| {
        let mut quad = vec![r1[2], r1[3], r2[3], r2[2]];
        let n = (quad[1] - quad[0])
            .cross(quad[3] - quad[0])
            .normalize_or_zero();
        let n = if n.x < 0.0 {
            quad.reverse();
            -n
        } else {
            n
        };
        for p in &mut quad {
            *p += n * 0.08;
        }
        dark_plate(b);
        b.pattern(pattern::EMBER_SLOT);
        b.face(&quad);
    });
    b.mirror_y(|b| {
        // The brow's horn plates, swept up and back over the crown.
        let x = aft + len * 0.78;
        let f = Frame::new(
            v3(x, half * 0.3, base(x) + tall * 0.75),
            v3(-1.0, 0.28, 0.18),
            v3(0.1, 0.35, 1.0),
        );
        shard(b, &f, (len * 0.85, half * 0.28, 0.7), 0.8, Finish::Dark);
        // Plates swept down the flanks.
        for k in 0..if b.fine() { 2 } else { 1 } {
            let x = aft + len * (0.5 - 0.22 * k as f32);
            let f = Frame::new(
                v3(x, half * 0.88, base(x) + tall * 0.3),
                v3(-1.0, 0.3, 0.0),
                v3(0.0, 1.0, 0.45),
            );
            shard(b, &f, (len * 0.4, tall * 0.25, 0.4), 0.6, Finish::Dark);
        }
    });
}

/// A canted fin rooted on the deck at `root`: three plates lapped up it, the top one
/// swept highest, `len` long and `half` deep at its root.
pub(super) fn fin(b: &mut MeshBuilder, root: Vec3, len: f32, half: f32) {
    let f = Frame::new(root, v3(-1.0, 0.18, 0.3), v3(0.0, 1.0, -0.3));
    shard(b, &f, (len, half, 0.8), half * 0.3, Finish::Dark);
    if b.coarse() {
        return;
    }
    let g = Frame::new(
        root + v3(-len * 0.38, 0.5, -0.4),
        v3(-1.0, 0.25, 0.32),
        v3(0.0, 1.0, -0.3),
    );
    shard(
        b,
        &g,
        (len * 0.85, half * 0.75, -0.6),
        half * 0.22,
        Finish::Dark,
    );
    if b.fine() {
        let h = Frame::new(
            root + v3(-len * 0.17, 0.6, 0.2),
            v3(-1.0, 0.2, 0.62),
            v3(0.0, 1.0, -0.3),
        );
        shard(
            b,
            &h,
            (len * 0.55, half * 0.5, 0.9),
            half * 0.15,
            Finish::Lit,
        );
    }
}

/// The keel blade under the bow, from `aft` forward to `fore` (`low` to `high`), and
/// its two mandible plates. Both sides.
pub(super) fn prow(b: &mut MeshBuilder, (aft, fore): (f32, f32), (low, high): (f32, f32)) {
    dark_plate(b);
    b.with_facets(|b| {
        b.extrude_y(
            &[
                [aft, low + (high - low) * 0.3],
                [fore, low + (high - low) * 0.6],
                [fore - 6.0, high],
                [aft, high],
            ],
            -0.8,
            0.8,
        )
    });
    if b.coarse() {
        return;
    }
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(fore - 6.0, 1.0, high - 0.4),
            v3(-1.0, 0.35, -0.12),
            v3(0.0, 1.0, -0.4),
        );
        shard(b, &f, ((fore - aft) * 0.7, 1.4, -0.6), 0.6, Finish::Lit);
    });
}

/// Stern drives, each `(mouth, radius, cowl length)`: a plated cowl ribbed round,
/// a graphite ring, the violet throat, and plates swept back off the cowl past the mouth.
pub(super) fn drives(b: &mut MeshBuilder, drives: &[(Vec3, f32, f32)]) {
    for &(mouth, r, len) in drives {
        let sides = b.sides(12);
        dark_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(mouth + Vec3::X * len, mouth, r * 0.85, r * 1.15, sides)
        });
        metal(b);
        b.cylinder_between(mouth, mouth - Vec3::X * 0.8, r, r * 0.95, sides);
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            mouth - Vec3::X * 0.3,
            mouth - Vec3::X * 0.4,
            r * 0.66,
            r * 0.66,
            sides,
        );
        b.add_exhaust(mouth - Vec3::X * 0.4, -Vec3::X, r * 0.7);
        if b.coarse() {
            continue;
        }
        if b.fine() {
            seam(b);
            for k in [0.3, 0.65] {
                let c = mouth + Vec3::X * (len * k);
                let rr = r * (1.15 - 0.3 * k) + 0.2;
                b.cylinder_between(c, c + Vec3::X * 0.8, rr, rr, sides);
            }
        }
        // A plate swept back over the cowl past the mouth.
        let f = Frame::new(
            mouth + v3(len * 0.9, 0.0, r * 1.1),
            v3(-1.0, 0.0, -0.02),
            Vec3::Z,
        );
        shard(b, &f, (len + r * 1.6, r * 0.75, 0.0), 0.6, Finish::Dark);
    }
}

/// Lifts slung under the hull, each `(top, radius, drop)`: a plated nacelle, its seam
/// lit by the plate, two plates swept back off its flanks and the bell under it.
pub(super) fn lifts(b: &mut MeshBuilder, lifts: &[(Vec3, f32, f32)]) {
    for &(at, r, drop) in lifts {
        let mouth = at - Vec3::Z * drop;
        if !b.coarse() {
            let sides = b.sides(8);
            dark_plate(b);
            b.with_facets(|b| {
                b.cylinder_between(mouth + Vec3::Z * (r * 0.5), at, r * 1.35, r * 0.9, sides)
            });
            if b.fine() {
                dark_plate(b);
                b.pattern(pattern::EMBER_RIM);
                let s = mouth + Vec3::Z * (r * 0.5 + drop * 0.35);
                b.cylinder_between(s, s + Vec3::Z * 0.5, r * 1.24, r * 1.22, sides);
                for side in [1.0, -1.0] {
                    let o = mouth + v3(r * 0.4, side * r * 1.2, r * 0.5 + drop * 0.5);
                    let f = Frame::new(o, v3(-1.0, side * 0.3, 0.1), v3(0.0, side, 0.0));
                    shard(b, &f, (r * 3.0, r * 0.45, 0.0), 0.4, Finish::Dark);
                }
            }
        }
        bell(b, mouth, r, r * 0.6);
    }
}

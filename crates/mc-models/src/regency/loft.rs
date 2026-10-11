//! Sculpted Regency structures (the Ark's language, docs/STYLE.md): hard-chined bodies
//! lofted along a path of stations, laid in backswept scales, an armour ridge down the
//! crest with the owner's colour on it, spines along it. The Slipway (`tidebrood`) and
//! the Mews (`mews`) are built from it.

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{dark_plate, nanite_plate, v3};
use super::machine::*;

/// A station of a lofted body: its centre, half its width across the path, half its depth.
#[derive(Clone, Copy)]
pub(super) struct St {
    pub(super) c: Vec3,
    pub(super) w: f32,
    pub(super) h: f32,
}

pub(super) const fn st(x: f32, y: f32, z: f32, w: f32, h: f32) -> St {
    St {
        c: Vec3::new(x, y, z),
        w,
        h,
    }
}

/// The path's direction at station `i`.
pub(super) fn dir(path: &[St], i: usize) -> Vec3 {
    let a = path[i.saturating_sub(1)].c;
    let c = path[(i + 1).min(path.len() - 1)].c;
    (c - a).normalize()
}

/// The section's side (across the path, level) and up.
pub(super) fn axes(d: Vec3) -> (Vec3, Vec3) {
    let s = d.cross(Vec3::Z).try_normalize().unwrap_or(Vec3::Y);
    (s, s.cross(d).normalize())
}

/// The body's section at `s` with direction `d`: a keel, two knuckles, two upper chines
/// and the crest: hard chines, like the Ark's hull.
pub(super) fn section(s: &St, d: Vec3) -> [Vec3; 6] {
    let (side, up) = axes(d);
    let p = |a: f32, b: f32| s.c + side * (a * s.w) + up * (b * s.h);
    [
        p(0.0, -1.0),
        p(-1.0, -0.3),
        p(-0.72, 0.5),
        p(0.0, 1.0),
        p(0.72, 0.5),
        p(1.0, -0.3),
    ]
}

/// The path's length to each station.
pub(super) fn lengths(path: &[St]) -> Vec<f32> {
    let mut acc = vec![0.0];
    for w in path.windows(2) {
        acc.push(acc[acc.len() - 1] + w[0].c.distance(w[1].c));
    }
    acc
}

/// The station `s` metres along the path, and the path's direction there.
pub(super) fn along(path: &[St], acc: &[f32], s: f32) -> (St, Vec3) {
    let s = s.clamp(0.0, acc[acc.len() - 1]);
    let i = acc
        .windows(2)
        .position(|w| s <= w[1])
        .unwrap_or(acc.len() - 2);
    let k = (s - acc[i]) / (acc[i + 1] - acc[i]).max(1e-4);
    let (a, c) = (path[i], path[i + 1]);
    (
        St {
            c: a.c.lerp(c.c, k),
            w: a.w + (c.w - a.w) * k,
            h: a.h + (c.h - a.h) * k,
        },
        (c.c - a.c).normalize(),
    )
}

/// The body lofted along `path`, capped at both ends, in dark plate.
pub(super) fn body(b: &mut MeshBuilder, path: &[St]) {
    let rings: Vec<Vec<Vec3>> = (0..path.len())
        .map(|i| section(&path[i], dir(path, i)).to_vec())
        .collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
}

/// Scales on `faces` of the body (face `j` runs from section point `j` to `j + 1`),
/// laid from `s0` to `s1` metres along the path, one every `step`, lapped toward `s1`
/// (`s1 < s0` laps them back toward the path's start) so their spikes point that way.
/// Below full detail every other one is left off. `glazed` faces show their nanites.
pub(super) fn scales(
    b: &mut MeshBuilder,
    path: &[St],
    faces: &[usize],
    s0: f32,
    s1: f32,
    step: f32,
    glazed: &[usize],
) {
    let acc = lengths(path);
    let sign = (s1 - s0).signum();
    let count = ((s1 - s0).abs() / step).floor() as usize;
    for k in 0..count {
        if !b.fine() && k % 2 == 1 {
            continue;
        }
        let s = s0 + sign * step * (k as f32 + 0.5);
        let (station, d) = along(path, &acc, s);
        let ring = section(&station, d);
        for &j in faces {
            let (p, q) = (ring[j], ring[(j + 1) % 6]);
            let across = q - p;
            let mut n = d.cross(across).normalize();
            let mid = (p + q) * 0.5;
            if n.dot(mid - station.c) < 0.0 {
                n = -n;
            }
            let u = d * sign;
            let f = Frame::new(mid - u * (step * 0.55), u, n);
            if glazed.contains(&j) {
                nanite_plate(b);
            } else {
                dark_plate(b);
            }
            let last = k + 1 == count;
            let len = step * 1.45 + if last { 3.0 } else { 0.0 };
            armour(b, &f, &swept(len, across.length() * 0.48, 0.0, 0.3), 0.7);
        }
    }
}

/// An armour ridge along the body's crest from `s0` to `s1`, the owner's colour on it.
pub(super) fn ridge(b: &mut MeshBuilder, path: &[St], s0: f32, s1: f32) {
    let acc = lengths(path);
    let top = |s: f32| {
        let (station, d) = along(path, &acc, s);
        section(&station, d)[3] + Vec3::Z * 0.4
    };
    let n = 6;
    for k in 0..n {
        let (a, c) = (
            top(s0 + (s1 - s0) * k as f32 / n as f32),
            top(s0 + (s1 - s0) * (k + 1) as f32 / n as f32),
        );
        dark_plate(b);
        b.beam(a, c, Vec2::new(1.6, 1.0), Vec2::new(1.6, 1.0));
        if (1..n - 1).contains(&k) {
            team(b, a + Vec3::Z * 0.52, c + Vec3::Z * 0.52, 0.5);
        }
    }
}

/// Spines along the crest from `s0` to `s1`: tall plates swept back toward `s1`, `rise`
/// high at the middle of the run, shorter toward its ends.
pub(super) fn spines(b: &mut MeshBuilder, path: &[St], s0: f32, s1: f32, count: usize, rise: f32) {
    let acc = lengths(path);
    for k in 0..count {
        let t = (k as f32 + 0.5) / count as f32;
        let (station, d) = along(path, &acc, s0 + (s1 - s0) * t);
        let foot = section(&station, d)[3];
        let u = (d * (s1 - s0).signum() * 0.9 + Vec3::Z).normalize();
        let (side, _) = axes(d);
        let f = Frame::new(foot - u * 1.0, u, side);
        let len = rise * (1.0 - (t - 0.5).abs() * 1.1);
        dark_plate(b);
        armour(b, &f, &swept(len, 1.4, -0.6, 0.35), 0.6);
        let back = Frame::new(foot - u * 1.0, u, -side);
        armour(b, &back, &swept(len, 1.4, 0.6, 0.35), 0.6);
    }
}

/// The owner's colour: a strip from `a` to `c`, `w` either side, facing up.
pub(super) fn team(b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32) {
    let across = Vec3::Z.cross(c - a).normalize() * w;
    b.paint(TEAM);
    b.face(&[a - across, c - across, c + across, a + across]);
}

/// Far off: a body as a three-sided loft through `rings` of its stations, spread along it.
pub(super) fn coarse_body(b: &mut MeshBuilder, path: &[St], rings: usize) {
    let last = path.len() - 1;
    let rings: Vec<Vec<Vec3>> = (0..rings)
        .map(|k| k * last / (rings - 1))
        .map(|i| {
            let ring = section(&path[i], dir(path, i));
            vec![ring[1], ring[3], ring[5]]
        })
        .collect();
    dark_plate(b);
    b.loft(&rings, true, true);
}

/// Far off: the crest's spines as one fin over the body at `at`, `top` high.
pub(super) fn coarse_fin(b: &mut MeshBuilder, at: Vec3, top: f32) {
    dark_plate(b);
    let fin = [
        at + v3(-6.0, 0.0, 0.0),
        at + v3(6.0, 0.0, 0.0),
        at.with_z(top),
    ];
    b.face(&fin);
    b.face(&[fin[2], fin[1], fin[0]]);
}

/// The mirror image across x = 0, for the far side of a symmetrical yard.
pub(super) fn mirrored(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    f(b);
    b.with(Affine3A::from_scale(v3(-1.0, 1.0, 1.0)), f);
}

/// The scales, ridge and spines of a body running across the lot (-x to +x): lapped out
/// from the middle both ways, so the yard is one piece, not a hull with a bow.
pub(super) fn dress(b: &mut MeshBuilder, path: &[St], spine_rise: f32, spine_count: usize) {
    let len = lengths(path)[path.len() - 1];
    let mid = len * 0.5;
    let step = 4.4;
    for (s1, glaze) in [(len, [4usize]), (0.0, [2usize])] {
        scales(b, path, &[1, 2, 4, 5], mid, s1, step, &glaze);
    }
    ridge(b, path, mid - len * 0.3, mid + len * 0.3);
    if spine_count > 0 {
        spines(b, path, mid + 3.0, len - 6.0, spine_count, spine_rise);
        spines(b, path, mid - 3.0, 6.0, spine_count, spine_rise);
    }
}

pub(super) fn mirror_st(path: &[St]) -> Vec<St> {
    path.iter()
        .map(|p| St {
            c: p.c * v3(-1.0, 1.0, 1.0),
            ..*p
        })
        .collect()
}

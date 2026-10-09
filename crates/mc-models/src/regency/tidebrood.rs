//! The Slipway, the Regency naval factory, on its 8 x 8 lot (96 m square): a scaled shell
//! crouched behind the berth on clawed legs that plunge through the water to the seabed,
//! its arms reaching out over the berth to hang the fabricator heads over the hull.
//!
//! Its origin is the water's surface. The berth is open water at the lot origin, where
//! the hull floats while it is printed; everything the Slipway has stands behind it (-y,
//! nothing past y = -2.5), so a capital ship longer or wider than the lot still fits.
//!
//! - The shell is a hard-chined body lofted across the lot (the Ark's language), laid in
//!   backswept scales lapped out from the middle both ways, a glazed course on each side
//!   where the nanites show, an armour ridge down its crest with the owner's colour on
//!   it, and spines along the crest.
//! - Eight clawed legs, four a side, reach out off its flanks and down through the water;
//!   the shader lets each foot down onto the seabed however deep it is (`part::PILE`), so
//!   nothing is levelled under it.
//! - The heads (their mounts are `mc_core::print_heads`) hang under scaled arms off the
//!   shell's front. The violet runs hot while it builds.
//!
//! Each tier adds reach, not light: tech 2 (Slipway II) a second, taller pair of arms at
//! the shell's ends and taller spines, tech 3 (Slipway III) an arch sprung off the shell's
//! ends over the front of the berth, three heads under it for the capital ships.

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::kit::{dark_plate, nanite_plate, seam, v3};
use super::machine::*;

const MESH: &str = "regency_tidebrood";

fn heads(tier: u8) -> impl Iterator<Item = (Vec3, f32, Vec3)> {
    tier_heads(MESH, tier)
}

// ---- The kit: lofted bodies along a path, laid in scales -------------------------------

/// A station of a lofted body: its centre, half its width across the path, half its depth.
#[derive(Clone, Copy)]
struct St {
    c: Vec3,
    w: f32,
    h: f32,
}

const fn st(x: f32, y: f32, z: f32, w: f32, h: f32) -> St {
    St {
        c: Vec3::new(x, y, z),
        w,
        h,
    }
}

/// The path's direction at station `i`.
fn dir(path: &[St], i: usize) -> Vec3 {
    let a = path[i.saturating_sub(1)].c;
    let c = path[(i + 1).min(path.len() - 1)].c;
    (c - a).normalize()
}

/// The section's side (across the path, level) and up.
fn axes(d: Vec3) -> (Vec3, Vec3) {
    let s = d.cross(Vec3::Z).try_normalize().unwrap_or(Vec3::Y);
    (s, s.cross(d).normalize())
}

/// The body's section at `s` with direction `d`: a keel, two knuckles, two upper chines
/// and the crest: hard chines, like the Ark's hull.
fn section(s: &St, d: Vec3) -> [Vec3; 6] {
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
fn lengths(path: &[St]) -> Vec<f32> {
    let mut acc = vec![0.0];
    for w in path.windows(2) {
        acc.push(acc[acc.len() - 1] + w[0].c.distance(w[1].c));
    }
    acc
}

/// The station `s` metres along the path, and the path's direction there.
fn along(path: &[St], acc: &[f32], s: f32) -> (St, Vec3) {
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
fn body(b: &mut MeshBuilder, path: &[St]) {
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
fn scales(
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
fn ridge(b: &mut MeshBuilder, path: &[St], s0: f32, s1: f32) {
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
fn spines(b: &mut MeshBuilder, path: &[St], s0: f32, s1: f32, count: usize, rise: f32) {
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
fn team(b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32) {
    let across = Vec3::Z.cross(c - a).normalize() * w;
    b.paint(TEAM);
    b.face(&[a - across, c - across, c + across, a + across]);
}

/// A head hung on a plated drop from `from_z` (the underside of what carries it).
fn hang(b: &mut MeshBuilder, mount: Vec3, s: f32, aim: Vec3, from_z: f32) {
    dark_plate(b);
    b.beam(
        v3(mount.x, mount.y, from_z),
        mount + Vec3::Z * 0.3,
        Vec2::new(1.4, 1.2),
        Vec2::new(1.0, 0.9),
    );
    if b.fine() {
        collar(b, v3(mount.x, mount.y, from_z - 0.4), Vec3::X, 0.8, 2.2);
    }
    fabricator(b, mount, aim, s);
}

/// A scaled arm along `path` (root to tip), the head at `mount` hung under it where the
/// arm passes over it.
fn arm(b: &mut MeshBuilder, path: &[St], mount: Vec3, s: f32, aim: Vec3) {
    body(b, path);
    let acc = lengths(path);
    let len = acc[acc.len() - 1];
    scales(b, path, &[2, 3, 4], len - 1.0, 0.0, len / 4.0, &[]);
    let under = (0..=40)
        .map(|k| along(path, &acc, len * k as f32 / 40.0).0)
        .min_by(|a, c| {
            a.c.truncate()
                .distance(mount.truncate())
                .total_cmp(&c.c.truncate().distance(mount.truncate()))
        })
        .map_or(mount.z + 1.3, |st| st.c.z - st.h * 0.8);
    hang(b, mount, s, aim, under);
}

/// Far off: a body as a three-sided loft through `rings` of its stations, spread along it.
fn coarse_body(b: &mut MeshBuilder, path: &[St], rings: usize) {
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
fn coarse_fin(b: &mut MeshBuilder, at: Vec3, top: f32) {
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
fn mirrored(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    f(b);
    b.with(Affine3A::from_scale(v3(-1.0, 1.0, 1.0)), f);
}

/// The scales, ridge and spines of a body running across the lot (-x to +x): lapped out
/// from the middle both ways, so the yard is one piece, not a hull with a bow.
fn dress(b: &mut MeshBuilder, path: &[St], spine_rise: f32, spine_count: usize) {
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

fn mirror_st(path: &[St]) -> Vec<St> {
    path.iter()
        .map(|p| St {
            c: p.c * v3(-1.0, 1.0, 1.0),
            ..*p
        })
        .collect()
}

/// The arm on either side for each of `tier`'s heads: `path` is the +x arm.
fn arms(b: &mut MeshBuilder, tier: u8, path: &[St]) {
    for (mount, s, aim) in heads(tier) {
        if mount.x > 0.0 {
            arm(b, path, mount, s, aim);
        } else {
            arm(b, &mirror_st(path), mount, s, aim);
        }
    }
}

/// Tech 3's arch over the front of the berth, sprung off the shell's ends.
const ARCH: [St; 9] = [
    st(-42.0, -18.0, 14.0, 2.4, 2.2),
    st(-37.0, -10.0, 23.0, 2.0, 1.9),
    st(-29.0, -5.0, 29.4, 1.8, 1.6),
    st(-15.0, -4.4, 31.2, 1.7, 1.5),
    st(0.0, -4.3, 31.6, 1.7, 1.5),
    st(15.0, -4.4, 31.2, 1.7, 1.5),
    st(29.0, -5.0, 29.4, 1.8, 1.6),
    st(37.0, -10.0, 23.0, 2.0, 1.9),
    st(42.0, -18.0, 14.0, 2.4, 2.2),
];

/// Tech 3: the arch, its heads hung under it.
fn arch(b: &mut MeshBuilder) {
    body(b, &ARCH);
    let len = lengths(&ARCH)[ARCH.len() - 1];
    scales(b, &ARCH, &[2, 3, 4], len * 0.5, len, 3.6, &[]);
    scales(b, &ARCH, &[2, 3, 4], len * 0.5, 0.0, 3.6, &[]);
    for (mount, s, aim) in heads(3) {
        hang(b, mount, s, aim, 29.6);
    }
}

/// The shell: low and broad behind the berth, crouched on its legs.
const SHELL: [St; 9] = [
    st(-41.0, -32.0, 6.0, 1.0, 1.0),
    st(-37.5, -32.0, 7.0, 7.0, 4.2),
    st(-30.0, -33.0, 8.0, 10.6, 5.6),
    st(-15.0, -34.0, 8.6, 12.0, 6.2),
    st(0.0, -34.0, 8.8, 12.4, 6.4),
    st(15.0, -34.0, 8.6, 12.0, 6.2),
    st(30.0, -33.0, 8.0, 10.6, 5.6),
    st(37.5, -32.0, 7.0, 7.0, 4.2),
    st(41.0, -32.0, 6.0, 1.0, 1.0),
];

/// A clawed leg (+x side): off the shell's flank, out and down through the water to the
/// seabed, its last station on the surface so the shader lets it down to the bottom.
fn leg(from: Vec3, out: Vec2) -> [St; 4] {
    // Its reach is how long `out` is.
    let (o, r) = (out.normalize(), out.length() / 12.0);
    let p = |k: f32, z: f32| (from.truncate() + o * (k * r)).extend(z);
    let at = |c: Vec3, w: f32, h: f32| St { c, w, h };
    [
        at(p(0.0, from.z), 2.4, 2.2),
        at(p(5.0, from.z + 3.0), 2.0, 1.9),
        at(p(10.0, 2.6), 1.6, 1.6),
        at(p(12.0, 1.0), 1.0, 1.0),
    ]
}

pub(super) fn tidebrood(b: &mut MeshBuilder, tech: u8) {
    let legs = [
        (v3(10.0, -24.0, 5.0), Vec2::new(1.8, 12.0)),
        (v3(30.0, -26.0, 5.0), Vec2::new(7.2, 9.6)),
        (v3(35.0, -37.0, 5.0), Vec2::new(5.6, -4.2)),
        (v3(15.0, -41.0, 5.0), Vec2::new(1.6, -4.8)),
    ];
    if b.coarse() {
        coarse_body(b, &SHELL, 3);
        coarse_fin(b, v3(0.0, -34.0, 14.0), if tech >= 2 { 26.0 } else { 21.0 });
        team(b, v3(-8.0, -34.0, 15.3), v3(8.0, -34.0, 15.3), 1.8);
        b.with_part(part::PILE, |b| {
            dark_plate(b);
            mirrored(b, |b| {
                for (from, out) in legs.into_iter().take(2) {
                    let foot = (from.truncate() + out).extend(0.0);
                    let s = Vec3::Z.cross(foot - from).normalize() * 1.2;
                    let quad = [from - s, foot - s, foot + s, from + s];
                    b.face(&quad);
                    let mut back = quad;
                    back.reverse();
                    b.face(&back);
                }
            });
        });
        if tech >= 3 {
            coarse_body(b, &ARCH, 3);
        }
        return;
    }
    body(b, &SHELL);
    dress(b, &SHELL, 9.0, 3);
    mirrored(b, |b| {
        for (from, out) in legs {
            let path = leg(from, out);
            // The leg's foot: the shader lets what is at the surface down to the seabed.
            b.with_part(part::PILE, |b| {
                body(b, &path);
                seam(b);
                let foot = path[3].c;
                b.beam(
                    foot,
                    foot.with_z(0.0),
                    Vec2::new(1.8, 1.8),
                    Vec2::new(2.6, 2.6),
                );
            });
            let len = lengths(&path)[path.len() - 1];
            b.with_part(part::PILE, |b| {
                scales(b, &path, &[2, 3, 4], len - 3.0, 0.0, len / 4.0, &[]);
            });
            if b.fine() {
                collar(b, from, out.extend(0.0), 2.2, 1.8);
            }
        }
    });
    let arm_1 = [
        st(12.0, -26.0, 12.0, 2.6, 2.2),
        st(13.0, -20.0, 15.6, 2.2, 1.8),
        st(14.0, -14.0, 17.6, 1.8, 1.4),
        st(14.0, -9.5, 18.0, 1.5, 1.2),
        st(14.0, -5.0, 17.6, 0.3, 0.3),
    ];
    arms(b, 1, &arm_1);
    tier(b, tech, 2, 0.2, |b| {
        let arm_2 = [
            st(30.0, -26.0, 12.0, 2.6, 2.2),
            st(31.5, -18.0, 18.4, 2.2, 1.8),
            st(32.0, -13.0, 21.4, 1.8, 1.4),
            st(32.0, -9.0, 22.0, 1.5, 1.2),
            st(32.0, -5.0, 21.6, 0.3, 0.3),
        ];
        arms(b, 2, &arm_2);
        let len = lengths(&SHELL)[SHELL.len() - 1];
        spines(b, &SHELL, len * 0.5 + 2.0, len * 0.5 + 24.0, 3, 14.0);
        spines(b, &SHELL, len * 0.5 - 2.0, len * 0.5 - 24.0, 3, 14.0);
    });
    tier(b, tech, 3, 0.25, arch);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tidebrood_fits_its_lot_at_every_tier() {
        let key = "regency_tidebrood";
        for (tech, height) in [(1, 20.0), (2, 24.0), (3, 32.0)] {
            super::super::check_at(key, tech, 46.0, height, Some(8), &[]);
            super::super::check_heads_at(key, tech, 46.0, height);
        }
    }
}

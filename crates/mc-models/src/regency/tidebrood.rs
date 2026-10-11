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

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::part;

use super::kit::{dark_plate, seam, v3};
use super::loft::*;
use super::machine::*;

const MESH: &str = "regency_tidebrood";

fn heads(tier: u8) -> impl Iterator<Item = (Vec3, f32, Vec3)> {
    tier_heads(MESH, tier)
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

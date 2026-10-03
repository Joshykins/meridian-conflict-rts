//! The Material Fabricator (mesh `fabricator`): a plant on a 4×4 lot that pours the grid's
//! energy into a sealed formation chamber and draws off a trickle of material. It reads at a
//! glance from the grid side (capacitor banks and the conduits out of them) through the
//! field coils round the chamber to a small hopper of product, and it looks like a bad
//! place to stand: a pressure vessel in hazard bands. It upgrades in place: each tier adds
//! working machinery (coils, banks, cooling), never spikes or glow for menace.
//!
//! Three designs are on the table (`fabricator`, `fabricator~ring`, `fabricator~line`), all
//! built from the kit here.

pub(super) mod line;
pub(super) mod ring;
pub(super) mod vessel;

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// The (radius, height) each tier is authored at: a 4×4 lot (half-extent 24 m), as tall
/// as the blueprint (`data/factions/aster/units/structures.ron`).
pub(super) const SIZES: [(f32, f32); 3] = [(23.0, 14.0), (23.0, 22.0), (23.0, 30.0)];

/// Half-width of the slab, and the top of its deck.
pub(super) const SLAB: f32 = 21.5;
pub(super) const DECK: f32 = 1.6;

/// The slab every design stands on: a dark foot, a plated deck with hazard-striped edges,
/// the owner's colour on all four sides.
pub(super) fn slab(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, DECK * 0.5), v3(SLAB * 2.0, SLAB * 2.0, DECK));
        return;
    }
    let plan = chamfered_rect(v2(SLAB, SLAB), 4.0);
    if !b.fine() {
        b.paint(PLATING);
        b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(DECK, 0.97)]);
        return;
    }
    b.paint(ACCENT);
    b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(0.7, 1.0)]);
    b.paint(PLATING);
    b.loft_z(&plan, &[Section::new(0.7, 0.99), Section::new(DECK, 0.97)]);
    if b.fine() {
        b.radial(4, |b| {
            b.paint(PLATING).pattern(pattern::HAZARD);
            b.plate(v3(SLAB - 1.4, 0.0, DECK), v2(0.7, 22.0), 0.06, 0.02);
        });
    }
}

/// A team panel on the deck at each side's middle, `out` from the centre.
pub(super) fn deck_marks(b: &mut MeshBuilder, out: f32) {
    b.radial(4, |b| team_panel(b, v3(out, 0.0, DECK), v2(1.0, 7.0)));
}

/// A capacitor bank standing on the deck, its row along y at `at`: a dark plinth and
/// `cans` hexagonal cans on it, each with a lit charge band, `h` tall.
pub(super) fn bank(b: &mut MeshBuilder, at: Vec3, cans: usize, h: f32) {
    let pitch = 2.6;
    let len = pitch * cans as f32 + 0.8;
    if b.coarse() {
        b.paint(PLATING_DARK);
        b.cuboid_open(at + Vec3::Z * (h * 0.5), v3(3.2, len, h));
        return;
    }
    b.paint(PLATING_DARK);
    if !b.fine() {
        // The middle distance: the plinth, each can a plain block with its lit top.
        b.cuboid_open(at + Vec3::Z * 0.5, v3(3.4, len, 1.0));
        for k in 0..cans {
            let y = (k as f32 - (cans as f32 - 1.0) * 0.5) * pitch;
            b.paint(PLATING);
            b.cuboid_open(
                at + v3(0.0, y, 1.0 + (h - 1.2) * 0.5),
                v3(1.9, 1.9, h - 1.2),
            );
            b.paint(GLOW).pattern(pattern::NONE);
            b.decal(at + v3(0.0, y, h - 0.18), v2(1.3, 1.3));
        }
        return;
    }
    b.chamfered_box(at + Vec3::Z * 0.5, v3(3.4, len, 1.0), 0.3);
    for k in 0..cans {
        let y = (k as f32 - (cans as f32 - 1.0) * 0.5) * pitch;
        let base = at + v3(0.0, y, 1.0);
        b.paint(PLATING);
        b.prism(base, 6, 1.15, 1.15, h - 1.8);
        b.paint(PLATING_DARK);
        b.prism(base + Vec3::Z * (h - 1.8), 6, 1.15, 0.7, 0.8);
        b.paint(GLOW).pattern(pattern::NONE);
        b.prism(base + Vec3::Z * ((h - 1.8) * 0.62), 6, 1.2, 1.2, 0.35);
        b.paint(METAL);
        b.prism(base + Vec3::Z * (h - 1.0), 6, 0.25, 0.2, 0.6);
    }
}

/// A power run: a dark duct through `path` carrying lit pulses toward its end
/// (`pattern::CONDUIT`), `w` across.
pub(super) fn conduit(b: &mut MeshBuilder, path: &[Vec3], w: f32) {
    if b.coarse() {
        return;
    }
    b.paint(PLATING_DARK).pattern(pattern::CONDUIT);
    for p in path.windows(2) {
        b.beam(p[0], p[1], v2(w, w), v2(w, w));
    }
    // Bolted flanges where the run leaves and where it goes in.
    if b.fine() {
        b.paint(ACCENT);
        let n = path.len();
        for (a, c) in [(path[0], path[1]), (path[n - 1], path[n - 2])] {
            let d = (c - a).normalize_or(Vec3::X);
            let f = Vec2::splat(w * 1.35);
            b.beam(a + d * 0.2, a + d * 0.55, f, f);
        }
    }
}

/// A field coil round the z axis at `z`: a dark ring `r` to `r + depth` out, `h` tall, its
/// winding lit round the outside with charge running round it (`pattern::CHARGE`).
pub(super) fn coil(b: &mut MeshBuilder, c: Vec3, r: f32, depth: f32, h: f32) {
    coil_on(b, c, Vec3::Z, r, depth, h);
}

/// [`coil`] about `axis` through `c` instead of z (an accelerator line's coils).
pub(super) fn coil_on(b: &mut MeshBuilder, c: Vec3, axis: Vec3, r: f32, depth: f32, h: f32) {
    if b.fine() {
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        annulus_on(b, c, axis, 8, r, r + depth, 0.0, h);
        b.paint(GLOW).pattern(pattern::CHARGE);
        band_on(b, c, axis, r + depth + 0.04, h * 0.3, h * 0.7);
    } else {
        // Further off, the winding's light alone, round what it wraps.
        b.paint(GLOW).pattern(pattern::CHARGE);
        band_on(b, c, axis, r + depth * 0.5, h * 0.15, h * 0.85);
    }
}

/// An octagonal pressure vessel round the z axis at `c`: a flared foot ringed in hazard
/// stripes, a straight body of `r` from `z0` to `z1`, a shoulder in to a sealed hatch.
/// Lit slits onto the forming matter wrap the body (`pattern::FUSION`).
pub(super) fn vessel(b: &mut MeshBuilder, c: Vec3, r: f32, z0: f32, z1: f32) {
    let plan = ngon(8, 1.0);
    let shoulder = z1 + r * 0.35;
    b.at(c, |b| {
        if b.coarse() {
            b.paint(PLATING);
            b.loft_z(
                &ngon(4, r * 1.25),
                &[Section::new(z0, 1.0), Section::new(shoulder, 0.6)],
            );
            return;
        }
        b.paint(PLATING);
        if !b.fine() {
            b.loft_z(
                &plan,
                &[
                    Section::new(z0, r * 1.1),
                    Section::new(z1, r),
                    Section::new(shoulder, r * 0.62),
                ],
            );
            team_panel(b, v3(0.0, 0.0, shoulder), Vec2::splat(r * 0.55));
            return;
        }
        b.with_facets(|b| {
            b.loft_z(
                &plan,
                &[
                    Section::new(z0, r * 1.18),
                    Section::new(z0 + 1.0, r * 1.18),
                    Section::new(z0 + 1.6, r),
                    Section::new(z1, r),
                    Section::new(shoulder, r * 0.62),
                ],
            )
        });
        // The hatch: a dark collar, a lid with the owner's colour on it.
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[
                Section::new(shoulder - 0.1, r * 0.6),
                Section::new(shoulder + 0.7, r * 0.56),
            ],
        );
        team_panel(b, v3(0.0, 0.0, shoulder + 0.7), Vec2::splat(r * 0.55));
        // Hazard band round the foot, and the slits round the body.
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.loft_z(
            &plan,
            &[
                Section::new(z0 + 0.15, r * 1.19),
                Section::new(z0 + 0.85, r * 1.19),
            ],
        );
        let (lo, hi) = (z0 + 2.4, z1 - 1.0);
        if hi > lo + 1.0 {
            b.radial(8, |b| {
                b.yawed(Vec3::ZERO, std::f32::consts::TAU / 16.0, |b| {
                    let x = r * (std::f32::consts::PI / 8.0).cos();
                    b.paint(GLOW).pattern(pattern::FUSION);
                    b.cuboid(
                        v3(x + 0.04, 0.0, (lo + hi) * 0.5),
                        v3(0.12, r * 0.18, hi - lo),
                    );
                });
            });
        }
    });
}

/// The output: a chute from `from` on the chamber down to a hopper at `at` on the deck,
/// heaped with ore-red product. The chute's glazing shows the material going down it
/// (`pattern::MASS_FLOW`).
pub(super) fn hopper(b: &mut MeshBuilder, from: Vec3, at: Vec3, size: f32) {
    b.paint(ACCENT);
    if b.coarse() {
        b.cuboid_open(at + Vec3::Z * (size * 0.5), Vec3::splat(size));
        return;
    }
    let into = at + v3(0.0, 0.0, size * 1.2);
    b.frustum(
        at,
        v2(size * 0.55, size * 0.55),
        v2(size, size),
        size,
        Vec2::ZERO,
    );
    if !b.fine() {
        b.beam(from, into, v2(1.0, 1.0), v2(0.9, 0.9));
        b.paint(GLOW_ORANGE).pattern(pattern::NONE);
        b.decal(at + Vec3::Z * (size + 0.02), v2(size * 0.6, size * 0.6));
        return;
    }
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.frustum_open(
        at + Vec3::Z * (size * 0.86),
        v2(size * 1.03, size * 1.03),
        v2(size * 1.03, size * 1.03),
        size * 0.14,
        Vec2::ZERO,
    );
    // The heap of product, its top lit where it is still hot.
    b.paint(ROCK);
    b.frustum(
        at + Vec3::Z * (size - 0.05),
        v2(size * 0.9, size * 0.9),
        v2(size * 0.3, size * 0.25),
        size * 0.3,
        Vec2::ZERO,
    );
    b.paint(GLOW_ORANGE).pattern(pattern::NONE);
    b.decal(
        at + Vec3::Z * (size * 1.25 + 0.02),
        v2(size * 0.26, size * 0.2),
    );
    // The chute, in to the hopper's top.
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.beam(from, into, v2(1.0, 1.0), v2(0.9, 0.9));
    b.paint(METAL);
    b.cylinder_between(into, into - Vec3::Z * 0.7, 0.6, 0.6, 6);
}

/// A radiator along `along` from `at`, `len` long and `w` across: a dark housing, hot
/// coolant glowing between its fins, the fins under a clamp bar.
pub(super) fn radiator(b: &mut MeshBuilder, at: Vec3, along: Vec3, len: f32, w: f32) {
    let base = 0.8;
    let fin = w * 0.7;
    b.yawed(at, along.y.atan2(along.x), |b| {
        b.paint(ACCENT);
        if b.coarse() {
            b.cuboid_open(
                v3(len * 0.5, 0.0, (base + fin) * 0.5),
                v3(len, w, base + fin),
            );
            return;
        }
        if b.fine() {
            b.chamfered_box(v3(len * 0.5, 0.0, base * 0.5), v3(len, w, base), 0.2);
        } else {
            b.cuboid_open(v3(len * 0.5, 0.0, base * 0.5), v3(len, w, base));
        }
        b.paint(GLOW_ORANGE).pattern(pattern::HEAT);
        b.cuboid_open(
            v3(len * 0.5, 0.0, base + fin * 0.2),
            v3(len * 0.9, w * 0.5, fin * 0.4),
        );
        let n = if b.fine() { 8 } else { 3 };
        let pitch = len * 0.9 / n as f32;
        b.paint(PLATING);
        for k in 0..n {
            let x = len * 0.05 + pitch * (k as f32 + 0.5);
            b.cuboid_open(
                v3(x, 0.0, base + fin * 0.5),
                v3(pitch * 0.35, w * 0.86, fin),
            );
        }
        if b.fine() {
            b.paint(ACCENT);
            b.cuboid(
                v3(len * 0.5, 0.0, base + fin + 0.12),
                v3(len * 0.94, w * 0.14, 0.24),
            );
        }
    });
}

/// A flat ring about `axis` through `c`, `r0` to `r1` out and `a0` to `a1` along the axis,
/// its flats square to the frame (a flat faces +x for the z axis, like the vessel's).
#[expect(
    clippy::too_many_arguments,
    reason = "a ring's place, axis, facets and four measures"
)]
pub(super) fn annulus_on(
    b: &mut MeshBuilder,
    c: Vec3,
    axis: Vec3,
    sides: usize,
    r0: f32,
    r1: f32,
    a0: f32,
    a1: f32,
) {
    let (e1, e2, axis) = ring_frame(axis);
    let section = [(r0, a0), (r1, a0), (r1, a1), (r0, a1)];
    let rings: Vec<Vec<Vec3>> = (0..=sides)
        .map(|i| {
            let a = (i as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
            let d = e1 * a.cos() + e2 * a.sin();
            section.iter().map(|&(r, h)| c + d * r + axis * h).collect()
        })
        .collect();
    b.loft(&rings, false, false);
}

/// The outer face of a ring about `axis` through `c`: eight flats `r` out, `a0` to `a1`
/// along the axis, open at both ends.
pub(super) fn band_on(b: &mut MeshBuilder, c: Vec3, axis: Vec3, r: f32, a0: f32, a1: f32) {
    let (e1, e2, axis) = ring_frame(axis);
    let ring = |h: f32| -> Vec<Vec3> {
        (0..8)
            .map(|i| {
                let a = (i as f32 + 0.5) * std::f32::consts::TAU / 8.0;
                c + (e1 * a.cos() + e2 * a.sin()) * r + axis * h
            })
            .collect()
    };
    b.loft(&[ring(a0), ring(a1)], false, false);
}

/// Two axes square to `axis` (the first along x for an upright axis, else nearest z), and
/// the axis made unit.
fn ring_frame(axis: Vec3) -> (Vec3, Vec3, Vec3) {
    let axis = axis.normalize();
    let e1 = if axis.z.abs() > 0.9 { Vec3::X } else { Vec3::Z };
    let e1 = (e1 - axis * e1.dot(axis)).normalize();
    (e1, axis.cross(e1), axis)
}

/// A plated strut from `a` to `c`, `w` across: what holds a containment frame together.
pub(super) fn brace(b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32) {
    b.paint(PLATING_DARK);
    b.beam(a, c, v2(w, w), v2(w * 0.85, w * 0.85));
}

#[cfg(test)]
mod tests {
    /// Prints each design's triangles per level of detail and tier, and the reduced
    /// level's share of the full one (the library rule is 0.45):
    /// `cargo test --profile gate -p mc-models --lib zz_fabricator_counts -- --ignored --nocapture`.
    #[test]
    #[ignore = "a probe: prints triangle counts"]
    fn zz_fabricator_counts() {
        for key in ["fabricator", "fabricator~ring", "fabricator~line"] {
            for (i, (r, h)) in super::SIZES.into_iter().enumerate() {
                let m = crate::build_model_scaled(key, r, h, i as u8 + 1).unwrap();
                let n: Vec<usize> = m.lods.iter().map(|l| l.indices.len() / 3).collect();
                println!("{key} T{}: {n:?} {:.2}", i + 1, n[1] as f32 / n[0] as f32);
            }
        }
    }

    use super::SIZES;
    use crate::{build_model_scaled, rig};

    /// Each design carries the next tier's machinery as upgrade pieces and grows by tier.
    #[test]
    fn every_design_upgrades_in_place_and_grows() {
        for key in ["fabricator", "fabricator~ring", "fabricator~line"] {
            let mut last = 0;
            for (i, (r, h)) in SIZES.into_iter().enumerate() {
                let tech = i as u8 + 1;
                let model = build_model_scaled(key, r, h, tech).unwrap();
                let lod = &model.lods[0];
                let upgrade = lod.vertices.iter().any(|v| v.rig & rig::UPGRADE != 0);
                assert_eq!(upgrade, tech < 3, "{key} T{tech}: upgrade pieces");
                let own = lod
                    .indices
                    .iter()
                    .filter(|&&i| lod.vertices[i as usize].rig & rig::UPGRADE == 0)
                    .count();
                assert!(own > last, "{key} T{tech} adds machinery");
                last = own;
            }
        }
    }
}

//! The Condenser (mesh `regency_fabricator`), the Regency's material fabricator: energy
//! cells on the grid side (-x) cabled into a sealed vessel where gravity squeezes the charge
//! until matter condenses out of it, and a small bin of product on the far side (+x). Each
//! tier is a building of its own on its power generator's lot (2x2, 4x4, 8x8), with more
//! working machinery the higher it goes (more cells, a taller vessel, more field gear), in
//! the Regency's dark lapped plate over graphite machinery, red where it runs hot.
//!
//! Three designs are on the table (`regency_fabricator`, `~orbit`, `~press`), all built
//! from the kit here.

pub(super) mod orbit;
pub(super) mod press;
pub(super) mod vessel;

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;

use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::*;

/// The (radius, height) of each tier, the blueprint's: its own lot, as big as that tier's
/// power plant's.
pub(super) const SIZES: [(f32, f32); 3] = [(11.0, 7.0), (23.0, 22.0), (42.5, 34.0)];
/// Each tier's lot, in build cells a side.
#[cfg(test)]
const LOTS: [u32; 3] = [2, 4, 8];
/// The plant is authored on the 4x4 lot: the tech 1 building is that plant's first tier
/// drawn down to a 2x2 lot, and the tech 3 one its full plant drawn up in the middle of
/// an 8x8 lot, with the yard round it.
const T1_SCALE: f32 = 0.52;
const T3_SCALE: f32 = 1.3;

/// Draws `plant`'s building for `tech`: each tier a building of its own (see `T1_SCALE`).
pub(super) fn standalone(b: &mut MeshBuilder, tech: u8, plant: fn(&mut MeshBuilder, u8)) {
    match tech.clamp(1, 3) {
        1 => b.with(Affine3A::from_scale(Vec3::splat(T1_SCALE)), |b| plant(b, 1)),
        2 => plant(b, 2),
        _ => {
            yard(b);
            b.with(Affine3A::from_scale(Vec3::splat(T3_SCALE)), |b| plant(b, 3));
        }
    }
}

/// A tier's machinery in a plant drawn at `tech`: there when the building has that tier.
pub(super) fn fitted(b: &mut MeshBuilder, tech: u8, tier: u8, f: impl FnOnce(&mut MeshBuilder)) {
    if tier <= tech {
        f(b);
    }
}

/// A plate's thickness.
pub(super) const THICK: f32 = 0.5;

/// An energy cell lying along y at `at` (its axis `z` up), `half` its half length, `r` its
/// radius: plated saddles, the drum in graphite bands, end caps, a red charge slot along its
/// outer shoulder (toward -x) and plates lapped back along its top.
pub(super) fn cell(b: &mut MeshBuilder, at: Vec3, half: f32, r: f32) {
    let axis = |y: f32| at + Vec3::Y * y;
    if b.coarse() {
        dark_plate(b);
        b.cylinder_between(axis(-half), axis(half), r, r, 3);
        return;
    }
    let fine = b.fine();
    let sides = b.sides(10);
    let saddles: &[f32] = if fine {
        &[-0.7, 0.0, 0.7]
    } else {
        &[-0.6, 0.6]
    };
    for &k in saddles {
        seam(b);
        b.frustum_open(
            v3(at.x, at.y + k * half, 0.0),
            Vec2::new(r * 2.4, 1.8),
            Vec2::new(r * 1.6, 1.4),
            at.z - r * 0.4,
            Vec2::ZERO,
        );
    }
    dark_plate(b);
    b.cylinder_between(axis(-half), axis(half), r, r, sides);
    metal(b);
    b.cylinder_between(axis(half), axis(half + 1.2), r * 0.8, r * 0.35, sides);
    b.cylinder_between(axis(-half), axis(-half - 1.2), r * 0.8, r * 0.35, sides);
    if !fine {
        return;
    }
    for k in [-0.75f32, -0.25, 0.25, 0.75] {
        collar(b, axis(k * half), Vec3::Y, r + 0.15, 0.45);
    }
    let f = Frame::new(axis(half - 0.4) + Vec3::Z * (r - 0.1), -Vec3::Y, Vec3::Z);
    dark_plate(b);
    Course {
        count: 2,
        step: half * 0.9,
        len: half * 1.1,
        half: r * 0.5,
        tip: 0.0,
        thick: THICK,
        tail: 0.8,
    }
    .lay(b, &f);
    red_slot(
        b,
        at + v3(-r * 0.72, 0.0, r * 0.69),
        v3(-0.7, 0.0, 0.7),
        Vec3::Y,
        half * 1.2,
        0.22,
    );
}

/// How far out on either flank the cells lie.
pub(super) const FLANK: f32 = 14.0;

/// A cell on each flank (±y) lying along x at `x`, `half` its half length and `r` its
/// radius, its charge slot facing in, each cabled from its top to `to(side)` (side ±1).
pub(super) fn flank_cells(
    b: &mut MeshBuilder,
    x: f32,
    half: f32,
    r: f32,
    to: impl Fn(f32) -> Vec3,
) {
    let z = r + 0.6;
    for s in [-1.0f32, 1.0] {
        let at = v3(x, s * FLANK, 0.0);
        b.yawed(at, s * std::f32::consts::FRAC_PI_2, |b| {
            cell(b, v3(0.0, 0.0, z), half, r)
        });
        feed(b, v3(x, s * (FLANK - r * 0.4), z + r * 0.9), to(s));
    }
}

/// A graphite feed from a cell's top at `from` to the vessel at `to`, in two runs (up, then
/// across), at full detail.
pub(super) fn feed(b: &mut MeshBuilder, from: Vec3, to: Vec3) {
    if !b.fine() {
        return;
    }
    metal(b);
    let over = v3(from.x, from.y, to.z);
    let run = (to - over).normalize_or(Vec3::X);
    let across = v3(-run.y, run.x, 0.0) * 0.5;
    for d in [-across, across] {
        cable(b, &[from + d, over + d, to + d], 0.28);
    }
}

/// An armoured octagonal vessel round the z axis at `c`: a seam footing, plates from `z0` to
/// `z1` at radius `r`, drawn in to `r * 0.7` at a shoulder, the owner's colour on a graphite
/// hatch. At full detail, a course of plates lapped up each other face.
pub(super) fn vessel(b: &mut MeshBuilder, c: Vec3, r: f32, z0: f32, z1: f32) -> f32 {
    let shoulder = z1 + r * 0.4;
    let plan = ngon(8, 1.0);
    b.at(c, |b| {
        seam(b);
        b.loft_z(
            &plan,
            &[Section::new(z0, r * 1.15), Section::new(z0 + 0.7, r * 1.1)],
        );
        dark_plate(b);
        b.with_facets(|b| {
            b.loft_z(
                &plan,
                &[
                    Section::new(z0 + 0.7, r),
                    Section::new(z1, r),
                    Section::new(shoulder, r * 0.7),
                ],
            )
        });
        metal(b);
        let sides = b.sides(8);
        b.prism(v3(0.0, 0.0, shoulder), sides, r * 0.45, r * 0.4, 0.8);
        b.paint(TEAM);
        let t = r * 0.3;
        b.face(&[
            v3(t, 0.0, shoulder + 0.82),
            v3(0.0, t, shoulder + 0.82),
            v3(-t, 0.0, shoulder + 0.82),
            v3(0.0, -t, shoulder + 0.82),
        ]);
        if !b.fine() {
            return;
        }
        // Plates lapped up every other face, their tails lifting under the shoulder.
        for k in 0..4 {
            let a = (std::f32::consts::FRAC_PI_4 * (2 * k) as f32) + std::f32::consts::PI / 8.0;
            let d = v3(a.cos(), a.sin(), 0.0);
            let face = r * (std::f32::consts::PI / 8.0).cos();
            let foot = d * (face + 0.05) + Vec3::Z * (z0 + 1.2);
            let f = Frame::new(foot, Vec3::Z, d);
            dark_plate(b);
            Course {
                count: 2,
                step: (z1 - z0) * 0.42,
                len: (z1 - z0) * 0.5,
                half: r * 0.3,
                tip: 0.0,
                thick: THICK,
                tail: 0.4,
            }
            .lay(b, &f);
        }
    });
    shoulder + 0.82
}

/// A graphite field collar round the vessel at height `z` (radius `r`), a red slot on each
/// side where the field runs hot.
pub(super) fn field_collar(b: &mut MeshBuilder, c: Vec3, r: f32, z: f32) {
    // The hoop's flats lie across the vessel's corners: its inside clears them.
    let mid = r * 1.09 + 0.45;
    metal(b);
    hoop(b, c + Vec3::Z * z, mid, 0.8, 0.9, 8);
    if b.fine() {
        let face = (mid + 0.4) * (std::f32::consts::PI / 8.0).cos();
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::PI / 8.0;
            let d = v3(a.cos(), a.sin(), 0.0);
            red_slot(
                b,
                c + d * face + Vec3::Z * z,
                d,
                v3(-d.y, d.x, 0.0),
                1.4,
                0.3,
            );
        }
    }
}

/// The output on the +x side: a plated bin at `at`, `size` across, heaped with product, and
/// the chute from `from` on the vessel down into it.
pub(super) fn bin(b: &mut MeshBuilder, from: Vec3, at: Vec3, size: f32) {
    dark_plate(b);
    if b.coarse() {
        b.cuboid_open(at + Vec3::Z * (size * 0.5), Vec3::splat(size));
        return;
    }
    b.frustum(
        at,
        v2(size * 0.6, size * 0.6),
        v2(size, size),
        size,
        Vec2::ZERO,
    );
    let into = at + Vec3::Z * (size * 1.15);
    seam(b);
    b.beam(from, into, Vec2::splat(0.9), Vec2::splat(0.8));
    if !b.fine() {
        b.paint(GLOW_ORANGE);
        b.decal(at + Vec3::Z * (size + 0.02), Vec2::splat(size * 0.6));
        return;
    }
    b.paint(ROCK);
    b.frustum(
        at + Vec3::Z * (size - 0.05),
        v2(size * 0.9, size * 0.9),
        v2(size * 0.3, size * 0.25),
        size * 0.3,
        Vec2::ZERO,
    );
    b.paint(GLOW_ORANGE);
    b.decal(
        at + Vec3::Z * (size * 1.25 + 0.02),
        v2(size * 0.26, size * 0.2),
    );
    metal(b);
    b.cylinder_between(into, into - Vec3::Z * 0.6, 0.55, 0.55, 6);
}

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// The tech 3 yard round the plant on its 8x8 lot: two more rows of cells on the grid side
/// and a long cell out on each flank, all cabled in to the plant's own cells, and two more
/// bins on the far side fed from the plant's.
fn yard(b: &mut MeshBuilder) {
    if b.coarse() {
        // The rows of cells as plates lying where they are.
        dark_plate(b);
        for (c, half) in [
            (v3(-37.2, 0.0, 3.4), v2(6.5, 21.0)),
            (v3(0.0, 36.0, 3.4), v2(24.0, 2.8)),
            (v3(0.0, -36.0, 3.4), v2(24.0, 2.8)),
        ] {
            b.decal(c, half * 2.0);
        }
        return;
    }
    let k = T3_SCALE;
    for x in [-34.0f32, -40.5] {
        for y in [-12.0f32, 12.0] {
            cell(b, v3(x, y, 3.4), 8.0, 2.8);
            feed(
                b,
                v3(x + 2.0, y, 6.0),
                v3(-19.4, y.signum() * FLANK * k, 4.7),
            );
        }
    }
    for s in [-1.0f32, 1.0] {
        for (x, into) in [(-14.0f32, -8.0 * k), (14.0, 5.0 * k)] {
            let at = v3(x, s * 36.0, 0.0);
            b.yawed(at, s * std::f32::consts::FRAC_PI_2, |b| {
                cell(b, v3(0.0, 0.0, 3.4), 9.0, 2.8)
            });
            feed(b, v3(x, s * 34.6, 6.0), v3(into, s * FLANK * k, 6.6));
        }
    }
    for y in [-10.0f32, 10.0] {
        bin(b, v3(14.5 * k, 0.0, 5.0), v3(34.0, y, 0.0), 4.5);
    }
}

#[cfg(test)]
mod tests {
    use super::{LOTS, SIZES};
    use crate::{build_model_scaled, rig};

    const DESIGNS: [&str; 3] = [
        "regency_fabricator",
        "regency_fabricator~orbit",
        "regency_fabricator~press",
    ];

    /// Each tier is a building of its own: nothing waits on it for a refit, it fills its
    /// own lot (and no more), and each tier carries more machinery than the one below.
    #[test]
    fn every_tier_stands_alone_on_its_own_lot() {
        for key in DESIGNS {
            let mut last = 0;
            for (i, ((r, h), cells)) in SIZES.into_iter().zip(LOTS).enumerate() {
                let tech = i as u8 + 1;
                let model = build_model_scaled(key, r, h, tech).unwrap();
                let half = cells as f32 * 6.0;
                for (lod, mesh) in model.lods.iter().enumerate() {
                    assert!(
                        mesh.vertices.iter().all(|v| v.rig & rig::UPGRADE == 0),
                        "{key} T{tech} lod{lod}: upgrade pieces"
                    );
                    let reach = mesh
                        .vertices
                        .iter()
                        .map(|v| v.pos[0].abs().max(v.pos[1].abs()))
                        .fold(0.0, f32::max);
                    assert!(
                        reach <= half && reach >= half * 0.55,
                        "{key} T{tech} lod{lod}: reach {reach} on a {cells}x{cells} lot"
                    );
                }
                let full = model.lods[0].indices.len();
                assert!(full > last, "{key} T{tech} has more machinery");
                last = full;
            }
        }
    }

    /// Prints each design's triangles per level of detail and tier, and the reduced
    /// level's share of the full one:
    /// `cargo test --profile gate -p mc-models --lib zz_condenser_counts -- --ignored --nocapture`.
    #[test]
    #[ignore = "a probe: prints triangle counts"]
    fn zz_condenser_counts() {
        for key in DESIGNS {
            for (i, (r, h)) in SIZES.into_iter().enumerate() {
                let m = build_model_scaled(key, r, h, i as u8 + 1).unwrap();
                let n: Vec<usize> = m.lods.iter().map(|l| l.indices.len() / 3).collect();
                println!("{key} T{}: {n:?} {:.2}", i + 1, n[1] as f32 / n[0] as f32);
            }
        }
    }
}

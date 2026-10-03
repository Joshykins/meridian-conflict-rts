//! The Material Fabricator (mesh `fabricator`): one machine-building on a 2x2 lot, built
//! at tech 2 and upgraded in place to tech 3. It is drawn in the fusion plants' manner, a
//! four-fold foundation building up to a raised middle, but what it raises is a chamber
//! where matter condenses, lit in the warm orange of hot product instead of the plants'
//! blue charge. Tech 3 is the tech 2 building with more and taller machinery on it: those
//! pieces ride the tech 2 model as upgrade pieces (`structures::kit`), going up during the
//! refit.
//!
//! Design candidates, one per file, all built from the kit here:
//! - `clamp` (`fabricator~a`): a glazed condensing column held by four leaning clamp plates.
//! - `crucible` (`fabricator~b`): a crucible bowl on a neck, a condenser head held over it.
//! - `stack` (`fabricator~c`): a press stack of plates with matter sheets forming between.

pub(super) mod clamp;
pub(super) mod crucible;
pub(super) mod stack;

use std::f32::consts::FRAC_PI_4;

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// The (radius, height) of each tier, the blueprints': tech 1 is never built and drawn
/// as tech 2.
pub(super) const SIZES: [(f32, f32); 3] = [(11.0, 16.0), (11.0, 16.0), (11.0, 22.0)];
/// Every tier's lot, in build cells a side.
#[cfg(test)]
const LOT: u32 = 2;

/// Half the plinth, its corner cut, and the top of its deck.
pub(super) const HALF: f32 = 11.6;
const CHAMFER: f32 = 2.6;
pub(super) const DECK: f32 = 0.9;

/// The tier a model drawn at `tech` has: 2 or 3.
pub(super) fn tier_of(tech: u8) -> u8 {
    tech.clamp(2, 3)
}

/// Tech 3's machinery on a building drawn at `tech`: built at tech 3, an upgrade piece
/// going up `at` of the way through the refit at tech 2.
pub(super) fn tech_3(b: &mut MeshBuilder, tech: u8, at: f32, f: impl FnOnce(&mut MeshBuilder)) {
    super::structures::kit(b, tier_of(tech), 3, at, f);
}

/// The plinth out to the lot's edge, the owner's colour on its two flanks (±y).
pub(super) fn plinth(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, DECK * 0.5), v3(HALF * 2.0, HALF * 2.0, DECK));
        team_panel(b, v3(0.0, HALF - 1.2, DECK), v2(4.0, 0.9));
        return;
    }
    let plan = chamfered_rect(v2(HALF, HALF), CHAMFER);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(0.0, 1.02), Section::new(DECK * 0.4, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(DECK * 0.4, 0.99), Section::new(DECK, 0.96)],
    );
    b.mirror_y(|b| team_panel(b, v3(0.0, HALF - 1.2, DECK), v2(4.0, 0.9)));
}

/// A heat sink on each diagonal, from `from` to `to` out, `w` across: the condensate's
/// heat dumped off the four corners.
pub(super) fn corner_sinks(b: &mut MeshBuilder, from: f32, to: f32, w: f32) {
    if b.coarse() {
        return;
    }
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            radiator(b, v3(from, 0.0, DECK), Vec3::X, to - from, w)
        })
    });
}

/// A radiator along `along` from `at`, `len` long and `w` across: a dark housing, hot
/// coolant glowing between its fins, the fins under a clamp bar.
pub(super) fn radiator(b: &mut MeshBuilder, at: Vec3, along: Vec3, len: f32, w: f32) {
    let base = 0.7;
    let fin = w * 0.6;
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
        let n = if b.fine() { 7 } else { 3 };
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
                v3(len * 0.5, 0.0, base + fin + 0.1),
                v3(len * 0.94, w * 0.14, 0.2),
            );
        }
    });
}

/// A glazed condensing chamber round the z axis from `z0` to `z1`, `r` out: the matter
/// forming in it glows through glazing between dark mullions, dark collars at its foot
/// and head.
pub(super) fn chamber(b: &mut MeshBuilder, z0: f32, z1: f32, r: f32) {
    if b.coarse() {
        b.paint(GLOW_ORANGE).pattern(pattern::NONE);
        b.prism(v3(0.0, 0.0, z0), 4, r * 1.3, r * 1.3, z1 - z0);
        return;
    }
    let sides = 8;
    let plan = ngon(sides, 1.0);
    let collar = (z1 - z0).min(6.0) * 0.12;
    b.paint(GLOW_ORANGE).pattern(pattern::NONE);
    b.loft_z(
        &plan,
        &[Section::new(z0 + collar, r), Section::new(z1 - collar, r)],
    );
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[
            Section::new(z0, r * 1.18),
            Section::new(z0 + collar, r * 1.12),
        ],
    );
    b.loft_z(
        &plan,
        &[
            Section::new(z1 - collar, r * 1.12),
            Section::new(z1, r * 1.18),
        ],
    );
    // Mullions on the glazing's corners, a rib band round its waist close to.
    let (a, c) = (z0 + collar, z1 - collar);
    let fine = b.fine();
    let w = r * 0.24;
    for k in 0..sides {
        let ang = (k as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
        let d = v3(ang.cos(), ang.sin(), 0.0);
        b.paint(ACCENT);
        b.beam(
            d * r + Vec3::Z * a,
            d * r + Vec3::Z * c,
            v2(w, w * 0.8),
            v2(w, w * 0.8),
        );
    }
    if fine && c - a > 3.0 {
        b.paint(ACCENT);
        let mid = (a + c) * 0.5;
        b.loft_z(
            &plan,
            &[
                Section::new(mid - 0.2, r * 1.06),
                Section::new(mid + 0.2, r * 1.06),
            ],
        );
    }
}

/// A capacitor bastion on the deck at `at`, its long side along y: a dark block with a row
/// of cans on it, the grid's power going in.
pub(super) fn bastion(b: &mut MeshBuilder, at: Vec3, len: f32, h: f32) {
    if b.coarse() {
        return;
    }
    b.paint(ACCENT);
    if !b.fine() {
        b.cuboid_open(at + Vec3::Z * (h * 0.5), v3(2.0, len, h));
        return;
    }
    b.chamfered_box(at + Vec3::Z * (h * 0.5), v3(2.0, len, h), 0.4);
    let n = (len / 1.4) as usize;
    for k in 0..n {
        let y = (k as f32 - (n as f32 - 1.0) * 0.5) * 1.4;
        b.paint(PLATING);
        b.prism(at + v3(0.0, y, h), 6, 0.5, 0.45, 0.6);
        b.paint(METAL);
        b.prism(at + v3(0.0, y, h + 0.6), 6, 0.2, 0.15, 0.35);
    }
}

/// A bar of rectangular section (`size`: across in y, deep in the path's plane) swept
/// along `path` in the x-z plane.
pub(super) fn sweep(b: &mut MeshBuilder, path: &[Vec3], size: Vec2) {
    let n = path.len();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|i| {
            let (prev, next) = (path[i.saturating_sub(1)], path[(i + 1).min(n - 1)]);
            let t = (next - prev).normalize_or(Vec3::Z);
            let side = Vec3::Y;
            let up = side.cross(t).normalize_or(Vec3::X);
            let (s, u) = (side * size.x * 0.5, up * size.y * 0.5);
            let p = path[i];
            vec![p - s - u, p + s - u, p + s + u, p - s + u]
        })
        .collect();
    b.loft(&rings, true, true);
}

#[cfg(test)]
mod tests {
    use super::{LOT, SIZES};
    use crate::{build_model_scaled, rig};

    const DESIGNS: [&str; 3] = ["fabricator~a", "fabricator~b", "fabricator~c"];

    /// Tech 2 and 3 stand on one 2x2 lot; tech 2 carries tech 3's machinery as upgrade
    /// pieces (only those reach over its height), and tech 3 has more and stands taller.
    #[test]
    fn tech_3_is_tech_2_upgraded_on_one_lot() {
        let half = LOT as f32 * 6.0;
        for key in DESIGNS {
            let mut tops = Vec::new();
            for tech in [2u8, 3] {
                let (r, h) = SIZES[tech as usize - 1];
                let model = build_model_scaled(key, r, h, tech).unwrap();
                for (lod, mesh) in model.lods.iter().enumerate() {
                    let reach = mesh
                        .vertices
                        .iter()
                        .map(|v| v.pos[0].abs().max(v.pos[1].abs()))
                        .fold(0.0, f32::max);
                    assert!(
                        reach <= half && reach >= half * 0.8,
                        "{key} T{tech} lod{lod}: reach {reach}"
                    );
                }
                let fine = &model.lods[0];
                let upgrades = fine
                    .vertices
                    .iter()
                    .filter(|v| v.rig & rig::UPGRADE != 0)
                    .count();
                assert_eq!(upgrades > 0, tech == 2, "{key} T{tech}: upgrade pieces");
                let top = fine
                    .vertices
                    .iter()
                    .filter(|v| v.rig & rig::UPGRADE == 0)
                    .map(|v| v.pos[2])
                    .fold(0.0, f32::max);
                assert!(top <= h + 0.05, "{key} T{tech}: {top} m tall, not {h}");
                tops.push(top);
            }
            assert!(tops[1] > tops[0] + 4.0, "{key}: tech 3 stands taller");
        }
    }
}

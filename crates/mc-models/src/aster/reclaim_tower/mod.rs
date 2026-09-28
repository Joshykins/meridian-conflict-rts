//! The ARC reclaim tower (`reclaim_tower`, tech 1 to 3 on one upgrade chain, a 4x4 lot):
//! a fixed installation that pulls wreckage in from far off (640 / 1100 / 1700 m) with a
//! slow reclaim head at the top of a real tower, and sends what it takes down the tower
//! to the plant at its foot.
//!
//! Three designs are open for the user to pick from (CLAUDE.md section 9):
//! - `~a` [`derrick`]: an open steel lattice, the drop tube seen through it, a plant house
//!   and bunkers on conveyors round its foot.
//! - `~b` [`spire`]: a solid faceted armoured spire on a buttressed plinth, the stream
//!   spiralling down it in a glazed chute.
//! - `~c` [`legs`]: four raked legs from the lot's corners to a crown, a chute down each
//!   into a bunker at its foot, a storage silo standing between them.
//!
//! All three share the rig and its numbers, so the blueprint holds whichever is picked:
//! - The head is gun house 0 ([`crate::builder::MeshBuilder::with_house`]): it turns about
//!   the tower's axis by weapon 0's yaw, and the projector in it pitches about the
//!   trunnion at [`PIVOT_Z`] (the house pivot), steeply both ways.
//! - The reclaim beam leaves [`EMIT_X`] ahead of the trunnion along the projector's bore.
//! - Tiers grow by height and by working machinery added round the foot and up the tower,
//!   never by spikes or glow.
//! - The chutes wear `pattern::MASS_FLOW`: dark glazing, and a stream of glowing
//!   Materials red-orange falling down them while the tower reclaims.

use glam::Vec3;

use super::parts::*;
use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

mod derrick;
mod head;
mod legs;
mod spire;

pub(super) use derrick::derrick;
pub(super) use legs::legs;
pub(super) use spire::spire;

/// Collision radius at every tier: a 4x4 lot of 12 m cells.
pub(super) const RADIUS: f32 = 22.5;
/// Authored height per tier (T1, T2, T3).
pub(super) const HEIGHT: [f32; 3] = [42.0, 60.0, 84.0];
/// The head's pivot height per tier: the yaw axis is the tower's (x = y = 0), and the
/// projector pitches about a y axis through (0, 0, PIVOT_Z). The blueprint's head pivot.
pub(crate) const PIVOT_Z: [f32; 3] = [36.5, 53.5, 76.0];
/// Where the reclaim beam leaves the projector: this far ahead of the trunnion along the
/// bore (+x at rest, level). The blueprint's emitter is (EMIT_X, 0, PIVOT_Z).
pub(crate) const EMIT_X: [f32; 3] = [11.0, 13.0, 15.5];
/// Full-detail triangle budget: a 4x4 installation up to 84 m tall (the Citadel's 4x4
/// keep has 4200; a factory 6000).
#[cfg(test)]
pub(crate) const TRIANGLES: usize = 6000;
/// How much bigger the head is drawn at each tier.
const HEAD_SCALE: [f32; 3] = [1.4, 1.6, 1.85];

/// Tier index 0..3 for `tech`.
fn tier(tech: u8) -> usize {
    usize::from(tech.clamp(1, 3) - 1)
}

/// Top of the tower's slewing ring, which the head turns on.
fn ring_top(tech: u8) -> f32 {
    PIVOT_Z[tier(tech)] - 5.4 * HEAD_SCALE[tier(tech)]
}

/// A glazed chute carrying reclaimed material from `from` down to `to`: dark
/// `MASS_FLOW` glazing, with a steel collar at each end at full detail.
fn chute(b: &mut MeshBuilder, from: Vec3, to: Vec3, radius: f32) {
    let sides = if b.fine() { 8 } else { 4 };
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.cylinder_between(from, to, radius, radius, sides);
    if b.fine() {
        let dir = (to - from).normalize_or_zero();
        b.paint(METAL);
        for end in [from + dir * 0.3, to - dir * 0.3] {
            b.cylinder_between(
                end - dir * 0.3,
                end + dir * 0.3,
                radius * 1.25,
                radius * 1.25,
                sides,
            );
        }
    }
}

/// The lot: a low dark slab across the 4x4 cells with cut corners, a light kerb.
fn lot_slab(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.loft_z(
        &ngon(8, 25.2)
            .iter()
            .map(|p| rotate8(*p))
            .collect::<Vec<_>>(),
        &[Section::new(0.0, 1.0), Section::new(0.45, 0.985)],
    );
}

/// An octagon point turned by half a side, so the flat sides face the axes and the
/// cut corners the diagonals.
fn rotate8(p: [f32; 2]) -> [f32; 2] {
    let (s, c) = std::f32::consts::FRAC_PI_8.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

/// A material bunker at `at` (ground centre): a squat dark bin with a light lid and a
/// glazed hopper throat on top where a chute comes in, `size` across.
fn bunker(b: &mut MeshBuilder, at: Vec3, size: f32, height: f32) {
    b.paint(PLATING);
    b.at(at, |b| {
        if b.coarse() {
            b.cuboid_open(Vec3::Z * (height * 0.5), Vec3::splat(size).with_z(height));
            return;
        }
        b.loft_z(
            &crate::builder::chamfered_rect(glam::Vec2::splat(size * 0.5), size * 0.14),
            &[
                Section::new(0.0, 1.0),
                Section::new(height * 0.72, 1.0),
                Section::new(height, 0.82),
            ],
        );
        b.paint(ACCENT);
        b.prism(
            v3(0.0, 0.0, height * 0.35),
            8,
            size * 0.52,
            size * 0.52,
            0.5,
        );
        // The throat the stream drops into.
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.prism(
            v3(0.0, 0.0, height - 0.05),
            b.sides(8),
            size * 0.26,
            size * 0.16,
            1.4,
        );
        if b.fine() {
            b.paint(METAL);
            b.prism(v3(0.0, 0.0, height + 1.35), 8, size * 0.2, size * 0.2, 0.3);
            // A hatch and a ladder cage down one side.
            b.paint(ACCENT);
            b.block(
                v3(size * 0.5 - 0.05, -0.9, 0.3),
                v3(size * 0.5 + 0.12, 0.9, 2.6),
            );
            b.paint(METAL);
            b.block(
                v3(-0.4, size * 0.5, 0.2),
                v3(0.4, size * 0.5 + 0.35, height),
            );
        }
    });
}

/// A conveyor gallery from `from` to `to` (both at deck height): a covered trough on
/// two stilts, the stream seen along its glazed top.
fn conveyor(b: &mut MeshBuilder, from: Vec3, to: Vec3) {
    let width = 1.5;
    b.paint(PLATING_DARK);
    b.beam(
        from,
        to,
        glam::Vec2::new(width, 1.1),
        glam::Vec2::new(width, 1.1),
    );
    if b.fine() {
        let up = Vec3::Z * 0.6;
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.beam(
            from + up,
            to + up,
            glam::Vec2::new(width * 0.6, 0.3),
            glam::Vec2::new(width * 0.6, 0.3),
        );
    }
    if b.fine() {
        b.paint(METAL);
        for t in [0.3, 0.7] {
            let p = from.lerp(to, t);
            if p.z > 1.5 {
                b.beam(
                    p.with_z(0.4),
                    p - Vec3::Z * 0.55,
                    glam::Vec2::splat(0.45),
                    glam::Vec2::splat(0.4),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::build_lod;

    /// The shaders read the sim's reclaiming flag out of `owner_flags` (`owner | flags << 8`).
    #[test]
    fn reclaiming_flag_matches_the_sim() {
        assert_eq!(
            crate::gpu_consts::unit_flag::RECLAIMING,
            u32::from(mc_sim::tables::flag::RECLAIMING) << 8
        );
    }

    /// Every variant has one head house at the rig's pivot, a projector reaching the
    /// emitter, and chutes, at every tier.
    #[test]
    fn variants_share_the_rig() {
        let mut over = Vec::new();
        for key in ["reclaim_tower~a", "reclaim_tower~b", "reclaim_tower~c"] {
            for tech in 1..=3u8 {
                let t = tier(tech);
                let model = crate::library::find(key).expect("registered");
                let (radius, height) = model.nominal[t];
                let built = crate::build_model_scaled(key, radius, height, tech).unwrap();
                assert_eq!(built.houses.len(), 1, "{key} t{tech}: one head house");
                let pivot = Vec3::from(built.houses[0].pivot);
                assert!(
                    (pivot - Vec3::new(0.0, 0.0, PIVOT_Z[t])).length() < 1e-3,
                    "{key} t{tech}: pivot {pivot}"
                );
                let fine = build_lod(key, 0, tech);
                let mesh = fine.mesh();
                let tip = mesh
                    .vertices
                    .iter()
                    .filter(|v| v.rig & crate::rig::RECOIL != 0)
                    .map(|v| v.pos[0])
                    .fold(f32::MIN, f32::max);
                assert!(
                    (tip - EMIT_X[t]).abs() < 0.6,
                    "{key} t{tech}: projector ends at x {tip}, emitter at {}",
                    EMIT_X[t]
                );
                let flow = mesh
                    .vertices
                    .iter()
                    .filter(|v| (v.surface & 0xFF) == pattern::MASS_FLOW)
                    .count();
                assert!(flow > 0, "{key} t{tech}: no chute");
                for (lod, m) in built.lods.iter().enumerate() {
                    let low = m.vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
                    assert!(
                        low >= -1e-3,
                        "{key} t{tech} lod{lod}: below ground at z {low}"
                    );
                }
                let tris = |lod: usize| built.lods[lod].indices.len() / 3;
                let (full, mid, coarse) = (tris(0), tris(1), tris(2));
                println!("{key} t{tech}: {full}/{mid}/{coarse} triangles");
                if !(full <= TRIANGLES && coarse < 60 && mid as f32 <= full as f32 * 0.45 + 20.0) {
                    over.push(format!("{key} t{tech}: {full}/{mid}/{coarse}"));
                }
            }
        }
        assert!(over.is_empty(), "over the triangle budgets: {over:?}");
    }
}

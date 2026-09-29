//! The ARC reclaim tower (`reclaim_tower`, tech 1 to 3 on one upgrade chain, a 3x3 lot):
//! a fixed installation that pulls wreckage in from far off (640 / 1100 / 1700 m) with a
//! slow reclaim head at the top of an armoured tower ([`body`]), and sends what it takes
//! down the tower to the processing works at its foot ([`works`]).
//!
//! The head is the Thresher's Cradle at tower scale with a long, slim barrel ([`turret`]).
//! Its rig:
//! - The head is gun house 0 ([`crate::builder::MeshBuilder::with_house`]): it turns about
//!   the tower's axis by weapon 0's yaw, and the processor in it pitches about the
//!   trunnion at [`PIVOT_Z`] (the house pivot).
//! - The reclaim beam leaves [`EMIT_X`] ahead of the trunnion, at the barrel's tip.
//! - The tower is as tall at every tier. Like the core mine, each tier builds onto the
//!   last (see [`works`]); the barrel gains a cooling jacket at tech 2 and induction
//!   rings and rails at tech 3. Never spikes or glow for menace.
//! - It is symmetric front to back: a flow channel down every face, and only the
//!   control tower stands alone, on its +y side, which is the player's right as it is
//!   placed (structures face south, `UnitBlueprint::build_heading`, and the camera
//!   starts looking north).
//! - It stands on a two-step foundation kept clean: the model's dust line holds the
//!   footing's dirt to the foundation's foot (`MeshBuilder::set_dust_line`).
//! - Glazed channels wear `pattern::MASS_FLOW`: dark glazing, and a stream of glowing
//!   Materials red-orange falling down them while the tower reclaims.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

mod body;
mod turret;
mod works;

/// Collision radius at every tier: a 3x3 lot of 12 m cells.
pub(super) const RADIUS: f32 = 16.9;
/// Authored height, the same at every tier: upgrades build round the tower, not up it.
pub(super) const HEIGHT: f32 = 38.0;
/// The head's pivot height: the yaw axis is the tower's (x = y = 0), and the processor
/// pitches about a y axis through (0, 0, PIVOT_Z). The blueprint's head pivot.
pub(crate) const PIVOT_Z: f32 = 34.2;
/// The head's scale: metres per unit of the Thresher's Cradle, which is authored with
/// its processor box 1.5 units long.
const K: f32 = 4.8;
/// Trunnion to the barrel's mouth, in units of [`K`].
const MUZZLE: f32 = 2.3;
/// Where the reclaim beam leaves the head: this far ahead of the trunnion along the
/// bore (+x at rest, level). The blueprint's emitter is (EMIT_X, 0, PIVOT_Z).
pub(crate) const EMIT_X: f32 = MUZZLE * K;
/// Top of the head house, which the head's turntable turns on.
const RING_TOP: f32 = PIVOT_Z - 0.69 * K;
/// How tall the plated head house under the slewing ring is.
const HOUSE: f32 = 3.4;
/// Top of the tower's shaft, where the head house sits.
const CAP: f32 = RING_TOP - HOUSE;
/// Top of the foundation, where the tower's foot stands.
const BASE: f32 = 2.8;
/// Top of the armoured foot the shaft rises from.
const FOOT: f32 = 7.5;
/// Full-detail triangle budget: a 3x3 installation 38 m tall with its works round it
/// (a factory has 6000, the core mine 9000).
#[cfg(test)]
pub(crate) const TRIANGLES: usize = 7500;

pub(super) fn tower(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    b.set_dust_line(0.5);
    if b.coarse() {
        body::coarse(b);
        works::coarse(b);
        turret::turret(b, tech);
        return;
    }
    body::body(b);
    works::foundation(b);
    turret::turret(b, tech);
    works::tiers(b, tech);
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

    /// The tower has one head house at the rig's pivot, a barrel reaching the emitter,
    /// and chutes, at every tier, and stays inside its lot and budget.
    #[test]
    fn head_fits_the_rig() {
        let mut over = Vec::new();
        let keys = crate::library::all_model_keys();
        for key in keys.into_iter().filter(|k| k.starts_with("reclaim_tower")) {
            for tech in 1..=3u8 {
                let t = usize::from(tech - 1);
                let model = crate::library::find(key).expect("registered");
                let (radius, height) = model.nominal[t];
                let built = crate::build_model_scaled(key, radius, height, tech).unwrap();
                assert_eq!(built.houses.len(), 1, "{key} t{tech}: one head house");
                let pivot = Vec3::from(built.houses[0].pivot);
                assert!(
                    (pivot - Vec3::new(0.0, 0.0, PIVOT_Z)).length() < 1e-3,
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
                    (tip - EMIT_X).abs() < 1.0,
                    "{key} t{tech}: barrel ends at x {tip}, emitter at {EMIT_X}"
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

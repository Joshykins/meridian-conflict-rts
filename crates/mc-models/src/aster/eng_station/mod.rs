//! The ARC engineering station, the Foreman (`eng_station`, tech 1 to 3 on one upgrade
//! chain, a 3x3 lot): a static engineer that builds and assists far across the base
//! (300 / 500 / 800 m) with one build arm on a slewing head.
//!
//! A braced armoured foot on a two-step foundation, a slim octagonal mast fed by amber
//! conduits ([`body`]), and on its head a halo: a ring of projector nodes on three
//! spokes that turns with the head, the forearm reaching out over it ([`head`]). Round
//! it the engineering works a builder post keeps: parts magazines, canister racks,
//! spare projector heads, servos and tool arms on the halo ([`works`], [`head`]). They grow
//! with the tiers, each tier building onto the last (`structures::kit`); the station is
//! as tall at every tier. The faces carry their zones and fittings as surface marks
//! ([`marks`]).
//!
//! Its rig:
//! - What turns toward the work is the turret part, yawing about the station's axis
//!   (`set_turret_pivot` on x = y = 0) by weapon 0's yaw. The halo turns with it.
//! - The forearm and its projector are `rig::ARM_TOOL`: one bone, pitching about the
//!   elbow (`set_arm_pivot`). No boom, no shoulder.
//! - The build beam leaves the projector's tip, level and facing +x at rest. The
//!   blueprints' `builder.arm` carries the elbow and tip ([`ELBOW`], [`EMITTER`]), the
//!   same at every tier.
//!
//! What builds is engineering amber: the projector's lens, and the lines of light in
//! the conduits (`pattern::CONDUIT`, running while it builds). No lit mesh but the lens
//! and lamps.
//!

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

mod body;
mod head;
mod marks;
mod works;

/// Collision radius at every tier: a 3x3 lot of 12 m cells.
pub(super) const RADIUS: f32 = 16.9;
/// Authored height, the same at every tier.
pub(super) const HEIGHT: f32 = 30.0;
/// The forearm's elbow: the blueprints' `builder.arm.pivot`.
const ELBOW: Vec3 = Vec3::new(2.0, 0.0, 25.8);
/// The projector's tip, level ahead of the elbow: the blueprints' `builder.arm.emitter`.
const EMITTER: Vec3 = Vec3::new(13.6, 0.0, 25.8);
/// Full-detail triangle budget, the reclaim tower's.
#[cfg(test)]
pub(crate) const TRIANGLES: usize = 7500;

/// Half the foundation's lower step, and its top.
const APRON: f32 = 15.2;
const STEP: f32 = 1.2;
/// Top of the plinth the station stands on, and half its width.
const BASE: f32 = 2.6;
const PLINTH: f32 = 10.2;
/// Half the foot, the top of its upright faces, and its top.
const FOOT: f32 = 6.6;
const FOOT_WALL: f32 = 4.6;
const FOOT_TOP: f32 = 7.4;
/// The mast's circumradius at its foot and at its head.
const MAST: f32 = 3.0;
const MAST_HEAD: f32 = 2.3;
/// The deck the head turns on, and the head's roof.
const DECK: f32 = 19.8;
const ROOF: f32 = 24.4;
/// The halo's radius and height, and how many nodes it carries at tech 1.
const HALO: f32 = 7.4;
const HALO_Z: f32 = 21.8;
const NODES: usize = 6;
/// Where the forearm ends and the projector begins.
const ARM_END: f32 = 9.6;

pub(super) fn station(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(ELBOW);
    foundation(b, PLINTH);
    if b.coarse() {
        body::coarse(b);
        return;
    }
    body::body(b, tech);
    head::head(b, tech);
    works::works(b, tech);
    marks::marks(b);
}

/// The foundation: a dark lower step with a lit kerb, a plated octagonal plinth of
/// half-width `plinth`, and steps up its front. Coarse: one block.
fn foundation(b: &mut MeshBuilder, plinth: f32) {
    b.set_dust_line(0.5);
    if b.coarse() {
        b.paint(PLATING_DARK);
        b.frustum_open(
            Vec3::ZERO,
            Vec2::splat(APRON * 2.0),
            Vec2::splat(plinth * 2.0),
            BASE,
            Vec2::ZERO,
        );
        return;
    }
    b.paint(PLATING_DARK);
    b.with_bevel(0.15, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(APRON), 3.0),
            &[
                Section::new(0.0, 1.0),
                Section::new(STEP - 0.25, 1.0),
                Section::new(STEP, 0.985),
            ],
        );
    });
    if b.mid() {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.loft_z(
            &chamfered_rect(Vec2::splat(APRON + 0.05), 3.02),
            &[Section::new(0.45, 1.0), Section::new(0.8, 1.0)],
        );
    }
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(plinth), plinth * 0.38),
            &[
                Section::new(STEP - 0.1, 1.0),
                Section::new(BASE - 0.3, 1.0),
                Section::new(BASE, 0.98),
            ],
        );
    });
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(Vec2::splat(plinth + 0.1), plinth * 0.38 + 0.04),
        &[Section::new(STEP, 1.0), Section::new(STEP + 0.4, 1.0)],
    );
    // Steps up the front.
    b.paint(PLATING_DARK);
    b.block(
        v3(plinth - 0.5, -2.6, STEP),
        v3(plinth + 1.1, 2.6, STEP + 0.7),
    );
    b.block(
        v3(plinth - 0.5, -2.6, STEP + 0.7),
        v3(plinth + 0.5, 2.6, BASE),
    );
    if b.fine() {
        b.paint(GLOW_LAMP);
        b.radial(4, |b| {
            let d = APRON - 2.0;
            b.cuboid(v3(d, d, STEP + 0.2), v3(0.5, 0.5, 0.4));
        });
    }
}

/// The projector at the end of a forearm, built along +x from `from` to its tip at
/// `tip` (same y and z): a dark focusing collar `r` in radius, a steel nose, the amber
/// lens let into its face, and three prongs round it. Tech 2 adds a lit band on the
/// collar; tech 3 a second, longer collar. The tip is the build beam's origin.
fn projector(b: &mut MeshBuilder, from: Vec3, tip: Vec3, r: f32, tech: u8) {
    let len = tip.x - from.x;
    let x = |u: f32| from + Vec3::X * (len * u);
    let sides = b.sides(10);
    // The wrist collar, and the head: a squat armoured block, its face sloped in to
    // the lens.
    b.paint(PLATING_DARK);
    b.cylinder_between(x(0.0), x(0.24), r * 0.8, r * 0.86, sides);
    b.paint(PLATING);
    b.with_bevel(0.06, |b| {
        b.extrude_x(
            &[
                [-r * 1.15, -r * 0.7],
                [-r * 0.7, -r * 1.05],
                [r * 0.7, -r * 1.05],
                [r * 1.15, -r * 0.7],
                [r * 1.15, r * 0.7],
                [r * 0.7, r * 1.05],
                [-r * 0.7, r * 1.05],
                [-r * 1.15, r * 0.7],
            ]
            .map(|[y, z]| [y, z + from.z]),
            from.x + len * 0.2,
            from.x + len * 0.46,
        );
    });
    b.paint(PLATING_DARK);
    b.cylinder_between(x(0.46), x(0.54), r * 0.95, r * 0.7, sides);
    b.paint(GLOW_AMBER);
    b.cylinder_between(x(0.54), x(0.57), r * 0.5, r * 0.46, b.sides(8));
    // Three prongs from the head's face closing on the tip, where the beam forms.
    b.paint(METAL);
    for k in 0..3 {
        let a = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / 3.0;
        let off = Vec3::new(0.0, a.cos(), a.sin());
        b.beam(
            x(0.4) + off * r * 0.92,
            x(0.8) + off * r * 0.62,
            Vec2::splat(r * 0.26),
            Vec2::splat(r * 0.2),
        );
        b.beam(
            x(0.8) + off * r * 0.62,
            tip + off * r * 0.3,
            Vec2::splat(r * 0.2),
            Vec2::splat(r * 0.12),
        );
    }
    super::structures::kit(b, tech, 2, 0.8, |b| {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.cylinder_between(x(0.06), x(0.18), r * 0.88, r * 0.9, sides);
    });
    super::structures::kit(b, tech, 3, 0.85, |b| {
        b.paint(PLATING);
        b.mirror_y(|b| {
            b.block(
                x(0.24) + Vec3::new(0.0, r * 1.15, -r * 0.6),
                x(0.44) + Vec3::new(0.0, r * 1.35, r * 0.6),
            );
        });
    });
}

/// A dark inset panel on the face at `x` (facing +x), `w` wide and from `z0` to `z1`,
/// a lit conduit slit across it at `slit` (full detail only when `None`).
fn inset(b: &mut MeshBuilder, x: f32, w: f32, z0: f32, z1: f32, slit: Option<f32>) {
    b.paint(PLATING_DARK);
    b.block(v3(x - 0.3, -w * 0.5, z0), v3(x + 0.06, w * 0.5, z1));
    if let Some(z) = slit {
        if b.mid() {
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(v3(x, -w * 0.4, z - 0.25), v3(x + 0.1, w * 0.4, z + 0.25));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{part, rig};

    /// The forearm pitches about its elbow and ends at its tip, on the turret, at every
    /// tier; and the station stays inside its lot and budget.
    #[test]
    fn arm_fits_the_rig() {
        let mut over = Vec::new();
        {
            let key = "eng_station";
            for tech in 1..=3u8 {
                let built = crate::build_model_scaled(key, RADIUS, HEIGHT, tech).unwrap();
                let pivot = Vec3::from(built.arm_pivot.expect("an elbow"));
                assert!(
                    (pivot - ELBOW).length() < 1e-3,
                    "{key} t{tech}: elbow {pivot}"
                );
                assert!(!built.arm_boom, "{key}: one bone");
                assert!(
                    Vec3::from(built.turret_pivot).truncate().length() < 1e-4,
                    "{key}: turret axis off the origin"
                );
                for (lod, m) in built.lods.iter().enumerate() {
                    let tip = m
                        .vertices
                        .iter()
                        .filter(|v| {
                            v.part == part::TURRET
                                && (v.rig & rig::LIMB_MASK) == rig::ARM_TOOL
                                && v.rig & rig::UPGRADE == 0
                        })
                        .map(|v| v.pos[0])
                        .fold(f32::MIN, f32::max);
                    assert!(
                        (tip - EMITTER.x).abs() < 0.6,
                        "{key} t{tech} lod{lod}: arm ends at x {tip}, emitter at {}",
                        EMITTER.x
                    );
                    let low = m.vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
                    assert!(low >= -1e-3, "{key} t{tech} lod{lod}: below ground {low}");
                    let reach = m
                        .vertices
                        .iter()
                        .map(|v| v.pos[0].abs().max(v.pos[1].abs()))
                        .fold(0.0, f32::max);
                    assert!(
                        reach <= 18.0,
                        "{key} t{tech} lod{lod}: out of its lot {reach}"
                    );
                }
                let amber = built.lods[0]
                    .vertices
                    .iter()
                    .any(|v| v.material == GLOW_AMBER && (v.rig & rig::LIMB_MASK) == rig::ARM_TOOL);
                assert!(amber, "{key} t{tech}: no amber lens on the projector");
                let marks = built.marks.len();
                assert!(
                    marks as u32 <= crate::gpu_consts::surface_mark::MAX && marks >= 20,
                    "{key} t{tech}: {marks} marks"
                );
                let tris = |lod: usize| built.lods[lod].indices.len() / 3;
                let (full, mid, coarse) = (tris(0), tris(1), tris(2));
                println!("{key} t{tech}: {full}/{mid}/{coarse} triangles, {marks} marks");
                if !(full <= TRIANGLES && coarse < 60 && mid as f32 <= full as f32 * 0.47 + 20.0) {
                    over.push(format!("{key} t{tech}: {full}/{mid}/{coarse}"));
                }
            }
        }
        assert!(over.is_empty(), "over the triangle budgets: {over:?}");
    }

    /// The blueprints' build arm is the model's, at every tier.
    #[test]
    fn blueprints_carry_the_arm() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let bps = mc_data::Blueprints::load(&data).unwrap();
        for tech in 1..=3 {
            let key = format!("aster_t{tech}_eng_station");
            let unit = bps.units.iter().find(|u| u.key == key).expect("blueprint");
            let arm = unit.builder.as_ref().and_then(|b| b.arm).expect("an arm");
            let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
            assert!(
                (v(arm.emitter) - EMITTER).length() < 0.01,
                "{key}: emitter {}",
                v(arm.emitter)
            );
            let pivot = v(arm.pivot.expect("an elbow"));
            assert!((pivot - ELBOW).length() < 0.01, "{key}: pivot {pivot}");
            assert!(arm.shoulder.is_none(), "{key}: one bone");
        }
    }
}

//! The Condenser (mesh `regency_fabricator`), the Regency's material fabricator: one
//! machine on a 2x2 lot, built at tech 2 and upgraded in place to tech 3, where gravity
//! squeezes the grid's charge until matter condenses out of it. Like the Power Generator
//! it is a single machine round its middle, but it holds no star: what it holds is the
//! matter forming, lit in the warm orange of hot product behind the dark lapped plate,
//! red slots where its fields run hot. Tech 3 is the tech 2 machine with more and taller
//! plate and field gear on it, riding the tech 2 model as upgrade pieces (`machine::tier`).
//!
//! The machine is in `spire`: an armoured vessel on four buttresses, field collars turning
//! a step round it at each beat of its work and a plated point pressing down on its hatch
//! (`gpu_consts::fab`), the matter glowing through slots down its faces, standby lamps
//! under the point.

pub(super) mod spire;

use glam::{Vec2, Vec3};

use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::*;

/// The (radius, height) of each tier, the blueprints': tech 1 is never built and drawn
/// as tech 2.
pub(super) const SIZES: [(f32, f32); 3] = [(11.0, 16.0), (11.0, 16.0), (11.0, 22.0)];
/// Every tier's lot, in build cells a side.
#[cfg(test)]
const LOT: u32 = 2;

/// A plate's thickness.
pub(super) const THICK: f32 = 0.5;

/// Tech 3's pieces on a machine drawn at `tech`: built at tech 3, an upgrade piece going
/// up `at` of the way through the refit at tech 2.
pub(super) fn tech_3(b: &mut MeshBuilder, tech: u8, at: f32, f: impl FnOnce(&mut MeshBuilder)) {
    tier(b, tech.clamp(2, 3), 3, at, f);
}

/// A low octagonal plated footing `r` out (to its corners) and `h` tall, a seam round
/// its foot.
pub(super) fn footing(b: &mut MeshBuilder, r: f32, h: f32) {
    let sides = if b.coarse() { 4 } else { 8 };
    team_tab(b, v3(r * 0.78, 0.0, h), r * 0.08);
    if b.coarse() {
        dark_plate(b);
        b.prism(Vec3::ZERO, sides, r * 1.3, r * 1.2, h);
        return;
    }
    seam(b);
    b.prism(Vec3::ZERO, sides, r, r * 0.99, 0.4);
    dark_plate(b);
    b.prism(v3(0.0, 0.0, 0.4), sides, r * 0.98, r * 0.9, h - 0.4);
}

/// The owner's colour: a diamond `half` across lying on a flat top at `at`.
pub(super) fn team_tab(b: &mut MeshBuilder, at: Vec3, half: f32) {
    b.paint(TEAM);
    let at = at + Vec3::Z * 0.02;
    b.face(&[
        at + v3(half, 0.0, 0.0),
        at + v3(0.0, half, 0.0),
        at + v3(-half, 0.0, 0.0),
        at + v3(0.0, -half, 0.0),
    ]);
}

/// An armoured octagonal vessel round the z axis: a seam footing at `z0`, plates up to
/// `z1` at radius `r`, drawn in to `r * 0.7` at a shoulder, a graphite hatch; a course of
/// plates lapped up every other face, and on the faces between, a slot onto the matter
/// forming inside. Returns the hatch's top.
pub(super) fn vessel(b: &mut MeshBuilder, r: f32, z0: f32, z1: f32) -> f32 {
    let shoulder = z1 + r * 0.4;
    let plan = ngon(8, 1.0);
    let top = shoulder + 0.82;
    if b.coarse() {
        dark_plate(b);
        b.prism(v3(0.0, 0.0, z0), 4, r * 1.25, r * 0.8, top - z0);
        return top;
    }
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
    b.prism(v3(0.0, 0.0, shoulder), b.sides(8), r * 0.45, r * 0.4, 0.82);
    let face = r * (std::f32::consts::PI / 8.0).cos();
    for k in 0..8 {
        let a = std::f32::consts::FRAC_PI_4 * k as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        if k % 2 == 0 {
            // The matter forming inside, seen through a slot down the face.
            let (lo, hi) = (z0 + 1.6, z1 - 0.6);
            glow_slot(
                b,
                pattern::FAB_MATTER,
                d * face + Vec3::Z * ((lo + hi) * 0.5),
                [d, Vec3::Z],
                Vec2::new(hi - lo, r * 0.24),
            );
        } else if b.fine() {
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
    }
    top
}

/// A slot in the Materials colour standing just proud of a face: onto the matter forming
/// inside (`pattern::FAB_MATTER`), or a status lamp (`pattern::FAB_LAMP`). `[out, along]`
/// are the face's outward normal and the slot's length; `size` its length and width.
pub(super) fn glow_slot(
    b: &mut MeshBuilder,
    look: u32,
    at: Vec3,
    [out, along]: [Vec3; 2],
    size: Vec2,
) {
    let out = out.normalize();
    let along = along.normalize();
    let across = out.cross(along) * (size.y * 0.5);
    let half = along * (size.x * 0.5);
    let quad = |lift: f32| -> Vec<Vec3> {
        [-half - across, half - across, half + across, -half + across]
            .iter()
            .map(|&p| at + p + out * lift)
            .collect()
    };
    b.paint(GLOW_MATERIALS).pattern(look);
    b.loft(&[quad(0.0), quad(0.1)], false, true);
}

/// A graphite field collar round a vessel of radius `r` at height `z`, toothed round its
/// rim, a slot on four of its flats where the field runs hot with the matter: the indexer, turned a
/// quarter round at each beat of the work (`part::FAB_INDEX`), so all of it repeats every
/// quarter turn.
pub(super) fn field_collar(b: &mut MeshBuilder, r: f32, z: f32) {
    if b.coarse() {
        return;
    }
    // The hoop's flats lie across the vessel's corners: its inside clears them.
    let mid = r * 1.09 + 0.45;
    b.with_part(part::FAB_INDEX, |b| {
        metal(b);
        hoop(b, Vec3::Z * z, mid, 0.8, 0.9, 8);
        if !b.fine() {
            return;
        }
        seam(b);
        b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_8, |b| {
            teeth(b, Vec3::Z * z, mid + 0.4, 8, v3(0.45, 0.6, 0.7))
        });
        let face = (mid + 0.4) * (std::f32::consts::PI / 8.0).cos();
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::PI / 8.0;
            let d = v3(a.cos(), a.sin(), 0.0);
            glow_slot(
                b,
                pattern::FAB_MATTER,
                d * face + Vec3::Z * z,
                [d, v3(-d.y, d.x, 0.0)],
                Vec2::new(1.4, 0.3),
            );
        }
    });
}

/// A plated buttress on +x (turned into place by the caller): a seam footing at `foot`
/// out, a strut up to `(out, up)` on what it holds, plates lapped down its back.
pub(super) fn buttress(b: &mut MeshBuilder, foot: f32, top: Vec2, w: f32) {
    if b.coarse() {
        return;
    }
    seam(b);
    b.prism(v3(foot, 0.0, 0.0), 6, w * 2.2, w * 1.9, 1.0);
    strut(b, v3(foot, 0.0, 0.9), v3(top.x, 0.0, top.y), w);
    if b.fine() {
        let at = v3(top.x + w * 1.1, 0.0, top.y - 0.2);
        let down = v3(foot, 0.0, 1.0) - at;
        let f = Frame::new(at, down, v3(0.8, 0.0, 0.6));
        dark_plate(b);
        Course {
            count: 2,
            step: down.length() * 0.38,
            len: down.length() * 0.42,
            half: w * 1.25,
            tip: 0.0,
            thick: THICK,
            tail: 0.6,
        }
        .lay(b, &f);
    }
}

#[cfg(test)]
mod tests {
    use super::{LOT, SIZES};
    use crate::{build_model_scaled, rig};

    const DESIGNS: [&str; 1] = ["regency_fabricator"];

    /// Tech 2 and 3 stand on one 2x2 lot; tech 2 carries tech 3's machinery as upgrade
    /// pieces (only those reach over its height), and tech 3 stands taller.
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

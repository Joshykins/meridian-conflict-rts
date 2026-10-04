//! The Regency's gun emplacements (docs/STYLE.md "The Regency look"), each its own
//! machine on a base of its own:
//!
//! - `regency_barb`: the Picket, a Plasmeric Repeater (`picket`).
//! - `regency_spitter`: the Canopy, a Plasmeric Repeater at the sky (`canopy`).
//! - `regency_airburst_repeater`: the Gorget, a Twin Pinched-plasmeric Airburst Repeater
//!   (`gorget`).
//! - `regency_seeker_silo`: the Belfry, a Gravitic Seeker Silo (`belfry`).
//! - `regency_pinch_cannon`: the Halberd, a Pinched-plasmeric Cannon (`halberd`).
//! - `regency_fusion_cannon`: the Sunspear, a Pinch-fusion Cannon (`sunspear`).
//! - `regency_missile_defense`: the Rondel, gravity lenses that crush missiles (`rondel`).
//! - `regency_springald`: the Springald map gun, a Triune Pinch-fusion Howitzer
//!   (`springald`).
//!
//! The pinch guns gather their charge in front of the bore, between projectors that reach
//! past its mouth, and their `muzzle` is the middle of that charge.
//!
//! Guns are drawn in their own frame ([`gun_frame`]): the origin at the trunnion, +x down
//! the bore, +z up off it.

mod belfry;
mod canopy;
mod gorget;
mod halberd;
mod picket;
/// Its three designs are catalogue keys of their own.
pub(super) mod rondel;
mod springald;
/// The Sunspear's gun pieces are shared by the tech 3 mobile fusion guns
/// (`fusion_guns`), the same gun drawn smaller.
pub(super) mod sunspear;

pub(super) use belfry::belfry;
pub(super) use canopy::canopy;
pub(super) use gorget::gorget;
pub(super) use halberd::halberd;
pub(super) use picket::picket;
pub(super) use springald::{springald, HEIGHT as SPRINGALD_HEIGHT, RADIUS as SPRINGALD_RADIUS};
pub(super) use sunspear::sunspear;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{dark_plate, metal, v3};
use super::machine::{armour, Frame};

/// A gun's line: its trunnion and the point its shot leaves from (or, for a pinch gun,
/// where its charge is held).
#[derive(Clone, Copy, Debug)]
struct Line {
    pivot: Vec3,
    muzzle: Vec3,
}

impl Line {
    const fn new(pivot: Vec3, muzzle: Vec3) -> Self {
        Self { pivot, muzzle }
    }

    fn len(&self) -> f32 {
        self.muzzle.distance(self.pivot)
    }

    /// How far the bore is raised, in radians.
    fn pitch(&self) -> f32 {
        let d = self.muzzle - self.pivot;
        d.z.atan2(d.x)
    }

    /// Records the turret's yaw axis, the gun's trunnion and its recoil.
    fn rig(&self, b: &mut MeshBuilder, travel: f32) {
        b.set_turret_pivot(self.pivot);
        b.set_arm_pivot(self.pivot);
        b.set_recoil(self.pivot, self.muzzle, travel);
    }
}

/// Draws `f` in the gun's frame: the origin at the trunnion, +x down the bore.
fn gun_frame(b: &mut MeshBuilder, line: &Line, f: impl FnOnce(&mut MeshBuilder)) {
    b.pitched(line.pivot, line.pitch(), f);
}

/// A ring of `shape` (points across `side` and `up`, scaled by `w` and `h`) about `c`.
fn section(c: Vec3, side: Vec3, up: Vec3, w: f32, h: f32, shape: &[[f32; 2]]) -> Vec<Vec3> {
    shape
        .iter()
        .map(|[s, u]| c + side * (s * w * 0.5) + up * (u * h * 0.5))
        .collect()
}

/// A faceted section with a raised keel: flat belly, chamfered corners.
const KEELED: [[f32; 2]; 7] = [
    [1.0, -0.5],
    [0.7, -1.0],
    [-0.7, -1.0],
    [-1.0, -0.5],
    [-0.8, 0.55],
    [0.0, 1.0],
    [0.8, 0.55],
];
/// A box with its corners cut.
const CHAMFERED: [[f32; 2]; 8] = [
    [1.0, -0.6],
    [0.6, -1.0],
    [-0.6, -1.0],
    [-1.0, -0.6],
    [-1.0, 0.6],
    [-0.6, 1.0],
    [0.6, 1.0],
    [1.0, 0.6],
];
/// A plain box section, for coarse levels.
const SQUARE: [[f32; 2]; 4] = [[1.0, -1.0], [-1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];

/// A body lofted along x through `stations` (x, width, height, lift of its middle above
/// the axis), every ring of `shape`.
fn hull_x(b: &mut MeshBuilder, stations: &[[f32; 4]], shape: &[[f32; 2]]) {
    let rings: Vec<Vec<Vec3>> = stations
        .iter()
        .map(|&[x, w, h, lift]| section(v3(x, 0.0, lift), Vec3::Y, Vec3::Z, w, h, shape))
        .collect();
    b.loft(&rings, true, true);
}

/// A bar through `points` (each with its width across `across` and its depth), its
/// section square: tines, claws and struts.
fn bar_through(b: &mut MeshBuilder, points: &[(Vec3, Vec2)], across: Vec3) {
    let rings: Vec<Vec<Vec3>> = points
        .iter()
        .enumerate()
        .map(|(i, &(c, size))| {
            let d = if i + 1 < points.len() {
                points[i + 1].0 - c
            } else {
                c - points[i - 1].0
            }
            .normalize();
            let side = (across - d * across.dot(d)).normalize();
            let up = d.cross(side);
            section(c, side, up, size.x, size.y, &SQUARE)
        })
        .collect();
    b.loft(&rings, true, true);
}

/// A plated fin's size: `len` out of its surface, `w0` wide at the root narrowing to
/// `w1`, `thick` through.
#[derive(Clone, Copy, Debug)]
struct Fin {
    len: f32,
    w0: f32,
    w1: f32,
    thick: f32,
}

impl Fin {
    /// The fin standing out of a surface at `root` toward `out`, its face toward `face`:
    /// radiator fins and vent flaps.
    fn at(&self, b: &mut MeshBuilder, root: Vec3, out: Vec3, face: Vec3) {
        let f = Frame::new(root, out, face);
        armour(
            b,
            &f,
            &[
                [0.0, -self.w0 * 0.5],
                [0.0, self.w0 * 0.5],
                [self.len, self.w1 * 0.5],
                [self.len, -self.w1 * 0.5],
            ],
            self.thick,
        );
    }
}

/// A projector's emitter: a bronze boss at `at` with a red lens turned to `toward`.
fn emitter(b: &mut MeshBuilder, at: Vec3, toward: Vec3, r: f32) {
    let d = (toward - at).normalize();
    let sides = b.sides(8);
    metal(b);
    b.cylinder_between(at - d * r * 0.6, at, r, r * 0.85, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(at, at + d * r * 0.25, r * 0.7, r * 0.5, sides);
}

/// The owner's colour as a flat patch on an upward face at height `z`.
fn team_patch(b: &mut MeshBuilder, x0: f32, x1: f32, half: f32, z: f32) {
    b.paint(TEAM);
    b.face(&[
        v3(x0, -half, z),
        v3(x1, -half, z),
        v3(x1, half, z),
        v3(x0, half, z),
    ]);
}

/// A plinth or step: an `n`-gon from `r0` at `z` to `r1` at `z + h`, a flat to +x.
fn step(b: &mut MeshBuilder, n: usize, r0: f32, r1: f32, z: f32, h: f32) {
    dark_plate(b);
    b.prism(Vec3::Z * z, n, r0, r1, h);
}

/// A coarse emplacement: its base, its turret's block and its gun in one bar.
#[derive(Clone, Copy, Debug)]
struct Coarse {
    /// The base: a square of this circumradius at the foot, this high.
    base_r: f32,
    base_h: f32,
    /// The turret's block: from `x0` to `x1`, `half` wide, up to `top`.
    x0: f32,
    x1: f32,
    half: f32,
    top: f32,
    /// The gun's bar: its size at the breech and at the muzzle (across, up).
    gun: Vec2,
    tip: Vec2,
    /// For a pinch gun: where its bore ends, and how far off the bore its two
    /// projectors stand either side of the charge.
    mouth: Option<(f32, f32)>,
}

/// Far off: [`Coarse`] drawn for `line`, the owner's colour on the turret's roof.
fn coarse(b: &mut MeshBuilder, line: &Line, c: &Coarse) {
    use crate::{part, rig};
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, c.base_r, c.base_r * 0.8, c.base_h);
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.block(v3(c.x0, -c.half, c.base_h), v3(c.x1, c.half, c.top));
        team_patch(b, c.x0 * 0.8, c.x0 * 0.3, c.half * 0.5, c.top + 0.01);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, line, |b| {
                let len = line.len();
                dark_plate(b);
                match c.mouth {
                    None => b.beam(v3(len * 0.2, 0.0, 0.0), v3(len, 0.0, 0.0), c.gun, c.tip),
                    Some((mouth, off)) => {
                        b.beam(
                            v3(len * 0.2, 0.0, 0.0),
                            v3(mouth, 0.0, 0.0),
                            c.gun,
                            c.gun * 0.7,
                        );
                        for y in [-off, off] {
                            b.cylinder_between(
                                v3(mouth - 1.0, y, 0.0),
                                v3(len, y, 0.0),
                                off * 0.3,
                                off * 0.2,
                                3,
                            );
                        }
                    }
                }
            });
        });
    });
}

/// A turret's hull in plan: `half` the outline (x, y >= 0) from the front round to the
/// back, mirrored across, lofted through `levels` (height, and how far the outline is
/// drawn in toward the middle there).
fn plan_hull(b: &mut MeshBuilder, half: &[[f32; 2]], levels: &[(f32, f32)]) {
    let outline: Vec<[f32; 2]> = half
        .iter()
        .copied()
        .chain(
            half.iter()
                .rev()
                .filter(|p| p[1] > 0.0)
                .map(|&[x, y]| [x, -y]),
        )
        .collect();
    let ring = |z: f32, k: f32| -> Vec<Vec3> {
        outline.iter().map(|&[x, y]| v3(x * k, y * k, z)).collect()
    };
    dark_plate(b);
    let rings: Vec<Vec<Vec3>> = levels.iter().map(|&(z, k)| ring(z, k)).collect();
    b.loft(&rings, true, true);
}

/// A keeled buttress along x on the ground, from `x0` (`w0` wide, `h0` high) out to `x1`.
fn buttress(b: &mut MeshBuilder, x0: f32, w0: f32, h0: f32, x1: f32, w1: f32, h1: f32) {
    let rings: Vec<Vec<Vec3>> = [(x0, w0, h0), (x1, w1, h1)]
        .iter()
        .map(|&(x, w, h)| section(v3(x, 0.0, h * 0.5), Vec3::Y, Vec3::Z, w, h, &KEELED))
        .collect();
    dark_plate(b);
    b.loft(&rings, true, true);
}

/// A thin lit red line let into a face (`machine::red_slot` at under half the
/// height asked): the Regency's red is inlaid hairlines, not lamps.
fn slit(b: &mut MeshBuilder, at: Vec3, out: Vec3, along: Vec3, len: f32, h: f32) {
    super::machine::red_slot(b, at, out, along, len, h * 0.45);
}

/// The cheeks of an open cradle turret: a plated wall either side of the gun, `y` out
/// from the bore and `thick` through, from `x[0]` to `x[1]` and `z[0]` up to `z[1]`, a
/// plate down its outside swept back past it into a spike, and the owner's colour along
/// its top.
fn cheeks(b: &mut MeshBuilder, x: [f32; 2], y: f32, z: [f32; 2], thick: f32) {
    let h = z[1] - z[0];
    b.mirror_y(|b| {
        dark_plate(b);
        b.block(v3(x[0], y, z[0]), v3(x[1], y + thick, z[1]));
        armour(
            b,
            &Frame::new(v3(x[1] + 0.2, y + thick, z[1] - h * 0.4), -Vec3::X, Vec3::Y),
            &[
                [0.0, -h * 0.4],
                [0.0, h * 0.45],
                [(x[1] - x[0]) * 0.8, h * 0.4],
                [(x[1] - x[0]) + h * 0.5, h * 0.2],
                [(x[1] - x[0]) * 0.8, -h * 0.4],
            ],
            thick * 0.5,
        );
        b.paint(TEAM);
        b.face(&[
            v3(x[0] + 0.3, y + 0.05, z[1] + 0.01),
            v3(x[1] - 0.3, y + 0.05, z[1] + 0.01),
            v3(x[1] - 0.3, y + thick - 0.05, z[1] + 0.01),
            v3(x[0] + 0.3, y + thick - 0.05, z[1] + 0.01),
        ]);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unit file's pivot and muzzle are the model's own.
    #[test]
    fn the_unit_files_guns_are_the_models() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        for (mesh, line) in [
            ("regency_barb", picket::LINE),
            ("regency_spitter", canopy::LINE),
            ("regency_airburst_repeater", gorget::LINE),
            ("regency_pinch_cannon", halberd::LINE),
            ("regency_fusion_cannon", sunspear::LINE),
        ] {
            let unit = bp.units.iter().find(|u| u.visual.mesh == mesh).expect(mesh);
            let w = &unit.weapons[0];
            let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
            assert!(v(w.muzzle).distance(line.muzzle) < 0.02, "{mesh} muzzle");
            assert!(
                v(w.pivot.unwrap()).distance(line.pivot) < 0.02,
                "{mesh} pivot"
            );
        }
    }
}

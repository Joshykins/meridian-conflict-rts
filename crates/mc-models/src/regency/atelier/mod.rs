//! The Atelier, the Regency engineering station (`regency_t{1,2,3}_eng_station`, tech 1 to 3
//! on one upgrade chain, a 2 x 2 lot): a static engineer, a tower whose crown streams
//! nanites onto sites hundreds of metres off. Nothing on it aims: the stream leaves the
//! crown's middle, at the top of the tower ([`EMITTER`]: the sim's beam origin for a
//! builder with no arm is the unit's position at its blueprint `height`).
//!
//! The tower ([`spire`]): a stepped plinth and vault, then a tall shaft of straight-sided
//! octagonal stages, plated, slotted and lit by turns. On it the crown ([`crown`]):
//! stacked machined collars, a slotted point, and floating halos of straight blades.
//!
//! Each tier builds onto the last (`machine::tier`): the next tier's pieces wait on the
//! model as upgrade pieces. Tech 1 is lean; tech 3 is rich.
//!
//! Light is the plates' own (CLAUDE.md section 9): nanites show in slots let into
//! `kit::nanite_plate` faces, red in `pattern::EMBER_SLOT` inlays, the gaskets are
//! `pattern::NANITE_RIM`. No lit pieces of mesh.

pub(super) mod crown;
mod spire;

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::kit::{dark_plate, frame, metal, nanite_plate, slab, v3};
use super::machine::{Course, Frame};
use super::parts::face_out;

/// Authored radius at every tier. The tower was drawn for a 3 x 3 lot; its blueprints
/// stand it on a 2 x 2 lot, so it is shown at two thirds of this size.
pub(super) const RADIUS: f32 = 16.9;
/// Authored height, the same at every tier: upgrades build round the tower, not up it.
pub(super) const HEIGHT: f32 = 46.0;
/// Where the stream leaves: the crown's middle, at the top.
pub(super) const EMITTER: Vec3 = Vec3::new(0.0, 0.0, HEIGHT);
/// The models' authored size, every tier the same.
pub(super) const SIZES: [(f32, f32); 3] = [(RADIUS, HEIGHT); 3];
/// Full-detail triangle budget: a 3 x 3 tower, plated in courses up its height, its
/// crown and its tech 3 kit.
#[cfg(test)]
pub(super) const TRIANGLES: usize = 9000;

/// A hexagon flat on top and bottom, (across, up): broad faces each side of a limb, wide
/// enough for a nanite slot.
const HEX: [[f32; 2]; 6] = [
    [1.0, 0.0],
    [0.5, 0.87],
    [-0.5, 0.87],
    [-1.0, 0.0],
    [-0.5, -0.87],
    [0.5, -0.87],
];
/// The station's dark armour in the machined finish: fine grain, recessed fasteners.
fn machined(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::EMBER_MACHINED);
}

/// The deeper shade of plate, for the faces between courses.
fn deep(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::EMBER_DEEP);
}

/// A section of `shape` (across, up) about `c`, square to `dir`, `w` across and `h` up.
fn section(c: Vec3, dir: Vec3, hint: Vec3, shape: &[[f32; 2]], w: f32, h: f32) -> Vec<Vec3> {
    let (side, up) = frame(dir, hint);
    shape
        .iter()
        .map(|[s, u]| c + side * (s * w) + up * (u * h))
        .collect()
}

/// A solid run through `points` (centre, half across, half up), its sections square to
/// the run, `hint` the way its "up" faces.
fn sleeve(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3, shape: &[[f32; 2]]) {
    let rings: Vec<Vec<Vec3>> = points
        .iter()
        .enumerate()
        .map(|(i, &(c, w, h))| {
            let dir = if i + 1 < points.len() {
                points[i + 1].0 - c
            } else {
                c - points[i - 1].0
            };
            section(c, dir, hint, shape, w, h)
        })
        .collect();
    b.with_facets(|b| b.loft(&rings, true, true));
}

/// One face of an upright prism or stage: its foot corners `a0`, `b0` and its head
/// corners `a1`, `b1` (`a` to `b` running anticlockwise seen from above).
#[derive(Clone, Copy)]
struct Facet {
    a0: Vec3,
    b0: Vec3,
    a1: Vec3,
    b1: Vec3,
}

impl Facet {
    /// The point `s` of the way up the face and `t` across it (-1 at `a`, 1 at `b`).
    fn at(&self, s: f32, t: f32) -> Vec3 {
        let k = (t + 1.0) * 0.5;
        self.a0.lerp(self.b0, k).lerp(self.a1.lerp(self.b1, k), s)
    }

    /// Out of the face.
    fn out(&self) -> Vec3 {
        let n = (self.b0 - self.a0)
            .cross(self.a1 - self.a0)
            .normalize_or_zero();
        let mid = (self.a0 + self.b1) * 0.5;
        if n.dot(mid.with_z(0.0)) < 0.0 {
            -n
        } else {
            n
        }
    }

    /// The patch from `s0` to `s1` up and `t0` to `t1` across, `lift` out of the face.
    fn patch(&self, s: [f32; 2], t: [f32; 2], lift: f32) -> [Vec3; 4] {
        let o = self.out() * lift;
        [
            self.at(s[0], t[0]) + o,
            self.at(s[0], t[1]) + o,
            self.at(s[1], t[1]) + o,
            self.at(s[1], t[0]) + o,
        ]
    }
}

/// The faces of an upright `n`-sided stage from `z0` (corner radius `r0`) to `z1` (`r1`),
/// its first corner at bearing `rot`.
fn facets(n: usize, rot: f32, z0: f32, z1: f32, r0: f32, r1: f32) -> Vec<Facet> {
    let corner = |k: usize, r: f32, z: f32| {
        let a = rot + std::f32::consts::TAU * k as f32 / n as f32;
        v3(a.cos() * r, a.sin() * r, z)
    };
    (0..n)
        .map(|k| Facet {
            a0: corner(k, r0, z0),
            b0: corner(k + 1, r0, z0),
            a1: corner(k, r1, z1),
            b1: corner(k + 1, r1, z1),
        })
        .collect()
}

/// An upright `n`-sided stage, as [`facets`] lays it out, solid, with the current brush.
fn stage(b: &mut MeshBuilder, n: usize, rot: f32, z0: f32, z1: f32, r0: f32, r1: f32) {
    let ring = |r: f32, z: f32| -> Vec<Vec3> {
        (0..n)
            .map(|k| {
                let a = rot + std::f32::consts::TAU * k as f32 / n as f32;
                v3(a.cos() * r, a.sin() * r, z)
            })
            .collect()
    };
    b.with_facets(|b| b.loft(&[ring(r0, z0), ring(r1, z1)], false, true));
}

/// A plate standing `thick` proud of a face over the patch `s` by `t`, with the current
/// brush: a panel let into the plating (a nanite slot with `nanite_plate`).
fn panel(b: &mut MeshBuilder, f: &Facet, s: [f32; 2], t: [f32; 2], thick: f32) {
    let q = f.patch(s, t, 0.0);
    slab(b, q, f.out() * thick);
}

/// A red slot let into a face over the patch `s` by `t` (`pattern::EMBER_SLOT`).
fn red_slot(b: &mut MeshBuilder, f: &Facet, s: [f32; 2], t: [f32; 2]) {
    let q = f.patch(s, t, 0.03);
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
    face_out(b, &q, f.out());
}

/// The machined finish laid on a face over the patch `s` by `t`: an access cover.
fn cover(b: &mut MeshBuilder, f: &Facet, s: [f32; 2], t: [f32; 2]) {
    let q = f.patch(s, t, 0.03);
    machined(b);
    face_out(b, &q, f.out());
}

/// A course of `count` swept plates laid down a face from `top` toward `foot`, facing
/// `out`, `half` across: armour lapped like feathers, the last one's spike at the foot.
fn lapped(b: &mut MeshBuilder, top: Vec3, foot: Vec3, out: Vec3, half: f32, count: usize) {
    let down = foot - top;
    let len = down.length();
    let step = len / (count as f32 + 0.3);
    let f = Frame::new(top + out * 0.02, down, out);
    dark_plate(b);
    Course {
        count,
        step,
        len: step * 1.25,
        half,
        tip: 0.0,
        thick: 0.3,
        tail: step * 0.2,
    }
    .lay(b, &f);
}

/// The owner's colour: a band of quads `z` up round a plan ring `inner` to `outer` out,
/// one quad between each pair of bearings in `bearings`.
fn team_band(b: &mut MeshBuilder, bearings: &[f32], inner: f32, outer: f32, z: f32) {
    b.paint(crate::material::TEAM);
    let at = |a: f32, r: f32| v3(a.cos() * r, a.sin() * r, z);
    for w in bearings.windows(2) {
        b.face(&[
            at(w[0], inner),
            at(w[0], outer),
            at(w[1], outer),
            at(w[1], inner),
        ]);
    }
}

/// The floating toothed ring a tech 3 tower carries, `r` out and `z` up, turning
/// (`part::SPINNER`) about the z axis.
fn spinner_ring(b: &mut MeshBuilder, z: f32, r: f32, teeth: usize) {
    use super::machine::{hoop, teeth as tooth_ring};
    b.with_part(crate::part::SPINNER, |b| {
        metal(b);
        let segs = b.sides(24);
        hoop(b, Vec3::Z * z, r, 1.0, 0.7, segs);
        if b.fine() {
            tooth_ring(b, Vec3::Z * z, r + 0.5, teeth, v3(0.45, 0.45, 0.5));
        }
    });
}

/// Slotted nanite plate as a hexagonal limb through `points`, its broad faces toward
/// `hint`: where the nanites gather on their way to the crown.
fn nanite_limb(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    nanite_plate(b);
    sleeve(b, points, hint, &HEX);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{part, rig};

    const DESIGNS: [&str; 1] = ["regency_atelier"];

    #[test]
    fn fits_the_librarys_checks_at_every_tier() {
        for key in DESIGNS {
            for tech in 1..=3 {
                super::super::check_at(key, tech, RADIUS, HEIGHT, Some(3), &[]);
            }
        }
    }

    /// The unit file's tower is the model's: no arm and no emitters, so the stream leaves
    /// the unit's position at the blueprint's height, and the model's crown is there, in
    /// its middle. Nothing aims: no turret, no arm.
    #[test]
    fn the_stream_leaves_the_crown() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        for tech in 1..=3u8 {
            let key = format!("regency_t{tech}_eng_station");
            let bp = blueprints.unit(blueprints.id_of(&key).unwrap());
            assert_eq!(bp.visual.mesh, "regency_atelier", "{key}");
            // The blueprint scales the tower evenly, so its height is still the crown's.
            let across = bp.radius.to_f32() / RADIUS;
            let up = bp.height.to_f32() / EMITTER.z;
            assert!((across / up - 1.0).abs() < 0.01, "{key}: stretched");
            let builder = bp.builder.as_ref().expect("it builds");
            assert!(builder.arm.is_none(), "{key}: a tower has no arm");
            assert!(
                builder.emitters.is_empty(),
                "{key}: one stream, from the crown"
            );
        }
        for key in DESIGNS {
            for tech in 1..=3 {
                let model = crate::build_model_scaled(key, RADIUS, HEIGHT, tech).unwrap();
                assert_eq!(model.arm_pivot, None, "{key}");
                for (lod, mesh) in model.lods.iter().enumerate() {
                    let name = format!("{key} T{tech} lod{lod}");
                    assert!(
                        mesh.vertices
                            .iter()
                            .all(|v| v.part != part::TURRET
                                && v.rig & rig::LIMB_MASK != rig::ARM_TOOL),
                        "{name}: nothing aims"
                    );
                    // The crown: the top of what stands (not the next tier's pieces).
                    let top = mesh
                        .vertices
                        .iter()
                        .filter(|v| v.rig & rig::UPGRADE == 0)
                        .map(|v| Vec3::from(v.pos))
                        .fold(Vec3::splat(f32::MIN), |a, p| if p.z > a.z { p } else { a });
                    assert!(
                        (top.z - EMITTER.z).abs() < 0.35,
                        "{name}: the crown tops out at {} for the stream at {}",
                        top.z,
                        EMITTER.z
                    );
                    assert!(
                        top.truncate().length() < 1.2,
                        "{name}: the crown's top {top} is off the middle"
                    );
                    // Nothing is lit by the mesh: light is the plates' own.
                    assert!(
                        !mesh.vertices.iter().any(|v| matches!(
                            v.material,
                            crate::material::GLOW_VIOLET | crate::material::GLOW_LASER
                        )),
                        "{name}: a lit piece of mesh"
                    );
                }
            }
        }
    }
}

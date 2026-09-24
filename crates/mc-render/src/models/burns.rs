//! Where a unit's burn marks are. The unit shader paints them (`surf_damage` in
//! `shaders/surface.wgsl`) and the renderer raises smoke and flame from them, so
//! both have to put them in the same place: everything here that decides a
//! position is integer arithmetic on the unit's id, mirrored line for line in
//! the shader. Change one side and the other must follow.

use super::{part, rig, MeshLod};

/// How many marks a unit can carry. Each comes up in its turn as health drops.
pub const BURN_MARKS: usize = 6;

/// One blast mark: a column through the model, in model space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurnMark {
    pub centre: [f32; 3],
    /// Radius once fully grown, metres.
    pub radius: f32,
    /// Zero until the mark's turn comes, one when it has grown in.
    pub grow: f32,
}

/// `surf_ihash` in the shader: a unit id and a small index to `0.0..1.0`. Kept
/// to 24 bits so the float is exact on both sides.
pub fn burn_hash(unit_id: u32, index: u32) -> f32 {
    let mut n = unit_id.wrapping_mul(1_597_334_677) ^ index.wrapping_mul(3_812_015_801);
    n ^= n >> 16;
    n = n.wrapping_mul(2_246_822_519);
    n ^= n >> 13;
    n = n.wrapping_mul(3_266_489_917);
    n ^= n >> 16;
    (n >> 8) as f32 / 16_777_216.0
}

/// The marks of unit `unit_id` at `health` (zero to one), on a model of this
/// reach (`Model::surface_reach`) and height.
pub fn burn_marks(unit_id: u32, health: f32, reach: f32, height: f32) -> [BurnMark; BURN_MARKS] {
    let hurt = 1.0 - health.clamp(0.0, 1.0);
    let spread = reach * 0.55;
    std::array::from_fn(|k| {
        let i = k as u32 * 4;
        let fk = k as f32;
        BurnMark {
            centre: [
                (burn_hash(unit_id, i) - 0.5) * 2.0 * spread,
                (burn_hash(unit_id, i + 1) - 0.5) * 2.0 * spread,
                burn_hash(unit_id, i + 2) * height,
            ],
            radius: reach * (0.22 + 0.18 * burn_hash(unit_id, i + 3)),
            grow: ((hurt - (fk + 0.4) / 6.6) * 5.0).clamp(0.0, 1.0),
        }
    })
}

/// A model's upper surface on a coarse grid, so an emitter can be stood on the
/// hull under a mark without searching the mesh every tick.
#[derive(Clone, Debug)]
pub struct BurnGrid {
    half: f32,
    /// Height of the highest face over each cell's middle; negative where there is none.
    z: Vec<f32>,
    /// That face turns with the turret.
    turret: Vec<bool>,
}

const RES: usize = 32;

impl BurnGrid {
    pub fn bake(mesh: &MeshLod) -> BurnGrid {
        let half = mesh
            .vertices
            .iter()
            .map(|v| v.pos[0].abs().max(v.pos[1].abs()))
            .fold(0.5f32, f32::max);
        let mut grid = BurnGrid {
            half,
            z: vec![-1.0; RES * RES],
            turret: vec![false; RES * RES],
        };
        let cell = 2.0 * half / RES as f32;
        let to_cell = |v: f32| ((v + half) / cell - 0.5).floor();
        for t in mesh.indices.chunks_exact(3) {
            let v = [0, 1, 2].map(|i| mesh.vertices[t[i] as usize]);
            // Not what spins, flies apart or is not fitted yet: fire would hang in the air there.
            let moving = matches!(v[0].part, part::SPINNER | part::ROTOR | part::VTOL_FRONT | part::VTOL_REAR | part::RAM | part::STRING | part::FEED)
                || v[0].rig & rig::UPGRADE != 0;
            if moving || v[0].normal[2] < 0.15 {
                continue;
            }
            let p = v.map(|v| v.pos);
            let (lo_x, hi_x) = (p.iter().map(|p| p[0]).fold(f32::MAX, f32::min), p.iter().map(|p| p[0]).fold(f32::MIN, f32::max));
            let (lo_y, hi_y) = (p.iter().map(|p| p[1]).fold(f32::MAX, f32::min), p.iter().map(|p| p[1]).fold(f32::MIN, f32::max));
            let det = (p[1][1] - p[2][1]) * (p[0][0] - p[2][0]) + (p[2][0] - p[1][0]) * (p[0][1] - p[2][1]);
            if det.abs() < 1e-9 {
                continue;
            }
            let (x0, x1) = (to_cell(lo_x).max(0.0) as usize, (to_cell(hi_x) + 1.0).clamp(0.0, RES as f32 - 1.0) as usize);
            let (y0, y1) = (to_cell(lo_y).max(0.0) as usize, (to_cell(hi_y) + 1.0).clamp(0.0, RES as f32 - 1.0) as usize);
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    let (x, y) = ((cx as f32 + 0.5) * cell - half, (cy as f32 + 0.5) * cell - half);
                    let a = ((p[1][1] - p[2][1]) * (x - p[2][0]) + (p[2][0] - p[1][0]) * (y - p[2][1])) / det;
                    let b = ((p[2][1] - p[0][1]) * (x - p[2][0]) + (p[0][0] - p[2][0]) * (y - p[2][1])) / det;
                    let c = 1.0 - a - b;
                    if a < -0.02 || b < -0.02 || c < -0.02 {
                        continue;
                    }
                    let z = a * p[0][2] + b * p[1][2] + c * p[2][2];
                    let i = cy * RES + cx;
                    if z > grid.z[i] {
                        grid.z[i] = z;
                        grid.turret[i] = v[0].part == part::TURRET;
                    }
                }
            }
        }
        grid
    }

    /// The point of the hull's upper surface nearest under `(x, y)` (model space),
    /// and whether it rides the turret. A mark off the side of the hull still
    /// scorches its edge, so the search walks in toward the middle until it finds
    /// metal. None for a model with no upper surface at all.
    pub fn surface(&self, x: f32, y: f32) -> Option<([f32; 3], bool)> {
        let cell = 2.0 * self.half / RES as f32;
        for step in 0..8 {
            let pull = 1.0 - step as f32 * 0.14;
            let (px, py) = (x * pull, y * pull);
            let cx = (((px + self.half) / cell).floor().clamp(0.0, RES as f32 - 1.0)) as usize;
            let cy = (((py + self.half) / cell).floor().clamp(0.0, RES as f32 - 1.0)) as usize;
            let i = cy * RES + cx;
            if self.z[i] >= 0.0 {
                return Some(([px, py, self.z[i]], self.turret[i]));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::builder::MeshBuilder;
    use glam::{Affine3A, Vec3};

    /// The shader holds the same constants; these values pin the CPU side so a
    /// change here is noticed and carried across.
    #[test]
    fn the_hash_is_the_one_the_shader_uses() {
        assert_eq!(burn_hash(0, 0), 0.0);
        let h = burn_hash(12345, 7);
        assert!((0.0..1.0).contains(&h));
        assert_eq!(h, burn_hash(12345, 7));
        assert_ne!(h, burn_hash(12345, 8));
        // 24 bits: exactly representable, so the GPU's conversion agrees.
        assert_eq!((h * 16_777_216.0).fract(), 0.0);
    }

    #[test]
    fn marks_come_up_one_by_one_and_stay_put() {
        let full = burn_marks(77, 1.0, 10.0, 4.0);
        assert!(full.iter().all(|m| m.grow == 0.0));
        let half = burn_marks(77, 0.5, 10.0, 4.0);
        let low = burn_marks(77, 0.1, 10.0, 4.0);
        let up = |marks: &[BurnMark; BURN_MARKS]| marks.iter().filter(|m| m.grow > 0.0).count();
        assert!(up(&half) >= 2 && up(&half) < up(&low));
        assert_eq!(up(&low), BURN_MARKS);
        for (a, b) in half.iter().zip(&low) {
            assert_eq!(a.centre, b.centre);
            assert!(a.centre[0].abs() <= 5.5 && a.centre[2] <= 4.0 && a.radius <= 4.0);
        }
    }

    #[test]
    fn an_emitter_stands_on_the_hull_and_knows_the_turret() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.cuboid(Vec3::new(0.0, 0.0, 1.0), Vec3::new(8.0, 4.0, 2.0));
        b.with_part(part::TURRET, |b| b.cuboid(Vec3::new(0.0, 0.0, 2.5), Vec3::new(2.0, 2.0, 1.0)));
        let grid = BurnGrid::bake(&b.finish());
        let (deck, on_turret) = grid.surface(3.0, 1.0).unwrap();
        assert!((deck[2] - 2.0).abs() < 1e-4 && !on_turret);
        let (roof, on_turret) = grid.surface(0.2, -0.3).unwrap();
        assert!((roof[2] - 3.0).abs() < 1e-4 && on_turret);
        // Off the side of the hull: pulled in until it is over metal.
        let (edge, _) = grid.surface(3.9, 3.9).unwrap();
        assert!(edge[1].abs() <= 2.0 && (edge[2] - 2.0).abs() < 1e-4);
    }
}

//! Cliff rock: the jointed sandstone the renderer lays over the canyon's walls
//! (mc-render `renderer/cliff_rocks.rs`). The 8 m heightfield can only draw a wall
//! as a smooth sheet; these give it the blocks, ledges, overhangs and broken
//! silhouette of real rock.
//!
//! A piece is a mass of rock blocks split by joints: vertical joints into columns,
//! bedding into courses. Each block is a box cut by a few fracture planes across its
//! front edges, so its faces are flat, its edges sharp and no two alike. Frame: the
//! wall's face is the plane x = 0 with the rock standing out toward +x and running
//! back into the hill toward -x; z runs up the wall, the origin at the middle of
//! the piece. Pieces stand upright, beds level, each course set back from the one
//! under it by the lean of the wall it is for (`LEANS`).

use glam::{Affine3A, Quat, Vec3};

use super::builder::{hash_unit, MeshBuilder};
use super::library::ModelDef;
use super::material::ROCK;
use crate::gpu_consts::scenery;

/// Half the piece's width (y) and height (z), metres at scale 1.
pub const HALF_WIDTH: f32 = 10.0;
pub const HALF_HEIGHT: f32 = 12.0;
/// How far the blocks run back into the hill behind the face.
const DEPTH: f32 = 7.0;

/// How far back each piece's courses step as they rise (run over rise): a piece
/// stands upright, its beds level, and follows a wall leaning back this much.
pub const LEANS: [f32; 3] = [0.2, 0.5, 0.9];
/// Pieces per lean, each its own mass of blocks.
pub const SEEDS: usize = 4;

/// The pieces, lean by lean (`LEANS`), `SEEDS` to a lean. The renderer's cliff
/// slots are these keys, in this order.
pub const KEYS: [&str; 12] = [
    "cliff_rock_a",
    "cliff_rock_b",
    "cliff_rock_c",
    "cliff_rock_d",
    "cliff_rock_e",
    "cliff_rock_f",
    "cliff_rock_g",
    "cliff_rock_h",
    "cliff_rock_i",
    "cliff_rock_j",
    "cliff_rock_k",
    "cliff_rock_l",
];

const R: f32 = 16.0;
const H: f32 = 2.0 * HALF_HEIGHT;
pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new(KEYS[0], R, H, |b, _| piece(b, 11, LEANS[0])),
    ModelDef::new(KEYS[1], R, H, |b, _| piece(b, 23, LEANS[0])),
    ModelDef::new(KEYS[2], R, H, |b, _| piece(b, 37, LEANS[0])),
    ModelDef::new(KEYS[3], R, H, |b, _| piece(b, 41, LEANS[0])),
    ModelDef::new(KEYS[4], R, H, |b, _| piece(b, 11, LEANS[1])),
    ModelDef::new(KEYS[5], R, H, |b, _| piece(b, 23, LEANS[1])),
    ModelDef::new(KEYS[6], R, H, |b, _| piece(b, 37, LEANS[1])),
    ModelDef::new(KEYS[7], R, H, |b, _| piece(b, 41, LEANS[1])),
    ModelDef::new(KEYS[8], R, H, |b, _| piece(b, 11, LEANS[2])),
    ModelDef::new(KEYS[9], R, H, |b, _| piece(b, 23, LEANS[2])),
    ModelDef::new(KEYS[10], R, H, |b, _| piece(b, 37, LEANS[2])),
    ModelDef::new(KEYS[11], R, H, |b, _| piece(b, 41, LEANS[2])),
];

/// A convex solid as its faces, each wound counter-clockwise seen from outside.
struct Block {
    faces: Vec<Vec<Vec3>>,
}

impl Block {
    fn cuboid(min: Vec3, max: Vec3) -> Self {
        let c = |i: u32| {
            Vec3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            )
        };
        let quads: [[u32; 4]; 6] = [
            [0, 4, 6, 2], // -x
            [1, 3, 7, 5], // +x
            [0, 1, 5, 4], // -y
            [2, 6, 7, 3], // +y
            [0, 2, 3, 1], // -z
            [4, 5, 7, 6], // +z
        ];
        Self {
            faces: quads.iter().map(|q| q.map(c).to_vec()).collect(),
        }
    }

    /// Cuts away everything on the side of the plane `n·p = d` that `n` points to,
    /// closing the cut with a face of its own.
    fn cut(&mut self, n: Vec3, d: f32) {
        let n = n.normalize();
        let mut cap: Vec<Vec3> = Vec::new();
        let mut kept = Vec::with_capacity(self.faces.len() + 1);
        for face in &self.faces {
            let mut out = Vec::with_capacity(face.len() + 1);
            for (i, &a) in face.iter().enumerate() {
                let b = face[(i + 1) % face.len()];
                let (da, db) = (n.dot(a) - d, n.dot(b) - d);
                if da <= 0.0 {
                    out.push(a);
                }
                if (da <= 0.0) != (db <= 0.0) {
                    let p = a + (b - a) * (da / (da - db));
                    out.push(p);
                    cap.push(p);
                }
            }
            if out.len() >= 3 {
                kept.push(out);
            }
        }
        cap.dedup_by(|a, b| a.distance(*b) < 1e-4);
        if cap.len() >= 3 {
            // Round the cut's middle, counter-clockwise seen from outside (+n).
            let mid = cap.iter().copied().sum::<Vec3>() / cap.len() as f32;
            let u = (cap[0] - mid).normalize_or(n.any_orthonormal_vector());
            let v = n.cross(u);
            let angle = |p: &Vec3| (*p - mid).dot(v).atan2((*p - mid).dot(u));
            cap.sort_by(|a, b| angle(a).total_cmp(&angle(b)));
            cap.dedup_by(|a, b| a.distance(*b) < 1e-4);
            if cap.len() >= 3 {
                kept.push(cap);
            }
        }
        self.faces = kept;
    }

    fn emit(&self, b: &mut MeshBuilder) {
        for face in &self.faces {
            b.face(face);
        }
    }
}

/// A random number in `lo..hi` from `seed` and a running counter.
struct Roll {
    seed: u32,
    n: u32,
}

impl Roll {
    fn next(&mut self, lo: f32, hi: f32) -> f32 {
        self.n += 1;
        lo + (hi - lo) * hash_unit(self.seed, self.n)
    }
}

/// Splits `lo..hi` into runs between `min` and `max` long.
fn split(roll: &mut Roll, lo: f32, hi: f32, min: f32, max: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut at = lo;
    while at < hi - 0.5 * min {
        let next = (at + roll.next(min, max)).min(hi);
        out.push((at, if hi - next < 0.6 * min { hi } else { next }));
        at = out[out.len() - 1].1;
    }
    out
}

fn piece(b: &mut MeshBuilder, seed: u32, lean: f32) {
    b.paint(ROCK).pattern(scenery::ROCK_CLIFF);
    let mut roll = Roll { seed, n: 0 };
    // Coarser levels join courses and drop the fractures: the blocks' outline is
    // what reads from afar.
    let (least, cuts) = match b.lod() {
        0 => (0.0, 2),
        1 => (5.0, 1),
        _ => (12.0, 0),
    };
    let columns = split(&mut roll, -HALF_WIDTH, HALF_WIDTH, 2.5, 11.0);
    b.with_facets(|b| {
        for (ci, &(y0, y1)) in columns.iter().enumerate() {
            // Each column stands out its own distance and frays at top and bottom,
            // so pieces side by side never meet in a straight line. Now and then a
            // buttress stands well proud, or a narrow chimney is cut deep.
            let mut col = Roll {
                seed: seed.wrapping_mul(31).wrapping_add(ci as u32 * 977),
                n: 0,
            };
            let kind = col.next(0.0, 1.0);
            let proud = if kind < 0.18 {
                col.next(2.0, 4.0)
            } else if kind < 0.3 && y1 - y0 < 5.0 {
                col.next(-3.5, -2.0)
            } else {
                col.next(-1.8, 2.0)
            };
            let top = HALF_HEIGHT - col.next(0.0, 8.0);
            let foot = -HALF_HEIGHT + col.next(0.0, 8.0);
            // The column's own beds, at full detail; coarser levels join them
            // until each is at least `least` tall.
            let fine = split(&mut col, foot, top, 1.6, 7.0);
            let mut beds: Vec<(f32, f32, usize)> = Vec::new();
            for (k, &(z0, z1)) in fine.iter().enumerate() {
                match beds.last_mut() {
                    Some(last) if last.1 - last.0 < least => last.1 = z1,
                    _ => beds.push((z0, z1, k)),
                }
            }
            let gap = col.next(0.05, 0.25);
            for &(lo, hi, k) in &beds {
                // The same rolls whatever the level, so levels agree on the shapes.
                let mut blk = Roll {
                    seed: col.seed ^ (k as u32 + 1).wrapping_mul(0x9E37_79B9),
                    n: 0,
                };
                // Each course set back up the wall: its front edge a ledge.
                let front = proud + blk.next(-1.2, 1.2) - lean * 0.5 * (lo + hi);
                let mut block = Block::cuboid(
                    Vec3::new(front - DEPTH, y0 + gap, lo + 0.06),
                    Vec3::new(front, y1 - gap, hi - 0.06),
                );
                // Fractures across the front: the first breaks the course's lip,
                // the rest come in at any angle from a point on its face. Each
                // cuts in from there a random depth.
                let middle = Vec3::new(front, 0.5 * (y0 + y1), 0.5 * (lo + hi));
                let half = Vec3::new(0.0, 0.5 * (y1 - y0), 0.5 * (hi - lo));
                let count = cuts + (blk.next(0.0, 2.99) as usize) * usize::from(cuts > 1);
                for c in 0..count {
                    let (dir, at) = if c == 0 {
                        (
                            Vec3::new(blk.next(0.4, 1.4), blk.next(-0.5, 0.5), 1.0),
                            middle + Vec3::Z * half.z,
                        )
                    } else {
                        let along = Vec3::new(0.0, blk.next(-1.0, 1.0), blk.next(-1.0, 1.0));
                        (
                            Vec3::new(blk.next(0.3, 1.2), along.y, along.z),
                            middle + along * half,
                        )
                    };
                    let n = dir.normalize();
                    let reach = blk.next(0.2, 0.6) * half.y.min(half.z).max(1.0);
                    block.cut(n, n.dot(at) - reach);
                }
                // Joints are never quite square: each block sits a little askew.
                let turn = Quat::from_euler(
                    glam::EulerRot::ZYX,
                    blk.next(-0.05, 0.05),
                    blk.next(-0.03, 0.03),
                    blk.next(-0.025, 0.025),
                );
                let frame = Affine3A::from_translation(middle)
                    * Affine3A::from_quat(turn)
                    * Affine3A::from_translation(-middle);
                b.with(frame, |b| block.emit(b));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::super::build_model;
    use super::KEYS;

    /// Every piece builds, closed blocks at each level, and the levels thin out:
    /// walls carry tens of thousands of them.
    #[test]
    fn cliff_pieces_build_with_falling_budgets() {
        for key in KEYS {
            let model = build_model(key).unwrap();
            let tris = model.lods.each_ref().map(|m| m.indices.len() / 3);
            assert!(tris[0] > 60 && tris[0] <= 900, "{key}: {tris:?}");
            assert!(tris[1] < tris[0] && tris[2] <= tris[1], "{key}: {tris:?}");
        }
    }
}

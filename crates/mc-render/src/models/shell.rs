//! Which way a hull shield pushes each vertex out.
//!
//! The meshes are flat shaded: a box corner is three vertices, one per face, each
//! with its own face's normal. Pushed out along those, the faces of every box come
//! apart into floating shards. Instead every vertex at the same place on the same
//! rigid piece gets one shared direction, the average of the faces meeting there,
//! lengthened so each of those faces still moves out by the full push. The skin
//! then stays closed.
//!
//! Packed into the high half of `MeshVertex::surface`, which the lit pass masks
//! off: octahedral direction 6 + 6 bits, then the length, 1.0 + 0.1 a step.

use std::collections::HashMap;

use super::{rig, MeshVertex};

pub const SHIFT: u32 = 16;
const LEVELS: f32 = 63.0;
/// Longest stretch: a corner sharper than this gets less than the full push on its faces.
const MAX_STRETCH: f32 = 2.5;

type Key = (i32, i32, i32, u32, u32);

fn key(v: &MeshVertex) -> Key {
    let q = |c: f32| (c * 512.0).round() as i32;
    // When in a refit a piece goes up does not move it: pieces welded by it stay one.
    (q(v.pos[0]), q(v.pos[1]), q(v.pos[2]), v.part, v.rig & !rig::UPGRADE_AT_MASK)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalized(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = dot(a, a).sqrt();
    (l > 1e-4).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// Octahedral map to 6 + 6 bits; `shell_dir` in `entity.wgsl` undoes it.
fn encode(d: [f32; 3]) -> u32 {
    let l1 = d[0].abs() + d[1].abs() + d[2].abs();
    let (mut x, mut y) = (d[0] / l1, d[1] / l1);
    if d[2] < 0.0 {
        let sx = if x >= 0.0 { 1.0 } else { -1.0 };
        let sy = if y >= 0.0 { 1.0 } else { -1.0 };
        (x, y) = ((1.0 - y.abs()) * sx, (1.0 - x.abs()) * sy);
    }
    let q = |c: f32| ((c * 0.5 + 0.5) * LEVELS).round().clamp(0.0, LEVELS) as u32;
    q(x) | q(y) << 6
}

/// Writes the shell direction into every vertex of one mesh.
pub fn pack(vertices: &mut [MeshVertex]) {
    let mut faces: HashMap<Key, Vec<[f32; 3]>> = HashMap::new();
    for v in vertices.iter() {
        let Some(n) = normalized(v.normal) else { continue };
        let seen = faces.entry(key(v)).or_default();
        // A face split into many triangles counts once.
        if !seen.iter().any(|m| dot(*m, n) > 0.999) {
            seen.push(n);
        }
    }
    let mut packed: HashMap<Key, u32> = HashMap::with_capacity(faces.len());
    for (k, normals) in &faces {
        let sum = normals.iter().fold([0.0; 3], |s, n| [s[0] + n[0], s[1] + n[1], s[2] + n[2]]);
        // Faces that cancel out (the two sides of a sheet) keep the first one's way.
        let dir = normalized(sum).unwrap_or(normals[0]);
        let least = normals.iter().map(|n| dot(*n, dir)).fold(1.0f32, f32::min);
        let stretch = (1.0 / least.max(1.0 / MAX_STRETCH)).clamp(1.0, MAX_STRETCH);
        let step = ((stretch - 1.0) * 10.0).round() as u32;
        packed.insert(*k, encode(dir) | step.min(15) << 12);
    }
    for v in vertices.iter_mut() {
        let shell = packed.get(&key(v)).copied().unwrap_or(0);
        v.surface = (v.surface & 0xFFFF) | shell << SHIFT;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bits: u32) -> ([f32; 3], f32) {
        let s = bits >> SHIFT;
        let x = (s & 63) as f32 / LEVELS * 2.0 - 1.0;
        let y = ((s >> 6) & 63) as f32 / LEVELS * 2.0 - 1.0;
        let mut d = [x, y, 1.0 - x.abs() - y.abs()];
        let t = (-d[2]).max(0.0);
        d[0] += if d[0] >= 0.0 { -t } else { t };
        d[1] += if d[1] >= 0.0 { -t } else { t };
        (normalized(d).unwrap(), 1.0 + ((s >> 12) & 15) as f32 * 0.1)
    }

    fn vertex(pos: [f32; 3], normal: [f32; 3]) -> MeshVertex {
        MeshVertex {
            pos,
            normal,
            uv: [0.0; 2],
            material: 0,
            part: 0,
            rig: 0,
            face: [0.0; 4],
            surface: 5 | 77 << 8,
        }
    }

    #[test]
    fn a_box_corner_moves_as_one_and_each_face_the_full_push() {
        let normals = [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]];
        let mut vs: Vec<_> = normals.iter().map(|&n| vertex([1.0, -1.0, 0.0], n)).collect();
        pack(&mut vs);
        assert!(vs.iter().all(|v| v.surface == vs[0].surface), "one corner, one way out");
        assert!(vs.iter().all(|v| v.surface & 0xFFFF == 5 | 77 << 8), "the lit pass keeps its bits");
        let (d, m) = decode(vs[0].surface);
        for n in normals {
            let along = dot(d, n) * m;
            assert!((along - 1.0).abs() < 0.08, "face {n:?} pushed {along}");
        }
    }

    #[test]
    fn directions_survive_the_packing() {
        for d in [[0.0, 0.0, 1.0], [0.0, 0.0, -1.0], [0.6, -0.8, 0.0], [-0.3, 0.4, -0.866]] {
            let mut vs = vec![vertex([0.0; 3], d)];
            pack(&mut vs);
            let (got, m) = decode(vs[0].surface);
            assert!(dot(got, d) > 0.995, "{d:?} came back {got:?}");
            assert_eq!(m, 1.0);
        }
    }
}

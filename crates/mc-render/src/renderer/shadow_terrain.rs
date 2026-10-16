//! The terrain's nodes each sun shadow cascade draws. The camera's nodes all cast into
//! every cascade otherwise: a low view's far ground, hundreds of nodes the near
//! cascade's square never holds, ran the whole terrain vertex shader three times over.
//!
//! A cascade draws a node only where it lands in the cascade's square, and not where
//! every point its shadow can fall on reads the nearer cascade (`shadow_rim`, as
//! cull.wgsl `held_nearer` does for units and props): a shadow falls along the light, so
//! in light space it lies under what casts it.

use std::ops::Range;

use glam::{Mat4, Vec3};

use super::shadow_cascades::{Cascade, CASCADES};
use crate::gpu_consts::shadow_rim;
use crate::terrain::TerrainNode;

impl super::Renderer {
    /// Writes each cascade's nodes after the camera's (`node_scratch`, already written)
    /// and returns where each cascade's run is. The camera's ground stands between
    /// heights `z`.
    pub(super) fn write_shadow_nodes(
        &mut self,
        cascades: &[Cascade; CASCADES],
        z: (f32, f32),
    ) -> [Range<u32>; CASCADES] {
        let camera = self.node_scratch.len();
        let runs = cascade_nodes(cascades, &mut self.node_scratch, z);
        self.nodes.write(
            (camera * size_of::<TerrainNode>()) as u64,
            bytemuck::cast_slice(&self.node_scratch[camera..]),
        );
        self.node_scratch.truncate(camera);
        runs
    }
}

/// Appends each cascade's nodes of `nodes` (the camera's, standing between heights
/// `z`) after them in `nodes`, and returns where each cascade's run is.
pub(super) fn cascade_nodes(
    cascades: &[Cascade; CASCADES],
    nodes: &mut Vec<TerrainNode>,
    z: (f32, f32),
) -> [Range<u32>; CASCADES] {
    let camera = nodes.len();
    std::array::from_fn(|c| {
        let start = nodes.len() as u32;
        for i in 0..camera {
            let node = nodes[i];
            let corners = corners(&node, z);
            if !lands_in(&cascades[c].view_proj, &corners) {
                continue;
            }
            if c > 0 && held(&cascades[c - 1], &corners) {
                continue;
            }
            nodes.push(node);
        }
        start..nodes.len() as u32
    })
}

/// The node's box, its whole height span.
fn corners(node: &TerrainNode, z: (f32, f32)) -> [Vec3; 8] {
    let [x, y, size, _] = node.rect;
    std::array::from_fn(|k| {
        Vec3::new(
            x + size * (k & 1) as f32,
            y + size * ((k >> 1) & 1) as f32,
            if k < 4 { z.0 } else { z.1 },
        )
    })
}

/// Whether any of the box lands in the cascade's square (orthographic: w is 1).
fn lands_in(view_proj: &Mat4, corners: &[Vec3; 8]) -> bool {
    let clip = corners.map(|p| view_proj.project_point3(p));
    let all = |f: &dyn Fn(Vec3) -> bool| clip.iter().all(|&p| f(p));
    !(all(&|p| p.x < -1.0)
        || all(&|p| p.x > 1.0)
        || all(&|p| p.y < -1.0)
        || all(&|p| p.y > 1.0)
        || all(&|p| p.z < 0.0)
        || all(&|p| p.z > 1.0))
}

/// Whether the whole box, with the texels to spare, is read from `cascade` alone.
fn held(cascade: &Cascade, corners: &[Vec3; 8]) -> bool {
    let m = cascade.view_proj;
    let [texel, depth_span, ..] = cascade.info;
    let per_metre = Vec3::new(m.x_axis.x, m.y_axis.x, m.z_axis.x).length();
    let spare = shadow_rim::INNER - per_metre * shadow_rim::TEXELS * texel;
    let depth = shadow_rim::DEPTH_M / depth_span;
    corners.iter().all(|&p| {
        let q = m.project_point3(p);
        q.x.abs() < spare && q.y.abs() < spare && q.z > depth && q.z < 1.0 - depth
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;
    use glam::Vec2;

    fn node(x: f32, y: f32, size: f32) -> TerrainNode {
        TerrainNode {
            rect: [x, y, size, 0.0],
            morph: [0.0; 4],
        }
    }

    fn cascades(distance: f32) -> [Cascade; CASCADES] {
        let mut camera = Camera::new(Vec2::splat(16384.0), Vec2::new(1920.0, 1080.0));
        camera.focus = Vec3::new(8000.0, 8000.0, 40.0);
        camera.distance = distance;
        let sun = Vec3::new(0.4, -0.3, 0.8).normalize();
        super::super::shadow_cascades::fit(&camera, sun, (0.0, 200.0), 2048)
    }

    /// A node under the focus is drawn by the first cascade that holds it and held back
    /// from those past it; one far off is in none.
    #[test]
    fn each_cascade_draws_only_what_it_needs() {
        for distance in [300.0, 1135.0, 4000.0] {
            let c = cascades(distance);
            let under = node(7990.0, 7990.0, 20.0);
            let far = node(1000.0, 1000.0, 20.0);
            let mut nodes = vec![under, far];
            let runs = cascade_nodes(&c, &mut nodes, (0.0, 60.0));
            let rect = |r: &Range<u32>| -> Vec<[f32; 4]> {
                nodes[r.start as usize..r.end as usize]
                    .iter()
                    .map(|n| n.rect)
                    .collect()
            };
            let box_ = corners(&under, (0.0, 60.0));
            let first = (0..CASCADES)
                .find(|&k| held(&c[k], &box_))
                .expect("a cascade holds the focus");
            assert_eq!(rect(&runs[first]), vec![under.rect], "at {distance} m");
            for r in &runs[first + 1..] {
                assert!(rect(r).is_empty(), "at {distance} m: {:?}", rect(r));
            }
        }
    }

    /// A node over the near cascade's rim still casts into the next one, which those
    /// pixels blend into.
    #[test]
    fn the_rim_is_drawn_twice() {
        let c = cascades(400.0);
        let mut ring = Vec::new();
        for i in -40..40 {
            for j in -40..40 {
                ring.push(node(
                    8000.0 + i as f32 * 20.0,
                    8000.0 + j as f32 * 20.0,
                    20.0,
                ));
            }
        }
        let mut nodes = ring.clone();
        let runs = cascade_nodes(&c, &mut nodes, (0.0, 60.0));
        let rims = ring.iter().filter(|n| {
            let corners = corners(n, (0.0, 60.0));
            let q = corners.map(|p| c[0].view_proj.project_point3(p));
            q.iter().any(|p| {
                p.x.abs().max(p.y.abs()) > shadow_rim::INNER && p.x.abs().max(p.y.abs()) < 1.0
            })
        });
        for n in rims {
            let in_next = nodes[runs[1].start as usize..runs[1].end as usize]
                .iter()
                .any(|m| m.rect == n.rect);
            assert!(in_next, "{:?} on the rim is missing from cascade 1", n.rect);
        }
    }
}

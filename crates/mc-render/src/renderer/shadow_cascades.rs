//! Sun shadow cascades: three shadow maps, each fitted to a slice of what the
//! camera sees, near slices small and sharp, far ones coarse.
//!
//! Two things keep them from swimming. Each cascade is a sphere, so turning
//! the camera does not change its size, and its radius only moves in steps of
//! 2^(1/8), so zooming changes the texel size now and then instead of every
//! frame. And its centre is snapped to its own texel grid in a light space that
//! turns only with the sun, so panning slides the map by whole texels and the
//! shadow edges stay put.

use glam::{Mat4, Vec3, Vec4};

use crate::camera::Camera;

pub(super) const CASCADES: usize = 3;
// The cull builds one draw list per cascade (cull.wgsl).
const _: () = assert!(
    crate::gpu_consts::cull_list::COUNT as usize
        == crate::gpu_consts::cull_list::SHADOW as usize + CASCADES
);

/// Metres toward the sun past a cascade's sphere that still cast into it: tall
/// terrain and aircraft under the cloud layer.
const CASTER_REACH: f32 = 900.0;

pub(super) struct Cascade {
    pub view_proj: Mat4,
    /// Metres per shadow texel, metres of depth the map spans.
    pub info: [f32; 4],
}

/// `z_range` is the lowest and highest ground on the map.
pub(super) fn fit(
    camera: &Camera,
    sun: Vec3,
    z_range: (f32, f32),
    size: u32,
) -> [Cascade; CASCADES] {
    let view_proj = camera.view_proj();
    let inv = view_proj.inverse();
    let eye = camera.eye();
    let near = (camera.distance * 0.02).clamp(0.5, 500.0);
    let d = camera.distance;

    // Where on each screen-corner ray, per metre of view depth. Reversed infinite
    // projection: NDC depth is near / view depth.
    let unproject = |x: f32, y: f32, t: f32| -> Vec3 {
        let p = inv * Vec4::new(x, y, near / t, 1.0);
        p.truncate() / p.w
    };
    let ray = |x: f32, y: f32| (unproject(x, y, 1000.0) - eye) / 1000.0;

    // Only the ground and what stands on it receives shadow: a slab around the focus.
    let reach = 30.0 + 0.6 * d;
    let z_lo = (camera.focus.z - reach).max(z_range.0 - 5.0);
    let z_hi = (camera.focus.z + reach).min(z_range.1 + 60.0);

    // Depths the ground spans on screen, from the bottom edge's ray to the top's.
    let ground_depth = |r: Vec3| {
        if r.z < -1e-4 {
            Some((z_lo - eye.z) / r.z).filter(|t| *t > 0.0)
        } else {
            None
        }
    };
    let start = ground_depth(ray(0.0, -1.0))
        .map(|t| t * 0.5)
        .unwrap_or(near)
        .max(near);
    let end = ground_depth(ray(0.0, 1.0))
        .map(|t| t * 1.1)
        .unwrap_or(f32::MAX)
        .min(d * 6.0)
        .max(start * 2.0);

    // Mostly logarithmic splits.
    let split = |i: usize| {
        let f = i as f32 / CASCADES as f32;
        let log = start * (end / start).powf(f);
        let lin = start + (end - start) * f;
        0.75 * log + 0.25 * lin
    };

    let up = if sun.z.abs() > 0.99 { Vec3::Y } else { Vec3::Z };
    let light = glam::camera::rh::view::look_at_mat4(Vec3::ZERO, -sun, up);
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let rays = corners.map(|(x, y)| ray(x, y));
    let ahead = Vec3::new(camera.yaw.sin(), camera.yaw.cos(), 0.0);
    let side = Vec3::new(ahead.y, -ahead.x, 0.0);

    std::array::from_fn(|i| {
        let (a, b) = (split(i), split(i + 1));
        let box_corners: [Vec3; 8] =
            std::array::from_fn(|k| eye + rays[k % 4] * if k < 4 { a } else { b });
        // The slice's edges clipped to the slab: their ends are the corners of
        // the part of the slice that can receive shadow.
        let mut points = Vec::with_capacity(24);
        for k in 0..4 {
            for (p, q) in [
                (box_corners[k], box_corners[(k + 1) % 4]),
                (box_corners[k + 4], box_corners[(k + 1) % 4 + 4]),
                (box_corners[k], box_corners[k + 4]),
            ] {
                if let Some((p, q)) = clip_to_slab(p, q, z_lo, z_hi) {
                    points.push(p);
                    points.push(q);
                }
            }
        }
        if points.is_empty() {
            points.extend_from_slice(&box_corners);
        }
        // Boxed in the camera's heading frame, so the centre turns with the camera
        // and the radius stays the same.
        let to_local = |p: Vec3| {
            let r = p - eye;
            Vec3::new(r.dot(ahead), r.dot(side), r.z)
        };
        let (lo, hi) = points.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), p| (lo.min(to_local(*p)), hi.max(to_local(*p))),
        );
        let mid = (lo + hi) * 0.5;
        let centre = eye + ahead * mid.x + side * mid.y + Vec3::Z * mid.z;
        let radius = points
            .iter()
            .map(|p| p.distance(centre))
            .fold(1.0f32, f32::max);
        let radius = (radius.log2() * 8.0).ceil() / 8.0;
        let radius = radius.exp2();

        let texel = radius * 2.0 / size as f32;
        let c = light.transform_point3(centre);
        let (cx, cy) = ((c.x / texel).round() * texel, (c.y / texel).round() * texel);
        // Light view looks down -Z: nearer the sun is larger z.
        let near_z = -c.z - radius - CASTER_REACH;
        let far_z = -c.z + radius;
        let proj = glam::camera::rh::proj::directx::orthographic(
            cx - radius,
            cx + radius,
            cy - radius,
            cy + radius,
            near_z,
            far_z,
        );
        Cascade {
            view_proj: proj * light,
            info: [texel, far_z - near_z, 0.0, 0.0],
        }
    })
}

fn clip_to_slab(p: Vec3, q: Vec3, lo: f32, hi: f32) -> Option<(Vec3, Vec3)> {
    let dz = q.z - p.z;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    if dz.abs() < 1e-6 {
        if p.z < lo || p.z > hi {
            return None;
        }
    } else {
        let (a, b) = ((lo - p.z) / dz, (hi - p.z) / dz);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some((p + (q - p) * t0, p + (q - p) * t1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn camera(focus: Vec3, distance: f32, yaw: f32) -> Camera {
        let mut c = Camera::new(Vec2::splat(16384.0), Vec2::new(1920.0, 1080.0));
        c.focus = focus;
        c.distance = distance;
        c.yaw = yaw;
        c
    }

    fn sun() -> Vec3 {
        Vec3::new(0.4, -0.3, 0.8).normalize()
    }

    /// Where a world point lands in a cascade's map, in texels.
    fn texel_of(c: &Cascade, p: Vec3) -> (f32, f32) {
        let clip = c.view_proj * p.extend(1.0);
        ((clip.x * 0.5 + 0.5) * 2048.0, (clip.y * 0.5 + 0.5) * 2048.0)
    }

    #[test]
    fn panning_moves_the_map_by_whole_texels() {
        let a = fit(
            &camera(Vec3::new(8000.0, 8000.0, 40.0), 400.0, 0.3),
            sun(),
            (0.0, 200.0),
            2048,
        );
        for step in [0.37, 1.9, 13.3] {
            let b = fit(
                &camera(
                    Vec3::new(8000.0 + step, 8000.0 - step * 0.6, 40.0),
                    400.0,
                    0.3,
                ),
                sun(),
                (0.0, 200.0),
                2048,
            );
            for i in 0..CASCADES {
                if a[i].info[0] != b[i].info[0] {
                    continue;
                }
                let p = Vec3::new(8010.0, 7990.0, 40.0);
                let (ax, ay) = texel_of(&a[i], p);
                let (bx, by) = texel_of(&b[i], p);
                let (fx, fy) = ((ax - bx).abs().fract(), (ay - by).abs().fract());
                assert!(
                    fx.min(1.0 - fx) < 0.02 && fy.min(1.0 - fy) < 0.02,
                    "cascade {i}: {fx} {fy}"
                );
            }
        }
    }

    #[test]
    fn turning_keeps_the_texel_size() {
        let focus = Vec3::new(8000.0, 8000.0, 40.0);
        let a = fit(&camera(focus, 400.0, 0.0), sun(), (0.0, 200.0), 2048);
        let b = fit(&camera(focus, 400.0, 2.1), sun(), (0.0, 200.0), 2048);
        for i in 0..CASCADES {
            assert_eq!(a[i].info[0], b[i].info[0], "cascade {i}");
        }
    }

    #[test]
    fn near_cascade_is_sharp_and_covers_the_focus() {
        let focus = Vec3::new(8000.0, 8000.0, 40.0);
        let c = fit(&camera(focus, 300.0, 0.7), sun(), (0.0, 200.0), 2048);
        // Old single map: 1.4 x distance either side over 2048 texels.
        let old = 300.0 * 1.4 * 2.0 / 2048.0;
        assert!(c[0].info[0] < old * 0.6, "{} vs {old}", c[0].info[0]);
        let inside = |c: &Cascade| {
            let p = c.view_proj * focus.extend(1.0);
            p.x.abs() < 1.0 && p.y.abs() < 1.0 && p.z > 0.0 && p.z < 1.0
        };
        assert!(c.iter().any(inside));
    }
}

//! Mesh-building toolkit for the procedural models.
//!
//! A [`MeshBuilder`] is a brush: it carries a current material, part and
//! transform, and every primitive is emitted with them. All primitives are
//! flat shaded (each face owns its vertices), which suits the hard-edged
//! look, and get box-projected UVs in metres.
//!
//! Every solid is built as a *loft*: a stack of point rings joined by quads
//! and closed by two caps. After the current transform is applied the solid's
//! signed volume decides its orientation, so faces wind counter-clockwise
//! seen from outside whatever the ring order, and mirrored transforms
//! (see [`MeshBuilder::mirror_y`]) need no special handling.

use std::f32::consts::{PI, TAU};

use glam::{Affine3A, Vec2, Vec3};

use super::{material, part, MeshLod, MeshVertex, LOD_COUNT};

/// Triangles smaller than this (m²) are dropped instead of emitted.
const MIN_TRIANGLE_AREA: f32 = 2.0e-5;
/// Points closer than this (m) are merged when a face is emitted.
const WELD_DISTANCE: f32 = 1.0e-4;

/// One horizontal slice of a [`MeshBuilder::loft_z`] solid.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub z: f32,
    /// Scale of the plan profile about the profile origin.
    pub scale: Vec2,
    /// Offset of the scaled profile.
    pub shift: Vec2,
}

impl Section {
    pub fn new(z: f32, scale: f32) -> Self {
        Section { z, scale: Vec2::splat(scale), shift: Vec2::ZERO }
    }

    pub fn scaled(z: f32, scale_x: f32, scale_y: f32) -> Self {
        Section { z, scale: Vec2::new(scale_x, scale_y), shift: Vec2::ZERO }
    }

    pub fn shifted(mut self, x: f32, y: f32) -> Self {
        self.shift = Vec2::new(x, y);
        self
    }
}

pub struct MeshBuilder {
    lod: usize,
    mesh: MeshLod,
    material: u32,
    part: u32,
    transform: Affine3A,
    turret_pivot: Vec3,
    spinner_pivot: Vec3,
    /// Index ranges of the closed solids, so tests can check each one's orientation.
    #[cfg(test)]
    solids: Vec<std::ops::Range<usize>>,
}

impl MeshBuilder {
    /// A builder for level of detail `lod` (0 = full) whose root transform is `root`.
    pub fn new(lod: usize, root: Affine3A) -> Self {
        assert!(lod < LOD_COUNT);
        MeshBuilder {
            lod,
            mesh: MeshLod::default(),
            material: material::PLATING,
            part: part::HULL,
            transform: root,
            turret_pivot: root.transform_point3(Vec3::ZERO),
            spinner_pivot: root.transform_point3(Vec3::ZERO),
            #[cfg(test)]
            solids: Vec::new(),
        }
    }

    pub fn finish(self) -> MeshLod {
        self.mesh
    }

    // ---- level of detail -------------------------------------------------

    pub fn lod(&self) -> usize {
        self.lod
    }

    /// Full detail only: greebles, glow strips, antennae.
    pub fn fine(&self) -> bool {
        self.lod == 0
    }

    /// Everything except the box-silhouette level.
    pub fn mid(&self) -> bool {
        self.lod <= 1
    }

    /// The last level: a handful of boxes.
    pub fn coarse(&self) -> bool {
        self.lod == LOD_COUNT - 1
    }

    /// Side count for a round shape that has `n` sides at full detail.
    pub fn sides(&self, n: usize) -> usize {
        match self.lod {
            0 => n,
            1 => (n * 3 / 4).max(4),
            _ => 4,
        }
    }

    // ---- brush -----------------------------------------------------------

    pub fn paint(&mut self, material: u32) -> &mut Self {
        self.material = material;
        self
    }

    /// Runs `f` with vertices assigned to `part`, then restores the previous part.
    pub fn with_part(&mut self, part: u32, f: impl FnOnce(&mut Self)) {
        let previous = std::mem::replace(&mut self.part, part);
        f(self);
        self.part = previous;
    }

    /// Runs `f` inside the local frame `local` (composed onto the current transform).
    pub fn with(&mut self, local: Affine3A, f: impl FnOnce(&mut Self)) {
        let previous = self.transform;
        self.transform = previous * local;
        f(self);
        self.transform = previous;
    }

    pub fn at(&mut self, offset: Vec3, f: impl FnOnce(&mut Self)) {
        self.with(Affine3A::from_translation(offset), f);
    }

    /// A frame at `pivot` whose +x axis is pitched up by `angle` radians: for
    /// elevated barrels and raked racks.
    pub fn pitched(&mut self, pivot: Vec3, angle: f32, f: impl FnOnce(&mut Self)) {
        self.with(Affine3A::from_translation(pivot) * Affine3A::from_rotation_y(-angle), f);
    }

    /// A frame at `pivot` yawed counter-clockwise (seen from above) by `angle` radians.
    pub fn yawed(&mut self, pivot: Vec3, angle: f32, f: impl FnOnce(&mut Self)) {
        self.with(Affine3A::from_translation(pivot) * Affine3A::from_rotation_z(angle), f);
    }

    /// Emits `f` twice: as written, and mirrored to the other side (y negated).
    pub fn mirror_y(&mut self, f: impl Fn(&mut Self)) {
        f(self);
        self.with(Affine3A::from_scale(Vec3::new(1.0, -1.0, 1.0)), |b| f(b));
    }

    /// Emits `f` `count` times, each turned a further `1/count` of a turn about the z axis.
    pub fn radial(&mut self, count: usize, f: impl Fn(&mut Self)) {
        for i in 0..count {
            self.with(Affine3A::from_rotation_z(TAU * i as f32 / count as f32), |b| f(b));
        }
    }

    /// Records the turret yaw axis (given in the current frame).
    pub fn set_turret_pivot(&mut self, pivot: Vec3) {
        self.turret_pivot = self.transform.transform_point3(pivot);
    }

    /// Records the spinner axis (given in the current frame).
    pub fn set_spinner_pivot(&mut self, pivot: Vec3) {
        self.spinner_pivot = self.transform.transform_point3(pivot);
    }

    pub fn turret_pivot(&self) -> Vec3 {
        self.turret_pivot
    }

    pub fn spinner_pivot(&self) -> Vec3 {
        self.spinner_pivot
    }

    // ---- boxes -----------------------------------------------------------

    /// Axis-aligned box.
    pub fn cuboid(&mut self, center: Vec3, size: Vec3) {
        let base = center - Vec3::Z * (size.z * 0.5);
        self.frustum(base, size.truncate(), size.truncate(), size.z, Vec2::ZERO);
    }

    /// Axis-aligned box from its two corners.
    pub fn block(&mut self, min: Vec3, max: Vec3) {
        self.cuboid((min + max) * 0.5, max - min);
    }

    /// Box without its bottom face, for shapes that sit on the ground or on another solid.
    pub fn cuboid_open(&mut self, center: Vec3, size: Vec3) {
        let base = center - Vec3::Z * (size.z * 0.5);
        self.frustum_open(base, size.truncate(), size.truncate(), size.z, Vec2::ZERO);
    }

    /// Trapezoid prism: a `base` rectangle (x, y size) at `base_center` rising
    /// `height` to a `top` rectangle offset by `top_shift`. Covers tapered
    /// boxes, wedges (`top.x` near zero) and pyramids.
    pub fn frustum(&mut self, base_center: Vec3, base: Vec2, top: Vec2, height: f32, top_shift: Vec2) {
        self.loft(&frustum_rings(base_center, base, top, height, top_shift), true, true);
    }

    /// [`Self::frustum`] without its bottom face.
    pub fn frustum_open(&mut self, base_center: Vec3, base: Vec2, top: Vec2, height: f32, top_shift: Vec2) {
        self.loft(&frustum_rings(base_center, base, top, height, top_shift), false, true);
    }

    /// Armour plate lying on a surface: straight sides, bevelled top edges, no
    /// bottom face. `base_center` is the middle of the underside. Below full
    /// detail the bevel is dropped.
    pub fn plate(&mut self, base_center: Vec3, size: Vec2, thickness: f32, bevel: f32) {
        if !self.fine() {
            self.cuboid_open(base_center + Vec3::Z * (thickness * 0.5), size.extend(thickness));
            return;
        }
        let c = base_center.truncate();
        let bevel = bevel.min(thickness * 0.9).min(size.min_element() * 0.45);
        let rings = [
            rect_ring(c, size * 0.5, base_center.z),
            rect_ring(c, size * 0.5, base_center.z + thickness - bevel),
            rect_ring(c, size * 0.5 - Vec2::splat(bevel), base_center.z + thickness),
        ];
        self.loft(&rings, false, true);
    }

    /// Box whose four vertical corners are cut at 45 degrees (octagonal plan).
    pub fn chamfered_box(&mut self, center: Vec3, size: Vec3, chamfer: f32) {
        let profile = chamfered_rect(size.truncate() * 0.5, chamfer);
        self.at(center.truncate().extend(0.0), |b| b.extrude_z(&profile, center.z - size.z * 0.5, center.z + size.z * 0.5));
    }

    // ---- round shapes ----------------------------------------------------

    /// Upright n-gon prism, cylinder, cone (`top_radius` 0) or tapered drum.
    /// A flat side faces +x. Radii are circumradii.
    pub fn prism(&mut self, base_center: Vec3, sides: usize, base_radius: f32, top_radius: f32, height: f32) {
        let ring = |radius: f32, z: f32| ngon_ring(base_center.truncate(), sides, radius, z);
        self.loft(&[ring(base_radius, base_center.z), ring(top_radius, base_center.z + height)], true, true);
    }

    /// Round bar from `a` to `b` with a radius at each end: barrels, struts, limbs, branches.
    pub fn cylinder_between(&mut self, a: Vec3, b: Vec3, radius_a: f32, radius_b: f32, sides: usize) {
        let Some((side, up)) = bar_frame(a, b) else { return };
        let ring = |center: Vec3, radius: f32| -> Vec<Vec3> {
            (0..sides)
                .map(|i| {
                    let angle = (i as f32 + 0.5) * TAU / sides as f32;
                    center + (side * angle.cos() + up * angle.sin()) * radius
                })
                .collect()
        };
        self.loft(&[ring(a, radius_a), ring(b, radius_b)], true, true);
    }

    /// Rectangular bar from `a` to `b`. Each size is (width, thickness): for a
    /// bar along x that is (y extent, z extent); for an upright bar (y extent,
    /// x extent); for a bar along y (x extent, z extent).
    pub fn beam(&mut self, a: Vec3, b: Vec3, size_a: Vec2, size_b: Vec2) {
        let Some((side, up)) = bar_frame(a, b) else { return };
        let ring = |center: Vec3, size: Vec2| -> Vec<Vec3> {
            let (s, u) = (side * size.x * 0.5, up * size.y * 0.5);
            vec![center - s - u, center + s - u, center + s + u, center - s + u]
        };
        self.loft(&[ring(a, size_a), ring(b, size_b)], true, true);
    }

    /// Faceted ellipsoid: `rings` latitude bands of `sides` facets.
    pub fn spheroid(&mut self, center: Vec3, radii: Vec3, sides: usize, rings: usize) {
        self.lumpy_spheroid(center, radii, sides, rings, 0.0, 0);
    }

    /// Ellipsoid whose vertices are pushed in and out by up to `roughness`
    /// (fraction of the radius), deterministically from `seed`: rocks, tree crowns.
    pub fn lumpy_spheroid(&mut self, center: Vec3, radii: Vec3, sides: usize, rings: usize, roughness: f32, seed: u32) {
        let rings = rings.max(2);
        let stack: Vec<Vec<Vec3>> = (0..=rings)
            .map(|j| {
                let latitude = -PI * 0.5 + PI * j as f32 / rings as f32;
                (0..sides)
                    .map(|i| {
                        let longitude = (i as f32 + 0.5 * (j % 2) as f32) * TAU / sides as f32;
                        let pole = j == 0 || j == rings;
                        let bump = if pole { 1.0 } else { 1.0 + roughness * (hash_unit(seed, (j * sides + i) as u32) * 2.0 - 1.0) };
                        let dir = Vec3::new(latitude.cos() * longitude.cos(), latitude.cos() * longitude.sin(), latitude.sin());
                        center + dir * radii * bump
                    })
                    .collect()
            })
            .collect();
        self.loft(&stack, true, true);
    }

    // ---- extrusions ------------------------------------------------------

    /// Side profile (x, z) extruded across the body from `y0` to `y1`.
    pub fn extrude_y(&mut self, profile: &[[f32; 2]], y0: f32, y1: f32) {
        let ring = |y: f32| profile.iter().map(|p| Vec3::new(p[0], y, p[1])).collect::<Vec<_>>();
        self.loft(&[ring(y0), ring(y1)], true, true);
    }

    /// Side profile (x, z) extruded symmetrically to `±half_width`, with the
    /// outer `chamfer` metres of each side drawn in toward the profile's
    /// middle: a hull with bevelled flanks. Below full detail the bevel is
    /// dropped and this is a plain extrusion.
    pub fn extrude_y_chamfered(&mut self, profile: &[[f32; 2]], half_width: f32, chamfer: f32) {
        if !self.fine() {
            self.extrude_y(profile, -half_width, half_width);
            return;
        }
        let (min, max) = profile_bounds(profile);
        let (mid, half) = ((min + max) * 0.5, (max - min) * 0.5);
        let inset = Vec2::new((half.x - chamfer).max(0.0) / half.x.max(1e-6), (half.y - chamfer).max(0.0) / half.y.max(1e-6));
        let ring = |y: f32, scale: Vec2| -> Vec<Vec3> {
            profile
                .iter()
                .map(|p| {
                    let q = mid + (Vec2::new(p[0], p[1]) - mid) * scale;
                    Vec3::new(q.x, y, q.y)
                })
                .collect()
        };
        let inner = (half_width - chamfer).max(0.0);
        self.loft(&[ring(-half_width, inset), ring(-inner, Vec2::ONE), ring(inner, Vec2::ONE), ring(half_width, inset)], true, true);
    }

    /// Cross-section (y, z) extruded along the body from `x0` to `x1`.
    pub fn extrude_x(&mut self, profile: &[[f32; 2]], x0: f32, x1: f32) {
        let ring = |x: f32| profile.iter().map(|p| Vec3::new(x, p[0], p[1])).collect::<Vec<_>>();
        self.loft(&[ring(x0), ring(x1)], true, true);
    }

    /// Plan profile (x, y) extruded upward from `z0` to `z1`.
    pub fn extrude_z(&mut self, profile: &[[f32; 2]], z0: f32, z1: f32) {
        self.loft_z(profile, &[Section::new(z0, 1.0), Section::new(z1, 1.0)]);
    }

    /// Plan profile (x, y) swept through scaled and shifted `sections`:
    /// faceted shells with sloped cheeks, glacis plates and tumblehome.
    pub fn loft_z(&mut self, profile: &[[f32; 2]], sections: &[Section]) {
        let rings: Vec<Vec<Vec3>> = sections
            .iter()
            .map(|s| profile.iter().map(|p| (Vec2::new(p[0], p[1]) * s.scale + s.shift).extend(s.z)).collect())
            .collect();
        self.loft(&rings, true, true);
    }

    // ---- core ------------------------------------------------------------

    /// Joins consecutive `rings` (all the same length) with quads and
    /// optionally caps the two ends. Rings may be concave and may collapse to
    /// a point. Orientation is fixed up from the solid's signed volume, caps
    /// included even when they are not emitted.
    pub fn loft(&mut self, rings: &[Vec<Vec3>], cap_start: bool, cap_end: bool) {
        let Some(first) = rings.first() else { return };
        let n = first.len();
        assert!(rings.len() >= 2 && n >= 3 && rings.iter().all(|r| r.len() == n), "loft needs matching rings");
        let rings: Vec<Vec<Vec3>> = rings.iter().map(|r| r.iter().map(|&p| self.transform.transform_point3(p)).collect()).collect();

        // (emitted, outline) for the two caps, then the side quads.
        let mut faces: Vec<(bool, Vec<Vec3>)> = Vec::with_capacity(n * (rings.len() - 1) + 2);
        faces.push((cap_start, rings[0].iter().rev().copied().collect()));
        faces.push((cap_end, rings[rings.len() - 1].clone()));
        for pair in rings.windows(2) {
            for i in 0..n {
                let j = (i + 1) % n;
                faces.push((true, vec![pair[0][i], pair[0][j], pair[1][j], pair[1][i]]));
            }
        }

        let volume: f32 = faces.iter().map(|(_, f)| (1..f.len() - 1).map(|i| f[0].dot(f[i].cross(f[i + 1]))).sum::<f32>()).sum();
        let inside_out = volume < 0.0;
        #[cfg(test)]
        let start = self.mesh.indices.len();
        for (_, mut face) in faces.into_iter().filter(|(emitted, _)| *emitted) {
            if inside_out {
                face.reverse();
            }
            self.emit_face(&face);
        }
        #[cfg(test)]
        if cap_start && cap_end {
            self.solids.push(start..self.mesh.indices.len());
        }
    }

    /// A single one-sided polygon, wound counter-clockwise seen from its
    /// front: stripes and markings laid just above a surface.
    pub fn face(&mut self, points: &[Vec3]) {
        let mut world: Vec<Vec3> = points.iter().map(|&p| self.transform.transform_point3(p)).collect();
        if self.transform.matrix3.determinant() < 0.0 {
            world.reverse();
        }
        self.emit_face(&world);
    }

    /// Upward-facing rectangle at height `center.z`.
    pub fn decal(&mut self, center: Vec3, size: Vec2) {
        self.face(&rect_ring(center.truncate(), size * 0.5, center.z));
    }

    /// Emits one polygon given in final (transformed) space.
    fn emit_face(&mut self, points: &[Vec3]) {
        let mut ring: Vec<Vec3> = Vec::with_capacity(points.len());
        for &p in points {
            if ring.last().is_none_or(|q| q.distance(p) > WELD_DISTANCE) {
                ring.push(p);
            }
        }
        while ring.len() > 1 && ring[0].distance(ring[ring.len() - 1]) <= WELD_DISTANCE {
            ring.pop();
        }
        match ring.len() {
            0..=2 => {}
            3 => self.emit_triangles(&ring, &[[0, 1, 2]]),
            4 => {
                let (n0, n1) = (triangle_normal(ring[0], ring[1], ring[2]), triangle_normal(ring[0], ring[2], ring[3]));
                let planar = n0.length() < 1e-9 || n1.length() < 1e-9 || n0.normalize().dot(n1.normalize()) > 0.9999;
                if planar {
                    self.emit_triangles(&ring, &[[0, 1, 2], [0, 2, 3]]);
                } else {
                    // A twisted quad is two flat facets, each with its own normal.
                    self.emit_triangles(&[ring[0], ring[1], ring[2]], &[[0, 1, 2]]);
                    self.emit_triangles(&[ring[0], ring[2], ring[3]], &[[0, 1, 2]]);
                }
            }
            _ => {
                let triangles = triangulate(&ring);
                self.emit_triangles(&ring, &triangles);
            }
        }
    }

    /// Pushes a planar face: shared flat normal, box-projected UVs.
    fn emit_triangles(&mut self, points: &[Vec3], triangles: &[[usize; 3]]) {
        let Some(normal) = newell_normal(points).try_normalize() else { return };
        let kept: Vec<&[usize; 3]> = triangles
            .iter()
            .filter(|t| {
                let n = triangle_normal(points[t[0]], points[t[1]], points[t[2]]);
                n.length() * 0.5 >= MIN_TRIANGLE_AREA && n.dot(normal) > 0.0
            })
            .collect();
        if kept.is_empty() {
            return;
        }
        let base = self.mesh.vertices.len() as u32;
        let abs = normal.abs();
        for &p in points {
            let uv = if abs.x >= abs.y && abs.x >= abs.z {
                [p.y, p.z]
            } else if abs.y >= abs.z {
                [p.x, p.z]
            } else {
                [p.x, p.y]
            };
            self.mesh.vertices.push(MeshVertex { pos: p.to_array(), normal: normal.to_array(), uv, material: self.material, part: self.part });
        }
        for t in kept {
            self.mesh.indices.extend(t.iter().map(|&i| base + i as u32));
        }
    }

    #[cfg(test)]
    pub fn solids(&self) -> &[std::ops::Range<usize>] {
        &self.solids
    }

    #[cfg(test)]
    pub fn mesh(&self) -> &MeshLod {
        &self.mesh
    }
}

// ---- profile helpers -------------------------------------------------------

/// Rectangle plan (x, y) with its corners cut by `chamfer`.
pub fn chamfered_rect(half: Vec2, chamfer: f32) -> Vec<[f32; 2]> {
    let c = chamfer.min(half.min_element() * 0.95);
    let (x, y) = (half.x, half.y);
    vec![[x, -y + c], [x, y - c], [x - c, y], [-x + c, y], [-x, y - c], [-x, -y + c], [-x + c, -y], [x - c, -y]]
}

/// Regular n-gon plan (x, y) with a flat side facing +x.
pub fn ngon(sides: usize, radius: f32) -> Vec<[f32; 2]> {
    (0..sides)
        .map(|i| {
            let angle = (i as f32 + 0.5) * TAU / sides as f32;
            [angle.cos() * radius, angle.sin() * radius]
        })
        .collect()
}

/// Deterministic hash of (`seed`, `index`) to `0.0..1.0`.
pub fn hash_unit(seed: u32, index: u32) -> f32 {
    let mut h = seed.wrapping_mul(0x9E37_79B9) ^ index.wrapping_mul(0x85EB_CA6B).wrapping_add(0xC2B2_AE35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn frustum_rings(base_center: Vec3, base: Vec2, top: Vec2, height: f32, top_shift: Vec2) -> [Vec<Vec3>; 2] {
    let c = base_center.truncate();
    [rect_ring(c, base * 0.5, base_center.z), rect_ring(c + top_shift, top * 0.5, base_center.z + height)]
}

/// Cross-section axes for a bar from `a` to `b`: `side` stays as close to +y
/// (left) as the bar's direction allows, `up` completes the frame.
fn bar_frame(a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    let axis = (b - a).try_normalize()?;
    let reference = if axis.y.abs() < 0.999 { Vec3::Y } else { Vec3::X };
    let side = (reference - axis * reference.dot(axis)).normalize();
    Some((side, axis.cross(side)))
}

fn rect_ring(center: Vec2, half: Vec2, z: f32) -> Vec<Vec3> {
    vec![
        Vec3::new(center.x - half.x, center.y - half.y, z),
        Vec3::new(center.x + half.x, center.y - half.y, z),
        Vec3::new(center.x + half.x, center.y + half.y, z),
        Vec3::new(center.x - half.x, center.y + half.y, z),
    ]
}

fn ngon_ring(center: Vec2, sides: usize, radius: f32, z: f32) -> Vec<Vec3> {
    ngon(sides, radius).iter().map(|p| Vec3::new(center.x + p[0], center.y + p[1], z)).collect()
}

fn profile_bounds(profile: &[[f32; 2]]) -> (Vec2, Vec2) {
    profile.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| {
        let v = Vec2::new(p[0], p[1]);
        (lo.min(v), hi.max(v))
    })
}

fn triangle_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (b - a).cross(c - a)
}

/// Area-weighted polygon normal; robust for concave and slightly non-planar rings.
fn newell_normal(points: &[Vec3]) -> Vec3 {
    let mut normal = Vec3::ZERO;
    for (i, &p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        normal += Vec3::new((p.y - q.y) * (p.z + q.z), (p.z - q.z) * (p.x + q.x), (p.x - q.x) * (p.y + q.y));
    }
    normal
}

/// Ear-clipping triangulation of a planar, possibly concave polygon.
fn triangulate(points: &[Vec3]) -> Vec<[usize; 3]> {
    let Some(normal) = newell_normal(points).try_normalize() else { return Vec::new() };
    let u = normal.any_orthonormal_vector();
    let v = normal.cross(u);
    let flat: Vec<Vec2> = points.iter().map(|p| Vec2::new(p.dot(u), p.dot(v))).collect();
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - a);

    let mut remaining: Vec<usize> = (0..points.len()).collect();
    let mut triangles = Vec::with_capacity(points.len() - 2);
    while remaining.len() > 3 {
        let m = remaining.len();
        let ear = (0..m).find(|&k| {
            let (a, b, c) = (flat[remaining[(k + m - 1) % m]], flat[remaining[k]], flat[remaining[(k + 1) % m]]);
            if cross(a, b, c) <= 1e-9 {
                return false;
            }
            !remaining.iter().any(|&other| {
                let p = flat[other];
                let corner = p.distance_squared(a) < 1e-10 || p.distance_squared(b) < 1e-10 || p.distance_squared(c) < 1e-10;
                !corner && cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
            })
        });
        // No ear means leftover collinear points: drop one and carry on.
        let k = ear.unwrap_or(0);
        if ear.is_some() {
            triangles.push([remaining[(k + m - 1) % m], remaining[k], remaining[(k + 1) % m]]);
        }
        remaining.remove(k);
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    triangles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_volume(mesh: &MeshLod, range: std::ops::Range<usize>) -> f32 {
        mesh.indices[range]
            .chunks(3)
            .map(|t| {
                let p = |i: u32| Vec3::from(mesh.vertices[i as usize].pos);
                p(t[0]).dot(p(t[1]).cross(p(t[2]))) / 6.0
            })
            .sum()
    }

    fn volume_of(build: impl Fn(&mut MeshBuilder), transform: Affine3A) -> f32 {
        let mut b = MeshBuilder::new(0, transform);
        build(&mut b);
        let mesh = b.finish();
        let n = mesh.indices.len();
        signed_volume(&mesh, 0..n)
    }

    #[test]
    fn primitives_have_expected_outward_volume() {
        let transforms = [
            Affine3A::IDENTITY,
            Affine3A::from_translation(Vec3::new(30.0, -20.0, 5.0)) * Affine3A::from_rotation_y(0.7) * Affine3A::from_rotation_z(2.0),
            Affine3A::from_scale(Vec3::new(1.0, -1.0, 1.0)),
            Affine3A::from_translation(Vec3::new(-9.0, 4.0, 1.0)) * Affine3A::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
        ];
        let l_profile = [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0], [1.0, 2.0], [0.0, 2.0]];
        let clockwise: Vec<[f32; 2]> = l_profile.iter().rev().copied().collect();
        type Case<'a> = (&'a str, Box<dyn Fn(&mut MeshBuilder) + 'a>, f32);
        let cases: Vec<Case> = vec![
            ("cuboid", Box::new(|b| b.cuboid(Vec3::new(1.0, 2.0, 3.0), Vec3::new(2.0, 3.0, 4.0))), 24.0),
            ("frustum", Box::new(|b| b.frustum(Vec3::ZERO, Vec2::new(2.0, 2.0), Vec2::ZERO, 3.0, Vec2::new(0.5, 0.0))), 4.0),
            ("prism", Box::new(|b| b.prism(Vec3::ZERO, 4, 2.0_f32.sqrt(), 2.0_f32.sqrt(), 5.0)), 20.0),
            ("bar", Box::new(|b| b.cylinder_between(Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0), 2.0_f32.sqrt(), 2.0_f32.sqrt(), 4)), 20.0),
            ("concave", Box::new(|b| b.extrude_y(&l_profile, -1.0, 1.0)), 6.0),
            ("clockwise", Box::new(|b| b.extrude_z(&clockwise, 0.0, 2.0)), 6.0),
            ("chamfered", Box::new(|b| b.chamfered_box(Vec3::ZERO, Vec3::new(4.0, 4.0, 1.0), 1.0)), 14.0),
        ];
        for (name, build, expected) in &cases {
            for transform in transforms {
                let volume = volume_of(build, transform);
                assert!((volume - expected).abs() < 1e-3 * expected.max(1.0) + 2e-3, "{name}: volume {volume}, expected {expected}");
            }
        }
    }

    #[test]
    fn open_primitives_face_outward() {
        let mut b = MeshBuilder::new(0, Affine3A::from_scale(Vec3::new(1.0, -1.0, 1.0)));
        b.plate(Vec3::new(5.0, 5.0, 1.0), Vec2::new(2.0, 1.0), 0.2, 0.05);
        b.cuboid_open(Vec3::new(-4.0, 3.0, 2.0), Vec3::ONE);
        b.decal(Vec3::new(0.0, 0.0, 1.0), Vec2::ONE);
        let mesh = b.finish();
        assert!(mesh.vertices.iter().all(|v| v.normal[2] > -1e-6), "no downward faces on open-bottom shapes");
        assert!(mesh.vertices.iter().any(|v| v.normal[2] > 0.99));
        for t in mesh.indices.chunks(3) {
            let p = |i: u32| Vec3::from(mesh.vertices[i as usize].pos);
            let n = triangle_normal(p(t[0]), p(t[1]), p(t[2])).normalize();
            assert!(n.dot(Vec3::from(mesh.vertices[t[0] as usize].normal)) > 0.999);
        }
    }

    #[test]
    fn collapsed_rings_make_clean_cones() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.prism(Vec3::ZERO, 6, 1.0, 0.0, 2.0);
        let mesh = b.finish();
        // Six side triangles and a hexagonal base.
        assert_eq!(mesh.indices.len() / 3, 6 + 4);
    }

    #[test]
    fn brush_state_is_scoped() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.paint(material::GLOW);
        b.with_part(part::TURRET, |b| b.at(Vec3::X * 10.0, |b| b.cuboid(Vec3::ZERO, Vec3::ONE)));
        b.cuboid(Vec3::ZERO, Vec3::ONE);
        let mesh = b.finish();
        let (turret, hull): (Vec<&MeshVertex>, Vec<&MeshVertex>) = mesh.vertices.iter().partition(|v| v.part == part::TURRET);
        assert!(turret.iter().all(|v| v.pos[0] > 9.0 && v.material == material::GLOW));
        assert!(hull.iter().all(|v| v.pos[0] < 1.0 && v.part == part::HULL));
        assert_eq!(turret.len(), hull.len());
    }
}

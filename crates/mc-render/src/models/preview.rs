//! Test-only inspection output: Wavefront OBJ/MTL dumps and a small software
//! rasteriser that draws a model from the RTS camera, so a human can look at
//! the procedural art without the Vulkan renderer.

use std::fmt::Write as _;
use std::path::Path;

use glam::{Vec2, Vec3};

use super::{material, MeshLod, Model};

/// Preview palette (linear-ish RGB) and whether the material is emissive.
pub fn material_color(id: u32) -> ([f32; 3], bool) {
    match id {
        material::PLATING => ([0.93, 0.94, 0.95], false),
        material::ACCENT => ([0.13, 0.14, 0.16], false),
        material::GLOW => ([0.62, 0.9, 1.0], true),
        material::TEAM => ([0.85, 0.1, 0.12], false),
        material::METAL => ([0.42, 0.44, 0.47], false),
        material::GLASS => ([0.1, 0.24, 0.34], false),
        material::TREAD => ([0.07, 0.07, 0.08], false),
        material::GLOW_ORANGE => ([1.0, 0.58, 0.15], true),
        material::BARK => ([0.3, 0.21, 0.14], false),
        material::FOLIAGE => ([0.16, 0.36, 0.14], false),
        material::ROCK => ([0.45, 0.43, 0.4], false),
        material::CONCRETE => ([0.62, 0.61, 0.58], false),
        material::WINDOWS => ([0.95, 0.85, 0.55], true),
        material::GLOW_AMBER => ([1.0, 0.72, 0.12], true),
        material::PLATING_DARK => ([0.2, 0.21, 0.24], false),
        _ => ([1.0, 0.0, 1.0], false),
    }
}

const MATERIAL_NAMES: [&str; 15] = [
    "plating",
    "accent",
    "glow",
    "team",
    "metal",
    "glass",
    "tread",
    "glow_orange",
    "bark",
    "foliage",
    "rock",
    "concrete",
    "windows",
    "glow_amber",
    "plating_dark",
];

/// Shared material library for the OBJ dumps.
pub fn mtl_text() -> String {
    let mut out = String::new();
    for (id, name) in MATERIAL_NAMES.iter().enumerate() {
        let ([r, g, b], emissive) = material_color(id as u32);
        writeln!(out, "newmtl {name}\nKd {r} {g} {b}").unwrap();
        if emissive {
            writeln!(out, "Ke {r} {g} {b}").unwrap();
        }
        out.push('\n');
    }
    out
}

/// One LOD as OBJ text, rotated to the y-up convention of DCC tools:
/// model (x forward, y left, z up) becomes OBJ (x, z, -y).
pub fn obj_text(model: &Model, lod: usize, mtl_file: &str) -> String {
    let mesh = &model.lods[lod];
    let mut out = format!(
        "# {} lod{lod}: {} triangles\nmtllib {mtl_file}\no {}\n",
        model.key,
        mesh.indices.len() / 3,
        model.key
    );
    for v in &mesh.vertices {
        writeln!(out, "v {} {} {}", v.pos[0], v.pos[2], -v.pos[1]).unwrap();
    }
    for v in &mesh.vertices {
        writeln!(out, "vn {} {} {}", v.normal[0], v.normal[2], -v.normal[1]).unwrap();
    }
    for v in &mesh.vertices {
        writeln!(out, "vt {} {}", v.uv[0], v.uv[1]).unwrap();
    }
    for (id, name) in MATERIAL_NAMES.iter().enumerate() {
        let mut header = false;
        for t in mesh
            .indices
            .chunks(3)
            .filter(|t| mesh.vertices[t[0] as usize].material == id as u32)
        {
            if !header {
                writeln!(out, "usemtl {name}").unwrap();
                header = true;
            }
            let [a, b, c] = [t[0] + 1, t[1] + 1, t[2] + 1];
            writeln!(out, "f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}").unwrap();
        }
    }
    out
}

/// RGB8 image.
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[u8; 3]>,
}

impl Image {
    pub fn write_ppm(&self, path: &Path) -> std::io::Result<()> {
        let mut data = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        data.extend(self.pixels.iter().flatten());
        std::fs::write(path, data)
    }
}

/// Orthographic render from the RTS camera: 50 degrees above the horizon,
/// from `azimuth_degrees` around the model (0 looks at its nose), framed to
/// fit. Back faces are culled, so a face wound the wrong way shows as a hole.
pub fn render(mesh: &MeshLod, size: usize, azimuth_degrees: f32) -> Image {
    let (elevation, azimuth) = (50f32.to_radians(), azimuth_degrees.to_radians());
    let to_camera = Vec3::new(
        elevation.cos() * azimuth.cos(),
        elevation.cos() * azimuth.sin(),
        elevation.sin(),
    );
    let right = (-to_camera).cross(Vec3::Z).normalize();
    let up = right.cross(-to_camera);
    let light = Vec3::new(0.35, 0.45, 0.82).normalize();
    let ground = [0.33, 0.37, 0.3];

    // Frame the mesh, then rasterise with 2x2 supersampling.
    let flat = |p: Vec3| Vec2::new(p.dot(right), p.dot(up));
    let (lo, hi) = mesh
        .vertices
        .iter()
        .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), v| {
            (lo.min(flat(v.pos.into())), hi.max(flat(v.pos.into())))
        });
    let n = size * 2;
    let scale = n as f32 * 0.92 / (hi - lo).max_element();
    let middle = (lo + hi) * 0.5;
    let project = |p: Vec3| {
        let q = (flat(p) - middle) * scale;
        Vec2::new(n as f32 * 0.5 + q.x, n as f32 * 0.5 - q.y)
    };
    let mut color = vec![ground; n * n];
    let mut depth = vec![f32::MIN; n * n];

    for t in mesh.indices.chunks(3) {
        let v = |i: u32| &mesh.vertices[i as usize];
        let p = [
            Vec3::from(v(t[0]).pos),
            Vec3::from(v(t[1]).pos),
            Vec3::from(v(t[2]).pos),
        ];
        if (p[1] - p[0]).cross(p[2] - p[0]).dot(to_camera) <= 0.0 {
            continue;
        }
        let normal = Vec3::from(v(t[0]).normal);
        let (base, emissive) = material_color(v(t[0]).material);
        let shade = if emissive {
            1.0
        } else {
            0.32 + 0.68 * normal.dot(light).max(0.0)
        };
        let rgb = base.map(|c| c * shade);
        let s = p.map(project);
        let z = p.map(|q| q.dot(to_camera));
        let area = (s[1] - s[0]).perp_dot(s[2] - s[0]);
        if area.abs() < 1e-6 {
            continue;
        }
        let (min, max) = (
            s[0].min(s[1]).min(s[2]).floor(),
            s[0].max(s[1]).max(s[2]).ceil(),
        );
        for y in (min.y.max(0.0) as usize)..(max.y.min(n as f32 - 1.0) as usize + 1) {
            for x in (min.x.max(0.0) as usize)..(max.x.min(n as f32 - 1.0) as usize + 1) {
                let q = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = (s[1] - q).perp_dot(s[2] - q) / area;
                let w1 = (s[2] - q).perp_dot(s[0] - q) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let d = w0 * z[0] + w1 * z[1] + w2 * z[2];
                if d > depth[y * n + x] {
                    depth[y * n + x] = d;
                    color[y * n + x] = rgb;
                }
            }
        }
    }

    let pixels = (0..size * size)
        .map(|i| {
            let (x, y) = (i % size * 2, i / size * 2);
            let sum = [0, 1, n, n + 1].iter().fold([0.0; 3], |acc, o| {
                let c = color[y * n + x + o];
                [acc[0] + c[0], acc[1] + c[1], acc[2] + c[2]]
            });
            sum.map(|c| ((c * 0.25).clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8)
        })
        .collect();
    Image {
        width: size,
        height: size,
        pixels,
    }
}

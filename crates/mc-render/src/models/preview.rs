//! Test-only inspection output: Wavefront OBJ/MTL dumps and the thumbnail
//! rasteriser's view of a model from the RTS camera on a ground colour, so a
//! human can look at the procedural art without the Vulkan renderer.

use std::fmt::Write as _;
use std::path::Path;

use super::thumbnail::{material_color, rasterise};
use super::{material, MeshLod, Model};

const MATERIAL_NAMES: [&str; 16] = [
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
    "glow_red",
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
    let ground = [0.33, 0.37, 0.3];
    let samples = rasterise(mesh, size, azimuth_degrees, material_color(material::TEAM).0);
    let n = samples.n;
    let ss = n / size;
    let pixels = (0..size * size)
        .map(|i| {
            let (x, y) = (i % size * ss, i / size * ss);
            let mut sum = [0.0; 3];
            for oy in 0..ss {
                for ox in 0..ss {
                    let at = (y + oy) * n + x + ox;
                    let c = if samples.covered[at] { samples.color[at] } else { ground };
                    sum = [sum[0] + c[0], sum[1] + c[1], sum[2] + c[2]];
                }
            }
            let k = 1.0 / (ss * ss) as f32;
            sum.map(|c| ((c * k).clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8)
        })
        .collect();
    Image {
        width: size,
        height: size,
        pixels,
    }
}

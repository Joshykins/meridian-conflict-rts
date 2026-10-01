//! What the website (`site/`) shows of a unit: its mesh as it stands at rest, for the
//! page's own viewer to draw, and a portrait for the directory's tiles. The `mc-site`
//! program writes both for every unit the site's build asks for, so the pages follow
//! the models with no step by hand.
//!
//! The mesh is the model's full detail level, fitted to the blueprint as the renderer
//! fits it, with only the pieces a finished, unrefitted unit on land shows. The vertex
//! shader's animation is left out: it stands in its bind pose, as the interface's
//! portraits do.

use std::fmt;

use super::thumbnail::{finish, framed, portrait, Shot};
use super::{build_model_fitted, material, part, rig, wall, MeshLod, MeshVertex, Model};

/// A faction's paint, as `faction.ron` and `entity.wgsl` `material_of` give it (linear RGB).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub plating: [f32; 3],
    pub accent: [f32; 3],
    pub glow: [f32; 3],
    pub shield: [f32; 3],
    pub team: [f32; 3],
    /// Bare metal is the Regency's dark bronze (`regency.wgsl`), not gunmetal.
    pub bronze: bool,
}

/// One unit to write out.
#[derive(Clone, Debug, PartialEq)]
pub struct Ask {
    pub mesh: String,
    /// The blueprint's radius, height and tech ([`build_model_fitted`]).
    pub radius: f32,
    pub height: f32,
    pub tech: u8,
    pub paint: Paint,
}

#[derive(Debug, PartialEq)]
pub enum SiteError {
    /// The catalogue has no model under this mesh key.
    NoModel(String),
    /// More than a mesh file's counts hold.
    TooBig(String),
    /// A line of the request that does not read: its number (from one) and why.
    Request(usize, String),
}

impl fmt::Display for SiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SiteError::NoModel(mesh) => write!(f, "no model is made for mesh {mesh:?}"),
            SiteError::TooBig(mesh) => write!(f, "mesh {mesh:?} is too big for a mesh file"),
            SiteError::Request(line, why) => write!(f, "request line {line}: {why}"),
        }
    }
}

impl std::error::Error for SiteError {}

/// Reads a request: the units to write out, each under the file name it is asked for by.
/// One thing a line, its fields parted by tabs, colours as `r,g,b` in linear RGB:
///
/// - `paint NAME PLATING ACCENT GLOW SHIELD TEAM BRONZE` names a faction's paint
///   (`BRONZE` 1 or 0);
/// - `unit NAME MESH RADIUS HEIGHT TECH PAINT` asks for a unit in a paint named above it.
pub fn parse_request(text: &str) -> Result<Vec<(String, Ask)>, SiteError> {
    let mut paints: Vec<(&str, Paint)> = Vec::new();
    let mut asks = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let bad = |why: &str| SiteError::Request(n + 1, why.to_owned());
        let fields: Vec<&str> = line.split('\t').collect();
        let colour = |s: &str| {
            let c: Vec<f32> = s.split(',').filter_map(|c| c.trim().parse().ok()).collect();
            <[f32; 3]>::try_from(c).map_err(|_| bad("a colour is r,g,b"))
        };
        match fields.as_slice() {
            [""] => {}
            ["paint", name, plating, accent, glow, shield, team, bronze] => paints.push((
                name,
                Paint {
                    plating: colour(plating)?,
                    accent: colour(accent)?,
                    glow: colour(glow)?,
                    shield: colour(shield)?,
                    team: colour(team)?,
                    bronze: *bronze == "1",
                },
            )),
            ["unit", name, mesh, radius, height, tech, paint] => {
                // The name becomes a file's: nothing in it may climb out of the folder.
                if name.is_empty()
                    || !name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
                    || name.starts_with('.')
                {
                    return Err(bad("a unit's name is letters, digits, - _ and ."));
                }
                let size = |s: &str| {
                    s.parse::<f32>()
                        .ok()
                        .filter(|v| v.is_finite() && *v > 0.0)
                        .ok_or_else(|| bad("a radius and a height are above zero"))
                };
                asks.push((
                    (*name).to_owned(),
                    Ask {
                        mesh: (*mesh).to_owned(),
                        radius: size(radius)?,
                        height: size(height)?,
                        tech: tech.parse().map_err(|_| bad("a tech is a number"))?,
                        paint: paints
                            .iter()
                            .find(|(p, _)| p == paint)
                            .ok_or_else(|| bad("no paint of that name above"))?
                            .1,
                    },
                ));
            }
            _ => return Err(bad("neither a paint nor a unit")),
        }
    }
    Ok(asks)
}

/// A unit as it stands at rest: its mesh file, and the mesh to take its portrait from.
pub struct Rest {
    /// The mesh file ([`encode`]).
    pub file: Vec<u8>,
    pub triangles: usize,
    mesh: MeshLod,
}

/// A structure standing in the sea shows its stilts in place of its footing; a portrait
/// and the site show it ashore.
fn shown(v: &MeshVertex) -> bool {
    v.rig & rig::UPGRADE == 0
        && v.rig & rig::MODULE_MASK == 0
        && v.part != part::AFLOAT
        && wall::shown(v.part, wall::PORTRAIT)
}

/// Only the triangles a unit at rest shows, and only their vertices.
fn at_rest(mesh: &MeshLod) -> MeshLod {
    let mut slot = vec![u32::MAX; mesh.vertices.len()];
    let mut out = MeshLod::default();
    for t in mesh.indices.as_chunks::<3>().0 {
        if !shown(&mesh.vertices[t[0] as usize]) {
            continue;
        }
        for &i in t {
            let at = &mut slot[i as usize];
            if *at == u32::MAX {
                *at = out.vertices.len() as u32;
                out.vertices.push(mesh.vertices[i as usize]);
            }
            out.indices.push(*at);
        }
    }
    out
}

/// `entity.wgsl` `material_of`, as flat paint: a material's colour and whether it gives
/// off light. A portrait is lit brighter than the game's sun leaves a unit, so the paints
/// are the shader's albedos as they are.
fn paint_of(paint: &Paint) -> impl Fn(u32) -> ([f32; 3], bool) + '_ {
    move |id| match id {
        material::PLATING => (paint.plating, false),
        material::ACCENT => (paint.accent, false),
        material::GLOW => (paint.glow, true),
        material::TEAM => (paint.team.map(|c| c * 0.45), false),
        material::METAL if paint.bronze => ([0.3, 0.19, 0.095], false),
        material::METAL => ([0.32, 0.33, 0.36], false),
        material::GLASS => ([0.02, 0.05, 0.09], false),
        material::TREAD => ([0.03, 0.03, 0.035], false),
        material::PLATING_DARK => (
            [
                paint.plating[0] * 0.3,
                paint.plating[1] * 0.33,
                paint.plating[2] * 0.39,
            ],
            false,
        ),
        material::GLOW_SHIELD => (paint.shield, true),
        id => super::thumbnail::material_color(id),
    }
}

/// Where the portrait's camera stands: off the unit's left bow, a little above it.
const PORTRAIT_AZIMUTH: f32 = 35.0;
const PORTRAIT_ELEVATION: f32 = 24.0;

/// The mesh file's magic number: "MCM2".
const MAGIC: [u8; 4] = *b"MCM2";
/// Bytes before the materials.
const HEADER: usize = 72;
/// Bytes a material takes, and how many there are: every id up to `material::LAST`.
const MATERIAL: usize = 16;
const MATERIALS: usize = material::LAST as usize + 1;
/// Bytes a vertex takes.
const VERTEX: usize = 20;

/// A material's flag bits in the mesh file.
mod flag {
    /// Gives off light.
    pub(super) const EMISSIVE: u8 = 1;
    /// Armour: seams run round each face's frame.
    pub(super) const FRAMED: u8 = 2;
    /// The owner's colour: the viewer may repaint it.
    pub(super) const TEAM: u8 = 4;
}

/// The mesh file the site's viewer reads (`site/src/lib/mesh.ts`), little-endian. It
/// carries its own paint, so the viewer knows no material by number:
///
/// - `MAGIC`, the vertex count, the index count and the material count (u32 each);
/// - the bounds' low and high corners (3 f32 each), metres, model space (x forward,
///   y left, z up, the origin on the ground under the middle);
/// - the face frames' scale: metres to one unit of their i16s (f32);
/// - the turret's and the spinner's pivots (3 f32 each), and 1 (u32) when the spinner
///   looks about rather than turning round;
/// - per material, by id: its colour (3 f32, linear RGB), then a byte each of its
///   [`flag`] bits, its roughness and how metallic it is (255 to one), and a spare;
/// - per vertex: its place in the bounds (3 u16, 0 the low corner and 65535 the high),
///   its material and its panel's tone (u8 each); its normal (3 i8, 127 to one) and its
///   part (u8: 0 hull, 1 turret, 2 spinner); its face frame (4 i16: where it sits on its
///   face and the face's half size, `MeshVertex::face`);
/// - the indices (u32 each).
fn encode(mesh: &MeshLod, model: &Model, paint: &Paint) -> Option<Vec<u8>> {
    let (lo, hi) =
        mesh.vertices
            .iter()
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(mut lo, mut hi), v| {
                for k in 0..3 {
                    lo[k] = lo[k].min(v.pos[k]);
                    hi[k] = hi[k].max(v.pos[k]);
                }
                (lo, hi)
            });
    let face_reach = mesh
        .vertices
        .iter()
        .flat_map(|v| v.face)
        .fold(0.0f32, |m, f| m.max(f.abs()));
    let face_scale = (face_reach / i16::MAX as f32).max(1e-6);
    let mut out = Vec::with_capacity(
        HEADER + MATERIALS * MATERIAL + mesh.vertices.len() * VERTEX + mesh.indices.len() * 4,
    );
    out.extend(MAGIC);
    out.extend(u32::try_from(mesh.vertices.len()).ok()?.to_le_bytes());
    out.extend(u32::try_from(mesh.indices.len()).ok()?.to_le_bytes());
    out.extend((MATERIALS as u32).to_le_bytes());
    for f in lo
        .into_iter()
        .chain(hi)
        .chain([face_scale])
        .chain(model.turret_pivot)
        .chain(model.spinner_pivot)
    {
        out.extend(f.to_le_bytes());
    }
    out.extend(u32::from(model.spinner_scans).to_le_bytes());
    debug_assert_eq!(out.len(), HEADER);
    let unit = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
    for id in 0..MATERIALS as u32 {
        let (colour, emissive) = paint_of(paint)(id);
        let (rough, metal) = finish(id);
        for c in colour {
            out.extend(c.to_le_bytes());
        }
        out.push(
            if emissive { flag::EMISSIVE } else { 0 }
                | if framed(id) { flag::FRAMED } else { 0 }
                | if id == material::TEAM { flag::TEAM } else { 0 },
        );
        out.extend([unit(rough), unit(metal), 0]);
    }
    for v in &mesh.vertices {
        for k in 0..3 {
            let span = (hi[k] - lo[k]).max(1e-6);
            let q = ((v.pos[k] - lo[k]) / span * 65535.0)
                .round()
                .clamp(0.0, 65535.0) as u16;
            out.extend(q.to_le_bytes());
        }
        out.push(v.material.min(MATERIALS as u32 - 1) as u8);
        out.push(((v.surface >> 8) & 0xFF) as u8);
        for n in v.normal {
            out.push((n.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8);
        }
        out.push(match v.part {
            part::TURRET => 1,
            part::SPINNER => 2,
            _ => 0,
        });
        for f in v.face {
            out.extend(((f / face_scale).round() as i16).to_le_bytes());
        }
    }
    for i in &mesh.indices {
        out.extend(i.to_le_bytes());
    }
    Some(out)
}

/// `ask`'s unit at rest.
pub fn rest(ask: &Ask) -> Result<Rest, SiteError> {
    let model = build_model_fitted(&ask.mesh, ask.radius, ask.height, ask.tech, &[])
        .ok_or_else(|| SiteError::NoModel(ask.mesh.clone()))?;
    let mesh = at_rest(&model.lods[0]);
    let file =
        encode(&mesh, &model, &ask.paint).ok_or_else(|| SiteError::TooBig(ask.mesh.clone()))?;
    Ok(Rest {
        file,
        triangles: mesh.indices.len() / 3,
        mesh,
    })
}

impl Rest {
    /// Its portrait in `paint`: RGBA8, sRGB, straight alpha, `size` pixels square, cut out
    /// on a transparent background. Most of the exporter's time: a second or so of one
    /// core at 512 pixels.
    pub fn portrait(&self, paint: &Paint, size: usize) -> Vec<u8> {
        let shot = Shot {
            azimuth_degrees: PORTRAIT_AZIMUTH,
            elevation_degrees: PORTRAIT_ELEVATION,
            paint: &paint_of(paint),
        };
        portrait(&self.mesh, size, &shot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARC: Paint = Paint {
        plating: [0.16, 0.185, 0.225],
        accent: [0.072, 0.082, 0.1],
        glow: [0.55, 0.85, 1.0],
        shield: [0.74, 0.85, 1.0],
        team: [0.05, 0.35, 1.0],
        bronze: false,
    };

    fn tank() -> Ask {
        Ask {
            mesh: "tank_light".to_owned(),
            radius: 4.6,
            height: 3.2,
            tech: 1,
            paint: ARC,
        }
    }

    /// The file is the header, the vertices and the indices, and every index is a vertex.
    #[test]
    fn a_mesh_file_holds_what_its_header_says() {
        let rest = rest(&tank()).unwrap();
        let f = &rest.file;
        assert_eq!(f[..4], MAGIC);
        let count = |at: usize| u32::from_le_bytes(f[at..at + 4].try_into().unwrap()) as usize;
        let (vertices, indices) = (count(4), count(8));
        assert_eq!(indices, rest.triangles * 3);
        assert_eq!(count(12), MATERIALS);
        let first_vertex = HEADER + MATERIALS * MATERIAL;
        assert_eq!(f.len(), first_vertex + vertices * VERTEX + indices * 4);
        let first = first_vertex + vertices * VERTEX;
        assert!((0..indices).all(|i| count(first + i * 4) < vertices));
        // The bounds hold a tank of that radius, standing on the ground.
        let bound = |k: usize| f32::from_le_bytes(f[16 + k * 4..20 + k * 4].try_into().unwrap());
        assert!(bound(2) > -1.0 && bound(5) > 1.0 && bound(3) > 2.0 && bound(3) < 12.0);
        // The team stripe is the owner's colour, flagged so the viewer may repaint it.
        let team = HEADER + material::TEAM as usize * MATERIAL;
        assert_eq!(f[team + 12], flag::TEAM | flag::FRAMED);
        let glow = HEADER + material::GLOW as usize * MATERIAL;
        assert_eq!(f[glow + 12], flag::EMISSIVE);
    }

    /// A portrait is cut out: transparent corners, a solid middle.
    #[test]
    fn a_portrait_is_cut_out() {
        let portrait = rest(&tank()).unwrap().portrait(&ARC, 96);
        assert_eq!(portrait.len(), 96 * 96 * 4);
        assert_eq!(portrait[3], 0);
        let solid = portrait.as_chunks::<4>().0.iter().filter(|p| p[3] == 255);
        assert!(solid.count() > 96 * 96 / 20);
    }

    /// What an upgrade or a refit adds is left off, and nothing else is.
    #[test]
    fn a_unit_at_rest_shows_no_upgrade_pieces() {
        let model = build_model_fitted("commander", 10.4, 24.0, 1, &["eng_2"]).unwrap();
        let mesh = at_rest(&model.lods[0]);
        assert!(!mesh.vertices.is_empty());
        assert!(mesh.vertices.len() < model.lods[0].vertices.len());
        assert!(mesh.vertices.iter().all(shown));
    }

    #[test]
    fn a_request_reads_paints_then_units() {
        let text = "paint\tarc\t0.16,0.185,0.225\t0.072,0.082,0.1\t0.55,0.85,1\t0.74,0.85,1\t0.05,0.35,1\t0\n\
                    unit\tt1-tank.arc\ttank_light\t4.6\t3.2\t1\tarc\n";
        assert_eq!(
            parse_request(text).unwrap(),
            vec![("t1-tank.arc".to_owned(), tank())]
        );
        for (bad, line) in [
            ("unit\t../x\ttank_light\t4.6\t3.2\t1\tarc", 1),
            ("unit\tx\ttank_light\t4.6\t3.2\t1\tarc", 1),
            ("paint\tarc\t1,1\t1,1,1\t1,1,1\t1,1,1\t1,1,1\t0", 1),
            ("\nship\tx", 2),
        ] {
            assert!(
                matches!(parse_request(bad), Err(SiteError::Request(n, _)) if n == line),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn an_unknown_mesh_is_an_error() {
        let ask = Ask {
            mesh: "no_such_mesh".to_owned(),
            ..tank()
        };
        assert_eq!(
            rest(&ask).err(),
            Some(SiteError::NoModel("no_such_mesh".to_owned()))
        );
    }
}

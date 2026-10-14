//! Stable authoring identities. Array positions and runtime unit handles are
//! deliberately absent from mission references; scripts address names.
mod geometry;
mod loaded;
use crate::{MapError, MapInfo};
pub use geometry::{prop_parts, PropPart};
pub(crate) use loaded::Loaded;
use mc_core::{Angle, Fx, FxVec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

const MAX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ITEMS: usize = 16_384;
pub const PLACED_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placed {
    pub version: u32,
    pub next_id: u64,
    pub units: Vec<PlacedUnit>,
    pub regions: Vec<Region>,
    pub points: Vec<Point>,
    pub cameras: Vec<Camera>,
    pub doodads: Vec<Doodad>,
}

impl Default for Placed {
    fn default() -> Self {
        Self {
            version: PLACED_VERSION,
            next_id: 1,
            units: vec![],
            regions: vec![],
            points: vec![],
            cameras: vec![],
            doodads: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacedUnit {
    pub id: u64,
    pub name: String,
    pub group: String,
    pub blueprint: String,
    pub owner: u8,
    pub at: FxVec2,
    pub heading: Angle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub id: u64,
    pub name: String,
    pub min: FxVec2,
    pub max: FxVec2,
}

impl Region {
    pub fn contains(&self, p: FxVec2) -> bool {
        p.x >= self.min.x && p.y >= self.min.y && p.x <= self.max.x && p.y <= self.max.y
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub id: u64,
    pub name: String,
    pub at: FxVec2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub id: u64,
    pub name: String,
    pub at: FxVec2,
    pub distance: u32,
    pub yaw: Angle,
    #[serde(default)]
    pub pose: Option<CameraPose>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraPose {
    pub height: Fx,
    /// Degrees below the horizon, in hundredths. Integer authoring metadata
    /// remains safe to carry through mission snapshots.
    pub pitch_centideg: i16,
    pub free: bool,
}

/// Additional properties for a baked prop. The editor remaps prop_index when
/// the writer sorts the terrain's prop table; id and name remain unchanged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Doodad {
    pub id: u64,
    pub name: String,
    pub prop_index: u32,
    pub tint: [u8; 3],
    pub blocking: bool,
    pub health: u32,
    pub reclaim: u32,
    #[serde(default)]
    pub footprint: Option<Footprint>,
}

/// A prop-local box in metres at scale one. Heading and scale follow its prop.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footprint {
    pub half: FxVec2,
    pub height: Fx,
}

impl Placed {
    pub fn read(path: &Path, info: &MapInfo, prop_count: usize) -> Result<Self, MapError> {
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() > MAX_BYTES {
            return Err(MapError::Invalid("placed.ron exceeds 8 MiB".into()));
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(MapError::Invalid("placed.ron exceeds 8 MiB".into()));
        }
        let placed: Self = ron::de::from_bytes(&bytes)
            .map_err(|e| MapError::Invalid(format!("placed.ron: {e}")))?;
        placed.validate(info, prop_count)?;
        Ok(placed)
    }

    pub fn validate(&self, info: &MapInfo, prop_count: usize) -> Result<(), MapError> {
        let bad = |s: &str| MapError::Invalid(format!("placed.ron: {s}"));
        if self.version != PLACED_VERSION {
            return Err(bad("unsupported version"));
        }
        let total = self.units.len()
            + self.regions.len()
            + self.points.len()
            + self.cameras.len()
            + self.doodads.len();
        if self.next_id == 0 || self.next_id > u64::MAX - MAX_ITEMS as u64 {
            return Err(bad("persistent id space exhausted"));
        }
        if total > MAX_ITEMS {
            return Err(bad("too many named objects"));
        }
        let size = info.size_metres();
        let inside =
            |p: FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size.x && p.y <= size.y;
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut identity = |kind: u8, id, name: &str| -> Result<(), MapError> {
            if id == 0 || id >= self.next_id || !ids.insert(id) {
                return Err(bad("duplicate or invalid persistent id"));
            }
            if name.is_empty()
                || name.len() > 128
                || name.contains('\0')
                || !names.insert((kind, name.to_owned()))
            {
                return Err(bad("names must be nonempty, unique and at most 128 bytes"));
            }
            Ok(())
        };
        for u in &self.units {
            identity(0, u.id, &u.name)?;
            if !inside(u.at)
                || u.owner as usize >= mc_core::MAX_PLAYERS
                || u.blueprint.is_empty()
                || u.blueprint.len() > 128
                || u.group.len() > 128
            {
                return Err(bad("invalid unit placement"));
            }
        }
        for r in &self.regions {
            identity(1, r.id, &r.name)?;
            if !inside(r.min) || !inside(r.max) || r.min.x >= r.max.x || r.min.y >= r.max.y {
                return Err(bad("invalid region bounds"));
            }
        }
        for p in &self.points {
            identity(2, p.id, &p.name)?;
            if !inside(p.at) {
                return Err(bad("point outside map"));
            }
        }
        for c in &self.cameras {
            identity(3, c.id, &c.name)?;
            if !inside(c.at)
                || !(25..=300_000).contains(&c.distance)
                || c.pose.as_ref().is_some_and(|p| {
                    p.height < Fx::from_int(-10_000)
                        || p.height > Fx::from_int(300_000)
                        || !(-8950..=8950).contains(&p.pitch_centideg)
                })
            {
                return Err(bad("invalid camera"));
            }
        }
        let mut props = BTreeSet::new();
        for d in &self.doodads {
            identity(4, d.id, &d.name)?;
            if d.health > 1_000_000
                || d.reclaim > 1_000_000
                || d.footprint.as_ref().is_some_and(|f| {
                    f.half.x <= Fx::ZERO
                        || f.half.y <= Fx::ZERO
                        || f.height <= Fx::ZERO
                        || f.half.x > Fx::from_int(256)
                        || f.half.y > Fx::from_int(256)
                        || f.height > Fx::from_int(1024)
                })
            {
                return Err(bad(
                    "doodad health, reclaim or footprint exceeds its limits",
                ));
            }
            if d.prop_index as usize >= prop_count || !props.insert(d.prop_index) {
                return Err(bad("invalid or repeated doodad reference"));
            }
        }
        Ok(())
    }

    pub fn write(&self, path: &Path, info: &MapInfo, prop_count: usize) -> Result<(), MapError> {
        self.validate(info, prop_count)?;
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| MapError::Invalid(e.to_string()))?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

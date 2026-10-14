//! The authoring document and undoable commands shared by the UI and mailbox.
mod objects;
mod properties;
mod stamp;
mod terrain;
use mc_core::{Angle, Fx, FxVec2};
use mc_map::edit::EditMap;
use mc_map::placed::{Placed, PlacedUnit, Region};
use mc_map::{MapFile, CELL_SIZE_M};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Brush {
    Raise,
    Lower,
    Flatten,
    Smooth,
    Cliff,
    Noise,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Command {
    Terrain {
        at: [f32; 2],
        radius: f32,
        strength: f32,
        brush: Brush,
    },
    Ramp {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
    },
    Stamp {
        at: [f32; 2],
        radius: f32,
        strength: f32,
        asset: String,
    },
    Unit {
        name: String,
        group: String,
        blueprint: String,
        owner: u8,
        at: [f32; 2],
        heading: f32,
    },
    Region {
        name: String,
        min: [f32; 2],
        max: [f32; 2],
    },
    Point {
        name: String,
        at: [f32; 2],
    },
    Camera {
        name: String,
        at: [f32; 2],
        distance: u32,
        yaw: f32,
    },
    Transform {
        id: u64,
        at: [f32; 2],
        heading: f32,
    },
    Properties {
        id: u64,
        name: String,
        group: Option<String>,
        owner: Option<u8>,
        blueprint: Option<String>,
    },
    Delete {
        id: u64,
    },
}

#[derive(Default)]
pub(super) struct Update {
    pub terrain: Option<[u32; 4]>,
    pub objects: bool,
}

struct Change {
    heights: Vec<(u32, u32, u16, u16)>,
    placed: Option<(Placed, Placed)>,
}

pub(super) struct Document {
    pub map: EditMap,
    pub placed: Placed,
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub dirty: bool,
    undo: Vec<Change>,
    redo: Vec<Change>,
}

fn point(p: [f32; 2]) -> Result<FxVec2, String> {
    if p.iter().any(|v| !v.is_finite() || v.abs() > 100_000.0) {
        return Err("Position must contain finite map coordinates".into());
    }
    Ok(FxVec2::new(Fx::from_f32(p[0]), Fx::from_f32(p[1])))
}

impl Document {
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = MapFile::open(path).map_err(|e| e.to_string())?;
        let map = EditMap::load(&file).map_err(|e| e.to_string())?;
        let folder = path.file_name().is_some_and(|p| p == "terrain.mcmap");
        let sidecar = path.with_file_name("placed.ron");
        let placed = if folder && sidecar.exists() {
            Placed::read(&sidecar, &map.info, map.props.len()).map_err(|e| e.to_string())?
        } else {
            Placed::default()
        };
        Ok(Self {
            map,
            placed,
            source: path.to_owned(),
            destination: folder.then(|| path.parent().unwrap().to_owned()),
            dirty: false,
            undo: vec![],
            redo: vec![],
        })
    }

    /// Validate a whole request before committing it. Several actions form one
    /// undo step. Height changes only retain touched samples.
    pub fn apply(
        &mut self,
        commands: &[Command],
        blueprints: &mc_data::Blueprints,
    ) -> Result<Update, String> {
        if commands.is_empty() || commands.len() > 512 {
            return Err("A batch needs 1 to 512 actions".into());
        }
        let mut placed = self.placed.clone();
        let mut patches = BTreeMap::new();
        for command in commands {
            match command {
                Command::Terrain {
                    at,
                    radius,
                    strength,
                    brush,
                } => {
                    self.brush(*at, *radius, *strength, *brush, &mut patches)?;
                }
                Command::Ramp { from, to, width } => self.ramp(*from, *to, *width, &mut patches)?,
                Command::Stamp {
                    at,
                    radius,
                    strength,
                    asset,
                } => self.stamp(*at, *radius, *strength, asset, &mut patches)?,
                _ => objects::apply(command, &mut placed, blueprints)?,
            }
        }
        self.commit(placed, patches)
    }

    fn commit(
        &mut self,
        placed: Placed,
        patches: BTreeMap<(u32, u32), u16>,
    ) -> Result<Update, String> {
        placed
            .validate(&self.map.info, self.map.props.len())
            .map_err(|e| e.to_string())?;
        let mut change = Change {
            heights: vec![],
            placed: (placed != self.placed).then(|| (self.placed.clone(), placed.clone())),
        };
        let width = self.map.info.size_cells().0 + 1;
        for ((x, y), value) in patches {
            let old = self.map.samples()[(y * width + x) as usize];
            if old != value {
                change.heights.push((x, y, old, value));
            }
        }
        let rect = self.restore(&change, true)?;
        self.placed = placed;
        if change.placed.is_some() || !change.heights.is_empty() {
            self.undo.push(change);
            self.redo.clear();
            self.dirty = true;
        }
        Ok(rect)
    }

    fn restore(&mut self, change: &Change, forward: bool) -> Result<Update, String> {
        let mut rect: Option<[u32; 4]> = None;
        for &(x, y, old, new) in &change.heights {
            self.map
                .set_sample(x, y, if forward { new } else { old })
                .map_err(|e| e.to_string())?;
            rect = Some(match rect {
                None => [x, y, x, y],
                Some([a, b, c, d]) => [a.min(x), b.min(y), c.max(x), d.max(y)],
            });
        }
        if let Some((old, new)) = &change.placed {
            self.placed = if forward { new.clone() } else { old.clone() };
        }
        Ok(Update {
            terrain: rect,
            objects: change.placed.is_some(),
        })
    }

    pub fn undo(&mut self) -> Result<Update, String> {
        let Some(change) = self.undo.pop() else {
            return Ok(Update::default());
        };
        let rect = self.restore(&change, false)?;
        self.redo.push(change);
        self.dirty = true;
        Ok(rect)
    }
    pub fn redo(&mut self) -> Result<Update, String> {
        let Some(change) = self.redo.pop() else {
            return Ok(Update::default());
        };
        let rect = self.restore(&change, true)?;
        self.undo.push(change);
        self.dirty = true;
        Ok(rect)
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn save(&mut self, folder: &Path) -> Result<(), String> {
        if folder.exists() && self.destination.as_deref() != Some(folder) {
            return Err("That map folder already exists. Choose a new Save As name.".into());
        }
        self.export(folder)?;
        self.destination = Some(folder.to_owned());
        self.dirty = false;
        Ok(())
    }

    pub fn export(&self, folder: &Path) -> Result<(), String> {
        super::save::export(self, folder)
    }
}

#[cfg(test)]
mod tests;

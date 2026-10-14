//! The authoring document and undoable commands shared by the UI and mailbox.
mod bounds;
mod doodads;
mod environment;
mod objects;
mod paint;
pub(super) use paint::Paint;
mod history;
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
    Bounds {
        origin: [i32; 2],
        tiles: [u32; 2],
        fill: f32,
    },
    Players {
        slots: Vec<mc_data::map_settings::MapPlayer>,
    },
    Mission {
        key: Option<String>,
    },
    Doodad {
        name: String,
        kind: String,
        at: [f32; 2],
        heading: f32,
        scale: f32,
        tint: [u8; 3],
        blocking: bool,
        health: u32,
        reclaim: u32,
        footprint: Option<[f32; 3]>,
    },
    Water {
        level: f32,
    },
    Time {
        hour: f32,
    },
    Climate {
        region: usize,
        value: mc_data::weather::Climate,
    },
    Weather {
        region: usize,
        preset: mc_data::weather::WeatherPreset,
    },
    ClimateRegions {
        regions: Vec<mc_data::regions::Region>,
        walls: Vec<mc_data::regions::Wall>,
        blend: f32,
    },
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
    Paint {
        at: [f32; 2],
        radius: f32,
        paint: Paint,
        value: u8,
        #[serde(default)]
        heading: f32,
    },
    Road {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        kind: u8,
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
        height: Option<f32>,
        pitch: Option<f32>,
        #[serde(default)]
        free: bool,
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
    pub reframe: Option<[f32; 2]>,
    pub objects: bool,
    pub props: bool,
    pub environment: bool,
    pub water: bool,
    pub cover: Vec<(mc_map::edit::Layer, [u32; 4])>,
}

#[derive(Clone)]
struct Change {
    bounds: Option<std::sync::Arc<bounds::Change>>,
    props: Option<(doodads::State, doodads::State)>,
    children: Vec<Change>,
    heights: Vec<(u32, u32, u16, u16)>,
    placed: Option<(Placed, Placed)>,
    layers: Vec<paint::Change>,
    environment: Option<(environment::State, environment::State)>,
}

#[derive(Clone)]
pub(super) struct Document {
    pub map: EditMap,
    pub placed: Placed,
    pub config: mc_data::weather::MapConfig,
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub dirty: bool,
    undo: Vec<Change>,
    redo: Vec<Change>,
    stroke: Option<Vec<Change>>,
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
        let placed = file.placed().clone();
        Ok(Self {
            map,
            placed,
            config: mc_data::weather::MapConfig::for_map(path)?,
            source: path.to_owned(),
            destination: folder.then(|| path.parent().unwrap().to_owned()),
            dirty: false,
            undo: vec![],
            redo: vec![],
            stroke: None,
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
        if commands.iter().any(|c| matches!(c, Command::Bounds { .. })) {
            let [Command::Bounds {
                origin,
                tiles,
                fill,
            }] = commands
            else {
                return Err("Resize must be the only action in its batch".into());
            };
            return self.resize(*origin, *tiles, *fill);
        }
        let mut environment = environment::State::from_doc(self);
        let mut placed = self.placed.clone();
        let mut props = None;
        let mut patches = BTreeMap::new();
        let mut layers = paint::Patches::default();
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
                Command::Paint {
                    at,
                    radius,
                    paint,
                    value,
                    heading,
                } => layers.brush(&self.map, *at, *radius, *paint, *value, *heading)?,
                Command::Road {
                    from,
                    to,
                    width,
                    kind,
                } => layers.road(&self.map, *from, *to, *width, *kind)?,
                command if environment.apply(command)? => {}
                command if doodads::apply(command, &self.map, &mut props, &mut placed)? => {}
                _ => objects::apply(command, &mut placed, blueprints)?,
            }
        }
        for slot in &environment.config.players {
            if blueprints.faction_by_key(&slot.faction).is_none() {
                return Err(format!("Unknown faction: {}", slot.faction));
            }
            if usize::from(slot.start) >= self.map.starts.len().max(1) {
                return Err("Player start is outside the map's start positions".into());
            }
        }
        if !environment.config.players.is_empty()
            && placed
                .units
                .iter()
                .any(|u| usize::from(u.owner) >= environment.config.players.len())
        {
            return Err("A placed unit belongs to a removed player slot".into());
        }
        self.commit(placed, patches, layers, environment, props)
    }

    fn commit(
        &mut self,
        placed: Placed,
        patches: BTreeMap<(u32, u32), u16>,
        layers: paint::Patches,
        environment: environment::State,
        props: Option<doodads::State>,
    ) -> Result<Update, String> {
        placed
            .validate(
                &self.map.info,
                props
                    .as_ref()
                    .map_or(self.map.props.len(), |p| p.props.len()),
            )
            .map_err(|e| e.to_string())?;
        let mut change = Change {
            bounds: None,
            children: vec![],
            props: props
                .filter(|p| !p.matches(&self.map))
                .map(|p| (doodads::State::from_map(&self.map), p)),
            environment: (environment != environment::State::from_doc(self))
                .then(|| (environment::State::from_doc(self), environment)),
            heights: vec![],
            layers: layers.changes(&self.map),
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
        if change.props.is_some()
            || change.environment.is_some()
            || change.placed.is_some()
            || !change.heights.is_empty()
            || !change.layers.is_empty()
        {
            if let Some(stroke) = &mut self.stroke {
                stroke.push(change);
            } else {
                self.undo.push(change);
            }
            self.redo.clear();
            self.dirty = true;
        }
        Ok(rect)
    }

    fn restore(&mut self, change: &Change, forward: bool) -> Result<Update, String> {
        if let Some(bounds) = &change.bounds {
            return Ok(bounds.restore(self, forward));
        }
        if !change.children.is_empty() {
            return self.restore_children(&change.children, forward);
        }
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
        if let Some((old, new)) = &change.props {
            (if forward { new } else { old }).restore(&mut self.map);
        }
        if let Some((old, new)) = &change.placed {
            self.placed = if forward { new.clone() } else { old.clone() };
        }
        if let Some((old, new)) = &change.environment {
            let state = if forward { new } else { old };
            self.config = state.config.clone();
            self.map.info.water_level = state.water;
        }
        Ok(Update {
            reframe: None,
            props: change.props.is_some()
                || change
                    .placed
                    .as_ref()
                    .is_some_and(|(a, b)| a.doodads != b.doodads),
            environment: change
                .environment
                .as_ref()
                .is_some_and(|(a, b)| a.config != b.config),
            water: change
                .environment
                .as_ref()
                .is_some_and(|(a, b)| a.water != b.water),
            terrain: rect,
            objects: change.placed.is_some()
                || change
                    .environment
                    .as_ref()
                    .is_some_and(|(a, b)| a.config.players != b.config.players),
            cover: change
                .layers
                .iter()
                .map(|patch| patch.restore(&mut self.map, forward))
                .collect(),
        })
    }

    pub fn undo(&mut self) -> Result<Update, String> {
        self.end_stroke();
        let Some(change) = self.undo.pop() else {
            return Ok(Update::default());
        };
        let rect = self.restore(&change, false)?;
        self.redo.push(change);
        self.dirty = true;
        Ok(rect)
    }
    pub fn redo(&mut self) -> Result<Update, String> {
        self.end_stroke();
        let Some(change) = self.redo.pop() else {
            return Ok(Update::default());
        };
        let rect = self.restore(&change, true)?;
        self.undo.push(change);
        self.dirty = true;
        Ok(rect)
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.stroke.as_ref().is_some_and(|s| !s.is_empty())
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
pub(super) mod tests;

#[cfg(test)]
mod paint_tests;

#[cfg(test)]
mod environment_tests;

#[cfg(test)]
mod doodad_tests;

#[cfg(test)]
mod player_tests;

#[cfg(test)]
mod bounds_tests;

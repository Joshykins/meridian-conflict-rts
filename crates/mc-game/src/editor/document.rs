//! The authoring document and undoable commands shared by the UI and mailbox.
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
    Delete {
        id: u64,
    },
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
    ) -> Result<Option<[u32; 4]>, String> {
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
                Command::Unit {
                    name,
                    group,
                    blueprint,
                    owner,
                    at,
                    heading,
                } => {
                    if blueprints.unit_by_key(blueprint).is_none() {
                        return Err(format!("Unknown unit: {blueprint}"));
                    }
                    if !heading.is_finite() {
                        return Err("Heading must be finite".into());
                    }
                    let at = point(*at)?;
                    let heading = Angle::from_radians_f32(heading.to_radians());
                    if let Some(u) = placed.units.iter_mut().find(|u| u.name == *name) {
                        u.group = group.clone();
                        u.blueprint = blueprint.clone();
                        u.owner = *owner;
                        u.at = at;
                        u.heading = heading;
                    } else {
                        placed.units.push(PlacedUnit {
                            id: placed.next_id,
                            name: name.clone(),
                            group: group.clone(),
                            blueprint: blueprint.clone(),
                            owner: *owner,
                            at,
                            heading,
                        });
                        placed.next_id += 1;
                    }
                }
                Command::Region { name, min, max } => {
                    let (min, max) = (point(*min)?, point(*max)?);
                    if let Some(region) = placed.regions.iter_mut().find(|r| r.name == *name) {
                        region.min = min;
                        region.max = max;
                    } else {
                        placed.regions.push(Region {
                            id: placed.next_id,
                            name: name.clone(),
                            min,
                            max,
                        });
                        placed.next_id += 1;
                    }
                }
                Command::Point { name, at } => {
                    let at = point(*at)?;
                    if let Some(p) = placed.points.iter_mut().find(|p| p.name == *name) {
                        p.at = at;
                    } else {
                        placed.points.push(mc_map::placed::Point {
                            id: placed.next_id,
                            name: name.clone(),
                            at,
                        });
                        placed.next_id += 1;
                    }
                }
                Command::Camera {
                    name,
                    at,
                    distance,
                    yaw,
                } => {
                    if !yaw.is_finite() {
                        return Err("Camera yaw must be finite".into());
                    }
                    let at = point(*at)?;
                    let yaw = Angle::from_radians_f32(yaw.to_radians());
                    if let Some(c) = placed.cameras.iter_mut().find(|c| c.name == *name) {
                        c.at = at;
                        c.distance = *distance;
                        c.yaw = yaw;
                    } else {
                        placed.cameras.push(mc_map::placed::Camera {
                            id: placed.next_id,
                            name: name.clone(),
                            at,
                            distance: *distance,
                            yaw,
                        });
                        placed.next_id += 1;
                    }
                }
                Command::Transform { id, at, heading } => {
                    let at = point(*at)?;
                    if !heading.is_finite() {
                        return Err("Heading must be finite".into());
                    }
                    if let Some(u) = placed.units.iter_mut().find(|u| u.id == *id) {
                        u.at = at;
                        u.heading = Angle::from_radians_f32(heading.to_radians());
                    } else if let Some(p) = placed.points.iter_mut().find(|p| p.id == *id) {
                        p.at = at;
                    } else if let Some(c) = placed.cameras.iter_mut().find(|c| c.id == *id) {
                        c.at = at;
                        c.yaw = Angle::from_radians_f32(heading.to_radians());
                    } else if let Some(r) = placed.regions.iter_mut().find(|r| r.id == *id) {
                        let half = (r.max - r.min) * Fx::HALF;
                        r.min = at - half;
                        r.max = at + half;
                    } else {
                        return Err("Select a unit, point, camera or region to transform".into());
                    }
                }
                Command::Delete { id } => {
                    placed.units.retain(|u| u.id != *id);
                    placed.regions.retain(|r| r.id != *id);
                    placed.points.retain(|p| p.id != *id);
                    placed.cameras.retain(|c| c.id != *id);
                    placed.doodads.retain(|d| d.id != *id);
                }
            }
        }
        self.commit(placed, patches)
    }

    fn commit(
        &mut self,
        placed: Placed,
        patches: BTreeMap<(u32, u32), u16>,
    ) -> Result<Option<[u32; 4]>, String> {
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

    fn brush(
        &self,
        at: [f32; 2],
        radius: f32,
        strength: f32,
        brush: Brush,
        patches: &mut BTreeMap<(u32, u32), u16>,
    ) -> Result<(), String> {
        point(at)?;
        if !radius.is_finite()
            || !(8.0..=1024.0).contains(&radius)
            || !strength.is_finite()
            || !(0.0..=256.0).contains(&strength)
        {
            return Err("Brush radius must be 8–1024 m and strength 0–256 m".into());
        }
        let (w, h) = self.map.info.size_cells();
        let cell = CELL_SIZE_M as f32;
        if at[0] < 0.0 || at[1] < 0.0 || at[0] > w as f32 * cell || at[1] > h as f32 * cell {
            return Err("Brush is outside the map".into());
        }
        let min = [
            ((at[0] - radius) / cell).max(0.0) as u32,
            ((at[1] - radius) / cell).max(0.0) as u32,
        ];
        let max = [
            ((at[0] + radius) / cell).ceil().min(w as f32) as u32,
            ((at[1] + radius) / cell).ceil().min(h as f32) as u32,
        ];
        let old = |x, y| {
            patches
                .get(&(x, y))
                .copied()
                .unwrap_or(self.map.samples()[(y * (w + 1) + x) as usize])
        };
        let target = old((at[0] / cell).round() as u32, (at[1] / cell).round() as u32) as f32;
        let step = self.map.info.z_step.to_f32();
        let mut updates = Vec::new();
        for y in min[1]..=max[1] {
            for x in min[0]..=max[0] {
                let distance =
                    ((x as f32 * cell - at[0]).powi(2) + (y as f32 * cell - at[1]).powi(2)).sqrt();
                if distance > radius {
                    continue;
                }
                let weight = (1.0 - distance / radius).clamp(0.0, 1.0);
                let value = old(x, y) as f32;
                let next = match brush {
                    Brush::Raise => value + strength / step * weight,
                    Brush::Lower => value - strength / step * weight,
                    Brush::Flatten => value + (target - value) * weight,
                    Brush::Smooth => {
                        let mut sum = 0.0;
                        let mut count = 0.0;
                        for ny in y.saturating_sub(1)..=(y + 1).min(h) {
                            for nx in x.saturating_sub(1)..=(x + 1).min(w) {
                                sum += old(nx, ny) as f32;
                                count += 1.0;
                            }
                        }
                        value + (sum / count - value) * weight
                    }
                };
                updates.push(((x, y), next.round().clamp(0.0, u16::MAX as f32) as u16));
            }
        }
        patches.extend(updates);
        Ok(())
    }

    fn restore(&mut self, change: &Change, forward: bool) -> Result<Option<[u32; 4]>, String> {
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
        Ok(rect)
    }

    pub fn undo(&mut self) -> Result<Option<[u32; 4]>, String> {
        let Some(change) = self.undo.pop() else {
            return Ok(None);
        };
        let rect = self.restore(&change, false)?;
        self.redo.push(change);
        self.dirty = true;
        Ok(rect)
    }
    pub fn redo(&mut self) -> Result<Option<[u32; 4]>, String> {
        let Some(change) = self.redo.pop() else {
            return Ok(None);
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

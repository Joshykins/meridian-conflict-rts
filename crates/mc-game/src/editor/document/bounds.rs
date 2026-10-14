//! Whole-document resize is rare; retain full before/after states for undo.
use super::*;
use std::sync::Arc;
pub(super) struct Change {
    before: State,
    after: State,
    delta: [f32; 2],
}
struct State {
    map: EditMap,
    placed: Placed,
    config: mc_data::weather::MapConfig,
}
impl State {
    fn of(doc: &Document) -> Self {
        Self {
            map: doc.map.clone(),
            placed: doc.placed.clone(),
            config: doc.config.clone(),
        }
    }
}
impl Change {
    pub fn restore(&self, doc: &mut Document, forward: bool) -> Update {
        let state = if forward { &self.after } else { &self.before };
        doc.map = state.map.clone();
        doc.placed = state.placed.clone();
        doc.config = state.config.clone();
        Update {
            reframe: Some(self.delta.map(|v| if forward { v } else { -v })),
            ..Default::default()
        }
    }
}
impl Document {
    pub fn history_reframes(&self, redo: bool) -> bool {
        (if redo { &self.redo } else { &self.undo })
            .last()
            .is_some_and(|c| c.bounds.is_some())
    }
    pub(super) fn resize(
        &mut self,
        origin: [i32; 2],
        tiles: [u32; 2],
        fill: f32,
    ) -> Result<Update, String> {
        if self.stroking() {
            return Err("Finish the brush stroke before resizing".into());
        }
        if !fill.is_finite() || !(-2000.0..=4000.0).contains(&fill) {
            return Err("Extension height must be between -2000 and 4000 metres".into());
        }
        let (map, remap) = self
            .map
            .resized(origin, tiles, Fx::from_f32(fill))
            .map_err(|e| e.to_string())?;
        let delta = FxVec2::from_ints(
            origin[0] * mc_map::TILE_SIZE_M,
            origin[1] * mc_map::TILE_SIZE_M,
        );
        let size = map.info.size_metres();
        let mut placed = self.placed.clone();
        placed.units.retain_mut(|p| {
            p.at -= delta;
            inside(p.at, size)
        });
        placed.points.retain_mut(|p| {
            p.at -= delta;
            inside(p.at, size)
        });
        placed.cameras.retain_mut(|p| {
            p.at -= delta;
            inside(p.at, size)
        });
        placed.regions.retain_mut(|p| {
            p.min = clamp(p.min - delta, size);
            p.max = clamp(p.max - delta, size);
            p.min.x < p.max.x && p.min.y < p.max.y
        });
        placed.doodads.retain_mut(|d| {
            if let Some(index) = remap[d.prop_index as usize] {
                d.prop_index = index;
                true
            } else {
                false
            }
        });
        placed
            .validate(&map.info, map.props.len())
            .map_err(|e| e.to_string())?;
        let mut config = self.config.clone();
        let shifted = delta.to_f32();
        let mut walls = config.walls().walls().to_vec();
        for wall in &mut walls {
            for point in &mut wall.line {
                point.0 -= shifted[0];
                point.1 -= shifted[1];
            }
        }
        config
            .set_regions(config.regions().to_vec(), walls)
            .map_err(|e| e.to_string())?;
        if let Some(survival) = &mut config.survival {
            shift_survival(survival, shifted, size.to_f32())?;
        }
        let change = Arc::new(Change {
            before: State::of(self),
            after: State {
                map,
                placed,
                config,
            },
            delta: shifted,
        });
        let update = change.restore(self, true);
        self.undo.push(super::Change {
            bounds: Some(change),
            props: None,
            children: vec![],
            heights: vec![],
            placed: None,
            layers: vec![],
            environment: None,
        });
        self.redo.clear();
        self.dirty = true;
        Ok(update)
    }
}
fn inside(p: FxVec2, size: FxVec2) -> bool {
    p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size.x && p.y <= size.y
}
fn clamp(p: FxVec2, size: FxVec2) -> FxVec2 {
    FxVec2::new(p.x.clamp(Fx::ZERO, size.x), p.y.clamp(Fx::ZERO, size.y))
}
fn shift_survival(
    s: &mut mc_data::survival::SurvivalLayout,
    delta: [f32; 2],
    size: [f32; 2],
) -> Result<(), String> {
    let shift = |p: &mut (f32, f32)| -> Result<(), String> {
        p.0 -= delta[0];
        p.1 -= delta[1];
        if p.0 < 0.0 || p.1 < 0.0 || p.0 > size[0] || p.1 > size[1] {
            return Err(
                "Crop would remove a required survival location; extend the bounds to retain it"
                    .into(),
            );
        }
        Ok(())
    };
    shift(&mut s.engine)?;
    if let Some(p) = &mut s.harbor {
        shift(p)?;
    }
    for bay in &mut s.bays {
        shift(&mut bay.at)?;
        let mut emitter = (bay.emitter.0, bay.emitter.1);
        shift(&mut emitter)?;
        bay.emitter.0 = emitter.0;
        bay.emitter.1 = emitter.1;
    }
    for guard in &mut s.guards {
        shift(&mut guard.at)?;
    }
    for front in &mut s.fronts {
        for p in &mut front.path {
            shift(p)?;
        }
    }
    for node in &mut s.node_sites {
        shift(&mut node.at)?;
        for p in &mut node.path {
            shift(p)?;
        }
    }
    Ok(())
}

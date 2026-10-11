//! A structure (or a unit raised on a lot) being placed: where it would go under the
//! pointer, the sites a place-drag lays, and which way it faces. R turns it a quarter
//! clockwise, Shift+R anticlockwise, as it turns a prefab in hand (`game_prefabs.rs`);
//! what is never turned (`UnitBlueprint::turnable`: walls, core mines) refuses.

use super::{Game, Mode};
use crate::audio::{Audio, Sfx};
use crate::orders::{self, Field};
use glam::Vec2;
use mc_core::{Angle, FxVec2};
use mc_data::BlueprintId;
use mc_render::Renderer;
use mc_sim::placement::Unfit;

impl Game {
    /// R with a structure or a prefab in hand: a quarter turn clockwise (Shift:
    /// anticlockwise). True when it was in hand, so the key is taken.
    pub(super) fn turn_in_hand(&mut self, audio: &Audio) -> bool {
        let turnable = match self.view.mode {
            Mode::Place(blueprint) => self.blueprints.unit(blueprint).turnable(),
            Mode::Prefab(_) => true,
            _ => return false,
        };
        if turnable {
            self.prefabs.turn.step(self.shift);
            audio.play(Sfx::Tick);
        } else {
            audio.play(Sfx::Deny);
        }
        true
    }

    /// The heading `blueprint` is placed with: turned as the hand is.
    pub(super) fn place_heading(&self, blueprint: BlueprintId) -> Angle {
        self.prefabs.turn.heading(self.blueprints.unit(blueprint))
    }

    /// Where the structure being placed would go, and whether it looks buildable.
    /// The sim has the final say; this only drives the preview colour.
    pub(super) fn placement(&self, blueprint: BlueprintId, r: &Renderer) -> Option<(FxVec2, bool)> {
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer: r,
        };
        orders::site(
            &field,
            blueprint,
            orders::surface_under(&field, self.cursor)?,
            self.place_heading(blueprint),
            None,
        )
    }

    /// Sites a click or a place-drag would put down. `drag` is the pointer having
    /// moved a footprint's worth; without it the ghost stays under the cursor.
    pub(super) fn placing_sites(
        &self,
        blueprint: BlueprintId,
        drag: bool,
        r: &Renderer,
    ) -> Vec<(FxVec2, Result<(), Unfit>)> {
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer: r,
        };
        let Some(ground) = orders::surface_under(&field, self.cursor) else {
            return Vec::new();
        };
        let heading = self.place_heading(blueprint);
        let Some((to, _)) = orders::site(&field, blueprint, ground, heading, None) else {
            return Vec::new();
        };
        let mut sites: Vec<_> = match (drag, self.place_from) {
            (true, Some(from)) => orders::drag_sites(&field, blueprint, heading, from, to),
            _ => orders::site_verdict(&field, blueprint, ground, heading, None, &[])
                .into_iter()
                .collect(),
        };
        self.refuse_beyond_stations(blueprint, &mut sites);
        sites
    }

    /// Placed only by engineering stations: a site beyond all their reach is refused,
    /// as the sim refuses it (`Refusal::OutOfReach`).
    pub(super) fn refuse_beyond_stations(
        &self,
        blueprint: BlueprintId,
        sites: &mut [(FxVec2, Result<(), Unfit>)],
    ) {
        let placers: Vec<(Vec2, &mc_data::UnitBlueprint)> = self
            .selected_units()
            .map(|u| (u, self.blueprints.unit(BlueprintId(u.blueprint as u16))))
            .filter(|(_, b)| {
                b.works_in_field()
                    && b.builder
                        .as_ref()
                        .is_some_and(|b| b.builds.contains(&blueprint))
            })
            .map(|(u, b)| (Vec2::new(u.pos[0], u.pos[1]), b))
            .collect();
        if placers.is_empty() || !placers.iter().all(|(_, b)| b.is_station()) {
            return;
        }
        let radius = self.blueprints.unit(blueprint).radius.to_f32();
        let reaches = |at: Vec2| {
            placers.iter().any(|&(s, b)| {
                s.distance(at) <= b.builder.as_ref().map_or(0.0, |b| b.range.to_f32()) + radius
            })
        };
        for (pos, verdict) in sites.iter_mut() {
            if verdict.is_ok() && !reaches(Vec2::from(pos.to_f32())) {
                *verdict = Err(Unfit::OutOfReach);
            }
        }
    }
}

//! Prefabs in a match (`crate::prefabs`): Ctrl+S (or the Save Prefab order) keeps the
//! selected structures' layout, and the construction panel's Prefabs tab puts one
//! down with the selected builders, turned with R.
//!
//! Placing one sends a `Command::Build` for each structure that can go down, nearest
//! the builders first, exactly as placing them one by one would.

use super::{Game, Mode};
use crate::audio::{Audio, Sfx};
use crate::hud::{self, HudAction};
use crate::orders::{self, Field};
use crate::prefabs::{self, Kept, Library, Prefab, PrefabId, PrefabSite};
use crate::ui::palette;
use glam::Vec2;
use mc_core::{Fx, FxVec2};
use mc_data::BlueprintId;
use mc_render::Renderer;
use mc_sim::mirror::UnitInstance;
use mc_sim::Command;

/// The library, and how the prefab in hand is turned.
#[derive(Default)]
pub struct Prefabs {
    pub library: Library,
    /// Quarter turns anticlockwise, seen from above.
    pub quarters: u8,
}

impl Prefabs {
    pub fn load() -> Prefabs {
        Prefabs {
            library: Library::load(),
            quarters: 0,
        }
    }
}

impl Game {
    /// What the selected builders can start.
    fn prefab_builds(&self) -> Vec<BlueprintId> {
        hud::prefabs::selection_builds(&self.view, &self.blueprints)
    }

    /// Where each structure of prefab `id` would go with its middle under the pointer,
    /// and whether it can. Empty off the map or for a prefab that is gone.
    pub(super) fn prefab_sites(&self, id: PrefabId, r: &Renderer) -> Vec<PrefabSite> {
        let Some(prefab) = self.prefabs.library.get(id) else {
            return Vec::new();
        };
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
        let builds = self.prefab_builds();
        orders::prefab_sites(&field, prefab, ground, self.prefabs.quarters, &builds)
    }

    /// The prefab in hand's holograms, and their lots outlined.
    pub(super) fn prefab_ghosts(
        &self,
        sites: &[PrefabSite],
        r: &Renderer,
        ghosts: &mut Vec<UnitInstance>,
    ) {
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer: r,
        };
        for s in sites {
            ghosts.push(orders::placement_ghost(
                &field,
                s.blueprint,
                s.at,
                s.goes_down(),
            ));
        }
    }

    /// A click with a prefab in hand: every structure of it that can go down is
    /// ordered, nearest the builders first; Shift queues them after what the builders
    /// have, and keeps the prefab in hand.
    pub(super) fn place_prefab(&mut self, id: PrefabId, r: &Renderer, audio: &Audio) {
        let sites = self.prefab_sites(id, r);
        let mut going: Vec<(BlueprintId, FxVec2)> = sites
            .iter()
            .filter(|s| s.goes_down())
            .map(|s| (s.blueprint, s.at))
            .collect();
        if going.is_empty() {
            audio.play(Sfx::Deny);
            return;
        }
        audio.play(Sfx::Order);
        // Nearest the builders' middle first, then each next nearest the last: one walk
        // through the layout rather than back and forth across it.
        let mut from = self.builders_middle();
        let mut ordered = Vec::with_capacity(going.len());
        while !going.is_empty() {
            let (i, _) = going
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, at))| (*at - from).length_sq())
                .expect("not empty");
            let next = going.remove(i);
            from = next.1;
            ordered.push(next);
        }
        let mines: Vec<(BlueprintId, FxVec2)> = ordered
            .iter()
            .copied()
            .filter(|(b, _)| self.blueprints.unit(*b).mine.is_some())
            .collect();
        for (b, at) in mines {
            self.warn_of_planned_mines(b, &[at]);
        }
        let units = self.selected_ids();
        for (i, (blueprint, pos)) in ordered.iter().enumerate() {
            self.send(Command::Build {
                units: units.clone(),
                blueprint: *blueprint,
                pos: *pos,
                heading: self.blueprints.unit(*blueprint).build_heading(),
                queue: self.shift || i > 0,
            });
        }
        let left = sites.len() - ordered.len();
        if left > 0 {
            self.hud.toast(
                format!(
                    "{left} of {} structures left out: {}",
                    sites.len(),
                    prefabs::left_out_reason(&sites)
                ),
                palette::WARN,
            );
        }
        if self.prefabs.library.touch(id) {
            self.save_prefabs();
        }
        if !self.shift {
            self.view.mode = Mode::Normal;
        }
    }

    fn builders_middle(&self) -> FxVec2 {
        let (sum, n) = self
            .selected_units()
            .filter(|u| {
                self.blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .is_mobile()
            })
            .fold((Vec2::ZERO, 0.0), |(s, n), u| {
                (s + Vec2::new(u.pos[0], u.pos[1]), n + 1.0)
            });
        let m = if n > 0.0 { sum / n } else { sum };
        FxVec2::new(Fx::from_f32(m.x), Fx::from_f32(m.y))
    }

    /// R with a prefab in hand: a quarter turn clockwise (Shift: anticlockwise).
    pub(super) fn turn_prefab(&mut self, audio: &Audio) {
        let step = if self.shift { 1 } else { 3 };
        self.prefabs.quarters = (self.prefabs.quarters + step) % 4;
        audio.play(Sfx::Tick);
    }

    /// Takes prefab `id` in hand.
    pub(super) fn hold_prefab(&mut self, id: PrefabId, audio: &Audio) {
        let Some(prefab) = self.prefabs.library.get(id) else {
            audio.play(Sfx::Deny);
            return;
        };
        let builds = self.prefab_builds();
        let (members, gone) = prefab.resolve(&self.blueprints);
        if !members.iter().any(|m| builds.contains(&m.blueprint)) {
            audio.play(Sfx::Deny);
            self.hud.toast(
                format!("Nothing in {} can be built by this selection", prefab.name),
                palette::BAD,
            );
            return;
        }
        if gone > 0 {
            self.hud.toast(
                format!(
                    "{gone} structures of {} are no longer in the game",
                    prefab.name
                ),
                palette::WARN,
            );
        }
        self.hud.build_keys = false;
        self.view.mode = Mode::Prefab(id);
        self.left_down = None;
        self.place_from = None;
    }

    /// Ctrl+S, or the Save Prefab order: the selected structures (and the structures
    /// the selected builders have planned) as a prefab, offered for naming.
    pub(super) fn offer_prefab(&mut self, audio: &Audio) {
        if self.view.observing {
            return;
        }
        let structures = hud::prefabs::selection_structures(&self.view);
        match prefabs::capture(&self.blueprints, &structures) {
            Ok(draft) => {
                audio.play(Sfx::Select);
                self.hud.prefabs.offer(draft);
            }
            Err(why) => {
                audio.play(Sfx::Deny);
                self.hud.toast(why.label(), palette::BAD);
            }
        }
    }

    pub(super) fn prefab_action(&mut self, action: HudAction, audio: &Audio) {
        match action {
            HudAction::HoldPrefab(id) => self.hold_prefab(id, audio),
            HudAction::KeepPrefab(prefab) => self.keep_prefab(prefab),
            HudAction::DropPrefab(id) => {
                if let Some(gone) = self.prefabs.library.remove(id) {
                    if self.view.mode == Mode::Prefab(id) {
                        self.view.mode = Mode::Normal;
                    }
                    self.save_prefabs();
                    audio.play(Sfx::Back);
                    self.hud
                        .toast(format!("Deleted prefab {}", gone.name), palette::DIM);
                }
            }
            _ => {}
        }
    }

    fn keep_prefab(&mut self, prefab: Prefab) {
        let name = prefab.name.clone();
        let count = prefab.members.len();
        match self.prefabs.library.keep(prefab) {
            Some(kept) => {
                if self.save_prefabs() {
                    let verb = match kept {
                        Kept::Added(_) => "Saved",
                        Kept::Replaced(_) => "Replaced",
                    };
                    self.hud.toast(
                        format!("{verb} prefab {name} \u{b7} {count} structures \u{b7} B, then P to place it"),
                        hud::HEALTHY,
                    );
                }
            }
            None => self.hud.toast(
                format!(
                    "The library holds {} prefabs: delete one first (right-click it twice)",
                    prefabs::MAX_PREFABS
                ),
                palette::BAD,
            ),
        }
    }

    /// Writes the library; says so when it cannot. True when it was written.
    fn save_prefabs(&mut self) -> bool {
        match self.prefabs.library.save() {
            Ok(()) => true,
            Err(e) => {
                log::warn!("prefabs not saved: {e}");
                self.hud.toast(
                    format!("Prefabs kept for this match only: {e}"),
                    palette::BAD,
                );
                false
            }
        }
    }
}

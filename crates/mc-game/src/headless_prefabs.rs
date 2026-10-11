//! Prefabs in a headless match screenshot: `--prefabs FILE` reads a library and opens
//! the construction panel on its Prefabs tab, `--place @NAME` holds that prefab at the
//! pointer, and `--save-prefab` opens the Save Prefab box on the selection.

use super::Shot;
use crate::game::{Mode, View};
use crate::hud::Hud;
use crate::orders::Field;
use crate::prefabs::{Library, PrefabSite};
use crate::ui::Ui;
use glam::Vec2;
use mc_data::Blueprints;
use mc_sim::mirror::UnitInstance;

pub(super) struct PrefabShot {
    library: Option<Library>,
}

impl PrefabShot {
    pub(super) fn stage(
        shot: &Shot,
        hud: &mut Hud,
        view: &mut View,
        blueprints: &Blueprints,
    ) -> Result<PrefabShot, String> {
        let library = shot
            .prefabs
            .as_deref()
            .map(|path| Library::read(std::path::Path::new(path)))
            .transpose()?;
        if library.is_some() {
            hud.prefabs.open_tab();
        }
        if let Some(name) = shot.place.as_deref().and_then(|p| p.strip_prefix('@')) {
            let prefab = library
                .as_ref()
                .and_then(|l| l.by_name(name))
                .ok_or(format!("--place @{name}: no such prefab (--prefabs FILE)"))?;
            view.mode = Mode::Prefab(prefab.id);
        }
        if shot.save_prefab {
            let structures = crate::hud::prefabs::selection_structures(view);
            let draft = crate::prefabs::capture(blueprints, &structures)
                .map_err(|why| format!("--save-prefab: {}", why.label()))?;
            hud.prefabs.offer(draft);
        }
        Ok(PrefabShot { library })
    }

    pub(super) fn library(&self) -> &Library {
        self.library.as_ref().unwrap_or(Library::none())
    }

    /// The prefab in hand's holograms at `cursor`, and where its structures would go.
    pub(super) fn ghosts(
        &self,
        field: &Field,
        cursor: Vec2,
        ghosts: &mut Vec<UnitInstance>,
    ) -> Vec<PrefabSite> {
        let Mode::Prefab(id) = field.view.mode else {
            return Vec::new();
        };
        let (Some(prefab), Some(ground)) = (
            self.library().get(id),
            crate::orders::surface_under(field, cursor),
        ) else {
            return Vec::new();
        };
        let builds = crate::hud::prefabs::selection_builds(field.view, field.blueprints);
        let sites = crate::orders::prefab_sites(field, prefab, ground, 0, &builds);
        for s in &sites {
            ghosts.push(crate::orders::placement_ghost(
                field,
                s.blueprint,
                s.at,
                s.goes_down(),
            ));
        }
        sites
    }

    /// The card by the pointer with the prefab in hand.
    pub(super) fn hint(&self, ui: &mut Ui, view: &View, sites: &[PrefabSite]) {
        if let Mode::Prefab(id) = view.mode {
            if let Some(prefab) = self.library().get(id) {
                crate::hud::prefabs::cursor_hint(ui, prefab, sites, 0);
            }
        }
    }
}

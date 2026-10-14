//! In-game map authoring. The document outlives playtests and owns all edits.
mod api;
mod browser;
mod cameras;
mod catalogue;
mod document;
mod doodads;
mod environment;
mod frame;
mod gestures;
mod interaction;
mod map_bounds;
mod map_controls;
mod navigation;
mod overlays;
mod panels;
mod playtest;
mod preview;
mod reload;
mod save;
pub(crate) use reload::ReloadRequest;
mod screenshot;
mod server;
mod snapshot;
mod strokes;
mod tools;
pub(crate) use screenshot::screenshot;
pub(crate) use server::serve;

use crate::game::{FrameCtx, GameStart};
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use document::{Brush, Command, Document};
use glam::Vec2;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput};
use mc_sim::{MatchConfig, PlayerSetup, RenderFrame, World};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub(crate) enum Event {
    Leave,
    Open(Box<Editor>),
    Test(Box<GameStart>),
    StopTest,
}

pub(crate) struct Editor {
    pub map: Arc<MapFile>,
    pub camera: Camera,
    doc: Document,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    frame: RenderFrame,
    fresh: bool,
    tool: usize,
    tool_group: usize,
    stroke: strokes::Stroke,
    paint: tools::PaintSettings,
    environment: environment::Controls,
    doodad: doodads::Controls,
    map_controls: map_controls::Controls,
    radius: f32,
    strength: f32,
    stamp_asset: String,
    name: String,
    blueprint: String,
    group: String,
    owner: u8,
    save_name: String,
    message: String,
    held: std::collections::BTreeSet<KeyCode>,
    region_start: Option<[f32; 2]>,
    selected: Option<u64>,
    object_scroll: usize,
    heading: f32,
    confirm_exit: bool,
    browser: Option<browser::Browse>,
    api: api::Api,
    reload: reload::Reload,
    navigation: navigation::Navigation,
    catalogue: Option<catalogue::Catalogue>,
}

fn config(placed: &mc_map::placed::Placed, map: &mc_data::weather::MapConfig) -> MatchConfig {
    MatchConfig {
        seed: 7,
        cheats: true,
        fog: false,
        spawn_commanders: false,
        players: if !map.players.is_empty() {
            crate::map_players::roster(&map.players)
        } else {
            (0..placed
                .units
                .iter()
                .map(|u| usize::from(u.owner) + 1)
                .max()
                .unwrap_or(6)
                .max(6))
                .map(|i| PlayerSetup {
                    name: ["Company", "Rebels", "PDF", "ARC", "Holdouts", "Miners"]
                        .get(i)
                        .map(|s| (*s).to_owned())
                        .unwrap_or_else(|| format!("Player {}", i + 1)),
                    faction: "Aster".into(),
                    ai: Default::default(),
                    team: if i == 1 { 1 } else { 0 },
                    controller: mc_sim::tables::Controller::Human,
                    start: 0,
                })
                .collect()
        },
    }
}

impl Editor {
    pub fn open(
        path: &Path,
        blueprints: Arc<Blueprints>,
        pool: Arc<Pool>,
        viewport: Vec2,
    ) -> Result<Self, String> {
        mc_render::models::remote::keep_meshes();
        let doc = Document::open(path)?;
        let map = snapshot::map(&doc, "view")?;
        let size = Vec2::from(map.info().size_metres().to_f32());
        let mut camera = Camera::new(size, viewport);
        if let Some(start) = map.start_positions().first() {
            camera.focus = Vec2::from(start.to_f32()).extend(0.0);
            camera.distance = 1600.0;
        }
        let mut editor = Self {
            map,
            camera,
            doc,
            blueprints,
            pool,
            frame: Default::default(),
            fresh: true,
            tool: 0,
            tool_group: 0,
            stroke: Default::default(),
            paint: Default::default(),
            environment: Default::default(),
            doodad: Default::default(),
            map_controls: Default::default(),
            radius: 96.0,
            strength: 8.0,
            stamp_asset: "crater.json".into(),
            name: "company_tank".into(),
            blueprint: "aster_t1_tank".into(),
            group: "company".into(),
            owner: 0,
            save_name: "vela_m1".into(),
            message: "Choose a tool; click the terrain to edit.".into(),
            held: Default::default(),
            region_start: None,
            selected: None,
            object_scroll: 0,
            heading: 0.0,
            confirm_exit: false,
            browser: None,
            api: api::Api::new(),
            reload: Default::default(),
            navigation: Default::default(),
            catalogue: None,
        };
        editor.refresh_units()?;
        Ok(editor)
    }

    fn refresh_units(&mut self) -> Result<(), String> {
        self.frame = preview::build(
            &self.doc,
            self.map.content_id(),
            self.blueprints.clone(),
            self.pool.clone(),
        )?;
        self.fresh = true;
        Ok(())
    }

    pub fn window_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if let PhysicalKey::Code(code) = event.physical_key {
                if event.state == ElementState::Pressed {
                    self.held.insert(code);
                } else {
                    self.held.remove(&code);
                }
            }
        }
        if matches!(event, WindowEvent::Focused(false)) {
            self.held.clear();
        }
    }

    pub fn restore(&mut self, renderer: &mut mc_render::Renderer) -> Result<(), String> {
        self.held.clear();
        self.api.test_state = None;
        renderer
            .edit_props(&self.doc.map, &self.doc.placed)
            .map_err(|e| e.to_string())?;
        renderer
            .restore_editor_terrain(&self.doc.map)
            .map_err(|e| e.to_string())?;
        self.apply_environment(renderer);
        renderer
            .edit_water(self.doc.map.info.water_level)
            .map_err(|e| e.to_string())?;
        self.fresh = true;
        Ok(())
    }

    fn apply(
        &mut self,
        command: Command,
        renderer: &mut mc_render::Renderer,
    ) -> Result<(), String> {
        self.apply_batch(&[command], renderer)
    }

    fn apply_batch(
        &mut self,
        commands: &[Command],
        renderer: &mut mc_render::Renderer,
    ) -> Result<(), String> {
        if commands.iter().any(|c| matches!(c, Command::Bounds { .. })) {
            let mut document = self.doc.clone();
            let update = document.apply(commands, &self.blueprints)?;
            return self.request_document(
                document,
                update.reframe.ok_or("Resize did not produce new bounds")?,
            );
        }
        let update = self.doc.apply(commands, &self.blueprints)?;
        self.sync_update(update, renderer)?;
        self.message = "Edit applied. Undo restores this action.".into();
        Ok(())
    }

    fn history(&mut self, redo: bool, renderer: &mut mc_render::Renderer) -> Result<(), String> {
        if self.doc.history_reframes(redo) {
            let mut document = self.doc.clone();
            let update = if redo {
                document.redo()?
            } else {
                document.undo()?
            };
            return self.request_document(
                document,
                update.reframe.ok_or("Resize history lost its bounds")?,
            );
        }
        let update = if redo {
            self.doc.redo()?
        } else {
            self.doc.undo()?
        };
        self.sync_update(update, renderer)
    }

    fn sync_update(
        &mut self,
        update: document::Update,
        renderer: &mut mc_render::Renderer,
    ) -> Result<(), String> {
        if let Some(rect) = update.terrain {
            renderer
                .edit_terrain(&self.doc.map, rect)
                .map_err(|e| e.to_string())?;
        }
        let scenery = update.props
            || update.terrain.is_some()
            || update.water
            || update
                .cover
                .iter()
                .any(|(layer, _)| *layer == mc_map::edit::Layer::Snow);
        for (layer, rect) in update.cover {
            renderer
                .edit_cover(&self.doc.map, layer, rect)
                .map_err(|e| e.to_string())?;
        }
        if update.environment {
            self.apply_environment(renderer);
        }
        if update.water || (update.terrain.is_some() && !self.doc.stroking()) {
            renderer
                .edit_water(self.doc.map.info.water_level)
                .map_err(|e| e.to_string())?;
        }
        if scenery && !self.doc.stroking() {
            self.fresh = true;
            renderer
                .edit_props(&self.doc.map, &self.doc.placed)
                .map_err(|e| e.to_string())?;
        }
        // Unit origins come from the simulation mirror; rebuild them when the
        // ground changes too. Props sample the live height tiles in the shader.
        if update.objects || update.water || update.terrain.is_some() {
            self.refresh_units()?;
        }
        if update.objects || update.props {
            if let Some(id) = self.selected {
                if self.select_object(id).is_err() {
                    self.selected = None;
                }
            }
        }
        Ok(())
    }

    fn save(&mut self) -> Result<(), String> {
        if self.save_name.is_empty()
            || !self
                .save_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("Use letters, numbers, underscores or hyphens for the map name".into());
        }
        let folder = self
            .doc
            .destination
            .clone()
            .unwrap_or_else(|| user_maps().join(&self.save_name));
        self.doc.save(&folder)?;
        self.message = format!("Saved {}", folder.display());
        Ok(())
    }

    fn test(&mut self) -> Result<GameStart, String> {
        // Playtests get a private map copy, so neither sim flattening nor test
        // saves can mutate the authored document.
        let map = snapshot::map(&self.doc, "test")?;
        let request = crate::ui::setup::MatchRequest {
            map: map.clone(),
            config: config(&self.doc.placed, &self.doc.config),
            colors: crate::setup::TEAM_COLORS,
            survival: None,
            sky: Default::default(),
        };
        let mission = crate::campaign::definition(&map)?;
        crate::app::local_editor_start(
            request,
            &self.blueprints,
            mission,
            self.map_controls.test_slot,
        )
    }
}

fn user_maps() -> PathBuf {
    crate::settings::path()
        .and_then(|p| p.parent().map(|p| p.join("maps")))
        .unwrap_or_else(|| PathBuf::from("maps"))
}

pub(crate) fn apply_file(
    path: &Path,
    actions: &Path,
    blueprints: &Blueprints,
) -> Result<(), String> {
    use std::io::Read;
    let file = std::fs::File::open(actions).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(262_145)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 262_144 {
        return Err("Editor batch exceeds 256 KiB".into());
    }
    let commands: Vec<Command> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let mut doc = Document::open(path)?;
    let folder = doc
        .destination
        .clone()
        .ok_or("Batch editing requires a map folder containing terrain.mcmap")?;
    doc.apply(&commands, blueprints)?;
    doc.save(&folder)
}

//! In-game map authoring. The document outlives playtests and owns all edits.
mod api;
mod browser;
mod catalogue;
mod document;
mod frame;
mod interaction;
mod navigation;
mod overlays;
mod panels;
mod preview;
mod reload;
mod save;
pub(crate) use reload::ReloadRequest;
mod screenshot;
mod server;
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

fn config() -> MatchConfig {
    MatchConfig {
        seed: 7,
        cheats: true,
        fog: false,
        spawn_commanders: false,
        players: ["Company", "Rebels", "PDF", "ARC", "Holdouts", "Miners"]
            .iter()
            .enumerate()
            .map(|(i, name)| PlayerSetup {
                name: (*name).into(),
                faction: "Aster".into(),
                ai: Default::default(),
                team: if i == 1 { 1 } else { 0 },
                controller: mc_sim::tables::Controller::Human,
                start: 0,
            })
            .collect(),
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
        let map = Arc::new(MapFile::open(path).map_err(|e| e.to_string())?);
        let doc = Document::open(path)?;
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
        let (w, h) = self.doc.map.info.size_cells();
        renderer
            .edit_terrain(&self.doc.map, [0, 0, w, h])
            .map_err(|e| e.to_string())?;
        self.fresh = true;
        Ok(())
    }

    fn apply(
        &mut self,
        command: Command,
        renderer: &mut mc_render::Renderer,
    ) -> Result<(), String> {
        let objects = !command.is_terrain();
        if let Some(rect) = self.doc.apply(&[command], &self.blueprints)? {
            renderer
                .edit_terrain(&self.doc.map, rect)
                .map_err(|e| e.to_string())?;
        }
        if objects {
            self.refresh_units()?;
        }
        self.message = "Edit applied. Undo restores this action.".into();
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
        let folder =
            std::env::temp_dir().join(format!("meridian-editor-test-{}", std::process::id()));
        self.doc.export(&folder)?;
        let map = Arc::new(MapFile::open(folder.join("terrain.mcmap")).map_err(|e| e.to_string())?);
        let request = crate::ui::setup::MatchRequest {
            map: map.clone(),
            config: config(),
            colors: crate::setup::TEAM_COLORS,
            survival: None,
            sky: Default::default(),
        };
        let mission = crate::campaign::definition(&map)?;
        let has_mission = mission.is_some();
        let mut start = crate::app::local_mission_start(request, &self.blueprints, false, mission)?;
        if has_mission {
            return Ok(start);
        }
        let units = self.doc.placed.units.clone();
        start.scene = Some((
            Box::new(move |world| {
                units
                    .iter()
                    .filter_map(|u| {
                        let bp = world.blueprints.id_of(&u.blueprint)?;
                        Some(mc_sim::PlayerCommand {
                            player: u.owner,
                            command: mc_sim::Command::DebugSpawn {
                                blueprint: bp,
                                pos: u.at,
                                heading: u.heading,
                                owner: u.owner,
                                count: 1,
                                flags: 0,
                                build: 1000,
                            },
                        })
                    })
                    .collect()
            }),
            Box::new(|_| vec![]),
        ));
        Ok(start)
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

//! Mission assets, startup and presentation. Simulation only sees stable text IDs.
mod camera;
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use camera::Pan;
use mc_sim::{
    mission::{Definition, Objective, Status},
    MatchConfig, PlayerSetup,
};
use serde::Deserialize;
use std::collections::BTreeMap;

pub(crate) fn definition(map: &mc_map::MapFile) -> Result<Option<Definition>, String> {
    let config = mc_data::weather::MapConfig::for_map(map.path())?;
    match config.mission.as_deref() {
        None => Ok(None),
        Some("vela_m1") => Ok(Some(mc_missions::definition(map.placed().clone()))),
        Some(key) => Err(format!("Unsupported mission script: {key}")),
    }
}

pub(crate) fn config() -> MatchConfig {
    MatchConfig {
        seed: 7,
        cheats: false,
        fog: true,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    speaker: String,
    text: String,
}

#[derive(Default)]
pub(crate) struct Presentation {
    strings: BTreeMap<String, Text>,
    camera: Option<u32>,
    pan: Option<Pan>,
    line: Option<String>,
    error: Option<String>,
}

impl Presentation {
    pub(crate) fn load(map: &mc_map::MapFile) -> Self {
        let mut result = Self::default();
        let path = map.path().with_file_name("strings.en.ron");
        if !path.exists() {
            return result;
        }
        let read = (|| {
            use std::io::Read;
            let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            let mut bytes = Vec::new();
            file.take(1_048_577)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 1_048_576 {
                return Err("Mission strings exceed 1 MiB".into());
            }
            ron::de::from_bytes(&bytes).map_err(|e| e.to_string())
        })();
        match read {
            Ok(strings) => result.strings = strings,
            Err(e) => result.error = Some(e),
        }
        result
    }

    pub(crate) fn update(
        &mut self,
        status: Option<&Status>,
        camera: &mut mc_render::Camera,
        renderer: &mc_render::Renderer,
        audio: &crate::audio::Audio,
        dt: f32,
    ) {
        let line = status.and_then(|s| s.line.as_ref());
        if self.line.as_ref() != line {
            self.line = line.cloned();
            if line.is_some() {
                audio.play(crate::audio::Sfx::Radio);
            }
        }
        let Some(status) = status else { return };
        if let Some((target, start, _)) = &status.camera {
            if self.camera != Some(*start) {
                self.camera = Some(*start);
                self.pan = Some(Pan::new(camera));
            }
            if let Some(pan) = &mut self.pan {
                let at = glam::Vec2::from(target.at.to_f32());
                if pan.advance(camera, target, renderer.ground_height(at), dt) {
                    self.pan = None;
                }
            }
        } else {
            self.pan = None;
        }
    }

    pub(crate) fn draw(&self, ui: &mut Ui, status: Option<&Status>) {
        let Some(status) = status else { return };
        if !status.controls {
            ui.fill(Rect::new(0.0, 0.0, ui.size.x, 48.0), rgb(0x000000, 1.0));
            ui.fill(
                Rect::new(0.0, ui.size.y - 48.0, ui.size.x, 48.0),
                rgb(0x000000, 1.0),
            );
        }
        if status.controls {
            self.objectives(ui, status);
        }
        let line = status.line.as_ref().and_then(|id| self.strings.get(id));
        if let Some(line) = line {
            let width = (ui.size.x - 440.0).min(860.0);
            let x = (ui.size.x - width) * 0.5;
            let lines = ui.wrap(type_scale::BODY, &line.text, width - 40.0);
            let height = 54.0 + lines.len() as f32 * 20.0;
            let y = ui.size.y - 260.0 - height;
            ui.panel(Rect::new(x, y, width, height));
            ui.text(
                x + 20.0,
                y + 25.0,
                type_scale::CAPTION,
                rgb(palette::ACCENT, 1.0),
                &line.speaker,
            );
            for (i, text) in lines.iter().enumerate() {
                ui.text(
                    x + 20.0,
                    y + 51.0 + i as f32 * 20.0,
                    type_scale::BODY,
                    rgb(palette::TEXT, 1.0),
                    text,
                );
            }
        } else if let Some(error) = &self.error {
            ui.text_fit_left(
                30.0,
                110.0,
                ui.size.x - 450.0,
                type_scale::BODY,
                rgb(palette::BAD, 1.0),
                error,
            );
        }
    }
    fn objectives(&self, ui: &mut Ui, status: &Status) {
        let x = 20.0;
        let active: Vec<_> = status
            .objectives
            .iter()
            .filter(|(_, s)| *s != Objective::Complete)
            .collect();
        ui.panel(Rect::new(
            x,
            100.0,
            360.0,
            58.0 + active.len() as f32 * 52.0,
        ));
        ui.text(
            x + 16.0,
            125.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            "THE CONTRACT",
        );
        ui.text(
            x + 16.0,
            143.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("Vela • checkpoint {}", status.beat),
        );
        for (i, (id, state)) in active.iter().enumerate() {
            let text = self
                .strings
                .get(id.as_str())
                .map_or(id.as_str(), |s| s.text.as_str());
            let color = if *state == Objective::Failed {
                palette::BAD
            } else {
                palette::TEXT
            };
            let wrapped = ui.wrap(type_scale::BODY, text, 325.0);
            for (j, line) in wrapped.iter().enumerate() {
                ui.text(
                    x + 16.0,
                    174.0 + i as f32 * 52.0 + j as f32 * 17.0,
                    type_scale::BODY,
                    rgb(color, 1.0),
                    line,
                );
            }
        }
    }
}

pub(crate) fn start(
    blueprints: &mc_data::Blueprints,
    resume: bool,
) -> Result<crate::game::GameStart, String> {
    let path = crate::setup::find_map(Some("vela_m1")).map_err(|_| {
        "Vela terrain is not baked. Run scripts/vela.sh once, then open The Contract.".to_owned()
    })?;
    let map = std::sync::Arc::new(mc_map::MapFile::open(path).map_err(|e| e.to_string())?);
    let mission = definition(&map)?.ok_or("Vela mission assets are missing")?;
    mission
        .validate(blueprints, 6, map.info().size_metres(), map.props().len())
        .map_err(|e| e.to_string())?;
    let checkpoint = if resume {
        Some(crate::campaign_checkpoints::Checkpoint::load(
            &map, blueprints,
        )?)
    } else {
        None
    };
    let request = crate::ui::setup::MatchRequest {
        map,
        config: config(),
        colors: crate::setup::TEAM_COLORS,
        survival: None,
        sky: Default::default(),
    };
    crate::app::local_checkpoint_start(request, blueprints, false, Some(mission), checkpoint)
}

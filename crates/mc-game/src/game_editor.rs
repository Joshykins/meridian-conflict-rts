//! Playtest presentation and debug intent; simulation changes use normal commands.
use super::*;
#[path = "game_editor_ui.rs"]
mod panel;
use serde::Deserialize;
#[derive(Clone, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DebugAction {
    Reveal {
        on: bool,
    },
    Pause {
        on: bool,
    },
    Speed {
        percent: u32,
    },
    Jump {
        tick: u32,
    },
    Spawn {
        blueprint: String,
        owner: u8,
        at: [f32; 2],
        heading: f32,
        count: u16,
    },
}
pub(super) struct Tools {
    pub(super) reveal: bool,
    pub(super) typing: bool,
    open: bool,
    query: String,
    blueprint: String,
    owner: u8,
    count: u16,
    seconds: String,
    scroll: usize,
    target: Option<u32>,
    rect: Option<ui::Rect>,
    message: String,
}
impl Game {
    pub(crate) fn editor_test_state(&self) -> serde_json::Value {
        serde_json::json!({
            "tick": self.tick(),
            "local": self.view.local,
            "speed": self.view.speed,
            "paused": self.view.paused,
            "debug": self.editor_tools.as_ref().map(|t|serde_json::json!({"reveal":t.reveal,"target":t.target})),
            "units": self.view.frame.units.len(),
            "beams": self.view.frame.beams.len(),
            "dead_props": self.view.frame.props_dead.iter().map(|v| v.count_ones()).sum::<u32>(),
            "reclaimed_props": self.view.frame.city.iter().filter(|s| s.flags & mc_sim::city::RECLAIMED != 0).count(),
            "players": self.view.status.players.iter().map(|p| serde_json::json!({"name":p.name,"materials":p.materials})).collect::<Vec<_>>(),
        })
    }
    pub(crate) fn editor_camera(&mut self, camera: Camera) {
        self.camera = camera;
        self.editor_tools = Some(Tools {
            reveal: true,
            typing: false,
            open: true,
            query: String::new(),
            blueprint: self
                .blueprints
                .units
                .first()
                .map_or(String::new(), |b| b.key.clone()),
            owner: self.view.local,
            count: 1,
            seconds: "60".into(),
            scroll: 0,
            target: None,
            rect: None,
            message: "Spawn at the camera centre. Stop returns to the editor.".into(),
        });
    }
    pub(super) fn editor_covers(&self, cursor: Vec2) -> bool {
        self.hud.covers(cursor)
            || self
                .editor_tools
                .as_ref()
                .and_then(|t| t.rect)
                .is_some_and(|r| r.contains(cursor))
    }
    pub(crate) fn editor_debug(&mut self, action: DebugAction) -> Result<(), String> {
        if self.editor_tools.is_none() {
            return Err("Debug tools require an editor playtest".into());
        }
        match action {
            DebugAction::Reveal { on } => self.editor_tools.as_mut().unwrap().reveal = on,
            DebugAction::Pause { on } => {
                if !self.view.status.owns_clock {
                    return Err("Wait for the local playtest to start".into());
                }
                if self.view.paused != on {
                    self.toggle_pause();
                    self.sim.paused.store(on, Ordering::Relaxed);
                }
            }
            DebugAction::Speed { percent } => {
                if !(25..=1600).contains(&percent) {
                    return Err("Speed must be between 25 and 1600 percent".into());
                }
                if !self.view.status.owns_clock {
                    return Err("Wait for the local playtest to start".into());
                }
                self.set_speed(percent);
            }
            DebugAction::Jump { tick } => {
                if tick <= self.tick() || tick > 36_000 {
                    return Err(
                        "Jump target must be ahead of the current tick and within one hour".into(),
                    );
                }
                let tools = self.editor_tools.as_mut().unwrap();
                if tools.target.is_some_and(|t| t > self.view.status.tick) {
                    return Err("A time jump is already running".into());
                }
                tools.target = Some(tick);
                *self.sim.seek.lock().unwrap() = Some(tick);
            }
            DebugAction::Spawn {
                blueprint,
                owner,
                at,
                heading,
                count,
            } => {
                let bp = self
                    .blueprints
                    .unit_by_key(&blueprint)
                    .ok_or("Unknown spawn blueprint")?;
                let size = self.map.info().size_metres().to_f32();
                if usize::from(owner) >= self.view.status.players.len() {
                    return Err("Unknown spawn player slot".into());
                }
                if count == 0 || count > 64 {
                    return Err("Spawn 1 to 64 units per request".into());
                }
                if !heading.is_finite()
                    || at
                        .iter()
                        .zip(size)
                        .any(|(&v, max)| !v.is_finite() || v < 0.0 || v > max)
                {
                    return Err("Spawn position or heading is outside the map".into());
                }
                self.send(Command::DebugSpawn {
                    owner,
                    blueprint: bp.id,
                    pos: FxVec2::new(Fx::from_f32(at[0]), Fx::from_f32(at[1])),
                    heading: mc_core::Angle::from_radians_f32(heading.to_radians()),
                    count,
                    flags: 0,
                    build: 1000,
                });
            }
        }
        Ok(())
    }
}

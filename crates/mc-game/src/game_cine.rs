//! The free camera's input (Ctrl+Alt): what the keys and the mouse do while the
//! panels are folded away, handing the view to `cine.rs` and back.
//!
//! Pressing Ctrl while Alt already swings the view round a unit hands over
//! without a jump: the angle stays, and the unit stays locked in frame and
//! followed, so the shot can be worked on from there.

use super::Game;
use crate::audio::{Audio, Sfx};
use crate::cine::{Aim, Controls, World};
use crate::hud::free_camera::held;
use crate::pointer::Pointer;
use glam::{Vec2, Vec3};
use mc_render::camera::FOV_Y;
use mc_render::Renderer;
use mc_sim::mirror::KIND_WRECK;
use std::time::Instant;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Radians per pixel of mouse-look.
const LOOK: f32 = 0.0022;
/// Radians per pixel of Alt-orbit.
const ORBIT: f32 = 0.0032;
/// The pointer hides after this long still, so a frame has nothing over it.
const POINTER_IDLE: f32 = 1.8;

/// What the window should do with the pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CursorMode {
    #[default]
    Normal,
    /// Out of the picture.
    Hidden,
    /// Hidden and held in place, for mouse-look; raw motion drives the view.
    Grabbed,
}

/// The world as the camera sees it: ground heights and units between ticks.
pub(super) struct CineWorld<'a> {
    pub renderer: Option<&'a Renderer>,
    pub game: &'a Game,
    pub alpha: f32,
}

impl World for CineWorld<'_> {
    fn ground(&self, xy: Vec2) -> f32 {
        match self.renderer {
            Some(r) => r.ground_height(xy),
            None => 0.0,
        }
    }

    fn unit(&self, id: u32) -> Option<(Vec3, f32)> {
        self.game.unit_now(id, self.alpha)
    }
}

impl Game {
    /// A unit's interpolated position and radius; `None` if gone or a wreck.
    pub(super) fn unit_now(&self, id: u32, alpha: f32) -> Option<(Vec3, f32)> {
        let &i = self.view.index_of.get(&id)?;
        let u = &self.view.frame.units[i];
        if u.owner_flags & KIND_WRECK != 0 {
            return None;
        }
        let pos = Vec3::from(u.prev_pos) + (Vec3::from(u.pos) - Vec3::from(u.prev_pos)) * alpha;
        Some((pos, u.radius))
    }

    /// Frees the camera (the panels fold away) or gives the panels back.
    pub(super) fn set_free_camera(&mut self, on: bool, renderer: &Renderer, audio: &Audio) {
        if on == self.hud.free.on {
            return;
        }
        if on {
            // Whatever Alt was swinging round, and whatever was tracked, stays in frame.
            let unit = self.orbit_unit.or(self.track);
            self.cine.enter(&self.camera);
            self.cine.release();
            if let Some(id) = unit {
                if let Some((pos, _)) = self.unit_now(id, self.cine_alpha) {
                    self.cine.aim = Some(Aim::Unit(id));
                    self.cine.follow = Some((id, self.cine.goal.eye - pos));
                }
            }
            self.orbit_saved = None;
            self.orbit_return = None;
            self.orbit_unit = None;
            self.orbit_aim = None;
            self.orbit_from = None;
            self.orbit_pivot = None;
            self.track = None;
            self.zoom_target = None;
            self.zoom_velocity = 0.0;
            self.left_down = None;
            self.view.formation_panel = false;
            self.pointer_moved = Instant::now();
        } else {
            // The strategic camera nearest the free one, the player's from this frame on.
            let mut cine = std::mem::take(&mut self.cine);
            let mut camera = self.camera.clone();
            let track = cine.leave(
                &mut camera,
                &CineWorld {
                    renderer: Some(renderer),
                    game: self,
                    alpha: self.cine_alpha,
                },
            );
            self.cine = cine;
            self.camera = camera;
            self.track = track;
            // The focus height eases from the ground it landed on, not a jump.
            self.focus_eased_at = Some(self.camera.focus.truncate());
            self.cine.hand_back_apply(&mut self.camera, 0.0);
            self.right_down = false;
        }
        self.hud.free.toggle(audio);
    }

    /// The free camera's keys. Letters it has no use for do nothing: they would
    /// give orders with no panel to show them. `true` if the key was taken.
    pub(super) fn free_camera_key(
        &mut self,
        code: KeyCode,
        renderer: &Renderer,
        audio: &Audio,
    ) -> bool {
        let digit = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ]
        .iter()
        .position(|k| *k == code);
        if let Some(slot) = digit {
            if self.ctrl {
                self.cine.save(slot);
                self.hud.free.flash(slot);
                audio.play(Sfx::Select);
            } else if self.cine.recall(slot, self.shift) {
                audio.play(Sfx::Tick);
            } else {
                audio.play(Sfx::Deny);
            }
            self.hud.free.poke();
            return true;
        }
        let tick = |on: bool| if on { Sfx::ToggleOn } else { Sfx::ToggleOff };
        match code {
            KeyCode::Escape => {
                if self.cine.playing.is_some() {
                    self.cine.play();
                } else if self.cine.release() {
                    audio.play(Sfx::Back);
                } else {
                    self.set_free_camera(false, renderer, audio);
                }
            }
            KeyCode::KeyH => {
                self.hud.free.toggle_guide();
                audio.play(Sfx::Tick);
            }
            KeyCode::KeyF => {
                let target = self
                    .unit_at(self.cursor)
                    .map(|i| self.view.frame.units[i].unit_id)
                    .and_then(|id| self.unit_now(id, 1.0))
                    .or_else(|| self.cine_ground.map(|g| (g, 0.0)));
                match target {
                    Some((at, radius)) => {
                        self.cine.frame(at, radius);
                        audio.play(Sfx::Select);
                    }
                    None => audio.play(Sfx::Deny),
                }
            }
            KeyCode::KeyT => {
                // Follow: the unit locked on, or the one under the pointer.
                if self.cine.follow.is_some() {
                    self.cine.follow = None;
                    audio.play(Sfx::ToggleOff);
                } else {
                    let id = match self.cine.aim {
                        Some(Aim::Unit(id)) => Some(id),
                        _ => self
                            .unit_at(self.cursor)
                            .map(|i| self.view.frame.units[i].unit_id),
                    };
                    match id.and_then(|id| self.unit_now(id, 1.0).map(|(p, _)| (id, p))) {
                        Some((id, pos)) => {
                            self.cine.aim = Some(Aim::Unit(id));
                            self.cine.follow = Some((id, self.cine.goal.eye - pos));
                            audio.play(Sfx::ToggleOn);
                        }
                        None => audio.play(Sfx::Deny),
                    }
                }
            }
            KeyCode::KeyL => {
                self.cine.locked = !self.cine.locked;
                audio.play(tick(self.cine.locked));
            }
            KeyCode::KeyN => {
                self.cine.smooth = self.cine.smooth.next();
                audio.play(Sfx::Tick);
            }
            KeyCode::KeyB => {
                self.hud.free.bars = !self.hud.free.bars;
                audio.play(tick(self.hud.free.bars));
            }
            KeyCode::KeyG => {
                self.cine.grid = !self.cine.grid;
                audio.play(tick(self.cine.grid));
            }
            KeyCode::KeyP => {
                if self.cine.play() {
                    audio.play(tick(self.cine.playing.is_some()));
                } else {
                    audio.play(Sfx::Deny);
                }
            }
            KeyCode::KeyR => {
                // The lens back to normal and the horizon level, eased.
                let mut to = self.cine.goal;
                to.fov = FOV_Y;
                if self.cine.aim.is_none() {
                    to.pitch = to.pitch.clamp(0.05, 0.9);
                }
                self.cine.glide_to(to);
                self.cine.speed = 1.0;
                audio.play(Sfx::Tick);
            }
            KeyCode::Home => {
                let player = self.view.local;
                let acu = self.view.frame.units.iter().find(|u| {
                    (u.owner_flags & 0xFF) as u8 == player
                        && u.owner_flags & KIND_WRECK == 0
                        && self
                            .blueprints
                            .unit(mc_data::BlueprintId(u.blueprint as u16))
                            .has(mc_data::cat::COMMANDER)
                });
                match acu {
                    Some(u) => self.cine.frame(Vec3::from(u.pos), u.radius),
                    None => audio.play(Sfx::Deny),
                }
            }
            c => return format!("{c:?}").starts_with("Key"),
        }
        self.hud.free.poke();
        true
    }

    /// A mouse button while the camera is free.
    pub(super) fn free_camera_button(&mut self, button: MouseButton, pressed: bool, audio: &Audio) {
        match (button, pressed) {
            (MouseButton::Right, p) => self.right_down = p,
            (MouseButton::Middle, p) => self.middle_down = p,
            (MouseButton::Left, true) => {
                if self.hud.covers(self.cursor) || self.cine.locked {
                    return;
                }
                // Lock on: a unit, the ground, or (on the sky) let go.
                let unit = self
                    .unit_at(self.cursor)
                    .map(|i| self.view.frame.units[i].unit_id)
                    .filter(|&id| self.unit_now(id, 1.0).is_some());
                let aim = match unit {
                    Some(id) if self.cine.aim == Some(Aim::Unit(id)) => None,
                    Some(id) => Some(Aim::Unit(id)),
                    None => self.cine_ground.map(Aim::Point),
                };
                if aim.is_none() {
                    self.cine.follow = None;
                } else if let (Some(Aim::Unit(id)), Some((fid, _))) = (aim, self.cine.follow) {
                    // Following one unit and locking on another: stop riding with the first.
                    if id != fid {
                        self.cine.follow = None;
                    }
                } else if matches!(aim, Some(Aim::Point(_))) {
                    self.cine.follow = None;
                }
                audio.play(if aim.is_some() {
                    Sfx::Select
                } else {
                    Sfx::ToggleOff
                });
                self.cine.aim = aim;
                self.cine.playing = None;
                self.hud.free.poke();
            }
            _ => {}
        }
    }

    /// Pointer motion, in pixels. Raw motion (when the window sends it) wins,
    /// so a held pointer still turns the view.
    pub(super) fn free_camera_motion(&mut self, delta: Vec2, raw: bool) {
        if raw {
            self.raw_motion = true;
        } else if self.raw_motion && self.cursor_mode() == CursorMode::Grabbed {
            return;
        }
        if raw && self.cursor_mode() != CursorMode::Grabbed {
            return;
        }
        if self.alt {
            self.cine_orbit += delta * ORBIT;
        } else if self.right_down {
            self.cine_look += delta * LOOK;
        } else if self.middle_down && !raw {
            self.cine_drag += delta;
        }
    }

    pub(super) fn free_camera_wheel(&mut self, lines: f32) {
        if self.right_down {
            self.cine_throttle += lines;
        } else {
            self.cine_dolly += lines;
        }
        self.pointer_moved = Instant::now();
    }

    /// Everything asked since the last frame, and the held keys.
    fn cine_controls(&mut self) -> Controls {
        let key = |k: KeyCode| self.keys.contains(&k);
        let axis = |a: bool, b: bool| (a as i32 - b as i32) as f32;
        let alt = self.alt && !self.ctrl;
        let c = Controls {
            fly: Vec3::new(
                axis(
                    key(KeyCode::KeyD) || key(KeyCode::ArrowRight),
                    key(KeyCode::KeyA) || key(KeyCode::ArrowLeft),
                ),
                axis(
                    key(KeyCode::KeyW) || key(KeyCode::ArrowUp),
                    key(KeyCode::KeyS) || key(KeyCode::ArrowDown),
                ),
                axis(
                    key(KeyCode::KeyE) || key(KeyCode::PageUp),
                    key(KeyCode::KeyQ) || key(KeyCode::PageDown),
                ),
            ),
            fast: self.shift,
            look: std::mem::take(&mut self.cine_look),
            orbit: std::mem::take(&mut self.cine_orbit),
            orbiting: alt,
            dolly: std::mem::take(&mut self.cine_dolly),
            throttle: std::mem::take(&mut self.cine_throttle),
            lens: axis(key(KeyCode::KeyZ), key(KeyCode::KeyX)),
            drag: std::mem::take(&mut self.cine_drag),
        };
        if c.fly != Vec3::ZERO || c.lens != 0.0 {
            self.hud.free.poke_quiet();
        }
        c
    }

    /// Runs the flight and puts it on the camera, once the frame's units are in.
    pub(super) fn cine_frame(&mut self, renderer: &Renderer, dt: f32, alpha: f32) {
        let controls = self.cine_controls();
        let mut cine = std::mem::take(&mut self.cine);
        let mut camera = self.camera.clone();
        {
            let world = CineWorld {
                renderer: Some(renderer),
                game: self,
                alpha,
            };
            cine.update(dt, &controls, &world, camera.map_size);
            cine.apply(&mut camera, &world);
        }
        self.cine = cine;
        self.camera = camera;
        self.cine_ground = self.ground_under_cursor(renderer);
        self.hud.free.held = self.free_camera_held();
        self.cine_status(alpha);
    }

    /// What the guide shows of the camera.
    fn cine_status(&mut self, alpha: f32) {
        let name_of = |id: u32| {
            self.view.index_of.get(&id).map(|&i| {
                let u = &self.view.frame.units[i];
                self.blueprints
                    .unit(mc_data::BlueprintId(u.blueprint as u16))
                    .name
                    .clone()
            })
        };
        let at = match self.cine.aim {
            Some(Aim::Unit(id)) => self.unit_now(id, alpha).map(|(p, r)| (p, r.max(3.0))),
            Some(Aim::Point(p)) => Some((p, 6.0)),
            None => None,
        };
        let aim_at = at.and_then(|(p, r)| {
            let px = self.camera.project(p)?;
            let size = r * self.camera.projection_scale() / self.camera.eye().distance(p).max(1.0);
            Some((px, size))
        });
        let aim = match self.cine.aim {
            Some(Aim::Unit(id)) => name_of(id).or(Some("Unit".into())),
            Some(Aim::Point(_)) => Some("Ground".into()),
            None => None,
        };
        let pointer_live = self.pointer_moved.elapsed().as_secs_f32() < POINTER_IDLE;
        let s = &mut self.hud.free.status;
        s.speed = self.cine.speed;
        s.focal = crate::cine::focal_mm(self.cine.shown.fov);
        s.smoothing = self.cine.smooth.label();
        s.locked = self.cine.locked;
        s.grid = self.cine.grid;
        s.following = self.cine.follow.is_some();
        s.playing = self.cine.playing;
        s.at_shot = self.cine.at_shot;
        s.shots = self.cine.shots.map(|p| p.is_some());
        s.game_speed = self.view.speed;
        s.paused = self.view.paused;
        s.aim = aim;
        s.aim_at = aim_at;
        s.pointer_live = pointer_live;
    }

    /// Which of the free camera's keys are down, so its guide can light them.
    fn free_camera_held(&self) -> u32 {
        use held::*;
        let any = |keys: &[KeyCode]| keys.iter().any(|k| self.keys.contains(k));
        let mut bits = 0;
        for (on, bit) in [
            (
                any(&[KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD]),
                FLY,
            ),
            (any(&[KeyCode::KeyE, KeyCode::KeyQ]), RISE),
            (self.shift, FAST),
            (self.cine_dolly != 0.0, DOLLY),
            (self.right_down, MOUSELOOK),
            (self.alt && !self.ctrl, ORBIT),
            (self.middle_down, DRAG),
            (any(&[KeyCode::KeyZ, KeyCode::KeyX]), LENS),
            (any(&[KeyCode::KeyF]), FRAME),
            (self.cine.follow.is_some(), FOLLOW),
            (self.cine.aim.is_some(), AIM),
            (self.cine.locked, LOCK),
            (self.cine.playing.is_some(), PLAY),
            (self.cine.grid, GRID),
            (any(&[KeyCode::KeyR]), RESET),
            (any(&[KeyCode::KeyN]), SMOOTH),
            (self.ctrl, CTRL),
        ] {
            if on {
                bits |= bit;
            }
        }
        bits
    }

    /// Hidden while still, held for mouse-look and Alt-orbit.
    pub fn cursor_mode(&self) -> CursorMode {
        if !self.hud.free.on || self.menu.is_some() {
            return CursorMode::Normal;
        }
        if self.right_down || (self.alt && !self.ctrl) {
            return CursorMode::Grabbed;
        }
        if self.pointer_moved.elapsed().as_secs_f32() > POINTER_IDLE
            && !self.hud.covers(self.cursor)
        {
            return CursorMode::Hidden;
        }
        CursorMode::Normal
    }

    /// Raw mouse motion from the window, for mouse-look while the pointer is held.
    pub fn raw_mouse(&mut self, delta: Vec2) {
        if self.hud.free.on && self.menu.is_none() {
            self.free_camera_motion(delta, true);
        }
    }

    /// The pointer in the free camera: a unit under it can be locked on.
    pub(super) fn free_camera_pointer(&self) -> Pointer {
        if !self.hud.covers(self.cursor) && self.unit_at(self.cursor).is_some() {
            Pointer::Select
        } else {
            Pointer::Arrow
        }
    }
}

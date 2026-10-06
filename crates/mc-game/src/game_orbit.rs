//! The strategic camera's own controls: Alt-orbit and its drift home, and the
//! keys that pan, turn and tilt the view.

use super::Game;
use glam::Vec2;
use mc_render::Renderer;
use mc_sim::mirror::KIND_WRECK;
use winit::keyboard::KeyCode;

/// How fast Alt-orbit closes on the yaw, tilt and pivot the mouse asked for, per second.
const ORBIT_RATE: f32 = 18.0;

/// How fast the camera settles back to its old view after Alt comes up, per second.
/// Slow on purpose: the view drifts home over about a second rather than snapping.
const ORBIT_RETURN_RATE: f32 = 3.0;

impl Game {
    /// Hold Alt: swing around the unit under the pointer, or the tracked unit,
    /// or the ground under the pointer. Releasing Alt puts the camera back.
    pub(super) fn begin_orbit(&mut self, r: &Renderer) {
        if self.orbit_saved.is_some() {
            return;
        }
        // Alt again while still drifting home: home stays the view from before the first orbit.
        let home = match self.orbit_return.take() {
            Some((yaw, tilt, _)) if self.track.is_some() || !self.orbit_return_focus => {
                (yaw, tilt, self.camera.focus)
            }
            Some(home) => home,
            None => (self.camera.yaw, self.camera.tilt, self.camera.focus),
        };
        self.orbit_saved = Some(home);
        self.orbit_aim = Some((self.camera.yaw, self.camera.tilt));
        self.orbit_from = Some(self.camera.focus);
        self.orbit_blend = 0.0;
        self.orbit_unit = self
            .unit_at(self.cursor)
            .map(|i| &self.view.frame.units[i])
            .filter(|u| u.owner_flags & KIND_WRECK == 0)
            .map(|u| u.unit_id)
            .or(self.track);
        self.orbit_pivot = if self.orbit_unit.is_some() {
            None
        } else {
            self.ground_under_cursor(r)
        };
    }

    pub(super) fn end_orbit(&mut self) {
        self.orbit_unit = None;
        self.orbit_aim = None;
        self.orbit_from = None;
        self.orbit_pivot = None;
        self.orbit_blend = 0.0;
        if let Some((yaw, tilt, focus)) = self.orbit_saved.take() {
            // Drift back the short way round, however many turns the orbit wound up.
            let turn = std::f32::consts::TAU;
            let yaw = self.camera.yaw + (yaw - self.camera.yaw + turn * 0.5).rem_euclid(turn)
                - turn * 0.5;
            self.orbit_return = Some((yaw, tilt, focus));
            self.orbit_return_far = true;
            self.orbit_return_focus = true;
        }
    }

    /// Hands the focus back to the player after a pan or a jump; the yaw and tilt
    /// still ease home, so the orbit's angle never sticks.
    pub(super) fn cancel_orbit_return(&mut self) {
        self.orbit_return_focus = false;
    }

    /// Closes a fixed share of the remaining orbit every frame, so mouse motion
    /// is smooth at every frame rate and does not jump when events bunch up.
    pub(super) fn ease_orbit(&mut self, dt: f32) {
        self.orbit_return_share = 0.0;
        if let Some((yaw, tilt, focus)) = self.orbit_return {
            let share = 1.0 - (-dt * ORBIT_RETURN_RATE).exp();
            self.orbit_return_share = share;
            self.camera.yaw += (yaw - self.camera.yaw) * share;
            self.camera.tilt += (tilt - self.camera.tilt) * share;
            // Tracking brings the focus home to the unit in `follow_camera`.
            let mut settled =
                (yaw - self.camera.yaw).abs() < 1e-3 && (tilt - self.camera.tilt).abs() < 1e-3;
            if self.track.is_some() {
                settled &= !self.orbit_return_far;
            } else if self.orbit_return_focus {
                self.camera.focus = self.camera.focus.lerp(focus, share);
                self.camera.clamp_focus();
                settled &= self.camera.focus.truncate().distance(focus.truncate()) < 0.05;
            }
            if settled {
                self.camera.yaw = yaw;
                self.camera.tilt = tilt;
                if self.track.is_none() && self.orbit_return_focus {
                    self.camera.focus = focus;
                    self.camera.clamp_focus();
                }
                self.orbit_return = None;
            }
        }
        if self.orbit_saved.is_none() {
            return;
        }
        let share = 1.0 - (-dt * ORBIT_RATE).exp();
        if let Some((yaw, tilt)) = &mut self.orbit_aim {
            let (lo, hi) = self.camera.tilt_limits();
            *tilt = tilt.clamp(lo, hi);
            self.camera.yaw += (*yaw - self.camera.yaw) * share;
            self.camera.tilt += (*tilt - self.camera.tilt) * share;
        }
        self.orbit_blend += (1.0 - self.orbit_blend) * share;
    }

    /// WASD/arrows pan, Q/E turn, Page Up/Down tilt.
    pub(super) fn keyboard_camera(&mut self, dt: f32) {
        let mut pan = Vec2::ZERO;
        for (key, d) in [
            (KeyCode::KeyW, Vec2::Y),
            (KeyCode::ArrowUp, Vec2::Y),
            (KeyCode::KeyS, -Vec2::Y),
            (KeyCode::ArrowDown, -Vec2::Y),
            (KeyCode::KeyA, Vec2::X),
            (KeyCode::ArrowLeft, Vec2::X),
            (KeyCode::KeyD, -Vec2::X),
            (KeyCode::ArrowRight, -Vec2::X),
        ] {
            if self.keys.contains(&key) {
                pan += d;
            }
        }
        if pan != Vec2::ZERO {
            self.pan_camera(pan * dt * 900.0);
        }
        let tilt_up = self.keys.contains(&KeyCode::PageUp);
        let tilt_down = self.keys.contains(&KeyCode::PageDown);
        let turn_left = self.keys.contains(&KeyCode::KeyQ);
        let turn_right = self.keys.contains(&KeyCode::KeyE);
        if turn_left {
            self.camera.yaw -= dt * 1.4;
        }
        if turn_right {
            self.camera.yaw += dt * 1.4;
        }
        if tilt_up {
            self.camera.orbit(0.0, dt * 0.8);
        }
        if tilt_down {
            self.camera.orbit(0.0, -dt * 0.8);
        }
        self.keep_hand_angle(turn_left || turn_right, tilt_up || tilt_down);
    }

    /// Turning or tilting by hand is the player's angle: the drift home lets go
    /// of that part and keeps easing the rest. Dropping the whole drift on a
    /// turn left the orbit's tilt stuck partway home.
    fn keep_hand_angle(&mut self, turned: bool, tilted: bool) {
        if let Some((yaw, tilt, _)) = &mut self.orbit_return {
            if turned {
                *yaw = self.camera.yaw;
            }
            if tilted {
                *tilt = self.camera.tilt;
            }
        }
    }
}

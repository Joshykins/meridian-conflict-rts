//! The interface's view of a window's events: the game's window and the crash
//! screen's read them the same way.

use super::{Input, Key};
use glam::Vec2;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

impl Input {
    /// Takes in one window event. `editing`: a text field has the keyboard, so W
    /// and S type rather than steer.
    pub fn feed(&mut self, event: &WindowEvent, editing: bool) {
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                self.scroll += match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Vec2::new(position.x as f32, position.y as f32)
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.down = *state == ElementState::Pressed;
                self.pressed |= self.down;
                self.released |= !self.down;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => self.right_pressed = true,
            WindowEvent::ModifiersChanged(m) => {
                let m = m.state();
                // Ctrl+Alt is AltGr on European layouts: that types characters.
                self.command = (m.control_key() && !m.alt_key()) || m.super_key();
            }
            WindowEvent::KeyboardInput { event: key, .. } if key.state == ElementState::Pressed => {
                if let PhysicalKey::Code(code) = key.physical_key {
                    let mapped = match code {
                        KeyCode::KeyA if self.command => Some(Key::SelectAll),
                        KeyCode::KeyC if self.command => Some(Key::Copy),
                        KeyCode::KeyX if self.command => Some(Key::Cut),
                        KeyCode::KeyV if self.command => Some(Key::Paste),
                        KeyCode::ArrowUp | KeyCode::KeyW => Some(Key::Up),
                        KeyCode::ArrowDown | KeyCode::KeyS => Some(Key::Down),
                        KeyCode::ArrowLeft => Some(Key::Left),
                        KeyCode::ArrowRight => Some(Key::Right),
                        KeyCode::Enter | KeyCode::NumpadEnter => Some(Key::Enter),
                        KeyCode::Escape => Some(Key::Escape),
                        KeyCode::Backspace => Some(Key::Backspace),
                        KeyCode::Tab => Some(Key::Tab),
                        _ => None,
                    };
                    // W and S steer menus only while nothing is being typed.
                    let letter = matches!(code, KeyCode::KeyW | KeyCode::KeyS);
                    if let Some(k) = mapped.filter(|_| !(letter && editing)) {
                        if !key.repeat || k == Key::Backspace {
                            self.keys.push(k);
                        }
                    }
                }
                // A shortcut types nothing, whatever text the platform attaches to it.
                if let Some(text) = key.text.as_ref().filter(|_| !self.command) {
                    self.typed.push_str(text);
                }
            }
            _ => {}
        }
    }
}

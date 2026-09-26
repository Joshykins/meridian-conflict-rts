//! The computer keyboard as a piano, laid out like every tracker: the Z row
//! is the lower octave (with S D G H J as black keys), the Q row the upper
//! (with 2 3 5 6 7 9 0). Minus and equals shift the octave.

use eframe::egui::Key;

/// Semitones above the base octave's C for a key, if it is a piano key.
pub fn semitone(key: Key) -> Option<i32> {
    Some(match key {
        Key::Z => 0,
        Key::S => 1,
        Key::X => 2,
        Key::D => 3,
        Key::C => 4,
        Key::V => 5,
        Key::G => 6,
        Key::B => 7,
        Key::H => 8,
        Key::N => 9,
        Key::J => 10,
        Key::M => 11,
        Key::Comma => 12,
        Key::Q => 12,
        Key::Num2 => 13,
        Key::W => 14,
        Key::Num3 => 15,
        Key::E => 16,
        Key::R => 17,
        Key::Num5 => 18,
        Key::T => 19,
        Key::Num6 => 20,
        Key::Y => 21,
        Key::Num7 => 22,
        Key::U => 23,
        Key::I => 24,
        Key::Num9 => 25,
        Key::O => 26,
        Key::Num0 => 27,
        Key::P => 28,
        _ => return None,
    })
}

/// Keys held on the computer keyboard and the MIDI note each one started,
/// so a release after an octave shift still stops the right note.
#[derive(Default)]
pub struct KeyPiano {
    /// The base C: 48 = C3.
    pub base: i32,
    pub velocity: u8,
    held: Vec<(Key, u8)>,
}

pub enum KeyEvent {
    On(u8),
    Off(u8),
}

impl KeyPiano {
    pub fn new() -> KeyPiano {
        KeyPiano {
            base: 48,
            velocity: 100,
            held: Vec::new(),
        }
    }

    /// Handles one key event; returns what to play.
    pub fn key(&mut self, key: Key, pressed: bool, repeat: bool) -> Option<KeyEvent> {
        if pressed {
            match key {
                Key::Minus => {
                    self.base = (self.base - 12).max(0);
                    return None;
                }
                Key::Equals | Key::Plus => {
                    self.base = (self.base + 12).min(108);
                    return None;
                }
                _ => {}
            }
        }
        let s = semitone(key)?;
        if pressed {
            if repeat || self.held.iter().any(|(k, _)| *k == key) {
                return None;
            }
            let note = (self.base + s).clamp(0, 127) as u8;
            self.held.push((key, note));
            Some(KeyEvent::On(note))
        } else {
            let i = self.held.iter().position(|(k, _)| *k == key)?;
            let (_, note) = self.held.remove(i);
            Some(KeyEvent::Off(note))
        }
    }

    /// Releases everything (the window lost focus, a text field took the keyboard).
    pub fn release_all(&mut self) -> Vec<u8> {
        self.held.drain(..).map(|(_, n)| n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_and_releases_across_an_octave_shift() {
        let mut p = KeyPiano::new();
        assert!(matches!(p.key(Key::Z, true, false), Some(KeyEvent::On(48))));
        assert!(p.key(Key::Z, true, true).is_none());
        p.key(Key::Equals, true, false);
        assert_eq!(p.base, 60);
        // The held key still releases the note it started.
        assert!(matches!(
            p.key(Key::Z, false, false),
            Some(KeyEvent::Off(48))
        ));
        assert!(matches!(p.key(Key::Q, true, false), Some(KeyEvent::On(72))));
        assert_eq!(p.release_all(), vec![72]);
        assert!(p.key(Key::F1, true, false).is_none());
    }
}

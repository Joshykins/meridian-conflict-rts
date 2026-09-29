//! Text entry: the one-line field and the wrapping text area, and the keys
//! both take (typing, Backspace, Ctrl+A/C/X/V).

use super::{ink, palette, rgb, type_scale, Id, Key, Rect, Style, Ui};
use crate::audio::Sfx;
use glam::Vec2;

/// A text area's inside margin and line pitch.
const AREA_PAD: f32 = 8.0;
const AREA_LINE: f32 = 19.0;

impl Ui<'_> {
    /// A single-line text field. Returns true when the text changed.
    pub fn text_field(&mut self, id: Id, r: Rect, text: &mut String, max: usize) -> bool {
        let (editing, changed, glow) = self.take_keyboard(id, r, text, max);
        let focus = self.ease(id ^ 5, if editing { 1.0 } else { 0.0 }, 16.0);
        self.fill(r, ink(0.55 + 0.2 * focus));
        let selected = editing && self.mem.selected == Some(id) && !text.is_empty();
        if selected {
            let w = self.text_width(type_scale::VALUE, text);
            self.fill(
                Rect::new(r.x + 10.0, r.mid_y() - 11.0, w + 4.0, 22.0),
                rgb(palette::ACCENT, 0.35),
            );
        }
        self.frame(
            r,
            rgb(
                if editing {
                    palette::ACCENT
                } else {
                    palette::LINE
                },
                0.2 + 0.3 * glow + 0.5 * focus,
            ),
        );
        let end = self.text(
            r.x + 12.0,
            r.mid_y(),
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            text,
        );
        if editing && !selected && (self.time * 1.6).fract() < 0.55 {
            self.fill(
                Rect::new(end + 2.0, r.mid_y() - 8.0, 2.0, 16.0),
                rgb(palette::ACCENT, 1.0),
            );
        }
        changed
    }

    /// Applies this frame's keys to the field that has the keyboard: typing,
    /// Backspace, and Ctrl+A/C/X/V. Returns true when the text changed.
    fn edit_text(&mut self, id: Id, text: &mut String, max: usize) -> bool {
        let fits = |ch: char| ch.is_ascii_graphic() || ch == ' ';
        let mut all = self.mem.selected == Some(id);
        let mut changed = false;
        if self.input.key(Key::SelectAll) {
            all = true;
        }
        // With nothing selected, Copy takes the whole field: it is one line with
        // the caret always at its end.
        if (self.input.key(Key::Copy) || (all && self.input.key(Key::Cut))) && !text.is_empty() {
            if let Err(e) = crate::clipboard::copy(text) {
                log::warn!("could not copy to the clipboard: {e}");
            }
        }
        let paste = if self.input.key(Key::Paste) {
            crate::clipboard::paste()
        } else {
            None
        };
        // What replaces a selection: a cut, a delete, or anything typed or pasted.
        let incoming = self
            .input
            .typed
            .chars()
            .chain(paste.iter().flat_map(|p| p.chars()));
        let mut incoming = incoming.filter(|&ch| fits(ch)).peekable();
        if all
            && (incoming.peek().is_some()
                || self.input.key(Key::Cut)
                || self.input.key(Key::Backspace))
        {
            changed |= !text.is_empty();
            text.clear();
            all = false;
        } else if self.input.key(Key::Backspace) && text.pop().is_some() {
            changed = true;
        }
        for ch in incoming {
            if text.chars().count() >= max {
                break;
            }
            text.push(ch);
            changed = true;
        }
        self.mem.selected = all.then_some(id);
        if self.input.key(Key::Enter) || self.input.key(Key::Escape) {
            self.mem.editing = None;
        }
        if changed {
            self.audio.play(Sfx::Tick);
        }
        changed
    }

    /// How tall a text area `w` wide is with `text` in it: it grows a line at a
    /// time from `min_lines` up to `max_lines`.
    pub fn text_area_height(
        &mut self,
        w: f32,
        text: &str,
        min_lines: usize,
        max_lines: usize,
    ) -> f32 {
        let lines = self
            .hard_wrap(type_scale::VALUE, text, w - 2.0 * AREA_PAD - 4.0)
            .len()
            .clamp(min_lines.max(1), max_lines.max(1));
        2.0 * AREA_PAD + lines as f32 * AREA_LINE
    }

    /// A text field for long notes: the text wraps between words (a word too
    /// long for a line breaks inside it), and past what `r` holds the last
    /// lines show, where the caret is. Size `r` with `text_area_height`.
    /// Returns true when the text changed.
    pub fn text_area(&mut self, id: Id, r: Rect, text: &mut String, max: usize) -> bool {
        let (editing, changed, glow) = self.take_keyboard(id, r, text, max);
        let focus = self.ease(id ^ 5, if editing { 1.0 } else { 0.0 }, 16.0);
        self.fill(r, ink(0.55 + 0.2 * focus));
        let tone = if editing {
            palette::ACCENT
        } else {
            palette::LINE
        };
        self.frame(r, rgb(tone, 0.2 + 0.3 * glow + 0.5 * focus));
        let st = type_scale::VALUE;
        let lines = self.hard_wrap(st, text, r.w - 2.0 * AREA_PAD - 4.0);
        let room = (((r.h - 2.0 * AREA_PAD) / AREA_LINE).round() as usize).max(1);
        let first = lines.len().saturating_sub(room);
        let selected = editing && self.mem.selected == Some(id) && !text.is_empty();
        let x = r.x + AREA_PAD + 4.0;
        let mut end = Vec2::new(x, r.y + AREA_PAD + AREA_LINE * 0.5);
        for (row, line) in lines[first..].iter().enumerate() {
            let y = r.y + AREA_PAD + (row as f32 + 0.5) * AREA_LINE;
            if selected {
                let w = self.text_width(st, line);
                self.fill(
                    Rect::new(x - 2.0, y - AREA_LINE * 0.5, w + 4.0, AREA_LINE),
                    rgb(palette::ACCENT, 0.35),
                );
            }
            let right = self.text(x, y, st, rgb(palette::TEXT, 1.0), line);
            end = Vec2::new(right, y);
        }
        if editing && !selected && (self.time * 1.6).fract() < 0.55 {
            self.fill(
                Rect::new(end.x + 2.0, end.y - 8.0, 2.0, 16.0),
                rgb(palette::ACCENT, 1.0),
            );
        }
        changed
    }

    /// `text` in lines no wider than `width`: broken between words, a word too
    /// wide for a line on its own broken where it runs out, spaces kept (so the
    /// caret after a typed space sits after it). Never empty.
    fn hard_wrap(&mut self, st: Style, text: &str, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for (i, word) in text.split(' ').enumerate() {
            let trial = if i == 0 {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if self.text_width(st, &trial) <= width || line.is_empty() && i == 0 {
                line = trial;
            } else {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            }
            // A word wider than a line: as much as fits, then the rest.
            while self.text_width(st, &line) > width && line.chars().count() > 1 {
                let chars: Vec<char> = line.chars().collect();
                let mut n = chars.len() - 1;
                while n > 1 && self.text_width(st, &chars[..n].iter().collect::<String>()) > width {
                    n -= 1;
                }
                lines.push(chars[..n].iter().collect());
                line = chars[n..].iter().collect();
            }
        }
        lines.push(line);
        lines
    }

    /// The click and keyboard handling every text control shares: a click takes
    /// the keyboard, a click elsewhere gives it up, and while it is held this
    /// frame's keys edit `text`. Returns (has the keyboard, changed, hover glow).
    fn take_keyboard(
        &mut self,
        id: Id,
        r: Rect,
        text: &mut String,
        max: usize,
    ) -> (bool, bool, f32) {
        let res = self.interact(id, r, true);
        if res.clicked {
            self.mem.editing = Some(id);
            self.audio.play(Sfx::Select);
        } else if self.input.pressed && !res.hovered && self.mem.editing == Some(id) {
            self.mem.editing = None;
        }
        let editing = self.mem.editing == Some(id);
        if res.clicked || !editing {
            // A click places the caret back at the end.
            if self.mem.selected == Some(id) {
                self.mem.selected = None;
            }
        }
        let changed = editing && self.edit_text(id, text, max);
        (editing, changed, res.glow)
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    /// One frame of a field that already has the keyboard.
    fn edit(mem: &mut Memory, input: Input, text: &mut String) {
        let mut o = Overlay::default();
        let audio = Audio::silent();
        let field = id("field", 0);
        mem.editing = Some(field);
        mem.begin_frame();
        let mut ui = Ui::new(
            &mut o,
            &input,
            mem,
            &audio,
            Vec2::new(1920.0, 1080.0),
            1.0,
            0.0,
            0.016,
        );
        ui.text_field(field, Rect::new(100.0, 100.0, 400.0, 40.0), text, 8);
    }

    fn keys(keys: &[Key]) -> Input {
        Input {
            keys: keys.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn select_all_is_replaced_by_typing_and_cleared_by_backspace() {
        let mut mem = Memory::default();
        let mut text = String::new();
        edit(
            &mut mem,
            Input {
                typed: "abc".into(),
                ..Default::default()
            },
            &mut text,
        );
        assert_eq!(text, "abc");
        edit(&mut mem, keys(&[Key::SelectAll]), &mut text);
        // Selecting changes nothing until something replaces it.
        edit(&mut mem, Input::default(), &mut text);
        assert_eq!(text, "abc");
        edit(
            &mut mem,
            Input {
                typed: "xy".into(),
                ..Default::default()
            },
            &mut text,
        );
        assert_eq!(text, "xy");
        // The selection went with the replace: this appends.
        edit(
            &mut mem,
            Input {
                typed: "z".into(),
                ..Default::default()
            },
            &mut text,
        );
        assert_eq!(text, "xyz");
        edit(&mut mem, keys(&[Key::SelectAll]), &mut text);
        edit(&mut mem, keys(&[Key::Backspace]), &mut text);
        assert_eq!(text, "");
    }

    #[test]
    fn a_click_in_the_field_drops_the_selection() {
        let mut mem = Memory::default();
        let mut text = "abc".to_owned();
        edit(&mut mem, keys(&[Key::SelectAll]), &mut text);
        edit(
            &mut mem,
            Input {
                cursor: Vec2::new(200.0, 120.0),
                down: true,
                pressed: true,
                ..Default::default()
            },
            &mut text,
        );
        edit(
            &mut mem,
            Input {
                cursor: Vec2::new(200.0, 120.0),
                released: true,
                ..Default::default()
            },
            &mut text,
        );
        edit(
            &mut mem,
            Input {
                typed: "d".into(),
                ..Default::default()
            },
            &mut text,
        );
        assert_eq!(text, "abcd");
    }

    #[test]
    fn a_long_note_wraps_and_keeps_every_word() {
        let mut o = Overlay::default();
        let audio = Audio::silent();
        let input = Input::default();
        let mut mem = Memory::default();
        let mut ui = Ui::new(
            &mut o,
            &input,
            &mut mem,
            &audio,
            Vec2::new(1920.0, 1080.0),
            1.0,
            0.0,
            0.016,
        );
        let note = "the doors on the courier open but there is still geometry in the way \
                    of seeing the unit walk out aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let lines = ui.hard_wrap(type_scale::VALUE, note, 200.0);
        assert!(lines.len() > 3, "{lines:?}");
        for line in &lines {
            assert!(ui.text_width(type_scale::VALUE, line) <= 200.0, "{line:?}");
        }
        assert_eq!(
            lines.join(" ").split_whitespace().collect::<String>(),
            note.split_whitespace().collect::<String>()
        );
        // A typed space shows: the caret goes after it.
        assert!(ui.hard_wrap(type_scale::VALUE, "abc ", 200.0)[0].ends_with(' '));
        let short = ui.text_area_height(280.0, "", 2, 6);
        let long = ui.text_area_height(280.0, note, 2, 6);
        assert!(long > short);
        // It stops growing at the most lines.
        assert_eq!(
            ui.text_area_height(280.0, &note.repeat(9), 2, 6),
            ui.text_area_height(280.0, &note.repeat(20), 2, 6)
        );
    }
}

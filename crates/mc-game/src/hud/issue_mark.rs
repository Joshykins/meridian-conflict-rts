//! The profiler panel's report card: the match id (and a button that copies
//! it), a note, and Mark Issue, which writes this moment to the issue log
//! (`crate::issues`).

use super::Scene;
use crate::audio::Sfx;
use crate::issues::{self, MatchRecord};
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use glam::Vec2;
use std::fmt::Write;

/// Longest note, in characters.
const NOTE_MAX: usize = 120;
/// How long the result of a click stays under the button, in seconds.
const SAID_FOR: f32 = 6.0;

#[derive(Default)]
pub struct IssueMark {
    record: Option<MatchRecord>,
    note: String,
    marks: u32,
    /// The note field had the keyboard last frame: keys are not orders.
    typing: bool,
    /// What the last click did, whether it worked, and when (UI time).
    said: Option<(String, bool, f32)>,
}

impl IssueMark {
    pub fn new(record: Option<MatchRecord>) -> IssueMark {
        IssueMark {
            record,
            ..Default::default()
        }
    }

    pub fn typing(&self) -> bool {
        self.typing
    }

    /// Draws the card with its top-right corner at `corner`; returns what it covered.
    pub fn draw(&mut self, ui: &mut Ui, s: &Scene, corner: Vec2) -> Rect {
        let w = 300.0;
        let said = self
            .said
            .as_ref()
            .filter(|(_, _, at)| ui.time - at < SAID_FOR)
            .cloned();
        let r = Rect::new(
            corner.x - w,
            corner.y,
            w,
            148.0 + if said.is_some() { 18.0 } else { 0.0 },
        );
        ui.fill(r, ink(0.7));
        ui.frame(r, rgb(palette::LINE, 0.12));
        ui.section(r.x + 12.0, r.y + 14.0, w - 24.0, "Report");

        let y = r.y + 40.0;
        ui.text(
            r.x + 12.0,
            y,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Match",
        );
        let shown = self
            .record
            .as_ref()
            .map_or("not recorded", |m| m.id.as_str());
        ui.text(
            r.x + 64.0,
            y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            shown,
        );
        let copy = Rect::new(r.right() - 12.0 - 62.0, y - 12.0, 62.0, 24.0);
        let can_copy = self.record.is_some();
        if ui.button(
            id("issue-copy", 0),
            copy,
            "Copy",
            ButtonKind::Secondary,
            can_copy,
        ) {
            if let Some(m) = &self.record {
                ui.audio.play(Sfx::Select);
                self.said = Some(match issues::copy(&m.id) {
                    Ok(()) => (format!("Copied {}", m.id), true, ui.time),
                    Err(e) => (format!("Could not copy: {e}"), false, ui.time),
                });
            }
        }

        let field = Rect::new(r.x + 12.0, y + 20.0, w - 24.0, 30.0);
        let note_id = id("issue-note", 0);
        // Enter in the field marks, as the button does.
        let entered = ui.mem.editing == Some(note_id) && ui.input.key(Key::Enter);
        ui.text_field(note_id, field, &mut self.note, NOTE_MAX);
        self.typing = ui.mem.editing == Some(note_id);
        if self.note.is_empty() && !self.typing {
            ui.text(
                field.x + 12.0,
                field.mid_y(),
                type_scale::VALUE,
                rgb(palette::FAINT, 1.0),
                "What's wrong? (optional)",
            );
        }

        let button = Rect::new(r.x + 12.0, field.bottom() + 10.0, w - 24.0, 32.0);
        let clicked = ui.button(
            id("issue-mark", 0),
            button,
            "Mark Issue",
            ButtonKind::Primary,
            true,
        );
        if clicked || entered {
            ui.audio.play(Sfx::Select);
            self.marks += 1;
            let entry = self.entry(s);
            self.said = Some(match issues::append(&entry) {
                Ok(path) => {
                    self.note.clear();
                    log::info!("issue marked:\n{entry}");
                    let t = s.view.status.tick / mc_core::TICKS_PER_SECOND;
                    (
                        format!(
                            "Marked #{} at {}:{:02}  \u{b7}  {}",
                            self.marks,
                            t / 60,
                            t % 60,
                            path.display()
                        ),
                        true,
                        ui.time,
                    )
                }
                Err(e) => {
                    self.marks -= 1;
                    (format!("Could not write the mark: {e}"), false, ui.time)
                }
            });
        }
        if let Some((text, ok, _)) = &said {
            ui.text_fit_left(
                r.x + 12.0,
                button.bottom() + 16.0,
                w - 24.0,
                type_scale::MICRO,
                rgb(if *ok { palette::DIM } else { palette::BAD }, 1.0),
                text,
            );
        }
        r
    }

    /// This moment as a block of the issue log: enough to stage it again and to
    /// see where the time went.
    fn entry(&self, s: &Scene) -> String {
        let st = &s.view.status;
        let cam = s.camera;
        let id = self.record.as_ref().map_or("unrecorded", |m| m.id.as_str());
        let mut e = issues::header(id, self.marks, st.tick);
        let note = self.note.trim();
        let _ = writeln!(e, "note: {}", if note.is_empty() { "-" } else { note });
        let _ = writeln!(e, "map: {}", s.map.name());
        let (w, h) = (cam.viewport.x as u32, cam.viewport.y as u32);
        let camera = format!(
            "{:.0},{:.0},{:.0},{:.1}",
            cam.focus.x,
            cam.focus.y,
            cam.distance,
            cam.yaw.to_degrees()
        );
        if let Some(m) = &self.record {
            let _ = writeln!(e, "replay: {}", m.replay.display());
            let _ = writeln!(
                e,
                "repro: MERIDIAN_TILT={:.3} meridian --replay {} --ticks {} --camera {camera} --size {w}x{h} --follow 10 --screenshot issue.png --perf issue.json",
                cam.tilt,
                m.replay.display(),
                st.tick
            );
        }
        let _ = write!(
            e,
            "camera: focus {:.0},{:.0},{:.0}  distance {:.0}  yaw {:.1}  tilt {:.3}  window {w}x{h}",
            cam.focus.x,
            cam.focus.y,
            cam.focus.z,
            cam.distance,
            cam.yaw.to_degrees(),
            cam.tilt
        );
        match cam.pitch_free {
            Some(p) => {
                let _ = writeln!(e, "  free camera: pitch {:.3} fov {:.3}", p, cam.fov);
            }
            None => e.push('\n'),
        }
        let gpu_total: f32 = s.gpu.gpu_passes.iter().map(|p| p.1).sum();
        let _ = writeln!(
            e,
            "frame: {:.0} FPS  CPU {:.2} ms  GPU {gpu_total:.2} ms",
            s.view.fps, s.view.cpu_ms
        );
        for scope in &s.gpu.gpu_scopes {
            let _ = write!(
                e,
                "  gpu {}{:<24} {:>7.2} ms",
                "  ".repeat(scope.depth as usize),
                scope.name,
                scope.ms
            );
            match &scope.stats {
                Some(d) => {
                    let _ = writeln!(e, "  {d:?}");
                }
                None => e.push('\n'),
            }
        }
        let ms = |ns: u64| ns as f64 / 1e6;
        let _ = writeln!(
            e,
            "sim: tick {:.2} ms  worst {:.2} ms  hash {:016x}",
            ms(st.tick_ns),
            ms(st.worst_tick_ns),
            st.hash
        );
        for (name, ns) in &st.phases {
            let _ = writeln!(e, "  sim {name:<24} {:>7.2} ms", ms(*ns));
        }
        let _ = writeln!(
            e,
            "counts: {} units  {} shots  {} wrecks  {} stains  {} orders  {} fields  {} late paths  {} + {} drawn  {} terrain nodes",
            st.units,
            st.projectiles,
            st.wrecks,
            st.stains,
            st.orders,
            st.flow_fields,
            st.late_paths,
            s.gpu.dynamic_entities,
            s.gpu.static_entities,
            s.gpu.terrain_nodes
        );
        e.push('\n');
        e
    }
}

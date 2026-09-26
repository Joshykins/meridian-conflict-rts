//! The top bar (mission time, game speed or the network link, pause, menu) and
//! the roster of commanders under it, with the speed list that drops from it.

use super::{
    clock, icons, speed_label, Hud, HudAction, Scene, EDGE, GAP, SPEEDS, SPEED_W, TOP_BAR_W,
};
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Rect, Response, Ui};
use glam::Vec2;
use icons::Glyph;

impl Hud {
    /// Clock, game speed, pause and menu; the commanders under them. Returns the y below it all.
    pub(super) fn top_bar(&mut self, ui: &mut Ui, s: &Scene) -> f32 {
        let view = s.view;
        let owns_clock = view.status.owns_clock;
        let wide = TOP_BAR_W;
        let r = Rect::new(ui.size.x - EDGE - wide, EDGE, wide, 44.0);
        self.glass(ui, r);
        let mid = r.mid_y();
        ui.text(
            r.x + 16.0,
            mid - 8.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Mission Time",
        );
        ui.text(
            r.x + 16.0,
            mid + 8.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &clock(view.status.tick as f32 * 0.1),
        );
        ui.vline(r.x + 132.0, r.y + 9.0, r.h - 18.0, rgb(palette::LINE, 0.14));

        // Slower and faster either side; the middle opens the list of every speed.
        ui.text(
            r.x + 146.0,
            mid,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Speed",
        );
        let at = SPEEDS.iter().position(|p| *p == view.speed);
        let sx = r.x + 196.0;
        let arrows = [
            (Rect::new(sx, r.y + 7.0, 30.0, 30.0), -1i32),
            (Rect::new(sx + SPEED_W - 30.0, r.y + 7.0, 30.0, 30.0), 1),
        ];
        for (i, (ar, step)) in arrows.into_iter().enumerate() {
            let next = match at {
                Some(a) => (a as i32 + step).clamp(0, SPEEDS.len() as i32 - 1) as usize,
                None => SPEEDS.iter().position(|p| *p == 100).unwrap_or(0),
            };
            let able = owns_clock && Some(next) != at;
            let t = self.tile(ui, id("hud-speed-step", i), ar, false, able);
            let tone = rgb(palette::TEXT, if able { 0.7 + 0.3 * t.glow } else { 0.25 });
            let c = Vec2::new(ar.x + ar.w * 0.5, ar.mid_y());
            let d = step as f32;
            ui.triangle(
                c + Vec2::new(d * 4.0, 0.0),
                c + Vec2::new(-d * 3.0, -5.0),
                c + Vec2::new(-d * 3.0, 5.0),
                tone,
            );
            if t.clicked && able {
                ui.audio.play(Sfx::Tick);
                self.actions.push(HudAction::SetSpeed(SPEEDS[next]));
            }
        }
        let centre = Rect::new(sx + 34.0, r.y + 7.0, SPEED_W - 68.0, 30.0);
        let t = self.tile(ui, id("hud-speed", 0), centre, self.speed_open, owns_clock);
        ui.text_centred(
            centre.x + centre.w * 0.5 - 6.0,
            centre.mid_y(),
            type_scale::VALUE,
            rgb(
                palette::TEXT,
                if owns_clock {
                    0.85 + 0.15 * t.glow
                } else {
                    0.3
                },
            ),
            &speed_label(view.speed),
        );
        let caret = Vec2::new(centre.right() - 10.0, centre.mid_y());
        ui.triangle(
            caret + Vec2::new(-3.5, -2.0),
            caret + Vec2::new(3.5, -2.0),
            caret + Vec2::new(0.0, 2.5),
            rgb(palette::DIM, if owns_clock { 1.0 } else { 0.3 }),
        );
        if t.clicked && owns_clock {
            ui.audio.play(Sfx::Tick);
            self.speed_open = !self.speed_open;
        }
        if !owns_clock {
            self.speed_open = false;
        }
        // The list of speeds is drawn last, over everything: see `speed_list`.
        self.speed_anchor = centre;
        let after = sx + SPEED_W + 10.0;

        let pause = Rect::new(after, r.y + 7.0, 40.0, 30.0);
        let t = self.tile(ui, id("hud-pause", 0), pause, view.paused, owns_clock);
        let tone = rgb(
            palette::TEXT,
            if owns_clock {
                0.75 + 0.25 * t.glow
            } else {
                0.3
            },
        );
        icons::glyph(
            ui,
            if view.paused {
                Glyph::Play
            } else {
                Glyph::Pause
            },
            Vec2::new(pause.x + pause.w * 0.5, pause.mid_y()),
            7.0,
            tone,
        );
        if t.clicked {
            ui.audio.play(Sfx::Select);
            self.actions.push(HudAction::Pause);
        }
        if ui.button(
            id("hud-menu", 0),
            Rect::new(after + 50.0, r.y + 7.0, 88.0, 30.0),
            "Menu",
            ButtonKind::Secondary,
            true,
        ) {
            self.actions.push(HudAction::Menu);
        }

        // The commanders in this match.
        let players = &view.status.players;
        let mut y = r.bottom() + GAP;
        // An observer has them all down the left instead.
        if players.len() > 1 && !view.observing {
            let (wide, row_h) = (250.0, 22.0);
            let list = Rect::new(
                r.right() - wide,
                y,
                wide,
                players.len() as f32 * row_h + 12.0,
            );
            self.claim(ui, list);
            ui.fill(list, ink(0.6));
            ui.frame(list, rgb(palette::LINE, 0.12));
            for (i, p) in players.iter().enumerate() {
                let row = Rect::new(list.x, list.y + 6.0 + i as f32 * row_h, list.w, row_h);
                let res = ui.interact(id("hud-player", i), row, !p.defeated);
                if res.clicked {
                    ui.audio.play(Sfx::Select);
                    self.actions.push(HudAction::FocusPlayer(i as u8));
                }
                if res.glow > 0.02 {
                    ui.fill(row, rgb(palette::TEXT, 0.10 * res.glow));
                }
                let ry = row.mid_y();
                let alive = if p.defeated { 0.35 } else { 1.0 };
                let mut c = s.team_color(i as u8);
                c[3] = alive;
                ui.fill(Rect::new(list.x + 12.0, ry - 4.0, 8.0, 8.0), c);
                if i == view.local as usize {
                    ui.frame(
                        Rect::new(list.x + 9.0, ry - 7.0, 14.0, 14.0),
                        rgb(palette::TEXT, 0.8),
                    );
                }
                ui.text(
                    list.x + 32.0,
                    ry,
                    type_scale::CAPTION,
                    rgb(palette::TEXT, 0.9 * alive),
                    &p.name,
                );
                let (tag, tone) = if p.defeated {
                    ("Defeated".to_owned(), palette::BAD)
                } else {
                    (format!("Team {}", p.team + 1), palette::FAINT)
                };
                ui.text_right(
                    list.right() - 12.0,
                    ry,
                    type_scale::MICRO,
                    rgb(tone, 1.0),
                    &tag,
                );
            }
            y = list.bottom();
        }
        y
    }

    /// Where the rows of the speed list are, as far as it has unrolled.
    fn speed_rows(&self, k: f32) -> (Rect, Vec<Rect>) {
        let centre = self.speed_anchor;
        let row = 28.0;
        let full = SPEEDS.len() as f32 * row + 8.0;
        let list = Rect::new(centre.x, centre.bottom() + 11.0, centre.w, full * k);
        let rows = (0..SPEEDS.len())
            .map(|i| {
                Rect::new(
                    list.x + 4.0,
                    list.y + 4.0 + i as f32 * row,
                    list.w - 8.0,
                    row,
                )
            })
            .take_while(|rr| rr.bottom() <= list.bottom() + 0.5)
            .collect();
        (list, rows)
    }

    /// The speed list takes the pointer before anything under it can: called
    /// first thing in the frame, while `speed_list` draws it last.
    pub(super) fn speed_hits(&mut self, ui: &mut Ui) -> Vec<Response> {
        if !self.speed_open {
            return Vec::new();
        }
        let (list, rows) = self.speed_rows(1.0);
        self.claim(ui, list);
        let hits = rows
            .iter()
            .enumerate()
            .map(|(i, rr)| ui.interact(id("hud-speed-pick", i), *rr, true))
            .collect();
        // The empty edges of the list are the list's too.
        ui.interact_with(id("hud-speed-list-bg", 0), list, true, false);
        hits
    }

    /// Every game speed, dropped down from the speed control; it unrolls and rolls back up.
    pub(super) fn speed_list(&mut self, ui: &mut Ui, s: &Scene, hits: &[Response]) {
        let k = ui.ease(
            id("hud-speed-list", 0),
            if self.speed_open { 1.0 } else { 0.0 },
            18.0,
        );
        if k < 0.01 {
            return;
        }
        let (list, rows) = self.speed_rows(k);
        let fade = ui.fade;
        ui.fade *= k;
        ui.panel(list);
        for (i, rr) in rows.iter().enumerate() {
            let pct = SPEEDS[i];
            let on = pct == s.view.speed;
            let res = hits.get(i).copied().unwrap_or_default();
            ui.fill(
                *rr,
                rgb(0xFFFFFF, 0.06 * res.glow + if on { 0.1 } else { 0.0 }),
            );
            if on {
                ui.fill(
                    Rect::new(rr.x + 2.0, rr.y + 6.0, 2.0, rr.h - 12.0),
                    rgb(palette::ACCENT, 1.0),
                );
            }
            ui.text_centred(
                rr.x + rr.w * 0.5,
                rr.mid_y(),
                type_scale::VALUE,
                rgb(
                    if on { 0xFFFFFF } else { palette::DIM },
                    0.85 + 0.15 * res.glow,
                ),
                &speed_label(pct),
            );
            if res.clicked {
                ui.audio.play(Sfx::Tick);
                self.speed_open = false;
                if !on {
                    self.actions.push(HudAction::SetSpeed(pct));
                }
            }
        }
        ui.fade = fade;
        // A click anywhere else closes it.
        let centre = self.speed_anchor;
        if self.speed_open
            && ui.input.pressed
            && !list.contains(ui.cursor)
            && !centre.contains(ui.cursor)
        {
            self.speed_open = false;
        }
    }
}

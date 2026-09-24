//! The observer's side of the screen: whose eyes the battlefield is seen
//! through, how the armies weigh against each other, and every commander's
//! economy and forces, with a short history of their income.

use super::{whole, Hud, HudAction, Scene, EDGE, ENERGY, GAP, MASS};
use crate::audio::Sfx;
use crate::sim_thread::PlayerStatus;
use crate::ui::{id, ink, palette, rgb, type_scale, Color, Rect, Ui};
use glam::Vec2;
use std::collections::VecDeque;

pub const WIDTH: f32 = 372.0;
/// Seconds of income kept for the graphs.
const HISTORY: usize = 240;
pub(super) const HEADER_H: f32 = 108.0;
pub(super) const CARD_FULL: f32 = 118.0;
const CARD_SHORT: f32 = 62.0;

/// Materials income sampled once a game second, per commander.
#[derive(Default)]
pub struct History {
    last_second: Option<u32>,
    mass: Vec<VecDeque<f32>>,
}

impl History {
    fn sample(&mut self, tick: u32, players: &[PlayerStatus]) {
        let second = tick / 10;
        if self.last_second == Some(second) {
            return;
        }
        // A new match (or a rewind) starts the graphs over.
        if self.last_second.is_some_and(|s| s > second) || self.mass.len() != players.len() {
            *self = History::default();
            self.mass = vec![VecDeque::new(); players.len()];
        }
        self.last_second = Some(second);
        for (series, p) in self.mass.iter_mut().zip(players) {
            series.push_back(p.mass_income + p.reclaim_income);
            if series.len() > HISTORY {
                series.pop_front();
            }
        }
    }
}

fn short(v: f32) -> String {
    if v >= 10_000.0 {
        format!("{:.0}k", v / 1000.0)
    } else if v >= 1000.0 {
        format!("{:.1}k", v / 1000.0)
    } else {
        format!("{v:.0}")
    }
}

fn rate(v: f32) -> String {
    if v.abs() >= 100.0 {
        format!("{:.0}", v)
    } else {
        format!("{v:.1}")
    }
}

impl Hud {
    /// The observer's panel down the left. Returns the y below it.
    pub(super) fn observer_panel(&mut self, ui: &mut Ui, s: &Scene, bottom: f32) -> f32 {
        let view = s.view;
        let players = &view.status.players;
        self.observed.sample(view.status.tick, players);

        let header = Rect::new(EDGE, EDGE, WIDTH, HEADER_H);
        self.glass(ui, header);
        self.observer_header(ui, s, header);

        // Full cards when they fit above the deck, a line or two each when they do not.
        let room = bottom - header.bottom() - GAP;
        let n = players.len().max(1) as f32;
        let full = n * (CARD_FULL + 6.0) <= room;
        let card_h = if full { CARD_FULL } else { CARD_SHORT };
        let mut y = header.bottom() + GAP;
        for i in 0..players.len() {
            let r = Rect::new(EDGE, y, WIDTH, card_h);
            if r.bottom() > bottom && i > 0 {
                break;
            }
            self.player_card(ui, s, i, r, full);
            y = r.bottom() + 6.0;
        }
        y
    }

    fn observer_header(&mut self, ui: &mut Ui, s: &Scene, r: Rect) {
        let view = s.view;
        let players = &view.status.players;
        let pulse = 0.55 + 0.45 * (ui.time * 1.4).sin().abs();
        ui.fill(Rect::new(r.x, r.y, 4.0, r.h), rgb(palette::TEXT, 1.0));
        ui.fill(
            Rect::new(r.x + 18.0, r.y + 14.0, 8.0, 8.0),
            rgb(palette::TEXT, pulse),
        );
        ui.text(
            r.x + 34.0,
            r.y + 18.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            "Observing",
        );
        let (seeing, tone) = match view.perspective {
            Some(p) => (
                format!(
                    "Vision: {}",
                    players.get(p as usize).map_or("", |p| p.name.as_str())
                ),
                s.team_color(p),
            ),
            None => ("Vision: Everything".to_owned(), rgb(palette::DIM, 1.0)),
        };
        ui.text_right(r.right() - 14.0, r.y + 18.0, type_scale::MICRO, tone, &seeing);

        // Whose eyes: everything, or one commander's. Keys 0..8 do the same.
        let chip_w = 34.0;
        let mut x = r.x + 18.0;
        let y = r.y + 34.0;
        let all = Rect::new(x, y, 52.0, 26.0);
        if self.vision_chip(ui, id("obs-vision", 99), all, view.perspective.is_none(), None, "All") {
            self.actions.push(HudAction::Vision(None));
        }
        x = all.right() + 6.0;
        for (i, p) in players.iter().enumerate() {
            let chip = Rect::new(x, y, chip_w, 26.0);
            if chip.right() > r.right() - 10.0 {
                break;
            }
            let mut c = s.team_color(i as u8);
            if p.defeated {
                c[3] = 0.35;
            }
            let on = view.perspective == Some(i as u8);
            if self.vision_chip(ui, id("obs-vision", i), chip, on, Some(c), &format!("{}", i + 1)) {
                self.actions.push(HudAction::Vision(Some(i as u8)));
            }
            x = chip.right() + 4.0;
        }

        // How the armies weigh against each other, by worth, in team order.
        let bar = Rect::new(r.x + 18.0, r.y + 80.0, r.w - 36.0, 8.0);
        ui.text(
            bar.x,
            bar.y - 8.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Army Strength",
        );
        let mut order: Vec<usize> = (0..players.len()).collect();
        order.sort_by_key(|&i| (players[i].team, i));
        let total: f32 = players.iter().map(|p| p.forces.army_value).sum();
        ui.fill(bar, rgb(palette::LINE, 0.10));
        if total > 0.0 {
            let mut at = bar.x;
            for (k, &i) in order.iter().enumerate() {
                let w = bar.w * players[i].forces.army_value / total;
                if w < 0.5 {
                    continue;
                }
                ui.fill(Rect::new(at, bar.y, w, bar.h), s.team_color(i as u8));
                // A team boundary gets a gap.
                if order
                    .get(k + 1)
                    .is_some_and(|&j| players[j].team != players[i].team)
                {
                    ui.fill(Rect::new(at + w - 1.0, bar.y - 2.0, 2.0, bar.h + 4.0), ink(0.9));
                }
                at += w;
            }
        }
        ui.text_right(
            bar.right(),
            bar.y - 8.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("{} total", short(total)),
        );
    }

    fn vision_chip(
        &mut self,
        ui: &mut Ui,
        chip_id: crate::ui::Id,
        r: Rect,
        on: bool,
        color: Option<Color>,
        label: &str,
    ) -> bool {
        let t = self.tile(ui, chip_id, r, on, true);
        if let Some(c) = color {
            ui.fill(Rect::new(r.x + 3.0, r.bottom() - 5.0, r.w - 6.0, 3.0), c);
        }
        if on {
            ui.frame(r, rgb(palette::ACCENT, 0.9));
        }
        ui.text_centred(
            r.x + r.w * 0.5,
            r.mid_y() - 1.0,
            type_scale::VALUE,
            rgb(if on { 0xFFFFFF } else { palette::DIM }, 0.85 + 0.15 * t.glow),
            label,
        );
        if t.clicked && !on {
            ui.audio.play(Sfx::Tick);
        }
        t.clicked && !on
    }

    /// One commander: name and team, both stores with what flows in and out,
    /// the materials income over the last minutes, and what they field.
    fn player_card(&mut self, ui: &mut Ui, s: &Scene, i: usize, r: Rect, full: bool) {
        let view = s.view;
        let p = &view.status.players[i];
        let viewing = view.perspective == Some(i as u8);
        let alive = if p.defeated { 0.4 } else { 1.0 };
        self.glass(ui, r);
        // The camera button takes its clicks before the card under it can.
        let find = Rect::new(r.right() - 66.0, r.y + 5.0, 58.0, 22.0);
        if !p.defeated {
            let t = self.tile(ui, id("obs-find", i), find, false, true);
            ui.text_centred(
                find.x + find.w * 0.5,
                find.mid_y(),
                type_scale::MICRO,
                rgb(palette::TEXT, 0.7 + 0.3 * t.glow),
                "Find",
            );
            if t.clicked {
                ui.audio.play(Sfx::Select);
                self.actions.push(HudAction::FocusPlayer(i as u8));
            }
        }
        let res = ui.interact(id("obs-card", i), r, true);
        if res.glow > 0.02 {
            ui.fill(r, rgb(palette::TEXT, 0.05 * res.glow));
        }
        if viewing {
            ui.frame(r, rgb(palette::ACCENT, 0.8));
        }
        // Clicking a card looks through that commander's eyes; again, through everyone's.
        if res.clicked && (p.defeated || !find.contains(ui.cursor)) {
            self.actions
                .push(HudAction::Vision(if viewing { None } else { Some(i as u8) }));
        }
        let mut c = s.team_color(i as u8);
        c[3] = alive;
        ui.fill(Rect::new(r.x, r.y, 4.0, r.h), c);

        // Name line: number, name, team; on the right, stalls and the camera button.
        let y = r.y + 16.0;
        ui.text(
            r.x + 14.0,
            y,
            type_scale::VALUE,
            rgb(palette::FAINT, alive),
            &format!("{}", i + 1),
        );
        let end = ui.text(
            r.x + 30.0,
            y,
            type_scale::CAPTION,
            rgb(palette::TEXT, alive),
            &p.name,
        );
        ui.text(
            end + 10.0,
            y + 0.5,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("Team {}", p.team + 1),
        );
        let status_right = find.x - 10.0;
        if p.defeated {
            ui.text_right(r.right() - 12.0, y, type_scale::CAPTION, rgb(palette::BAD, 1.0), "Defeated");
        } else if p.efficiency < 0.999 {
            ui.text_right(
                status_right,
                y,
                type_scale::MICRO,
                rgb(palette::BAD, 1.0),
                &format!("Stalling {:.0}%", p.efficiency * 100.0),
            );
        } else if viewing {
            ui.text_right(status_right, y, type_scale::MICRO, rgb(palette::ACCENT, 1.0), "Viewing");
        }

        if !full {
            // One line: both stores and their nets, then what they field.
            let y = r.y + 42.0;
            let mut x = r.x + 14.0;
            for (have, net, tone) in [
                (p.mass, p.mass_income + p.reclaim_income - p.mass_demand, MASS),
                (p.energy, p.energy_income - p.energy_demand, ENERGY),
            ] {
                ui.fill(Rect::new(x, y - 4.0, 3.0, 8.0), rgb(tone, alive));
                let end = ui.text(x + 8.0, y, type_scale::VALUE, rgb(palette::TEXT, alive), &whole(have));
                ui.text(
                    end + 5.0,
                    y,
                    type_scale::MICRO,
                    rgb(if net < -0.05 { palette::BAD } else { tone }, alive),
                    &format!("{}{}", if net < 0.0 { "" } else { "+" }, rate(net)),
                );
                x += 116.0;
            }
            let f = p.forces;
            ui.text_right(
                r.right() - 12.0,
                y,
                type_scale::MICRO,
                rgb(palette::DIM, alive),
                &format!("Army {} \u{b7} {}", f.army, short(f.army_value)),
            );
            return;
        }

        // Both stores: level, a bar of how full, and what comes in and goes out.
        let graph = Rect::new(r.right() - 116.0, r.y + 34.0, 104.0, 46.0);
        let rows_w = graph.x - 14.0 - (r.x + 14.0);
        for (k, (have, cap, income, spend, tone)) in [
            (p.mass, p.mass_capacity, p.mass_income + p.reclaim_income, p.mass_demand, MASS),
            (p.energy, p.energy_capacity, p.energy_income, p.energy_demand, ENERGY),
        ]
        .into_iter()
        .enumerate()
        {
            let x = r.x + 14.0;
            let y = r.y + 40.0 + k as f32 * 24.0;
            let end = ui.text(x, y, type_scale::VALUE, rgb(palette::TEXT, alive), &whole(have));
            ui.text(
                end + 4.0,
                y + 0.5,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                &format!("/ {}", short(cap)),
            );
            let net = income - spend;
            ui.text_right(
                x + rows_w,
                y,
                type_scale::MICRO,
                rgb(if net < -0.05 { palette::BAD } else { tone }, alive),
                &format!("+{}  -{}", rate(income), rate(spend)),
            );
            let track = Rect::new(x, y + 9.0, rows_w, 3.0);
            ui.fill(track, rgb(palette::LINE, 0.10));
            let fill = (have / cap.max(1.0)).clamp(0.0, 1.0);
            ui.fill(Rect::new(track.x, track.y, track.w * fill, track.h), rgb(tone, 0.9 * alive));
        }

        // Materials income over the last few minutes.
        ui.fill(graph, ink(0.35));
        if let Some(series) = self.observed.mass.get(i) {
            // Every card on one scale, so the graphs compare at a glance.
            let peak = self
                .observed
                .mass
                .iter()
                .flat_map(|s| s.iter().copied())
                .fold(1.0f32, f32::max);
            let n = series.len();
            if n > 1 {
                let step = graph.w / (HISTORY - 1) as f32;
                let x0 = graph.right() - (n - 1) as f32 * step;
                let at = |j: usize, v: f32| {
                    Vec2::new(
                        x0 + j as f32 * step,
                        graph.bottom() - 2.0 - (graph.h - 6.0) * (v / peak).clamp(0.0, 1.0),
                    )
                };
                for (j, (a, b)) in series.iter().zip(series.iter().skip(1)).enumerate() {
                    ui.stroke(at(j, *a), at(j + 1, *b), 1.4, rgb(MASS, 0.9 * alive));
                }
            }
        }
        ui.text(
            graph.x + 4.0,
            graph.y + 7.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Income",
        );

        // What they field, and how the fighting has gone.
        let f = p.forces;
        let y = r.bottom() - 14.0;
        let facts = [
            ("Army", format!("{} ({})", f.army, short(f.army_value))),
            ("Eng", format!("{}", f.engineers)),
            ("Fac", format!("{}", f.factories)),
            ("Mines", format!("{}", f.mines)),
            ("K/L", format!("{}/{}", p.units_killed, p.units_lost)),
        ];
        let mut x = r.x + 14.0;
        for (label, value) in facts {
            let end = ui.text(x, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
            let end = ui.text(end + 4.0, y, type_scale::MICRO, rgb(palette::TEXT, 0.9 * alive), &value);
            x = end + 12.0;
        }
    }
}

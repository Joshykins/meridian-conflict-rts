//! The chart in the middle of the line-up: the map with every seat's landing
//! zone in its colour, allies joined up. In co-op survival it is the siege
//! chart, with the Progenitor's fronts. Whoever may plan clicks a zone to
//! deploy there; whoever held it takes theirs.

use super::roster::Seat;
use super::{Catalog, Lineup, Mode, Table};
use crate::audio::Sfx;
use crate::setup::TEAM_COLORS;
use crate::ui::survival::siege::{self, Holder, Siege};
use crate::ui::{id, ink, palette, preview, rgb, teams, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::weather::Climate;
use mc_map::MapFile;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

/// Which map a picture is of: the mode and its index in the catalog.
type Key = (Mode, usize);

/// The chart's picture of the map. It takes tens of milliseconds to draw, so it
/// is drawn on a worker and the chart fades up when it arrives: opening the
/// screen or changing the map never stalls a frame. The last one drawn is
/// kept, so coming back to the screen puts it back without drawing it again.
#[derive(Default)]
pub(super) struct Picture {
    /// The map whose picture is in the image slot.
    shown: Option<Key>,
    /// The last picture drawn.
    kept: Option<(Key, Vec<u8>)>,
    /// The picture being drawn.
    job: Option<(Key, Receiver<Vec<u8>>)>,
}

impl Picture {
    /// Something else drew in the image slot.
    pub(super) fn lost(&mut self) {
        self.shown = None;
    }

    #[cfg(test)]
    pub(super) fn is_shown(&self) -> bool {
        self.shown.is_some()
    }

    /// Puts the picture of `map` in `slot` once it is drawn; true when it is there.
    fn show(
        &mut self,
        ui: &mut Ui,
        slot: usize,
        key: Key,
        map: &Arc<MapFile>,
        climate: Climate,
    ) -> bool {
        if self.shown == Some(key) {
            return true;
        }
        if let Some((done, rx)) = &self.job {
            match rx.try_recv() {
                Ok(rgba) => {
                    self.kept = Some((*done, rgba));
                    self.job = None;
                }
                Err(TryRecvError::Disconnected) => self.job = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some((_, rgba)) = self.kept.as_ref().filter(|(k, _)| *k == key) {
            ui.o.set_image(slot, preview::SIZE, preview::SIZE, rgba);
            self.shown = Some(key);
            return true;
        }
        // A picture of another map still being drawn is dropped when it arrives.
        if self.job.as_ref().is_none_or(|(k, _)| *k != key) {
            let (tx, rx) = channel();
            let worker_map = map.clone();
            let spawned = std::thread::Builder::new()
                .name("lineup-chart".into())
                .spawn(move || {
                    let _ = tx.send(preview::render(&worker_map, climate));
                });
            match spawned {
                Ok(_) => self.job = Some((key, rx)),
                Err(e) => {
                    log::warn!("no thread for the chart: {e}; drawn here");
                    self.kept = Some((key, preview::render(map, climate)));
                }
            }
        }
        false
    }
}

/// The chart in `area`, drawn in image slot `slot`.
pub fn chart(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &Catalog,
    table: &Table,
    slot: usize,
    area: Rect,
) {
    // Only the host moves commanders, and only their own by a click.
    let mover = table.me.filter(|_| table.host);
    let picked = match lineup.mode {
        Mode::Skirmish => zones(ui, lineup, catalog, table, slot, mover, area),
        Mode::Survival => siege_chart(ui, lineup, catalog, table, slot, mover, area),
    };
    if let (Some(me), Some(n)) = (mover, picked) {
        lineup.roster.take_zone(me, n);
        ui.audio.play(Sfx::Select);
    }
}

fn colour(s: &Seat) -> [f32; 3] {
    TEAM_COLORS[s.color as usize % TEAM_COLORS.len()]
}

fn siege_chart(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &Catalog,
    table: &Table,
    slot: usize,
    mover: Option<usize>,
    area: Rect,
) -> Option<u8> {
    let theatre = catalog.theatres.get(lineup.map)?;
    let holders: Vec<Holder> = lineup
        .roster
        .seats
        .iter()
        .enumerate()
        .filter(|(_, s)| s.open())
        .map(|(i, s)| Holder {
            spawn: s.start as usize,
            color: colour(s),
            you: table.me == Some(i) && !table.observe,
            name: match table.occupant(i) {
                Some(p) => p.name.clone(),
                None if table.is_ai(s) => lineup.ai_name(table, i),
                None if table.lobby => "An open seat".to_owned(),
                None => table.name.to_owned(),
            },
        })
        .collect();
    let view = Siege {
        theatre,
        key: lineup.map,
        rules: &lineup.rules,
        hover_domain: lineup.hover_domain,
        holders: &holders,
        can_pick: mover.is_some(),
        slot,
        drawn: lineup.picture.show(
            ui,
            slot,
            (Mode::Survival, lineup.map),
            &theatre.map,
            theatre.climate,
        ),
    };
    let picked = siege::chart(ui, &view, &mut lineup.markers, area);
    picked.map(|i| i as u8)
}

/// The skirmish chart: grid, zones, allies' lines. The zone clicked, if any.
fn zones(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &Catalog,
    table: &Table,
    slot: usize,
    mover: Option<usize>,
    area: Rect,
) -> Option<u8> {
    let entry = catalog.maps.get(lineup.map)?;
    let drawn = lineup.picture.show(
        ui,
        slot,
        (Mode::Skirmish, lineup.map),
        &entry.map,
        entry.climate,
    );
    let map = entry.map.clone();
    let side = area.w.min(area.h - 64.0);
    let frame = Rect::new(area.x + (area.w - side) * 0.5, area.y, side, side);
    // The chart fades up once the map's picture is drawn.
    let shown = ui.ease(
        id("preview-shown", lineup.map),
        if drawn { 1.0 } else { 0.0 },
        5.0,
    );
    ui.fill(frame, ink(0.85));
    ui.image(
        slot,
        [0.0, 0.0, preview::SIZE as f32, preview::SIZE as f32],
        frame,
        [shown, shown, shown, 1.0],
    );
    furniture(ui, &map, frame);

    // Allies' landing zones joined up, so each side reads as one on the chart.
    let seats = &lineup.roster.seats;
    let allied = lineup.roster.allied();
    let at_zone = |n: usize| {
        let p = map
            .start_positions()
            .get(n)
            .map_or([0.0; 2], |p| p.to_f32());
        Vec2::new(frame.x, frame.y) + preview::locate(&map, p, side)
    };
    if allied {
        let seated: Vec<&Seat> = seats.iter().filter(|s| s.open()).collect();
        let at: Vec<Vec2> = seated.iter().map(|s| at_zone(s.start as usize)).collect();
        let team_of: Vec<u8> = seated.iter().map(|s| s.team).collect();
        for (a, b) in teams::links(&at, &team_of) {
            let lit = lineup.hover_team == Some(team_of[a]);
            let d = (at[b] - at[a]).normalize_or_zero();
            // Stop short of the markers' rings.
            let (p, q) = (at[a] + d * 17.0, at[b] - d * 17.0);
            if (q - p).dot(d) > 0.0 {
                ui.stroke(p, q, 5.0, ink(0.5));
                ui.stroke(
                    p,
                    q,
                    if lit { 2.2 } else { 1.4 },
                    rgb(palette::TEXT, if lit { 0.95 } else { 0.55 }),
                );
            }
        }
    }

    let mut take = None;
    lineup.markers.clear();
    let zones = map.start_positions().len().min(mc_core::MAX_PLAYERS);
    // A crowded ring (up to 32 zones): one badge per team, outside its first zone,
    // rather than one on every marker.
    let crowded = zones > 12;
    let mut badged: Vec<u8> = Vec::new();
    let centre = Vec2::new(frame.x + side * 0.5, frame.y + side * 0.5);
    for n in 0..zones {
        let p = at_zone(n);
        lineup.markers.push(p + ui.shift);
        let holder = seats.iter().position(|s| s.open() && s.start as usize == n);
        let res = ui.interact(
            id("start-marker", n),
            Rect::new(p.x - 17.0, p.y - 17.0, 34.0, 34.0),
            true,
        );
        let yours = holder.is_some() && holder == table.me;
        if res.clicked && !yours && mover.is_some() {
            take = Some(n as u8);
        }
        let color = holder.map_or(rgb(palette::DIM, 0.9), |i| {
            let c = colour(&seats[i]);
            [c[0], c[1], c[2], 1.0]
        });
        ui.disc(p, 13.0 + 2.0 * res.glow, ink(0.85));
        ui.arc(
            p,
            13.0 + 2.0 * res.glow,
            0.0,
            std::f32::consts::TAU,
            if holder.is_some() { 2.2 } else { 1.2 },
            color,
        );
        if yours && !table.observe {
            let pulse = (ui.time * 0.8).fract();
            ui.arc(
                p,
                14.0 + 16.0 * pulse,
                0.0,
                std::f32::consts::TAU,
                1.4,
                [color[0], color[1], color[2], 0.8 * (1.0 - pulse)],
            );
        }
        ui.text_centred(
            p.x + 1.0,
            p.y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &format!("{}", n + 1),
        );
        // Whose side the zone is on.
        if let Some(i) = holder.filter(|_| allied) {
            let team = seats[i].team;
            let lit = lineup.hover_team == Some(team);
            if !crowded {
                teams::badge(ui, p.x + 9.0, p.y - 17.0, team, 1.0, lit);
            } else if !badged.contains(&team) {
                badged.push(team);
                let out = p + (p - centre).normalize_or_zero() * 34.0;
                teams::badge(ui, out.x - 12.0, out.y, team, 1.0, lit);
            }
        }
        if res.glow > 0.05 {
            let tip = match holder {
                _ if yours && table.observe => "AI Landing Zone".to_owned(),
                _ if yours => "Your Landing Zone".to_owned(),
                Some(i) if mover.is_some() => format!(
                    "Click to Swap Places with {}",
                    holder_name(lineup, table, i)
                ),
                Some(i) => holder_name(lineup, table, i),
                None if mover.is_some() => "Click to Deploy Here".to_owned(),
                None => "Unoccupied".to_owned(),
            };
            let tw = ui.text_width(type_scale::MICRO, &tip);
            let tag = Rect::new(
                (p.x - tw * 0.5 - 8.0).clamp(frame.x, (frame.right() - tw - 16.0).max(frame.x)),
                p.y + 22.0,
                tw + 16.0,
                20.0,
            );
            ui.fill(tag, ink(0.9 * res.glow));
            ui.text(
                tag.x + 8.0,
                tag.mid_y(),
                type_scale::MICRO,
                rgb(palette::TEXT, res.glow),
                &tip,
            );
        }
    }

    // Under the chart (the bar over it names the map): its size, and the key,
    // which drops to a line of its own when the chart is too narrow for both.
    let size_m = map.info().size_metres().to_f32();
    let y = frame.bottom() + 24.0;
    let end = ui.text(
        frame.x,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!(
            "{:.1} \u{d7} {:.1} km",
            size_m[0] / 1000.0,
            size_m[1] / 1000.0
        ),
    );
    let key = if allied {
        "Red: Ore fields    Rings: Landing zones    Lines: Allies"
    } else {
        "Red: Ore fields    Rings: Landing zones"
    };
    let key_w = ui.text_width(type_scale::MICRO, key);
    if end + 24.0 + key_w <= frame.right() {
        ui.text_right(
            frame.right(),
            y,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            key,
        );
    } else {
        ui.text_fit_left(
            frame.x,
            y + 20.0,
            frame.w,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            key,
        );
    }
    take
}

/// Who holds seat `i`, as a zone's tip names them.
fn holder_name(lineup: &Lineup, table: &Table, i: usize) -> String {
    match table.occupant(i) {
        Some(p) => p.name.clone(),
        None if table.is_ai(&lineup.roster.seats[i]) => lineup.ai_name(table, i),
        None if table.lobby => "Open Seat".to_owned(),
        None => table.name.to_owned(),
    }
}

/// Chart furniture: grid, frame, brackets, a scale bar and north.
fn furniture(ui: &mut Ui, map: &mc_map::MapFile, frame: Rect) {
    for k in 1..4 {
        let t = k as f32 / 4.0;
        ui.vline(
            frame.x + frame.w * t,
            frame.y,
            frame.h,
            rgb(palette::LINE, 0.07),
        );
        ui.hline(
            frame.x,
            frame.y + frame.h * t,
            frame.w,
            rgb(palette::LINE, 0.07),
        );
    }
    ui.frame(frame, rgb(palette::LINE, 0.25));
    ui.brackets(frame.inset(-6.0), 14.0, rgb(palette::ACCENT, 0.7));
    let size_m = map.info().size_metres().to_f32();
    let km_pts = frame.w / (size_m[0].max(size_m[1]) / 1000.0);
    let bar_km = if size_m[0] > 30_000.0 { 10.0 } else { 2.0 };
    ui.fill(
        Rect::new(frame.x + 14.0, frame.bottom() - 16.0, km_pts * bar_km, 2.0),
        rgb(palette::TEXT, 0.8),
    );
    ui.text(
        frame.x + 14.0,
        frame.bottom() - 28.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.8),
        &format!("{bar_km:.0} km"),
    );
    ui.text_right(
        frame.right() - 12.0,
        frame.y + 16.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.6),
        "N",
    );
    ui.stroke(
        Vec2::new(frame.right() - 16.0, frame.y + 44.0),
        Vec2::new(frame.right() - 16.0, frame.y + 26.0),
        1.2,
        rgb(palette::TEXT, 0.6),
    );
}

//! The chart in the middle of the line-up: the map with every seat's landing
//! zone in its colour, allies joined up. In co-op survival it is the siege
//! chart, with the Progenitor's fronts.
//!
//! Whoever may plan moves commanders here: click a commander's zone (or the
//! Zone cell in their row) to pick them up, then the zone they go to; whoever
//! held it takes theirs. A free zone clicked with nobody picked up takes your
//! own commander. Escape or a right click puts a picked-up commander down.

use super::roster::Seat;
use super::{Catalog, Lineup, Mode, Table};
use crate::audio::Sfx;
use crate::setup::TEAM_COLORS;
use crate::ui::survival::siege::{self, Holder, Siege};
use crate::ui::{id, ink, palette, preview, rgb, teams, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::weather::MapLook;
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
        look: &MapLook,
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
            let (worker_map, worker_look) = (map.clone(), look.clone());
            let spawned = std::thread::Builder::new()
                .name("lineup-chart".into())
                .spawn(move || {
                    let _ = tx.send(preview::render(&worker_map, &worker_look));
                });
            match spawned {
                Ok(_) => self.job = Some((key, rx)),
                Err(e) => {
                    log::warn!("no thread for the chart: {e}; drawn here");
                    self.kept = Some((key, preview::render(map, look)));
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
    let cancel = ui.input.right_pressed
        || (ui.input.key(crate::ui::Key::Escape)
            && ui.mem.popup.is_none()
            && ui.mem.editing.is_none());
    if lineup.placing.is_some() && (!table.host || (ui.interactive && cancel)) {
        lineup.placing = None;
        ui.audio.play(Sfx::Back);
    }
    // The commander pointed at in the list last frame; the chart says who it points at now.
    let lit = lineup.hover_seat.take();
    let picked = match lineup.mode {
        Mode::Skirmish => zones(ui, lineup, catalog, table, slot, lit, area),
        Mode::Survival => siege_chart(ui, lineup, catalog, table, slot, area),
    };
    if let Some(n) = picked.filter(|_| table.host) {
        let sfx = lineup.place(table, n);
        ui.audio.play(sfx);
    }
    if let Some(i) = lineup.placing.and_then(|k| lineup.roster.index_of(k)) {
        let name = super::seats::who(lineup, table, i, &lineup.roster.seats[i]);
        moving_banner(ui, &name, colour(&lineup.roster.seats[i]), area);
    }
}

/// Over the top of the chart while a commander is picked up: who, and what to do.
fn moving_banner(ui: &mut Ui, name: &str, c: [f32; 3], area: Rect) {
    let text = format!("Moving {name}: click a landing zone");
    let hint = "Esc or Right Click to Cancel";
    let tw = ui.text_width(type_scale::CAPTION, &text);
    let hw = ui.text_width(type_scale::MICRO, hint);
    let w = (tw + hw + 70.0).min(area.w);
    let r = Rect::new(area.x + (area.w - w) * 0.5, area.y + 12.0, w, 34.0);
    let pulse = 0.5 + 0.5 * (ui.time * 4.0).sin();
    ui.fill(r, ink(0.88));
    ui.frame(r, rgb(palette::ACCENT, 0.45 + 0.35 * pulse));
    ui.fill(Rect::new(r.x, r.y, 4.0, r.h), [c[0], c[1], c[2], 1.0]);
    let end = ui.text(
        r.x + 18.0,
        r.mid_y(),
        type_scale::CAPTION,
        rgb(palette::TEXT, 1.0),
        &text,
    );
    ui.text_fit_left(
        end + 16.0,
        r.mid_y() + 0.5,
        r.right() - end - 26.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        hint,
    );
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
    area: Rect,
) -> Option<u8> {
    let theatre = catalog.theatres.get(lineup.map)?;
    let moving = lineup
        .placing
        .and_then(|k| lineup.roster.index_of(k))
        .map(|i| super::seats::who(lineup, table, i, &lineup.roster.seats[i]));
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
        can_pick: table.host,
        moving: moving.as_deref(),
        slot,
        drawn: lineup.picture.show(
            ui,
            slot,
            (Mode::Survival, lineup.map),
            &theatre.map,
            &theatre.look,
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
    lit: Option<u8>,
    area: Rect,
) -> Option<u8> {
    let entry = catalog.maps.get(lineup.map)?;
    let host = table.host;
    let moving = lineup.placing.and_then(|k| lineup.roster.index_of(k));
    let drawn = lineup.picture.show(
        ui,
        slot,
        (Mode::Skirmish, lineup.map),
        &entry.map,
        &entry.look,
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
        if res.clicked && host {
            take = Some(n as u8);
        }
        if res.hovered {
            lineup.hover_seat = holder.map(|i| seats[i].key);
        }
        let fill = holder.map(|i| colour(&seats[i]));
        let picked_up = holder.is_some() && holder == moving;
        // Pointed at in the list, or this marker under the pointer.
        let pointed = holder.is_some_and(|i| lit == Some(seats[i].key)) || res.hovered;
        let look = Marker {
            fill,
            hover: res.glow,
            lit: ui.ease(
                id("start-marker-lit", n),
                if pointed || picked_up { 1.0 } else { 0.0 },
                12.0,
            ),
            picked_up,
            target: moving.is_some() && !picked_up,
            yours: yours && !table.observe && moving.is_none(),
        };
        marker(ui, p, n, &look);
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
            let tip = zone_tip(lineup, table, holder, moving, n);
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

    caption(ui, &map, frame, allied);
    take
}

/// How a landing zone's marker looks this frame.
struct Marker {
    /// The colour of the commander who holds it, if anyone does.
    fill: Option<[f32; 3]>,
    /// Under the pointer, eased.
    hover: f32,
    /// Its commander pointed at here or in the list, or picked up, eased.
    lit: f32,
    /// Its commander is picked up to move.
    picked_up: bool,
    /// Somewhere a picked-up commander may go.
    target: bool,
    /// Yours, with nobody picked up.
    yours: bool,
}

/// Landing zone `n`'s marker at `p`: filled in its commander's colour, ringed
/// while lit, spinning while picked up, a faint target while a commander is
/// moving, a pulse when it is yours.
fn marker(ui: &mut Ui, p: Vec2, n: usize, m: &Marker) {
    use std::f32::consts::TAU;
    let color = m
        .fill
        .map_or(rgb(palette::DIM, 0.9), |c| [c[0], c[1], c[2], 1.0]);
    let radius = 13.0 + 2.0 * m.hover + 2.0 * m.lit;
    ui.disc(p, radius, ink(0.85));
    if let Some(c) = m.fill {
        // The commander's colour fills the marker, so a zone reads as theirs at a glance.
        ui.disc(p, radius - 3.0, [c[0], c[1], c[2], 0.35 + 0.25 * m.lit]);
    }
    ui.arc(
        p,
        radius,
        0.0,
        TAU,
        if m.fill.is_some() { 2.2 } else { 1.2 },
        color,
    );
    if m.picked_up {
        let a = ui.time * 3.0;
        ui.arc(p, radius + 6.0, a, a + 4.4, 2.0, rgb(palette::ACCENT, 1.0));
    } else if m.lit > 0.01 {
        ui.arc(
            p,
            radius + 5.0,
            0.0,
            TAU,
            1.4,
            [color[0], color[1], color[2], 0.8 * m.lit],
        );
    }
    if m.target {
        let pulse = (ui.time * 1.2 + n as f32 * 0.13).fract();
        ui.arc(
            p,
            radius + 3.0 + 10.0 * pulse,
            0.0,
            TAU,
            1.0,
            rgb(palette::ACCENT, 0.6 * (1.0 - pulse)),
        );
    } else if m.yours {
        let pulse = (ui.time * 0.8).fract();
        ui.arc(
            p,
            radius + 1.0 + 16.0 * pulse,
            0.0,
            TAU,
            1.4,
            [color[0], color[1], color[2], 0.8 * (1.0 - pulse)],
        );
    }
}

/// Under the chart: the map's size, and the key, which drops to a line of its
/// own when the chart is too narrow for both.
fn caption(ui: &mut Ui, map: &MapFile, frame: Rect, allied: bool) {
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
}

/// What a zone's tip says: whose it is, and what a click on it would do.
fn zone_tip(
    lineup: &Lineup,
    table: &Table,
    holder: Option<usize>,
    moving: Option<usize>,
    n: usize,
) -> String {
    let host = table.host;
    let yours = holder.is_some() && holder == table.me;
    match (moving, holder) {
        (Some(m), h) if h == Some(m) => "Click to Put Down".to_owned(),
        (Some(m), Some(h)) => format!(
            "Move {} Here  \u{b7}  {} Takes Theirs",
            holder_name(lineup, table, m),
            holder_name(lineup, table, h)
        ),
        (Some(m), None) => format!("Move {} Here", holder_name(lineup, table, m)),
        (None, Some(_)) if yours && table.observe => {
            "AI Landing Zone  \u{b7}  Click to Move".to_owned()
        }
        (None, Some(_)) if yours && host => "Your Landing Zone  \u{b7}  Click to Move".to_owned(),
        (None, Some(_)) if yours => "Your Landing Zone".to_owned(),
        (None, Some(h)) if host => {
            format!("{}  \u{b7}  Click to Move", holder_name(lineup, table, h))
        }
        (None, Some(h)) => holder_name(lineup, table, h),
        (None, None) if host && table.me.is_some() => {
            format!("Zone {}  \u{b7}  Click to Deploy Here", n + 1)
        }
        (None, None) => format!("Zone {}  \u{b7}  Unoccupied", n + 1),
    }
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

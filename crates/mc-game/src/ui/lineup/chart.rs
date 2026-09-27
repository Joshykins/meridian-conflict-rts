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
    };
    let mut drawn = (lineup.chart_of == Some((Mode::Survival, lineup.map))).then_some(lineup.map);
    let picked = siege::chart(ui, &view, &mut drawn, &mut lineup.markers, area);
    lineup.chart_of = drawn.map(|m| (Mode::Survival, m));
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
    if lineup.chart_of != Some((Mode::Skirmish, lineup.map)) {
        ui.o.set_image(
            slot,
            preview::SIZE,
            preview::SIZE,
            &preview::render(&entry.map, entry.climate),
        );
        lineup.chart_of = Some((Mode::Skirmish, lineup.map));
    }
    let map = entry.map.clone();
    let side = area.w.min(area.h - 64.0);
    let frame = Rect::new(area.x + (area.w - side) * 0.5, area.y, side, side);
    // The chart fades up when the map changes.
    let shown = ui.ease(id("preview-shown", lineup.map), 1.0, 5.0);
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
    for n in 0..map.start_positions().len().min(8) {
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
            teams::badge(
                ui,
                p.x + 9.0,
                p.y - 17.0,
                team,
                1.0,
                lineup.hover_team == Some(team),
            );
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
                (p.x - tw * 0.5 - 8.0).clamp(frame.x, frame.right() - tw - 16.0),
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

    // Caption under the chart.
    let size_m = map.info().size_metres().to_f32();
    let y = frame.bottom() + 30.0;
    let end = ui.text(
        frame.x,
        y,
        type_scale::ITEM,
        rgb(palette::TEXT, 1.0),
        map.name(),
    );
    ui.text(
        end + 18.0,
        y + 1.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!(
            "{:.1} \u{d7} {:.1} Km",
            size_m[0] / 1000.0,
            size_m[1] / 1000.0
        ),
    );
    ui.text_right(
        frame.right(),
        y + 1.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        if allied {
            "Red: Ore fields    Rings: Landing zones    Lines: Allies"
        } else {
            "Red: Ore fields    Rings: Landing zones"
        },
    );
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

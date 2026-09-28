//! The commanders column: a row per seat with its colour, who plays it, race,
//! control, team and landing zone; rows grouped by team under headings when
//! anyone is allied; quick team layouts under them. In co-op survival the rows
//! are the defenders, with the Progenitor's rules under them.
//!
//! On one machine every cell is yours to change. In a lobby the host changes
//! the plan and each person picks their own race; an open seat shows who sits
//! there, or a Sit Here for everyone else.

use super::roster::{Control, Seat};
use super::{Ask, Catalog, Lineup, Mode, Occupant, Table};
use crate::audio::Sfx;
use crate::setup::TEAM_COLORS;
use crate::ui::race_picker::race_cell;
use crate::ui::{id, ink, palette, rgb, teams, type_scale, ButtonKind, Id, Rect, Ui};
use glam::Vec2;
use mc_sim::{Difficulty, Doctrine};

const DIFFICULTIES: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
/// A seat's Control list on one machine; a lobby's adds Open (for a person) first.
const CONTROL: [&str; 4] = [
    "Closed",
    "AI \u{b7} Easy",
    "AI \u{b7} Normal",
    "AI \u{b7} Hard",
];
const LOBBY_CONTROL: [&str; 5] = [
    "Open",
    "Closed",
    "AI \u{b7} Easy",
    "AI \u{b7} Normal",
    "AI \u{b7} Hard",
];
const ROW_H: f32 = 54.0;
const ROW_PITCH: f32 = 60.0;
/// The doctrine strip that opens under an AI row.
const TUNE_H: f32 = 74.0;
const FORCE_PRESETS: [[u8; 3]; 4] = [[100, 100, 100], [160, 60, 60], [60, 160, 60], [60, 60, 160]];
const FORCE_LABELS: [&str; 4] = ["Balanced", "Land", "Air", "Naval"];
/// Height of a team's heading over its rows.
const TEAM_HEAD_H: f32 = 30.0;
/// The colour strip that opens under a row.
const SWATCH_H: f32 = 52.0;

/// Where a row's cells sit.
struct Columns {
    race: f32,
    control: f32,
    team: f32,
    zone: f32,
}

impl Columns {
    fn of(area: Rect) -> Columns {
        Columns {
            race: area.right() - 470.0,
            control: area.right() - 358.0,
            team: area.right() - 196.0,
            zone: area.right() - 84.0,
        }
    }
}

/// The commanders column in `area`. On one machine `observe` is the watch
/// switch beside the heading. Returns what the lobby must do.
pub fn commanders(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &mut Catalog,
    table: &Table,
    observe: Option<&mut bool>,
    area: Rect,
) -> Vec<Ask> {
    let mut asks = Vec::new();
    let survival = lineup.mode == Mode::Survival;
    let switch_w = 236.0;
    ui.section(
        area.x,
        area.y + 6.0,
        area.w - switch_w - 16.0,
        if survival { "Defenders" } else { "Commanders" },
    );
    if let Some(observe) = observe {
        watch_switch(ui, observe, area, switch_w);
    }
    let cols = Columns::of(area);
    let head = area.y + 44.0;
    for (x, label) in [
        (area.x + 58.0, "Commander"),
        (cols.race, "Faction"),
        (cols.control, "Control"),
        (cols.team, if survival { "" } else { "Team" }),
        (cols.zone, "Zone"),
    ] {
        ui.text(x, head, type_scale::MICRO, rgb(palette::DIM, 0.8), label);
    }
    // Open strips follow their row; a row that is no longer an editable AI closes its own.
    let is_ai_row = |k: u8| {
        lineup
            .roster
            .index_of(k)
            .is_some_and(|i| table.is_ai(&lineup.roster.seats[i]))
    };
    if lineup.tuning.is_some_and(|k| !is_ai_row(k) || !table.host) {
        lineup.tuning = None;
    }
    if lineup.coloring.is_some_and(|k| {
        !table.host
            || lineup
                .roster
                .index_of(k)
                .is_none_or(|i| !lineup.roster.seats[i].open())
    }) {
        lineup.coloring = None;
    }

    // Rows go in team order under a heading per team when anyone is allied;
    // closed seats follow. A row that changes team eases over to its new side.
    let allied = lineup.roster.allied() && !survival;
    let n = lineup.roster.seats.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| {
        let s = &lineup.roster.seats[i];
        (!s.open(), if allied { s.team } else { 0 }, i)
    });
    let sizes = teams::sizes(&lineup.roster.seated_teams());
    let mut hover_team = None;
    let mut heading: Option<(u8, f32)> = None;
    let mut y = head + 16.0;
    for i in order {
        // An earlier row's change may have moved seats: skip what is gone.
        let Some(&seat) = lineup.roster.seats.get(i) else {
            continue;
        };
        if allied && heading.map(|(t, _)| t) != seat.open().then_some(seat.team) {
            if let Some((t, top)) = heading.take() {
                team_rail(ui, area.x, top, y - (ROW_PITCH - ROW_H), t);
            }
            if seat.open() {
                let hy = glide(ui, id("team-head-y", seat.team as usize), y);
                let count = sizes
                    .iter()
                    .find(|&&(t, _)| t == seat.team)
                    .map_or(0, |&(_, n)| n);
                if team_heading(
                    ui,
                    Rect::new(area.x, hy, area.w, TEAM_HEAD_H - 4.0),
                    seat.team,
                    count,
                    lineup.hover_team == Some(seat.team),
                ) {
                    hover_team = Some(seat.team);
                }
                heading = Some((seat.team, y));
                y += TEAM_HEAD_H;
            } else {
                y += 10.0;
            }
        }
        let row_y = glide(ui, id("slot-y", seat.key as usize), y);
        let row = Rect::new(area.x, row_y, area.w, ROW_H);
        asks.extend(seat_row(ui, lineup, catalog, table, &cols, i, seat, row));
        y += ROW_PITCH;
        if lineup.tuning == Some(seat.key) {
            if let Some(i) = lineup.roster.index_of(seat.key) {
                ai_tuning(
                    ui,
                    &mut lineup.roster.seats[i],
                    Rect::new(row.x + 14.0, row.bottom(), row.w - 14.0, TUNE_H),
                );
            }
            y += TUNE_H;
        }
        if lineup.coloring == Some(seat.key) {
            if let Some(i) = lineup.roster.index_of(seat.key) {
                colour_picker(
                    ui,
                    lineup,
                    i,
                    Rect::new(row.x + 14.0, row.bottom(), row.w - 14.0, SWATCH_H),
                );
            }
            y += SWATCH_H;
        }
    }
    if let Some((t, top)) = heading {
        team_rail(ui, area.x, top, y - (ROW_PITCH - ROW_H), t);
    }
    lineup.hover_team = hover_team;

    let y = y + 8.0;
    if survival {
        let top = y + 12.0;
        let counts = lineup.counts(catalog);
        let live = ui.interactive;
        ui.interactive = live && table.host;
        crate::ui::survival::rules::engagement(
            ui,
            &mut lineup.rules,
            counts,
            &mut lineup.hover_domain,
            Rect::new(area.x, top, area.w, area.bottom() - top),
        );
        ui.interactive = live;
    } else {
        layouts(ui, lineup, catalog, table, area, y);
    }
    asks
}

/// Command or Observe: whether you play your seat or an AI takes it while you watch.
fn watch_switch(ui: &mut Ui, observe: &mut bool, area: Rect, switch_w: f32) {
    for (k, (label, watch)) in [("Command", false), ("Observe", true)]
        .into_iter()
        .enumerate()
    {
        let r = Rect::new(
            area.right() - switch_w + k as f32 * (switch_w * 0.5 + 2.0),
            area.y - 10.0,
            switch_w * 0.5 - 2.0,
            32.0,
        );
        let on = *observe == watch;
        let res = ui.tile(id("seat-mode", k), r, on, true);
        ui.text_centred(
            r.x + r.w * 0.5,
            r.mid_y(),
            type_scale::BUTTON,
            rgb(
                if on { 0xFFFFFF } else { palette::DIM },
                0.85 + 0.15 * res.glow,
            ),
            label,
        );
        if res.clicked && !on {
            *observe = watch;
            ui.audio.play(Sfx::Select);
        }
    }
}

/// One seat's row at `row`.
#[expect(clippy::too_many_arguments, reason = "a row reads the whole table")]
fn seat_row(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &Catalog,
    table: &Table,
    cols: &Columns,
    i: usize,
    seat: Seat,
    row: Rect,
) -> Vec<Ask> {
    let mut asks = Vec::new();
    let key = seat.key as usize;
    let open = seat.open();
    let ai = table.is_ai(&seat);
    let occupant = table.occupant(i);
    let mine = table.me == Some(i);
    let tuning = lineup.tuning == Some(seat.key);
    let live = if open { 1.0 } else { 0.4 };
    ui.fill(row, ink(if mine && table.lobby { 0.55 } else { 0.5 }));
    ui.frame(
        row,
        rgb(
            if tuning {
                palette::ACCENT
            } else {
                palette::LINE
            },
            if tuning { 0.5 } else { 0.12 },
        ),
    );

    // Colour swatch: the host opens every colour under the row.
    let coloring = lineup.coloring == Some(seat.key);
    let c = TEAM_COLORS[seat.color as usize % TEAM_COLORS.len()];
    let hit = Rect::new(row.x, row.y, 48.0, row.h);
    let res = ui.interact(id("slot-color", key), hit, open && table.host);
    let swatch = Rect::new(row.x + 8.0, row.mid_y() - 13.0, 26.0, 26.0).inset(-2.0 * res.glow);
    ui.fill(
        Rect::new(row.x, row.y, 3.0, row.h),
        [c[0], c[1], c[2], live],
    );
    ui.fill(swatch, [c[0], c[1], c[2], live]);
    ui.frame(
        swatch,
        rgb(0xFFFFFF, if coloring { 0.9 } else { 0.15 + 0.6 * res.glow }),
    );
    if open && table.host {
        // A caret in the corner says it opens.
        let k = Vec2::new(swatch.right() - 5.0, swatch.bottom() - 5.0);
        ui.triangle(
            k + Vec2::new(-4.0, 0.0),
            k + Vec2::new(0.0, 0.0),
            k + Vec2::new(0.0, -4.0),
            ink(0.8),
        );
    }
    if res.clicked {
        lineup.coloring = if coloring { None } else { Some(seat.key) };
        lineup.tuning = None;
        ui.audio.play(Sfx::Select);
    }

    // Who: a person, an AI with its doctrine under its name, an open seat, or nobody.
    let name_w = cols.race - row.x - 70.0;
    let x = row.x + 58.0;
    let name = who(lineup, table, i, &seat);
    let waiting = table.lobby && seat.control == Control::Person && occupant.is_none();
    let name_tone = if waiting {
        rgb(palette::DIM, 0.55 + 0.25 * (ui.time * 2.0 + i as f32).sin())
    } else if occupant.is_some() {
        rgb(0xFFFFFF, 1.0)
    } else {
        rgb(palette::TEXT, live)
    };
    let name_y = if open { row.y + 18.0 } else { row.mid_y() };
    ui.text_fit_left(x, name_y, name_w, type_scale::BODY, name_tone, &name);
    if ai && table.host {
        if ai_summary(ui, key, &seat, tuning, x, row, name_w) {
            lineup.tuning = if tuning { None } else { Some(seat.key) };
            lineup.coloring = None;
            ui.audio.play(Sfx::Select);
        }
    } else {
        let (line, tone) = match occupant {
            Some(p) => (p.tags.join("  \u{b7}  "), palette::DIM),
            None if ai => (
                format!(
                    "{}  \u{b7}  {}",
                    doctrine_label(seat.ai.doctrine),
                    force_label(seat.ai.domain_weights)
                ),
                palette::DIM,
            ),
            None if waiting => ("Waiting for a player".to_owned(), palette::FAINT),
            None if seat.control == Control::Person => ("You Command".to_owned(), palette::ACCENT),
            None => (String::new(), palette::DIM),
        };
        ui.text_fit_left(
            x,
            row.y + 38.0,
            name_w,
            type_scale::MICRO,
            rgb(tone, 1.0),
            &line,
        );
    }

    let field = |x: f32, w: f32| Rect::new(x, row.y + 10.0, w, row.h - 20.0);
    // Race: yours in your own seat, the host's for the rest.
    let can_race = if table.lobby {
        mine || (table.host && occupant.is_none())
    } else {
        table.host
    };
    let race = occupant.and_then(|p| p.race).unwrap_or(seat.race);
    if race_cell(
        ui,
        id("slot-race", key),
        field(cols.race - 10.0, 110.0),
        race,
        open && can_race,
    ) {
        lineup.races.open(key, &name, race);
    }

    asks.extend(control_cell(
        ui,
        lineup,
        catalog,
        table,
        i,
        &seat,
        field(cols.control, 150.0),
    ));

    if lineup.mode == Mode::Skirmish {
        let n = lineup.roster.seats.len().max(1);
        let step = ui.stepper(
            id("slot-team", key),
            field(cols.team, 100.0),
            &format!("Team {}", seat.team + 1),
            rgb(palette::TEXT, 1.0),
            open && table.host,
        );
        if step != 0 {
            if let Some(i) = lineup.roster.index_of(seat.key) {
                lineup.roster.seats[i].team = (seat.team as i32 + step).rem_euclid(n as i32) as u8;
            }
        }
    }
    let zones = lineup.zones(catalog);
    let step = ui.stepper(
        id("slot-start", key),
        field(cols.zone, 84.0),
        &format!("{}", seat.start + 1),
        rgb(palette::TEXT, 1.0),
        open && zones > 1 && table.host,
    );
    if step != 0 {
        if let Some(i) = lineup.roster.index_of(seat.key) {
            lineup
                .roster
                .step_unique(i, step, zones, |s| s.start, |s, v| s.start = v);
        }
    }
    asks
}

/// The name a row shows.
fn who(lineup: &Lineup, table: &Table, i: usize, seat: &Seat) -> String {
    match (seat.control, table.occupant(i)) {
        (Control::Closed, _) => "Empty Slot".to_owned(),
        (_, Some(p)) => p.name.clone(),
        _ if table.is_ai(seat) => lineup.ai_name(table, i),
        (Control::Person, None) if table.lobby => "Open Seat".to_owned(),
        _ => table.name.to_owned(),
    }
}

/// An AI's doctrine under its name, which opens its tuning. True when clicked.
fn ai_summary(
    ui: &mut Ui,
    key: usize,
    seat: &Seat,
    tuning: bool,
    x: f32,
    row: Rect,
    name_w: f32,
) -> bool {
    let tune = Rect::new(row.x + 48.0, row.y + 28.0, name_w + 10.0, 22.0);
    let res = ui.interact(id("slot-ai-tune", key), tune, true);
    let summary = format!(
        "{}  \u{b7}  {}",
        doctrine_label(seat.ai.doctrine),
        force_label(seat.ai.domain_weights),
    );
    let tone = rgb(
        if tuning || res.glow > 0.3 {
            palette::ACCENT
        } else {
            palette::DIM
        },
        1.0,
    );
    ui.text_fit_left(
        x,
        row.y + 38.0,
        name_w - 16.0,
        type_scale::MICRO,
        tone,
        &summary,
    );
    // A caret: down to open the tuning, up to close it.
    let tw = ui
        .text_width(type_scale::MICRO, &summary)
        .min(name_w - 16.0);
    let c = Vec2::new(x + tw + 9.0, row.y + 38.0);
    let d = if tuning { -1.0 } else { 1.0 };
    ui.triangle(
        c + Vec2::new(-3.5, -2.0 * d),
        c + Vec2::new(3.5, -2.0 * d),
        c + Vec2::new(0.0, 2.5 * d),
        tone,
    );
    res.clicked
}

/// Who plays the seat: a list for whoever may change it, the person's state,
/// or a Sit Here on an open seat for everyone else in a lobby.
fn control_cell(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &Catalog,
    table: &Table,
    i: usize,
    seat: &Seat,
    f: Rect,
) -> Vec<Ask> {
    let key = seat.key as usize;
    let label = |ui: &mut Ui, text: &str, tone: u32| {
        ui.text(
            f.x + 12.0,
            f.mid_y(),
            type_scale::VALUE,
            rgb(tone, 1.0),
            text,
        );
    };
    let mine = table.me == Some(i);
    // Someone sits here: their state, or the host's hand to remove them.
    if let Some(p) = table.occupant(i) {
        if table.host && !mine {
            if ui.button(
                id("slot-kick", key),
                f,
                "Remove",
                ButtonKind::Secondary,
                true,
            ) {
                ui.audio.play(Sfx::Back);
                return vec![Ask::Kick(i)];
            }
        } else {
            let (text, tone) = state_of(p);
            label(ui, text, tone);
        }
        return Vec::new();
    }
    // Your seat on one machine: yours to play, or an AI's while you watch.
    if !table.lobby && seat.control == Control::Person {
        if !table.observe {
            label(ui, "Player", palette::ACCENT);
            return Vec::new();
        }
        let at = difficulty_at(seat.ai.difficulty);
        if let Some(pick) = ui.dropdown(id("slot-seat", key), f, &CONTROL[1..], at, true) {
            lineup.roster.seats[i].ai.difficulty = DIFFICULTIES[pick];
        }
        return Vec::new();
    }
    if !table.host {
        return match seat.control {
            Control::Person
                if table.lobby
                    && ui.button(
                        id("slot-sit", key),
                        f,
                        "Sit Here",
                        ButtonKind::Secondary,
                        true,
                    ) =>
            {
                ui.audio.play(Sfx::Select);
                vec![Ask::Sit(i)]
            }
            Control::Person => Vec::new(),
            Control::Ai => {
                label(
                    ui,
                    CONTROL[1 + difficulty_at(seat.ai.difficulty)],
                    palette::TEXT,
                );
                Vec::new()
            }
            Control::Closed => {
                label(ui, "Closed", palette::FAINT);
                Vec::new()
            }
        };
    }
    // The host's list: open to a person (a lobby's), closed, or an AI of a difficulty.
    let options: &[&str] = if table.lobby {
        &LOBBY_CONTROL
    } else {
        &CONTROL
    };
    let first_ai = options.len() - 3;
    let at = match seat.control {
        Control::Person => 0,
        Control::Closed => first_ai - 1,
        Control::Ai => first_ai + difficulty_at(seat.ai.difficulty),
    };
    let Some(pick) = ui.dropdown(id("slot-seat", key), f, options, at, true) else {
        return Vec::new();
    };
    let (to, difficulty) = if pick >= first_ai {
        (Control::Ai, Some(DIFFICULTIES[pick - first_ai]))
    } else if pick == first_ai - 1 {
        (Control::Closed, None)
    } else {
        (Control::Person, None)
    };
    let zones = lineup.zones(catalog);
    match lineup
        .roster
        .set_control(i, to, zones, |j| table.occupied(j))
    {
        Ok(at) => {
            if let Some(d) = difficulty {
                lineup.roster.seats[at].ai.difficulty = d;
            }
            if lineup.mode == Mode::Survival {
                lineup.roster.seats[at].team = 0;
            }
            Vec::new()
        }
        Err(e) => {
            ui.audio.play(Sfx::Deny);
            vec![Ask::Say(e)]
        }
    }
}

/// A person's state as their Control cell says it.
fn state_of(p: &Occupant) -> (&'static str, u32) {
    if p.host {
        ("Host", palette::DIM)
    } else if p.ready {
        ("Ready", palette::TEXT)
    } else {
        ("Not Ready", palette::FAINT)
    }
}

fn difficulty_at(d: Difficulty) -> usize {
    DIFFICULTIES.iter().position(|x| *x == d).unwrap_or(1)
}

/// Quick team layouts under the rows: sides drawn from where the landing zones
/// lie, so allies start next to each other whatever the seat order. Then the
/// matchup as it stands.
fn layouts(ui: &mut Ui, lineup: &mut Lineup, catalog: &Catalog, table: &Table, area: Rect, y: f32) {
    if y + 34.0 >= area.bottom() {
        return;
    }
    ui.text(
        area.x,
        y + 16.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Teams",
    );
    let open = lineup.roster.seated_teams().len();
    let options = [
        ("Free for All", open, open > 2),
        ("Two Sides", 2, open > 2),
        ("Pairs", open / 2, open >= 6 && open.is_multiple_of(2)),
    ];
    for (n, (label, groups, enabled)) in options.into_iter().enumerate() {
        let r = Rect::new(area.x + 64.0 + n as f32 * 128.0, y, 120.0, 32.0);
        if ui.button(
            id("team-layout", n),
            r,
            label,
            ButtonKind::Secondary,
            enabled && table.host,
        ) {
            let at = lineup.zone_points(catalog);
            lineup
                .roster
                .teams_by_ground(&at, groups, table.me.unwrap_or(0));
            ui.audio.play(Sfx::Tick);
        }
    }
    let seated = lineup.roster.seated_teams();
    if seated.len() >= 2 {
        let right = area.right();
        let uneven = teams::uneven(&seated) && !teams::free_for_all(&seated);
        ui.text_right(
            right,
            y + 10.0,
            type_scale::ITEM,
            rgb(if uneven { palette::WARN } else { palette::TEXT }, 1.0),
            &teams::matchup(&seated),
        );
        ui.text_right(
            right,
            y + 28.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            if uneven { "Uneven Teams" } else { "Matchup" },
        );
    }
    if y + 60.0 < area.bottom() {
        ui.text(
            area.x,
            y + 56.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            if table.lobby {
                "Open seats nobody takes are played by the AI. AI commanders get no resource bonuses."
            } else {
                "AI commanders get no resource bonuses. Difficulty sets reaction speed and memory."
            },
        );
    }
}

/// A position that eases to `to`; the first time it is seen it starts there.
fn glide(ui: &mut Ui, key: Id, to: f32) -> f32 {
    if !ui.mem.anims.contains_key(&key) {
        ui.snap(key, to);
    }
    ui.ease(key, to, 14.0)
}

/// A team's heading: badge, name and head count. True while pointed at.
fn team_heading(ui: &mut Ui, r: Rect, team: u8, count: usize, lit: bool) -> bool {
    let res = ui.interact_with(id("team-head", team as usize), r, true, false);
    let glow = ui.ease(
        id("team-head-glow", team as usize),
        if lit || res.hovered { 1.0 } else { 0.0 },
        10.0,
    );
    ui.gradient_h(
        r,
        rgb(palette::LINE, 0.06 + 0.06 * glow),
        rgb(palette::LINE, 0.0),
    );
    let end = teams::badge(ui, r.x, r.mid_y(), team, 1.0, glow > 0.5);
    let end = ui.text(
        end + 10.0,
        r.mid_y(),
        type_scale::CAPTION,
        rgb(palette::TEXT, 1.0),
        &format!("Team {}", team + 1),
    );
    ui.text(
        end + 10.0,
        r.mid_y() + 0.5,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!("{count} Commander{}", if count == 1 { "" } else { "s" }),
    );
    res.hovered
}

/// A bracket down the left of a team's rows, from its heading to its last row.
fn team_rail(ui: &mut Ui, x: f32, top: f32, bottom: f32, team: u8) {
    let top = glide(ui, id("team-rail-top", team as usize), top);
    let bottom = glide(ui, id("team-rail-bottom", team as usize), bottom);
    let x = x - 10.0;
    ui.vline(
        x,
        top + 4.0,
        (bottom - top - 4.0).max(0.0),
        rgb(palette::LINE, 0.55),
    );
    ui.hline(x, top + 4.0, 5.0, rgb(palette::LINE, 0.55));
    ui.hline(x, bottom - 1.0, 5.0, rgb(palette::LINE, 0.55));
}

fn force_label(weights: [u8; 3]) -> &'static str {
    FORCE_PRESETS
        .iter()
        .position(|w| *w == weights)
        .map_or("Custom", |i| FORCE_LABELS[i])
}

/// Every colour for seat `i`; one another commander wears shows their seat
/// number, and picking it swaps the two.
fn colour_picker(ui: &mut Ui, lineup: &mut Lineup, i: usize, area: Rect) {
    ui.fill(area, ink(0.35));
    ui.fill(
        Rect::new(area.x, area.y, 2.0, area.h),
        rgb(palette::ACCENT, 0.6),
    );
    ui.text(
        area.x + 12.0,
        area.mid_y(),
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Colour",
    );
    let seats = &lineup.roster.seats;
    let key = seats[i].key as usize;
    let size = 34.0;
    let mut pick = None;
    for (n, c) in TEAM_COLORS.iter().enumerate() {
        let r = Rect::new(
            area.x + 72.0 + n as f32 * (size + 10.0),
            area.mid_y() - size * 0.5,
            size,
            size,
        );
        let mine = seats[i].color as usize == n;
        let holder =
            (0..seats.len()).find(|&k| k != i && seats[k].open() && seats[k].color as usize == n);
        let res = ui.interact(id("slot-swatch", key * 16 + n), r, !mine);
        let grow = r.inset(-2.0 * res.glow);
        ui.fill(
            grow,
            [c[0], c[1], c[2], if holder.is_some() { 0.45 } else { 1.0 }],
        );
        ui.frame(
            grow.inset(-3.0),
            rgb(0xFFFFFF, if mine { 0.95 } else { 0.5 * res.glow }),
        );
        if let Some(k) = holder {
            ui.text_centred(
                r.x + r.w * 0.5,
                r.mid_y(),
                type_scale::VALUE,
                rgb(0xFFFFFF, 0.95),
                &format!("{}", k + 1),
            );
        }
        if res.glow > 0.3 && holder.is_some() {
            ui.text_right(
                area.right() - 12.0,
                area.mid_y(),
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                "Taken \u{b7} Click to Swap",
            );
        }
        if res.clicked {
            pick = Some(n as u8);
        }
    }
    if let Some(n) = pick {
        lineup.roster.take_color(i, n);
        lineup.coloring = None;
        ui.audio.play(Sfx::Tick);
    }
}

/// Doctrine, force preference and adaptation for an AI seat, opened under its row.
fn ai_tuning(ui: &mut Ui, seat: &mut Seat, area: Rect) {
    ui.fill(area, ink(0.35));
    ui.fill(
        Rect::new(area.x, area.y, 2.0, area.h),
        rgb(palette::ACCENT, 0.6),
    );
    let key = seat.key as usize;
    let ai = &mut seat.ai;
    let gap = 14.0;
    let w = (area.w - 24.0 - 2.0 * gap) / 3.0;
    let field = |k: f32| Rect::new(area.x + 12.0 + k * (w + gap), area.y + 30.0, w, 32.0);
    for (k, label) in ["Doctrine", "Force Preference", "Adaptation"]
        .into_iter()
        .enumerate()
    {
        let f = field(k as f32);
        ui.text(
            f.x,
            f.y - 12.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            label,
        );
    }
    const DOCTRINES: [Doctrine; 4] = [
        Doctrine::Adaptive,
        Doctrine::Aggressive,
        Doctrine::Economic,
        Doctrine::Defensive,
    ];
    let at = DOCTRINES
        .iter()
        .position(|d| *d == ai.doctrine)
        .unwrap_or(0);
    let labels = DOCTRINES.map(doctrine_label);
    if let Some(pick) = ui.dropdown(id("ai-doctrine", key), field(0.0), &labels, at, true) {
        ai.doctrine = DOCTRINES[pick];
    }
    // A custom mix (from a saved config) shows as its nearest preset until changed.
    let at = FORCE_PRESETS
        .iter()
        .position(|w| *w == ai.domain_weights)
        .unwrap_or(0);
    if let Some(pick) = ui.dropdown(id("ai-domain", key), field(1.0), &FORCE_LABELS, at, true) {
        ai.domain_weights = FORCE_PRESETS[pick];
    }
    let step = ui.stepper(
        id("ai-adaptation", key),
        field(2.0),
        &format!("{}%", ai.adaptation),
        rgb(palette::TEXT, 1.0),
        true,
    );
    if step != 0 {
        ai.adaptation = ((ai.adaptation as i32 / 25 + step).rem_euclid(5) * 25) as u8;
    }
}

fn doctrine_label(d: Doctrine) -> &'static str {
    match d {
        Doctrine::Adaptive => "Adaptive",
        Doctrine::Aggressive => "Aggressive",
        Doctrine::Economic => "Economic",
        Doctrine::Defensive => "Defensive",
    }
}

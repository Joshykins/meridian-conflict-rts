//! The lobby screen: the map and rules with the room's chat on the left, the
//! chart of landing zones in the middle, and the seats on the right. The host
//! sets the seats up; each person picks a race, takes a seat and readies up.

use super::browse::header;
use super::lobby::{Lobby, Place, Plan, OPEN_NAME};
use super::{choice_of, MultiplayerAction, MultiplayerState, Page, CHART_SLOT};
use crate::audio::Sfx;
use crate::setup::TEAM_COLORS;
use crate::ui::faction::{self, Pick};
use crate::ui::maps::BrowserAction;
use crate::ui::skirmish::theatre_card;
use crate::ui::{id, ink, palette, preview, rgb, teams, type_scale, ButtonKind, Key, Rect, Ui};
use glam::Vec2;
use mc_net::{Link, LobbyPlayer};
use mc_sim::Difficulty;

const LEFT: f32 = 64.0;
const SIDE_W: f32 = 380.0;
const SEATS_W: f32 = 640.0;
const SEAT_H: f32 = 58.0;

/// One seat as the screen shows it, from the plan (host) or the options (everyone else).
struct SeatView {
    person: Option<LobbyPlayer>,
    ai: bool,
    name: String,
    team: u8,
    start: u8,
    color: [f32; 3],
    faction: String,
    difficulty: Difficulty,
}

fn seats(lobby: &Lobby) -> Vec<SeatView> {
    let Some(o) = &lobby.options else {
        return Vec::new();
    };
    let people = lobby.state.as_ref().map_or(&[][..], |s| &s.players[..]);
    o.config
        .players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let person = people.iter().find(|q| q.slot.index() == i).cloned();
            let faction = person
                .as_ref()
                .and_then(|q| crate::match_options::SeatChoice::decode(&q.setup))
                .map(|c| c.faction)
                .filter(|f| !f.is_empty())
                .unwrap_or_else(|| p.faction.clone());
            SeatView {
                name: person
                    .as_ref()
                    .map_or_else(|| p.name.clone(), |q| q.name.clone()),
                ai: p.name != OPEN_NAME,
                person,
                team: p.team,
                start: p.start,
                color: o.colors[i],
                faction,
                difficulty: p.ai.difficulty,
            }
        })
        .collect()
}

pub(super) fn draw(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    lobby: &mut Lobby,
    enter: f32,
) -> Option<MultiplayerAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    let browsing = state.browser.is_open();
    let live = ui.interactive;
    ui.interactive = live && !browsing;
    let caption = if lobby.title.is_empty() {
        "Waiting for commanders".to_owned()
    } else {
        lobby.title.clone()
    };
    header(ui, "Lobby", &caption, enter);
    where_chip(ui, lobby);

    if let Some(error) = lobby.error.clone() {
        failed(ui, state, &error);
        ui.fade = 1.0;
        ui.shift.y = 0.0;
        ui.interactive = live;
        return None;
    }
    if lobby.connecting() || lobby.options.is_none() {
        joining(ui, state, lobby);
        ui.fade = 1.0;
        ui.shift.y = 0.0;
        ui.interactive = live;
        return None;
    }

    let (top, bottom) = (160.0, h - 172.0);
    let side = Rect::new(LEFT, top, SIDE_W, bottom - top);
    let right = Rect::new(w - LEFT - SEATS_W, top, SEATS_W, bottom - top);
    for r in [side, right] {
        ui.panel(Rect::new(r.x - 22.0, top - 20.0, r.w + 44.0, r.h + 40.0));
    }
    let centre = Rect::new(
        side.right() + 60.0,
        top,
        right.x - side.right() - 120.0,
        bottom - top,
    );
    let map = lobby.map_index(&state.maps);
    let host = lobby.is_host();
    theatre(ui, state, lobby, map, host, side);
    let chat_top = side.y + 350.0;
    chat(
        ui,
        lobby,
        Rect::new(side.x, chat_top, side.w, side.bottom() - chat_top),
    );
    let views = seats(lobby);
    chart(ui, state, map, &views, centre);
    let starts = map.and_then(|i| state.maps.get(i)).map_or(2, |m| m.starts);
    seat_list(ui, lobby, &views, starts, right);
    let action = footer(ui, state, lobby, &views, map.is_some());
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    ui.interactive = live;

    // The map browser over it all, for the host.
    let (fade, shift) = (ui.fade, ui.shift);
    ui.fade = enter;
    if let Some(BrowserAction::Pick(i)) =
        state
            .browser
            .draw(ui, &state.maps, "Choose a Map", CHART_SLOT)
    {
        change_map(state, lobby, i);
    }
    if state.browser.release_slot() {
        state.chart_lost();
    }
    (ui.fade, ui.shift) = (fade, shift);
    countdown(ui, lobby);
    action
}

/// The room's code (or address), top right: what to tell friends.
fn where_chip(ui: &mut Ui, lobby: &Lobby) {
    let (label, value) = match &lobby.place {
        Place::Server { code, private } => (
            if *private {
                "Private Game  \u{b7}  Code"
            } else {
                "Game Code"
            },
            code.to_string(),
        ),
        Place::Lan { addr } => ("On This Network  \u{b7}  Join At", addr.clone()),
    };
    let right = ui.size.x - LEFT;
    ui.text_right(
        right,
        72.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        label,
    );
    ui.text_right(
        right,
        98.0,
        type_scale::TITLE,
        rgb(palette::ACCENT, 1.0),
        &value,
    );
}

fn failed(ui: &mut Ui, state: &mut MultiplayerState, error: &str) {
    let (w, h) = (ui.size.x, ui.size.y);
    let r = Rect::new((w - 620.0) * 0.5, h * 0.34, 620.0, 210.0);
    ui.panel(r);
    ui.text_centred(
        w * 0.5,
        r.y + 46.0,
        type_scale::TITLE,
        rgb(palette::WARN, 1.0),
        "Could Not Join",
    );
    let mut y = r.y + 92.0;
    for line in ui.wrap(type_scale::BODY, error, r.w - 80.0) {
        ui.text_centred(w * 0.5, y, type_scale::BODY, rgb(palette::TEXT, 0.9), &line);
        y += 22.0;
    }
    if ui.button(
        id("lobby-failed-back", 0),
        Rect::new(w * 0.5 - 120.0, r.bottom() - 62.0, 240.0, 42.0),
        "Back to Games",
        ButtonKind::Primary,
        true,
    ) || ui.input.key(Key::Escape)
    {
        ui.audio.play(Sfx::Back);
        state.next = Some(Page::Browse);
    }
}

fn joining(ui: &mut Ui, state: &mut MultiplayerState, lobby: &Lobby) {
    let (w, h) = (ui.size.x, ui.size.y);
    let t = ui.time;
    let c = Vec2::new(w * 0.5, h * 0.42);
    ui.arc(
        c,
        26.0,
        t * 4.0,
        t * 4.0 + 4.2,
        2.4,
        rgb(palette::TEXT, 1.0),
    );
    ui.arc(
        c,
        34.0,
        -t * 2.0,
        -t * 2.0 + 1.4,
        1.2,
        rgb(palette::ACCENT, 0.6),
    );
    let what = if lobby.plan.is_some() {
        "Opening the Lobby"
    } else {
        "Joining"
    };
    ui.text_centred(
        c.x,
        c.y + 70.0,
        type_scale::TITLE,
        rgb(palette::TEXT, 0.95),
        what,
    );
    if ui.button(
        id("lobby-join-cancel", 0),
        Rect::new(c.x - 100.0, c.y + 112.0, 200.0, 42.0),
        "Cancel",
        ButtonKind::Secondary,
        true,
    ) || ui.input.key(Key::Escape)
    {
        ui.audio.play(Sfx::Back);
        state.next = Some(Page::Browse);
    }
}

fn theatre(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    lobby: &mut Lobby,
    map: Option<usize>,
    host: bool,
    r: Rect,
) {
    ui.section(r.x, r.y + 6.0, r.w, "Theatre");
    match map {
        Some(i) => {
            let live = ui.interactive;
            ui.interactive = live && host;
            if theatre_card(
                ui,
                &mut state.browser,
                &state.maps,
                i,
                Rect::new(r.x, r.y + 30.0, r.w, 210.0),
            ) && host
            {
                state.browser.open(i);
            }
            ui.interactive = live;
        }
        None => {
            let name = lobby
                .options
                .as_ref()
                .map_or("", |o| o.map.as_str())
                .to_owned();
            let card = Rect::new(r.x, r.y + 30.0, r.w, 170.0);
            ui.fill(card, ink(0.5));
            ui.frame(card, rgb(palette::WARN, 0.6));
            ui.text(
                card.x + 18.0,
                card.y + 30.0,
                type_scale::ITEM,
                rgb(palette::WARN, 1.0),
                "Missing Map",
            );
            let body = format!(
                "The host picked {name}, which this game does not have. Ask for another map, or update the game."
            );
            let mut y = card.y + 62.0;
            for line in ui.wrap(type_scale::BODY, &body, card.w - 36.0) {
                ui.text(
                    card.x + 18.0,
                    y,
                    type_scale::BODY,
                    rgb(palette::TEXT, 0.9),
                    &line,
                );
                y += 21.0;
            }
        }
    }
    // Rules: the host sets them, everyone sees them.
    let y = r.y + 260.0;
    ui.section(r.x, y, r.w, "Rules");
    let row = Rect::new(r.x, y + 22.0, r.w, 42.0);
    match &mut lobby.plan {
        Some(plan) => {
            ui.toggle(id("lobby-fog", 0), row, "Fog of War", "", &mut plan.fog);
        }
        None => {
            let fog = lobby.options.as_ref().is_some_and(|o| o.config.fog);
            ui.text(
                row.x + 16.0,
                row.mid_y(),
                type_scale::BODY,
                rgb(palette::TEXT, 0.82),
                "Fog of War",
            );
            ui.text_right(
                row.right() - 12.0,
                row.mid_y(),
                type_scale::VALUE,
                rgb(palette::TEXT, 1.0),
                if fog { "On" } else { "Off" },
            );
        }
    }
}

/// The host picked another map: seats beyond its zones go, unless someone sits in them.
fn change_map(state: &mut MultiplayerState, lobby: &mut Lobby, map: usize) {
    let Some(card) = state.maps.get(map) else {
        return;
    };
    let occupied = lobby.state.as_ref().map_or(0, |s| {
        s.players
            .iter()
            .map(|p| p.slot.index() + 1)
            .max()
            .unwrap_or(0)
    });
    let Some(plan) = &mut lobby.plan else { return };
    if card.starts < occupied {
        state.say(format!(
            "{} has {} landing zones; seat {occupied} is taken.",
            card.name, card.starts
        ));
        return;
    }
    plan.map = map;
    plan.count = plan.count.clamp(2, card.starts.max(2));
    for (i, s) in plan.seats.iter_mut().enumerate() {
        s.start = (i % card.starts.max(1)) as u8;
    }
}

fn chat(ui: &mut Ui, lobby: &mut Lobby, r: Rect) {
    ui.section(r.x, r.y, r.w, "Chat");
    let input = Rect::new(r.x, r.bottom() - 40.0, r.w, 40.0);
    let area = Rect::new(r.x, r.y + 22.0, r.w, input.y - r.y - 30.0);
    let line_h = 21.0;
    let mut lines: Vec<(Option<u8>, String, String)> = Vec::new();
    for c in lobby.chat.iter().rev() {
        let who = if c.from.is_none() && c.name.is_empty() {
            String::new()
        } else {
            format!("{}:", c.name)
        };
        let prefix_w = ui.text_width(type_scale::VALUE, &who) + 8.0;
        let wrapped = ui.wrap(type_scale::BODY, &c.text, r.w - prefix_w);
        for (k, l) in wrapped.iter().enumerate().rev() {
            lines.push((
                c.from,
                if k == 0 { who.clone() } else { String::new() },
                l.clone(),
            ));
        }
        if lines.len() as f32 * line_h > area.h {
            break;
        }
    }
    let mut y = area.bottom() - line_h * 0.5;
    for (from, who, text) in &lines {
        if y < area.y {
            break;
        }
        let mut x = r.x;
        if !who.is_empty() {
            let tone = from.map_or(rgb(palette::DIM, 1.0), |s| {
                let c = TEAM_COLORS[s as usize % 8];
                [c[0], c[1], c[2], 1.0]
            });
            x = ui.text(x, y, type_scale::VALUE, tone, who) + 8.0;
        } else if from.is_none() {
            ui.text(x, y, type_scale::BODY, rgb(palette::WARN, 1.0), text);
            y -= line_h;
            continue;
        }
        ui.text(x, y, type_scale::BODY, rgb(palette::TEXT, 0.9), text);
        y -= line_h;
    }
    if lobby.chat.is_empty() {
        ui.text(
            r.x,
            area.y + 12.0,
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "Say hello: type below and press Enter.",
        );
    }
    let field = id("lobby-chat", 0);
    ui.text_field(field, input, &mut lobby.draft, 200);
    if lobby.draft.is_empty() && ui.mem.editing != Some(field) {
        ui.text(
            input.x + 12.0,
            input.mid_y(),
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "Message the lobby",
        );
    }
    if ui.mem.editing == Some(field) && ui.input.key(Key::Enter) {
        lobby.send_chat();
        // Keep the keyboard: chat is a conversation.
        ui.mem.editing = Some(field);
    }
}

fn chart(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    map: Option<usize>,
    views: &[SeatView],
    area: Rect,
) {
    let Some(i) = map else { return };
    let card = &state.maps[i];
    if state.chart_of != Some(i) {
        ui.o.set_image(
            CHART_SLOT,
            preview::SIZE,
            preview::SIZE,
            &preview::render(&card.map, card.climate),
        );
        state.chart_of = Some(i);
    }
    let side = area.w.min(area.h - 40.0);
    let frame = Rect::new(area.x + (area.w - side) * 0.5, area.y, side, side);
    let shown = ui.ease(id("lobby-chart", i), 1.0, 5.0);
    ui.fill(frame, ink(0.85));
    ui.image(
        CHART_SLOT,
        [0.0, 0.0, preview::SIZE as f32, preview::SIZE as f32],
        frame,
        [shown, shown, shown, 1.0],
    );
    ui.frame(frame, rgb(palette::LINE, 0.25));
    ui.brackets(frame.inset(-6.0), 14.0, rgb(palette::ACCENT, 0.7));
    let starts = card.map.start_positions();
    let at: Vec<Vec2> = views
        .iter()
        .map(|v| {
            let p = starts
                .get(v.start as usize)
                .map_or([0.0; 2], |p| p.to_f32());
            Vec2::new(frame.x, frame.y) + preview::locate(&card.map, p, side)
        })
        .collect();
    let team_of: Vec<u8> = views.iter().map(|v| v.team).collect();
    for (a, b) in teams::links(&at, &team_of) {
        let d = (at[b] - at[a]).normalize_or_zero();
        ui.stroke(
            at[a] + d * 17.0,
            at[b] - d * 17.0,
            1.4,
            rgb(palette::LINE, 0.4),
        );
    }
    for (k, (v, p)) in views.iter().zip(&at).enumerate() {
        let c = [v.color[0], v.color[1], v.color[2], 1.0];
        ui.disc(*p, 13.0, ink(0.7));
        ui.arc(*p, 13.0, 0.0, std::f32::consts::TAU, 2.0, c);
        ui.text_centred(
            p.x,
            p.y,
            type_scale::CAPTION,
            rgb(0xFFFFFF, 1.0),
            &(k + 1).to_string(),
        );
    }
    // Zones nobody starts on, faint.
    for (z, s) in starts.iter().enumerate() {
        if views.iter().any(|v| v.start as usize == z) {
            continue;
        }
        let p = Vec2::new(frame.x, frame.y) + preview::locate(&card.map, s.to_f32(), side);
        ui.arc(
            p,
            9.0,
            0.0,
            std::f32::consts::TAU,
            1.0,
            rgb(palette::LINE, 0.35),
        );
    }
    let matchup = teams::matchup(&team_of);
    ui.text_centred(
        frame.x + frame.w * 0.5,
        frame.bottom() + 22.0,
        type_scale::VALUE,
        rgb(palette::TEXT, 0.9),
        &matchup,
    );
}

/// Next value of `get` among `count`, skipping ones another seat has.
fn step_unique(
    plan: &Plan,
    i: usize,
    count: usize,
    get: impl Fn(&super::lobby::SeatPlan) -> u8,
) -> u8 {
    let mut v = get(&plan.seats[i]);
    for _ in 0..count {
        v = ((v as usize + 1) % count) as u8;
        if !(0..plan.count).any(|j| j != i && get(&plan.seats[j]) == v) {
            return v;
        }
    }
    get(&plan.seats[i])
}

fn seat_list(ui: &mut Ui, lobby: &mut Lobby, views: &[SeatView], starts: usize, r: Rect) {
    ui.section(r.x, r.y + 6.0, r.w, "Commanders");
    let me = lobby.slot;
    let host = lobby.is_host();
    // The host sets how many seats there are.
    if let Some(plan) = &mut lobby.plan {
        // Not below a seat someone sits in; not above the map's landing zones.
        let occupied = lobby.state.as_ref().map_or(1, |s| {
            s.players
                .iter()
                .map(|p| p.slot.index() + 1)
                .max()
                .unwrap_or(1)
        });
        let max = starts.clamp(2, 8);
        let label = format!("{} Seats", plan.count);
        let stepper = Rect::new(r.right() - 190.0, r.y - 8.0, 190.0, 34.0);
        let step = ui.stepper(
            id("lobby-seats", 0),
            stepper,
            &label,
            rgb(palette::TEXT, 1.0),
            true,
        );
        if step != 0 {
            let next = (plan.count as i32 + step).clamp(occupied.max(2) as i32, max.max(2) as i32);
            plan.count = next as usize;
        }
    }
    let mut kick = None;
    let mut sit = None;
    for (i, v) in views.iter().enumerate() {
        let row = Rect::new(r.x, r.y + 34.0 + i as f32 * (SEAT_H + 6.0), r.w, SEAT_H);
        let mine = me == Some(i as u8);
        ui.fill(row, ink(if mine { 0.55 } else { 0.35 }));
        if mine {
            ui.fill(
                Rect::new(row.x, row.y, 3.0, row.h),
                rgb(palette::ACCENT, 1.0),
            );
        }
        let c = [v.color[0], v.color[1], v.color[2], 1.0];
        // Colour, then the seat number: the host clicks the colour to change it.
        let sw = Rect::new(row.x + 14.0, row.mid_y() - 9.0, 18.0, 18.0);
        let res = ui.interact(id("lobby-color", i), sw.inset(-4.0), host);
        ui.fill(sw, c);
        if res.glow > 0.05 {
            ui.frame(sw.inset(-3.0), rgb(palette::TEXT, res.glow));
        }
        if res.clicked {
            if let Some(plan) = &mut lobby.plan {
                ui.audio.play(Sfx::Tick);
                plan.seats[i].color = step_unique(plan, i, 8, |s| s.color);
            }
        }
        ui.text(
            row.x + 44.0,
            row.mid_y(),
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &(i + 1).to_string(),
        );
        // Who: a person, an AI, or an open seat.
        let x = row.x + 62.0;
        match &v.person {
            Some(p) => {
                ui.text(
                    x,
                    row.y + 22.0,
                    type_scale::ITEM,
                    rgb(0xFFFFFF, 1.0),
                    &p.name,
                );
                let mut tags = Vec::new();
                if lobby.state.as_ref().is_some_and(|s| s.host == Some(p.slot)) {
                    tags.push("Host".to_owned());
                }
                if mine {
                    tags.push("You".to_owned());
                }
                if p.verified {
                    tags.push("Verified".to_owned());
                }
                if let Some(st) = lobby.stats.iter().find(|s| s.slot == p.slot) {
                    match st.link {
                        Link::Connected if st.rtt_ms > 0 => tags.push(format!("{} ms", st.rtt_ms)),
                        Link::Dropped => tags.push("Disconnected".into()),
                        _ => {}
                    }
                }
                ui.text(
                    x,
                    row.y + 42.0,
                    type_scale::MICRO,
                    rgb(palette::DIM, 1.0),
                    &tags.join("  \u{b7}  "),
                );
            }
            None if v.ai => {
                ui.text(
                    x,
                    row.y + 22.0,
                    type_scale::ITEM,
                    rgb(palette::TEXT, 0.85),
                    &v.name,
                );
                let label = match v.difficulty {
                    Difficulty::Easy => "AI  \u{b7}  Easy",
                    Difficulty::Normal => "AI  \u{b7}  Normal",
                    Difficulty::Hard => "AI  \u{b7}  Hard",
                };
                let lr = Rect::new(x - 4.0, row.y + 34.0, 150.0, 18.0);
                let res = ui.interact(id("lobby-difficulty", i), lr, host);
                ui.text(
                    x,
                    row.y + 42.0,
                    type_scale::MICRO,
                    rgb(
                        if res.glow > 0.05 {
                            palette::TEXT
                        } else {
                            palette::DIM
                        },
                        1.0,
                    ),
                    label,
                );
                if res.clicked {
                    if let Some(plan) = &mut lobby.plan {
                        ui.audio.play(Sfx::Tick);
                        let d = &mut plan.seats[i].difficulty;
                        *d = match d {
                            Difficulty::Easy => Difficulty::Normal,
                            Difficulty::Normal => Difficulty::Hard,
                            Difficulty::Hard => Difficulty::Easy,
                        };
                    }
                }
            }
            None => {
                let pulse = 0.55 + 0.25 * (ui.time * 2.0 + i as f32).sin();
                ui.text(
                    x,
                    row.y + 22.0,
                    type_scale::ITEM,
                    rgb(palette::DIM, pulse),
                    "Open Seat",
                );
                ui.text_fit_left(
                    x,
                    row.y + 42.0,
                    220.0,
                    type_scale::MICRO,
                    rgb(palette::FAINT, 1.0),
                    "Waiting  \u{b7}  the AI plays it if nobody comes",
                );
            }
        }
        // Race: yours to pick in your seat; the host picks for AI and open seats.
        let race_r = Rect::new(row.x + 300.0, row.mid_y() - 15.0, 84.0, 30.0);
        let can_race = mine || (host && v.person.is_none());
        let res = ui.tile(id("lobby-race", i), race_r, false, can_race);
        faction::sigil(
            ui,
            &v.faction,
            Vec2::new(race_r.x + 16.0, race_r.mid_y()),
            8.0,
            1.0,
        );
        let abbr = faction::race_by_key(&v.faction).map_or("?", |r| r.abbreviation.as_str());
        ui.text(
            race_r.x + 30.0,
            race_r.mid_y(),
            type_scale::VALUE,
            rgb(
                palette::TEXT,
                if can_race { 0.9 + 0.1 * res.glow } else { 0.6 },
            ),
            abbr,
        );
        if res.clicked && can_race {
            ui.audio.play(Sfx::Tick);
            let all = faction::races();
            let now = all
                .iter()
                .position(|r| r.key.eq_ignore_ascii_case(&v.faction))
                .unwrap_or(0);
            let next = Pick::Race(((now + 1) % all.len().max(1)) as u8);
            if mine {
                lobby.set_choice(choice_of(next));
            } else if let Some(plan) = &mut lobby.plan {
                plan.seats[i].race = next;
            }
        }
        // Team, then landing zone: the host's.
        let team_r = Rect::new(row.x + 396.0, row.mid_y() - 12.0, 40.0, 24.0);
        let res = ui.interact(id("lobby-team", i), team_r, host);
        teams::badge(ui, team_r.x, row.mid_y(), v.team, 1.0, res.glow > 0.05);
        if res.clicked {
            if let Some(plan) = &mut lobby.plan {
                ui.audio.play(Sfx::Tick);
                plan.seats[i].team = (plan.seats[i].team + 1) % plan.count as u8;
            }
        }
        let zone_r = Rect::new(row.x + 446.0, row.mid_y() - 12.0, 70.0, 24.0);
        let res = ui.interact(id("lobby-zone", i), zone_r, host);
        ui.text(
            zone_r.x,
            row.mid_y(),
            type_scale::CAPTION,
            rgb(
                if res.glow > 0.05 {
                    palette::TEXT
                } else {
                    palette::DIM
                },
                1.0,
            ),
            &format!("Zone {}", v.start + 1),
        );
        if res.clicked {
            if let Some(plan) = &mut lobby.plan {
                ui.audio.play(Sfx::Tick);
                plan.seats[i].start = step_unique(plan, i, starts.clamp(1, 8), |s| s.start);
            }
        }
        // The right end: ready, or the host's hand on the seat, or a place to sit.
        let end = Rect::new(row.right() - 112.0, row.mid_y() - 16.0, 100.0, 32.0);
        match &v.person {
            Some(p) => {
                let is_host = lobby.state.as_ref().is_some_and(|s| s.host == Some(p.slot));
                if host && !mine {
                    if ui.button(
                        id("lobby-kick", i),
                        end,
                        "Remove",
                        ButtonKind::Secondary,
                        true,
                    ) {
                        kick = Some(i as u8);
                    }
                } else {
                    let (text, tone) = if is_host {
                        ("Hosting", palette::DIM)
                    } else if p.ready {
                        ("Ready", palette::TEXT)
                    } else {
                        ("Not Ready", palette::FAINT)
                    };
                    ui.text_right(
                        end.right(),
                        end.mid_y(),
                        type_scale::VALUE,
                        rgb(tone, 1.0),
                        text,
                    );
                }
                if host && !mine && p.ready {
                    ui.text_right(
                        end.x - 10.0,
                        end.mid_y(),
                        type_scale::MICRO,
                        rgb(palette::TEXT, 0.9),
                        "Ready",
                    );
                }
            }
            None if host => {
                let label = if v.ai { "Make Open" } else { "Make AI" };
                if ui.button(id("lobby-open", i), end, label, ButtonKind::Secondary, true) {
                    if let Some(plan) = &mut lobby.plan {
                        ui.audio.play(Sfx::Tick);
                        plan.seats[i].ai = !plan.seats[i].ai;
                    }
                }
            }
            None if !v.ai
                && ui.button(
                    id("lobby-sit", i),
                    end,
                    "Sit Here",
                    ButtonKind::Secondary,
                    true,
                ) =>
            {
                sit = Some(i as u8);
            }
            None => {}
        }
    }
    if let Some(k) = kick {
        ui.audio.play(Sfx::Back);
        lobby.kick(k);
    }
    if let Some(s) = sit {
        ui.audio.play(Sfx::Select);
        lobby.take_seat(s);
    }
    let watching = lobby.state.as_ref().map_or(0, |s| s.observers);
    if watching > 0 {
        let y = r.y + 34.0 + views.len() as f32 * (SEAT_H + 6.0) + 18.0;
        ui.text(
            r.x,
            y,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("{watching} watching"),
        );
    }
}

fn footer(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    lobby: &mut Lobby,
    views: &[SeatView],
    have_map: bool,
) -> Option<MultiplayerAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    let leave = ui.button(
        id("lobby-leave", 0),
        Rect::new(LEFT, h - 64.0 - 52.0, 200.0, 52.0),
        "Leave",
        ButtonKind::Secondary,
        true,
    );
    let go = Rect::new(w - LEFT - 300.0, h - 64.0 - 58.0, 300.0, 58.0);
    let host = lobby.is_host();
    let waiting: Vec<String> = views
        .iter()
        .filter_map(|v| v.person.as_ref())
        .filter(|p| !p.ready && Some(p.slot) != lobby.state.as_ref().and_then(|s| s.host))
        .map(|p| p.name.clone())
        .collect();
    let team_list: Vec<u8> = views.iter().map(|v| v.team).collect();
    let sides = teams::sizes(&team_list).len();
    let empty = views.iter().filter(|v| v.person.is_none() && !v.ai).count();
    let (line, tone, can) = if !have_map {
        (
            "This game does not have the map".to_owned(),
            palette::WARN,
            false,
        )
    } else if sides < 2 {
        (
            "Everyone is on the same team".to_owned(),
            palette::WARN,
            false,
        )
    } else if !waiting.is_empty() {
        (
            format!("Waiting for {}", waiting.join(", ")),
            palette::DIM,
            false,
        )
    } else if empty > 0 {
        (
            format!(
                "{empty} open seat{} will be played by the AI",
                if empty == 1 { "" } else { "s" }
            ),
            palette::DIM,
            true,
        )
    } else {
        (
            format!("{}  \u{b7}  everyone is ready", teams::matchup(&team_list)),
            palette::DIM,
            true,
        )
    };
    ui.text_right(
        go.x - 24.0,
        go.mid_y(),
        type_scale::CAPTION,
        rgb(tone, 1.0),
        &line,
    );
    if let Some((text, _)) = &state.notice {
        ui.text_right(
            go.x - 24.0,
            go.mid_y() - 26.0,
            type_scale::CAPTION,
            rgb(palette::WARN, 1.0),
            text,
        );
    }
    if host {
        if ui.button(
            id("lobby-start", 0),
            go,
            "Start Match",
            ButtonKind::Primary,
            can,
        ) && can
        {
            ui.audio.play(Sfx::Launch);
            lobby.start();
        }
    } else {
        let ready = lobby.ready();
        let label = if ready { "Not Ready" } else { "Ready" };
        let kind = if ready {
            ButtonKind::Secondary
        } else {
            ButtonKind::Primary
        };
        if ui.button(id("lobby-ready", 0), go, label, kind, have_map) && have_map {
            ui.audio
                .play(if ready { Sfx::ToggleOff } else { Sfx::ToggleOn });
            lobby.set_ready(!ready);
        }
    }
    let typing = ui.mem.editing.is_some();
    if leave || (ui.input.key(Key::Escape) && !typing && ui.interactive) {
        ui.audio.play(Sfx::Back);
        state.next = Some(Page::Browse);
    }
    None
}

/// The last seconds before the start, over everything.
fn countdown(ui: &mut Ui, lobby: &Lobby) {
    let Some(ms) = lobby.countdown_left() else {
        return;
    };
    let (w, h) = (ui.size.x, ui.size.y);
    let band = Rect::new(0.0, h * 0.5 - 70.0, w, 140.0);
    ui.scrim(Rect::new(0.0, band.y, w * 0.5, band.h), 0.0, 0.8, true);
    ui.scrim(Rect::new(w * 0.5, band.y, w * 0.5, band.h), 0.8, 0.0, true);
    let secs = (ms as f32 / 1000.0).ceil().max(1.0);
    ui.text_centred(
        w * 0.5,
        band.y + 50.0,
        type_scale::DISPLAY,
        rgb(0xFFFFFF, 1.0),
        &format!("{secs:.0}"),
    );
    ui.text_centred(
        w * 0.5,
        band.y + 104.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        "Deploying  \u{b7}  anyone who changes their mind stops the count",
    );
}

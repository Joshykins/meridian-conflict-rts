//! The browser: who you are, which server, and the games to join, on the server
//! and on this network. When the server cannot be reached the list says why and
//! what to try, and games on this network still work.

use super::server::{Problem, Status};
use super::{MultiplayerAction, MultiplayerState, Tab};
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use glam::Vec2;
use mc_net::{LanGame, Role, RoomListing, RoomPhase};

const LEFT: f32 = 64.0;
const SIDE_W: f32 = 400.0;
const ROW_H: f32 = 66.0;

/// The page header shared by the multiplayer pages.
fn header(ui: &mut Ui, title: &str, caption: &str, enter: f32) {
    let (w, h) = (ui.size.x, ui.size.y);
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.66 * enter));
    ui.scrim(Rect::new(0.0, 0.0, w, 220.0), 0.6 * enter, 0.0, false);
    ui.fade = enter;
    ui.shift.y = 14.0 * (1.0 - enter);
    ui.emblem(Vec2::new(LEFT + 15.0, 84.0), 13.0, rgb(palette::TEXT, 0.9));
    let end = ui.text(
        LEFT + 50.0,
        84.0,
        type_scale::TITLE,
        rgb(0xFFFFFF, 1.0),
        title,
    );
    ui.text(
        end + 18.0,
        90.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        caption,
    );
    ui.fill(Rect::new(LEFT, 124.0, 58.0, 2.0), rgb(palette::ACCENT, 1.0));
    ui.gradient_h(
        Rect::new(LEFT + 66.0, 124.0, w - 2.0 * LEFT - 66.0, 1.0),
        rgb(palette::LINE, 0.35),
        rgb(palette::LINE, 0.04),
    );
}

/// A label and a line of help under a section heading's row.
fn caption(ui: &mut Ui, x: f32, y: f32, w: f32, text: &str, tone: u32) {
    ui.text_fit_left(x, y, w, type_scale::MICRO, rgb(tone, 1.0), text);
}

pub(super) fn draw(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    enter: f32,
) -> Option<MultiplayerAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    let signed = match &state.server.status {
        Status::Online { name, .. } => format!("Signed in as {name}"),
        _ => "Play with friends over the internet or on your network".to_owned(),
    };
    header(ui, "Multiplayer", &signed, enter);

    let (top, bottom) = (160.0, h - 172.0);
    let side = Rect::new(LEFT, top, SIDE_W, bottom - top);
    ui.panel(Rect::new(
        side.x - 22.0,
        top - 20.0,
        side.w + 44.0,
        side.h + 40.0,
    ));
    let games = Rect::new(
        LEFT + SIDE_W + 72.0,
        top,
        w - 2.0 * LEFT - SIDE_W - 72.0,
        bottom - top,
    );
    ui.panel(Rect::new(
        games.x - 22.0,
        top - 20.0,
        games.w + 44.0,
        games.h + 40.0,
    ));
    let mut y = commander(ui, state, side);
    y = server_panel(ui, state, Rect::new(side.x, y, side.w, side.bottom() - y));
    join_panel(ui, state, Rect::new(side.x, y, side.w, side.bottom() - y));
    game_list(ui, state, games);

    // Footer.
    let back = ui.button(
        id("mp-back", 0),
        Rect::new(LEFT, h - 64.0 - 52.0, 200.0, 52.0),
        "Back",
        ButtonKind::Secondary,
        true,
    );
    let host_rect = Rect::new(w - LEFT - 300.0, h - 64.0 - 58.0, 300.0, 58.0);
    let name_ok = mc_net::check_name(&state.name).is_ok();
    let host = ui.button(
        id("mp-host", 0),
        host_rect,
        "Host Game",
        ButtonKind::Primary,
        name_ok,
    );
    if !name_ok && state.notice.is_none() {
        ui.text_right(
            host_rect.x - 24.0,
            host_rect.mid_y(),
            type_scale::CAPTION,
            rgb(palette::ACCENT, 1.0),
            "Choose a callsign to host",
        );
    }
    if let Some((text, at)) = &state.notice {
        let k = (1.0 - (at.elapsed().as_secs_f32() - 6.0).max(0.0)).clamp(0.0, 1.0);
        ui.text_right(
            host_rect.x - 24.0,
            host_rect.mid_y(),
            type_scale::CAPTION,
            rgb(palette::WARN, k),
            text,
        );
    }
    let mut action = None;
    let typing = ui.mem.editing.is_some();
    if back || (ui.input.key(Key::Escape) && !typing && ui.interactive) {
        ui.audio.play(Sfx::Back);
        action = Some(MultiplayerAction::Back);
    } else if host {
        ui.audio.play(Sfx::Select);
        action = Some(MultiplayerAction::Host);
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    action
}

/// Callsign and device key. Returns the y under it. Until there is a callsign
/// the server will take, the field pulses and says what to do; Enter, or
/// leaving the field, connects with the new one.
fn commander(ui: &mut Ui, state: &mut MultiplayerState, r: Rect) -> f32 {
    ui.section(r.x, r.y + 6.0, r.w, "Your Callsign");
    let field = Rect::new(r.x, r.y + 30.0, r.w, 40.0);
    let name_id = id("mp-name", 0);
    let editing = ui.mem.editing == Some(name_id);
    ui.text_field(name_id, field, &mut state.name, mc_net::MAX_PLAYER_NAME);
    let valid = mc_net::check_name(&state.name);
    let taken = matches!(
        state.server.status,
        Status::Refused {
            reason: mc_net::DirRefuseReason::NameTaken,
            ..
        }
    ) && state.name == state.server.name();
    if state.name.is_empty() && !editing {
        ui.text(
            field.x + 12.0,
            field.mid_y(),
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "type the name other players will see",
        );
    }
    if (valid.is_err() || taken) && !editing {
        let pulse = 0.35 + 0.65 * (ui.time * 3.0).sin().abs();
        let tone = if state.name.is_empty() {
            palette::ACCENT
        } else {
            palette::WARN
        };
        ui.frame(field, rgb(tone, pulse));
    }
    let (text, tone) = match valid {
        Err(_) if state.name.is_empty() => (
            "Choose a callsign to play online".to_owned(),
            palette::ACCENT,
        ),
        Err(why) => (format!("That name will not do: {why}"), palette::WARN),
        Ok(()) if taken => (
            "Another player owns this callsign on the server: choose another".to_owned(),
            palette::WARN,
        ),
        Ok(()) => {
            let key = state
                .identity
                .as_ref()
                .map_or_else(|| "none".to_owned(), |i| i.fingerprint());
            (
                format!("Device key {key}  \u{b7}  keeps this name yours on a server"),
                palette::FAINT,
            )
        }
    };
    caption(ui, r.x, r.y + 84.0, r.w, &text, tone);
    // A new callsign connects at once: on Enter, or on leaving the field when
    // the callsign was what stopped the link.
    let entered = editing && ui.input.key(Key::Enter);
    let left = !editing && state.server.name_problem() && state.name != state.server.name();
    if (entered || left) && valid.is_ok() && !state.address.trim().is_empty() {
        if entered {
            ui.audio.play(Sfx::Select);
        }
        let (address, name) = (state.address.clone(), state.name.clone());
        state.server.reconnect(&address, &name);
        state.tab = Tab::Online;
    }
    r.y + 108.0
}

/// The server's address, and how the link to it stands. Returns the y under it.
fn server_panel(ui: &mut Ui, state: &mut MultiplayerState, r: Rect) -> f32 {
    ui.section(r.x, r.y + 6.0, r.w, "Server");
    let button_w = 118.0;
    let field = Rect::new(r.x, r.y + 30.0, r.w - button_w - 10.0, 40.0);
    let editing = ui.mem.editing == Some(id("mp-server", 0));
    ui.text_field(id("mp-server", 0), field, &mut state.address, 64);
    if state.address.is_empty() && !editing {
        ui.text(
            field.x + 12.0,
            field.mid_y(),
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "address, e.g. 203.0.113.7",
        );
    }
    let changed = state.address.trim() != state.server.address || state.name != state_name(state);
    let label = if changed || !state.server.online() {
        "Connect"
    } else {
        "Reconnect"
    };
    let go = ui.button(
        id("mp-connect", 0),
        Rect::new(field.right() + 10.0, field.y, button_w, field.h),
        label,
        ButtonKind::Secondary,
        !state.address.trim().is_empty(),
    );
    let enter = editing && ui.input.key(Key::Enter);
    if go || enter {
        ui.audio.play(Sfx::Select);
        let (address, name) = (state.address.clone(), state.name.clone());
        state.server.reconnect(&address, &name);
        state.tab = Tab::Online;
    }

    // The link: a mark that says how it stands at a glance, then the words.
    let y = r.y + 88.0;
    let mark = Vec2::new(r.x + 12.0, y + 9.0);
    let t = ui.time;
    let (tone, headline, detail) = match &state.server.status {
        Status::NoAddress => (
            palette::FAINT,
            "Not Connected".to_owned(),
            "Enter the address of a Meridian server to find games on it.".to_owned(),
        ),
        Status::Connecting { since } => (
            palette::TEXT,
            "Connecting".to_owned(),
            format!(
                "Reaching {}  \u{b7}  {:.0} s",
                state.server.target(),
                since.elapsed().as_secs_f32()
            ),
        ),
        Status::Online {
            online,
            rooms,
            motd,
            ..
        } => {
            let ms = state
                .server
                .latency()
                .map_or(String::new(), |d| format!("  \u{b7}  {} ms", d.as_millis()));
            let plural =
                |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
            let mut line = format!(
                "{}  \u{b7}  {}{ms}",
                plural(*online, "commander", "commanders"),
                plural(*rooms, "game", "games")
            );
            if !motd.is_empty() {
                line = format!("{line}\n{motd}");
            }
            (palette::TEXT, "Online".to_owned(), line)
        }
        Status::Offline { problem, .. } => {
            let (title, body, _) = problem.explain(state.server.target());
            (palette::WARN, title.to_owned(), body)
        }
        Status::NeedsName => (
            palette::ACCENT,
            "Choose a Callsign".to_owned(),
            "Type the name other players will see under Your Callsign, then press Enter to connect."
                .to_owned(),
        ),
        Status::Refused {
            reason: mc_net::DirRefuseReason::NameTaken,
            ..
        } => (
            palette::WARN,
            "Callsign Taken".to_owned(),
            format!(
                "Another player owns \u{201c}{}\u{201d} on this server. Choose another callsign above and press Enter.",
                state.server.name()
            ),
        ),
        Status::Refused { reason, detail } => (
            palette::BAD,
            "Turned Away".to_owned(),
            if detail.is_empty() {
                capitalise(reason.describe())
            } else {
                format!("{}: {detail}", capitalise(reason.describe()))
            },
        ),
    };
    // A ring: turning while connecting, whole when online, broken when not.
    match &state.server.status {
        Status::Connecting { .. } => {
            ui.arc(mark, 7.0, t * 5.0, t * 5.0 + 3.8, 2.0, rgb(tone, 1.0));
        }
        Status::Online { .. } => {
            ui.arc(mark, 7.0, 0.0, std::f32::consts::TAU, 2.0, rgb(tone, 1.0));
            ui.disc(
                mark,
                3.0,
                rgb(palette::ACCENT, 0.6 + 0.4 * (t * 2.0).sin().abs()),
            );
        }
        _ => {
            ui.arc(mark, 7.0, 0.6, 2.6, 2.0, rgb(tone, 1.0));
            ui.arc(mark, 7.0, 3.7, 5.7, 2.0, rgb(tone, 1.0));
        }
    }
    ui.text(
        r.x + 30.0,
        y + 9.0,
        type_scale::VALUE,
        rgb(tone, 1.0),
        &headline,
    );
    let mut ly = y + 32.0;
    for para in detail.lines() {
        for line in ui.wrap(type_scale::BODY, para, r.w - 30.0) {
            ui.text(
                r.x + 30.0,
                ly,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                &line,
            );
            ly += 20.0;
        }
    }
    if let Status::Offline {
        retry_at, attempts, ..
    } = state.server.status
    {
        let left = retry_at
            .saturating_duration_since(std::time::Instant::now())
            .as_secs_f32()
            .ceil();
        let r2 = Rect::new(r.x + 30.0, ly + 4.0, 130.0, 34.0);
        if ui.button(
            id("mp-retry", 0),
            r2,
            "Try Now",
            ButtonKind::Secondary,
            true,
        ) {
            ui.audio.play(Sfx::Select);
            state.server.retry();
        }
        ui.text(
            r2.right() + 14.0,
            r2.mid_y(),
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!(
                "Trying again in {left:.0} s  \u{b7}  attempt {}",
                attempts + 1
            ),
        );
        ly += 46.0;
    }
    ly + 12.0
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

/// The name the server link was made with (to tell when it has changed).
fn state_name(state: &MultiplayerState) -> String {
    match &state.server.status {
        Status::Online { name, .. } => name.clone(),
        _ => state.name.clone(),
    }
}

/// Join by code, and by address. Returns nothing: it is the last of the column.
fn join_panel(ui: &mut Ui, state: &mut MultiplayerState, r: Rect) {
    if r.h < 150.0 {
        return;
    }
    let y = r.bottom() - 212.0;
    ui.section(r.x, y, r.w, "Join by Code");
    let button_w = 118.0;
    let field = Rect::new(r.x, y + 24.0, r.w - button_w - 10.0, 40.0);
    let code_id = id("mp-code", 0);
    // The field gives up the keyboard on Enter, so ask first.
    let editing = ui.mem.editing == Some(code_id);
    ui.text_field(code_id, field, &mut state.code, 7);
    let parsed = state.code.parse::<mc_net::RoomCode>();
    let online = state.server.online();
    let go = ui.button(
        id("mp-code-join", 0),
        Rect::new(field.right() + 10.0, field.y, button_w, field.h),
        "Join",
        ButtonKind::Secondary,
        online && parsed.is_ok(),
    );
    let hint = if !online {
        "Needs the server"
    } else if state.code.is_empty() || parsed.is_ok() {
        "The host sees the code in their lobby"
    } else {
        "Codes are six letters and digits, like K7F-Q2M"
    };
    caption(ui, r.x, y + 78.0, r.w, hint, palette::FAINT);
    if let (true, Ok(code)) = (go || (editing && ui.input.key(Key::Enter)), parsed) {
        if online {
            ui.audio.play(Sfx::Select);
            if let Some(c) = state.server.client() {
                c.find_room(code);
            }
        }
    }

    let y = r.bottom() - 100.0;
    ui.section(r.x, y, r.w, "Direct Connect");
    let field = Rect::new(r.x, y + 24.0, r.w - button_w - 10.0, 40.0);
    let direct_id = id("mp-direct", 0);
    let editing = ui.mem.editing == Some(direct_id);
    ui.text_field(direct_id, field, &mut state.direct, 64);
    if state.direct.is_empty() && !editing {
        ui.text(
            field.x + 12.0,
            field.mid_y(),
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "a host's address, e.g. 192.168.1.20",
        );
    }
    let go = ui.button(
        id("mp-direct-join", 0),
        Rect::new(field.right() + 10.0, field.y, button_w, field.h),
        "Join",
        ButtonKind::Secondary,
        !state.direct.trim().is_empty(),
    );
    if (go || (editing && ui.input.key(Key::Enter))) && !state.direct.trim().is_empty() {
        ui.audio.play(Sfx::Select);
        let addr = state.direct.clone();
        state.join_direct(&addr, "Direct Game");
    }
}

/// The games: tabs for the server's and this network's, then the rows.
fn game_list(ui: &mut Ui, state: &mut MultiplayerState, r: Rect) {
    let online_n = state.server.rooms.len();
    let lan_n = state.lan.len();
    let mut x = r.x;
    for (tab, label, n) in [
        (Tab::Online, "Online Games", online_n),
        (Tab::Network, "This Network", lan_n),
    ] {
        let text = format!("{label}  {n}");
        let tw = ui.text_width(type_scale::BUTTON, &text) + 36.0;
        let chip = Rect::new(x, r.y, tw, 38.0);
        let on = state.tab == tab;
        let res = ui.tile(id("mp-tab", tab as usize), chip, on, true);
        ui.text_centred(
            chip.x + chip.w * 0.5,
            chip.mid_y(),
            type_scale::BUTTON,
            rgb(
                if on { 0xFFFFFF } else { palette::DIM },
                0.85 + 0.15 * res.glow,
            ),
            &text,
        );
        if on {
            ui.fill(
                Rect::new(chip.x, chip.bottom() - 2.0, chip.w, 2.0),
                rgb(palette::ACCENT, 1.0),
            );
        }
        if res.clicked && !on {
            ui.audio.play(Sfx::Tick);
            state.tab = tab;
        }
        x += tw + 8.0;
    }
    let list = Rect::new(r.x, r.y + 56.0, r.w, r.h - 56.0);
    // Column heads.
    let cols = columns(list);
    for (label, cx) in [
        ("Game", cols[0]),
        ("Map", cols[1]),
        ("Commanders", cols[2]),
        ("", cols[3]),
    ] {
        ui.text(
            cx,
            list.y + 8.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            label,
        );
    }
    ui.hline(list.x, list.y + 22.0, list.w, rgb(palette::LINE, 0.12));
    let rows = Rect::new(list.x, list.y + 30.0, list.w, list.h - 30.0);
    match state.tab {
        Tab::Online => online_rows(ui, state, rows),
        Tab::Network => lan_rows(ui, state, rows),
    }
}

/// Where the list's columns start.
fn columns(r: Rect) -> [f32; 4] {
    [
        r.x + 18.0,
        r.x + r.w * 0.40,
        r.x + r.w * 0.64,
        r.right() - 140.0,
    ]
}

fn online_rows(ui: &mut Ui, state: &mut MultiplayerState, r: Rect) {
    let problem = match &state.server.status {
        Status::Online { .. } => None,
        other => Some(other),
    };
    if let Some(status) = problem {
        let card = status_card(status, state.server.target(), state.server.name());
        return offline_card(ui, state, card, r);
    }
    if state.server.rooms.is_empty() {
        return empty(
            ui,
            r,
            "No Games Open",
            "Nobody is hosting on this server right now. Host one, and friends can join it from here or by its code.",
        );
    }
    let rooms = state.server.rooms.clone();
    for (i, room) in rooms.iter().enumerate() {
        let row = Rect::new(r.x, r.y + i as f32 * (ROW_H + 6.0), r.w, ROW_H);
        if row.bottom() > r.bottom() {
            break;
        }
        room_row(ui, state, room, row, i);
    }
}

/// What the offline list says, by what went wrong.
fn status_card(status: &Status, target: &str, name: &str) -> (String, String, Vec<String>) {
    match status {
        Status::NeedsName => (
            "Choose a Callsign".into(),
            "Games on a server are played under a callsign of your own. Type one under Your Callsign at the top left and press Enter: it connects straight away.".into(),
            vec![
                "Letters, digits, spaces, _ - and . up to 24 characters".into(),
                "A server keeps a callsign for the computer that first used it".into(),
            ],
        ),
        Status::Refused {
            reason: mc_net::DirRefuseReason::NameTaken,
            ..
        } => (
            "Callsign Taken".into(),
            format!("Another player already owns \u{201c}{name}\u{201d} on this server."),
            vec![
                "Choose another callsign at the top left and press Enter".into(),
                "A server keeps a callsign for the computer that first used it".into(),
            ],
        ),
        Status::NoAddress => (
            "Connect to a Server".into(),
            "Games over the internet go through a Meridian server. Whoever runs one gives you its address; enter it on the left.".into(),
            vec![
                "Anyone can run one: meridian-server, see docs/SERVER.md".into(),
                "Friends on your own network need no server: see This Network".into(),
            ],
        ),
        Status::Connecting { .. } => (
            "Connecting".into(),
            format!("Reaching {target}\u{2026}"),
            Vec::new(),
        ),
        Status::Offline { problem, .. } => {
            let (title, body, hints) = problem.explain(target);
            let mut hints: Vec<String> = hints.into_iter().map(String::from).collect();
            if matches!(problem, Problem::NoAnswer | Problem::NothingListening) {
                hints.push("Games on your own network still work: see This Network".into());
            }
            (title.to_owned(), body, hints)
        }
        Status::Refused { reason, detail } => (
            "Turned Away".into(),
            if detail.is_empty() {
                capitalise(reason.describe())
            } else {
                format!("{}: {detail}", capitalise(reason.describe()))
            },
            match reason {
                mc_net::DirRefuseReason::VersionMismatch => vec![
                    "The server and this game must be the same build".into(),
                ],
                _ => Vec::new(),
            },
        ),
        Status::Online { .. } => (String::new(), String::new(), Vec::new()),
    }
}

/// The list's place taken by what is wrong with the server, and what to do.
fn offline_card(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    (title, body, hints): (String, String, Vec<String>),
    r: Rect,
) {
    let w = (r.w - 80.0).min(620.0);
    let x = r.x + (r.w - w) * 0.5;
    let mut y = r.y + r.h * 0.22;
    let t = ui.time;
    let connecting = matches!(state.server.status, Status::Connecting { .. });
    let tone = if connecting {
        palette::TEXT
    } else {
        palette::WARN
    };
    // A broken ring, or a turning one while trying.
    let c = Vec2::new(r.x + r.w * 0.5, y);
    if connecting {
        ui.arc(c, 26.0, t * 4.0, t * 4.0 + 4.2, 2.4, rgb(tone, 1.0));
        ui.arc(
            c,
            34.0,
            -t * 2.0,
            -t * 2.0 + 1.4,
            1.2,
            rgb(palette::LINE, 0.4),
        );
    } else {
        for (a, b) in [(0.5, 2.2), (2.9, 4.6), (5.2, 5.9)] {
            ui.arc(c, 26.0, a, b, 2.4, rgb(tone, 0.9));
        }
        ui.disc(c, 3.0, rgb(tone, 0.6 + 0.4 * (t * 2.0).sin().abs()));
    }
    y += 62.0;
    ui.text_centred(c.x, y, type_scale::ITEM, rgb(0xFFFFFF, 1.0), &title);
    y += 32.0;
    for line in ui.wrap(type_scale::BODY, &body, w) {
        ui.text_centred(c.x, y, type_scale::BODY, rgb(palette::DIM, 1.0), &line);
        y += 22.0;
    }
    y += 12.0;
    for hint in &hints {
        let lines = ui.wrap(type_scale::BODY, hint, w - 30.0);
        for (k, line) in lines.iter().enumerate() {
            if k == 0 {
                ui.fill(
                    Rect::new(x + 2.0, y - 1.5, 6.0, 3.0),
                    rgb(palette::ACCENT, 0.9),
                );
            }
            ui.text(
                x + 20.0,
                y,
                type_scale::BODY,
                rgb(palette::TEXT, 0.85),
                line,
            );
            y += 22.0;
        }
        y += 4.0;
    }
    y += 16.0;
    let buttons = if matches!(state.server.status, Status::Offline { .. }) {
        2
    } else {
        1
    };
    let bw = 220.0;
    let mut bx = c.x - (bw * buttons as f32 + 12.0 * (buttons - 1) as f32) * 0.5;
    if buttons == 2 {
        if ui.button(
            id("mp-card-retry", 0),
            Rect::new(bx, y, bw, 44.0),
            "Try Again",
            ButtonKind::Primary,
            true,
        ) {
            ui.audio.play(Sfx::Select);
            state.server.retry();
        }
        bx += bw + 12.0;
    }
    if ui.button(
        id("mp-card-lan", 0),
        Rect::new(bx, y, bw, 44.0),
        "Games on This Network",
        ButtonKind::Secondary,
        true,
    ) {
        ui.audio.play(Sfx::Tick);
        state.tab = Tab::Network;
    }
}

fn empty(ui: &mut Ui, r: Rect, title: &str, body: &str) {
    let c = r.x + r.w * 0.5;
    let mut y = r.y + r.h * 0.3;
    ui.text_centred(c, y, type_scale::ITEM, rgb(palette::TEXT, 0.9), title);
    y += 30.0;
    for line in ui.wrap(type_scale::BODY, body, (r.w - 80.0).min(560.0)) {
        ui.text_centred(c, y, type_scale::BODY, rgb(palette::DIM, 1.0), &line);
        y += 22.0;
    }
}

/// Why a game cannot be joined from here, if it cannot.
fn blocker(state: &MultiplayerState, build: &str, map_id: Option<u64>) -> Option<&'static str> {
    if build != crate::BUILD && !build.is_empty() {
        return Some("Other Version");
    }
    match map_id {
        Some(id) if id != 0 && state.catalog.find(id).is_none() => Some("Missing Map"),
        _ => None,
    }
}

/// The part of a game row that takes the pointer: all of it but the button
/// at its right end. Clicking it joins, like the button.
fn row_face(row: Rect) -> Rect {
    Rect::new(row.x, row.y, (row.w - 140.0).max(0.0), row.h)
}

/// Seats as pips, the taken ones lit: a line of eight, or for a bigger room
/// two lines of smaller ones in the same width.
fn pips(ui: &mut Ui, x: f32, y: f32, taken: usize, seats: usize) {
    let seats = seats.min(mc_core::MAX_PLAYERS);
    let per_line = if seats <= 8 { 8 } else { seats.div_ceil(2) };
    let pitch = 112.0 / per_line as f32;
    let size = (pitch - 4.0).min(10.0);
    let lines = seats.div_ceil(per_line);
    for i in 0..seats {
        let (line, col) = (i / per_line, i % per_line);
        let r = Rect::new(
            x + col as f32 * pitch,
            y - 5.0 + (line as f32 - (lines as f32 - 1.0) * 0.5) * (size + 3.0),
            size,
            size,
        );
        if i < taken {
            ui.fill(r, rgb(palette::TEXT, 0.9));
        } else {
            ui.frame(r, rgb(palette::LINE, 0.35));
        }
    }
}

fn room_row(ui: &mut Ui, state: &mut MultiplayerState, room: &RoomListing, row: Rect, i: usize) {
    let blocked = blocker(state, &room.build, room.content.map(|c| c.map_id));
    let playing = matches!(room.phase, RoomPhase::Playing { .. } | RoomPhase::Loading);
    let full = room.free == 0 && !playing;
    // The row stops short of its button: the first control under the pointer
    // takes it, so a row covering the button would swallow the button's click.
    let res = ui.interact(id("mp-room", i), row_face(row), blocked.is_none());
    ui.fill(row, ink(0.35 + 0.15 * res.glow));
    ui.gradient_h(
        row,
        rgb(palette::ACCENT, 0.1 * res.glow),
        rgb(palette::ACCENT, 0.0),
    );
    ui.fill(
        Rect::new(row.x, row.y, 3.0, row.h),
        rgb(palette::ACCENT, 0.3 + 0.7 * res.glow),
    );
    let dim = if blocked.is_some() { 0.45 } else { 1.0 };
    let cols = columns(row);
    let title = if room.title.is_empty() {
        format!("{}'s Game", room.host)
    } else {
        room.title.clone()
    };
    ui.text_fit_left(
        cols[0],
        row.y + 24.0,
        cols[1] - cols[0] - 16.0,
        type_scale::ITEM,
        rgb(0xFFFFFF, dim),
        &title,
    );
    let by = if room.host.is_empty() {
        "Opening".to_owned()
    } else {
        format!("Hosted by {}", room.host)
    };
    ui.text(
        cols[0],
        row.y + 46.0,
        type_scale::MICRO,
        rgb(palette::DIM, dim),
        &by,
    );
    ui.text_fit_left(
        cols[1],
        row.y + 24.0,
        cols[2] - cols[1] - 16.0,
        type_scale::VALUE,
        rgb(palette::TEXT, dim),
        if room.map.is_empty() {
            "\u{2014}"
        } else {
            &room.map
        },
    );
    ui.text(
        cols[1],
        row.y + 46.0,
        type_scale::MICRO,
        rgb(palette::DIM, dim),
        &room.mode,
    );
    let taken = room.players.len();
    pips(ui, cols[2], row.y + 24.0, taken, room.seats as usize);
    let status = match room.phase {
        RoomPhase::Playing { tick } => {
            let s = tick / mc_core::TICKS_PER_SECOND;
            format!("In battle  \u{b7}  {}:{:02}", s / 60, s % 60)
        }
        RoomPhase::Loading => "Starting".into(),
        _ if full => "Full".into(),
        _ => format!("{} open", room.free),
    };
    ui.text(
        cols[2],
        row.y + 46.0,
        type_scale::MICRO,
        rgb(palette::DIM, dim),
        &status,
    );
    let button = Rect::new(row.right() - 128.0, row.mid_y() - 18.0, 116.0, 36.0);
    match blocked {
        Some(why) => ui.text_centred(
            button.x + button.w * 0.5,
            button.mid_y(),
            type_scale::CAPTION,
            rgb(palette::WARN, 0.9),
            why,
        ),
        None => {
            let (label, role) = if playing || full {
                ("Watch", Role::Observer)
            } else {
                ("Join", Role::Player)
            };
            let kind = if role == Role::Player {
                ButtonKind::Primary
            } else {
                ButtonKind::Secondary
            };
            let clicked = ui.button(id("mp-room-join", i), button, label, kind, true);
            if clicked || res.clicked {
                ui.audio.play(Sfx::Select);
                state.join_room(room, role);
            }
        }
    }
}

fn lan_rows(ui: &mut Ui, state: &mut MultiplayerState, r: Rect) {
    if state.scanner.is_none() {
        return empty(
            ui,
            r,
            "Cannot Listen on This Network",
            "Another program holds the port games announce themselves on (7778). Direct Connect on the left still works.",
        );
    }
    if state.lan.is_empty() {
        let dots = ".".repeat(1 + (ui.time * 2.0) as usize % 3);
        return empty(
            ui,
            r,
            &format!("Listening for Games{dots}"),
            "Games hosted on computers on your network show up here on their own. Host one with Host Game, and pick This Network.",
        );
    }
    let games = state.lan.clone();
    for (i, g) in games.iter().enumerate() {
        let row = Rect::new(r.x, r.y + i as f32 * (ROW_H + 6.0), r.w, ROW_H);
        if row.bottom() > r.bottom() {
            break;
        }
        lan_row(ui, state, g, row, i);
    }
}

fn lan_row(ui: &mut Ui, state: &mut MultiplayerState, g: &LanGame, row: Rect, i: usize) {
    let other = g.protocol != mc_net::PROTOCOL_VERSION;
    let blocked = if other {
        Some("Other Version")
    } else {
        blocker(state, &g.info.build, Some(g.info.content.map_id))
    };
    let res = ui.interact(id("mp-lan", i), row_face(row), blocked.is_none());
    ui.fill(row, ink(0.35 + 0.15 * res.glow));
    ui.fill(
        Rect::new(row.x, row.y, 3.0, row.h),
        rgb(palette::ACCENT, 0.3 + 0.7 * res.glow),
    );
    let dim = if blocked.is_some() { 0.45 } else { 1.0 };
    let cols = columns(row);
    ui.text_fit_left(
        cols[0],
        row.y + 24.0,
        cols[1] - cols[0] - 16.0,
        type_scale::ITEM,
        rgb(0xFFFFFF, dim),
        &g.info.title,
    );
    ui.text(
        cols[0],
        row.y + 46.0,
        type_scale::MICRO,
        rgb(palette::DIM, dim),
        &format!("Hosted by {}  \u{b7}  {}", g.info.host, g.addr),
    );
    ui.text_fit_left(
        cols[1],
        row.y + 24.0,
        cols[2] - cols[1] - 16.0,
        type_scale::VALUE,
        rgb(palette::TEXT, dim),
        &g.info.map,
    );
    ui.text(
        cols[1],
        row.y + 46.0,
        type_scale::MICRO,
        rgb(palette::DIM, dim),
        &g.info.mode,
    );
    pips(
        ui,
        cols[2],
        row.y + 24.0,
        g.info.players as usize,
        g.info.seats as usize,
    );
    ui.text(
        cols[2],
        row.y + 46.0,
        type_scale::MICRO,
        rgb(palette::DIM, dim),
        &format!("{} open", g.info.free),
    );
    let button = Rect::new(row.right() - 128.0, row.mid_y() - 18.0, 116.0, 36.0);
    match blocked {
        Some(why) => ui.text_centred(
            button.x + button.w * 0.5,
            button.mid_y(),
            type_scale::CAPTION,
            rgb(palette::WARN, 0.9),
            why,
        ),
        None => {
            let clicked = ui.button(
                id("mp-lan-join", i),
                button,
                "Join",
                ButtonKind::Primary,
                true,
            );
            if clicked || res.clicked {
                ui.audio.play(Sfx::Select);
                let (addr, title) = (g.addr.to_string(), g.info.title.clone());
                state.join_direct(&addr, &title);
            }
        }
    }
}

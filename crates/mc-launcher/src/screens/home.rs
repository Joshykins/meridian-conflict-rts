//! The left column: the brand, the channel picker, and the selected channel's
//! build with Play at its foot.

use mc_core::Channel;

use crate::app::{date, megabytes, meter_lines, Action, App};
use crate::install::CHANNELS;
use crate::paint::{palette, rgb, type_scale, Color, Rect};
use crate::ui::{id, ButtonKind, Ui};

pub(super) fn brand(ui: &mut Ui, r: Rect) {
    ui.p.panel(r);
    let turn = (ui.time * 0.45).sin();
    ui.busy = true;
    ui.p.emblem((r.x + 42.0, r.mid_y()), 20.0, turn, rgb(palette::TEXT, 1.0));
    let x = r.x + 84.0;
    ui.p.text(
        x,
        r.y + 33.0,
        type_scale::NAME_BOLD,
        rgb(0xFFFFFF, 1.0),
        "Meridian",
    );
    ui.p.text(
        x,
        r.y + 63.0,
        type_scale::NAME_LIGHT,
        rgb(palette::TEXT, 0.92),
        "Conflict",
    );
    ui.p.text_right(
        r.right() - 16.0,
        r.y + 22.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Launcher",
    );
    ui.p.text_right(
        r.right() - 16.0,
        r.y + 40.0,
        type_scale::VALUE,
        rgb(palette::TEXT, 0.9),
        env!("CARGO_PKG_VERSION"),
    );
}

fn title(c: Channel) -> &'static str {
    match c {
        Channel::Release => "Release",
        Channel::Playtest => "Playtest",
        Channel::Dev => "Dev",
    }
}

fn blurb(c: Channel) -> &'static str {
    match c {
        Channel::Release => "The game as everyone plays it",
        Channel::Playtest => "Builds ahead of release, on their own server",
        Channel::Dev => "",
    }
}

pub(super) fn channels(ui: &mut Ui, app: &mut App, r: Rect) {
    ui.p.panel(r);
    ui.p.section(r.x + 12.0, r.y + 20.0, r.w - 24.0, "Channel");
    let w = (r.w - 24.0) / CHANNELS.len() as f32;
    for (i, c) in CHANNELS.iter().enumerate() {
        let tab = Rect::new(r.x + 12.0 + i as f32 * w, r.y + 36.0, w - 4.0, 36.0);
        if ui.tab(id("channel", i), tab, title(*c), app.channel == *c) {
            app.pick_channel(*c);
        }
    }
}

/// What the status line says, and in what colour.
fn state(app: &App) -> (String, u32) {
    let lane = app.lane();
    let current = app.current();
    if let Some(job) = &lane.update {
        if job.running() {
            let m = job.meter();
            let which = lane
                .newest()
                .map(|n| format!(" Build {}", n.number))
                .unwrap_or_default();
            let line = match (m.progress, m.fraction()) {
                (Some(p), Some(k)) if p.done < p.total => {
                    format!("{}{which}   \u{b7}   {:.0}%", m.stage, 100.0 * k)
                }
                (Some(_), _) => format!("Unpacking{which}\u{2026}"),
                _ => format!("Getting{which} ready\u{2026}"),
            };
            return (line, palette::TEXT);
        }
    }
    if let Some(e) = lane.update_error() {
        return (format!("The update did not download: {e}"), palette::WARN);
    }
    if app.store.is_none() {
        return (
            "A dev launcher: no build store to update from".into(),
            palette::DIM,
        );
    }
    match (lane.newest(), lane.remote_error(), &current) {
        (Some(n), _, Some(c)) if n.number <= c.number => ("Up to date".into(), palette::GOOD),
        (Some(n), _, Some(_)) => (format!("Build {} is out", n.number), palette::ACCENT),
        (Some(n), _, None) => (
            format!(
                "Not installed   \u{b7}   {} to download",
                megabytes(app.install.versions.download_size(&n))
            ),
            palette::DIM,
        ),
        (None, Some(_), Some(_)) => (
            "Offline: the build store did not answer".into(),
            palette::WARN,
        ),
        (None, Some(e), None) => (
            format!("The build store did not answer: {e}"),
            palette::WARN,
        ),
        (None, None, _) => ("Checking for updates\u{2026}".into(), palette::DIM),
    }
}

fn dot(ui: &mut Ui, x: f32, y: f32, c: Color) {
    ui.p.disc((x, y), 4.0, c);
    ui.p.ring((x, y), 7.0, 1.0, [c[0], c[1], c[2], c[3] * 0.35]);
}

/// The selected channel's build: which, how it stands, its server, Play.
pub(super) fn status(ui: &mut Ui, app: &mut App, r: Rect) -> Option<Action> {
    ui.p.panel(r);
    let channel = app.channel;
    ui.p.section(
        r.x + 12.0,
        r.y + 20.0,
        r.w - 24.0,
        &format!("{} build", title(channel)),
    );
    let x = r.x + 24.0;
    let lane = app.lane();
    let current = app.current();
    let newest = lane.newest();
    let shown = current.clone().or_else(|| newest.clone());
    match &shown {
        Some(m) => {
            ui.p.text(
                x,
                r.y + 62.0,
                type_scale::HERO,
                rgb(palette::TEXT, 1.0),
                &format!("Build {}", m.number),
            );
            ui.p.text_right(
                r.right() - 24.0,
                r.y + 54.0,
                type_scale::CAPTION,
                rgb(palette::DIM, 1.0),
                &date(m.created),
            );
            ui.p.text_right(
                r.right() - 24.0,
                r.y + 74.0,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                &m.commit.chars().take(10).collect::<String>(),
            );
        }
        None => {
            ui.p.text(
                x,
                r.y + 62.0,
                type_scale::HERO,
                rgb(palette::TEXT, 0.5),
                "No build",
            );
        }
    }
    ui.p.text_fit(
        x,
        r.y + 100.0,
        r.w - 48.0,
        type_scale::BODY,
        rgb(palette::DIM, 1.0),
        blurb(channel),
    );

    // How it stands.
    let (line, tone) = state(app);
    let y = r.y + 146.0;
    dot(ui, x + 4.0, y, rgb(tone, 1.0));
    let lines = ui.p.wrap(type_scale::VALUE, &line, r.w - 72.0);
    for (i, l) in lines.iter().take(2).enumerate() {
        ui.p.text(
            x + 20.0,
            y + i as f32 * 20.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 0.95),
            l,
        );
    }
    let lane = app.lane();
    if let Some(job) = lane.update.as_ref().filter(|j| j.running()) {
        let meter = job.meter();
        ui.progress(Rect::new(x, y + 24.0, r.w - 48.0, 6.0), meter.fraction());
        ui.busy = true;
        let (_, tail) = meter_lines(&meter);
        ui.p.text(
            x,
            y + 48.0,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            &tail,
        );
        if ui.link(
            id("cancel", 0),
            r.right() - 24.0 - 44.0,
            y + 48.0,
            type_scale::CAPTION,
            "Cancel",
        ) {
            app.cancel_update();
        }
    }

    // The facts underneath.
    let mut fy = r.y + 236.0;
    let fact = |ui: &mut Ui, fy: f32, label: &str, value: &str, tone: Color| {
        ui.p.text(x, fy, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
        ui.p.text_fit(
            x + 96.0,
            fy,
            r.w - 48.0 - 96.0,
            type_scale::CAPTION,
            tone,
            value,
        );
    };
    ui.p.hline(x, fy - 22.0, r.w - 48.0, rgb(palette::LINE, 0.1));
    let lane = app.lane();
    let (server, server_tone) = match (lane.addr.as_deref(), &lane.server) {
        (None, _) => ("Local network only".to_owned(), rgb(palette::DIM, 1.0)),
        (Some(addr), Some(job)) => match job.peek(|r| r.cloned()) {
            Some(Ok(true)) => (format!("Online   \u{b7}   {addr}"), rgb(palette::GOOD, 1.0)),
            Some(_) => (
                format!("Not answering   \u{b7}   {addr}"),
                rgb(palette::WARN, 1.0),
            ),
            None => (
                format!("Checking   \u{b7}   {addr}"),
                rgb(palette::DIM, 1.0),
            ),
        },
        (Some(addr), None) => (addr.to_owned(), rgb(palette::DIM, 1.0)),
    };
    fact(ui, fy, "SERVER", &server, server_tone);
    fy += 26.0;
    let (all, total) = &app.installed;
    let here = if all.is_empty() {
        "Nothing installed yet".to_owned()
    } else {
        format!(
            "{} build{} installed   \u{b7}   {}",
            all.len(),
            if all.len() == 1 { "" } else { "s" },
            megabytes(*total)
        )
    };
    fact(ui, fy, "ON DISK", &here, rgb(palette::TEXT, 0.85));
    fy += 26.0;
    if let (Some(n), Some(c)) = (&newest, &current) {
        if n.number > c.number {
            let pending = format!(
                "Build {}   \u{b7}   {}",
                n.number,
                megabytes(app.install.versions.download_size(n))
            );
            fact(ui, fy, "UPDATE", &pending, rgb(palette::ACCENT, 1.0));
        }
    }

    play(ui, app, r, current, newest)
}

/// The big button: Play, or what stands before it.
fn play(
    ui: &mut Ui,
    app: &mut App,
    r: Rect,
    current: Option<mc_builds::Manifest>,
    newest: Option<mc_builds::Manifest>,
) -> Option<Action> {
    let button = Rect::new(r.x + 16.0, r.bottom() - 16.0 - 68.0, r.w - 32.0, 68.0);
    let lane = app.lane();
    let updating = lane.updating();
    let failed =
        lane.update_error().is_some() || (current.is_none() && lane.remote_error().is_some());
    let (label, enabled) = if updating {
        ("Updating".to_owned(), false)
    } else if current.is_some() {
        ("Play".to_owned(), true)
    } else if failed {
        ("Retry".to_owned(), true)
    } else if newest.is_some() {
        ("Install".to_owned(), true)
    } else {
        ("Play".to_owned(), false)
    };
    let pressed = ui.button_styled(
        id("play", 0),
        button,
        &label,
        ButtonKind::Primary,
        enabled,
        type_scale::PLAY,
    );
    // A failed update leaves Play on the installed build, and a way to try again.
    if lane.update_error().is_some() && current.is_some() {
        let at = button.y - 24.0;
        if ui.link(
            id("retry", 0),
            r.x + 24.0,
            at,
            type_scale::CAPTION,
            "Try the update again",
        ) {
            app.retry();
        }
    }
    if !pressed {
        return None;
    }
    match (current, newest) {
        (Some(m), _) => Some(Action::Play(m, Vec::new())),
        (None, _) if failed => {
            app.retry();
            None
        }
        (None, Some(n)) => {
            if let Some(store) = app.store.clone() {
                app.start_install(store, n);
            }
            None
        }
        (None, None) => None,
    }
}

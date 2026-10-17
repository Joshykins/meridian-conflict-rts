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

fn blurb(app: &App) -> &'static str {
    match app.channel {
        _ if app.lane().unreleased() => "Not out yet. Pick Playtest to play now",
        Channel::Release => "The game as everyone plays it",
        Channel::Playtest => "Try what is coming next, before everyone else",
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
        (Some(_), _, None) => ("Not installed yet".into(), palette::DIM),
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

/// The selected channel's build: which, how it stands, Play.
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
            let none = if lane.unreleased() {
                "Coming soon"
            } else {
                "No build"
            };
            ui.p.text(
                x,
                r.y + 62.0,
                type_scale::HERO,
                rgb(palette::TEXT, 0.5),
                none,
            );
        }
    }
    ui.p.text_fit(
        x,
        r.y + 100.0,
        r.w - 48.0,
        type_scale::BODY,
        rgb(palette::DIM, 1.0),
        blurb(app),
    );

    // How it stands; an unreleased channel's heading has said it all.
    if !app.lane().unreleased() {
        stands(ui, app, x, r);
    }

    // Where a first install goes, under a rule.
    if let (Some(n), None) = (&newest, &current) {
        if !app.lane().unreleased() {
            install_to(ui, app, x, r, n);
        }
    }

    // An update waiting, under a rule.
    if let (Some(n), Some(c)) = (&newest, &current) {
        if n.number > c.number {
            let fy = r.y + 236.0;
            ui.p.hline(x, fy - 22.0, r.w - 48.0, rgb(palette::LINE, 0.1));
            ui.p.text(x, fy, type_scale::MICRO, rgb(palette::DIM, 1.0), "UPDATE");
            let pending = format!(
                "Build {}   \u{b7}   {}",
                n.number,
                megabytes(app.install.versions.download_size(n))
            );
            ui.p.text_fit(
                x + 96.0,
                fy,
                r.w - 48.0 - 96.0,
                type_scale::CAPTION,
                rgb(palette::ACCENT, 1.0),
                &pending,
            );
        }
    }

    play(ui, app, r, current, newest)
}

/// Where the first build goes, with a way to change it before Install, and
/// what the download needs of that drive.
fn install_to(ui: &mut Ui, app: &mut App, x: f32, r: Rect, n: &mc_builds::Manifest) {
    let (y, w) = (r.y + 236.0, r.w - 48.0);
    ui.p.hline(x, y - 22.0, w, rgb(palette::LINE, 0.1));
    ui.p.text(
        x,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "INSTALL TO",
    );
    let right = r.right() - 24.0;
    if app.folder.asking.is_some() {
        ui.p.text_right(
            right,
            y,
            type_scale::CAPTION,
            rgb(palette::FAINT, 1.0),
            "Choosing\u{2026}",
        );
    } else if !app.builds_busy() {
        let label = "Change Folder";
        let lw = ui.p.text_width(type_scale::CAPTION, label);
        if ui.link(
            id("change-folder", 0),
            right - lw,
            y,
            type_scale::CAPTION,
            label,
        ) {
            app.choose_folder();
        }
    }
    let path = fit_path(ui, &app.install.builds, w);
    ui.p.text_fit(
        x,
        y + 26.0,
        w,
        type_scale::VALUE,
        rgb(palette::TEXT, 0.95),
        &path,
    );
    let room = app
        .folder
        .room(&app.install.versions, &app.install.builds, n, app.still);
    let on = crate::folder::drive(&app.install.builds)
        .map(|d| format!(" on {d}"))
        .unwrap_or_default();
    let (line, tone) = match room.free {
        Some(free) if room.short() => (
            format!(
                "Not enough room{on}: it needs {}, and {} is free",
                megabytes(room.needed),
                megabytes(free)
            ),
            palette::WARN,
        ),
        Some(free) => (
            format!(
                "{} to download   \u{b7}   {} free{on}",
                megabytes(room.download),
                megabytes(free)
            ),
            palette::DIM,
        ),
        None => (
            format!("{} to download", megabytes(room.download)),
            palette::DIM,
        ),
    };
    ui.p.text_fit(x, y + 50.0, w, type_scale::CAPTION, rgb(tone, 1.0), &line);
}

/// A path cut to `width` in its middle, so the drive and the folder it ends
/// in both show: `C:\Users\…\Programs\Meridian Conflict`.
pub(super) fn fit_path(ui: &mut Ui, path: &std::path::Path, width: f32) -> String {
    let full = path.display().to_string();
    if ui.p.text_width(type_scale::VALUE, &full) <= width {
        return full;
    }
    let sep = std::path::MAIN_SEPARATOR;
    let parts: Vec<&str> = full.split(sep).collect();
    // Drop folders after the first until it fits, keeping at least the last.
    for cut in 1..parts.len().saturating_sub(1) {
        let mut kept = vec![parts[0], "\u{2026}"];
        kept.extend(&parts[cut + 1..]);
        let trial = kept.join(&sep.to_string());
        if ui.p.text_width(type_scale::VALUE, &trial) <= width {
            return trial;
        }
    }
    full
}

/// The status line under the build, with the update's meter while it runs.
fn stands(ui: &mut Ui, app: &mut App, x: f32, r: Rect) {
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
    let short = match (&current, &newest) {
        (None, Some(n)) => app
            .folder
            .room(&app.install.versions, &app.install.builds, n, app.still)
            .short(),
        _ => false,
    };
    let lane = app.lane();
    let updating = lane.updating();
    let failed =
        lane.update_error().is_some() || (current.is_none() && lane.remote_error().is_some());
    let (label, enabled) = if updating {
        ("Updating".to_owned(), false)
    } else if app.folder.moving.is_some() {
        ("Moving Builds".to_owned(), false)
    } else if current.is_none() && lane.unreleased() {
        ("Coming soon".to_owned(), false)
    } else if current.is_some() {
        ("Play".to_owned(), true)
    } else if failed {
        ("Retry".to_owned(), true)
    } else if newest.is_some() {
        // Not while the folder is being picked, nor onto a drive too full.
        ("Install".to_owned(), !short && !app.folder.busy())
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

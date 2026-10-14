//! The launcher's one screen, laid out like the game's front end: the brand
//! and the build's state in a column of glass down the left, Play at its foot;
//! patch notes, news and the installed builds in a panel to the right; links
//! along the foot. 1280 x 760 points.

mod builds;
mod home;
mod pages;

use crate::app::{Action, App, Page, ReplayStep};
use crate::paint::{ink, palette, rgb, type_scale, Rect};
use crate::ui::{id, ButtonKind, Ui};

/// The window's size in points.
pub(crate) const SIZE: (f32, f32) = (1280.0, 760.0);
const MARGIN: f32 = 24.0;
const COLUMN_W: f32 = 404.0;
const FOOT_H: f32 = 44.0;
/// The open window onto the backdrop, top right, with the latest news on it.
const HERO_H: f32 = 232.0;

/// The latest news item (or, with none, the newest build) over the art.
fn hero(ui: &mut Ui, app: &mut App, r: Rect) {
    // Only a frame: the art is the point. A shade along its foot for the type.
    ui.p.gradient_h(Rect::new(r.x, r.y, r.w * 0.75, r.h), ink(0.55), ink(0.0));
    ui.p.bevel(r, 10.0, 0.7);
    let item = app.news.as_ref().and_then(|n| {
        n.peek(|r| {
            r.and_then(|r| r.as_ref().ok())
                .and_then(|n| n.items.first().cloned())
        })
    });
    let (over, title, body, page) = match item {
        Some(i) => (
            format!("LATEST NEWS   \u{b7}   {}", crate::app::news_date(&i.date)),
            i.title,
            i.body,
            Page::News,
        ),
        None => {
            let Some(m) = app.lane().newest().or_else(|| app.current()) else {
                return;
            };
            (
                format!("NEWEST BUILD   \u{b7}   {}", crate::app::date(m.created)),
                format!("Build {}", m.number),
                m.notes,
                Page::Notes,
            )
        }
    };
    let x = r.x + 32.0;
    ui.p.text(
        x,
        r.bottom() - 128.0,
        type_scale::MICRO,
        rgb(palette::ACCENT, 1.0),
        &over,
    );
    ui.p.text_fit(
        x,
        r.bottom() - 94.0,
        r.w - 64.0,
        type_scale::HERO,
        rgb(0xFFFFFF, 1.0),
        &title,
    );
    // The body's first words, as a teaser.
    let first = crate::notes::parse(&body)
        .into_iter()
        .find_map(|b| match b {
            crate::notes::Block::Paragraph(t) | crate::notes::Block::Bullet(t) => Some(t),
            crate::notes::Block::Heading(_) => None,
        })
        .unwrap_or_default();
    let lines = ui.p.wrap(type_scale::BODY, &first, (r.w - 64.0).min(440.0));
    for (i, l) in lines.iter().take(2).enumerate() {
        ui.p.text(
            x,
            r.bottom() - 56.0 + i as f32 * 21.0,
            type_scale::BODY,
            rgb(palette::TEXT, 0.85),
            l,
        );
    }
    let more = if page == Page::News {
        "All news"
    } else {
        "Patch notes"
    };
    let more_x = r.right() - 32.0 - ui.p.text_width(type_scale::CAPTION, more);
    if app.page != page
        && ui.link(
            id("hero-more", 0),
            more_x,
            r.bottom() - 24.0,
            type_scale::CAPTION,
            more,
        )
    {
        app.page = page;
        app.scroll = 0.0;
    }
}

pub(crate) fn frame(ui: &mut Ui, app: &mut App) -> Option<Action> {
    let (w, h) = ui.size();
    let mut action = None;
    home::brand(ui, Rect::new(MARGIN, MARGIN, COLUMN_W, 96.0));
    home::channels(ui, app, Rect::new(MARGIN, MARGIN + 108.0, COLUMN_W, 84.0));
    let status = Rect::new(
        MARGIN,
        MARGIN + 204.0,
        COLUMN_W,
        h - FOOT_H - MARGIN - (MARGIN + 204.0),
    );
    if let Some(a) = home::status(ui, app, status) {
        action = Some(a);
    }
    let (rx, rw) = (MARGIN * 2.0 + COLUMN_W, w - MARGIN * 3.0 - COLUMN_W);
    hero(ui, app, Rect::new(rx, MARGIN, rw, HERO_H));
    let top = MARGIN + HERO_H + 12.0;
    let right = Rect::new(rx, top, rw, h - FOOT_H - MARGIN - top);
    if let Some(a) = pages::panel(ui, app, right) {
        action = Some(a);
    }
    if let Some(a) = foot(ui, app, Rect::new(0.0, h - FOOT_H, w, FOOT_H)) {
        action = Some(a);
    }
    if app.replay.is_some() {
        if let Some(a) = replay_card(ui, app) {
            action = Some(a);
        }
    }
    action
}

/// The links from the news, and a word from the launcher.
fn foot(ui: &mut Ui, app: &mut App, r: Rect) -> Option<Action> {
    let mut action = None;
    let y = r.mid_y() - 4.0;
    let mut x = MARGIN + 4.0;
    let links = app
        .news
        .as_ref()
        .map(|n| n.peek(|r| r.and_then(|r| r.as_ref().ok()).map(|n| n.links.clone())))
        .unwrap_or_default()
        .unwrap_or_default();
    for (i, link) in links.iter().enumerate() {
        let (w, clicked) = ui.link_out(id("foot-link", i), x, y, type_scale::CAPTION, &link.label);
        if clicked {
            action = Some(Action::Open(link.url.clone()));
        }
        x += w + 28.0;
    }
    let word = app
        .notice
        .clone()
        .unwrap_or_else(|| format!("Launcher {}", env!("CARGO_PKG_VERSION")));
    let tone = if app.notice.is_some() {
        rgb(palette::WARN, 1.0)
    } else {
        rgb(palette::FAINT, 1.0)
    };
    let room = r.right() - MARGIN - x - 20.0;
    let w = ui.p.text_width(type_scale::MICRO, &word).min(room);
    ui.p.text_fit(
        r.right() - MARGIN - w,
        y,
        room,
        type_scale::MICRO,
        tone,
        &word,
    );
    action
}

/// Over everything while an old replay's build is fetched (the launcher was
/// started with `--replay`), or when it could not be.
fn replay_card(ui: &mut Ui, app: &mut App) -> Option<Action> {
    let (w, h) = ui.size();
    ui.p.fill(Rect::new(0.0, 0.0, w, h), ink(0.62));
    let r = Rect::new(w * 0.5 - 280.0, h * 0.5 - 120.0, 560.0, 220.0);
    ui.p.panel(r);
    ui.p.section(r.x + 16.0, r.y + 24.0, r.w - 32.0, "Old replay");
    let replay = app.replay.as_ref()?;
    let file = replay
        .file
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    ui.p.text_fit(
        r.x + 24.0,
        r.y + 64.0,
        r.w - 48.0,
        type_scale::ITEM,
        rgb(palette::TEXT, 1.0),
        &file,
    );
    let recorded = if replay.build.is_empty() {
        "Its build could not be read".to_owned()
    } else {
        format!("Recorded in {}", replay.build)
    };
    ui.p.text_fit(
        r.x + 24.0,
        r.y + 92.0,
        r.w - 48.0,
        type_scale::BODY,
        rgb(palette::DIM, 1.0),
        &recorded,
    );
    match &replay.step {
        ReplayStep::Fetching(job) => {
            let meter = job.meter();
            ui.progress(
                Rect::new(r.x + 24.0, r.y + 132.0, r.w - 48.0, 6.0),
                meter.fraction(),
            );
            let line = if meter.stage.is_empty() {
                "Getting that build ready\u{2026}".to_owned()
            } else {
                crate::app::meter_line(&meter)
            };
            ui.p.text(
                r.x + 24.0,
                r.y + 160.0,
                type_scale::CAPTION,
                rgb(palette::TEXT, 0.85),
                &line,
            );
            ui.busy = true;
            None
        }
        ReplayStep::Playing(_) => None,
        ReplayStep::Failed(e) => {
            let lines = ui.p.wrap(type_scale::CAPTION, e, r.w - 48.0 - 140.0);
            for (i, l) in lines.iter().take(3).enumerate() {
                ui.p.text(
                    r.x + 24.0,
                    r.y + 130.0 + i as f32 * 20.0,
                    type_scale::CAPTION,
                    rgb(palette::WARN, 1.0),
                    l,
                );
            }
            let close = Rect::new(
                r.right() - 24.0 - 124.0,
                r.bottom() - 24.0 - 44.0,
                124.0,
                44.0,
            );
            if ui.button(
                id("replay-close", 0),
                close,
                "Close",
                ButtonKind::Secondary,
                true,
            ) {
                app.replay = None;
            }
            None
        }
    }
}

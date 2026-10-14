//! The right-hand panel: patch notes, news and the installed builds, under
//! tabs, in a list that scrolls.

use crate::app::{date, Action, App, Page};
use crate::notes::{self, Block};
use crate::paint::{palette, rgb, type_scale, Rect};
use crate::ui::{id, Ui};

const PAGES: [(Page, &str); 3] = [
    (Page::Notes, "Patch Notes"),
    (Page::News, "News"),
    (Page::Builds, "Builds"),
];

/// Lines a wheel notch scrolls, in points.
const NOTCH: f32 = 48.0;

pub(super) fn panel(ui: &mut Ui, app: &mut App, r: Rect) -> Option<Action> {
    ui.p.panel(r);
    for (i, (page, label)) in PAGES.iter().enumerate() {
        let tab = Rect::new(r.x + 20.0 + i as f32 * 150.0, r.y + 12.0, 146.0, 38.0);
        if ui.tab(id("page", i), tab, label, app.page == *page) {
            app.page = *page;
            app.scroll = 0.0;
        }
    }
    let view = Rect::new(r.x + 28.0, r.y + 66.0, r.w - 56.0, r.h - 80.0);
    if view.contains(ui.cursor()) {
        app.scroll += ui.scroll() * NOTCH;
    }
    app.scroll = app.scroll.max(0.0);
    ui.p.clip_to(view);
    let top = view.y - app.scroll;
    let (height, action) = match app.page {
        Page::Notes => (patch_notes(ui, app, view, top), None),
        Page::News => (news(ui, app, view, top), None),
        Page::Builds => super::builds::page(ui, app, view, top),
    };
    ui.p.unclip();
    // Never past the end.
    let most = (height - view.h).max(0.0);
    app.scroll = app.scroll.min(most);
    if most > 0.0 {
        scrollbar(ui, view, app.scroll, most, height);
    }
    action
}

fn scrollbar(ui: &mut Ui, view: Rect, scroll: f32, most: f32, height: f32) {
    let track = Rect::new(view.right() + 12.0, view.y, 2.0, view.h);
    ui.p.fill(track, rgb(palette::LINE, 0.08));
    let len = (view.h * view.h / height).max(32.0);
    let at = track.y + (track.h - len) * (scroll / most);
    ui.p.fill(Rect::new(track.x, at, 2.0, len), rgb(palette::LINE, 0.45));
}

/// Draws `blocks` from `y` in `width`; returns the y under them.
pub(super) fn blocks(ui: &mut Ui, x: f32, mut y: f32, width: f32, blocks: &[Block]) -> f32 {
    for b in blocks {
        match b {
            Block::Heading(h) => {
                y += 12.0;
                ui.p.text(x, y + 10.0, type_scale::VALUE, rgb(palette::TEXT, 1.0), h);
                y += 30.0;
            }
            Block::Bullet(t) => {
                let lines = ui.p.wrap(type_scale::BODY, t, width - 22.0);
                ui.p.fill(
                    Rect::new(x + 4.0, y + 9.0, 5.0, 2.0),
                    rgb(palette::ACCENT, 1.0),
                );
                for l in &lines {
                    ui.p.text(
                        x + 22.0,
                        y + 10.0,
                        type_scale::BODY,
                        rgb(palette::TEXT, 0.86),
                        l,
                    );
                    y += 22.0;
                }
                y += 4.0;
            }
            Block::Paragraph(t) => {
                for l in ui.p.wrap(type_scale::BODY, t, width) {
                    ui.p.text(x, y + 10.0, type_scale::BODY, rgb(palette::TEXT, 0.8), &l);
                    y += 22.0;
                }
                y += 10.0;
            }
        }
    }
    y
}

/// A heading for one build or news item: a title, then a caption at the right.
fn head(ui: &mut Ui, view: Rect, y: f32, title: &str, caption: &str, tags: &[(&str, u32)]) {
    let end = ui.p.text(
        view.x,
        y + 12.0,
        type_scale::TITLE,
        rgb(palette::TEXT, 1.0),
        title,
    );
    let mut tx = end + 14.0;
    for (tag, tone) in tags {
        let w = ui.p.text_width(type_scale::MICRO, tag) + 14.0;
        let r = Rect::new(tx, y + 3.0, w, 18.0);
        ui.p.fill(r, rgb(*tone, 0.16));
        ui.p.frame(r, rgb(*tone, 0.7));
        ui.p.text(tx + 7.0, r.mid_y(), type_scale::MICRO, rgb(*tone, 1.0), tag);
        tx += w + 8.0;
    }
    ui.p.text_right(
        view.right(),
        y + 12.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        caption,
    );
}

/// Every build in the channel's history, newest first, with its notes; or,
/// when the store cannot be read, the installed builds' own.
fn patch_notes(ui: &mut Ui, app: &App, view: Rect, top: f32) -> f32 {
    let lane = app.lane();
    let history = lane.remote.as_ref().and_then(|j| {
        j.peek(|r| {
            r.and_then(|r| r.as_ref().ok())
                .map(|r| r.history.builds.clone())
        })
    });
    let releases: Vec<(String, u32, u64, String)> = match history {
        Some(h) if !h.is_empty() => h
            .into_iter()
            .map(|r| (r.build, r.number, r.created, r.notes))
            .collect(),
        _ => app
            .installed
            .0
            .iter()
            .filter(|(m, _)| m.channel() == Some(app.channel))
            .map(|(m, _)| (m.build.clone(), m.number, m.created, m.notes.clone()))
            .collect(),
    };
    if releases.is_empty() {
        let line = if lane.remote.as_ref().is_some_and(|j| j.running()) {
            "Reading the patch notes\u{2026}"
        } else {
            "No patch notes yet."
        };
        ui.p.text(
            view.x,
            top + 12.0,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            line,
        );
        return 40.0;
    }
    let current = app.current().map(|m| m.build);
    let mut y = top;
    for (i, (build, number, created, text)) in releases.iter().enumerate() {
        if i > 0 {
            y += 18.0;
            ui.p.hline(view.x, y, view.w, rgb(palette::LINE, 0.1));
            y += 22.0;
        }
        let mut tags = Vec::new();
        if current.as_deref() == Some(build.as_str()) {
            tags.push(("INSTALLED", palette::GOOD));
        } else if i == 0 {
            tags.push(("NEW", palette::ACCENT));
        }
        head(
            ui,
            view,
            y,
            &format!("Build {number}"),
            &date(*created),
            &tags,
        );
        y += 40.0;
        let parsed = notes::parse(text);
        if parsed.is_empty() {
            ui.p.text(
                view.x,
                y + 10.0,
                type_scale::BODY,
                rgb(palette::FAINT, 1.0),
                "No notes for this build.",
            );
            y += 26.0;
        } else {
            y = blocks(ui, view.x, y, view.w, &parsed);
        }
    }
    y - top + 12.0
}

fn news(ui: &mut Ui, app: &App, view: Rect, top: f32) -> f32 {
    let Some(job) = &app.news else {
        ui.p.text(
            view.x,
            top + 12.0,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            "No news in a dev launcher.",
        );
        return 40.0;
    };
    let items = job.peek(|r| r.map(|r| r.as_ref().map(|n| n.items.clone()).map_err(Clone::clone)));
    let items = match items {
        None => {
            ui.p.text(
                view.x,
                top + 12.0,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                "Reading the news\u{2026}",
            );
            return 40.0;
        }
        Some(Err(e)) => {
            ui.p.text_fit(
                view.x,
                top + 12.0,
                view.w,
                type_scale::BODY,
                rgb(palette::WARN, 1.0),
                &format!("The news did not load: {e}"),
            );
            return 40.0;
        }
        Some(Ok(items)) if items.is_empty() => {
            ui.p.text(
                view.x,
                top + 12.0,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                "No news yet.",
            );
            return 40.0;
        }
        Some(Ok(items)) => items,
    };
    let mut y = top;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            y += 18.0;
            ui.p.hline(view.x, y, view.w, rgb(palette::LINE, 0.1));
            y += 22.0;
        }
        head(
            ui,
            view,
            y,
            &item.title,
            &crate::app::news_date(&item.date),
            &[],
        );
        y += 40.0;
        y = blocks(ui, view.x, y, view.w, &notes::parse(&item.body));
    }
    y - top + 12.0
}

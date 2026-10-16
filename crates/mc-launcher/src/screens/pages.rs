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
            app.show(*page);
        }
    }
    let view = Rect::new(r.x + 28.0, r.y + 66.0, r.w - 56.0, r.h - 80.0);
    let reader = &mut app.reader;
    if view.contains(ui.cursor()) {
        reader.scroll += ui.scroll() * NOTCH;
    }
    // Held to the room the last frame measured before drawing, so the wheel
    // at the end never draws a frame past it that springs back on the next.
    reader.scroll = reader.scroll.clamp(0.0, reader.room);
    ui.p.clip_to(view);
    let top = view.y - reader.scroll;
    let (height, action) = match app.page {
        Page::Notes => (patch_notes(ui, app, view, top), None),
        Page::News => (news(ui, app, view, top), None),
        Page::Builds => super::builds::page(ui, app, view, top),
    };
    ui.p.unclip();
    // The page grew or shrank (notes opened or shut, the window resized).
    let reader = &mut app.reader;
    reader.room = (height - view.h).max(0.0);
    reader.scroll = reader.scroll.min(reader.room);
    if reader.room > 0.0 {
        scrollbar(ui, view, reader.scroll, reader.room, height);
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

/// A heading for one news item: a title, then a caption at the right.
fn head(ui: &mut Ui, view: Rect, y: f32, title: &str, caption: &str) {
    ui.p.text(
        view.x,
        y + 12.0,
        type_scale::TITLE,
        rgb(palette::TEXT, 1.0),
        title,
    );
    ui.p.text_right(
        view.right(),
        y + 12.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        caption,
    );
}

/// Small framed words in a row from `x`, centred on `y`; returns the x after them.
fn tags(ui: &mut Ui, mut x: f32, y: f32, tags: &[(&str, u32)]) -> f32 {
    for (tag, tone) in tags {
        let w = ui.p.text_width(type_scale::MICRO, tag) + 14.0;
        let r = Rect::new(x, y - 9.0, w, 18.0);
        ui.p.fill(r, rgb(*tone, 0.16));
        ui.p.frame(r, rgb(*tone, 0.7));
        ui.p.text(x + 7.0, r.mid_y(), type_scale::MICRO, rgb(*tone, 1.0), tag);
        x += w + 8.0;
    }
    x
}

/// One build's patch notes as the panel lists them.
struct Release {
    build: String,
    number: u32,
    created: u64,
    notes: String,
}

/// A build's row in the patch notes, taller than a line so it is easy to hit.
const ROW_H: f32 = 50.0;
/// How far a build's notes sit in from its row, under its title.
const INDENT: f32 = 30.0;

/// Every build in the channel's history, newest first; or, when the store
/// cannot be read, the installed builds' own. Each is a row that opens to its
/// notes: the newest starts open and marked, the rest shut, so the list stays
/// short however many builds there have been.
fn patch_notes(ui: &mut Ui, app: &mut App, view: Rect, top: f32) -> f32 {
    let lane = app.lane();
    let history = lane.remote.as_ref().and_then(|j| {
        j.peek(|r| {
            r.and_then(|r| r.as_ref().ok())
                .map(|r| r.history.builds.clone())
        })
    });
    let releases: Vec<Release> = match history {
        Some(h) if !h.is_empty() => h
            .into_iter()
            .map(|r| Release {
                build: r.build,
                number: r.number,
                created: r.created,
                notes: r.notes,
            })
            .collect(),
        _ => app
            .installed
            .0
            .iter()
            .filter(|(m, _)| m.channel() == Some(app.channel))
            .map(|(m, _)| Release {
                build: m.build.clone(),
                number: m.number,
                created: m.created,
                notes: m.notes.clone(),
            })
            .collect(),
    };
    if releases.is_empty() {
        let line = if lane.unreleased() {
            "Not out yet. Its patch notes will be here when it is."
        } else if lane.remote.as_ref().is_some_and(|j| j.running()) {
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
    for (i, release) in releases.iter().enumerate() {
        let installed = current.as_deref() == Some(release.build.as_str());
        y = note(ui, app, view, y, i, release, installed);
    }
    y - top + 12.0
}

/// Build `i`'s row, and its notes under it if open; returns the y under it.
fn note(
    ui: &mut Ui,
    app: &mut App,
    view: Rect,
    top: f32,
    i: usize,
    release: &Release,
    installed: bool,
) -> f32 {
    let newest = i == 0;
    let open = app.reader.notes_open(&release.build, newest);
    let row = Rect::new(view.x, top, view.w, ROW_H);
    let res = ui.interact(id("notes", i), row, true);
    if res.clicked {
        app.reader.toggle_notes(&release.build);
    }
    let open_k = ui.ease(id("notes-open", i), if open { 1.0 } else { 0.0 }, 18.0);
    if newest {
        ui.p.gradient_h(row, rgb(palette::ACCENT, 0.14), rgb(palette::ACCENT, 0.0));
    }
    ui.p.fill(row, rgb(palette::LINE, 0.05 * res.glow));
    let mid = row.mid_y();
    chevron(
        ui,
        view.x + 13.0,
        mid,
        open_k,
        rgb(palette::TEXT, 0.55 + 0.45 * res.glow.max(open_k)),
    );
    let end = ui.p.text(
        view.x + INDENT,
        mid,
        type_scale::TITLE,
        rgb(palette::TEXT, 1.0),
        &format!("Build {}", release.number),
    );
    let mut marks = Vec::new();
    if newest {
        marks.push(("LATEST", palette::ACCENT));
    }
    if installed {
        marks.push(("INSTALLED", palette::GOOD));
    }
    let after = tags(ui, end + 14.0, mid, &marks);
    let when = date(release.created);
    let when_x = view.right() - ui.p.text_width(type_scale::CAPTION, &when);
    ui.p.text(
        when_x,
        mid,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        &when,
    );
    let parsed = notes::parse(&release.notes);
    // Shut, the row says what the build is about: its notes' first line.
    if !open {
        let first = parsed.iter().find_map(|b| match b {
            Block::Paragraph(t) | Block::Bullet(t) => Some(t.as_str()),
            Block::Heading(_) => None,
        });
        let room = when_x - 24.0 - (after + 8.0);
        if let (Some(first), true) = (first, room > 60.0) {
            ui.p.text_fit(
                after + 8.0,
                mid,
                room,
                type_scale::BODY,
                rgb(palette::DIM, 0.9),
                first,
            );
        }
    }
    let mut y = row.bottom();
    if open {
        y += 4.0;
        if parsed.is_empty() {
            ui.p.text(
                view.x + INDENT,
                y + 10.0,
                type_scale::BODY,
                rgb(palette::FAINT, 1.0),
                "No notes for this build.",
            );
            y += 26.0;
        } else {
            y = blocks(ui, view.x + INDENT, y, view.w - INDENT, &parsed);
        }
        y += 10.0;
    }
    if newest {
        ui.p.fill(
            Rect::new(view.x, top, 3.0, y - top),
            rgb(palette::ACCENT, 1.0),
        );
    }
    ui.p.hline(view.x, y, view.w, rgb(palette::LINE, 0.1));
    y + 1.0
}

/// A chevron at `(x, y)` turning from pointing right (`k` 0, shut) to
/// pointing down (`k` 1, open).
fn chevron(ui: &mut Ui, x: f32, y: f32, k: f32, c: crate::paint::Color) {
    let (s, co) = (k * std::f32::consts::FRAC_PI_2).sin_cos();
    let turn = |(px, py): (f32, f32)| (x + px * co - py * s, y + px * s + py * co);
    ui.p.polyline(
        &[turn((-2.5, -5.0)), turn((2.5, 0.0)), turn((-2.5, 5.0))],
        1.6,
        c,
    );
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
        head(ui, view, y, &item.title, &crate::app::news_date(&item.date));
        y += 40.0;
        y = blocks(ui, view.x, y, view.w, &notes::parse(&item.body));
    }
    y - top + 12.0
}

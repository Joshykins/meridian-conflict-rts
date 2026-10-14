//! The Builds page: every installed build with what it takes on disk,
//! checking and repairing its files, removing the old ones, and the folders a
//! player is asked about in a bug report.

use crate::app::{date, megabytes, meter_lines, Action, App, Files};
use crate::install::{Install, CHANNELS};
use crate::paint::{palette, rgb, type_scale, Rect};
use crate::ui::{id, ButtonKind, Ui};

const ROW_H: f32 = 64.0;

/// Returns the page's height and what it asked for.
pub(super) fn page(ui: &mut Ui, app: &mut App, view: Rect, top: f32) -> (f32, Option<Action>) {
    app.step_files();
    let mut action = None;
    let mut y = top;
    let (total_line, n) = {
        let (all, total) = &app.installed;
        (
            format!(
                "{} build{} installed, taking {} together: a file two builds share is kept once.",
                all.len(),
                if all.len() == 1 { "" } else { "s" },
                megabytes(*total)
            ),
            all.len(),
        )
    };
    ui.p.text_fit(
        view.x,
        y + 10.0,
        view.w,
        type_scale::BODY,
        rgb(palette::DIM, 1.0),
        &total_line,
    );
    y += 34.0;
    // Column heads.
    let cols = [0.0, 220.0, 360.0, 480.0];
    for (c, label) in cols
        .iter()
        .zip(["BUILD", "CHANNEL", "RELEASED", "OWN FILES"])
    {
        ui.p.text(
            view.x + c,
            y + 6.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            label,
        );
    }
    y += 18.0;
    let starts: Vec<String> = CHANNELS
        .iter()
        .filter_map(|c| app.install.versions.current(*c))
        .map(|m| m.key())
        .collect();
    let rows = app.installed.0.clone();
    for (i, (m, own)) in rows.iter().enumerate() {
        let key = m.key();
        let r = Rect::new(view.x, y, view.w, ROW_H);
        ui.p.hline(r.x, r.y, r.w, rgb(palette::LINE, 0.1));
        let mid = r.y + 24.0;
        ui.p.text(
            r.x,
            mid,
            type_scale::ITEM,
            rgb(palette::TEXT, 1.0),
            &format!("Build {}", m.number),
        );
        let in_use = starts.contains(&key);
        if in_use {
            ui.p.text(
                r.x,
                mid + 22.0,
                type_scale::MICRO,
                rgb(palette::GOOD, 1.0),
                "STARTS ON PLAY",
            );
        } else {
            ui.p.text_fit(
                r.x,
                mid + 22.0,
                200.0,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                &m.build,
            );
        }
        let meter = match &app.files {
            Some(Files::Checking(k, job)) if *k == key => Some(job.meter()),
            Some(Files::Repairing(k, job)) if *k == key => Some(job.meter()),
            _ => None,
        };
        let word = match &app.files {
            Some(Files::Done(k, word)) if *k == key => Some(word.clone()),
            _ => None,
        };
        let busy = meter.is_some();
        let bw = 108.0;
        let check = Rect::new(r.right() - bw * 2.0 - 8.0, r.y + 10.0, bw, 40.0);
        if let Some(meter) = meter {
            // Its files being read, where its facts were.
            let (head, tail) = meter_lines(&meter);
            let span = check.x - 20.0 - (view.x + cols[1]);
            ui.p.text(
                view.x + cols[1],
                mid - 2.0,
                type_scale::CAPTION,
                rgb(palette::TEXT, 0.9),
                &head,
            );
            ui.p.text_right(
                check.x - 20.0,
                mid - 2.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &tail,
            );
            ui.progress(
                Rect::new(view.x + cols[1], mid + 14.0, span, 4.0),
                meter.fraction(),
            );
            ui.busy = true;
        } else {
            let channel = m.channel().map_or("?", |c| c.name());
            let mut caps = channel.chars();
            let channel = caps
                .next()
                .map(|f| f.to_uppercase().collect::<String>() + caps.as_str())
                .unwrap_or_default();
            ui.p.text(
                view.x + cols[1],
                mid,
                type_scale::CAPTION,
                rgb(palette::TEXT, 0.85),
                &channel,
            );
            ui.p.text(
                view.x + cols[2],
                mid,
                type_scale::CAPTION,
                rgb(palette::TEXT, 0.85),
                &date(m.created),
            );
            ui.p.text(
                view.x + cols[3],
                mid,
                type_scale::CAPTION,
                rgb(palette::TEXT, 0.85),
                &megabytes(*own),
            );
        }
        if let Some(word) = word {
            let (line, tone) = match word {
                Ok(w) => (w, palette::GOOD),
                Err(e) => (e, palette::WARN),
            };
            ui.p.text_fit(
                view.x + cols[1],
                mid + 22.0,
                check.x - 20.0 - view.x - cols[1],
                type_scale::MICRO,
                rgb(tone, 1.0),
                &line,
            );
        }
        let any_busy = matches!(
            app.files,
            Some(Files::Checking(..)) | Some(Files::Repairing(..))
        );
        if ui.button(
            id("check", i),
            check,
            "Check Files",
            ButtonKind::Secondary,
            !any_busy && !busy,
        ) {
            app.check_files(m);
        }
        let remove = Rect::new(r.right() - bw, r.y + 10.0, bw, 40.0);
        let confirming = app.removing.as_deref() == Some(key.as_str());
        let label = if confirming { "Sure?" } else { "Remove" };
        if ui.button(
            id("remove", i),
            remove,
            label,
            ButtonKind::Secondary,
            !in_use && !busy,
        ) {
            if confirming {
                app.remove(&key);
            } else {
                app.removing = Some(key.clone());
            }
        }
        y += ROW_H;
    }
    if n == 0 {
        ui.p.text(
            view.x,
            y + 16.0,
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "Nothing is installed yet.",
        );
        y += 40.0;
    }
    ui.p.hline(view.x, y, view.w, rgb(palette::LINE, 0.1));
    y += 28.0;

    ui.p.section(view.x, y, view.w, "Folders");
    y += 22.0;
    let install = Rect::new(view.x, y, 220.0, 44.0);
    if ui.button(
        id("open-install", 0),
        install,
        "Install Folder",
        ButtonKind::Secondary,
        true,
    ) {
        action = Some(Action::Open(
            app.install.root.to_string_lossy().into_owned(),
        ));
    }
    let files = Install::game_files();
    let logs = Rect::new(view.x + 232.0, y, 260.0, 44.0);
    if ui.button(
        id("open-logs", 0),
        logs,
        "Logs and Crash Reports",
        ButtonKind::Secondary,
        files.is_some(),
    ) {
        if let Some(dir) = files {
            let _ = std::fs::create_dir_all(&dir);
            action = Some(Action::Open(dir.to_string_lossy().into_owned()));
        }
    }
    let replays = Rect::new(view.x + 504.0, y, 160.0, 44.0);
    if ui.button(
        id("open-replays", 0),
        replays,
        "Replays",
        ButtonKind::Secondary,
        true,
    ) {
        let dir = app.install.root.join("replays");
        let _ = std::fs::create_dir_all(&dir);
        action = Some(Action::Open(dir.to_string_lossy().into_owned()));
    }
    y += 60.0;
    let note = "A bug report needs the newest crash-*.log and meridian.log from Logs and Crash Reports, and launcher.log from the install folder.";
    for l in ui.p.wrap(type_scale::CAPTION, note, view.w) {
        ui.p.text(view.x, y, type_scale::CAPTION, rgb(palette::DIM, 1.0), &l);
        y += 20.0;
    }
    (y - top + 12.0, action)
}

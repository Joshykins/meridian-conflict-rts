//! mc-studio: the music workstation for Meridian Conflict's songs.
//!
//! mc-studio [song.ron] [--song path] [--screenshot out.png] [--view arrange|piano|drums|instrument|mixer|director]
//!           [--size WxH] [--smoke] [--play] [--audition <log id>]
//!           [--reference <audio file>] [--edit] [--notice] [--music dir]
//!           [--part NAME] [--whole] [--moment NAME] [--audio file]
//!
//! `--screenshot` opens the window, waits for it to settle, writes a PNG and
//! exits; `--smoke` plays for two seconds, prints what the engine did and exits.

mod app;
mod arrange;
mod audio;
mod browser;
mod collab;
mod director;
mod drums;
mod edit;
mod effects;
mod export;
mod fft;
mod files;
mod history;
mod instrument;
mod keys;
mod midi;
mod mixer;
mod moments;
mod notes;
mod piano;
mod picker;
mod player;
mod reference;
mod sketch;
mod songops;
mod theme;
mod transport;
mod widgets;
mod workbench;

use std::path::PathBuf;

fn parse_args() -> app::Cli {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cli = app::Cli::default();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let mut value = || {
            i += 1;
            args.get(i).cloned()
        };
        match a.as_str() {
            "--song" => cli.song = value().map(PathBuf::from),
            "--screenshot" => cli.screenshot = value().map(PathBuf::from),
            "--view" => cli.view = value(),
            "--size" => {
                cli.size = value().and_then(|v| {
                    let (w, h) = v.split_once(['x', 'X'])?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                })
            }
            "--audition" => cli.audition = value(),
            "--reference" => cli.reference = value().map(PathBuf::from),
            "--smoke" => cli.smoke = true,
            "--edit" => cli.edit = true,
            "--music" => cli.music = value().map(PathBuf::from),
            "--part" => cli.part = value(),
            "--whole" => cli.whole = true,
            "--moment" => cli.moment = value(),
            "--audio" => cli.audio = value().map(PathBuf::from),
            "--notice" => cli.notice = true,
            "--play" => cli.play = true,
            other if !other.starts_with("--") => cli.song = Some(PathBuf::from(other)),
            other => eprintln!("mc-studio: unknown option {other}"),
        }
        i += 1;
    }
    cli
}

fn main() {
    let cli = parse_args();
    let (w, h) = cli.size.unwrap_or((1600.0, 960.0));
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("mc-studio")
            .with_inner_size([w, h])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "mc-studio",
        options,
        Box::new(move |cc| Ok(Box::new(app::Studio::new(&cc.egui_ctx, cli)))),
    );
    if let Err(e) = result {
        eprintln!("mc-studio: {e}");
        std::process::exit(1);
    }
}

//! A recorded match's battle report, read from Match History. Its record is the
//! one kept beside the replay when the match was played (`chronicle::file_for`);
//! an older replay without one is played through on a worker, as fast as it
//! goes, and the record it makes is kept for next time. The unit pictures and
//! the map's chart are drawn alongside, into the image slots the report reads.

use super::super::report::{Ctx, Place, Report, ReportAction};
use super::super::{id, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::hud::replay_clock as clock;
use crate::hud::thumbs::{Baked, Thumbs};
use crate::replay::Summary;
use crate::ui::preview;
use mc_data::Blueprints;
use mc_jobs::Pool;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

const LOST: &str = "the report could not be worked out";

pub enum SummaryAction {
    Close,
    Watch(PathBuf),
}

/// What the worker hands back.
struct Ready {
    report: Box<Report>,
    pictures: Baked,
    chart: Vec<u8>,
}

enum Stage {
    Working(Receiver<Result<Ready, String>>),
    Shown(Box<Report>),
    Failed(String),
}

pub struct SummaryState {
    path: PathBuf,
    map_name: String,
    /// The one human seat, whose verdict the report gives; `None` when there were
    /// none or several.
    local: Option<u8>,
    colors: crate::setup::Palette,
    blueprints: Arc<Blueprints>,
    thumbs: Thumbs,
    stage: Stage,
    /// The worker's result, waited for by a headless shot (`wait`) and taken in by
    /// the next frame; the report then opens already drawn in.
    arrived: Option<Result<Ready, String>>,
    /// The page a headless shot opens on.
    page: String,
    /// Ticks played through so far, of `length`, when there was no kept record.
    played: Arc<AtomicU32>,
    length: u32,
    stop: Arc<AtomicBool>,
    enter: f32,
    closing: bool,
}

/// Leaving before the worker is done stops it.
impl Drop for SummaryState {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl SummaryState {
    pub fn open(s: &Summary, blueprints: &Arc<Blueprints>, pool: &Arc<Pool>) -> SummaryState {
        let humans: Vec<usize> = (0..s.players.len()).filter(|&i| s.players[i].1).collect();
        let local = match humans[..] {
            [one] => Some(one as u8),
            _ => None,
        };
        let colors = crate::replay::colors(s.survival);
        let team = colors[local.unwrap_or(0) as usize % colors.len()];
        let played = Arc::new(AtomicU32::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let job = {
            let (path, map_path) = (s.path.clone(), s.map_path.clone());
            let (blueprints, pool) = (blueprints.clone(), pool.clone());
            let (played, stop) = (played.clone(), stop.clone());
            move || -> Result<Ready, String> {
                let map_path = map_path.ok_or("its map is not in maps/")?;
                let map = mc_map::MapFile::open(&map_path)
                    .map_err(|e| format!("{}: {e}", map_path.display()))?;
                std::thread::scope(|scope| {
                    let chart = scope
                        .spawn(|| preview::render(&map, &crate::setup::map_config(&map).look()));
                    let pictures = scope.spawn(|| Thumbs::render(&blueprints, team));
                    let chronicle = crate::replay::chronicle_of(
                        &path,
                        &map,
                        &blueprints,
                        &pool,
                        &played,
                        &stop,
                    )?;
                    let report = Box::new(Report::of_record(&chronicle, &blueprints, local));
                    Ok(Ready {
                        report,
                        pictures: pictures.join().map_err(|_| "the unit pictures failed")?,
                        chart: chart.join().map_err(|_| "the map's chart failed")?,
                    })
                })
            }
        };
        let spawned = std::thread::Builder::new()
            .name("mc-history-report".into())
            .spawn(move || {
                crate::app::set_this_thread_priority(-2);
                let _ = tx.send(job());
            });
        SummaryState {
            path: s.path.clone(),
            map_name: s.map.clone().unwrap_or_else(|| "Unknown map".into()),
            local,
            colors,
            blueprints: blueprints.clone(),
            thumbs: Thumbs::default(),
            arrived: None,
            page: String::new(),
            stage: match spawned {
                Ok(_) => Stage::Working(rx),
                Err(e) => Stage::Failed(format!("could not start: {e}")),
            },
            played,
            length: if s.report_kept { 0 } else { s.length },
            stop,
            enter: 0.0,
            closing: false,
        }
    }

    /// The report fades out; true once it is gone.
    pub fn gone(&self) -> bool {
        self.closing && self.enter <= 0.0
    }

    /// Blocks until the report is worked out, and has it open fully drawn in: a
    /// headless shot (`MERIDIAN_HISTORY_REPORT`).
    pub fn wait(&mut self, page: &str) {
        self.page = page.to_owned();
        if let Stage::Working(rx) = &self.stage {
            self.arrived = Some(rx.recv().unwrap_or_else(|_| Err(LOST.into())));
            self.enter = 1.0;
        }
    }

    /// Takes in the worker's result: the pictures and chart go into their slots.
    fn pump(&mut self, ui: &mut Ui) {
        let Stage::Working(rx) = &self.stage else {
            return;
        };
        let waited = self.arrived.is_some();
        let result = match self.arrived.take() {
            Some(r) => r,
            None => match rx.try_recv() {
                Ok(r) => r,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => Err(LOST.into()),
            },
        };
        self.stage = match result {
            Ok(mut ready) => {
                self.thumbs.install(ui.o, ready.pictures);
                ui.o.set_image(
                    crate::hud::MINIMAP_SLOT,
                    preview::SIZE,
                    preview::SIZE,
                    &ready.chart,
                );
                if waited {
                    ready.report.settle();
                    ready.report.open_page(&self.page);
                }
                Stage::Shown(ready.report)
            }
            Err(e) => Stage::Failed(e),
        };
    }

    pub fn draw(&mut self, ui: &mut Ui) -> Option<SummaryAction> {
        self.pump(ui);
        self.enter = (self.enter
            + if self.closing {
                -ui.dt / 0.18
            } else {
                ui.dt / 0.45
            })
        .clamp(0.0, 1.0);
        let shown = 1.0 - (1.0 - self.enter).powi(3);
        let interactive = ui.interactive;
        ui.interactive &= !self.closing;
        let mut out = None;
        match &mut self.stage {
            Stage::Shown(report) => {
                let ctx = Ctx {
                    blueprints: &self.blueprints,
                    colors: &self.colors,
                    local: self.local,
                    map_name: &self.map_name,
                    thumbs: &self.thumbs,
                    chart: crate::hud::MINIMAP_SLOT,
                    place: Place::History,
                };
                match report.draw(ui, &ctx, shown) {
                    Some(ReportAction::Watch) => {
                        out = Some(SummaryAction::Watch(self.path.clone()))
                    }
                    Some(_) => self.closing = true,
                    None => {}
                }
            }
            stage => {
                if waiting(ui, stage, &self.map_name, &self.played, self.length, shown) {
                    self.closing = true;
                }
            }
        }
        ui.interactive = interactive;
        if self.gone() {
            out = Some(SummaryAction::Close);
        }
        out
    }
}

/// The card shown while the report is being worked out, or why it could not be;
/// returns whether Back was asked for.
fn waiting(
    ui: &mut Ui,
    stage: &Stage,
    map_name: &str,
    played: &AtomicU32,
    length: u32,
    shown: f32,
) -> bool {
    let (w, h) = (ui.size.x, ui.size.y);
    ui.fade = shown;
    ui.frost(Rect::new(0.0, 0.0, w, h), 0.9);
    let card = Rect::new((w - 560.0) * 0.5, (h - 200.0) * 0.5, 560.0, 200.0);
    ui.panel(card);
    let (x, cw) = (card.x + 32.0, card.w - 64.0);
    ui.section(x, card.y + 36.0, cw, map_name);
    match stage {
        Stage::Failed(why) => {
            ui.text(
                x,
                card.y + 74.0,
                type_scale::BODY,
                rgb(palette::TEXT, 1.0),
                "The battle report could not be made",
            );
            ui.text_fit_left(
                x,
                card.y + 100.0,
                cw,
                type_scale::CAPTION,
                rgb(palette::WARN, 1.0),
                why,
            );
        }
        _ => {
            let done = played.load(Ordering::Relaxed);
            let line = if length == 0 {
                "Opening the battle report\u{2026}".to_owned()
            } else {
                format!(
                    "Reading the match: {} of {}",
                    clock(done.min(length)),
                    clock(length)
                )
            };
            ui.text(
                x,
                card.y + 74.0,
                type_scale::BODY,
                rgb(palette::TEXT, 1.0),
                &line,
            );
            let bar = Rect::new(x, card.y + 100.0, cw, 3.0);
            ui.fill(bar, rgb(palette::LINE, 0.12));
            let k = if length == 0 {
                // No length to measure against: a sweep across the bar.
                (ui.time * 0.8).fract()
            } else {
                done as f32 / length as f32
            };
            ui.fill(
                Rect::new(bar.x, bar.y, bar.w * k.clamp(0.0, 1.0), bar.h),
                rgb(palette::ACCENT, 1.0),
            );
            if length != 0 {
                ui.text(
                    x,
                    card.y + 124.0,
                    type_scale::CAPTION,
                    rgb(palette::DIM, 1.0),
                    "Once, for a replay recorded before reports were kept",
                );
            }
        }
    }
    let back = ui.button(
        id("history-report-back", 0),
        Rect::new(x, card.bottom() - 28.0 - 40.0, 160.0, 40.0),
        "Back",
        ButtonKind::Secondary,
        true,
    );
    let esc = ui.interactive && ui.input.key(Key::Escape);
    if back || esc {
        ui.audio.play(Sfx::Back);
    }
    ui.fade = 1.0;
    back || esc
}

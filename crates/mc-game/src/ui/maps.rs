//! The map browser: every map as a card with its chart beside it, filtered by
//! size, biome and how it is played, with a larger chart of the map under the
//! pointer. Skirmish and survival set-up open it from their theatre card.
//!
//! The thumbnails are drawn on a worker thread when a screen opens. The
//! overlay has one image slot the front end does not use: it holds sixteen
//! 128 px cells, handed to whichever maps are on screen. The large chart in the
//! detail pane is drawn at full size on demand into the calling screen's own
//! chart slot, which that screen redraws once the browser closes.

use super::{id, ink, palette, preview, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use glam::Vec2;
use mc_data::weather::{Biome, MapConfig, MapLook, MapStyle};
use mc_map::MapFile;
use mc_render::overlay::IMAGE_SLOT;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// Edge of the thumbnails kept in memory, pixels.
const THUMB: usize = IMAGE_SLOT / 2;
/// The overlay slot the cells live in (0-2 hold the skirmish, menu and survival charts).
const CELL_SLOT: usize = 3;
/// A cell's edge: a thumbnail halved.
const CELL: usize = THUMB / 2;
const CELLS_ROW: usize = IMAGE_SLOT / CELL;
const CELLS: usize = CELLS_ROW * CELLS_ROW;

/// What the browser knows about one map.
pub struct MapCard {
    pub stem: String,
    pub name: String,
    pub map: Arc<MapFile>,
    /// Longest side, km.
    pub km: f32,
    pub size_m: [f32; 2],
    pub starts: usize,
    pub ores: usize,
    pub biome: Biome,
    pub style: MapStyle,
    /// What its preview is drawn in: its climate, or its regions' and their walls.
    pub look: MapLook,
    /// Its own settings file: the regions set-up picks a weather for.
    pub config: Arc<MapConfig>,
}

impl MapCard {
    pub fn new(stem: String, map: Arc<MapFile>, config: &MapConfig) -> MapCard {
        let size_m = map.info().size_metres().to_f32();
        let starts = map.start_positions().len().min(mc_core::MAX_PLAYERS);
        MapCard {
            stem,
            name: map.name().to_owned(),
            km: size_m[0].max(size_m[1]) / 1000.0,
            size_m,
            starts,
            ores: map.ore_regions().len(),
            biome: config.biome(),
            style: config.style(starts),
            look: config.look(),
            config: Arc::new(config.clone()),
            map,
        }
    }

    pub fn size_class(&self) -> SizeClass {
        SizeClass::of(self.km)
    }

    /// "12 km · 4 Players · 9 Ore Fields".
    pub fn summary(&self) -> String {
        format!(
            "{:.0} km  \u{b7}  {} Players  \u{b7}  {} Ore Fields",
            self.km, self.starts, self.ores
        )
    }
}

/// Opens the map at `path` and reads its settings file; `None` (logged) when it will not open.
pub fn open_card(path: &Path, config: &MapConfig) -> Option<MapCard> {
    match MapFile::open(path) {
        Ok(map) => Some(MapCard::new(
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Arc::new(map),
            config,
        )),
        Err(e) => {
            log::warn!("{}: {e}; not listed", path.display());
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum SizeClass {
    Small,
    Medium,
    Large,
}

impl SizeClass {
    pub const ALL: [SizeClass; 3] = [SizeClass::Small, SizeClass::Medium, SizeClass::Large];

    pub fn of(km: f32) -> SizeClass {
        if km <= 12.5 {
            SizeClass::Small
        } else if km <= 20.5 {
            SizeClass::Medium
        } else {
            SizeClass::Large
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SizeClass::Small => "Small",
            SizeClass::Medium => "Medium",
            SizeClass::Large => "Large",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            SizeClass::Small => "up to 12 km",
            SizeClass::Medium => "13-20 km",
            SizeClass::Large => "over 20 km",
        }
    }
}

/// Which browser last wrote the thumbnail slots: another screen's browser may
/// have put its own there since, and then ours go back in.
static SLOT_OWNER: AtomicU64 = AtomicU64::new(0);

/// Something else drew over the cells' slot: the next browser drawn puts its cells back.
pub fn slots_lost() {
    SLOT_OWNER.store(0, Ordering::Relaxed);
}

static NEXT_BROWSER: AtomicU64 = AtomicU64::new(1);

/// The thumbnails: drawn on a worker, handed out to cells as they are shown.
struct Thumbs {
    me: u64,
    rx: Option<Receiver<(usize, Vec<u8>)>>,
    /// Drawn but not yet taken in (`Browser::wait_for_thumbs`).
    pending: Vec<(usize, Vec<u8>)>,
    /// Each map's chart at `THUMB` pixels, once drawn.
    pics: Vec<Option<Vec<u8>>>,
    /// The cell slot's pixels.
    sheet: Vec<u8>,
    /// The map in each cell and the frame it was last drawn in.
    cells: [Option<(usize, u64)>; CELLS],
    frame: u64,
}

impl Thumbs {
    fn start(maps: Vec<(Arc<MapFile>, MapLook)>) -> Thumbs {
        let n = maps.len();
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("map-thumbs".into())
            .spawn(move || {
                for (i, (map, look)) in maps.into_iter().enumerate() {
                    if tx
                        .send((i, preview::render_at(&map, &look, THUMB)))
                        .is_err()
                    {
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            log::warn!("no thread for the map thumbnails: {e}");
        }
        Thumbs {
            me: NEXT_BROWSER.fetch_add(1, Ordering::Relaxed),
            rx: Some(rx),
            pending: Vec::new(),
            pics: vec![None; n],
            sheet: vec![0; IMAGE_SLOT * IMAGE_SLOT * 4],
            cells: [None; CELLS],
            frame: 0,
        }
    }

    /// Takes in what the worker has drawn; puts the cells back if another
    /// browser has had the slot since.
    fn pump(&mut self, ui: &mut Ui) {
        self.frame += 1;
        let mut arrived = std::mem::take(&mut self.pending);
        if let Some(rx) = &self.rx {
            arrived.extend(rx.try_iter());
        }
        for (i, rgba) in arrived {
            if let Some(p) = self.pics.get_mut(i) {
                *p = Some(rgba);
            }
        }
        if SLOT_OWNER.swap(self.me, Ordering::Relaxed) != self.me {
            ui.o.set_image(CELL_SLOT, IMAGE_SLOT, IMAGE_SLOT, &self.sheet);
        }
    }

    /// Draws map `i`'s thumbnail into `r`; false when it has none yet (or
    /// every cell is taken this frame).
    fn draw(&mut self, ui: &mut Ui, i: usize, r: Rect, tint: f32) -> bool {
        let Some(Some(pic)) = self.pics.get(i) else {
            return false;
        };
        let cell = match self
            .cells
            .iter()
            .position(|c| c.is_some_and(|(m, _)| m == i))
        {
            Some(c) => c,
            None => {
                // The cell drawn longest ago, never one drawn this frame.
                let frame = self.frame;
                let Some(c) = (0..CELLS)
                    .filter(|&c| self.cells[c].is_none_or(|(_, f)| f < frame))
                    .min_by_key(|&c| self.cells[c].map_or(0, |(_, f)| f + 1))
                else {
                    return false;
                };
                // Halve the thumbnail into the cell, a 2x2 box each pixel.
                let (cx, cy) = ((c % CELLS_ROW) * CELL, (c / CELLS_ROW) * CELL);
                for y in 0..CELL {
                    for x in 0..CELL {
                        let to = ((cy + y) * IMAGE_SLOT + cx + x) * 4;
                        for k in 0..4 {
                            let at = |dx: usize, dy: usize| {
                                pic[((2 * y + dy) * THUMB + 2 * x + dx) * 4 + k] as u32
                            };
                            self.sheet[to + k] =
                                ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
                        }
                    }
                }
                ui.o.set_image(CELL_SLOT, IMAGE_SLOT, IMAGE_SLOT, &self.sheet);
                SLOT_OWNER.store(self.me, Ordering::Relaxed);
                c
            }
        };
        self.cells[cell] = Some((i, self.frame));
        let src = [
            ((cell % CELLS_ROW) * CELL) as f32,
            ((cell / CELLS_ROW) * CELL) as f32,
            CELL as f32,
            CELL as f32,
        ];
        ui.image(CELL_SLOT, src, r, [tint, tint, tint, 1.0]);
        true
    }
}

/// The detail pane's full-size chart, drawn on a thread on demand.
#[derive(Default)]
struct Detail {
    /// The map being drawn, and where its pixels will arrive.
    job: Option<(usize, Receiver<Vec<u8>>)>,
    /// The map whose chart is in the slot.
    shown: Option<usize>,
    /// Draw on the calling thread (headless shots and tests).
    now: bool,
}

pub enum BrowserAction {
    /// The index of the map picked, into the list the browser was drawn with.
    Pick(usize),
    Cancel,
}

/// The browser's state: open or not, the filters, what is under the pointer.
pub struct Browser {
    open: bool,
    /// Presence, 0..1: the browser fades and rises in.
    shown: f32,
    size: Option<SizeClass>,
    biome: Option<Biome>,
    style: Option<MapStyle>,
    /// The card picked but not yet confirmed.
    chosen: usize,
    /// First row of cards shown.
    top_row: usize,
    /// When a card was last clicked, for a double click.
    last_click: Option<(usize, f32)>,
    thumbs: Thumbs,
    detail: Detail,
    /// The Choose Map button's rect last frame (tests click it).
    pub select_at: Vec2,
    /// Card centres drawn last frame, by map index (tests click them).
    pub cards: Vec<(usize, Vec2)>,
}

impl Browser {
    /// Starts drawing the thumbnails of `maps` (in the order the browser will be given them).
    pub fn new(maps: &[MapCard]) -> Browser {
        Browser {
            open: false,
            shown: 0.0,
            size: None,
            biome: None,
            style: None,
            chosen: 0,
            top_row: 0,
            last_click: None,
            thumbs: Thumbs::start(
                maps.iter()
                    .map(|m| (m.map.clone(), m.look.clone()))
                    .collect(),
            ),
            detail: Detail::default(),
            select_at: Vec2::ZERO,
            cards: Vec::new(),
        }
    }

    /// Blocks until every thumbnail is drawn (headless shots and tests).
    pub fn wait_for_thumbs(&mut self) {
        if let Some(rx) = &self.thumbs.rx {
            self.thumbs.pending.extend(rx.iter());
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open, or still fading out after closing.
    pub fn is_shown(&self) -> bool {
        self.open || self.shown > 0.0
    }

    /// Opens on `selected`.
    pub fn open(&mut self, selected: usize) {
        self.open = true;
        self.chosen = selected;
        self.top_row = usize::MAX; // scrolled to the chosen card on the first frame
        self.last_click = None;
    }

    /// Opens on `selected`, fully arrived (tools and tests).
    pub fn open_now(&mut self, selected: usize) {
        self.open(selected);
        self.shown = 1.0;
        self.detail.now = true;
    }

    /// True once after the browser has closed if it drew into the screen's
    /// chart slot: the screen must put its own chart back.
    pub fn release_slot(&mut self) -> bool {
        if self.open || self.shown > 0.0 {
            return false;
        }
        self.detail.job = None;
        self.detail.shown.take().is_some()
    }

    fn close(&mut self) {
        self.open = false;
    }

    fn passes(&self, m: &MapCard) -> bool {
        self.size.is_none_or(|s| m.size_class() == s)
            && self.biome.is_none_or(|b| m.biome == b)
            && self.style.is_none_or(|s| m.style == s)
    }

    /// Keeps thumbnails flowing into the overlay; call every frame the screen draws.
    pub fn pump(&mut self, ui: &mut Ui) {
        self.thumbs.pump(ui);
    }

    /// Map `i`'s thumbnail into `r`, or a plain dark square while it is drawn.
    pub fn thumb(&mut self, ui: &mut Ui, i: usize, r: Rect, tint: f32) {
        ui.fill(r, ink(0.85));
        if !self.thumbs.draw(ui, i, r, tint) {
            // Still drawing: a slow sweep says so.
            let k = (ui.time * 0.8).fract();
            ui.fill(
                Rect::new(r.x, r.y + r.h * k, r.w, 1.0),
                rgb(palette::LINE, 0.12),
            );
        }
        ui.frame(r, rgb(palette::LINE, 0.18));
    }

    /// Draws the browser over the screen when it is open. The screen underneath
    /// must have been drawn with `ui.interactive` off while `is_open`.
    /// `chart_slot` is the screen's own chart slot, lent for the detail pane
    /// (see `release_slot`).
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        maps: &[MapCard],
        title: &str,
        chart_slot: usize,
    ) -> Option<BrowserAction> {
        self.shown = if self.open {
            (self.shown + ui.dt * 6.0).min(1.0)
        } else {
            (self.shown - ui.dt * 8.0).max(0.0)
        };
        if self.shown <= 0.0 {
            self.cards.clear();
            return None;
        }
        let k = 1.0 - (1.0 - self.shown) * (1.0 - self.shown);
        let (fade, shift) = (ui.fade, ui.shift);
        ui.fade = fade * k;
        ui.shift = shift + Vec2::new(0.0, 16.0 * (1.0 - k));
        let action = self.body(ui, maps, title, chart_slot);
        ui.fade = fade;
        ui.shift = shift;
        if action.is_some() {
            self.close();
        }
        action
    }

    fn body(
        &mut self,
        ui: &mut Ui,
        maps: &[MapCard],
        title: &str,
        chart_slot: usize,
    ) -> Option<BrowserAction> {
        let (w, h) = (ui.size.x, ui.size.y);
        let live = self.open && ui.interactive;
        // Whatever is underneath goes quiet: dim it and take every click.
        ui.fill(Rect::new(-ui.shift.x, -ui.shift.y, w, h), ink(0.6));
        let pw = (w - 160.0).min(1640.0);
        let panel = Rect::new((w - pw) * 0.5, 84.0, pw, h - 168.0);
        ui.panel(panel);
        let inner = panel.inset(36.0);

        // Header.
        let end = ui.text(
            inner.x,
            inner.y + 14.0,
            type_scale::TITLE,
            rgb(0xFFFFFF, 1.0),
            title,
        );
        let shown: Vec<usize> = (0..maps.len()).filter(|&i| self.passes(&maps[i])).collect();
        ui.text(
            end + 18.0,
            inner.y + 20.0,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            &format!("{} of {} Maps", shown.len(), maps.len()),
        );
        ui.fill(
            Rect::new(inner.x, inner.y + 46.0, 58.0, 2.0),
            rgb(palette::ACCENT, 1.0),
        );
        ui.gradient_h(
            Rect::new(inner.x + 66.0, inner.y + 46.0, inner.w - 66.0, 1.0),
            rgb(palette::LINE, 0.35),
            rgb(palette::LINE, 0.04),
        );

        // Filters: a row of chips per kind, each with how many maps it would show.
        let fy = inner.y + 72.0;
        let mut x = inner.x;
        let changed_before = (self.size, self.biome, self.style);
        {
            let counts: Vec<usize> = SizeClass::ALL
                .iter()
                .map(|s| maps.iter().filter(|m| m.size_class() == *s).count())
                .collect();
            let options: Vec<(String, bool, usize)> = SizeClass::ALL
                .iter()
                .zip(&counts)
                .map(|(s, n)| (s.label().to_owned(), self.size == Some(*s), *n))
                .collect();
            if let Some(i) = chips(ui, "maps-size", &mut x, fy, "Size", &options, live) {
                let s = SizeClass::ALL[i];
                self.size = if self.size == Some(s) { None } else { Some(s) };
            }
            // What the size classes mean, under the chips.
            if let Some(s) = self.size {
                ui.text(
                    inner.x + 44.0,
                    fy + 32.0,
                    type_scale::MICRO,
                    rgb(palette::FAINT, 1.0),
                    s.hint(),
                );
            }
        }
        x += 30.0;
        {
            let mut biomes: Vec<Biome> = maps.iter().map(|m| m.biome).collect();
            biomes.sort();
            biomes.dedup();
            let options: Vec<(String, bool, usize)> = biomes
                .iter()
                .map(|b| {
                    let n = maps.iter().filter(|m| m.biome == *b).count();
                    (b.label().to_owned(), self.biome == Some(*b), n)
                })
                .collect();
            if let Some(i) = chips(ui, "maps-biome", &mut x, fy, "Biome", &options, live) {
                let b = biomes[i];
                self.biome = if self.biome == Some(b) { None } else { Some(b) };
            }
        }
        x += 30.0;
        {
            const STYLES: [MapStyle; 3] = [MapStyle::Duel, MapStyle::Teams, MapStyle::FreeForAll];
            let options: Vec<(String, bool, usize)> = STYLES
                .iter()
                .map(|s| {
                    let n = maps.iter().filter(|m| m.style == *s).count();
                    (s.label().to_owned(), self.style == Some(*s), n)
                })
                .collect();
            if let Some(i) = chips(ui, "maps-style", &mut x, fy, "Mode", &options, live) {
                let s = STYLES[i];
                self.style = if self.style == Some(s) { None } else { Some(s) };
            }
        }
        let filtered = self.size.is_some() || self.biome.is_some() || self.style.is_some();
        let clear = Rect::new(inner.right() - 120.0, fy - 16.0, 120.0, 32.0);
        if ui.button(
            id("maps-clear", 0),
            clear,
            "Clear Filters",
            ButtonKind::Secondary,
            filtered && live,
        ) {
            self.size = None;
            self.biome = None;
            self.style = None;
            ui.audio.play(Sfx::Tick);
        }
        if changed_before != (self.size, self.biome, self.style) {
            self.top_row = 0;
            ui.audio.play(Sfx::Select);
        }
        let shown: Vec<usize> = (0..maps.len()).filter(|&i| self.passes(&maps[i])).collect();

        // Body: cards on the left, the map under the pointer large on the right.
        let body_top = fy + 52.0;
        let footer = 64.0;
        let detail_w = (inner.w * 0.34)
            .clamp(320.0, 520.0)
            .min(inner.h - (body_top - inner.y) - footer - 110.0);
        let grid = Rect::new(
            inner.x,
            body_top,
            inner.w - detail_w - 40.0,
            inner.bottom() - body_top - footer,
        );
        let detail = Rect::new(grid.right() + 40.0, body_top, detail_w, grid.h);

        const CARD_H: f32 = 118.0;
        const GAP: f32 = 12.0;
        let cols = (((grid.w + GAP) / (380.0 + GAP)).floor() as usize).max(1);
        let card_w = (grid.w - GAP * (cols - 1) as f32) / cols as f32;
        let rows_fit = (((grid.h + GAP) / (CARD_H + GAP)).floor() as usize).max(1);
        let rows = shown.len().div_ceil(cols);
        let most = rows.saturating_sub(rows_fit);
        if self.top_row == usize::MAX {
            let at = shown.iter().position(|&i| i == self.chosen).unwrap_or(0) / cols;
            self.top_row = at.saturating_sub(rows_fit.saturating_sub(1));
        }
        if live && grid.contains(ui.cursor - ui.shift) && ui.input.scroll != 0.0 {
            self.top_row = if ui.input.scroll > 0.0 {
                self.top_row.saturating_sub(1)
            } else {
                self.top_row + 1
            };
        }
        self.top_row = self.top_row.min(most);
        if most > 0 {
            let track = Rect::new(grid.right() + 14.0, grid.y, 3.0, grid.h);
            ui.fill(track, rgb(palette::LINE, 0.2));
            let n = rows as f32;
            ui.fill(
                Rect::new(
                    track.x,
                    track.y + track.h * self.top_row as f32 / n,
                    track.w,
                    track.h * rows_fit as f32 / n,
                ),
                rgb(palette::ACCENT, 0.8),
            );
        }

        let mut hovered = None;
        let mut confirm = false;
        self.cards.clear();
        for (k, &i) in shown
            .iter()
            .enumerate()
            .skip(self.top_row * cols)
            .take(rows_fit * cols)
        {
            let slot = k - self.top_row * cols;
            let (col, row) = (slot % cols, slot / cols);
            let r = Rect::new(
                grid.x + col as f32 * (card_w + GAP),
                grid.y + row as f32 * (CARD_H + GAP),
                card_w,
                CARD_H,
            );
            self.cards
                .push((i, Vec2::new(r.x + r.w * 0.5, r.mid_y()) + ui.shift));
            let res = ui.interact(id("maps-card", i), r, live);
            if res.hovered {
                hovered = Some(i);
            }
            if res.clicked {
                let now = ui.time;
                if self
                    .last_click
                    .is_some_and(|(j, t)| j == i && now - t < 0.4)
                {
                    confirm = true;
                } else {
                    ui.audio.play(Sfx::Select);
                }
                self.chosen = i;
                self.last_click = Some((i, now));
            }
            self.card(ui, &maps[i], i, r, res.glow);
        }
        if shown.is_empty() {
            ui.text_centred(
                grid.x + grid.w * 0.5,
                grid.y + 60.0,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                "No map matches these filters",
            );
        }

        // The detail pane: the hovered map, else the chosen one.
        let focus = hovered.or(Some(self.chosen).filter(|i| *i < maps.len()));
        if let Some(i) = focus {
            self.detail(ui, &maps[i], i, detail, chart_slot);
        }

        // Footer.
        let fy = inner.bottom() - 48.0;
        let cancel = ui.button(
            id("maps-cancel", 0),
            Rect::new(inner.x, fy, 180.0, 48.0),
            "Cancel",
            ButtonKind::Secondary,
            live,
        );
        let pick_ok = shown.contains(&self.chosen);
        let pick_r = Rect::new(inner.right() - 260.0, fy, 260.0, 48.0);
        self.select_at = Vec2::new(pick_r.x + pick_r.w * 0.5, pick_r.mid_y()) + ui.shift;
        let pick = ui.button(
            id("maps-pick", 0),
            pick_r,
            "Choose Map",
            ButtonKind::Primary,
            pick_ok && live,
        );
        if let Some(m) = maps.get(self.chosen) {
            let tone = if pick_ok {
                palette::DIM
            } else {
                palette::FAINT
            };
            ui.text_right(
                pick_r.x - 24.0,
                pick_r.mid_y(),
                type_scale::CAPTION,
                rgb(tone, 1.0),
                &m.name,
            );
        }
        ui.text(
            inner.x + 204.0,
            fy + 24.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Double-click a map to choose it  \u{b7}  Esc to go back",
        );

        if !live {
            return None;
        }
        let typing = ui.mem.editing.is_some();
        if cancel || (ui.input.key(Key::Escape) && !typing) {
            ui.audio.play(Sfx::Back);
            return Some(BrowserAction::Cancel);
        }
        if (pick || confirm || (ui.input.key(Key::Enter) && !typing)) && pick_ok {
            ui.audio.play(Sfx::Select);
            return Some(BrowserAction::Pick(self.chosen));
        }
        None
    }

    fn card(&mut self, ui: &mut Ui, m: &MapCard, i: usize, r: Rect, glow: f32) {
        let chosen = self.chosen == i;
        let lit = ui.ease(id("maps-card-lit", i), if chosen { 1.0 } else { 0.0 }, 12.0);
        let g = lit.max(glow * 0.6);
        ui.fill(r, ink(0.5));
        ui.gradient_h(
            r,
            rgb(palette::ACCENT, 0.18 * g),
            rgb(palette::ACCENT, 0.01),
        );
        ui.frame(
            r,
            rgb(
                if chosen {
                    palette::ACCENT
                } else {
                    palette::LINE
                },
                0.14 + 0.4 * g,
            ),
        );
        ui.fill(Rect::new(r.x, r.y, 4.0, r.h), rgb(palette::ACCENT, lit));
        let side = r.h - 16.0;
        let t = Rect::new(r.x + 12.0, r.y + 8.0, side, side);
        self.thumb(ui, i, t, 0.8 + 0.2 * g);
        let x = t.right() + 16.0 + 3.0 * g;
        let tw = r.right() - x - 12.0;
        ui.text_fit_left(
            x,
            r.y + 24.0,
            tw,
            type_scale::ITEM,
            rgb(
                if chosen {
                    palette::ACCENT
                } else {
                    palette::TEXT
                },
                0.85 + 0.15 * g,
            ),
            &m.name,
        );
        ui.text_fit_left(
            x + 1.0,
            r.y + 48.0,
            tw,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &m.summary(),
        );
        let mut tx = x;
        for tag in [m.style.label(), m.biome.label(), m.size_class().label()] {
            tx = tag_chip(ui, tx, r.bottom() - 26.0, tag) + 6.0;
        }
    }

    fn detail(&mut self, ui: &mut Ui, m: &MapCard, i: usize, area: Rect, slot: usize) {
        let side = area.w.min(area.h - 120.0).max(80.0);
        let frame = Rect::new(area.x, area.y, side, side);
        // The full-size chart: take one that has arrived, start one when none is on the way.
        let d = &mut self.detail;
        if let Some((j, rx)) = &d.job {
            match rx.try_recv() {
                Ok(rgba) => {
                    ui.o.set_image(slot, preview::SIZE, preview::SIZE, &rgba);
                    d.shown = Some(*j);
                    d.job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => d.job = None,
                Err(_) => {}
            }
        }
        if d.shown != Some(i) && d.job.is_none() {
            if d.now {
                ui.o.set_image(
                    slot,
                    preview::SIZE,
                    preview::SIZE,
                    &preview::render(&m.map, &m.look),
                );
                d.shown = Some(i);
            } else {
                let (tx, rx) = channel();
                let (map, look) = (m.map.clone(), m.look.clone());
                let spawned =
                    std::thread::Builder::new()
                        .name("map-chart".into())
                        .spawn(move || {
                            let _ = tx.send(preview::render(&map, &look));
                        });
                if spawned.is_ok() {
                    d.job = Some((i, rx));
                }
            }
        }
        if self.detail.shown == Some(i) {
            ui.fill(frame, ink(0.85));
            ui.image(
                slot,
                [0.0, 0.0, preview::SIZE as f32, preview::SIZE as f32],
                frame,
                [1.0, 1.0, 1.0, 1.0],
            );
            ui.frame(frame, rgb(palette::LINE, 0.18));
        } else {
            self.thumb(ui, i, frame, 1.0);
        }
        ui.brackets(frame.inset(-6.0), 14.0, rgb(palette::ACCENT, 0.7));
        // Landing zones on the chart.
        for (n, s) in m
            .map
            .start_positions()
            .iter()
            .enumerate()
            .take(mc_core::MAX_PLAYERS)
        {
            let p = Vec2::new(frame.x, frame.y) + preview::locate(&m.map, s.to_f32(), side);
            ui.disc(p, 9.0, ink(0.85));
            ui.arc(
                p,
                9.0,
                0.0,
                std::f32::consts::TAU,
                1.2,
                rgb(palette::TEXT, 0.85),
            );
            ui.text_centred(
                p.x + 0.5,
                p.y,
                type_scale::MICRO,
                rgb(palette::TEXT, 1.0),
                &(n + 1).to_string(),
            );
        }
        let y = frame.bottom() + 28.0;
        ui.text_fit_left(
            area.x,
            y,
            area.w,
            type_scale::ITEM,
            rgb(palette::TEXT, 1.0),
            &m.name,
        );
        let rows = [
            (
                "Size",
                format!(
                    "{:.1} \u{d7} {:.1} km  \u{b7}  {}",
                    m.size_m[0] / 1000.0,
                    m.size_m[1] / 1000.0,
                    m.size_class().label()
                ),
            ),
            (
                "Players",
                format!("{}  \u{b7}  {}", m.starts, m.style.label()),
            ),
            ("Biome", m.biome.label().to_owned()),
            ("Ore Fields", m.ores.to_string()),
        ];
        for (k, (label, value)) in rows.iter().enumerate() {
            let ry = y + 28.0 + k as f32 * 22.0;
            if ry > area.bottom() {
                break;
            }
            ui.text(area.x, ry, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
            ui.text(
                area.x + 96.0,
                ry,
                type_scale::VALUE,
                rgb(palette::TEXT, 0.9),
                value,
            );
        }
    }
}

/// A small outlined label; returns its right edge.
fn tag_chip(ui: &mut Ui, x: f32, y: f32, text: &str) -> f32 {
    let w = ui.text_width(type_scale::MICRO, text) + 14.0;
    let r = Rect::new(x, y, w, 18.0);
    ui.fill(r, rgb(palette::LINE, 0.05));
    ui.frame(r, rgb(palette::LINE, 0.18));
    ui.text(
        x + 7.0,
        r.mid_y(),
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        text,
    );
    r.right()
}

/// A labelled row of filter chips at `x`; moves `x` past them. Returns the chip clicked.
fn chips(
    ui: &mut Ui,
    key: &str,
    x: &mut f32,
    y: f32,
    label: &str,
    options: &[(String, bool, usize)],
    live: bool,
) -> Option<usize> {
    let end = ui.text(*x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
    *x = end.max(*x + 34.0) + 10.0;
    let mut clicked = None;
    for (i, (text, on, n)) in options.iter().enumerate() {
        let count = format!("{n}");
        let w = ui.text_width(type_scale::BUTTON, text)
            + ui.text_width(type_scale::MICRO, &count)
            + 34.0;
        let r = Rect::new(*x, y - 16.0, w, 32.0);
        let res = ui.tile(id(key, i), r, *on, live && (*n > 0 || *on));
        let a = if *n > 0 || *on { 1.0 } else { 0.4 };
        let end = ui.text(
            r.x + 12.0,
            r.mid_y(),
            type_scale::BUTTON,
            rgb(
                if *on { 0xFFFFFF } else { palette::DIM },
                (0.85 + 0.15 * res.glow) * a,
            ),
            text,
        );
        ui.text(
            end + 8.0,
            r.mid_y() + 1.0,
            type_scale::MICRO,
            rgb(if *on { palette::ACCENT } else { palette::FAINT }, a),
            &count,
        );
        if res.clicked {
            clicked = Some(i);
        }
        *x = r.right() + 6.0;
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_map_draws_a_thumbnail() {
        assert_eq!(CELLS, 16);
        for path in crate::setup::list_maps() {
            let map = MapFile::open(&path).unwrap();
            let look = MapConfig::for_map(&path).unwrap_or_default().look();
            let rgba = preview::render_at(&map, &look, THUMB);
            assert_eq!(rgba.len(), THUMB * THUMB * 4, "{}", path.display());
        }
    }

    #[test]
    fn size_classes_split_where_the_labels_say() {
        assert_eq!(SizeClass::of(8.0), SizeClass::Small);
        assert_eq!(SizeClass::of(12.0), SizeClass::Small);
        assert_eq!(SizeClass::of(16.0), SizeClass::Medium);
        assert_eq!(SizeClass::of(20.0), SizeClass::Medium);
        assert_eq!(SizeClass::of(40.0), SizeClass::Large);
    }
}

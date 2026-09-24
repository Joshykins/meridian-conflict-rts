//! The test range's panel, down the left edge. What is always wanted stays in
//! view: the subject, spawning it or copying the selection, and Reset. The rest
//! sits under tabs so the panel stays short: what to do to the units, staged
//! scenarios, each side's economy, the sky, and the range itself.
//! Like the rest of the HUD it only reports what was asked for.

use super::{has_flag, Hud, HudAction, Scene, EDGE, ENERGY, GAP, MASS};
use crate::audio::Sfx;
use crate::game::Mode;
use crate::range::{self as rng, Range, RangeAction, Scenario, Side, BLUE, RED};
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Id, Rect, Ui};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::flag;

pub const WIDTH: f32 = 340.0;
const PAD: f32 = 14.0;
const ROW: f32 = 28.0;
const TAB_H: f32 = 26.0;
/// Space above the tab strip: title, subject picker, its line, count and side, spawn.
const HEAD_H: f32 = 22.0 + 20.0 + 32.0 + 10.0 + 14.0 + ROW + 4.0 + ROW + 14.0;
const FOOT_H: f32 = 14.0 + 34.0 + PAD;

/// The panel's tabs, in strip order.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Unit,
    Stage,
    Economy,
    Sky,
    Range,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Unit, Tab::Stage, Tab::Economy, Tab::Sky, Tab::Range];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Unit => "Unit",
            Tab::Stage => "Stage",
            Tab::Economy => "Economy",
            Tab::Sky => "Sky",
            Tab::Range => "Range",
        }
    }

    /// How tall the tab's page is.
    fn body_h(self) -> f32 {
        match self {
            Tab::Unit => 16.0 + ROW + 4.0 + ROW + 8.0 + ROW,
            Tab::Stage => 2.0 * (ROW + 4.0) + 4.0,
            Tab::Economy => 26.0 + 2.0 * (22.0 + ROW + 10.0) + ROW + 6.0 + ROW,
            Tab::Sky => crate::ui::sky::ROWS as f32 * (ROW + 4.0) + ROW,
            Tab::Range => 16.0 + ROW + 4.0 + ROW + 4.0 + ROW,
        }
    }
}

/// How tall the panel is on `tab`.
pub fn height(tab: Tab) -> f32 {
    HEAD_H + TAB_H + 14.0 + tab.body_h() + FOOT_H
}

/// A HUD tile with a word on it. `tone` colours the word while it is lit.
#[allow(clippy::too_many_arguments)]
pub(super) fn word_tile(
    hud: &mut Hud,
    ui: &mut Ui,
    id: Id,
    r: Rect,
    label: &str,
    lit: bool,
    enabled: bool,
    tone: u32,
) -> bool {
    let t = hud.tile(ui, id, r, lit, enabled);
    let live = if enabled { 1.0 } else { 0.3 };
    ui.text_centred(
        r.x + r.w * 0.5 + 1.5,
        r.mid_y() - 1.0,
        type_scale::MICRO,
        rgb(
            if lit { tone } else { palette::TEXT },
            (0.74 + 0.26 * t.glow) * live,
        ),
        label,
    );
    if t.clicked {
        ui.audio.play(Sfx::Select);
    }
    t.clicked
}

/// `n` equal rectangles across `r`, `gap` apart.
fn split(r: Rect, n: usize, gap: f32) -> impl Iterator<Item = Rect> {
    let w = (r.w - gap * (n as f32 - 1.0)) / n as f32;
    (0..n).map(move |i| Rect::new(r.x + i as f32 * (w + gap), r.y, w, r.h))
}

pub fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene, range: &Range, top: f32) {
    let view = s.view;
    let subject = s.blueprints.unit(range.subject);
    let tab = hud.range_tab;

    // The glass eases to the height of the tab that is open; it opens at full height.
    let goal = height(tab);
    if hud.range_tall <= 0.0 {
        hud.range_tall = goal;
    }
    hud.range_tall += (goal - hud.range_tall) * (1.0 - (-18.0 * ui.dt).exp());
    let tall = hud.range_tall;
    let panel = Rect::new(EDGE, top + GAP, WIDTH, tall);
    hud.glass(ui, panel);
    let (x, w) = (panel.x + PAD, panel.w - 2.0 * PAD);
    let mut y = panel.y + 22.0;
    let mut asked: Vec<RangeAction> = Vec::new();

    // -- title, and the side this machine commands ------------------------------------
    ui.text(
        x,
        y,
        type_scale::OVERLINE,
        rgb(palette::ACCENT, 1.0),
        "Test Range",
    );
    let chips = Rect::new(x + w - 2.0 * 52.0 - 4.0, y - 12.0, 2.0 * 52.0 + 4.0, 24.0);
    ui.text_right(
        chips.x - 8.0,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Commanding",
    );
    for (i, ((label, player, tone), r)) in [("Blue", BLUE, palette::ACCENT), ("Red", RED, palette::BAD)]
        .into_iter()
        .zip(split(chips, 2, 4.0))
        .enumerate()
    {
        if word_tile(hud, ui, id("range-control", i), r, label, view.local == player, true, tone) {
            asked.push(RangeAction::Control(player));
        }
    }
    y += 20.0;

    // -- the subject ------------------------------------------------------------------
    choose(
        hud,
        ui,
        Rect::new(x, y, w, 32.0),
        &subject.name,
        super::unit_picker::Target::Subject,
        &mut asked,
    );
    y += 32.0 + 10.0;
    ui.text(
        x,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!("T{}  \u{b7}  {}", subject.tech, subject.role),
    );
    ui.text_right(
        x + w,
        y,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &subject.key,
    );
    y += 14.0;
    let count = ui.stepper(
        id("range-count", 0),
        Rect::new(x, y, 96.0, ROW),
        &format!("\u{d7} {}", range.count()),
        rgb(palette::TEXT, 1.0),
        subject.is_mobile(),
    );
    if count != 0 {
        asked.push(RangeAction::Count(count));
    }
    for (i, (side, r)) in Side::ALL
        .into_iter()
        .zip(split(Rect::new(x + 104.0, y, w - 104.0, ROW), 3, 4.0))
        .enumerate()
    {
        let tone = if side == Side::Blue {
            palette::ACCENT
        } else {
            palette::BAD
        };
        if word_tile(hud, ui, id("range-side", i), r, side.label(), range.side == side, true, tone) {
            asked.push(RangeAction::Side(side));
        }
    }
    y += ROW + 4.0;
    let mut row = split(Rect::new(x, y, w, ROW), 2, 4.0);
    if word_tile(
        hud,
        ui,
        id("range-subject-spawn", 0),
        row.next().unwrap(),
        "Spawn   [ G ]",
        view.mode == Mode::SpawnSubject,
        true,
        palette::ACCENT,
    ) {
        asked.push(RangeAction::ArmSubject);
    }
    if word_tile(
        hud,
        ui,
        id("range-spawn", 0),
        row.next().unwrap(),
        "Duplicate   [ Shift G ]",
        view.mode == Mode::Spawn,
        true,
        palette::ACCENT,
    ) {
        asked.push(RangeAction::ArmSpawn);
    }
    y += ROW + 14.0;

    // -- the tab strip ------------------------------------------------------------------
    for (i, (t, r)) in Tab::ALL
        .into_iter()
        .zip(split(Rect::new(x, y, w, TAB_H), Tab::ALL.len(), 3.0))
        .enumerate()
    {
        let lit = t == tab;
        if word_tile(hud, ui, id("range-tab", i), r, t.label(), lit, true, palette::ACCENT) && !lit {
            hud.range_tab = t;
            hud.range_page = 0.0;
        }
        if lit {
            ui.fill(Rect::new(r.x + 6.0, r.bottom() - 2.0, r.w - 12.0, 2.0), rgb(palette::ACCENT, 0.9));
        }
    }
    y += TAB_H + 14.0;

    // -- the open page, fading in ---------------------------------------------------------
    hud.range_page += (1.0 - hud.range_page) * (1.0 - (-12.0 * ui.dt).exp());
    let fade = ui.fade;
    ui.fade *= hud.range_page;
    let body = Rect::new(x, y, w, hud.range_tab.body_h());
    match hud.range_tab {
        Tab::Unit => unit_page(hud, ui, s, range, body, &mut asked),
        Tab::Stage => stage_page(hud, ui, body, &mut asked),
        Tab::Economy => economy_page(hud, ui, s, range, body, &mut asked),
        Tab::Sky => sky_page(hud, ui, range, body, &mut asked),
        Tab::Range => range_page(hud, ui, body, &mut asked),
    }
    ui.fade = fade;

    // -- always there: start over ---------------------------------------------------------
    let foot = panel.bottom() - PAD - 34.0;
    if ui.button(
        id("range-reset", 0),
        Rect::new(x, foot, w, 34.0),
        "Reset   [ F5 ]",
        ButtonKind::Primary,
        true,
    ) {
        asked.push(RangeAction::Reset);
    }

    hud.actions.extend(asked.into_iter().map(HudAction::Range));
}

/// Hurt, heal, remove, flag and scrub the build state of the units acted on.
fn unit_page(hud: &mut Hud, ui: &mut Ui, s: &Scene, range: &Range, r: Rect, asked: &mut Vec<RangeAction>) {
    let view = s.view;
    let acted = range.acted(&view.selection, &view.frame.units, s.blueprints, view.local);
    let targets: Vec<&UnitInstance> = acted
        .ids
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
        .collect();
    let any = !targets.is_empty();
    let (x, w) = (r.x, r.w);
    let mut y = r.y;

    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Acts on");
    ui.text_right(
        x + w,
        y,
        type_scale::MICRO,
        rgb(if any { palette::TEXT } else { palette::FAINT }, 1.0),
        &acted.label,
    );
    y += 16.0;
    let hurts: [(&str, i16, u32); 4] = [
        ("\u{2212}10%", 100, palette::WARN),
        ("\u{2212}25%", 250, palette::WARN),
        ("Heal", -1000, palette::ACCENT),
        ("Kill", 1000, palette::BAD),
    ];
    for (i, ((label, permille, tone), r)) in hurts
        .into_iter()
        .zip(split(Rect::new(x, y, w, ROW), 4, 4.0))
        .enumerate()
    {
        if word_tile(hud, ui, id("range-hurt", i), r, label, false, any, tone) {
            asked.push(RangeAction::Damage(permille));
        }
    }
    y += ROW + 4.0;
    let all_have = |f: u16| any && targets.iter().all(|u| has_flag(u, f));
    let mut row = split(Rect::new(x, y, w, ROW), 3, 4.0);
    if word_tile(hud, ui, id("range-remove", 0), row.next().unwrap(), "Remove", false, any, palette::TEXT) {
        asked.push(RangeAction::Remove);
    }
    for (i, (label, f)) in [("Hold Fire", flag::PASSIVE), ("Cannot Die", flag::INVULNERABLE)]
        .into_iter()
        .enumerate()
    {
        let on = all_have(f);
        if word_tile(hud, ui, id("range-flag", i), row.next().unwrap(), label, on, any, palette::ACCENT) {
            asked.push(RangeAction::Flag(f, !on));
        }
    }
    y += ROW + 8.0;

    // Build state: drag anywhere along the track. 100% finishes the unit.
    let built = if any {
        targets.iter().map(|u| u.build).sum::<f32>() / targets.len() as f32
    } else {
        1.0
    };
    let track = Rect::new(x + 52.0, y, w - 52.0 - 44.0, ROW);
    let res = ui.interact(id("range-build", 0), track, any);
    let live = if any { 1.0 } else { 0.3 };
    ui.text(x, track.mid_y(), type_scale::MICRO, rgb(palette::DIM, live), "Built");
    let line = Rect::new(track.x, track.mid_y() - 2.0, track.w, 4.0);
    ui.fill(line, rgb(palette::LINE, 0.16 * live));
    ui.fill(
        Rect::new(line.x, line.y, line.w * built, line.h),
        rgb(palette::ACCENT, 0.9 * live),
    );
    for i in 0..=10 {
        ui.vline(
            line.x + line.w * i as f32 / 10.0,
            line.bottom() + 4.0,
            if i % 5 == 0 { 5.0 } else { 3.0 },
            rgb(palette::LINE, 0.3 * live),
        );
    }
    let grow = 1.5 * res.glow + if res.held { 1.5 } else { 0.0 };
    ui.fill(
        Rect::new(
            line.x + line.w * built - 3.0 - grow * 0.5,
            track.mid_y() - 8.0 - grow * 0.5,
            6.0 + grow,
            16.0 + grow,
        ),
        rgb(0xFFFFFF, live),
    );
    ui.text_right(
        x + w,
        track.mid_y(),
        type_scale::VALUE,
        rgb(palette::TEXT, live),
        &format!("{:.0}%", built * 100.0),
    );
    if res.held {
        // 5% detents, and only a new detent is an order.
        let wanted = ((((ui.cursor.x - ui.shift.x - line.x) / line.w).clamp(0.0, 1.0) * 20.0)
            .round()
            * 50.0) as u16;
        let current = hud.range_built.unwrap_or((built * 1000.0).round() as u16);
        if wanted != current {
            hud.range_built = Some(wanted);
            ui.audio.play(Sfx::Tick);
            asked.push(RangeAction::Build(wanted));
        }
    } else {
        hud.range_built = None;
    }
}

/// Scenarios: around the subject first, then what the subject is told to do itself.
fn stage_page(hud: &mut Hud, ui: &mut Ui, r: Rect, asked: &mut Vec<RangeAction>) {
    let mut y = r.y;
    for (n, line) in [&Scenario::ALL[..4], &Scenario::ALL[4..]].into_iter().enumerate() {
        for (i, (scenario, cell)) in line
            .iter()
            .zip(split(Rect::new(r.x, y, r.w, ROW + 4.0), line.len(), 4.0))
            .enumerate()
        {
            if word_tile(hud, ui, id("range-scenario", n * 4 + i), cell, scenario.label(), false, true, palette::ACCENT) {
                asked.push(RangeAction::Scenario(*scenario));
            }
        }
        y += ROW + 4.0 + 4.0;
    }
}

/// One side's stores and income: fill or empty them, turn the income up or down,
/// stage a shortage in one click, and leave wrecks about to reclaim.
fn economy_page(hud: &mut Hud, ui: &mut Ui, s: &Scene, range: &Range, r: Rect, asked: &mut Vec<RangeAction>) {
    let (x, w) = (r.x, r.w);
    let mut y = r.y;
    let side = hud.range_econ.min(RED);
    let status = s.view.status.players.get(side as usize);
    let free = range.free_build;

    // Whose economy, and how well it is covering what it spends.
    for (i, ((label, player, tone), cell)) in [("Blue", BLUE, palette::ACCENT), ("Red", RED, palette::BAD)]
        .into_iter()
        .zip(split(Rect::new(x, y, 116.0, 24.0), 2, 4.0))
        .enumerate()
    {
        if word_tile(hud, ui, id("range-econ-side", i), cell, label, side == player, true, tone) {
            hud.range_econ = player;
        }
    }
    let (what, tone) = match status {
        _ if free => ("Free build: nothing is spent".to_owned(), palette::DIM),
        Some(p) if p.efficiency < 0.995 => (
            format!("Stalling  \u{b7}  {:.0}% efficiency", p.efficiency * 100.0),
            palette::BAD,
        ),
        Some(_) => ("Covering what it spends".to_owned(), palette::DIM),
        None => (String::new(), palette::DIM),
    };
    ui.text_right(x + w, y + 12.0, type_scale::MICRO, rgb(tone, 1.0), &what);
    y += 26.0;

    for (n, (name, tone)) in [("Materials", MASS), ("Energy", ENERGY)].into_iter().enumerate() {
        let (have, cap, income, demand, reclaim) = status.map_or((0.0, 0.0, 0.0, 0.0, 0.0), |p| {
            if n == 0 {
                (p.mass, p.mass_capacity, p.mass_income + p.reclaim_income, p.mass_demand, p.reclaim_income)
            } else {
                (p.energy, p.energy_capacity, p.energy_income, p.energy_demand, 0.0)
            }
        });
        let end = ui.text(x, y + 6.0, type_scale::CAPTION, rgb(tone, 1.0), name);
        // Reclaim is part of the income; say how much while there is any.
        if reclaim > 0.05 {
            ui.text(end + 8.0, y + 6.0, type_scale::MICRO, rgb(MASS, 0.9), &format!("reclaim +{reclaim:.1}"));
        }
        let net = income - if free { 0.0 } else { demand };
        ui.text_right(
            x + w,
            y + 6.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("{have:.0} / {cap:.0}    +{income:.1}  \u{2212}{demand:.1}  =  {net:+.1}/s"),
        );
        let bar = Rect::new(x, y + 16.0, w, 3.0);
        ui.fill(bar, rgb(palette::LINE, 0.12));
        let full = if cap > 0.0 { (have / cap).clamp(0.0, 1.0) } else { 0.0 };
        ui.fill(Rect::new(bar.x, bar.y, bar.w * full, bar.h), rgb(tone, 0.9));
        y += 22.0;

        // Stores: empty, half, full.
        let stock = Rect::new(x, y, 150.0, ROW);
        for (i, ((label, permille), cell)) in [("Empty", 0u16), ("Half", 500), ("Fill", 1000)]
            .into_iter()
            .zip(split(stock, 3, 4.0))
            .enumerate()
        {
            if word_tile(hud, ui, id("range-stock", n * 3 + i), cell, label, false, cap > 0.0, tone) {
                asked.push(RangeAction::Stock {
                    player: side,
                    mass: (n == 0).then_some(permille),
                    energy: (n == 1).then_some(permille),
                });
            }
        }
        // Income share: less than normal is a shortage, more a glut.
        let at = range.income[side as usize][n];
        let label = format!("Income  {}", rng::income_label(at));
        let colour = match at.cmp(&rng::INCOME_NORMAL) {
            std::cmp::Ordering::Less => palette::WARN,
            std::cmp::Ordering::Equal => palette::TEXT,
            std::cmp::Ordering::Greater => tone,
        };
        let step = ui.stepper(
            id("range-income", n),
            Rect::new(x + 158.0, y, w - 158.0, ROW),
            &label,
            rgb(colour, 1.0),
            true,
        );
        if step != 0 {
            asked.push(RangeAction::Income {
                player: side,
                resource: n,
                step,
            });
        }
        y += ROW + 10.0;
    }

    // One click to a common state.
    let low = |n: usize| !free && range.income[side as usize][n] < rng::INCOME_NORMAL;
    let normal = !free && range.income[side as usize] == [rng::INCOME_NORMAL; 2];
    let mut row = split(Rect::new(x, y, w, ROW), 4, 4.0);
    if word_tile(hud, ui, id("range-econ-preset", 0), row.next().unwrap(), "Free Build", free, true, palette::ACCENT) {
        asked.push(RangeAction::FreeBuild(!free));
    }
    // A shortage: the store run dry and a quarter of the income coming in.
    for (i, (label, n)) in [("Low Power", 1usize), ("Low Mass", 0)].into_iter().enumerate() {
        if word_tile(hud, ui, id("range-econ-preset", 1 + i), row.next().unwrap(), label, low(n), true, palette::WARN) {
            if free {
                asked.push(RangeAction::FreeBuild(false));
            }
            asked.push(RangeAction::SetIncome {
                player: side,
                resource: n,
                index: 2,
            });
            asked.push(RangeAction::Stock {
                player: side,
                mass: (n == 0).then_some(0),
                energy: (n == 1).then_some(0),
            });
        }
    }
    if word_tile(hud, ui, id("range-econ-preset", 3), row.next().unwrap(), "Normal", normal, true, palette::ACCENT) {
        if free {
            asked.push(RangeAction::FreeBuild(false));
        }
        for resource in 0..2 {
            asked.push(RangeAction::SetIncome {
                player: side,
                resource,
                index: rng::INCOME_NORMAL,
            });
        }
        asked.push(RangeAction::Stock {
            player: side,
            mass: Some(1000),
            energy: Some(1000),
        });
    }
    y += ROW + 6.0;

    // Something to reclaim, and stores on top of the side's own, in commanders' worth.
    if word_tile(hud, ui, id("range-wrecks", 0), Rect::new(x, y, 150.0, ROW), "Wreck Field", false, true, MASS) {
        asked.push(RangeAction::Wrecks);
    }
    let k = rng::STORAGE_STEPS[range.storage[side as usize]];
    let label = if k == 0 { "Extra Store  Off".to_owned() } else { format!("Extra Store  \u{d7}{k}") };
    let step = ui.stepper(
        id("range-storage", 0),
        Rect::new(x + 158.0, y, w - 158.0, ROW),
        &label,
        rgb(palette::TEXT, 1.0),
        true,
    );
    if step != 0 {
        asked.push(RangeAction::Storage { player: side, step });
    }
}

/// The weather, set up here and applied at once.
fn sky_page(hud: &mut Hud, ui: &mut Ui, range: &Range, r: Rect, asked: &mut Vec<RangeAction>) {
    let (x, w) = (r.x, r.w);
    // Applied at once with Apply: the sky starts over each time, so it must not
    // jump at every click on the way to what is wanted.
    let applied = range.sky.unwrap_or_default();
    let mut sky = hud.range_sky.unwrap_or(applied);
    let look = crate::ui::sky::Look { row_h: ROW, pitch: ROW + 4.0, value_w: w - 96.0, compact: true };
    crate::ui::sky::rows(ui, 1, x, r.y, w, look, &mut sky.choice);
    let y = r.y + crate::ui::sky::ROWS as f32 * (ROW + 4.0);
    let mut row = split(Rect::new(x, y, w, ROW), 2, 4.0);
    if word_tile(hud, ui, id("range-storm", 0), row.next().unwrap(), "Storm Overhead", sky.storm_overhead, true, palette::ACCENT) {
        sky.storm_overhead = !sky.storm_overhead;
    }
    let pending = sky != applied;
    if ui.button(id("range-sky-apply", 0), row.next().unwrap(), "Apply", ButtonKind::Primary, pending) {
        asked.push(RangeAction::Sky(sky));
    }
    // Keep the draft only while it differs; once applied it is the range's own.
    hud.range_sky = pending.then_some(sky);
}

/// The camera presets and reloading the data.
fn range_page(hud: &mut Hud, ui: &mut Ui, r: Rect, asked: &mut Vec<RangeAction>) {
    let (x, w) = (r.x, r.w);
    let mut y = r.y;
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Camera");
    y += 16.0;
    for (i, (label, cell)) in ["Close  F2", "Mid  F3", "Far  F4"]
        .into_iter()
        .zip(split(Rect::new(x, y, w, ROW), 3, 4.0))
        .enumerate()
    {
        if word_tile(hud, ui, id("range-zoom", i), cell, label, false, true, palette::ACCENT) {
            asked.push(RangeAction::Zoom(i));
        }
    }
    y += ROW + 4.0 + 4.0;
    if word_tile(
        hud,
        ui,
        id("range-reload", 0),
        Rect::new(x, y, w, ROW),
        "Reload data/ and restart   [ F9 ]",
        false,
        true,
        palette::ACCENT,
    ) {
        asked.push(RangeAction::Reload);
    }
}

/// Keep the arrows for adjacent units; the large name opens the whole catalog.
fn choose(
    hud: &mut Hud,
    ui: &mut Ui,
    r: Rect,
    name: &str,
    target: super::unit_picker::Target,
    asked: &mut Vec<RangeAction>,
) {
    let salt = target as usize;
    let label = super::unit_picker::fitted(
        ui,
        &format!("{}  ...", name),
        r.w - 76.0,
        type_scale::BUTTON,
    );
    if ui.button(
        id("range-browse", salt),
        Rect::new(r.x + 28.0, r.y, r.w - 56.0, r.h),
        &label,
        ButtonKind::Secondary,
        true,
    ) {
        hud.unit_picker = Some(super::unit_picker::Picker::new(target));
        ui.mem.editing = Some(id("unit-search", 0));
    }
    for (i, step, x, label) in [(0, -1, r.x, "<"), (1, 1, r.right() - 26.0, ">")] {
        if ui.button(
            id("range-step", salt * 2 + i),
            Rect::new(x, r.y, 26.0, r.h),
            label,
            ButtonKind::Secondary,
            true,
        ) {
            asked.push(match target {
                super::unit_picker::Target::Subject => RangeAction::Subject(step),
                super::unit_picker::Target::Spawn => RangeAction::Spawn(step),
            });
        }
    }
}

//! The refit tab: what can be fitted to a unit with refit slots (`mc_data::refit`),
//! a row per slot. In a row, a module's later tiers follow it behind an arrow;
//! alternatives are split by "or". Each tile says whether its module is fitted,
//! being fitted, waiting in the queue, or what it costs, and a click queues it:
//! with the earlier tiers it needs in front of it, and, when it would take
//! another module off, only after the player confirms. Right-click takes a
//! queued module back out. Nothing here knows about any one unit: every unit
//! whose file lists `refits` gets this tab.

use super::selection::dps;
use super::{whole, Hud, HudAction, Scene, ENERGY, GAP, MASS};
use crate::audio::Sfx;
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use glam::Vec2;
use mc_data::{BlueprintId, Blueprints, RefitSet, UnitBlueprint, MAX_REFIT_SLOTS};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::OrderKind;

/// The construction amber: what is being fitted right now.
const BUILDING: u32 = 0xFFA928;
/// What the unit has on.
const FITTED: u32 = 0x78E08A;
const LABEL_W: f32 = 104.0;
const TILE_W: f32 = 172.0;
const ARROW_W: f32 = 26.0;
const OR_W: f32 = 34.0;

/// A choice that takes a module off, waiting for the player to say yes.
pub struct Prompt {
    title: String,
    body: Vec<String>,
    confirm: String,
    actions: Vec<HudAction>,
    /// The tile it was asked from, and the top of the panel: the card stands over the panel.
    anchor: Rect,
    top: f32,
}

/// Where a unit's refits stand: what is on, what the queue will add, and in what order.
struct Plan<'a> {
    set: &'a RefitSet,
    /// Fitted now, and once the queue is done.
    now: [u8; MAX_REFIT_SLOTS],
    planned: [u8; MAX_REFIT_SLOTS],
    /// The refits in the queue, in order: (slot, module, kit).
    queue: Vec<(usize, u8, BlueprintId)>,
    /// How far along the first refit in the queue is, when it is the unit's current order.
    progress: Option<f32>,
}

impl<'a> Plan<'a> {
    fn of(s: &'a Scene, u: &UnitInstance) -> Option<Plan<'a>> {
        let (set, loadout) = s.blueprints.loadout(BlueprintId(u.blueprint as u16))?;
        let mut planned = loadout.fitted;
        let mut queue = Vec::new();
        let orders = s.queue_of(u.unit_id);
        let mut progress = None;
        for (i, o) in orders.iter().flat_map(|q| q.orders.iter()).enumerate() {
            if o.kind != OrderKind::Upgrade {
                continue;
            }
            let Some((kit_set, slot, module)) = s.blueprints.kit(o.blueprint) else {
                continue;
            };
            if kit_set.base != set.base {
                continue;
            }
            if let Ok(next) = set.fit(&planned, slot, module) {
                planned = next;
                if i == 0 {
                    progress = orders.map(|q| q.progress);
                }
                queue.push((slot, module, o.blueprint));
            }
        }
        Some(Plan {
            set,
            now: loadout.fitted,
            planned,
            queue,
            progress,
        })
    }

    fn fitted(&self, fitted: &[u8; MAX_REFIT_SLOTS], slot: usize, module: u8) -> bool {
        fitted[slot]
            .checked_sub(1)
            .is_some_and(|on| self.set.chain(slot, on).any(|m| m == module))
    }

    /// The modules to queue, earliest tier first, for `module` to go on after the queue.
    fn route(&self, slot: usize, module: u8) -> Vec<u8> {
        let mut route: Vec<u8> = self
            .set
            .chain(slot, module)
            .take_while(|&m| !self.fitted(&self.planned, slot, m))
            .collect();
        route.reverse();
        route
    }

    fn blueprint(&self, fitted: &[u8; MAX_REFIT_SLOTS]) -> BlueprintId {
        self.set.loadouts[self.set.index(fitted)]
    }
}

/// How one module stands on the unit.
#[derive(Clone, Copy, PartialEq)]
enum State {
    Fitted,
    /// Going on now: how far along.
    Fitting(f32),
    /// Waiting in the queue, this many refits from the front.
    Queued(usize),
    /// Can be queued.
    Open,
}

/// `floor`: the top of what stands over the panel, the hover card's foot.
pub fn tab(hud: &mut Hud, ui: &mut Ui, s: &Scene, u: &UnitInstance, r: Rect, floor: f32) {
    let Some(plan) = Plan::of(s, u) else {
        return;
    };
    let slots = plan.set.slots.len().max(1);
    let gap = 4.0;
    let row_h = ((r.h - gap * (slots as f32 - 1.0)) / slots as f32).clamp(26.0, 44.0);
    let mut hovered: Option<(usize, u8, Rect)> = None;
    // While a replacement waits for an answer, the tiles under it are not live.
    let live = ui.interactive;
    ui.interactive &= hud.refit_prompt.is_none();
    for (si, slot) in plan.set.slots.iter().enumerate() {
        let y = r.y + si as f32 * (row_h + gap);
        let row = Rect::new(r.x, y, r.w, row_h);
        // The slot: where on the unit it is, and what it has on.
        ui.fill(Rect::new(row.x, row.y + 3.0, 2.0, row.h - 6.0), rgb(palette::TEXT, 0.8));
        ui.text_fit_left(
            row.x + 9.0,
            row.y + row_h * 0.34,
            LABEL_W - 12.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            &slot.name,
        );
        let on = match plan.now[si].checked_sub(1) {
            Some(m) => slot.modules[m as usize].name.as_str(),
            None => "Empty",
        };
        ui.text_fit_left(
            row.x + 9.0,
            row.y + row_h * 0.74,
            LABEL_W - 12.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            on,
        );

        // The modules: each root, its later tiers behind arrows; "or" between roots.
        let mut x = row.x + LABEL_W;
        let roots: Vec<u8> = (0..slot.modules.len() as u8)
            .filter(|&m| slot.modules[m as usize].after.is_none())
            .collect();
        for (ri, &root) in roots.iter().enumerate() {
            if ri > 0 {
                or_mark(ui, x, row);
                x += OR_W;
            }
            let mut line = vec![root];
            // Later tiers in the order the file gives them.
            let mut i = 0;
            while i < line.len() {
                let at = line[i];
                for (m, module) in slot.modules.iter().enumerate() {
                    if module.after == Some(at) {
                        line.push(m as u8);
                    }
                }
                i += 1;
            }
            for (li, &m) in line.iter().enumerate() {
                if li > 0 {
                    tier_arrow(ui, x, row);
                    x += ARROW_W;
                }
                let w = TILE_W.min((r.right() - x).max(60.0));
                let at = Rect::new(x, row.y, w, row_h);
                if at.right() <= r.right() + 0.5 {
                    if module_tile(hud, ui, s, &plan, si, m, at, r.y - 44.0) {
                        hovered = Some((si, m, at));
                    }
                }
                x += w;
            }
        }
    }
    ui.interactive = live;
    if let Some((slot, m, at)) = hovered.filter(|_| hud.refit_prompt.is_none()) {
        card(ui, s, u, &plan, slot, m, at, floor - GAP);
    }
}

fn or_mark(ui: &mut Ui, x: f32, row: Rect) {
    let cx = x + OR_W * 0.5;
    ui.vline(cx, row.y + 3.0, row.h * 0.5 - 9.0, rgb(palette::LINE, 0.18));
    ui.vline(cx, row.mid_y() + 6.0, row.h * 0.5 - 9.0, rgb(palette::LINE, 0.18));
    ui.text_centred(cx, row.mid_y(), type_scale::MICRO, rgb(palette::DIM, 1.0), "or");
}

fn tier_arrow(ui: &mut Ui, x: f32, row: Rect) {
    let (a, b) = (Vec2::new(x + 5.0, row.mid_y()), Vec2::new(x + ARROW_W - 8.0, row.mid_y()));
    ui.stroke(a, b, 1.4, rgb(palette::TEXT, 0.8));
    super::icons::arrow_head(ui, Vec2::new(b.x + 2.0, b.y), Vec2::X, 4.5, 1.4, rgb(palette::TEXT, 0.8));
}

fn state(plan: &Plan, slot: usize, m: u8) -> State {
    if plan.fitted(&plan.now, slot, m) {
        return State::Fitted;
    }
    match plan.queue.iter().position(|&(s, q, _)| s == slot && q == m) {
        Some(0) if plan.progress.is_some() => State::Fitting(plan.progress.unwrap_or(0.0)),
        Some(i) => State::Queued(i),
        None => State::Open,
    }
}

/// One module's tile. Returns whether the pointer is on it.
#[allow(clippy::too_many_arguments)]
fn module_tile(hud: &mut Hud, ui: &mut Ui, s: &Scene, plan: &Plan, slot: usize, m: u8, r: Rect, top: f32) -> bool {
    let module = plan.set.module(slot, m);
    let st = state(plan, slot, m);
    // Superseded: an earlier tier under what is (or will be) on. Nothing to do there.
    let under = plan.fitted(&plan.planned, slot, m) && plan.planned[slot] != m + 1;
    let lit = matches!(st, State::Fitted | State::Fitting(_) | State::Queued(_));
    let t = hud.tile(ui, id("refit", module.kit.0 as usize), r, lit, true);
    let inner = Rect::new(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h - 6.0);
    let (tone, status) = match st {
        State::Fitted if under => (FITTED, "Fitted  \u{b7}  under the next tier".to_owned()),
        State::Fitted => (FITTED, "Fitted".to_owned()),
        State::Fitting(p) => (BUILDING, format!("Refitting  {:.0}%", p * 100.0)),
        State::Queued(i) => (palette::TEXT, format!("Queued  \u{b7}  {}", ordinal(i + 1))),
        State::Open => {
            let route = plan.route(slot, m);
            let mass: f32 = route.iter().map(|&x| plan.set.module(slot, x).cost_mass.to_f32()).sum();
            let energy: f32 = route.iter().map(|&x| plan.set.module(slot, x).cost_energy.to_f32()).sum();
            (palette::DIM, format!("{}  \u{b7}  {}", short(mass), short(energy)))
        }
    };
    match st {
        State::Fitting(p) => {
            ui.gradient_v(inner, rgb(BUILDING, 0.06), rgb(BUILDING, 0.2 + 0.15 * (ui.time * 3.2).sin().abs()));
            let track = Rect::new(inner.x + 4.0, inner.bottom() - 4.0, inner.w - 8.0, 2.0);
            ui.fill(track, rgb(BUILDING, 0.2));
            ui.fill(Rect::new(track.x, track.y, track.w * p.clamp(0.0, 1.0), 2.0), rgb(BUILDING, 1.0));
        }
        State::Fitted => ui.gradient_h(inner, rgb(FITTED, 0.14), rgb(FITTED, 0.0)),
        _ => {}
    }
    ui.fill(Rect::new(r.x + 3.0, r.y + 5.0, 2.0, r.h - 10.0), rgb(tone, if st == State::Open { 0.35 } else { 0.95 }));
    let text_w = r.w - 18.0;
    ui.text_fit_left(
        r.x + 11.0,
        r.y + r.h * 0.36,
        text_w,
        super::build::NAME,
        rgb(0xFFFFFF, if under { 0.55 } else { 1.0 }),
        &module.name,
    );
    ui.text_fit_left(
        r.x + 11.0,
        r.y + r.h * 0.72,
        text_w,
        type_scale::MICRO,
        rgb(tone, 1.0),
        &status,
    );
    if t.clicked {
        click(hud, ui, s, plan, slot, m, r, top, st);
    }
    if t.right_clicked {
        if let State::Fitting(_) | State::Queued(_) = st {
            ui.audio.play(Sfx::Back);
            hud.actions.push(HudAction::CancelRefit(module.kit));
        }
    }
    t.hovered
}

#[allow(clippy::too_many_arguments)]
fn click(hud: &mut Hud, ui: &mut Ui, s: &Scene, plan: &Plan, slot: usize, m: u8, r: Rect, top: f32, st: State) {
    if st != State::Open || plan.fitted(&plan.planned, slot, m) {
        ui.audio.play(Sfx::Deny);
        return;
    }
    let route = plan.route(slot, m);
    let kits: Vec<BlueprintId> = route.iter().map(|&x| plan.set.module(slot, x).kit).collect();
    let module = plan.set.module(slot, m);
    let first = route[0];
    let Some(out) = plan.set.replaces(&plan.planned, slot, first) else {
        ui.audio.play(Sfx::Select);
        hud.actions.push(HudAction::Refit(kits));
        return;
    };
    // It would take something off: say what, and wait for a yes.
    let old = plan.set.module(slot, out);
    let queued: Vec<BlueprintId> = plan
        .queue
        .iter()
        .filter(|&&(qs, _, _)| qs == slot)
        .map(|&(_, _, kit)| kit)
        .collect();
    let mut body = Vec::new();
    let mut actions = Vec::new();
    let fitted_now = plan.now[slot] != 0;
    if !queued.is_empty() {
        // What is only queued comes back out instead of going on and straight off again.
        body.push(format!(
            "{} comes out of the queue.",
            queued
                .iter()
                .filter_map(|&k| s.blueprints.kit(k).map(|(set, sl, mm)| set.module(sl, mm).name.clone()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        actions.extend(queued.iter().map(|&k| HudAction::CancelRefit(k)));
    }
    if fitted_now {
        let on = plan.set.module(slot, plan.now[slot] - 1);
        body.push(format!("{} is taken off when the refit starts; its cost is not returned.", on.name));
    }
    if route.len() > 1 {
        body.push(format!(
            "Queues {} first.",
            route[..route.len() - 1]
                .iter()
                .map(|&x| plan.set.module(slot, x).name.as_str())
                .collect::<Vec<_>>()
                .join(", then ")
        ));
    }
    actions.push(HudAction::Refit(kits));
    let replaced = if fitted_now { plan.set.module(slot, plan.now[slot] - 1) } else { old };
    ui.audio.play(Sfx::Tick);
    hud.refit_prompt = Some(Prompt {
        title: format!("Replace {} with {}?", replaced.name, module.name),
        body,
        confirm: "Replace".to_owned(),
        actions,
        anchor: r,
        top,
    });
}

/// The confirmation over the panel. Clicking away or Keep leaves things as they are.
/// `floor` is the top of whatever stands over the panel (the queue strip), so the card clears it.
pub fn prompt(hud: &mut Hud, ui: &mut Ui, floor: f32) {
    let Some(p) = hud.refit_prompt.take() else {
        return;
    };
    let w = 420.0;
    let h = 96.0 + p.body.len() as f32 * 20.0;
    let r = Rect::new(
        (p.anchor.x + p.anchor.w * 0.5 - w * 0.5).clamp(14.0, ui.size.x - w - 14.0),
        (p.top.min(floor) - GAP - h).max(14.0),
        w,
        h,
    );
    hud.claim(ui, r);
    ui.panel(r);
    ui.fill(Rect::new(r.x, r.y, 3.0, r.h), rgb(palette::WARN, 1.0));
    ui.text_fit_left(r.x + 18.0, r.y + 24.0, w - 36.0, type_scale::ITEM, rgb(0xFFFFFF, 1.0), &p.title);
    let mut y = r.y + 50.0;
    for line in &p.body {
        ui.text_fit_left(r.x + 18.0, y, w - 36.0, type_scale::BODY, rgb(palette::DIM, 1.0), line);
        y += 20.0;
    }
    let by = r.bottom() - 42.0;
    let yes = ui.button(
        id("refit-yes", 0),
        Rect::new(r.x + 18.0, by, 150.0, 30.0),
        &p.confirm,
        ButtonKind::Primary,
        true,
    );
    let no = ui.button(
        id("refit-no", 0),
        Rect::new(r.x + 178.0, by, 120.0, 30.0),
        "Keep",
        ButtonKind::Secondary,
        true,
    );
    let pointer = ui.cursor - ui.shift;
    let away = ui.input.pressed && !r.contains(pointer) && !p.anchor.contains(pointer);
    if yes {
        ui.audio.play(Sfx::Select);
        hud.actions.extend(p.actions);
    } else if no || away || ui.input.key(Key::Escape) {
        ui.audio.play(Sfx::Back);
    } else {
        hud.refit_prompt = Some(p);
    }
}

/// Everything about a module, over its tile: what it adds, its price, and what it takes off.
#[allow(clippy::too_many_arguments)]
fn card(ui: &mut Ui, s: &Scene, u: &UnitInstance, plan: &Plan, slot: usize, m: u8, tile: Rect, bottom: f32) {
    let module = plan.set.module(slot, m);
    let bps = s.blueprints;
    // Against what the unit will have when this comes up.
    let before = plan.blueprint(&plan.planned);
    let route = plan.route(slot, m);
    let mut after = plan.planned;
    for &x in &route {
        after[slot] = x + 1;
    }
    let (from, to) = (bps.unit(before), bps.unit(if route.is_empty() { before } else { plan.blueprint(&after) }));
    let mut rows: Vec<(String, String, u32)> = Vec::new();
    let mut diff = |label: &str, a: f32, b: f32, unit: &str, tone: u32| {
        if (b - a).abs() >= 0.5 {
            let sign = if b > a { "+" } else { "\u{2212}" };
            let t = if b > a { tone } else { palette::BAD };
            rows.push((label.to_owned(), format!("{sign}{}{unit}", whole((b - a).abs())), t));
        }
    };
    diff("Integrity", from.health.to_f32(), to.health.to_f32(), "", palette::TEXT);
    diff("Damage / s", dps(from), dps(to), "", palette::TEXT);
    diff("Weapon Range", from.max_weapon_range().to_f32(), to.max_weapon_range().to_f32(), " m", palette::TEXT);
    let power = |b: &UnitBlueprint| b.builder.as_ref().map_or(0.0, |x| x.power.to_f32());
    diff("Build Power", power(from), power(to), "", palette::TEXT);
    let (ea, eb) = (&from.economy, &to.economy);
    diff("Materials Income", ea.mass_income.to_f32(), eb.mass_income.to_f32(), " / s", MASS);
    diff("Energy Income", ea.energy_income.to_f32(), eb.energy_income.to_f32(), " / s", ENERGY);
    diff("Energy Upkeep", eb.energy_upkeep.to_f32(), ea.energy_upkeep.to_f32(), " / s", ENERGY);
    let shield = |b: &UnitBlueprint| b.shield.map_or(0.0, |x| x.health.to_f32());
    diff("Shield", shield(from), shield(to), "", palette::TEXT);
    if to.tech > from.tech {
        rows.push(("Construction".into(), format!("Tech {}", to.tech), palette::TEXT));
    }
    let new_weapons: Vec<&str> = to
        .weapons
        .iter()
        .filter(|w| !from.weapons.iter().any(|f| f.name == w.name))
        .map(|w| w.name.as_str())
        .collect();

    let mut notes: Vec<(String, u32)> = Vec::new();
    match state(plan, slot, m) {
        State::Fitted => notes.push(("Fitted.".into(), FITTED)),
        State::Fitting(_) => notes.push(("Being fitted now  \u{b7}  right-click to cancel".into(), BUILDING)),
        State::Queued(_) => notes.push(("In the queue  \u{b7}  right-click to take it out".into(), palette::TEXT)),
        State::Open => {
            if route.len() > 1 {
                let names: Vec<&str> =
                    route[..route.len() - 1].iter().map(|&x| plan.set.module(slot, x).name.as_str()).collect();
                notes.push((format!("Queues {} first", names.join(", then ")), palette::TEXT));
            }
            if let Some(out) = route.first().and_then(|&f| plan.set.replaces(&plan.planned, slot, f)) {
                notes.push((format!("Replaces {}", plan.set.module(slot, out).name), palette::WARN));
            }
        }
    }

    let w = 360.0;
    let (x, cw) = (18.0, w - 36.0);
    let h = 118.0 + rows.len() as f32 * 19.0 + if new_weapons.is_empty() { 0.0 } else { 22.0 } + notes.len() as f32 * 20.0 + 26.0;
    let r = Rect::new(
        (tile.x + tile.w * 0.5 - w * 0.5).clamp(14.0, ui.size.x - w - 14.0),
        (bottom - h).max(14.0),
        w,
        h,
    );
    ui.panel(r);
    let x = r.x + x;
    ui.text_fit_left(x, r.y + 24.0, cw, type_scale::ITEM, rgb(0xFFFFFF, 1.0), &module.name);
    ui.text_fit_left(
        x,
        r.y + 45.0,
        cw,
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        &format!("{}  \u{b7}  {}", plan.set.slots[slot].name, module.summary),
    );
    // The price of this module (and the tiers queued before it), and how long this unit takes.
    let (mass, energy, time): (f32, f32, f32) = if route.is_empty() {
        (module.cost_mass.to_f32(), module.cost_energy.to_f32(), module.build_time.to_f32())
    } else {
        route.iter().fold((0.0, 0.0, 0.0), |(a, b, c), &x| {
            let mm = plan.set.module(slot, x);
            (a + mm.cost_mass.to_f32(), b + mm.cost_energy.to_f32(), c + mm.build_time.to_f32())
        })
    };
    let seconds = time / power(s.bp(u)).max(0.1);
    for (i, (label, value, tone)) in [
        ("Materials", whole(mass), MASS),
        ("Energy", whole(energy), ENERGY),
        ("Time", super::clock(seconds), palette::TEXT),
    ]
    .iter()
    .enumerate()
    {
        let cx = x + i as f32 * cw / 3.0;
        ui.fill(Rect::new(cx, r.y + 60.0, 2.0, 28.0), rgb(*tone, 0.9));
        ui.text(cx + 10.0, r.y + 66.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
        ui.text(cx + 10.0, r.y + 82.0, type_scale::VALUE, rgb(*tone, 1.0), value);
    }
    ui.hline(x, r.y + 100.0, cw, rgb(palette::LINE, 0.16));
    let mut y = r.y + 116.0;
    for (label, value, tone) in &rows {
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
        ui.text_right(x + cw, y, type_scale::VALUE, rgb(*tone, 1.0), value);
        y += 19.0;
    }
    if !new_weapons.is_empty() {
        ui.fill(Rect::new(x, y - 5.0, 2.0, 10.0), rgb(super::style::Family::Combat.tone(), 1.0));
        ui.text_fit_left(x + 8.0, y, cw - 8.0, type_scale::CAPTION, rgb(palette::TEXT, 1.0), &new_weapons.join("  \u{b7}  "));
        y += 22.0;
    }
    for (line, tone) in &notes {
        ui.text_fit_left(x, y, cw, type_scale::CAPTION, rgb(*tone, 1.0), line);
        y += 20.0;
    }
    let hint = match state(plan, slot, m) {
        State::Open => "Click to queue",
        State::Fitting(_) | State::Queued(_) => "Right-click to cancel",
        State::Fitted => "",
    };
    ui.text(x, r.bottom() - 14.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), hint);
}

/// 1st, 2nd, 3rd.
fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A price on a tile: 600, 9k, 24k.
fn short(v: f32) -> String {
    if v >= 10_000.0 {
        format!("{:.0}k", v / 1000.0)
    } else if v >= 1000.0 {
        format!("{:.1}k", v / 1000.0)
    } else {
        whole(v)
    }
}

/// The queue strip's words for a refit waiting there: the module's name.
pub fn queued_name<'a>(blueprints: &'a Blueprints, kit: BlueprintId) -> Option<&'a str> {
    blueprints.kit(kit).map(|(set, slot, m)| set.module(slot, m).name.as_str())
}

/// Tip text for a queued refit in the queue strip.
pub fn queue_hint(blueprints: &Blueprints, kit: BlueprintId) -> Option<String> {
    queued_name(blueprints, kit).map(|n| format!("Refit  {n}  \u{b7}  Right-Click Cancels"))
}

/// What a unit of blueprint `id` has fitted: the top module in each slot, with the
/// slot it is in and its tier in that slot (one for a module fitted over nothing).
pub fn fitted(blueprints: &Blueprints, id: BlueprintId) -> Vec<(&str, &mc_data::Module, u8)> {
    let Some((set, loadout)) = blueprints.loadout(id) else {
        return Vec::new();
    };
    set.slots
        .iter()
        .enumerate()
        .filter_map(|(s, slot)| {
            let m = loadout.module(s)?;
            Some((slot.name.as_str(), set.module(s, m), set.chain(s, m).count() as u8))
        })
        .collect()
}

/// The fitted modules as a row of small icons from `x` at mid-height `y`, and a list of
/// them over the row while the pointer is on it. Returns the row's width (zero: nothing fitted).
pub fn icon_row(ui: &mut Ui, blueprints: &Blueprints, id: BlueprintId, x: f32, y: f32, size: f32) -> f32 {
    let on = fitted(blueprints, id);
    if on.is_empty() {
        return 0.0;
    }
    let gap = 3.0;
    let row = Rect::new(x, y - size * 0.5, on.len() as f32 * (size + gap) - gap, size);
    for (i, (_, module, tier)) in on.iter().enumerate() {
        let cell = Rect::new(x + i as f32 * (size + gap), row.y, size, size);
        ui.fill_cut(cell, 3.0, crate::ui::ink(0.75));
        ui.frame(cell, rgb(palette::LINE, 0.3));
        super::icons::strategic(
            ui,
            module.icon,
            *tier,
            Vec2::new(cell.x + size * 0.5, cell.y + size * 0.5),
            size * 0.36,
            rgb(0xFFFFFF, 0.95),
            crate::ui::ink(0.9),
        );
    }
    let pointer = ui.cursor - ui.shift;
    if row.contains(pointer) {
        let lines: Vec<String> = on
            .iter()
            .map(|(slot, module, _)| format!("{slot}  \u{b7}  {}", module.name))
            .collect();
        let w = lines
            .iter()
            .map(|l| ui.text_width(type_scale::MICRO, l))
            .fold(0.0, f32::max)
            + 28.0;
        let h = 30.0 + lines.len() as f32 * 17.0;
        let card = Rect::new(
            row.x.min(ui.size.x - w - 14.0).max(14.0),
            (row.y - h - 6.0).max(14.0),
            w,
            h,
        );
        ui.frost(card, 0.85);
        ui.fill(Rect::new(card.x, card.y, 2.0, card.h), rgb(palette::TEXT, 1.0));
        ui.text(card.x + 12.0, card.y + 14.0, type_scale::CAPTION, rgb(palette::TEXT, 1.0), "Refits");
        for (i, line) in lines.iter().enumerate() {
            ui.text(card.x + 12.0, card.y + 33.0 + i as f32 * 17.0, type_scale::MICRO, rgb(palette::DIM, 1.0), line);
        }
    }
    row.w
}

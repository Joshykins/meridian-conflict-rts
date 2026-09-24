//! Construction: what the selected builder can make, in tech tabs (T1 to T5)
//! holding a strip of tiles that scrolls sideways, with a button per shelf
//! above it to jump along it, an UPGRADE tab for what the
//! selection can refit into, the queue it is working through above it, and a
//! data card for whatever is hovered.

use super::icons;
use super::selection::{dps, weapon_rows};
use super::style::{domain_wash, Domain, Purpose, ITEM_KEYS};
use super::{has_flag, whole, Hud, HudAction, Scene, ENERGY, GAP, MASS};
use crate::audio::Sfx;
use crate::game::Mode;
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_core::{FxVec2, TICKS_PER_SECOND};
use mc_data::{cat, BlueprintId, UnitBlueprint};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::{flag, OrderKind};

const TILE_W: f32 = 96.0;
/// Tile names: the caption face with less tracking, so most names fit on a line.
pub(super) const NAME: crate::ui::Style = crate::ui::style(mc_render::Face::Medium, 11.0, 1.6);
const TILE_GAP: f32 = 6.0;
const QUEUE_H: f32 = 62.0;
pub const TIERS: u8 = 5;
/// The tab past the tiers: refits.
pub const UPGRADE_TAB: u8 = TIERS + 1;
/// How fast a structure that is not a builder puts on its own upgrade (the sim's `SELF_UPGRADE_POWER`).
const SELF_UPGRADE_POWER: f32 = 10.0;

/// Consecutive queue entries for the same blueprint, as one stack.
struct Stack {
    blueprint: BlueprintId,
    count: usize,
    /// The unit's own upgrade, waiting its turn among what it builds.
    upgrade: bool,
    /// Where the last entry of the stack stands: how the sim finds it to take it out.
    last: FxVec2,
}

/// What the queue strip shows.
struct Queue<'a> {
    stacks: &'a [Stack],
    /// How far along the front entry is.
    progress: f32,
    is_factory: bool,
    repeating: bool,
    /// The builder's work is paused: the queue waits, the front entry holds where it got to.
    paused: bool,
}

pub fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene, units: &[&UnitInstance], r: Rect) {
    // The first finished builder in the selection speaks for it. A factory still going
    // up takes its queue already: it starts on it once it stands.
    let builder_unit = units
        .iter()
        .map(|u| (*u, s.bp(u)))
        .find(|(u, bp)| {
            bp.builder.as_ref().is_some_and(|b| !b.builds.is_empty())
                && (!has_flag(u, flag::UNDER_CONSTRUCTION) || bp.has(cat::FACTORY))
        });
    let upgrader = units
        .iter()
        .map(|u| (*u, s.bp(u)))
        .find(|(u, bp)| {
            (bp.upgrades_to.is_some() || s.blueprints.refit_set(bp.id).is_some())
                && !has_flag(u, flag::UNDER_CONSTRUCTION)
        });
    let refits = upgrader.is_some_and(|(_, bp)| s.blueprints.refit_set(bp.id).is_some());
    if (builder_unit.is_none() && upgrader.is_none()) || r.w < TILE_W + 28.0 {
        hud.build_keys = false;
        hud.build_key = None;
        return;
    }
    let speaker = builder_unit.or(upgrader).expect("one of them");
    let (unit, bp) = speaker;
    // Tier upgrades are tiles on their tiers' tabs, queued like what it builds; the
    // upgrade tab is for refits. What a queued tier makes takes orders at once, and
    // what the tiers beyond make is shown, locked, until they are queued too.
    let climbs = !refits && upgrader.is_some_and(|(u, _)| u.unit_id == unit.unit_id);
    let line = if climbs { successors(s, bp) } else { Vec::new() };
    let plan = planned(s, unit, bp);
    let reached = line.iter().position(|b| b.id == plan.id).map_or(0, |i| i + 1);
    let now: &[BlueprintId] = bp.builder.as_ref().map_or(&[], |b| &b.builds);
    let mut builds: Vec<BlueprintId> = now.to_vec();
    for more in line.iter().filter_map(|b| b.builder.as_ref()) {
        for b in &more.builds {
            if !builds.contains(b) {
                builds.push(*b);
            }
        }
    }
    let is_factory = bp.has(cat::FACTORY);

    // Tech tabs: the tiers this builder has anything in. A new kind of builder
    // opens on its own tier, which is what it was most likely selected for.
    let mut tiers: Vec<u8> = builds
        .iter()
        .map(|b| s.blueprints.unit(*b).tech)
        .chain(line.iter().map(|b| b.tech))
        .collect();
    tiers.sort_unstable();
    tiers.dedup();
    let open = |tab: u8| tiers.contains(&tab) || (tab == UPGRADE_TAB && refits);
    // A refit changes the unit's blueprint to another loadout of the same unit: the panel stays put.
    let family = s.blueprints.base_of(BlueprintId(unit.blueprint as u16)).0 as u32;
    if std::mem::take(&mut hud.want_refit_tab) && refits {
        hud.tab_for = Some(family);
        hud.tab = UPGRADE_TAB;
        rewind(hud);
    }
    if hud.tab_for != Some(family) || !open(hud.tab) {
        hud.tab_for = Some(family);
        hud.tab = if tiers.contains(&bp.tech) {
            bp.tech
        } else {
            tiers.first().copied().unwrap_or(UPGRADE_TAB)
        };
        rewind(hud);
    }

    let queue = s.queue_of(unit.unit_id);
    let wanted = if is_factory {
        OrderKind::Produce
    } else {
        OrderKind::Build
    };
    let mut stacks: Vec<Stack> = Vec::new();
    for o in queue
        .iter()
        .flat_map(|q| &q.orders)
        .filter(|o| o.kind == wanted || o.kind == OrderKind::Upgrade)
    {
        let upgrade = o.kind == OrderKind::Upgrade;
        match stacks.last_mut() {
            Some(top) if top.blueprint == o.blueprint && top.upgrade == upgrade => {
                top.count += 1;
                top.last = o.at;
            }
            _ => stacks.push(Stack {
                blueprint: o.blueprint,
                count: 1,
                upgrade,
                last: o.at,
            }),
        }
    }

    hud.glass(ui, r);
    let (x, cw) = (r.x + 14.0, r.w - 28.0);
    let mut tx = x;
    // A tier's digit or U from the keyboard picks the tab.
    if let Some(key) = hud.build_key {
        let tab = match key {
            '1'..='5' => Some(key as u8 - b'0'),
            // The upgrade tab, or the tier of the next upgrade to queue.
            'U' if refits => Some(UPGRADE_TAB),
            'U' => line.get(reached).map(|b| b.tech),
            _ => None,
        };
        if let Some(tab) = tab {
            hud.build_key = None;
            if open(tab) {
                ui.audio.play(Sfx::Tick);
                hud.tab = tab;
                rewind(hud);
                hud.shelf = None;
            } else {
                ui.audio.play(Sfx::Deny);
            }
        }
    }
    for tab in (1..=TIERS).chain([UPGRADE_TAB]) {
        let upgrade = tab == UPGRADE_TAB;
        let has = open(tab);
        if upgrade && !has {
            continue;
        }
        let w = if upgrade { 124.0 } else { 58.0 };
        let upgrade_label = if refits { "Refit" } else { "Upgrade" };
        let tr = Rect::new(tx, r.y + 10.0, w, 26.0);
        let t = hud.tile(ui, id("tech-tab", tab as usize), tr, hud.tab == tab, has);
        let tone = if hud.tab == tab {
            rgb(0xFFFFFF, 1.0)
        } else {
            rgb(palette::TEXT, if has { 0.6 + 0.4 * t.glow } else { 0.2 })
        };
        let label = if upgrade {
            upgrade_label.to_owned()
        } else {
            format!("T{tab}")
        };
        key_cap(
            ui,
            tr.x + 5.0,
            tr.y + 5.5,
            if upgrade { 'U' } else { (b'0' + tab) as char },
            hud.build_keys && has,
        );
        if upgrade {
            icons::glyph(ui, icons::Glyph::Upgrade, Vec2::new(tr.x + 33.0, tr.mid_y() + 1.0), 6.0, tone);
            ui.text_fit_left(tr.x + 44.0, tr.mid_y(), tr.w - 48.0, type_scale::BUTTON, tone, &label);
        } else {
            ui.text_centred(tr.x + tr.w * 0.5 + 9.0, tr.mid_y(), type_scale::BUTTON, tone, &label);
        }
        if has {
            // A mark while an upgrade (a refit, or this tier's) is waiting or under way.
            let pending = if upgrade {
                upgrader.is_some_and(|(u, _)| {
                    s.queue_of(u.unit_id)
                        .is_some_and(|q| q.orders.iter().any(|o| o.kind == OrderKind::Upgrade))
                })
            } else {
                line[..reached].iter().any(|b| b.tech == tab)
            };
            if pending {
                let k = 0.5 + 0.5 * (ui.time * 4.0).sin();
                ui.disc(
                    Vec2::new(tr.right() - 7.0, tr.y + 7.0),
                    2.5,
                    rgb(palette::TEXT, 0.5 + 0.5 * k),
                );
            }
        }
        if t.clicked && hud.tab != tab {
            ui.audio.play(Sfx::Tick);
            hud.tab = tab;
            rewind(hud);
            hud.shelf = None;
        }
        tx += w + 4.0;
    }
    ui.section(
        tx + 10.0,
        r.y + 23.0,
        r.right() - 14.0 - tx - 10.0,
        &if hud.tab == UPGRADE_TAB {
            format!("Refit  \u{b7}  {}", bp.name)
        } else if builds.is_empty() {
            format!("Upgrades  \u{b7}  {}", bp.name)
        } else {
            format!("Construction  \u{b7}  {}", bp.name)
        },
    );

    // How to drive the panel from the keyboard, at the header's right end.
    let hint = if hud.build_keys {
        "Tier 1\u{2013}5  \u{b7}  Shelf  \u{b7}  Item  \u{b7}  ESC"
    } else {
        "Keys"
    };
    let hw = ui.text_width(type_scale::MICRO, hint);
    ui.fill(Rect::new(r.right() - 36.0 - hw - 10.0, r.y + 12.0, hw + 32.0, 22.0), ink(0.9));
    key_cap(ui, r.right() - 34.0, r.y + 15.5, 'B', hud.build_keys);
    ui.text_right(
        r.right() - 40.0,
        r.y + 23.0,
        type_scale::MICRO,
        rgb(if hud.build_keys { 0xFFFFFF } else { palette::DIM }, 1.0),
        hint,
    );
    let grid = Rect::new(x, r.y + 44.0, cw - 10.0, r.bottom() - 8.0 - (r.y + 44.0));
    let mut hovered: Option<Hover> = None;
    if hud.tab == UPGRADE_TAB {
        if let Some((u, _)) = upgrader.filter(|_| refits) {
            let floor = if stacks.is_empty() { r.y } else { r.y - GAP - QUEUE_H };
            super::refit::tab(hud, ui, s, u, grid, floor);
        }
    } else {
        let offer = Offer {
            builds: &builds,
            open: plan.builder.as_ref().map_or(&[], |b| &b.builds),
            now,
            line: &line,
            reached,
            under_way: queue
                .and_then(|q| q.orders.first().filter(|o| o.kind == OrderKind::Upgrade).map(|o| (o.blueprint, q.progress))),
            // The test range's free building needs no tech (`Command::Upgrade` skips it too).
            locked: queue
                .map(|q| q.side_tech)
                .filter(|&t| t > 0 && !s.view.range.as_ref().is_some_and(|r| r.free_build))
                .and_then(|tech| {
                line.iter()
                    .enumerate()
                    .skip(reached)
                    .map(|(i, b)| (i, s.blueprints.upgrade_needs(b)))
                    .find(|&(_, needs)| needs > tech)
            }),
        };
        // The strip runs the full width: it has no scroll bar at the side.
        hovered = tiles(hud, ui, s, &offer, &stacks, is_factory, Rect { w: cw, ..grid });
    }

    let queue_rect = Rect::new(r.x, r.y - GAP - QUEUE_H, r.w, QUEUE_H);
    // A factory always shows its strip: REPEAT lives there.
    let has_queue = !stacks.is_empty() || (is_factory && builder_unit.is_some());
    if has_queue {
        let strip = Queue {
            stacks: &stacks,
            progress: queue.map_or(0.0, |q| q.progress),
            is_factory,
            repeating: has_flag(unit, flag::REPEAT),
            paused: unit.paused(),
        };
        draw_queue(hud, ui, s, queue_rect, &strip);
    }
    super::refit::prompt(hud, ui, if has_queue { queue_rect.y } else { r.y });
    let floor = if has_queue { queue_rect.y } else { r.y };
    match hovered {
        Some(Hover::Unit(item, tile, locked)) => {
            let power = bp.builder.as_ref().map_or(0.0, |b| b.power.to_f32());
            let hint = match locked {
                Some(up) => format!("Queue the {} Upgrade First", up.name),
                None if is_factory => "Click +1  \u{b7}  Shift +5  \u{b7}  Right-Click -1".to_owned(),
                None => "Click or drag to place  \u{b7}  Shift keeps placing  \u{b7}  Right-click -1".to_owned(),
            };
            data_card(hud, ui, item, power, &hint, tile, floor - GAP);
        }
        Some(Hover::Climb(i, tile)) => upgrade_card(hud, ui, s, unit, bp, &line, i, reached, tile, floor - GAP),
        None => {}
    }
}

/// A key cap: the letter in a small box, lit while the construction keys are live.
pub fn key_cap(ui: &mut Ui, x: f32, y: f32, key: char, live: bool) {
    let r = Rect::new(x, y, 15.0, 15.0);
    ui.fill(r, rgb(0xFFFFFF, if live { 0.9 } else { 0.1 }));
    ui.frame(r, rgb(0xFFFFFF, if live { 1.0 } else { 0.35 }));
    ui.text_centred(
        r.x + r.w * 0.5 + 0.5,
        r.mid_y(),
        crate::ui::style(mc_render::Face::Bold, 10.5, 0.0),
        rgb(if live { palette::INK } else { palette::TEXT }, if live { 1.0 } else { 0.75 }),
        &key.to_string(),
    );
}

/// The shelf buttons over the strip.
const SHELF_H: f32 = 24.0;
/// Tiles on the strip: taller than a grid tile, for a bigger picture.
const STRIP_H: f32 = 126.0;
/// Room between the last tile of one shelf and the first of the next.
const SHELF_GAP: f32 = 22.0;
/// The arrow buttons at the strip's ends while it runs past the panel.
const ARROW_W: f32 = 24.0;
/// How far past the strip's edge a tile takes to fade out.
const FADE: f32 = 34.0;

/// A tile's place along the strip.
struct Slot<'a> {
    item: &'a UnitBlueprint,
    shelf: Purpose,
    /// Its place on its shelf, which picks its item key.
    n: usize,
    at: f32,
    /// A tier upgrade: its step along the builder's line of successors.
    climb: Option<usize>,
}

/// What a builder offers on its tier tabs.
struct Offer<'a> {
    /// Everything on the tabs: what it builds now and what the tiers up its line will.
    builds: &'a [BlueprintId],
    /// What it takes orders for: what the last tier queued builds.
    open: &'a [BlueprintId],
    /// What it builds as it stands.
    now: &'a [BlueprintId],
    /// Its tier upgrades, in order, and how many of them are queued.
    line: &'a [&'a UnitBlueprint],
    reached: usize,
    /// The upgrade at the front of its queue, and how far along it is.
    under_way: Option<(BlueprintId, f32)>,
    /// The first tier up `line` the side has not reached the tech for, and that tech
    /// (`Blueprints::upgrade_needs`): it and the tiers after it cannot be queued yet.
    locked: Option<(usize, u8)>,
}

/// The tile under the cursor.
enum Hover<'a> {
    /// What it builds, and the upgrade it waits on when it is locked.
    Unit(&'a UnitBlueprint, Rect, Option<&'a UnitBlueprint>),
    /// A tier upgrade: its step along the line.
    Climb(usize, Rect),
}

/// Starts the strip over at its left end.
fn rewind(hud: &mut Hud) {
    hud.build_scroll = 0.0;
    hud.build_shown = 0.0;
}

/// The tiles of the open tier on one strip, shelf after shelf, with a button
/// per shelf above it. The strip scrolls sideways (wheel, arrows, its track,
/// or a shelf's button or key) when it is longer than the panel. Returns the
/// tile hovered.
fn tiles<'a>(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &'a Scene,
    offer: &Offer<'a>,
    stacks: &[Stack],
    is_factory: bool,
    grid: Rect,
) -> Option<Hover<'a>> {
    let placing = match s.view.mode {
        Mode::Place(b) => Some(b),
        _ => None,
    };
    let mut items: Vec<&UnitBlueprint> = offer
        .builds
        .iter()
        .map(|b| s.blueprints.unit(*b))
        .filter(|b| b.tech == hud.tab)
        .collect();
    items.sort_by_key(|b| Purpose::of(b));
    // The tier's upgrade leads the strip, ahead of what it opens.
    let climbs: Vec<(usize, &UnitBlueprint)> = offer
        .line
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, b)| b.tech == hud.tab)
        .collect();
    // The upgrade a locked item waits on: the first tier up the line that builds it.
    let needs = |item: &UnitBlueprint| -> Option<&'a UnitBlueprint> {
        if offer.open.contains(&item.id) {
            return None;
        }
        offer
            .line
            .iter()
            .copied()
            .find(|b| b.builder.as_ref().is_some_and(|k| k.builds.contains(&item.id)))
    };
    if items.is_empty() && climbs.is_empty() {
        hud.build_key = None;
        ui.text(
            grid.x,
            grid.y + 20.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Nothing at this tier yet",
        );
        return None;
    }

    // Where every tile sits along the strip, and where each shelf starts and ends.
    let lead = items.first().map(|b| Purpose::of(b));
    let lay = |tile_w: f32, shelf_gap: f32| {
        let mut slots: Vec<Slot> = Vec::new();
        let mut shelves: Vec<(Purpose, f32, f32, usize)> = Vec::new();
        let mut at = 0.0;
        for &(i, item) in &climbs {
            let shelf = lead.unwrap_or_else(|| Purpose::of(item));
            slots.push(Slot { item, shelf, n: usize::MAX, at, climb: Some(i) });
            at += tile_w + TILE_GAP;
        }
        for p in Purpose::ALL {
            let on: Vec<&UnitBlueprint> = items.iter().copied().filter(|b| Purpose::of(b) == p).collect();
            if on.is_empty() {
                continue;
            }
            if !shelves.is_empty() || !climbs.is_empty() {
                at += shelf_gap - TILE_GAP;
            }
            let start = at;
            for (n, item) in on.iter().enumerate() {
                slots.push(Slot { item, shelf: p, n, at, climb: None });
                at += tile_w + TILE_GAP;
            }
            shelves.push((p, start, at - TILE_GAP, on.len()));
        }
        (slots, shelves, at - TILE_GAP)
    };
    let strip = Rect::new(grid.x, grid.y + SHELF_H + 8.0, grid.w, STRIP_H);
    let (mut slots, mut shelves, mut length) = lay(TILE_W, SHELF_GAP);
    let overflow = length > strip.w;
    let view = if overflow {
        Rect::new(strip.x + ARROW_W + 6.0, strip.y, strip.w - 2.0 * (ARROW_W + 6.0), STRIP_H)
    } else {
        strip
    };
    // Past the panel, the strip steps a whole tile at a time and its tiles are sized
    // so a whole number of them fill the view: it never stops on half a tile, or on
    // a gap where one has faded out. Shelves then meet at a rule, without the extra room.
    let (tile_w, shelf_gap) = if overflow {
        let fit = ((view.w + TILE_GAP) / (TILE_W + TILE_GAP)).round().max(1.0);
        let w = (view.w + TILE_GAP) / fit - TILE_GAP;
        // Never wider than a tile and a quarter: one more, smaller, fits better.
        let fit = if w > TILE_W * 1.25 { fit + 1.0 } else { fit };
        ((view.w + TILE_GAP) / fit - TILE_GAP, TILE_GAP)
    } else {
        (TILE_W, SHELF_GAP)
    };
    if overflow {
        (slots, shelves, length) = lay(tile_w, shelf_gap);
    }
    let pitch = tile_w + TILE_GAP;
    let max_scroll = (length - view.w).max(0.0);
    // How many tiles the view holds, and a page of them for the arrows.
    let in_view = ((view.w + TILE_GAP) / pitch).round().max(1.0);
    let page = (in_view - 1.0).max(1.0) * pitch;
    let snap = |x: f32| if overflow { ((x / pitch).round() * pitch).clamp(0.0, max_scroll) } else { x.clamp(0.0, max_scroll) };
    let start_of = |p: Purpose| shelves.iter().find(|s| s.0 == p).map(|s| s.1);

    // A key from the keyboard: a shelf, then an item on it.
    if let Some(key) = hud.build_key.take() {
        if let Some(p) = Purpose::ALL.into_iter().find(|p| p.key() == key) {
            if let Some(start) = start_of(p) {
                ui.audio.play(Sfx::Tick);
                hud.shelf = Some(p);
                hud.build_scroll = snap(start);
            } else {
                ui.audio.play(Sfx::Deny);
            }
        } else if let Some(n) = ITEM_KEYS.iter().position(|k| *k == key) {
            let shelf = current_shelf(hud, &slots, tile_w);
            match slots.iter().find(|t| t.shelf == shelf && t.n == n) {
                Some(t) if needs(t.item).is_none() => {
                    ui.audio.play(Sfx::Select);
                    hud.actions.push(HudAction::Build(t.item.id));
                }
                _ => ui.audio.play(Sfx::Deny),
            }
        }
    }

    // The wheel runs the strip along, a tile a notch; the shelf keys follow what is in view.
    if grid.contains(ui.cursor - ui.shift) && ui.input.scroll != 0.0 && overflow {
        let was = hud.build_scroll;
        hud.build_scroll = snap(hud.build_scroll - ui.input.scroll.signum() * pitch);
        if hud.build_scroll != was {
            ui.audio.play(Sfx::Tick);
            hud.shelf = None;
        }
    }
    // Only while not dragged along the track, which moves smoothly under the pointer.
    if !ui.input.down {
        hud.build_scroll = snap(hud.build_scroll);
    }
    hud.build_shown += (hud.build_scroll - hud.build_shown) * (1.0 - (-16.0 * ui.dt).exp());
    if (hud.build_shown - hud.build_scroll).abs() < 0.3 {
        hud.build_shown = hud.build_scroll;
    }
    let shown = hud.build_shown;
    let current = current_shelf(hud, &slots, tile_w);
    let live = hud.build_keys;

    // The shelves, as buttons with their keys: a click runs the strip to the shelf.
    let mut bx = grid.x;
    for &(p, start, end, count) in &shelves {
        let label = p.label(!is_factory);
        let count_text = count.to_string();
        let w = 8.0 + 15.0 + 7.0
            + ui.text_width(type_scale::MICRO, label)
            + 10.0
            + ui.text_width(type_scale::MICRO, &count_text)
            + 9.0;
        if bx + w > grid.right() {
            break;
        }
        let br = Rect::new(bx, grid.y, w, SHELF_H);
        let lit = current == p;
        let t = hud.tile(ui, id("shelf", p as usize), br, lit, true);
        key_cap(ui, br.x + 8.0, br.y + 4.5, p.key(), live && start_of(p).is_some());
        ui.text(
            br.x + 30.0,
            br.mid_y(),
            type_scale::MICRO,
            rgb(if lit { 0xFFFFFF } else { palette::TEXT }, if lit { 1.0 } else { 0.7 + 0.3 * t.glow }),
            label,
        );
        ui.text_right(br.right() - 9.0, br.mid_y(), type_scale::MICRO, rgb(palette::FAINT, 1.0), &count_text);
        if overflow {
            // How much of the shelf is in view, as a bar under its button.
            let a = ((shown - start) / (end - start)).clamp(0.0, 1.0);
            let b = ((shown + view.w - start) / (end - start)).clamp(0.0, 1.0);
            if b > a {
                ui.fill(
                    Rect::new(br.x + br.w * a, br.bottom() + 3.0, br.w * (b - a), 2.0),
                    rgb(palette::TEXT, if lit { 0.9 } else { 0.45 }),
                );
            }
        }
        if t.clicked {
            ui.audio.play(Sfx::Tick);
            hud.shelf = Some(p);
            hud.build_scroll = snap(start);
        }
        bx += w + 4.0;
    }

    let mut hovered = None;
    for slot in &slots {
        let x = view.x + slot.at - shown;
        // Past the strip's edge a tile fades away; the part still in view can be
        // clicked, and brings the tile the rest of the way in.
        let over = (view.x - x).max(x + tile_w - view.right()).max(0.0);
        if over >= FADE {
            continue;
        }
        let item = slot.item;
        let tr = Rect::new(x, view.y, tile_w, STRIP_H);
        let (fade, interactive) = (ui.fade, ui.interactive);
        let k = 1.0 - over / FADE;
        ui.fade *= k * k;
        // Only inside the view, so the arrows keep the part under them.
        ui.interactive &= view.contains(ui.cursor - ui.shift);
        if let Some(i) = slot.climb {
            let t = climb_tile(hud, ui, offer, i, item, tr);
            if t.clicked && over > 0.5 {
                let to = if x < view.x { slot.at } else { slot.at + tile_w - view.w };
                hud.build_scroll = snap(to);
                hud.shelf = None;
            }
            if t.hovered {
                hovered = Some(Hover::Climb(i, tr));
            }
            (ui.fade, ui.interactive) = (fade, interactive);
            continue;
        }
        let locked = needs(item);
        if locked.is_some() {
            // Not until its tier is queued: dimmed, but its card still shows what it is.
            ui.fade *= 0.45;
        }
        let t = hud.tile(
            ui,
            id("build", item.id.0 as usize),
            tr,
            placing == Some(item.id),
            true,
        );
        unit_face(hud, ui, item, tr, t.glow);
        let name = shorten_name(ui, &item.name, tr.w - 8.0);
        ui.text_centred(
            tr.x + tr.w * 0.5,
            tr.bottom() - 24.0,
            NAME,
            rgb(palette::TEXT, 0.88 + 0.12 * t.glow),
            &name,
        );
        ui.text_centred(
            tr.x + tr.w * 0.5,
            tr.bottom() - 10.0,
            type_scale::MICRO,
            rgb(MASS, 0.95),
            &whole(item.cost_mass.to_f32()),
        );
        if slot.n < ITEM_KEYS.len() && slot.shelf == current {
            key_cap(ui, tr.right() - 19.0, tr.y + 4.0, ITEM_KEYS[slot.n], live);
        }
        if !offer.now.contains(&item.id) {
            // Opened by an upgrade: it waits in the queue until the new tier stands.
            icons::glyph(ui, icons::Glyph::Upgrade, Vec2::new(tr.x + 11.0, tr.bottom() - 44.0), 5.0, rgb(palette::TEXT, 0.9));
        }
        let queued: usize = stacks
            .iter()
            .filter(|k| k.blueprint == item.id && !k.upgrade)
            .map(|k| k.count)
            .sum();
        if queued > 0 {
            let badge = Rect::new(tr.right() - 25.0, tr.bottom() - 56.0, 21.0, 16.0);
            ui.fill(badge, rgb(palette::TEXT, 0.95));
            ui.text_centred(
                badge.x + badge.w * 0.5,
                badge.mid_y(),
                type_scale::MICRO,
                rgb(palette::INK, 1.0),
                &queued.to_string(),
            );
        }
        if t.clicked && locked.is_some() {
            ui.audio.play(Sfx::Deny);
        } else if t.clicked {
            ui.audio.play(Sfx::Select);
            hud.actions.push(HudAction::Build(item.id));
            if over > 0.5 {
                let to = if x < view.x { slot.at } else { slot.at + tile_w - view.w };
                hud.build_scroll = snap(to);
                hud.shelf = None;
            }
        }
        if t.right_clicked {
            if is_factory {
                ui.audio
                    .play(if queued > 0 { Sfx::Back } else { Sfx::Deny });
                hud.actions.push(HudAction::Cancel(item.id));
            } else if let Some(k) = stacks
                .iter()
                .rev()
                .find(|k| k.blueprint == item.id && !k.upgrade)
            {
                ui.audio.play(Sfx::Back);
                hud.actions.push(HudAction::CancelOrder {
                    kind: OrderKind::Build,
                    pos: k.last,
                });
            } else {
                ui.audio.play(Sfx::Deny);
            }
        }
        if t.hovered {
            hovered = Some(Hover::Unit(item, tr, locked));
        }
        (ui.fade, ui.interactive) = (fade, interactive);
    }
    // A rule between one shelf and the next, and after the upgrade.
    for &(_, start, ..) in shelves.iter().skip(if climbs.is_empty() { 1 } else { 0 }) {
        let lx = view.x + start - shown - shelf_gap * 0.5;
        if lx > view.x && lx < view.right() {
            ui.vline(lx, view.y + 10.0, STRIP_H - 20.0, rgb(palette::LINE, 0.22));
        }
    }

    if overflow {
        // Arrows at the ends, each counting the tiles out of sight its way and
        // nudging toward them.
        for (side, dir) in [(0usize, -1.0f32), (1, 1.0)] {
            let ar = Rect::new(
                if side == 0 { strip.x } else { strip.right() - ARROW_W },
                strip.y,
                ARROW_W,
                STRIP_H,
            );
            let hidden = slots
                .iter()
                .filter(|t| {
                    let x = view.x + t.at - shown;
                    if side == 0 {
                        x < view.x - 1.0
                    } else {
                        x + tile_w > view.right() + 1.0
                    }
                })
                .count();
            // Solid under the arrow, so a tile sliding off the strip goes behind it.
            ui.fill_cut(ar, 5.0, ink(0.94));
            let t = hud.tile(ui, id("strip-arrow", side), ar, false, hidden > 0);
            let nudge = if hidden > 0 { 2.5 * (ui.time * 3.4).sin().max(0.0) } else { 0.0 };
            let tone = rgb(palette::TEXT, if hidden > 0 { 0.75 + 0.25 * t.glow } else { 0.18 });
            let tip = Vec2::new(ar.x + ar.w * 0.5 + dir * (4.0 + nudge), ar.mid_y() - 6.0);
            icons::arrow_head(ui, tip, Vec2::X * dir, 6.0, 1.8, tone);
            if hidden > 0 {
                ui.text_centred(
                    ar.x + ar.w * 0.5,
                    ar.mid_y() + 14.0,
                    type_scale::MICRO,
                    rgb(palette::TEXT, 0.8),
                    &format!("+{hidden}"),
                );
            }
            if t.clicked {
                ui.audio.play(Sfx::Tick);
                hud.build_scroll = snap(hud.build_scroll + dir * page);
                hud.shelf = None;
            }
        }

        // The track under the strip: where the view is along it, dragged to scroll.
        let track = Rect::new(view.x, view.bottom() + 8.0, view.w, 3.0);
        let res = ui.interact_with(
            id("strip-track", 0),
            Rect::new(track.x, track.y - 5.0, track.w, 13.0),
            true,
            false,
        );
        let thumb_w = (track.w * view.w / length).max(28.0);
        if res.held {
            let f = (ui.cursor.x - ui.shift.x - track.x - thumb_w * 0.5) / (track.w - thumb_w);
            hud.build_scroll = f.clamp(0.0, 1.0) * max_scroll;
            hud.build_shown = hud.build_scroll;
            hud.shelf = None;
        }
        let along = if max_scroll > 0.0 { hud.build_shown / max_scroll } else { 0.0 };
        ui.fill(track, rgb(palette::LINE, 0.12 + 0.1 * res.glow));
        ui.fill(
            Rect::new(
                track.x + (track.w - thumb_w) * along,
                track.y - res.glow,
                thumb_w,
                track.h + 2.0 * res.glow,
            ),
            rgb(palette::TEXT, 0.7 + 0.3 * res.glow),
        );
    }
    hovered
}

/// The shelf the item keys pick from: the one chosen, or else the first in view.
fn current_shelf(hud: &Hud, slots: &[Slot], tile_w: f32) -> Purpose {
    hud.shelf
        .filter(|p| slots.iter().any(|t| t.shelf == *p))
        .or_else(|| {
            slots
                .iter()
                .find(|t| t.at + tile_w * 0.5 >= hud.build_scroll)
                .or(slots.last())
                .map(|t| t.shelf)
        })
        .unwrap_or(Purpose::Economy)
}

/// A unit's face on a tile: its domain's colour, its picture (or its strategic
/// icon when it has none), and the icon small in the corner.
pub fn unit_face(hud: &Hud, ui: &mut Ui, item: &UnitBlueprint, tr: Rect, glow: f32) {
    // Inside the tile's cut corners.
    let art = Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w - 6.0, tr.h - 36.0);
    // A darker well for the picture, lit from above where the model stands.
    ui.gradient_v(art, ink(0.5), ink(0.2));
    domain_wash(ui, art, Domain::of(item), glow);
    let side = art.h;
    let pic = Rect::new(art.x + (art.w - side) * 0.5, art.y, side, side);
    let pool = Rect::new(art.x + art.w * 0.5 - side * 0.62, art.y - side * 0.04, side * 1.24, side);
    hud.thumbs.stage(ui, pool, rgb(0xDCE6F0, 0.1 + 0.08 * glow));
    let drew = hud.thumbs.draw(ui, item.id, pic, 0.92 + 0.08 * glow);
    let icon_c = if drew {
        Vec2::new(art.x + 10.0, art.y + 10.0)
    } else {
        Vec2::new(art.x + art.w * 0.5, art.y + art.h * 0.5)
    };
    icons::strategic(
        ui,
        item.visual.icon,
        item.tech,
        icon_c,
        if drew { 6.5 } else { 12.0 },
        rgb(palette::TEXT, 0.85 + 0.15 * glow),
        ink(0.9),
    );
}

/// What `bp` will be once the tier upgrades in the unit's queue are done.
fn planned<'a>(s: &Scene<'a>, u: &UnitInstance, bp: &'a UnitBlueprint) -> &'a UnitBlueprint {
    let mut at = bp;
    for o in s.queue_of(u.unit_id).iter().flat_map(|q| &q.orders) {
        if o.kind == OrderKind::Upgrade && at.upgrades_to == Some(o.blueprint) {
            at = s.blueprints.unit(o.blueprint);
        }
    }
    at
}

/// The tiers `bp` upgrades through, one after another.
fn successors<'a>(s: &Scene<'a>, bp: &UnitBlueprint) -> Vec<&'a UnitBlueprint> {
    let blueprints = s.blueprints;
    let mut line: Vec<&'a UnitBlueprint> = Vec::new();
    let mut next = bp.upgrades_to;
    while let Some(id) = next {
        if line.iter().any(|b| b.id == id) {
            break;
        }
        let b = blueprints.unit(id);
        line.push(b);
        next = b.upgrades_to;
    }
    line
}

/// A tier upgrade on the strip, queued as a unit is: a click queues it (and the tiers
/// below it not yet queued), a right-click takes it and what relied on it back out.
fn climb_tile(hud: &mut Hud, ui: &mut Ui, offer: &Offer, i: usize, item: &UnitBlueprint, tr: Rect) -> super::Tile {
    let queued = i < offer.reached;
    // Past the side's tech: dimmed, and says what it waits for.
    let locked = offer.locked.filter(|&(from, _)| i >= from).map(|(_, tech)| tech);
    let fade = ui.fade;
    if locked.is_some() {
        ui.fade *= 0.45;
    }
    let t = hud.tile(ui, id("climb", item.id.0 as usize), tr, queued, true);
    unit_face(hud, ui, item, tr, t.glow);
    // What it is, across the top right where a unit has its key.
    let tone = rgb(0xFFFFFF, 0.8 + 0.2 * t.glow);
    let end = tr.right() - 7.0;
    let label = locked.map_or_else(|| "Upgrade".to_string(), |tech| format!("Needs T{tech}"));
    let w = ui.text_width(type_scale::MICRO, &label);
    ui.fill(Rect::new(end - w - 18.0, tr.y + 4.0, w + 22.0, 15.0), ink(0.8));
    icons::glyph(ui, icons::Glyph::Upgrade, Vec2::new(end - w - 9.0, tr.y + 11.5), 4.5, tone);
    ui.text_right(end, tr.y + 11.5, type_scale::MICRO, tone, &label);
    let name = shorten_name(ui, &item.name, tr.w - 8.0);
    ui.text_centred(tr.x + tr.w * 0.5, tr.bottom() - 24.0, NAME, rgb(palette::TEXT, 0.88 + 0.12 * t.glow), &name);
    ui.text_centred(
        tr.x + tr.w * 0.5,
        tr.bottom() - 10.0,
        type_scale::MICRO,
        rgb(MASS, 0.95),
        &whole(item.cost_mass.to_f32()),
    );
    if let Some((_, progress)) = offer.under_way.filter(|(b, _)| *b == item.id) {
        // Under way: its progress in the construction amber, along the foot of the picture.
        let track = Rect::new(tr.x + 6.0, tr.bottom() - 37.0, tr.w - 12.0, 3.0);
        ui.fill(track, rgb(BUILDING, 0.18));
        ui.fill(
            Rect::new(track.x, track.y, track.w * progress.clamp(0.0, 1.0), track.h),
            rgb(BUILDING, 0.95),
        );
    } else if queued {
        let badge = Rect::new(tr.right() - 25.0, tr.bottom() - 56.0, 21.0, 16.0);
        ui.fill(badge, rgb(palette::TEXT, 0.95));
        ui.text_centred(badge.x + badge.w * 0.5, badge.mid_y(), type_scale::MICRO, rgb(palette::INK, 1.0), "1");
    }
    ui.fade = fade;
    if t.clicked {
        if queued || locked.is_some() {
            ui.audio.play(Sfx::Deny);
        } else {
            ui.audio.play(Sfx::Select);
            // Each one queues the next tier after the last queued, up to this one.
            for _ in offer.reached..=i {
                hud.actions.push(HudAction::Upgrade);
            }
        }
    }
    if t.right_clicked {
        if queued {
            ui.audio.play(Sfx::Back);
            hud.actions.push(HudAction::CancelRefit(item.id));
        } else {
            ui.audio.play(Sfx::Deny);
        }
    }
    t
}

/// The card over a hovered tier upgrade: from and to, what changes, and the price.
#[allow(clippy::too_many_arguments)]
fn upgrade_card(
    hud: &Hud,
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    current: &UnitBlueprint,
    line: &[&UnitBlueprint],
    i: usize,
    reached: usize,
    tile: Rect,
    bottom: f32,
) {
    let next = line[i];
    let from = if i == 0 { current } else { line[i - 1] };
    // What changes.
    let mut rows: Vec<(&str, f32, f32, &str)> = vec![(
        "Integrity",
        from.health.to_f32(),
        next.health.to_f32(),
        "",
    )];
    if !next.weapons.is_empty() || !from.weapons.is_empty() {
        rows.push(("Damage / s", dps(from), dps(next), ""));
        rows.push((
            "Range",
            from.max_weapon_range().to_f32(),
            next.max_weapon_range().to_f32(),
            " m",
        ));
    }
    let power = |bp: &UnitBlueprint| bp.builder.as_ref().map_or(0.0, |b| b.power.to_f32());
    if power(from) + power(next) > 0.0 {
        rows.push(("Build Power", power(from), power(next), ""));
    }
    let e = (&from.economy, &next.economy);
    for (label, a, b) in [
        ("Materials Income", e.0.mass_income, e.1.mass_income),
        ("Energy Income", e.0.energy_income, e.1.energy_income),
        ("Energy Upkeep", e.0.energy_upkeep, e.1.energy_upkeep),
    ] {
        if a.to_f32() + b.to_f32() > 0.0 {
            rows.push((label, a.to_f32(), b.to_f32(), ""));
        }
    }
    // A core mine: what it makes on its own territory now and at the next tier.
    let mine = s.queue_of(u.unit_id).and_then(|q| q.mine).filter(|_| from.mine.is_some());
    if let Some(view) = mine {
        let was = if from.id == current.id {
            view.rate
        } else {
            super::mine::rate_on(from, &view.land)
        };
        rows.insert(0, ("Materials / s", was, super::mine::rate_on(next, &view.land), ""));
    }
    rows.push(("Vision", from.vision.to_f32(), next.vision.to_f32(), " m"));
    let payback = mine
        .filter(|_| from.id == current.id)
        .and_then(|v| super::mine::upgrade_gain(s.blueprints, from, &v));
    let rows: Vec<_> = rows.into_iter().take(8).collect();

    let w = 380.0;
    let row_h = 19.0;
    let h = 112.0 + if payback.is_some() { 24.0 } else { 0.0 } + rows.len() as f32 * row_h + 40.0;
    let r = Rect::new(
        (tile.x + tile.w * 0.5 - w * 0.5).clamp(14.0, ui.size.x - w - 14.0),
        (bottom - h).max(14.0),
        w,
        h,
    );
    ui.panel(r);
    let (x, cw) = (r.x + 18.0, w - 36.0);
    hud.thumbs.draw(ui, next.id, Rect::new(r.right() - 76.0, r.y + 8.0, 64.0, 64.0), 1.0);
    ui.text(x, r.y + 26.0, type_scale::ITEM, rgb(0xFFFFFF, 1.0), &next.name);
    ui.text(
        x,
        r.y + 48.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        &format!("Upgrade  \u{b7}  Tech {} to {}  \u{b7}  {}", from.tech, next.tech, next.role),
    );

    // The price: mass, energy, and how long it takes to put on.
    let power = from.builder.as_ref().map_or(SELF_UPGRADE_POWER, |b| b.power.to_f32());
    let costs = [
        ("Materials", whole(next.cost_mass.to_f32()), MASS),
        ("Energy", whole(next.cost_energy.to_f32()), ENERGY),
        ("Time", super::clock(next.build_time.to_f32() / power.max(0.1)), palette::TEXT),
    ];
    for (k, (label, value, tone)) in costs.iter().enumerate() {
        let cx = x + k as f32 * (cw - 70.0) / 3.0;
        ui.fill(Rect::new(cx, r.y + 66.0, 2.0, 28.0), rgb(*tone, 0.9));
        ui.text(cx + 10.0, r.y + 72.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
        ui.text(cx + 10.0, r.y + 88.0, type_scale::VALUE, rgb(*tone, 1.0), value);
    }
    ui.hline(x, r.y + 104.0, cw, rgb(palette::LINE, 0.16));
    let mut y = r.y + 120.0;
    // A mine's upgrade pays for itself out of what it adds.
    if let Some((gain, payback)) = payback {
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Pays Back In");
        ui.text_right(
            x + cw,
            y,
            type_scale::VALUE,
            rgb(MASS, 1.0),
            &format!("{}  \u{b7}  +{gain:.1}/s", super::mine::duration(payback)),
        );
        y += 24.0;
    }
    // Each figure as it is, and as it will be.
    for (label, was, will, unit) in &rows {
        let diff = will - was;
        let tone = if diff.abs() < 0.05 {
            palette::TEXT
        } else if diff > 0.0 {
            0x78E08A
        } else {
            palette::BAD
        };
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
        let will_text = format!("{}{unit}", whole(*will));
        let ww = ui.text_width(type_scale::VALUE, &will_text);
        ui.text_right(x + cw, y, type_scale::VALUE, rgb(tone, 1.0), &will_text);
        let ax = x + cw - ww - 14.0;
        icons::arrow_head(ui, Vec2::new(ax + 4.0, y), Vec2::X, 3.5, 1.2, rgb(palette::FAINT, 1.0));
        ui.text_right(ax - 4.0, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &format!("{}{unit}", whole(*was)));
        y += row_h;
    }
    let hint = if i < reached {
        "Queued  \u{b7}  Right-Click Cancels It and What Waits on It".to_owned()
    } else if i > reached {
        format!("Click Queues {} First, Then This", line[reached].name)
    } else {
        "Click Queues  \u{b7}  Right-Click Cancels".to_owned()
    };
    ui.text(x, r.bottom() - 16.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), &hint);
}

/// A name on one line, cut short with a full stop when it does not fit.
fn shorten_name(ui: &mut Ui, text: &str, width: f32) -> String {
    if ui.text_width(NAME, text) <= width {
        return text.to_owned();
    }
    let mut cut = text.to_owned();
    while cut.pop().is_some() {
        let candidate = format!("{}.", cut.trim_end());
        if ui.text_width(NAME, &candidate) <= width {
            return candidate;
        }
    }
    String::new()
}

/// Cuts a caption short, with a full stop for the missing part, until it fits.
fn shorten(ui: &mut Ui, text: &str, width: f32) -> String {
    if ui.text_width(type_scale::MICRO, text) <= width {
        return text.to_owned();
    }
    let mut cut = text.to_owned();
    while cut.pop().is_some() {
        let candidate = format!("{}.", cut.trim_end());
        if ui.text_width(type_scale::MICRO, &candidate) <= width {
            return candidate;
        }
    }
    String::new()
}

fn draw_queue(hud: &mut Hud, ui: &mut Ui, s: &Scene, r: Rect, queue: &Queue) {
    let Queue {
        stacks,
        progress,
        is_factory,
        repeating,
        paused,
    } = *queue;
    hud.glass(ui, r);
    let label = if is_factory {
        "Production Queue"
    } else {
        "Build Queue"
    };
    ui.fill(
        Rect::new(r.x + 14.0, r.mid_y() - 5.0, 3.0, 10.0),
        rgb(palette::TEXT, 0.9),
    );
    let end = ui.text(
        r.x + 26.0,
        r.mid_y() - 8.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        label,
    );
    let total: usize = stacks.iter().map(|k| k.count).sum();
    let note_end = if paused {
        ui.text(
            r.x + 26.0,
            r.mid_y() + 9.0,
            type_scale::MICRO,
            rgb(BUILDING, 1.0),
            &format!("Paused  \u{b7}  {total} waiting  \u{b7}  Z resumes"),
        )
    } else {
        ui.text(
            r.x + 26.0,
            r.mid_y() + 9.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("{total} queued  \u{b7}  right-click removes"),
        )
    };
    let end = end.max(note_end);

    // Repeat, for a factory: build the queue over and over.
    let mut right = r.right() - 12.0;
    if is_factory {
        let tr = Rect::new(right - 96.0, r.y + (r.h - 34.0) * 0.5, 96.0, 34.0);
        let t = hud.tile(ui, id("queue-repeat", 0), tr, repeating, true);
        icons::glyph(
            ui,
            icons::Glyph::Repeat,
            Vec2::new(tr.x + 16.0, tr.mid_y()),
            7.0,
            rgb(if repeating { palette::TEXT } else { palette::TEXT }, 0.8 + 0.2 * t.glow),
        );
        ui.text(
            tr.x + 30.0,
            tr.mid_y(),
            type_scale::MICRO,
            rgb(if repeating { palette::TEXT } else { palette::DIM }, 1.0),
            "Repeat",
        );
        ui.text_right(tr.right() - 5.0, tr.y + 8.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), "L");
        if t.clicked {
            ui.audio.play(Sfx::Select);
            hud.actions.push(HudAction::Repeat(!repeating));
        }
        if t.hovered {
            tip(ui, tr.x, r.y - 32.0, "The factory builds its queue over and over.");
        }
        right = tr.x - 10.0;
    }
    // Pause, for any builder: the queue stays, the spending stops.
    {
        let tr = Rect::new(right - 96.0, r.y + (r.h - 34.0) * 0.5, 96.0, 34.0);
        let t = hud.tile(ui, id("queue-pause", 0), tr, paused, true);
        let tone = if paused { BUILDING } else { palette::TEXT };
        icons::glyph(
            ui,
            if paused { icons::Glyph::Play } else { icons::Glyph::Pause },
            Vec2::new(tr.x + 16.0, tr.mid_y()),
            7.0,
            rgb(tone, 0.8 + 0.2 * t.glow),
        );
        ui.text(
            tr.x + 30.0,
            tr.mid_y(),
            type_scale::MICRO,
            rgb(if paused { BUILDING } else { palette::DIM }, 1.0),
            if paused { "Resume" } else { "Pause" },
        );
        ui.text_right(tr.right() - 5.0, tr.y + 8.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), "Z");
        if t.clicked {
            ui.audio.play(Sfx::Select);
            hud.actions.push(HudAction::PauseWork(!paused));
        }
        if t.hovered {
            tip(
                ui,
                tr.x,
                r.y - 32.0,
                if paused {
                    "Carry on from where it stopped."
                } else {
                    "Keep the queue but spend nothing. Helpers wait too."
                },
            );
        }
        right = tr.x - 10.0;
    }

    let (w, h, gap) = (48.0, 46.0, 5.0);
    let mut x = end + 18.0;
    let room = (((right - 40.0 - x) / (w + gap)).max(0.0) as usize).max(1);
    for (i, k) in stacks.iter().take(room).enumerate() {
        let item = s.blueprints.unit(k.blueprint);
        let tr = Rect::new(x, r.y + (r.h - h) * 0.5, w, h);
        let t = hud.tile(ui, id("queue", i), tr, false, true);
        // What is being built is lit in the construction amber alone; the rest wait in their domain's colour.
        if i == 0 && paused {
            held(ui, tr, progress);
        } else if i == 0 {
            building(ui, tr, progress);
        } else {
            domain_wash(ui, Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w - 6.0, tr.h - 6.0), Domain::of(item), t.glow * 0.5);
        }
        if !hud.thumbs.draw(ui, item.id, Rect::new(tr.x + 3.0, tr.y + 2.0, 36.0, 36.0), 1.0) {
            icons::strategic(
                ui,
                item.visual.icon,
                item.tech,
                Vec2::new(tr.x + 18.0, tr.y + 18.0),
                9.5,
                rgb(palette::TEXT, 0.8 + 0.2 * t.glow),
                ink(0.9),
            );
        }
        if i == 0 && paused {
            pause_mark(ui, Vec2::new(tr.x + 18.0, tr.y + 18.0), 18.0);

        }
        let refit_name = super::refit::queued_name(s.blueprints, k.blueprint);
        if let Some(name) = refit_name {
            // A refit: the module's name across the foot of the tile.

            ui.fill(Rect::new(tr.x + 3.0, tr.bottom() - 17.0, tr.w - 6.0, 13.0), ink(0.75));
            ui.text_fit(tr.x + tr.w * 0.5, tr.bottom() - 10.5, tr.w - 8.0, type_scale::MICRO, rgb(0xFFFFFF, 1.0), name);
        }
        let count = if k.upgrade {
            "UP".to_owned()
        } else {
            format!("{}", k.count)
        };
        ui.text_right(
            tr.right() - 5.0,
            tr.y + 12.0,
            type_scale::VALUE,
            rgb(0xFFFFFF, 1.0),
            &count,
        );
        if i == 0 && paused {
            // Where it stopped, still: no glint while nothing is spent.
            let track = Rect::new(tr.x + 5.0, tr.bottom() - 8.0, tr.w - 10.0, 3.0);
            ui.fill(track, rgb(BUILDING, 0.18));
            ui.fill(Rect::new(track.x, track.y, track.w * progress.clamp(0.0, 1.0), track.h), rgb(BUILDING, 0.55));
        } else if i == 0 {
            // Its progress, in the construction amber, with a glint running along it.
            let track = Rect::new(tr.x + 5.0, tr.bottom() - 8.0, tr.w - 10.0, 3.0);
            ui.fill(track, rgb(BUILDING, 0.18));
            let done = track.w * progress.clamp(0.0, 1.0);
            ui.gradient_h(
                Rect::new(track.x, track.y, done, track.h),
                rgb(BUILDING, 0.7),
                rgb(0xFFE3A0, 1.0),
            );
            let glint = (ui.time * 0.8).fract();
            let gx = track.x + done * glint;
            ui.gradient_h(
                Rect::new((gx - 10.0).max(track.x), track.y - 1.0, (gx - track.x).min(10.0), track.h + 2.0),
                rgb(0xFFFFFF, 0.0),
                rgb(0xFFFFFF, 0.8),
            );
        }
        if k.upgrade {
            if t.right_clicked {
                ui.audio.play(Sfx::Back);
                // A tier is cancelled by its own blueprint too, with the tiers after it.
                hud.actions.push(HudAction::CancelRefit(k.blueprint));
            }
        } else if is_factory {
            if t.clicked {
                ui.audio.play(Sfx::Select);
                hud.actions.push(HudAction::Build(k.blueprint));
            }
            if t.right_clicked {
                ui.audio.play(Sfx::Back);
                hud.actions.push(HudAction::Cancel(k.blueprint));
            }
        } else if t.right_clicked {
            // An engineer's site: the last one of the stack comes out.
            ui.audio.play(Sfx::Back);
            hud.actions.push(HudAction::CancelOrder {
                kind: OrderKind::Build,
                pos: k.last,
            });
        }
        if t.hovered {
            let hint = if let Some(hint) = super::refit::queue_hint(s.blueprints, k.blueprint) {
                hint
            } else if k.upgrade {
                format!(
                    "Upgrade to {}  \u{b7}  Right-Click Cancels",
                    item.name
                )
            } else if is_factory {
                format!(
                    "{}  \u{b7}  Click Adds  \u{b7}  Right-Click Removes",
                    item.name
                )
            } else {
                format!("{}  \u{b7}  Right-Click Removes the Last", item.name)
            };
            tip(ui, tr.x, r.y - 32.0, &hint);
        }
        x += w + gap;
    }
    if stacks.len() > room {
        ui.text(
            x + 4.0,
            r.mid_y(),
            type_scale::VALUE,
            rgb(palette::DIM, 1.0),
            &format!("+{}", stacks.len() - room),
        );
    }
}

/// The construction amber: what is being built right now.
const BUILDING: u32 = 0xFFA928;

/// The item being built: an amber frame breathing around it, amber light
/// welling up from the foot, and a sheen sweeping across it.
fn building(ui: &mut Ui, tr: Rect, progress: f32) {
    let breathe = 0.55 + 0.45 * (ui.time * 3.2).sin().abs();
    let inner = Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w - 6.0, tr.h - 6.0);
    ui.gradient_v(
        inner,
        rgb(BUILDING, 0.08),
        rgb(BUILDING, 0.22 + 0.2 * progress.clamp(0.0, 1.0) * breathe),
    );
    let sweep = (ui.time * 0.55).fract();
    let sx = inner.x - 14.0 + (inner.w + 28.0) * sweep;
    let (x0, x1) = (sx.max(inner.x), (sx + 14.0).min(inner.right()));
    if x1 > x0 {
        ui.gradient_h(
            Rect::new(x0, inner.y, x1 - x0, inner.h),
            rgb(0xFFD58A, 0.0),
            rgb(0xFFD58A, 0.16),
        );
    }
    ui.outline_cut(tr, 5.0, rgb(BUILDING, 0.9 * breathe), rgb(BUILDING, breathe));
}

/// The front of a paused queue: the construction amber held still and dimmed, with a
/// pause mark over the picture.
fn held(ui: &mut Ui, tr: Rect, progress: f32) {
    let inner = Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w - 6.0, tr.h - 6.0);
    ui.gradient_v(
        inner,
        rgb(BUILDING, 0.04),
        rgb(BUILDING, 0.10 + 0.08 * progress.clamp(0.0, 1.0)),
    );
    ui.outline_cut(tr, 5.0, rgb(BUILDING, 0.45), rgb(BUILDING, 0.6));
}

/// The pause mark over a tile's picture, drawn after it: two amber bars on a dark plate
/// `size` across, centred on `c`. The strategic icon in the world carries the same mark.
pub fn pause_mark(ui: &mut Ui, c: Vec2, size: f32) {
    let h = size * 0.5;
    ui.fill(Rect::new(c.x - h, c.y - h, size, size), ink(0.7));
    let (bar, tall) = (size * 0.17, size * 0.6);
    for dx in [-bar * 1.5, bar * 0.5] {
        ui.fill(Rect::new(c.x + dx, c.y - tall * 0.5, bar, tall), rgb(BUILDING, 1.0));
    }
}

/// A one-line hint over something, kept on screen.

pub fn tip(ui: &mut Ui, x: f32, y: f32, text: &str) {
    let tw = ui.text_width(type_scale::MICRO, text) + 24.0;
    let r = Rect::new(x.min(ui.size.x - tw - 14.0).max(14.0), y, tw, 26.0);
    ui.frost(r, 0.8);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(palette::TEXT, 1.0));
    ui.text(
        r.x + 12.0,
        r.mid_y(),
        type_scale::MICRO,
        rgb(palette::TEXT, 0.95),
        text,
    );
}

/// Everything about a blueprint, over the tile the pointer is on.
fn data_card(
    hud: &Hud,
    ui: &mut Ui,
    item: &UnitBlueprint,
    build_power: f32,
    hint: &str,
    tile: Rect,
    bottom: f32,
) {
    let mut rows: Vec<(String, String, u32)> = vec![(
        "Integrity".into(),
        whole(item.health.to_f32()),
        palette::TEXT,
    )];
    if let Some(m) = &item.motion {
        rows.push((
            "Speed".into(),
            format!("{:.0} m/s", m.speed.to_f32()),
            palette::TEXT,
        ));
    }
    let e = &item.economy;
    for (label, v, tone, sign) in [
        ("Materials Income", e.mass_income, MASS, "+"),
        ("Energy Income", e.energy_income, ENERGY, "+"),
        ("Energy Upkeep", e.energy_upkeep, ENERGY, "-"),
        ("Materials Storage", e.mass_storage, MASS, "+"),
        ("Energy Storage", e.energy_storage, ENERGY, "+"),
    ] {
        if v.to_f32() > 0.0 {
            rows.push((label.into(), format!("{sign}{}", whole(v.to_f32())), tone));
        }
    }
    if let Some(r) = &item.reclaimer {
        let charge = if r.charge_ticks > 0 {
            format!(
                "  \u{b7}  {:.1} S Charge",
                r.charge_ticks as f32 / TICKS_PER_SECOND as f32
            )
        } else {
            String::new()
        };
        rows.push((
            "Reclaim Beam".into(),
            format!(
                "{:.0} / S  \u{b7}  {:.0} M{charge}",
                r.power.to_f32(),
                r.range.to_f32()
            ),
            MASS,
        ));
    }
    if let Some(a) = &item.airbase {
        rows.push((
            "Hangar".into(),
            format!(
                "{} Aircraft  \u{b7}  {} Tunnels",
                a.capacity,
                a.tunnels.len()
            ),
            super::style::AIR,
        ));
        rows.push((
            "Reach".into(),
            format!("{:.0} M  \u{b7}  Mends {:.0}% / S", a.reach.to_f32(), a.heal.to_f32() * 100.0),
            super::style::AIR,
        ));
    }
    if let Some(b) = &item.builder {
        rows.push((
            "Build Power".into(),
            format!("{:.0}", b.power.to_f32()),
            palette::TEXT,
        ));
    }
    if let Some(sh) = item.shield {
        let line = if sh.is_hull() {
            format!(
                "{}  \u{b7}  +{:.0}/S",
                whole(sh.health.to_f32()),
                sh.regen.to_f32()
            )
        } else {
            format!(
                "{}  \u{b7}  {:.0} M  \u{b7}  +{:.0}/S",
                whole(sh.health.to_f32()),
                sh.radius.to_f32(),
                sh.regen.to_f32()
            )
        };
        rows.push(("Shield".into(), line, palette::TEXT));
    }
    if item.radar.to_f32() > 0.0 {
        rows.push((
            "Radar".into(),
            format!("{:.0} m", item.radar.to_f32()),
            palette::TEXT,
        ));
    }
    if item.sonar.to_f32() > 0.0 {
        rows.push(("Sonar".into(), format!("{:.0} m", item.sonar.to_f32()), palette::TEXT));
    }

    let w = 380.0;
    let (x, cw) = (18.0, w - 36.0);
    // Lore, wrapped to the card.
    let lore = super::selection::wrap_text(ui, type_scale::BODY, &item.lore, cw);
    let weapons: Vec<_> = item.weapons.iter().map(|wp| weapon_rows(wp)).collect();
    let row_h = 19.0;
    let weapons_h: f32 = weapons.iter().map(|w| 22.0 + (w.1.len().div_ceil(2)) as f32 * 17.0 + 6.0).sum();
    let h = 112.0
        + if lore.is_empty() { 0.0 } else { lore.len() as f32 * 19.0 + 10.0 }
        + rows.len() as f32 * row_h
        + weapons_h
        + 34.0;
    let r = Rect::new(
        (tile.x + tile.w * 0.5 - w * 0.5).clamp(14.0, ui.size.x - w - 14.0),
        (bottom - h).max(14.0),
        w,
        h,
    );
    ui.panel(r);
    let x = r.x + x;
    let pic = Rect::new(r.right() - 76.0, r.y + 8.0, 64.0, 64.0);
    hud.thumbs.draw(ui, item.id, pic, 1.0);
    ui.text(
        x,
        r.y + 26.0,
        type_scale::ITEM,
        rgb(0xFFFFFF, 1.0),
        &item.name,
    );
    let end = ui.text(
        x,
        r.y + 48.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        &format!(
            "Tech {}  \u{b7}  {}  \u{b7}  {}",
            item.tech,
            item.role,
            Domain::of(item).label()
        ),
    );
    super::volatile::chip(ui, item, end + 8.0, r.y + 48.0);

    // The price: mass, energy, and how long this builder takes over it.
    let seconds = item.build_time.to_f32() / build_power.max(0.1);
    let costs = [
        ("Materials", whole(item.cost_mass.to_f32()), MASS),
        ("Energy", whole(item.cost_energy.to_f32()), ENERGY),
        ("Time", super::clock(seconds), palette::TEXT),
    ];
    for (i, (label, value, tone)) in costs.iter().enumerate() {
        let cx = x + i as f32 * (cw - 70.0) / 3.0;
        ui.fill(Rect::new(cx, r.y + 66.0, 2.0, 28.0), rgb(*tone, 0.9));
        ui.text(
            cx + 10.0,
            r.y + 72.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            label,
        );
        ui.text(
            cx + 10.0,
            r.y + 88.0,
            type_scale::VALUE,
            rgb(*tone, 1.0),
            value,
        );
    }
    ui.hline(x, r.y + 104.0, cw, rgb(palette::LINE, 0.16));
    let mut y = r.y + 120.0;
    for line in &lore {
        ui.text(x, y, type_scale::BODY, rgb(palette::DIM, 1.0), line);
        y += 19.0;
    }
    if !lore.is_empty() {
        y += 10.0;
    }
    for (label, value, tone) in &rows {
        // A long label gives way to its figures, never runs into them.
        let room = cw - ui.text_width(type_scale::VALUE, value) - 14.0;
        let label = shorten(ui, label, room);
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &label);
        ui.text_right(x + cw, y, type_scale::VALUE, rgb(*tone, 1.0), value);
        y += row_h;
    }
    for (name, figures) in &weapons {
        y += 4.0;
        ui.fill(Rect::new(x, y - 5.0, 2.0, 10.0), rgb(super::style::Family::Combat.tone(), 1.0));
        ui.text(x + 8.0, y, type_scale::CAPTION, rgb(palette::TEXT, 1.0), name);
        y += 18.0;
        y = super::selection::figures_grid(ui, figures, x, y, cw);
        y += 6.0;
    }
    ui.text(
        x,
        r.bottom() - 16.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        hint,
    );
}

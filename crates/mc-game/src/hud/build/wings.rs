//! A carrier's air wings (`mc_sim::air_wings`) in the construction panel's place, drawn
//! as a factory's is: a tile a wing (its aircraft's picture, the wing's name, how many of
//! what, the price of all of it, a count while queued, its item key), the queue strip with
//! Repeat, Pause and Priority over it (`queue.rs`), and the build card for a hovered wing.
//! No tiers, and no Batch: a carrier's wings are all it builds, and they form up round it.

use super::queue::{self, Maker, Queue, Split};
use super::{
    data_card, key_cap, keys_hint, queued_badge, strip, tile_caption, unit_face, Pace, Stack,
    PANEL_H, QUEUE_H, SHELF_H, STRIP_DROP, STRIP_H, TILE_GAP, TILE_W,
};
use crate::audio::Sfx;
use crate::hud::priority::{self, Mix};
use crate::hud::style::ITEM_KEYS;
use crate::hud::{has_flag, Hud, HudAction, Scene, GAP};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use mc_core::FxVec2;
use mc_data::AirWings;
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::{flag, OrderKind};

/// The first of the selection that builds air wings and is the viewer's own.
pub(super) fn carrier_of<'a>(
    s: &'a Scene,
    units: &[&'a UnitInstance],
) -> Option<(&'a UnitInstance, &'a AirWings)> {
    units.iter().find_map(|u| {
        let own = (u.owner_flags & 0xFF) as u8 == s.view.local;
        Some(*u).zip(s.bp(u).air_wings.as_ref().filter(|_| own))
    })
}

/// The panel for `carrier` in `r`, the construction panel's room, with its queue strip
/// over it.
pub(super) fn draw(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    units: &[&UnitInstance],
    carrier: &UnitInstance,
    wings: &AirWings,
    r: Rect,
) {
    let r = Rect::new(r.x, r.bottom() - PANEL_H.min(r.h), r.w, PANEL_H.min(r.h));
    let view = s
        .view
        .frame
        .wings
        .iter()
        .find(|w| w.unit_id == carrier.unit_id);
    let queued = view.map_or(&[][..], |v| &v.queue[..]);
    // Wings of a kind one after another, as one stack, like a factory's products.
    let mut stacks: Vec<Stack> = Vec::new();
    for &w in queued {
        let Some(wing) = wings.wings.get(w as usize) else {
            continue;
        };
        match stacks.last_mut() {
            Some(top) if top.wing == Some(w) => top.count += 1,
            _ => stacks.push(Stack {
                blueprint: wing.aircraft,
                count: 1,
                upgrade: false,
                last: FxVec2::ZERO,
                wing: Some(w),
            }),
        }
    }

    hud.glass(ui, r);
    let (x, cw) = (r.x + 14.0, r.w - 28.0);
    ui.section(
        x,
        r.y + 23.0,
        cw,
        &format!("Air Wings  \u{b7}  {}", s.bp(carrier).name),
    );
    keys_hint(hud, ui, r, "Wing  \u{b7}  ESC");
    // An item key queues its wing; there are no tiers or shelves to pick.
    if let Some(key) = hud.build_key.take() {
        match ITEM_KEYS.iter().position(|k| *k == key) {
            Some(n) if n < wings.wings.len() => {
                ui.audio.play(Sfx::Select);
                hud.actions.push(HudAction::QueueWing(n as u8));
            }
            _ => ui.audio.play(Sfx::Deny),
        }
    }

    // One shelf, as a factory's strip has its shelves over it.
    let grid = Rect::new(x, r.y + 44.0, cw, STRIP_DROP + STRIP_H);
    let n = wings.wings.len();
    let label = "Wings";
    let count = n.to_string();
    let w = 8.0
        + ui.text_width(type_scale::MICRO, label)
        + 10.0
        + ui.text_width(type_scale::MICRO, &count)
        + 9.0;
    let br = Rect::new(grid.x, grid.y, w, SHELF_H);
    hud.tile(ui, id("wing-shelf", 0), br, true, false);
    ui.text(
        br.x + 8.0,
        br.mid_y(),
        type_scale::MICRO,
        rgb(0xFFFFFF, 1.0),
        label,
    );
    ui.text_right(
        br.right() - 9.0,
        br.mid_y(),
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &count,
    );

    // The tiles, at a factory's size while they fit, closed up when they do not.
    let view_r = Rect::new(grid.x, grid.y + STRIP_DROP, grid.w, STRIP_H);
    let tile_w = TILE_W.min((view_r.w + TILE_GAP) / n.max(1) as f32 - TILE_GAP);
    strip::bracket(ui, view_r, 0.0, n as f32 * (tile_w + TILE_GAP) - TILE_GAP);
    let live = hud.build_keys;
    let mut hovered = None;
    for (i, wing) in wings.wings.iter().enumerate() {
        let tr = Rect::new(
            view_r.x + i as f32 * (tile_w + TILE_GAP),
            view_r.y,
            tile_w,
            STRIP_H,
        );
        let aircraft = s.blueprints.unit(wing.aircraft);
        let t = hud.tile(ui, id("wing", i), tr, false, true);
        unit_face(hud, ui, aircraft, tr, t.glow);
        let what = format!("{} \u{d7} {}", wing.count, aircraft.name);
        let price = aircraft.cost_mass.to_f32() * wing.count as f32;
        tile_caption(ui, &wing.name, Some(&what), tr, t.glow, price);
        if let Some(&key) = ITEM_KEYS.get(i) {
            key_cap(ui, tr.right() - 19.0, tr.y + 4.0, key, live);
        }
        let waiting = queued.iter().filter(|&&w| w == i as u8).count();
        if waiting > 0 {
            queued_badge(ui, tr, &waiting.to_string());
        }
        if t.clicked {
            ui.audio.play(Sfx::Select);
            hud.actions.push(HudAction::QueueWing(i as u8));
        }
        if t.right_clicked {
            ui.audio
                .play(if waiting > 0 { Sfx::Back } else { Sfx::Deny });
            hud.actions.push(HudAction::CancelWing(i as u8));
        }
        if t.hovered {
            hovered = Some((aircraft, wing, tr));
        }
    }

    // The strip: always up, as a factory's is, since Repeat lives there.
    let launching = view.is_some_and(|v| v.launching > 0);
    let strip = Queue {
        stacks: &stacks,
        progress: view.map_or(0.0, |v| v.progress),
        eta: None,
        platform: launching.then_some("Launching the Last Wing"),
        maker: Maker::Carrier,
        wings: &wings.wings,
        repeat: Split::count(
            units
                .iter()
                .filter(|u| s.bp(u).air_wings.is_some())
                .map(|u| has_flag(u, flag::REPEAT)),
        ),
        batch: Split::default(),
        muster: None,
        pause: Split::count(
            units
                .iter()
                .filter(|u| mc_sim::pause::pausable(s.blueprints, s.bp(u)))
                .map(|u| u.paused()),
        ),
        paused: carrier.paused(),
        priority: Mix::of(s, units),
        own_priority: carrier.priority(),
        inherit: priority::inherited(s, carrier),
        front: queued.first().map(|_| OrderKind::Build),
        unit: carrier.unit_id,
        panel: r,
    };
    let queue_rect = Rect::new(r.x, r.y - GAP - QUEUE_H, r.w, QUEUE_H);
    let floor = queue::draw(hud, ui, s, queue_rect, &strip);
    if let Some((aircraft, wing, tile)) = hovered {
        let pace = Pace::Wing {
            power: wings.power.to_f32(),
            count: wing.count as u32,
            name: &wing.name,
        };
        let hint = "Click +1  \u{b7}  Shift +5  \u{b7}  Right-Click -1";
        data_card(hud, ui, s, aircraft, pace, hint, tile, floor - GAP);
    }
}

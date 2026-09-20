//! What is selected, and what it can be told to do: the selection panel and the order card.

use super::icons::{self, Glyph};
use super::{has_flag, whole, Hud, HudAction, Scene, ENERGY, MASS};
use crate::audio::Sfx;
use crate::game::{Mode, Targeting};
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::{cat, UnitBlueprint};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::{flag, OrderKind};
use mc_sim::{veterancy_health, VETERANCY_MAX};

pub const ORDER_W: f32 = 78.0;
pub const ORDER_H: f32 = 52.0;
pub const ORDER_GAP: f32 = 6.0;

/// Damage per second of everything a unit carries.
pub fn dps(bp: &UnitBlueprint) -> f32 {
    bp.weapons
        .iter()
        .map(|w| w.damage.to_f32() * w.salvo.max(1) as f32 / (w.reload_ticks.max(1) as f32 * 0.1))
        .sum()
}

fn health_tone(share: f32) -> u32 {
    if share > 0.6 {
        MASS
    } else if share > 0.3 {
        palette::WARN
    } else {
        palette::BAD
    }
}

fn bar(ui: &mut Ui, r: Rect, share: f32, tone: u32) {
    ui.fill(r, rgb(palette::LINE, 0.13));
    ui.gradient_h(
        Rect::new(r.x, r.y, r.w * share.clamp(0.0, 1.0), r.h),
        rgb(tone, 0.6),
        rgb(tone, 1.0),
    );
}

const RANKS: [&str; 6] = [
    "RECRUIT",
    "VETERAN",
    "HARDENED",
    "ELITE",
    "HEROIC",
    "LEGENDARY",
];

fn chevrons(ui: &mut Ui, origin: Vec2, level: u8) {
    for i in 0..VETERANCY_MAX {
        let c = origin + Vec2::new(i as f32 * 11.0, 0.0);
        let r = 5.0;
        let tip = c + Vec2::new(0.0, -r);
        let left = c + Vec2::new(-r * 0.85, r * 0.5);
        let right = c + Vec2::new(r * 0.85, r * 0.5);
        if i < level {
            ui.triangle(tip, left, right, rgb(palette::ACCENT, 1.0));
        } else {
            ui.stroke(left, tip, 1.2, rgb(palette::LINE, 0.7));
            ui.stroke(tip, right, 1.2, rgb(palette::LINE, 0.7));
        }
    }
}

fn activity(kind: OrderKind) -> &'static str {
    match kind {
        OrderKind::Move => "MOVING",
        OrderKind::AttackMove => "ATTACK-MOVING",
        OrderKind::Attack => "ATTACKING",
        OrderKind::Build => "BUILDING",
        OrderKind::Assist => "ASSISTING",
        OrderKind::Reclaim | OrderKind::ReclaimUnit => "RECLAIMING",
        OrderKind::Produce => "PRODUCING",
        OrderKind::Upgrade => "UPGRADING",
    }
}

pub fn info(hud: &mut Hud, ui: &mut Ui, s: &Scene, units: &[&UnitInstance], r: Rect) {
    hud.glass(ui, r);
    let (x, cw) = (r.x + 16.0, r.w - 32.0);
    if let [u] = units {
        single(ui, s, u, r);
        return;
    }

    // Many units: a tile per type, most numerous first.
    let mut types: Vec<(u32, Vec<u32>)> = Vec::new();
    for u in units {
        match types.iter_mut().find(|(bp, _)| *bp == u.blueprint) {
            Some((_, ids)) => ids.push(u.unit_id),
            None => types.push((u.blueprint, vec![u.unit_id])),
        }
    }
    types.sort_by_key(|(bp, ids)| (std::cmp::Reverse(ids.len()), *bp));
    ui.section(
        x,
        r.y + 20.0,
        cw,
        &format!("{} UNITS SELECTED", units.len()),
    );
    let kills: u32 = units.iter().map(|u| u.kill_count()).sum();
    ui.text_right(
        x + cw,
        r.y + 20.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        &format!("{kills} KILL{}", if kills == 1 { "" } else { "S" }),
    );
    let health: f32 = units.iter().map(|u| u.health).sum::<f32>() / units.len() as f32;
    bar(
        ui,
        Rect::new(x, r.y + 36.0, cw, 4.0),
        health,
        health_tone(health),
    );

    let (tw, th, gap, per_row) = (56.0, 66.0, 6.0, 5);
    let shown = types.len().min(per_row * 2);
    for (i, (bp_id, ids)) in types.iter().take(shown).enumerate() {
        let bp = s.blueprints.unit(mc_data::BlueprintId(*bp_id as u16));
        let tr = Rect::new(
            x + (i % per_row) as f32 * (tw + gap),
            r.y + 52.0 + (i / per_row) as f32 * (th + gap),
            tw,
            th,
        );
        let t = hud.tile(ui, id("sel-type", *bp_id as usize), tr, false, true);
        let c = Vec2::new(tr.x + tr.w * 0.5, tr.y + 22.0);
        icons::strategic(
            ui,
            bp.visual.icon,
            bp.tech,
            c,
            12.0,
            rgb(palette::TEXT, 0.8 + 0.2 * t.glow),
            ink(0.9),
        );
        ui.text_centred(
            c.x,
            tr.bottom() - 12.0,
            type_scale::VALUE,
            rgb(palette::ACCENT, 1.0),
            &ids.len().to_string(),
        );
        if t.clicked {
            ui.audio.play(Sfx::Select);
            // Click keeps only this type; shift-click drops it.
            let keep: Vec<u32> = if s.view.shift {
                units
                    .iter()
                    .filter(|u| u.blueprint != *bp_id)
                    .map(|u| u.unit_id)
                    .collect()
            } else {
                ids.clone()
            };
            hud.actions.push(HudAction::Select {
                units: keep,
                focus: false,
            });
        }
        if t.hovered {
            ui.text_right(
                r.right() - 16.0,
                r.bottom() - 12.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &format!("{}  \u{b7}  SHIFT-CLICK REMOVES", bp.name.to_uppercase()),
            );
        }
    }
    if types.len() > shown {
        ui.text(
            x,
            r.bottom() - 12.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("+{} MORE TYPES", types.len() - shown),
        );
    }
}

fn single(ui: &mut Ui, s: &Scene, u: &UnitInstance, r: Rect) {
    let bp = s.bp(u);
    let (x, cw) = (r.x + 16.0, r.w - 32.0);
    // The unit's strategic icon in an instrument frame.
    let badge = Rect::new(x, r.y + 16.0, 50.0, 50.0);
    ui.fill(badge, ink(0.6));
    ui.frame(badge, rgb(palette::LINE, 0.2));
    ui.brackets(badge, 6.0, rgb(palette::ACCENT, 0.7));
    let owner = s.team_color((u.owner_flags & 0xFF) as u8);
    icons::strategic(
        ui,
        bp.visual.icon,
        bp.tech,
        Vec2::new(badge.x + 25.0, badge.y + 22.0),
        14.0,
        owner,
        ink(0.9),
    );

    ui.text(
        badge.right() + 14.0,
        r.y + 30.0,
        type_scale::ITEM,
        rgb(0xFFFFFF, 1.0),
        &bp.name.to_uppercase(),
    );
    ui.text(
        badge.right() + 14.0,
        r.y + 54.0,
        type_scale::MICRO,
        rgb(palette::ACCENT, 1.0),
        &format!("T{}  \u{b7}  {}", bp.tech, bp.role.to_uppercase()),
    );

    let level = u.veterancy_level();
    let kills = u.kill_count();
    let hp = veterancy_health(bp.health, level).to_f32();
    let mut y = r.y + 84.0;
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "INTEGRITY");
    ui.text_right(
        x + cw,
        y,
        type_scale::VALUE,
        rgb(palette::TEXT, 1.0),
        &format!("{} / {}", whole(u.health * hp), whole(hp)),
    );
    bar(
        ui,
        Rect::new(x, y + 10.0, cw, 5.0),
        u.health,
        health_tone(u.health),
    );
    y += 30.0;

    let rank = RANKS[level.min(VETERANCY_MAX) as usize];
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "VETERANCY");
    chevrons(ui, Vec2::new(x + 78.0, y), level);
    let vet_label = if level >= VETERANCY_MAX {
        format!(
            "{} KILL{}  ·  {}",
            kills,
            if kills == 1 { "" } else { "S" },
            rank
        )
    } else {
        format!(
            "{} KILL{}  ·  {}  ·  {:.0}%",
            kills,
            if kills == 1 { "" } else { "S" },
            rank,
            u.veterancy_progress() * 100.0
        )
    };
    ui.text_right(
        x + cw,
        y,
        type_scale::VALUE,
        rgb(palette::ACCENT, 1.0),
        &vet_label,
    );
    bar(
        ui,
        Rect::new(x, y + 10.0, cw, 5.0),
        if level >= VETERANCY_MAX {
            1.0
        } else {
            u.veterancy_progress()
        },
        palette::ACCENT,
    );
    y += 30.0;

    let queue = s.queue_of(u.unit_id);
    if has_flag(u, flag::UNDER_CONSTRUCTION) {
        ui.text(
            x,
            y,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "UNDER CONSTRUCTION",
        );
        ui.text_right(
            x + cw,
            y,
            type_scale::VALUE,
            rgb(palette::ACCENT, 1.0),
            &format!("{:.0}%", u.build * 100.0),
        );
        bar(
            ui,
            Rect::new(x, y + 10.0, cw, 5.0),
            u.build,
            palette::ACCENT,
        );
        y += 30.0;
    } else if let Some(front) = queue.and_then(|q| q.orders.first()) {
        let progress = queue.map_or(0.0, |q| q.progress);
        let making = matches!(
            front.kind,
            OrderKind::Build | OrderKind::Produce | OrderKind::Upgrade
        );
        let label = if making {
            format!(
                "{} {}",
                activity(front.kind),
                s.blueprints.unit(front.blueprint).name.to_uppercase()
            )
        } else {
            activity(front.kind).to_owned()
        };
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &label);
        if making && progress > 0.0 {
            ui.text_right(
                x + cw,
                y,
                type_scale::VALUE,
                rgb(palette::ACCENT, 1.0),
                &format!("{:.0}%", progress * 100.0),
            );
            bar(
                ui,
                Rect::new(x, y + 10.0, cw, 5.0),
                progress,
                palette::ACCENT,
            );
            y += 30.0;
        } else {
            y += 18.0;
        }
    }

    // Figures worth knowing, two to a row.
    let mut facts: Vec<(&str, String, u32)> = Vec::new();
    if !bp.weapons.is_empty() {
        facts.push(("DAMAGE / S", format!("{:.0}", dps(bp)), palette::TEXT));
        facts.push((
            "RANGE",
            format!("{:.0} M", bp.max_weapon_range().to_f32()),
            palette::TEXT,
        ));
    }
    if let Some(m) = &bp.motion {
        facts.push((
            "SPEED",
            format!("{:.0} M/S", m.speed.to_f32()),
            palette::TEXT,
        ));
    }
    if let Some(r) = &bp.reclaimer {
        // Its reach is on the map, as a ring.
        facts.push(("RECLAIM", format!("{:.0} / S", r.power.to_f32()), MASS));
    }
    if let Some(b) = &bp.builder {
        facts.push((
            "BUILD POWER",
            format!("{:.0}", b.power.to_f32()),
            palette::TEXT,
        ));
    }
    let e = &bp.economy;
    if e.mass_income.to_f32() > 0.0 {
        facts.push(("MASS", format!("+{:.1}", e.mass_income.to_f32()), MASS));
    }
    if e.energy_income.to_f32() > 0.0 {
        facts.push((
            "ENERGY",
            format!("+{:.0}", e.energy_income.to_f32()),
            ENERGY,
        ));
    }
    if e.energy_upkeep.to_f32() > 0.0 {
        facts.push((
            "UPKEEP",
            format!("-{:.0}", e.energy_upkeep.to_f32()),
            ENERGY,
        ));
    }
    if bp.vision.to_f32() > 0.0 {
        facts.push((
            "VISION",
            format!("{:.0} M", bp.vision.to_f32()),
            palette::TEXT,
        ));
    }
    let col = cw * 0.5;
    for (i, (label, value, tone)) in facts.iter().enumerate() {
        let (fx, fy) = (x + (i % 2) as f32 * (col + 6.0), y + (i / 2) as f32 * 19.0);
        if fy > r.bottom() - 12.0 {
            break;
        }
        ui.text(fx, fy, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
        ui.text_right(
            fx + col - 8.0,
            fy,
            type_scale::VALUE,
            rgb(*tone, 1.0),
            value,
        );
    }
}

struct Order {
    glyph: Glyph,
    label: &'static str,
    key: &'static str,
    hint: &'static str,
    action: HudAction,
    available: bool,
    lit: bool,
}

pub fn orders(hud: &mut Hud, ui: &mut Ui, s: &Scene, units: &[&UnitInstance], r: Rect) {
    hud.glass(ui, r);
    let (x, cw) = (r.x + 14.0, r.w - 28.0);
    ui.section(x, r.y + 20.0, cw, "ORDERS");

    let bps: Vec<&UnitBlueprint> = units.iter().map(|u| s.bp(u)).collect();
    let mobile = bps.iter().any(|b| b.is_mobile());
    let armed = bps.iter().any(|b| !b.weapons.is_empty());
    let builders = bps.iter().any(|b| b.is_mobile() && b.builder.is_some());
    let reclaimers = bps.iter().any(|b| b.reclaims().is_some());
    let factory = units.iter().zip(&bps).find(|(_, b)| b.has(cat::FACTORY));
    let upgradable = bps.iter().any(|b| b.upgrades_to.is_some());
    let repeating = factory.is_some_and(|(u, _)| has_flag(u, flag::REPEAT));
    let targeting = |t: Targeting| s.view.mode == Mode::Target(t);

    let card = [
        Order { glyph: Glyph::Move, label: "MOVE", key: "M", hint: "MOVE TO A POINT. RIGHT-CLICK DOES THE SAME.", action: HudAction::Target(Targeting::Move), available: mobile, lit: targeting(Targeting::Move) },
        Order { glyph: Glyph::Attack, label: "ATTACK", key: "T", hint: "ATTACK ONE ENEMY UNIT.", action: HudAction::Target(Targeting::Attack), available: armed, lit: targeting(Targeting::Attack) },
        Order { glyph: Glyph::AttackMove, label: "ATK-MOVE", key: "F", hint: "MOVE, ENGAGING ANYTHING MET ON THE WAY.", action: HudAction::Target(Targeting::AttackMove), available: mobile && armed, lit: targeting(Targeting::AttackMove) },
        Order { glyph: Glyph::Stop, label: "STOP", key: "X", hint: "DROP EVERY ORDER. A FACTORY CLEARS ITS QUEUE.", action: HudAction::Stop, available: true, lit: false },
        Order { glyph: Glyph::Assist, label: "ASSIST", key: "C", hint: "HELP A BUILDER, OR FINISH AND REPAIR A UNIT.", action: HudAction::Target(Targeting::Assist), available: builders, lit: targeting(Targeting::Assist) },
        Order { glyph: Glyph::Reclaim, label: "RECLAIM", key: "R", hint: "TAKE A WRECK, ONE OF YOUR OWN UNITS OR AN ENEMY APART FOR ITS MASS.", action: HudAction::Target(Targeting::Reclaim), available: reclaimers, lit: targeting(Targeting::Reclaim) },
        Order { glyph: Glyph::Upgrade, label: "UPGRADE", key: "U", hint: "REFIT TO THE NEXT TIER. IT STANDS STILL UNTIL DONE BUT STILL SHOOTS. STOP, OR A RIGHT-CLICK IN THE QUEUE, CANCELS.", action: HudAction::Upgrade, available: upgradable, lit: false },
        Order { glyph: Glyph::Repeat, label: "REPEAT", key: "L", hint: "THE FACTORY BUILDS ITS QUEUE OVER AND OVER.", action: HudAction::Repeat(!repeating), available: factory.is_some(), lit: repeating },
    ];

    let mut hint = None;
    for (i, o) in card.iter().enumerate() {
        let tr = Rect::new(
            x + (i % 3) as f32 * (ORDER_W + ORDER_GAP),
            r.y + 36.0 + (i / 3) as f32 * (ORDER_H + ORDER_GAP),
            ORDER_W,
            ORDER_H,
        );
        let t = hud.tile(ui, id("order", i), tr, o.lit, o.available);
        let live = if o.available { 1.0 } else { 0.22 };
        let tone = rgb(
            if o.lit {
                palette::ACCENT
            } else {
                palette::TEXT
            },
            (0.72 + 0.28 * t.glow) * live,
        );
        icons::glyph(
            ui,
            o.glyph,
            Vec2::new(tr.x + tr.w * 0.5, tr.y + 20.0),
            9.0,
            tone,
        );
        ui.text_centred(
            tr.x + tr.w * 0.5 + 1.5,
            tr.bottom() - 11.0,
            type_scale::MICRO,
            rgb(palette::DIM, (0.9 + 0.1 * t.glow) * live),
            o.label,
        );
        ui.text_right(
            tr.right() - 5.0,
            tr.y + 9.0,
            type_scale::MICRO,
            rgb(palette::FAINT, live),
            o.key,
        );
        if t.clicked {
            ui.audio.play(Sfx::Select);
            hud.actions.push(o.action.clone());
        }
        if t.hovered {
            hint = Some(o.hint);
        }
    }
    if let Some(hint) = hint {
        let w = ui.text_width(type_scale::MICRO, hint) + 24.0;
        let tip = Rect::new(r.x, r.y - 34.0, w, 26.0);
        ui.fill(tip, ink(0.8));
        ui.fill(
            Rect::new(tip.x, tip.y, 2.0, tip.h),
            rgb(palette::ACCENT, 1.0),
        );
        ui.text(
            tip.x + 12.0,
            tip.mid_y(),
            type_scale::MICRO,
            rgb(palette::TEXT, 0.95),
            hint,
        );
    }
}

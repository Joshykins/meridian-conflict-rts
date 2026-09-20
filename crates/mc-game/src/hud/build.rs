//! Construction: what the selected builder can make, in tech tabs, with the
//! queue it is working through above it and a data card for whatever is hovered.

use super::icons;
use super::selection::dps;
use super::{has_flag, whole, Hud, HudAction, Scene, ENERGY, GAP, MASS};
use crate::audio::Sfx;
use crate::game::Mode;
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_core::TICKS_PER_SECOND;
use mc_data::{cat, BlueprintId, UnitBlueprint};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::{flag, OrderKind};

const TILE_W: f32 = 94.0;
const TILE_H: f32 = 78.0;
/// Tile names: the caption face with less tracking, so most names fit on a line.
const NAME: crate::ui::Style = crate::ui::style(mc_render::Face::Medium, 11.0, 1.6);
const TILE_GAP: f32 = 6.0;
const QUEUE_H: f32 = 62.0;

/// Consecutive queue entries for the same blueprint, as one stack.
struct Stack {
    blueprint: BlueprintId,
    count: usize,
    /// The unit's own upgrade, waiting its turn among what it builds.
    upgrade: bool,
}

/// What the queue strip shows.
struct Queue<'a> {
    stacks: &'a [Stack],
    /// How far along the front entry is.
    progress: f32,
    is_factory: bool,
    repeating: bool,
}

pub fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene, units: &[&UnitInstance], r: Rect) {
    // The first finished builder in the selection speaks for it.
    let Some((unit, bp)) = units
        .iter()
        .map(|u| (*u, s.bp(u)))
        .find(|(u, bp)| bp.builder.is_some() && !has_flag(u, flag::UNDER_CONSTRUCTION))
    else {
        return;
    };
    let builder = bp.builder.as_ref().expect("filtered above");
    if builder.builds.is_empty() || r.w < TILE_W + 28.0 {
        return;
    }
    let is_factory = bp.has(cat::FACTORY);

    // Tech tabs: the tiers this builder has anything in. A new kind of builder
    // opens on its own tier, which is what it was most likely selected for.
    let mut tiers: Vec<u8> = builder
        .builds
        .iter()
        .map(|b| s.blueprints.unit(*b).tech)
        .collect();
    tiers.sort_unstable();
    tiers.dedup();
    if hud.tab_for != Some(unit.blueprint) || !tiers.contains(&hud.tab) {
        hud.tab_for = Some(unit.blueprint);
        hud.tab = if tiers.contains(&bp.tech) {
            bp.tech
        } else {
            tiers[0]
        };
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
            Some(top) if top.blueprint == o.blueprint && top.upgrade == upgrade => top.count += 1,
            _ => stacks.push(Stack {
                blueprint: o.blueprint,
                count: 1,
                upgrade,
            }),
        }
    }

    hud.glass(ui, r);
    let (x, cw) = (r.x + 14.0, r.w - 28.0);
    let mut tx = x;
    for tier in 1..=3u8 {
        let has = tiers.contains(&tier);
        let tr = Rect::new(tx, r.y + 10.0, 58.0, 26.0);
        let t = hud.tile(ui, id("tech-tab", tier as usize), tr, hud.tab == tier, has);
        let tone = if hud.tab == tier {
            rgb(0xFFFFFF, 1.0)
        } else {
            rgb(palette::TEXT, if has { 0.6 + 0.4 * t.glow } else { 0.2 })
        };
        ui.text_centred(
            tr.x + tr.w * 0.5 + 2.0,
            tr.mid_y(),
            type_scale::BUTTON,
            tone,
            &format!("T{tier}"),
        );
        if t.clicked && hud.tab != tier {
            ui.audio.play(Sfx::Tick);
            hud.tab = tier;
        }
        tx += 58.0 + 4.0;
    }
    ui.section(
        tx + 10.0,
        r.y + 23.0,
        r.right() - 14.0 - tx - 10.0,
        &format!("CONSTRUCTION  \u{b7}  {}", bp.name.to_uppercase()),
    );

    let placing = match s.view.mode {
        Mode::Place(b) => Some(b),
        _ => None,
    };
    let per_row = (((cw + TILE_GAP) / (TILE_W + TILE_GAP)) as usize).max(1);
    let items: Vec<&UnitBlueprint> = builder
        .builds
        .iter()
        .map(|b| s.blueprints.unit(*b))
        .filter(|b| b.tech == hud.tab)
        .take(per_row * 2)
        .collect();
    let mut hovered: Option<(&UnitBlueprint, Rect)> = None;
    for (i, item) in items.iter().enumerate() {
        let tr = Rect::new(
            x + (i % per_row) as f32 * (TILE_W + TILE_GAP),
            r.y + 46.0 + (i / per_row) as f32 * (TILE_H + TILE_GAP),
            TILE_W,
            TILE_H,
        );
        let t = hud.tile(
            ui,
            id("build", item.id.0 as usize),
            tr,
            placing == Some(item.id),
            true,
        );
        let c = Vec2::new(tr.x + tr.w * 0.5, tr.y + 21.0);
        icons::strategic(
            ui,
            item.visual.icon,
            item.tech,
            c,
            12.0,
            rgb(palette::TEXT, 0.78 + 0.22 * t.glow),
            ink(0.9),
        );
        let lines = wrap(ui, &item.name.to_uppercase(), tr.w - 8.0);
        let top = if lines.len() > 1 {
            tr.y + 44.0
        } else {
            tr.y + 49.0
        };
        for (k, line) in lines.iter().enumerate() {
            ui.text_centred(
                c.x,
                top + k as f32 * 11.5,
                NAME,
                rgb(palette::TEXT, 0.85 + 0.15 * t.glow),
                line,
            );
        }
        ui.text_centred(
            c.x,
            tr.y + 68.0,
            type_scale::MICRO,
            rgb(MASS, 0.9),
            &whole(item.cost_mass.to_f32()),
        );
        let queued: usize = stacks
            .iter()
            .filter(|k| k.blueprint == item.id)
            .map(|k| k.count)
            .sum();
        if queued > 0 {
            let badge = Rect::new(tr.right() - 25.0, tr.y + 4.0, 21.0, 16.0);
            ui.fill(badge, rgb(palette::ACCENT_DEEP, 0.9));
            ui.text_centred(
                badge.x + badge.w * 0.5,
                badge.mid_y(),
                type_scale::MICRO,
                rgb(0xFFFFFF, 1.0),
                &queued.to_string(),
            );
        }
        if t.clicked {
            ui.audio.play(Sfx::Select);
            hud.actions.push(HudAction::Build(item.id));
        }
        if t.right_clicked && is_factory {
            ui.audio
                .play(if queued > 0 { Sfx::Back } else { Sfx::Deny });
            hud.actions.push(HudAction::Cancel(item.id));
        }
        if t.hovered {
            hovered = Some((item, tr));
        }
    }

    let queue_rect = Rect::new(r.x, r.y - GAP - QUEUE_H, r.w, QUEUE_H);
    let has_queue = !stacks.is_empty();
    if has_queue {
        let strip = Queue {
            stacks: &stacks,
            progress: queue.map_or(0.0, |q| q.progress),
            is_factory,
            repeating: has_flag(unit, flag::REPEAT),
        };
        draw_queue(hud, ui, s, queue_rect, &strip);
    }
    if let Some((item, tile)) = hovered {
        let floor = if has_queue { queue_rect.y } else { r.y };
        data_card(
            ui,
            item,
            builder.power.to_f32(),
            is_factory,
            tile,
            floor - GAP,
        );
    }
}

/// A name on one line if it fits, else split at the space nearest its middle.
fn wrap(ui: &mut Ui, text: &str, width: f32) -> Vec<String> {
    if ui.text_width(NAME, text) <= width {
        return vec![text.to_owned()];
    }
    let spaces: Vec<usize> = text.match_indices(' ').map(|(i, _)| i).collect();
    match spaces
        .iter()
        .min_by_key(|i| (**i as i64 - text.len() as i64 / 2).abs())
    {
        Some(&at) => vec![text[..at].to_owned(), text[at + 1..].to_owned()],
        None => vec![text.to_owned()],
    }
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
    } = *queue;
    hud.glass(ui, r);
    let label = if is_factory {
        "PRODUCTION QUEUE"
    } else {
        "BUILD QUEUE"
    };
    ui.fill(
        Rect::new(r.x + 14.0, r.mid_y() - 5.0, 3.0, 10.0),
        rgb(palette::ACCENT, 0.9),
    );
    let end = ui.text(
        r.x + 26.0,
        r.mid_y() - 8.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        label,
    );
    let total: usize = stacks.iter().map(|k| k.count).sum();
    let note = if repeating {
        format!("{total} QUEUED  \u{b7}  REPEATING")
    } else {
        format!("{total} QUEUED")
    };
    ui.text(
        r.x + 26.0,
        r.mid_y() + 9.0,
        type_scale::MICRO,
        rgb(
            if repeating {
                palette::ACCENT
            } else {
                palette::FAINT
            },
            1.0,
        ),
        &note,
    );

    let (w, h, gap) = (48.0, 46.0, 5.0);
    let mut x = end.max(r.x + 150.0) + 18.0;
    let room = (((r.right() - 60.0 - x) / (w + gap)) as usize).max(1);
    for (i, k) in stacks.iter().take(room).enumerate() {
        let item = s.blueprints.unit(k.blueprint);
        let tr = Rect::new(x, r.y + (r.h - h) * 0.5, w, h);
        let t = hud.tile(ui, id("queue", i), tr, i == 0, true);
        icons::strategic(
            ui,
            item.visual.icon,
            item.tech,
            Vec2::new(tr.x + 18.0, tr.y + 18.0),
            9.5,
            rgb(palette::TEXT, 0.8 + 0.2 * t.glow),
            ink(0.9),
        );
        let count = if k.upgrade {
            "UP".to_owned()
        } else {
            format!("{}", k.count)
        };
        ui.text_right(
            tr.right() - 5.0,
            tr.y + 12.0,
            type_scale::VALUE,
            rgb(palette::ACCENT, 1.0),
            &count,
        );
        if i == 0 {
            let track = Rect::new(tr.x + 4.0, tr.bottom() - 8.0, tr.w - 8.0, 3.0);
            ui.fill(track, rgb(palette::LINE, 0.2));
            ui.fill(
                Rect::new(
                    track.x,
                    track.y,
                    track.w * progress.clamp(0.0, 1.0),
                    track.h,
                ),
                rgb(palette::ACCENT, 1.0),
            );
        }
        if k.upgrade {
            if t.right_clicked {
                ui.audio.play(Sfx::Back);
                hud.actions.push(HudAction::CancelUpgrade);
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
        }
        if t.hovered {
            let hint = if k.upgrade {
                format!(
                    "UPGRADE TO {}  \u{b7}  RIGHT-CLICK CANCELS",
                    item.role.to_uppercase()
                )
            } else if is_factory {
                format!(
                    "{}  \u{b7}  CLICK ADDS  \u{b7}  RIGHT-CLICK REMOVES",
                    item.name.to_uppercase()
                )
            } else {
                item.name.to_uppercase()
            };
            let tw = ui.text_width(type_scale::MICRO, &hint) + 24.0;
            let tip = Rect::new(tr.x.min(ui.size.x - tw - 14.0), r.y - 32.0, tw, 26.0);
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
                &hint,
            );
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

/// Everything about a blueprint, over the tile the pointer is on.
fn data_card(
    ui: &mut Ui,
    item: &UnitBlueprint,
    build_power: f32,
    is_factory: bool,
    tile: Rect,
    bottom: f32,
) {
    let mut rows: Vec<(String, String, u32)> = vec![(
        "INTEGRITY".into(),
        whole(item.health.to_f32()),
        palette::TEXT,
    )];
    for w in &item.weapons {
        let one = w.damage.to_f32() * w.salvo.max(1) as f32 / (w.reload_ticks.max(1) as f32 * 0.1);
        rows.push((
            w.name.to_uppercase(),
            format!("{one:.0} DPS  \u{b7}  {:.0} M", w.range_max.to_f32()),
            palette::TEXT,
        ));
    }
    if item.weapons.len() > 1 {
        rows.push((
            "TOTAL DAMAGE / S".into(),
            format!("{:.0}", dps(item)),
            palette::TEXT,
        ));
    }
    if let Some(m) = &item.motion {
        rows.push((
            "SPEED".into(),
            format!("{:.0} M/S", m.speed.to_f32()),
            palette::TEXT,
        ));
    }
    let e = &item.economy;
    for (label, v, tone, sign) in [
        ("MASS INCOME", e.mass_income, MASS, "+"),
        ("ENERGY INCOME", e.energy_income, ENERGY, "+"),
        ("ENERGY UPKEEP", e.energy_upkeep, ENERGY, "-"),
        ("MASS STORAGE", e.mass_storage, MASS, "+"),
        ("ENERGY STORAGE", e.energy_storage, ENERGY, "+"),
    ] {
        if v.to_f32() > 0.0 {
            rows.push((label.into(), format!("{sign}{}", whole(v.to_f32())), tone));
        }
    }
    if let Some(r) = &item.reclaimer {
        let charge = if r.charge_ticks > 0 {
            format!(
                "  \u{b7}  {:.1} S CHARGE",
                r.charge_ticks as f32 / TICKS_PER_SECOND as f32
            )
        } else {
            String::new()
        };
        rows.push((
            "RECLAIM BEAM".into(),
            format!(
                "{:.0} / S  \u{b7}  {:.0} M{charge}",
                r.power.to_f32(),
                r.range.to_f32()
            ),
            MASS,
        ));
    }
    if let Some(b) = &item.builder {
        rows.push((
            "BUILD POWER".into(),
            format!("{:.0}", b.power.to_f32()),
            palette::TEXT,
        ));
    }
    if item.radar.to_f32() > 0.0 {
        rows.push((
            "RADAR".into(),
            format!("{:.0} M", item.radar.to_f32()),
            palette::TEXT,
        ));
    }

    let (w, row_h) = (360.0, 19.0);
    let h = 112.0 + rows.len() as f32 * row_h + 30.0;
    let r = Rect::new(
        (tile.x + tile.w * 0.5 - w * 0.5).clamp(14.0, ui.size.x - w - 14.0),
        bottom - h,
        w,
        h,
    );
    ui.panel(r);
    let (x, cw) = (r.x + 18.0, r.w - 36.0);
    ui.text(
        x,
        r.y + 26.0,
        type_scale::ITEM,
        rgb(0xFFFFFF, 1.0),
        &item.name.to_uppercase(),
    );
    ui.text(
        x,
        r.y + 48.0,
        type_scale::MICRO,
        rgb(palette::ACCENT, 1.0),
        &format!("TECH {}  \u{b7}  {}", item.tech, item.role.to_uppercase()),
    );

    // The price: mass, energy, and how long this builder takes over it.
    let seconds = item.build_time.to_f32() / build_power.max(0.1);
    let costs = [
        ("MASS", whole(item.cost_mass.to_f32()), MASS),
        ("ENERGY", whole(item.cost_energy.to_f32()), ENERGY),
        ("TIME", super::clock(seconds), palette::TEXT),
    ];
    for (i, (label, value, tone)) in costs.iter().enumerate() {
        let cx = x + i as f32 * cw / 3.0;
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
    for (label, value, tone) in &rows {
        // A long weapon name gives way to its figures, never runs into them.
        let room = cw - ui.text_width(type_scale::VALUE, value) - 14.0;
        let label = shorten(ui, label, room);
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &label);
        ui.text_right(x + cw, y, type_scale::VALUE, rgb(*tone, 1.0), value);
        y += row_h;
    }
    let hint = if is_factory {
        "CLICK +1  \u{b7}  SHIFT +5  \u{b7}  RIGHT-CLICK -1"
    } else {
        "CLICK OR DRAG TO PLACE  \u{b7}  SHIFT KEEPS PLACING"
    };
    ui.text(
        x,
        r.bottom() - 16.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        hint,
    );
}

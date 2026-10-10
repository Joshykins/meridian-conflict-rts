//! A carrier's air wings in the unit panel (`mc_sim::air_wings`): a tile for each kind of
//! wing it builds (its aircraft, how many, the whole wing's price), the queue, and the wing
//! under way. Click a tile to queue one, right-click to take the last one
//! of it off the queue.

use super::build::{unit_face, BUILDING};
use super::{whole, Hud, HudAction, Scene, MASS};
use crate::audio::Sfx;
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use mc_data::AirWings;
use mc_sim::mirror::{UnitInstance, WingView};

/// The panel's width at most.
pub const WIDTH: f32 = 460.0;

/// The first of the selection that builds wings and is the viewer's own, with its queue.
pub fn carrier_of<'a>(
    s: &'a Scene,
    units: &[&'a UnitInstance],
) -> Option<(&'a UnitInstance, &'a AirWings, Option<&'a WingView>)> {
    let u = units
        .iter()
        .find(|u| (u.owner_flags & 0xFF) as u8 == s.view.local && s.bp(u).air_wings.is_some())?;
    let wings = s.bp(u).air_wings.as_ref()?;
    let view = s.view.frame.wings.iter().find(|w| w.unit_id == u.unit_id);
    Some((u, wings, view))
}

pub fn panel(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    wings: &AirWings,
    view: Option<&WingView>,
    r: Rect,
) {
    hud.glass(ui, r);
    let (x, cw) = (r.x + 16.0, r.w - 32.0);
    let queue = view.map_or(&[][..], |v| &v.queue[..]);
    let mut y = r.y + 22.0;
    ui.text(x, y, type_scale::ITEM, rgb(palette::TEXT, 1.0), "Air Wings");
    let line = match view {
        Some(v) if v.launching > 0 => format!("Launching  \u{b7}  {} queued", queue.len()),
        _ if queue.is_empty() => "Nothing queued".to_owned(),
        _ => format!("{} queued", queue.len()),
    };
    let lw = ui.text_width(type_scale::MICRO, &line);
    ui.text(
        x + cw - lw,
        y + 1.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &line,
    );
    y += 14.0;

    // One tile a wing: the aircraft's face, "8 x Fighter Wing", the price of all of it.
    let n = wings.wings.len().max(1) as f32;
    let gap = 6.0;
    let tile_w = ((cw - gap * (n - 1.0)) / n).min(104.0);
    let tile_h = r.bottom() - y - 52.0;
    let carrier = vec![mc_sim::Handle(u.unit_id)];
    for (i, wing) in wings.wings.iter().enumerate() {
        let tr = Rect::new(x + i as f32 * (tile_w + gap), y, tile_w, tile_h);
        let aircraft = s.blueprints.unit(wing.aircraft);
        let building = queue.first() == Some(&(i as u8));
        let t = hud.tile(ui, id("wing", i), tr, building, true);
        unit_face(hud, ui, aircraft, tr, t.glow);
        let cx = tr.x + tr.w * 0.5;
        let (st, name) = ui.fitted(type_scale::MICRO, &wing.name, tr.w - 6.0);
        ui.text_centred(cx, tr.bottom() - 32.0, st, rgb(0xFFFFFF, 0.95), &name);
        let (st, what) = ui.fitted(
            type_scale::MICRO,
            &format!("{} \u{d7} {}", wing.count, aircraft.name),
            tr.w - 8.0,
        );
        ui.text_centred(cx, tr.bottom() - 20.0, st, rgb(palette::DIM, 0.95), &what);
        let price = aircraft.cost_mass.to_f32() * wing.count as f32;
        ui.text_centred(
            cx,
            tr.bottom() - 8.5,
            type_scale::MICRO,
            rgb(MASS, 0.95),
            &whole(price),
        );
        let queued = queue.iter().filter(|&&w| w == i as u8).count();
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
        if t.clicked {
            ui.audio.play(Sfx::Select);
            hud.actions
                .push(HudAction::Send(mc_sim::Command::QueueWing {
                    units: carrier.clone(),
                    wing: i as u8,
                    count: 1,
                }));
        }
        if t.right_clicked {
            ui.audio
                .play(if queued > 0 { Sfx::Back } else { Sfx::Deny });
            hud.actions
                .push(HudAction::Send(mc_sim::Command::CancelWing {
                    units: carrier.clone(),
                    wing: i as u8,
                }));
        }
    }

    // The wing under way, filling, and what follows it.
    let by = r.bottom() - 30.0;
    let Some((&first, v)) = queue.first().zip(view) else {
        return;
    };
    let name = wings
        .wings
        .get(first as usize)
        .map_or("", |w| w.name.as_str());
    let label = format!("{name}  {:.0}%", v.progress * 100.0);
    ui.text(x, by, type_scale::MICRO, rgb(BUILDING, 1.0), &label);
    let bar = Rect::new(x, by + 8.0, cw, 4.0);
    ui.fill(bar, rgb(BUILDING, 0.15));
    ui.fill(
        Rect::new(bar.x, bar.y, bar.w * v.progress.clamp(0.0, 1.0), bar.h),
        rgb(BUILDING, 0.95),
    );
}

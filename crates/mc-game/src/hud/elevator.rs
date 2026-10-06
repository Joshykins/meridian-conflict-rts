//! A space elevator (the Skyhook) in the HUD (`mc_sim::elevator`, `docs/ELEVATOR.md`).
//!
//! - Its panel, right of the order card, beside its build menu: its tier, where the
//!   platform's cycle stands, and how much of the next platform's payload the queue
//!   fills, the rest coming down as materials (`ElevatorView`, `UnitOrders::elevator`).
//! - The queue strip over the build menu counts down to the next platform instead of a
//!   build's progress (`strip`): nothing on an elevator's queue is ever built.
//! - Calls: an enemy elevator sighted or its lot spotted, ours online once its cable is
//!   clamped, each with its own sound (`data/sounds/elevator_alerts.ron`) and a card that
//!   stays up a while; a platform docking is a quiet note.

use super::notices::{Glyph, Live};
use super::{Hud, Scene, MASS};
use crate::titan_marks::SIGHTED_CARD_SECONDS;
use crate::ui::{ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_sim::mirror::{ElevatorPhase, ElevatorView, UnitInstance, KIND_WRECK, STATE_UNIDENTIFIED};
use mc_sim::tables::{flag, OrderKind};
use mc_sim::SimEvent;
use std::collections::HashMap;

/// Width of the elevator panel, right of the order card.
pub const WIDTH: f32 = 318.0;
/// A space elevator's own mark: the pale sky-cyan of its cable.
pub const CABLE: u32 = 0x8FE3FF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    /// An enemy elevator seen standing.
    Sighted,
    /// An enemy elevator's lot seen, still going up.
    SiteSpotted,
    /// One of ours has its cable down and clamped.
    Online,
}

/// A call on screen for a while: which, about whom, and how long it has been up.
struct Card {
    call: Call,
    unit: u32,
    name: String,
    at: Vec2,
    age: f32,
}

/// The ticks read so far, the enemy elevators known, and the cards still up.
#[derive(Default)]
pub struct Calls {
    tick: Option<u32>,
    /// Enemy elevators seen: still going up when last seen?
    known: HashMap<u32, bool>,
    cards: Vec<Card>,
}

fn owner_of(u: &UnitInstance) -> u8 {
    (u.owner_flags & 0xFF) as u8
}

/// The name of the elevator `unit`, from its blueprint while it is in sight.
fn name_of(s: &Scene, unit: u32) -> String {
    s.view
        .index_of
        .get(&unit)
        .map(|&i| &s.view.frame.units[i])
        .map_or_else(|| "Space elevator".to_owned(), |u| s.bp(u).name.clone())
}

/// Calls for what just happened, and this frame's cards.
pub fn alerts(hud: &mut Hud, ui: &mut Ui, s: &Scene, dt: f32) {
    let _t = mc_core::perf_span!("ui.elevator_alerts");
    let view = s.view;
    let tick = view.frame.tick;
    let calls = &mut hud.elevator;
    // A new match, or the range reset: start over.
    if calls.tick.is_some_and(|t| tick < t) {
        *calls = Calls::default();
    }
    let mut heard: Vec<(Call, u32, Vec2)> = Vec::new();
    if calls.tick != Some(tick) {
        calls.tick = Some(tick);
        let ours = |owner: u8| !view.observing && owner == view.local;
        for e in &view.frame.events {
            match e {
                SimEvent::ElevatorTethering { unit, pos, owner } if ours(*owner) => {
                    hud.notices.note(
                        format!("elevator-tether-{}", unit.0),
                        format!("{}: lowering its cable", name_of(s, unit.0)),
                        CABLE,
                        Glyph::Elevator,
                        Some(Vec2::from(pos.xy().to_f32())),
                    );
                }
                SimEvent::ElevatorTethered { unit, pos, owner } if ours(*owner) => {
                    heard.push((Call::Online, unit.0, Vec2::from(pos.xy().to_f32())));
                }
                SimEvent::ElevatorDocked {
                    pos,
                    owner,
                    units,
                    mass,
                    ..
                } if ours(*owner) => {
                    hud.notices.note(
                        "elevator-docked",
                        docked_title(*units, mass.to_f32()),
                        CABLE,
                        Glyph::Elevator,
                        Some(Vec2::from(pos.xy().to_f32())),
                    );
                }
                _ => {}
            }
        }
        // Enemy elevators in sight: the first look at one is called, and its standing up
        // once it was seen going up.
        let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
        for u in &view.frame.units {
            if u.owner_flags & (KIND_WRECK | STATE_UNIDENTIFIED) != 0
                || s.bp(u).elevator.is_none()
                || view.observing
                || team(owner_of(u)) == team(view.local)
            {
                continue;
            }
            let building = super::has_flag(u, flag::UNDER_CONSTRUCTION);
            let at = Vec2::new(u.pos[0], u.pos[1]);
            match (hud.elevator.known.insert(u.unit_id, building), building) {
                (None, true) => heard.push((Call::SiteSpotted, u.unit_id, at)),
                (None, false) | (Some(true), false) => heard.push((Call::Sighted, u.unit_id, at)),
                _ => {}
            }
        }
    }
    let (library, _) = ui.audio.library();
    for (call, unit, at) in heard {
        let (sound, gain) = match call {
            Call::Sighted => ("elevator_sighted", 0.85),
            Call::SiteSpotted => ("elevator_site_spotted", 0.75),
            Call::Online => ("elevator_online", 0.8),
        };
        if let Some(id) = library.id_of(sound) {
            ui.audio.play_response(id, gain);
        }
        let calls = &mut hud.elevator;
        // One card per elevator and call: a second call about it restarts its card.
        calls.cards.retain(|c| !(c.unit == unit && c.call == call));
        calls.cards.push(Card {
            call,
            unit,
            name: name_of(s, unit),
            at,
            age: 0.0,
        });
    }
    for c in &mut hud.elevator.cards {
        c.age += dt;
    }
    hud.elevator.cards.retain(|c| c.age < SIGHTED_CARD_SECONDS);

    for (call, key) in [
        (Call::Sighted, "elevator-sighted"),
        (Call::SiteSpotted, "elevator-site"),
        (Call::Online, "elevator-online"),
    ] {
        let group: Vec<&Card> = hud
            .elevator
            .cards
            .iter()
            .filter(|c| c.call == call)
            .collect();
        let Some(first) = group.first() else { continue };
        let n = group.len();
        let name = &first.name;
        let (title, sub, tone, loud) = match (call, n) {
            (Call::Sighted, 1) => (
                "Enemy space elevator sighted".to_owned(),
                format!("{name}  \u{b7}  Tier 5  \u{b7}  lands an army from orbit"),
                palette::BAD,
                true,
            ),
            (Call::Sighted, _) => (
                format!("{n} enemy space elevators sighted"),
                "Tier 5  \u{b7}  each lands an army from orbit".to_owned(),
                palette::BAD,
                true,
            ),
            (Call::SiteSpotted, 1) => (
                "Enemy space elevator site spotted".to_owned(),
                format!("{name} going up  \u{b7}  strike before its cable is down"),
                palette::WARN,
                true,
            ),
            (Call::SiteSpotted, _) => (
                format!("{n} enemy space elevator sites spotted"),
                "Their lots are in sight".to_owned(),
                palette::WARN,
                true,
            ),
            (_, 1) => (
                format!("{name} online"),
                "Cable clamped  \u{b7}  platforms bring the queue down free".to_owned(),
                CABLE,
                false,
            ),
            _ => (
                format!("{n} space elevators online"),
                "Cables clamped  \u{b7}  platforms bring their queues down free".to_owned(),
                CABLE,
                false,
            ),
        };
        let marks = group
            .iter()
            .map(|c| {
                let now = view
                    .index_of
                    .get(&c.unit)
                    .map(|&i| &view.frame.units[i])
                    .filter(|u| u.owner_flags & STATE_UNIDENTIFIED == 0)
                    .map_or(c.at, |u| Vec2::new(u.pos[0], u.pos[1]));
                // Kept calm: a card's urgency follows its soonest mark.
                (30.0, now)
            })
            .collect();
        hud.notices.live(Live {
            key,
            title,
            sub,
            tone,
            glyph: Glyph::Elevator,
            loud,
            figure: Some("T5".to_owned()),
            marks,
        });
    }
}

/// A platform's docking note: what came off it.
fn docked_title(units: u16, mass: f32) -> String {
    let what = match units {
        0 => "no units".to_owned(),
        1 => "1 unit".to_owned(),
        n => format!("{n} units"),
    };
    format!("Platform docked: {what}, {} materials", super::whole(mass))
}

/// The first of our own finished elevators among the selection, its view, and how many
/// there are.
pub fn elevator_of<'a>(
    s: &'a Scene,
    units: &[&'a UnitInstance],
) -> Option<(&'a UnitInstance, ElevatorView, usize)> {
    let ours: Vec<(&UnitInstance, ElevatorView)> = units
        .iter()
        .filter(|u| owner_of(u) == s.view.local && s.bp(u).elevator.is_some())
        .filter_map(|u| Some((*u, s.queue_of(u)?.elevator?)))
        .collect();
    ours.first().map(|&(u, v)| (u, v, ours.len()))
}

/// What the cycle is doing, and how long it has left, for the panel and the strip.
pub(super) fn phase_line(v: &ElevatorView) -> (String, u32) {
    let secs = |t: f32| format!("{:.0} s", t.ceil().max(0.0));
    match v.phase {
        ElevatorPhase::Tethering => (format!("Lowering cable  {}", secs(v.left)), palette::WARN),
        ElevatorPhase::Inbound => (format!("Platform inbound  {}", secs(v.left)), CABLE),
        ElevatorPhase::Docked => ("Docked \u{2014} unloading".to_owned(), 0xFFFFFF),
        ElevatorPhase::Ascending => ("Platform ascending".to_owned(), palette::TEXT),
        ElevatorPhase::Away => (
            format!("Next platform in {}", secs(v.to_touchdown)),
            palette::TEXT,
        ),
    }
}

/// The queue strip's head for an elevator: what it is doing, how far along the wait for
/// the next platform is, and the seconds to it (in place of a build's progress and time).
pub(super) fn strip(v: &ElevatorView) -> (&'static str, f32, f32) {
    let doing = match v.phase {
        ElevatorPhase::Tethering => "Lowering Cable",
        ElevatorPhase::Inbound => "Platform Inbound",
        ElevatorPhase::Docked => "Platform Docked",
        ElevatorPhase::Ascending | ElevatorPhase::Away => "Next Platform",
    };
    (doing, v.approach, v.to_touchdown)
}

/// The mass of each unit riding the next platform, in order: the front of the queue, as
/// many as the view says ride.
fn riders(s: &Scene, u: &UnitInstance, v: &ElevatorView) -> Vec<f32> {
    s.queue_of(u)
        .into_iter()
        .flat_map(|q| &q.orders)
        .take_while(|o| o.kind == OrderKind::Produce)
        .take(v.riding as usize)
        .map(|o| s.blueprints.unit(o.blueprint).cost_mass.to_f32())
        .collect()
}

/// An elevator's panel over `r`: its tier, the platform's cycle, and the next payload.
pub fn panel(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    v: &ElevatorView,
    count: usize,
    r: Rect,
) {
    let bp = s.bp(u);
    hud.glass(ui, r);
    let t = ui.time;
    let (x, cw) = (r.x + 16.0, r.w - 32.0);
    let mut y = r.y + 22.0;

    // Header: its name, the tier-5 plate, and how many are picked.
    ui.text(x, y, type_scale::ITEM, rgb(palette::TEXT, 1.0), &bp.name);
    let nx = x + ui.text_width(type_scale::ITEM, &bp.name) + 10.0;
    let tier = format!("Tier {}", bp.tech);
    let tw = ui.text_width(type_scale::MICRO, &tier) + 16.0;
    let plate = Rect::new(nx, y - 10.0, tw, 17.0);
    ui.fill(plate, rgb(CABLE, 0.92));
    ui.brackets(plate.inset(-3.0), 4.0, rgb(CABLE, 0.7));
    ui.text_centred(
        plate.x + plate.w * 0.5,
        plate.mid_y(),
        type_scale::MICRO,
        ink(1.0),
        &tier,
    );
    let right = if count > 1 {
        format!("{count} picked")
    } else {
        bp.role.clone()
    };
    ui.text_right(x + cw, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &right);
    y += 24.0;

    // The cycle: what the platform is doing, and a bar of the whole period split into its
    // descent, dock, ascent and time away, filled to where it stands.
    let (label, tone) = phase_line(v);
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Platform");
    ui.text_right(x + cw, y, type_scale::VALUE, rgb(tone, 1.0), &label);
    y += 10.0;
    let bar = Rect::new(x, y, cw, 10.0);
    ui.fill_cut(bar, 3.0, ink(0.6));
    let inner = Rect::new(bar.x + 2.0, bar.y + 2.0, bar.w - 4.0, bar.h - 4.0);
    let at = |k: f32| inner.x + inner.w * k.clamp(0.0, 1.0);
    if v.phase == ElevatorPhase::Tethering {
        // The cable coming down: amber, filling.
        let fill = Rect::new(inner.x, inner.y, inner.w * v.cycle, inner.h);
        ui.gradient_h(fill, rgb(palette::WARN, 0.45), rgb(palette::WARN, 0.95));
    } else {
        // The trip's spans, faint, so the fill reads against where the platform will be.
        let period = v.period.max(0.1);
        let [down, dock, up] = v.trip.map(|s| s / period);
        ui.fill(
            Rect::new(at(0.0), inner.y, at(down) - at(0.0), inner.h),
            rgb(CABLE, 0.12),
        );
        ui.fill(
            Rect::new(at(down), inner.y, at(down + dock) - at(down), inner.h),
            rgb(0xFFFFFF, 0.16),
        );
        ui.fill(
            Rect::new(
                at(down + dock),
                inner.y,
                at(down + dock + up) - at(down + dock),
                inner.h,
            ),
            rgb(CABLE, 0.08),
        );
        let fill = Rect::new(inner.x, inner.y, at(v.cycle) - inner.x, inner.h);
        ui.gradient_h(fill, rgb(CABLE, 0.4), rgb(CABLE, 0.9));
        let shimmer = if v.phase == ElevatorPhase::Docked {
            0.15 + 0.15 * (t * 6.0).sin()
        } else {
            0.0
        };
        ui.fill(fill, rgb(0xFFFFFF, shimmer.max(0.0)));
        // Touchdown and lift-off ticks.
        for k in [down, down + dock, down + dock + up] {
            ui.vline(at(k), bar.y + 1.0, bar.h - 2.0, rgb(0xFFFFFF, 0.5));
        }
        ui.fill(
            Rect::new(at(v.cycle) - 1.0, bar.y - 2.0, 2.0, bar.h + 4.0),
            rgb(0xFFFFFF, 0.9),
        );
    }
    ui.frame(bar, rgb(tone, 0.4));
    y += 28.0;

    // The next platform: what of the queue rides it, a block per unit, and the rest of
    // its payload, which comes down as materials.
    let riders = riders(s, u, v);
    let queued = s.queue_of(u).map_or(0, |q| {
        q.orders
            .iter()
            .filter(|o| o.kind == OrderKind::Produce)
            .count()
    });
    ui.text(
        x,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Next platform",
    );
    let rides = match (v.riding, queued) {
        (0, 0) => "Queue units to bring them down".to_owned(),
        (n, q) if n as usize >= q => format!("{n} of {q} queued ride it"),
        (n, q) => format!("{n} of {q} queued ride it  \u{b7}  rest wait"),
    };
    ui.text_fit_left(
        x + 96.0,
        y,
        cw - 96.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.9),
        &rides,
    );
    y += 9.0;
    let bar = Rect::new(x, y, cw, 14.0);
    ui.fill_cut(bar, 3.0, ink(0.6));
    let inner = Rect::new(bar.x + 2.0, bar.y + 2.0, bar.w - 4.0, bar.h - 4.0);
    let payload = v.payload.max(1.0);
    let mut px = inner.x;
    for &mass in &riders {
        let w = inner.w * mass / payload;
        let block = Rect::new(px, inner.y, (w - 1.0).max(1.0), inner.h);
        ui.gradient_v(block, rgb(CABLE, 0.95), rgb(CABLE, 0.55));
        px += w;
    }
    let rest = Rect::new(px, inner.y, (inner.right() - px).max(0.0), inner.h);
    ui.gradient_h(rest, rgb(MASS, 0.55), rgb(MASS, 0.3));
    ui.frame(bar, rgb(CABLE, 0.35));
    y += 32.0;

    // Its figures, a column each: label over value.
    let materials = (v.payload - v.riding_mass).max(0.0);
    let facts = [
        ("Units", super::whole(v.riding_mass), CABLE),
        ("Materials", super::whole(materials), MASS),
        ("Payload", super::whole(v.payload), palette::TEXT),
        ("Every", format!("{:.0} s", v.period), palette::TEXT),
    ];
    let widths = [0.27, 0.27, 0.27, 0.19];
    let mut fx = x;
    for ((k, val, tone), share) in facts.iter().zip(widths) {
        ui.text(fx, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), k);
        ui.text(fx, y + 17.0, type_scale::VALUE, rgb(*tone, 0.95), val);
        fx += cw * share;
    }
    ui.text_fit_left(
        x,
        r.bottom() - 14.0,
        cw,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "Queued units ride down free  \u{b7}  the rest of the payload is materials",
    );
}

#[cfg(test)]
mod tests {
    use super::docked_title;
    use crate::audio::Bank;
    use mc_data::sounds::SoundLibrary;

    #[test]
    fn a_docking_note_names_what_came_off() {
        assert_eq!(
            docked_title(6, 5600.0),
            "Platform docked: 6 units, 5,600 materials"
        );
        assert_eq!(docked_title(1, 0.0), "Platform docked: 1 unit, 0 materials");
        assert_eq!(
            docked_title(0, 20000.0),
            "Platform docked: no units, 20,000 materials"
        );
    }

    /// The elevator's calls (`elevator_alerts.ron`) start and end cleanly, ring out before
    /// they stop, and sit at a sane level: the checks `audio.rs` gives the whole library.
    #[test]
    fn elevator_calls_are_clean() {
        let library = SoundLibrary::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let bank = Bank::synthesise(44_100, &library);
        for name in [
            "select_elevator",
            "elevator_sighted",
            "elevator_site_spotted",
            "elevator_online",
        ] {
            let id = library
                .id_of(name)
                .unwrap_or_else(|| panic!("{name} is in the library"));
            let frames = bank.world(id);
            let peak = frames.iter().flatten().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(
                frames.iter().flatten().all(|s| s.is_finite()),
                "{name} has a non-finite sample"
            );
            assert!((0.1..=0.9).contains(&peak), "{name} peaks at {peak}");
            assert!(
                frames[0].iter().all(|s| s.abs() < 0.02),
                "{name} starts at {:?}",
                frames[0]
            );
            assert!(
                frames[frames.len() - 1].iter().all(|s| s.abs() < 1e-4),
                "{name} ends at {:?}",
                frames[frames.len() - 1]
            );
            let tail = &frames[frames.len() - 1200..frames.len() - 600];
            let tail_peak = tail.iter().flatten().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(
                tail_peak < 0.06 * peak,
                "{name} is cut off while still at {tail_peak} (peak {peak})"
            );
        }
    }
}

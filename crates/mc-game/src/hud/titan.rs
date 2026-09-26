//! A tier-5 titan (the Behemoth) in the HUD. What `titan_marks.rs` follows turns into:
//!
//! - Calls and cards: an enemy titan sighted, its lot spotted, ours standing up, each with
//!   its own horn (`data/sounds/titan_alerts.ron`) and a card that stays up a while; a
//!   click looks at the titan where it is now. Great-bore strikes charging and storms
//!   raging are live cards grouped by side, like warheads in flight.
//! - The titan's own panel, right of the order card: its tier, the great bore charging,
//!   recharging or ready, the storm it raises, and a strike button (J).
//! - The minimap: every titan in sight as its own figure, ringed, and every strike's mark
//!   and storm.

use super::notices::{Glyph, Live};
use super::{Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::game::{Mode, Targeting};
use crate::titan_marks::{self as marks, BoreState, NewsKind, Phase, STORM, TITAN};
use crate::ui::{id, ink, palette, rgb, style, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::BlueprintId;
use mc_render::Face;
use mc_sim::mirror::{UnitInstance, KIND_WRECK, STATE_UNIDENTIFIED};

/// Width of the titan panel, right of the order card.
pub const WIDTH: f32 = 318.0;

/// A call on screen for a while: which, about whom, and how long it has been up.
struct Card {
    kind: NewsKind,
    unit: u32,
    name: String,
    at: Vec2,
    age: f32,
}

/// The news read so far, and the cards still up.
#[derive(Default)]
pub struct Alerts {
    read: u64,
    cards: Vec<Card>,
}

fn owner_of(u: &UnitInstance) -> u8 {
    (u.owner_flags & 0xFF) as u8
}

/// Calls for what just happened, and this frame's live cards.
pub fn alerts(hud: &mut Hud, ui: &mut Ui, s: &Scene, dt: f32) {
    let view = s.view;
    let titans = &view.titans;
    let (library, _) = ui.audio.library();
    for n in titans.news.iter().filter(|n| n.serial > hud.titan.read) {
        let (sound, gain) = match n.kind {
            NewsKind::Sighted => ("titan_sighted", 0.85),
            NewsKind::SiteSpotted => ("titan_site_spotted", 0.75),
            NewsKind::Online => ("titan_online", 0.8),
            NewsKind::StrikeWarning => ("titan_strike_warning", 0.8),
        };
        if let Some(id) = library.id_of(sound) {
            ui.audio.play_response(id, gain);
        }
        if n.kind != NewsKind::StrikeWarning {
            // One card per titan and kind: a second call about it restarts its card.
            hud.titan.cards.retain(|c| !(c.unit == n.unit && c.kind == n.kind));
            hud.titan.cards.push(Card { kind: n.kind, unit: n.unit, name: n.name.clone(), at: n.at, age: 0.0 });
        }
    }
    hud.titan.read = titans.news.last().map_or(hud.titan.read, |n| n.serial.max(hud.titan.read));
    // A new match starts the count again.
    if titans.news.last().is_some_and(|n| n.serial < hud.titan.read) {
        hud.titan.read = 0;
    }
    for c in &mut hud.titan.cards {
        c.age += dt;
    }
    hud.titan.cards.retain(|c| c.age < marks::SIGHTED_CARD_SECONDS);

    // Titan calls, grouped by kind; each mark is where the titan is now, if it is in sight.
    let now = |unit: u32, at: Vec2| {
        view.index_of
            .get(&unit)
            .map(|&i| &view.frame.units[i])
            .filter(|u| u.owner_flags & STATE_UNIDENTIFIED == 0)
            .map_or(at, |u| Vec2::new(u.pos[0], u.pos[1]))
    };
    for (kind, key) in [(NewsKind::Sighted, "titan-sighted"), (NewsKind::SiteSpotted, "titan-site"), (NewsKind::Online, "titan-online")] {
        let group: Vec<&Card> = hud.titan.cards.iter().filter(|c| c.kind == kind).collect();
        let Some(first) = group.first() else { continue };
        let n = group.len();
        let name = &first.name;
        let (title, sub, tone, loud) = match (kind, n) {
            (NewsKind::Sighted, 1) => (format!("Enemy {name} sighted"), "Tier 5 siege titan  \u{b7}  its great bore levels a base".to_owned(), palette::BAD, true),
            (NewsKind::Sighted, _) => (format!("{n} enemy titans sighted"), "Tier 5 siege titans on the field".to_owned(), palette::BAD, true),
            (NewsKind::SiteSpotted, 1) => (format!("Enemy {name} under construction"), "Its lot is in sight  \u{b7}  strike before it stands".to_owned(), palette::WARN, true),
            (NewsKind::SiteSpotted, _) => (format!("{n} enemy titans under construction"), "Their lots are in sight".to_owned(), palette::WARN, true),
            (_, 1) => (format!("{name} online"), "Tier 5 siege titan ready for orders".to_owned(), TITAN, false),
            _ => (format!("{n} titans online"), "Tier 5 siege titans ready for orders".to_owned(), TITAN, false),
        };
        hud.notices.live(Live {
            key,
            title,
            sub,
            tone,
            glyph: Glyph::Titan,
            loud,
            figure: Some("T5".to_owned()),
            // Kept calm: the urgency of a card follows its soonest mark.
            marks: group.iter().map(|c| (30.0, now(c.unit, c.at))).collect(),
        });
    }

    // Great-bore strikes: enemy, ours, allied, like warheads in flight.
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    let tick = view.frame.tick as i64;
    let mut sides: [Vec<(f32, Vec2, bool, f32)>; 3] = Default::default();
    for st in titans.strikes.iter().filter(|st| marks::shown(view, st)) {
        let side = if !view.observing && team(st.owner) != team(view.local) {
            0
        } else if st.owner == view.local && !view.observing {
            1
        } else {
            2
        };
        let c = st.centre().truncate();
        match st.phase(tick, 0.0) {
            Phase::Charging(_, left) => sides[side].push((left, c, true, st.radius)),
            Phase::Bolt => sides[side].push((0.0, c, true, st.radius)),
            Phase::Storm(_, _, left) => sides[side].push((left, c, false, st.radius)),
            Phase::Spent(_) => {}
        }
    }
    for (side, group) in sides.iter().enumerate() {
        if group.is_empty() {
            continue;
        }
        let charging: Vec<f32> = group.iter().filter(|g| g.2).map(|g| g.0).collect();
        let storms = group.len() - charging.len();
        let soonest = charging.iter().copied().fold(f32::INFINITY, f32::min);
        let (key, title) = match (side, charging.is_empty()) {
            (0, false) => ("titan-strike-enemy", "AEB-3 strike incoming".to_owned()),
            (0, true) => ("titan-strike-enemy", "Enemy lightning storm".to_owned()),
            (1, false) => ("titan-strike-own", "AEB-3 charging".to_owned()),
            (1, true) => ("titan-strike-own", "Lightning storm raging".to_owned()),
            (_, false) => ("titan-strike-ally", if view.observing { "AEB-3 charging" } else { "Allied AEB-3 charging" }.to_owned()),
            _ => ("titan-strike-ally", "Lightning storm".to_owned()),
        };
        let mut sub = if soonest.is_finite() {
            if soonest > 0.05 {
                format!("Fires in {:.0} s  \u{b7}  storm spreads {}", soonest.ceil(), marks::metres(group.iter().map(|g| g.3).fold(0.0, f32::max)))
            } else {
                "Bolt away".to_owned()
            }
        } else {
            String::new()
        };
        if storms > 0 {
            if !sub.is_empty() {
                sub.push_str("  \u{b7}  ");
            }
            let last = group.iter().filter(|g| !g.2).map(|g| g.0).fold(0.0f32, f32::max);
            sub.push_str(&if storms == 1 { format!("Storm {:.0} s left", last.ceil()) } else { format!("{storms} storms  \u{b7}  {:.0} s left", last.ceil()) });
        }
        let figure = if soonest.is_finite() {
            Some(format!("{:.0}", soonest.ceil()))
        } else {
            group.iter().map(|g| g.0).fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b)))).map(|v| format!("{:.0}", v.ceil()))
        };
        hud.notices.live(Live {
            key,
            title,
            sub,
            tone: if side == 0 { palette::BAD } else { STORM },
            glyph: Glyph::Storm,
            loud: side == 0,
            figure,
            marks: group.iter().map(|g| (g.0, g.1)).collect(),
        });
    }
}

/// The first of our own finished titans among the selection, and how many there are.
pub fn titan_of<'a>(s: &Scene, units: &[&'a UnitInstance]) -> Option<(&'a UnitInstance, usize)> {
    let ours: Vec<&&UnitInstance> = units
        .iter()
        .filter(|u| {
            owner_of(u) == s.view.local
                && !super::has_flag(u, mc_sim::tables::flag::UNDER_CONSTRUCTION)
                && marks::is_titan(s.bp(u))
        })
        .collect();
    ours.first().map(|u| (**u, ours.len()))
}

/// A titan's panel over `r`: its tier, its great bore's state, the storm it raises, and
/// the strike button.
pub fn panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, u: &UnitInstance, count: usize, r: Rect) {
    let bp = s.bp(u);
    let Some((_, w)) = marks::storm_weapon(bp) else { return };
    let Some(storm) = w.bore.and_then(|b| b.storm) else { return };
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
    ui.fill(plate, rgb(TITAN, 0.92));
    ui.brackets(plate.inset(-3.0), 4.0, rgb(TITAN, 0.7));
    ui.text_centred(plate.x + plate.w * 0.5, plate.mid_y(), type_scale::MICRO, ink(1.0), &tier);
    if count > 1 {
        ui.text_right(x + cw, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &format!("{count} picked"));
    } else {
        ui.text_right(x + cw, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &bp.role);
    }
    y += 24.0;

    // The great bore.
    let state = s.view.titans.bore(u.unit_id, s.view.frame.tick);
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &w.name);
    let (label, tone) = match state {
        BoreState::Charging(_, left) => (format!("Charging  {:.1} s", left.max(0.0)), STORM),
        BoreState::Recharging(_, left) => (format!("Recharging  {:.0} s", left.ceil()), palette::WARN),
        BoreState::Ready => ("Ready".to_owned(), STORM),
    };
    ui.text_right(x + cw, y, type_scale::VALUE, rgb(tone, 1.0), &label);
    y += 10.0;
    // The charge bar: violet filling while it charges, amber while it recharges, full and
    // shimmering when ready.
    let bar = Rect::new(x, y, cw, 10.0);
    ui.fill_cut(bar, 3.0, ink(0.6));
    let (fill, shimmer) = match state {
        BoreState::Charging(k, _) => (k, 0.25 + 0.25 * (t * 14.0).sin()),
        BoreState::Recharging(k, _) => (k, 0.0),
        BoreState::Ready => (1.0, 0.12 + 0.12 * (t * 2.2).sin()),
    };
    let inner = Rect::new(bar.x + 2.0, bar.y + 2.0, (bar.w - 4.0) * fill, bar.h - 4.0);
    ui.gradient_h(inner, rgb(tone, 0.55), rgb(tone, 0.95));
    ui.fill(inner, rgb(0xFFFFFF, shimmer.max(0.0)));
    // Ticks: the charge's share of the cycle.
    for k in 1..10 {
        let tx = bar.x + bar.w * k as f32 / 10.0;
        ui.vline(tx, bar.y + 2.0, bar.h - 4.0, ink(0.35));
    }
    ui.frame(bar, rgb(tone, 0.4));
    y += 24.0;

    // Its figures, a column each: label over value.
    let facts = [
        ("Storm", marks::metres(storm.radius.to_f32()), STORM),
        ("Spreads", format!("{:.0} s", storm.ticks as f32 / 10.0), palette::TEXT),
        ("Reach", format!("{:.1}\u{2013}{:.1} km", w.range_min.to_f32() / 1000.0, w.range_max.to_f32() / 1000.0), palette::TEXT),
        ("Charge", format!("{:.0} s", w.charge_ticks as f32 / 10.0), palette::TEXT),
        ("Cycle", format!("{:.0} s", w.reload_ticks as f32 / 10.0), palette::TEXT),
    ];
    let widths = [0.2, 0.18, 0.28, 0.17, 0.17];
    let mut fx = x;
    for ((k, v, tone), share) in facts.iter().zip(widths) {
        ui.text(fx, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), k);
        ui.text(fx, y + 17.0, type_scale::VALUE, rgb(*tone, 0.95), v);
        fx += cw * share;
    }
    y += 34.0;

    // The strike button: a storm under the pointer, the great bore alone (`Command::Strike`;
    // plain fire on ground, J, fires every gun).
    let foot = r.bottom() - 30.0;
    let h = (foot - y).clamp(40.0, 58.0);
    let button = Rect::new(x, foot - h, cw, h);
    let aiming = matches!(s.view.mode, Mode::Target(Targeting::Strike));
    let res = ui.interact(id("titan-strike", 0), button, true);
    hud.claim(ui, button);
    let hot = if aiming { 1.0 } else { res.glow };
    ui.fill_cut(button, 7.0, ink(0.66));
    ui.gradient_h(button, rgb(STORM, 0.14 + 0.22 * hot), rgb(STORM, 0.02));
    ui.outline_cut(button, 7.0, rgb(STORM, 0.35 + 0.45 * hot), rgb(0xFFFFFF, 0.3 + 0.4 * hot));
    // A bolt in a ring on the left, turning while a mark is being chosen.
    let c = Vec2::new(button.x + 28.0, button.mid_y());
    bolt(ui, c, 11.0, rgb(STORM, 0.9), if aiming { t * 1.5 } else { 0.0 });
    let big = style(Face::Bold, 19.0, 1.0);
    ui.text(button.x + 52.0, button.mid_y() - 5.0, big, rgb(0xFFFFFF, 0.85 + 0.15 * hot), if aiming { "Choose the mark" } else { "Strike" });
    let sub = if aiming { "Click the ground  \u{b7}  RMB cancels" } else { "Fire the great bore on the ground" };
    ui.text_fit_left(button.x + 52.0, button.mid_y() + 13.0, button.w - 90.0, type_scale::MICRO, rgb(STORM, 0.9), sub);
    if res.clicked {
        ui.audio.play(Sfx::Select);
        hud.actions.push(HudAction::Target(Targeting::Strike));
    }
    ui.text_fit_left(x, r.bottom() - 14.0, cw, type_scale::MICRO, rgb(palette::FAINT, 1.0),
        "Strides over structures  \u{b7}  crushes what is underfoot");
}

/// A lightning bolt in a ring: the great bore's mark.
pub fn bolt(ui: &mut Ui, c: Vec2, r: f32, color: crate::ui::Color, spin: f32) {
    let pts = [(-0.18, -0.62), (0.22, -0.62), (0.02, -0.1), (0.3, -0.1), (-0.22, 0.66), (-0.06, 0.08), (-0.32, 0.08)];
    let p: Vec<Vec2> = pts.iter().map(|&(x, y)| c + Vec2::new(x, y) * r).collect();
    for i in 0..p.len() {
        ui.stroke(p[i], p[(i + 1) % p.len()], 1.6, color);
    }
    ui.triangle(p[0], p[1], p[2], color);
    ui.triangle(p[0], p[2], p[6], color);
    ui.triangle(p[6], p[3], p[4], color);
    ui.triangle(p[6], p[2], p[3], color);
    for k in 0..4 {
        let a = spin + k as f32 * std::f32::consts::FRAC_PI_2;
        ui.arc(c, r * 1.45, a + 0.2, a + std::f32::consts::FRAC_PI_2 - 0.2, 1.4, color);
    }
}

/// Titans and their strikes on the minimap. `at` places a world point on the chart.
pub fn minimap(ui: &mut Ui, s: &Scene, chart: Rect, at: &dyn Fn(Vec2) -> Vec2) {
    let view = s.view;
    let t = ui.time;
    let per_metre = (at(Vec2::new(1000.0, 0.0)).x - at(Vec2::ZERO).x) / 1000.0;
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    let tick = view.frame.tick as i64;
    for st in view.titans.strikes.iter().filter(|st| marks::shown(view, st)) {
        let c = at(st.centre().truncate());
        if !chart.contains(c) {
            continue;
        }
        let enemy = !view.observing && team(st.owner) != team(view.local);
        let tone = if enemy { palette::BAD } else { STORM };
        let full = st.radius * per_metre;
        match st.phase(tick, 0.0) {
            Phase::Charging(k, _) => {
                let pulse = 0.5 + 0.5 * (t * (4.0 + 8.0 * k)).sin();
                ui.arc(c, full.max(3.0), 0.0, std::f32::consts::TAU, 1.0, rgb(tone, 0.5 + 0.5 * pulse));
                ui.arc(c, full.max(3.0) + 2.5, -std::f32::consts::FRAC_PI_2, -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k, 1.6, rgb(STORM, 1.0));
                ui.disc(c, 1.6, rgb(tone, 1.0));
            }
            Phase::Bolt => {
                ui.arc(c, full.max(3.0), 0.0, std::f32::consts::TAU, 1.4, rgb(0xFFFFFF, 0.9));
            }
            Phase::Storm(reach, _, _) => {
                let r = (reach * per_metre).max(2.0);
                ui.disc(c, r, rgb(STORM, 0.28));
                ui.arc(c, r, 0.0, std::f32::consts::TAU, 1.2, rgb(STORM, 0.9));
                ui.arc(c, full.max(3.0), 0.0, std::f32::consts::TAU, 0.8, rgb(tone, 0.4));
            }
            Phase::Spent(k) => {
                ui.disc(c, full.max(2.0), rgb(STORM, 0.18 * k));
            }
        }
    }
    // Every titan in sight: its figure, ringed; an enemy's ring pulses.
    for u in &view.frame.units {
        if u.owner_flags & (KIND_WRECK | STATE_UNIDENTIFIED) != 0 {
            continue;
        }
        let bp = s.blueprints.unit(BlueprintId(u.blueprint as u16));
        if !marks::is_titan(bp) {
            continue;
        }
        let p = at(Vec2::new(u.pos[0], u.pos[1]));
        if !chart.contains(p) {
            continue;
        }
        let owner = owner_of(u);
        let enemy = !view.observing && team(owner) != team(view.local);
        let pulse = 0.5 + 0.5 * (t * 3.0).sin();
        ui.disc(p, 8.5, ink(0.7));
        let halo = if enemy { rgb(palette::BAD, 0.5 + 0.5 * pulse) } else { rgb(TITAN, 0.7) };
        ui.arc(p, 9.0 + if enemy { 2.0 * pulse } else { 0.0 }, 0.0, std::f32::consts::TAU, 1.2, halo);
        let building = super::has_flag(u, mc_sim::tables::flag::UNDER_CONSTRUCTION);
        let color = s.team_color(owner);
        let color = if building { [color[0] * 0.5, color[1] * 0.5, color[2] * 0.5, 1.0] } else { color };
        super::icons::titan(ui, p + Vec2::new(0.0, 0.6), 6.2, color);
    }
}

#[cfg(test)]
mod tests {
    use crate::audio::Bank;
    use mc_data::sounds::SoundLibrary;

    /// The titan's calls (`titan_alerts.ron`) start and end cleanly, ring out before they
    /// stop, and sit at a sane level: the checks `audio.rs` gives the whole library.
    #[test]
    fn titan_calls_are_clean() {
        let library = SoundLibrary::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
        let bank = Bank::synthesise(44_100, &library);
        for name in ["select_titan", "titan_sighted", "titan_site_spotted", "titan_online", "titan_strike_warning"] {
            let id = library.id_of(name).unwrap_or_else(|| panic!("{name} is in the library"));
            let frames = bank.world(id);
            let peak = frames.iter().flatten().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(frames.iter().flatten().all(|s| s.is_finite()), "{name} has a non-finite sample");
            assert!((0.1..=0.9).contains(&peak), "{name} peaks at {peak}");
            assert!(frames[0].iter().all(|s| s.abs() < 0.02), "{name} starts at {:?}", frames[0]);
            assert!(frames[frames.len() - 1].iter().all(|s| s.abs() < 1e-4), "{name} ends at {:?}", frames[frames.len() - 1]);
            let tail = &frames[frames.len() - 1200..frames.len() - 600];
            let tail_peak = tail.iter().flatten().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(tail_peak < 0.06 * peak, "{name} is cut off while still at {tail_peak} (peak {peak})");
        }
    }
}

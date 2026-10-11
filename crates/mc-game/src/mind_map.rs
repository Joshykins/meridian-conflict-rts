//! An AI's map of the zones, for an observer (`docs/AI_COMMANDER.md`, "Overlay"),
//! drawn on the ground while the AI Mind card's map switch is on:
//!
//! - **Mind**: what the watched Commander believes. Its own zones are tinted its colour;
//!   each enemy zone is tinted its holder's colour, fading as what it knows of the zone
//!   ages. Ground it has never had eyes on is dim. A ring at a zone's middle is an army
//!   it believes in, dashed and fading with how sure it is.
//! - **Truth**: what every player really holds there, counted the same way.
//!
//! Each zone with anything in it carries a tag (main base, expansion, firebase...), and
//! the zone under the pointer gets a card: everything known of it, and in Mind, what
//! is really there beside it.

use crate::nuke_marks::{project, surface, tag};
use crate::orders::Field;
use crate::ui::{ink, palette, rgb, type_scale, Color, Rect, Ui};
use glam::{Vec2, Vec3};
use mc_sim::ai::{MindHolding, MindZone, ZoneMapView, NO_ZONE};

/// What the map shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MapMode {
    #[default]
    Off,
    /// What the watched AI believes.
    Mind,
    /// What is there.
    Truth,
}

/// Metres above the ground the map is drawn at, clear of the terrain's own shading.
const LIFT: f32 = 3.0;
/// Seconds of age at which what is known of a zone is drawn at its faintest.
const STALE: f32 = 600.0;

/// One zone as drawn: whose it is, how sure, and what to say over it.
struct Shown<'a> {
    owner: Option<u8>,
    /// 0..1: how strongly to tint it.
    strength: f32,
    /// Never looked at (Mind only).
    unseen: bool,
    held: Vec<&'a MindHolding>,
    /// Army believed or there, mass, and how sure (0..1).
    army: Option<(i32, f32)>,
}

/// The holder a zone is tinted for: whoever holds the most structure mass there, or,
/// with none standing, the largest army.
fn chief<'a>(held: &[&'a MindHolding]) -> Option<&'a MindHolding> {
    held.iter().copied().max_by_key(|h| {
        (
            h.built.iter().map(|b| b.1).sum::<i32>(),
            h.army,
            std::cmp::Reverse(h.player),
        )
    })
}

fn built_mass(h: &MindHolding) -> i32 {
    h.built.iter().map(|b| b.1).sum()
}

fn kilo(m: i32) -> String {
    if m >= 1000 {
        format!("{}.{}k", m / 1000, m / 100 % 10)
    } else {
        m.to_string()
    }
}

/// What to draw of each zone, by zone id.
fn shown<'a>(
    map: &ZoneMapView,
    mode: MapMode,
    who: u8,
    known: &'a [MindZone],
    truth: &'a [Vec<MindHolding>],
) -> Vec<Shown<'a>> {
    let mut out: Vec<Shown> = (0..map.zones.len())
        .map(|_| Shown {
            owner: None,
            strength: 0.0,
            unseen: mode == MapMode::Mind,
            held: Vec::new(),
            army: None,
        })
        .collect();
    for (z, s) in out.iter_mut().enumerate() {
        let real = truth.get(z).map(Vec::as_slice).unwrap_or(&[]);
        match mode {
            MapMode::Truth => {
                s.held = real.iter().collect();
                s.strength = 1.0;
            }
            // Its own ground it knows as it is; the rest as it believes.
            MapMode::Mind => s.held = real.iter().filter(|h| h.player == who).collect(),
            MapMode::Off => {}
        }
    }
    if mode == MapMode::Mind {
        for k in known {
            let Some(s) = out.get_mut(k.zone as usize) else {
                continue;
            };
            s.unseen = k.seen_ago.is_none() && k.view == 0;
            let age = k.seen_ago.map_or(STALE, |a| a as f32);
            s.strength = (1.0 - age / STALE).clamp(0.25, 1.0);
            s.held.extend(k.held.iter());
        }
    }
    for s in &mut out {
        let c = chief(&s.held);
        s.owner = c.map(|h| h.player);
        if mode == MapMode::Mind {
            if let Some(h) = c.filter(|h| h.player != who) {
                s.strength = s.strength.min(h.built_sure.max(h.army_sure) as f32 / 100.0);
            } else if c.is_some() {
                s.strength = 1.0;
            }
        }
        s.army = s
            .held
            .iter()
            .filter(|h| h.player != who || mode == MapMode::Truth)
            .map(|h| (h.army, h.army_sure))
            .filter(|&(m, _)| m > 0)
            .max()
            .map(|(m, sure)| (m, sure as f32 / 100.0));
    }
    out
}

fn color(colors: &[[f32; 3]], player: u8, a: f32) -> Color {
    let c = colors[player as usize % colors.len().max(1)];
    [c[0], c[1], c[2], a]
}

/// Draws the map, and returns the zone under `cursor`.
pub fn draw(
    ui: &mut Ui,
    field: &Field,
    mode: MapMode,
    who: u8,
    cursor: Option<Vec3>,
) -> Option<u16> {
    let status = &field.view.status;
    let (Some(map), MapMode::Mind | MapMode::Truth) = (status.zone_map.as_deref(), mode) else {
        return None;
    };
    let colors = &field.view.colors;
    let known: &[MindZone] = status
        .players
        .get(who as usize)
        .and_then(|p| p.mind.as_ref())
        .map_or(&[], |m| m.known.as_slice());
    let shown = shown(map, mode, who, known, &status.zone_truth);
    let node = map.node as f32;
    let (w, h) = (map.w as usize, map.h as usize);
    let at = |x: usize, y: usize| map.id[y * w + x];
    // Every node corner on screen, a row at a time.
    let corner = |ui: &Ui, x: usize, y: usize| {
        let p = Vec2::new(x as f32 * node, y as f32 * node);
        project(ui, field, p.extend(surface(field, p) + LIFT))
    };
    let mut above: Vec<Option<Vec2>> = (0..=w).map(|x| corner(ui, x, 0)).collect();
    let (sw, sh) = (ui.size.x, ui.size.y);
    let on_screen = |p: Vec2| p.x > -80.0 && p.y > -80.0 && p.x < sw + 80.0 && p.y < sh + 80.0;
    for y in 0..h {
        let below: Vec<Option<Vec2>> = (0..=w).map(|x| corner(ui, x, y + 1)).collect();
        for x in 0..w {
            let z = at(x, y);
            let (Some(a), Some(b), Some(c), Some(d)) =
                (above[x], above[x + 1], below[x + 1], below[x])
            else {
                continue;
            };
            if !(on_screen(a) || on_screen(c)) {
                continue;
            }
            let right = (x + 1 < w).then(|| at(x + 1, y));
            let down = (y + 1 < h).then(|| at(x, y + 1));
            let left = (x > 0).then(|| at(x - 1, y));
            let up = (y > 0).then(|| at(x, y - 1));
            // A band along each zone's edge is tinted; its middle is left clear.
            let edge = [right, down, left, up]
                .iter()
                .any(|n| n.is_some_and(|n| n != z));
            if z != NO_ZONE && edge {
                let s = &shown[z as usize];
                let tint = match s.owner {
                    Some(o) => color(colors, o, 0.10 + 0.20 * s.strength),
                    None if s.unseen => ink(0.28),
                    None => rgb(palette::LINE, 0.04),
                };
                ui.triangle(a, b, c, tint);
                ui.triangle(a, c, d, tint);
            }
            // The border itself: a fine line where two zones meet.
            let line = |n: Option<u16>| n.is_some_and(|n| n != z && (z != NO_ZONE || n != NO_ZONE));
            let ink_line = rgb(palette::TEXT, 0.30);
            if line(right) {
                ui.stroke(b, c, 1.2, ink_line);
            }
            if line(down) {
                ui.stroke(d, c, 1.2, ink_line);
            }
        }
        above = below;
    }
    // Over each zone: what it is, and an army believed there.
    for (z, s) in shown.iter().enumerate() {
        let shape = &map.zones[z];
        let mid = Vec2::new(shape.centre.0 as f32, shape.centre.1 as f32);
        let Some(p) = project(ui, field, mid.extend(surface(field, mid) + LIFT)) else {
            continue;
        };
        if !on_screen(p) {
            continue;
        }
        if let Some((mass, sure)) = s.army {
            let radius = 60.0 + (mass as f32).sqrt() * 2.5;
            ring(ui, field, mid, radius, sure, mode == MapMode::Mind);
        }
        let Some(c) = chief(&s.held) else {
            if s.unseen {
                ui.text_centred(
                    p.x,
                    p.y,
                    type_scale::MICRO,
                    rgb(palette::FAINT, 0.7),
                    "unseen",
                );
            }
            continue;
        };
        let built = built_mass(c);
        let text = if built > 0 {
            format!("{}  {}", c.tag, kilo(built))
        } else {
            format!("{}  {}", c.tag, kilo(c.army))
        };
        let tone = colors[c.player as usize % colors.len().max(1)];
        let tone = ((tone[0] * 255.0) as u32) << 16
            | ((tone[1] * 255.0) as u32) << 8
            | (tone[2] * 255.0) as u32;
        tag(ui, p, &text, tone);
    }
    let z = cursor.map(|c| {
        let (x, y) = ((c.x / node).floor() as i32, (c.y / node).floor() as i32);
        if x < 0 || y < 0 || x >= map.w || y >= map.h {
            NO_ZONE
        } else {
            at(x as usize, y as usize)
        }
    })?;
    (z != NO_ZONE).then_some(z)
}

/// A ring on the ground for an army: dashed when believed, solid when there.
fn ring(ui: &mut Ui, field: &Field, c: Vec2, radius: f32, sure: f32, dashed: bool) {
    const POINTS: usize = 48;
    let pts: Vec<Option<Vec2>> = (0..=POINTS)
        .map(|i| {
            let a = i as f32 / POINTS as f32 * std::f32::consts::TAU + ui.time * 0.2;
            let p = c + Vec2::from_angle(a) * radius;
            project(ui, field, p.extend(surface(field, p) + LIFT))
        })
        .collect();
    let tone = rgb(palette::BAD, 0.25 + 0.6 * sure);
    for (i, w) in pts.windows(2).enumerate() {
        if dashed && i % 2 == 1 {
            continue;
        }
        if let [Some(a), Some(b)] = *w {
            ui.stroke(a, b, 2.0, tone);
        }
    }
}

/// The card for zone `z`: everything the map knows of it. In Mind, what is really
/// there follows, so a wrong belief shows.
pub fn card(ui: &mut Ui, field: &Field, mode: MapMode, who: u8, z: u16, at: Vec2) {
    let status = &field.view.status;
    let Some(map) = status.zone_map.as_deref() else {
        return;
    };
    let Some(shape) = map.zones.get(z as usize) else {
        return;
    };
    let colors = &field.view.colors;
    let names: Vec<&str> = status.players.iter().map(|p| p.name.as_str()).collect();
    let known = status
        .players
        .get(who as usize)
        .and_then(|p| p.mind.as_ref())
        .and_then(|m| m.known.iter().find(|k| k.zone == z));
    let truth: &[MindHolding] = status.zone_truth.get(z as usize).map_or(&[], Vec::as_slice);
    let mut lines: Vec<(Color, String, bool)> = Vec::new();
    let dim = rgb(palette::DIM, 1.0);
    let text = rgb(palette::TEXT, 1.0);
    let ground = if shape.water { "water" } else { "land" };
    let ore = if shape.ore > 0 {
        format!("  \u{b7}  {} ore", shape.ore)
    } else {
        String::new()
    };
    lines.push((text, format!("Zone {z}  \u{b7}  {ground}{ore}"), true));
    let holder = |lines: &mut Vec<(Color, String, bool)>, h: &MindHolding, belief: bool| {
        let name = names.get(h.player as usize).copied().unwrap_or("?");
        lines.push((
            color(colors, h.player, 1.0),
            format!("{name}  \u{b7}  {}", h.tag),
            true,
        ));
        for (kind, mass, count) in &h.built {
            lines.push((text, format!("  {count} {kind}  {}", kilo(*mass)), false));
        }
        if !h.built.is_empty() && belief {
            lines.push((
                dim,
                format!("  seen {} s ago, {}% sure", h.built_ago, h.built_sure),
                false,
            ));
        }
        if h.army_seen > 0 {
            let line = if belief {
                format!(
                    "  army {} seen {} s ago, ~{} now",
                    kilo(h.army_seen),
                    h.army_ago,
                    kilo(h.army)
                )
            } else {
                format!("  army {}", kilo(h.army))
            };
            lines.push((text, line, false));
        }
        if h.builders > 0 {
            lines.push((text, format!("  {} builders", h.builders), false));
        }
        if h.king {
            lines.push((rgb(palette::WARN, 1.0), "  their commander".into(), false));
        }
    };
    match mode {
        MapMode::Mind => {
            match known {
                Some(k) => {
                    let seen = k.seen_ago.map_or("never looked at".to_string(), |a| {
                        format!("looked at {a} s ago")
                    });
                    lines.push((
                        dim,
                        format!("{seen}  \u{b7}  {}% in view", k.view / 10),
                        false,
                    ));
                    if k.blips > 0 {
                        lines.push((dim, format!("{} unknown on radar", k.blips), false));
                    }
                    for h in &k.held {
                        holder(&mut lines, h, true);
                    }
                }
                None => lines.push((dim, "never looked at".into(), false)),
            }
            for h in truth.iter().filter(|h| h.player == who) {
                holder(&mut lines, h, false);
            }
            let real: Vec<String> = truth
                .iter()
                .filter(|h| h.player != who)
                .map(|h| {
                    let name = names.get(h.player as usize).copied().unwrap_or("?");
                    format!(
                        "{name} {} built, {} army",
                        kilo(built_mass(h)),
                        kilo(h.army)
                    )
                })
                .collect();
            lines.push((dim, String::new(), false));
            lines.push((
                dim,
                if real.is_empty() {
                    "Really there: nothing of the enemy".into()
                } else {
                    format!("Really there: {}", real.join("; "))
                },
                false,
            ));
        }
        MapMode::Truth => {
            if truth.is_empty() {
                lines.push((dim, "nothing stands here".into(), false));
            }
            for h in truth {
                holder(&mut lines, h, false);
            }
        }
        MapMode::Off => return,
    }
    const LINE: f32 = 17.0;
    let wide = lines
        .iter()
        .map(|(_, t, head)| {
            ui.text_width(
                if *head {
                    type_scale::CAPTION
                } else {
                    type_scale::MICRO
                },
                t,
            )
        })
        .fold(180.0f32, f32::max)
        + 28.0;
    let tall = lines.len() as f32 * LINE + 18.0;
    let x = (at.x + 24.0).min(ui.size.x - wide - 8.0);
    let y = (at.y + 16.0).min(ui.size.y - tall - 8.0);
    let r = Rect::new(x, y, wide, tall);
    ui.fill(r, ink(0.82));
    ui.fill(Rect::new(r.x, r.y, 3.0, r.h), rgb(palette::ACCENT, 0.9));
    let mut ty = r.y + 16.0;
    for (c, t, head) in &lines {
        let st = if *head {
            type_scale::CAPTION
        } else {
            type_scale::MICRO
        };
        ui.text(r.x + 14.0, ty, st, *c, t);
        ty += LINE;
    }
}

//! A side's interceptor network on the map: every array of the team, its cover drawn as
//! one merged outline (arcs inside another array's cover are left out), a pip row of its
//! rounds over each array, and a card of what the network holds.
//!
//! Shown while an interceptor array is being placed, the new site's cover drawn against
//! the network (what it adds bright, what it overlaps faint, and how much ground is new),
//! and while an array is selected (then without the card: the unit panel has its rounds).

use super::{owner_of, project, surface, tag};
use crate::hud::silo::{self, Launcher, INTERCEPT};
use crate::orders::Field;
use crate::ui::{self, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::strategic::StrategicKind;
use mc_data::BlueprintId;
use mc_sim::mirror::KIND_WRECK;

/// Construction amber, for arrays still being built.
const BUILDING: u32 = 0xFFA928;
/// Most arrays the card draws the rounds of, one group each.
const CARD_ARRAYS: usize = 6;

/// One array of the network.
struct Array {
    at: Vec2,
    height: f32,
    cover: f32,
    owner: u8,
    /// How far it is built, 0..1; it covers nothing until it is finished.
    build: f32,
    launcher: Option<Launcher>,
    round_seconds: f32,
    selected: bool,
}

impl Array {
    fn covers(&self) -> bool {
        self.build >= 1.0
    }
}

/// Every array of `team`'s, built or being built.
fn arrays(field: &Field, team: u8) -> Vec<Array> {
    let view = field.view;
    let team_of = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    view.frame
        .units
        .iter()
        .filter(|u| u.owner_flags & KIND_WRECK == 0 && team_of(owner_of(u)) == Some(team))
        .filter_map(|u| {
            let bp = field.blueprints.unit(BlueprintId(u.blueprint as u16));
            let s = bp
                .strategic
                .as_ref()
                .filter(|s| s.kind == StrategicKind::Interceptor)?;
            Some(Array {
                at: Vec2::new(u.pos[0], u.pos[1]),
                height: bp.height.to_f32(),
                cover: s.coverage.to_f32(),
                owner: owner_of(u),
                build: u.build,
                launcher: Launcher::of(u),
                round_seconds: s.round_seconds(),
                selected: view.selection.contains(&u.unit_id),
            })
        })
        .collect()
}

/// Whether `p` lies deeper than `inset` metres inside any of `circles` but `skip`.
fn inside_any(p: Vec2, circles: &[(Vec2, f32)], skip: usize, inset: f32) -> bool {
    circles.iter().enumerate().any(|(j, &(c, r))| {
        // Two arrays in the same spot: the first one draws the shared edge.
        let same = j < skip && c.distance(circles[skip].0) < 1.0 && (r - circles[skip].1) < 1.0;
        j != skip && (same || p.distance(c) < r - inset - 1.0)
    })
}

/// The union of `circles` as one outline: each circle's arcs that no other covers, with a
/// soft band just inside. Arcs that lie under `beneath` (the new site's cover while
/// placing) are drawn faint and dashed: after it is built they are inside the network.
fn outline(
    ui: &mut Ui,
    field: &Field,
    circles: &[(Vec2, f32)],
    beneath: Option<(Vec2, f32)>,
    tone: u32,
    alpha: f32,
    width: f32,
) {
    let s = ui.s;
    let band = |r: f32| (r * 0.035).clamp(30.0, 90.0);
    let at = |p: Vec2| {
        field
            .camera
            .project(p.extend(surface(field, p) + 2.0))
            .map(|q| q / s)
    };
    for (i, &(c, r)) in circles.iter().enumerate() {
        let segments = ((r / 14.0) as usize).clamp(64, 320);
        let inner = r - band(r);
        let point = |k: usize, radius: f32| {
            c + Vec2::from_angle(k as f32 / segments as f32 * std::f32::consts::TAU) * radius
        };
        for k in 0..segments {
            let (a, b) = (point(k, r), point(k + 1, r));
            let mid = (a + b) * 0.5;
            if inside_any(mid, circles, i, 0.0) {
                continue;
            }
            let under = beneath.is_some_and(|(nc, nr)| mid.distance(nc) < nr - 1.0);
            let (Some(pa), Some(pb)) = (at(a), at(b)) else {
                continue;
            };
            if under {
                if k % 2 == 0 {
                    ui.stroke(pa, pb, width * 0.7, rgb(tone, alpha * 0.35));
                }
                continue;
            }
            ui.stroke(pa, pb, width, rgb(tone, alpha));
            // The band: a faint wash between the edge and the inset line, where the
            // inset is itself outside every other circle's inset.
            let (ia, ib) = (point(k, inner), point(k + 1, inner));
            if inside_any((ia + ib) * 0.5, circles, i, band(r)) {
                continue;
            }
            if let (Some(qa), Some(qb)) = (at(ia), at(ib)) {
                ui.triangle(pa, pb, qb, rgb(tone, alpha * 0.10));
                ui.triangle(pa, qb, qa, rgb(tone, alpha * 0.10));
                ui.stroke(qa, qb, 1.0, rgb(tone, alpha * 0.22));
            }
        }
    }
}

/// Share of the ground within `radius` of `c` that none of `circles` covers yet, 0..1.
fn new_ground(c: Vec2, radius: f32, circles: &[(Vec2, f32)]) -> f32 {
    const N: i32 = 28;
    let (mut inside, mut fresh) = (0u32, 0u32);
    for y in 0..N {
        for x in 0..N {
            let d = Vec2::new(
                (x as f32 + 0.5) / N as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / N as f32 * 2.0 - 1.0,
            );
            if d.length_squared() > 1.0 {
                continue;
            }
            let p = c + d * radius;
            inside += 1;
            fresh += u32::from(!circles.iter().any(|&(o, r)| p.distance(o) < r));
        }
    }
    fresh as f32 / inside.max(1) as f32
}

/// A row of pips over an array: one per round it holds room for, lit when loaded, the
/// next one filling amber as it is assembled. An array being built shows how far.
fn badge(ui: &mut Ui, field: &Field, a: &Array, ally: Option<&str>) {
    let Some(g) = project(
        ui,
        field,
        a.at.extend(surface(field, a.at) + a.height + 6.0),
    ) else {
        return;
    };
    if !a.covers() {
        tag(
            ui,
            g - Vec2::new(0.0, 14.0),
            &format!("Building  \u{b7}  {:.0}%", a.build * 100.0),
            BUILDING,
        );
        return;
    }
    let Some(l) = a.launcher else { return };
    let (pip, gap) = (7.0, 3.0);
    let n = l.capacity.max(1) as f32;
    let w = n * pip + (n - 1.0) * gap + 12.0;
    let r = Rect::new(g.x - w * 0.5, g.y - 24.0, w, 15.0);
    ui.fill(r, ui::ink(0.72));
    ui.fill(Rect::new(r.x, r.y, w, 1.5), rgb(INTERCEPT, 0.8));
    for k in 0..l.capacity {
        let p = Rect::new(r.x + 6.0 + k as f32 * (pip + gap), r.y + 4.0, pip, 8.0);
        if k < l.stock {
            ui.fill(p, rgb(INTERCEPT, 1.0));
        } else {
            ui.fill(p, rgb(palette::LINE, 0.10));
            if k == l.stock && l.assembling() {
                let h = p.h * l.progress;
                ui.fill(Rect::new(p.x, p.bottom() - h, p.w, h), rgb(BUILDING, 0.9));
            }
        }
    }
    if let Some(name) = ally {
        ui.text_centred(
            g.x,
            r.y - 8.0,
            type_scale::MICRO,
            rgb(palette::TEXT, 0.7),
            name,
        );
    }
}

/// The network's card beside `anchor`: arrays, interceptors ready, each array's rounds,
/// when the next is loaded, and what the array being placed would add.
fn card(ui: &mut Ui, anchor: Vec2, net: &[Array], adding: (u32, f32)) {
    let built: Vec<&Array> = net.iter().filter(|a| a.covers()).collect();
    let ready: u32 = built
        .iter()
        .filter_map(|a| a.launcher)
        .map(|l| l.stock)
        .sum();
    let room: u32 = built
        .iter()
        .filter_map(|a| a.launcher)
        .map(|l| l.capacity)
        .sum();
    let building = net.len() - built.len();
    let next = built
        .iter()
        .filter_map(|a| a.launcher.map(|l| (l, a.round_seconds)))
        .filter(|(l, _)| l.assembling())
        .map(|(l, secs)| (1.0 - l.progress) * secs)
        .fold(None, |m: Option<f32>, s| Some(m.map_or(s, |m| m.min(s))));

    let w = 262.0;
    let slots = built.len().min(CARD_ARRAYS);
    let rows_h = if slots > 0 { 34.0 } else { 0.0 };
    let h = 58.0 + rows_h + 40.0;
    let r = Rect::new(anchor.x + 26.0, anchor.y - 34.0, w, h);
    ui.frost(r, 0.72);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(INTERCEPT, 1.0));

    let x = r.x + 12.0;
    ui.text(
        x,
        r.y + 14.0,
        type_scale::MICRO,
        rgb(INTERCEPT, 0.95),
        "Interceptor network",
    );
    let arrays = match built.len() {
        0 => "No array standing".to_owned(),
        1 => "1 array".to_owned(),
        n => format!("{n} arrays"),
    };
    ui.text_right(
        r.right() - 10.0,
        r.y + 14.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.7),
        &arrays,
    );
    let count = format!("{ready}");
    let end = ui.text(
        x,
        r.y + 36.0,
        type_scale::VALUE,
        rgb(if ready > 0 { 0xFFFFFF } else { palette::BAD }, 1.0),
        &count,
    );
    ui.text(
        end + 6.0,
        r.y + 36.0,
        type_scale::BODY,
        rgb(palette::TEXT, 0.8),
        &format!("of {room} interceptors ready"),
    );

    let mut y = r.y + 50.0;
    if slots > 0 {
        // Each array's rounds, a group per array, filled as the silo panel fills them.
        let per = built[..slots]
            .iter()
            .map(|a| a.launcher.map_or(1, |l| l.capacity.max(1)))
            .max()
            .unwrap_or(1) as f32;
        let group_w = (w - 24.0 - (slots as f32 - 1.0) * 8.0) / slots as f32;
        let slot_w = (group_w / per).min(14.0);
        for (i, a) in built[..slots].iter().enumerate() {
            let gx = x + i as f32 * (group_w + 8.0);
            let Some(l) = a.launcher else { continue };
            for k in 0..l.capacity {
                let fill = if k < l.stock {
                    1.0
                } else if k == l.stock && l.assembling() {
                    l.progress
                } else {
                    0.0
                };
                let sr = Rect::new(gx + k as f32 * slot_w, y, slot_w, 22.0);
                silo::round_slot(ui, sr, fill, k < l.stock, INTERCEPT, false);
            }
            if a.selected {
                ui.fill(
                    Rect::new(gx, y + 25.0, l.capacity as f32 * slot_w, 1.5),
                    rgb(palette::LINE, 0.8),
                );
            }
        }
        y += rows_h;
    }
    let mut status = match next {
        Some(s) => format!("Next loaded in {}", super::clock(s)),
        None if room > 0 && ready == room => "Every cell loaded".to_owned(),
        None if room > 0 => "Assembly stopped".to_owned(),
        None => "Nothing covers the base from warheads".to_owned(),
    };
    if built.len() > CARD_ARRAYS {
        status += &format!("  \u{b7}  +{} arrays", built.len() - CARD_ARRAYS);
    }
    if building > 0 {
        status += &format!("  \u{b7}  {building} building");
    }
    ui.text(x, y, type_scale::MICRO, rgb(palette::TEXT, 0.75), &status);

    {
        let (cells, fresh) = adding;
        y += 16.0;
        ui.fill(
            Rect::new(x, y - 2.0, w - 24.0, 1.0),
            rgb(palette::LINE, 0.12),
        );
        let (text, tone) = if built.is_empty() {
            (
                format!("+{cells} interceptors  \u{b7}  first cover"),
                INTERCEPT,
            )
        } else if fresh < 0.02 {
            (
                format!("+{cells} interceptors  \u{b7}  no new ground"),
                palette::WARN,
            )
        } else {
            (
                format!(
                    "+{cells} interceptors  \u{b7}  {:.0}% new ground",
                    fresh * 100.0
                ),
                INTERCEPT,
            )
        };
        ui.text(x, y + 12.0, type_scale::VALUE, rgb(tone, 1.0), &text);
    }
}

/// The network, if an interceptor array is being placed at `placing` or one is selected.
pub(super) fn draw(ui: &mut Ui, field: &Field, placing: Option<(BlueprintId, Vec2)>) {
    let view = field.view;
    let site = placing.and_then(|(bp, at)| {
        let bp = field.blueprints.unit(bp);
        let s = bp
            .strategic
            .as_ref()
            .filter(|s| s.kind == StrategicKind::Interceptor)?;
        Some((at, s.coverage.to_f32(), s.stock as u32))
    });
    let team_of = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    // Whose network: ours while placing; the selected array's side otherwise.
    let team = if site.is_some() {
        team_of(view.local)
    } else {
        view.frame
            .units
            .iter()
            .filter(|u| view.selection.contains(&u.unit_id))
            .find(|u| {
                field
                    .blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .strategic
                    .as_ref()
                    .is_some_and(|s| s.kind == StrategicKind::Interceptor)
            })
            .and_then(|u| team_of(owner_of(u)))
    };
    let Some(team) = team else { return };
    let net = arrays(field, team);
    let circles: Vec<(Vec2, f32)> = net
        .iter()
        .filter(|a| a.covers())
        .map(|a| (a.at, a.cover))
        .collect();
    let t = ui.time;
    let placing_alpha = if site.is_some() { 0.75 } else { 0.7 };
    outline(
        ui,
        field,
        &circles,
        site.map(|(at, r, _)| (at, r)),
        INTERCEPT,
        placing_alpha,
        1.8,
    );
    // Arrays still going up: their cover to come, dashed, apart from the network.
    for a in net.iter().filter(|a| !a.covers()) {
        super::ground_ring(
            ui,
            field,
            a.at,
            a.cover,
            1.2,
            rgb(BUILDING, 0.45),
            true,
            t * 0.02,
        );
    }
    for a in &net {
        let ally = (a.owner != view.local).then(|| {
            view.status
                .players
                .get(a.owner as usize)
                .map_or("Ally", |p| p.name.as_str())
        });
        badge(ui, field, a, ally);
    }
    if let Some((at, r, cells)) = site {
        // The new site's cover against the network: bright where it adds ground, faint
        // and dashed where the network already reaches.
        let breathe = 0.85 + 0.15 * (t * 3.0).sin();
        let s = ui.s;
        let segments = ((r / 14.0) as usize).clamp(64, 320);
        let pt = |k: usize| {
            at + Vec2::from_angle(k as f32 / segments as f32 * std::f32::consts::TAU) * r
        };
        let scr = |p: Vec2| {
            field
                .camera
                .project(p.extend(surface(field, p) + 2.0))
                .map(|q| q / s)
        };
        for k in 0..segments {
            let (a, b) = (pt(k), pt(k + 1));
            let mid = (a + b) * 0.5;
            let covered = circles.iter().any(|&(c, cr)| mid.distance(c) < cr - 1.0);
            let (Some(pa), Some(pb)) = (scr(a), scr(b)) else {
                continue;
            };
            if covered {
                if k % 2 == 0 {
                    ui.stroke(pa, pb, 1.2, rgb(INTERCEPT, 0.35));
                }
            } else {
                ui.stroke(pa, pb, 2.4, rgb(0xE6FAFF, breathe));
            }
        }
        let fresh = new_ground(at, r, &circles);
        if let Some(g) = project(ui, field, at.extend(surface(field, at) + 2.0)) {
            card(ui, g, &net, (cells, fresh));
        }
    }
}

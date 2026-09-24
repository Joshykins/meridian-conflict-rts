//! Survival's panel, top centre: which round it is and how many are left,
//! what the engine is doing (counting down, replicating, sending the round),
//! what is coming by domain and tier, and the replication nodes standing,
//! the bonus objectives, each a tile that takes the camera to it.

use super::icons;
use super::style::{AIR, LAND, NAVY};
use super::{Hud, HudAction, Scene, EDGE, GAP};
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::BlueprintId;
use mc_sim::survival::Phase;

/// Replication violet: the engine, its ray, its nodes.
pub const VIOLET: u32 = 0xA070FF;
const W: f32 = 520.0;
const H: f32 = 68.0;
const NODE_W: f32 = 170.0;
const NODE_H: f32 = 40.0;
const DOMAINS: [(u32, &str); 3] = [(LAND, "Land"), (AIR, "Air"), (NAVY, "Naval")];

fn clock(secs: f32) -> String {
    let s = secs.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Draws the panel if this is a survival match. Returns the y below it.
pub fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene, top: f32) -> Option<f32> {
    let status = s.view.status.survival.as_ref()?;
    if !hud.survival_read {
        hud.survival_read = true;
        if let Some(layout) = crate::survival::layout(s.map) {
            hud.survival_sites = layout.node_sites.iter().map(|n| n.name.clone()).collect();
            hud.survival_fronts = layout
                .fronts
                .iter()
                .map(|f| (f.domain, f.path.iter().map(|p| Vec2::new(p.0, p.1)).collect()))
                .collect();
        }
    }
    let w = ui.size.x;
    // Top centre when the middle of the top edge is free, else under the economy.
    let x = ((w - W) * 0.5).max(EDGE);
    let r = Rect::new(x, top, W, H);
    hud.glass(ui, r);
    let t = ui.time;

    // The round: big, with how many there are.
    let round_w = 108.0;
    ui.text(r.x + 16.0, r.y + 18.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), "Round");
    let big = if status.round == 0 { "—".to_string() } else { status.round.to_string() };
    ui.text(r.x + 16.0, r.y + 44.0, type_scale::TITLE, rgb(palette::TEXT, 1.0), &big);
    let of = if status.rounds == 0 { "Endless".to_string() } else { format!("of {}", status.rounds) };
    let bw = ui.text_width(type_scale::TITLE, &big);
    ui.text(r.x + 22.0 + bw, r.y + 48.0, type_scale::CAPTION, rgb(palette::DIM, 1.0), &of);
    ui.vline(r.x + round_w, r.y + 10.0, r.h - 20.0, rgb(palette::LINE, 0.14));

    // What the engine is doing, and a bar for it.
    let px = r.x + round_w + 14.0;
    let pw = 196.0;
    let (label, value, share, tone) = match status.phase {
        Phase::Grace => (
            "First Contact In",
            clock(status.next_in as f32 * 0.1),
            None,
            palette::TEXT,
        ),
        Phase::Launched => (
            "Next Round In",
            clock(status.next_in as f32 * 0.1),
            None,
            palette::TEXT,
        ),
        Phase::Printing => {
            let (done, of) = status.printed;
            (
                "Replicating",
                format!("{done} / {of}"),
                Some(done as f32 / of.max(1) as f32),
                VIOLET,
            )
        }
        Phase::Final => ("Final Round  \u{b7}  Clear the Field", format!("{} Left", status.remaining), None, palette::WARN),
        Phase::Won => ("Held", "The Line Stands".to_string(), None, super::HEALTHY),
    };
    ui.text(px, r.y + 18.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
    ui.text(px, r.y + 38.0, type_scale::VALUE, rgb(tone, 1.0), &value);
    let bar = Rect::new(px, r.y + 52.0, pw, 3.0);
    ui.fill(bar, rgb(palette::LINE, 0.1));
    match share {
        Some(k) => {
            ui.fill(Rect::new(bar.x, bar.y, bar.w * k, bar.h), rgb(VIOLET, 0.95));
            // A print head running along the bar.
            let head = bar.x + bar.w * ((t * 0.7).fract());
            ui.fill(Rect::new(head, bar.y - 1.0, 2.0, bar.h + 2.0), rgb(0xFFFFFF, 0.5));
        }
        None if matches!(status.phase, Phase::Grace | Phase::Launched) => {
            // What is left of the wait, the last two and a half minutes of it.
            let urgent = status.next_in < 300;
            let pulse = if urgent { 0.6 + 0.4 * (t * 6.0).sin().abs() } else { 0.8 };
            let k = (status.next_in as f32 / 1500.0).min(1.0);
            ui.fill(
                Rect::new(bar.x + bar.w * (1.0 - k), bar.y, bar.w * k, bar.h),
                rgb(if urgent { palette::WARN } else { palette::TEXT }, pulse),
            );
        }
        _ => {}
    }
    // Tech the engine prints at.
    let chip = Rect::new(px + pw - 34.0, r.y + 12.0, 34.0, 18.0);
    ui.frame(chip, rgb(VIOLET, 0.6));
    ui.text_centred(chip.x + chip.w * 0.5, chip.mid_y(), type_scale::MICRO, rgb(VIOLET, 1.0), &format!("T{}", status.tier));
    ui.vline(px + pw + 12.0, r.y + 10.0, r.h - 20.0, rgb(palette::LINE, 0.14));

    // What is coming, by domain; hostiles in the field under it.
    let ix = px + pw + 26.0;
    let heading = if status.phase == Phase::Printing { "Inbound" } else { "Forecast" };
    ui.text(ix, r.y + 18.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), heading);
    let mut cx = ix;
    for (d, (tone, name)) in DOMAINS.iter().enumerate() {
        let n: u16 = status.incoming[d].iter().sum();
        let top_tier = status.incoming[d].iter().rposition(|&c| c > 0).map_or(1, |i| i + 1) as u8;
        let a = if n == 0 { 0.3 } else { 1.0 };
        let _ = name;
        let kind = match d {
            0 => mc_data::IconKind::Tank,
            1 => mc_data::IconKind::Fighter,
            _ => mc_data::IconKind::Ship,
        };
        icons::strategic(ui, kind, top_tier, Vec2::new(cx + 8.0, r.y + 38.0), 7.0, rgb(*tone, a), ink(0.9));
        let text = n.to_string();
        ui.text(cx + 20.0, r.y + 38.0, type_scale::VALUE, rgb(palette::TEXT, a), &text);
        cx += 30.0 + ui.text_width(type_scale::VALUE, &text);
    }
    ui.text(
        ix,
        r.y + 56.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!("{} hostile  \u{b7}  +{:.0}/s reclaim", status.hostile, status.reclaim),
    );

    // The nodes: bonus objectives.
    let mut bottom = r.bottom();
    if !status.nodes.is_empty() || status.nodes_destroyed > 0 {
        let y = r.bottom() + 6.0;
        let count = status.nodes.len().min(6);
        let strip_w = (count.max(1) as f32) * (NODE_W + 6.0) - 6.0;
        let sx = (w - strip_w.max(W)) * 0.5;
        // A line over the strip saying what it is.
        let label = format!(
            "Replication Nodes  \u{b7}  {} standing  \u{b7}  {} destroyed",
            status.nodes.len(),
            status.nodes_destroyed
        );
        let lw = ui.text_width(type_scale::MICRO, &label) + 16.0;
        ui.fill(Rect::new(sx, y, lw, 16.0), ink(0.6));
        ui.fill(Rect::new(sx, y, 2.0, 16.0), rgb(VIOLET, 0.9));
        ui.text(sx + 8.0, y + 8.0, type_scale::MICRO, rgb(VIOLET, 1.0), &label);
        let y = y + 18.0;
        for (i, n) in status.nodes.iter().take(6).enumerate() {
            let tr = Rect::new(sx + i as f32 * (NODE_W + 6.0), y, NODE_W, NODE_H);
            let tile = hud.tile(ui, id("survival-node", n.site as usize), tr, false, true);
            let bp = s.blueprints.unit(BlueprintId(n.product.0));
            let raising = n.raised < 1.0;
            let a = if raising { 0.55 + 0.45 * (t * 4.0).sin().abs() } else { 1.0 };
            // Diamond for the node, the unit it prints beside it.
            let c = Vec2::new(tr.x + 14.0, tr.mid_y());
            let d = 6.0;
            ui.triangle(c + Vec2::new(0.0, -d), c + Vec2::new(d, 0.0), c + Vec2::new(0.0, d), rgb(VIOLET, a));
            ui.triangle(c + Vec2::new(0.0, -d), c + Vec2::new(0.0, d), c + Vec2::new(-d, 0.0), rgb(VIOLET, a));
            let tone = match super::style::Domain::of(bp) {
                super::style::Domain::Air => AIR,
                super::style::Domain::Navy => NAVY,
                _ => LAND,
            };
            icons::strategic(ui, bp.visual.icon, bp.tech, Vec2::new(tr.x + 34.0, tr.mid_y()), 7.0, rgb(tone, 1.0), ink(0.9));
            let site = hud.survival_sites.get(n.site as usize).cloned().unwrap_or_else(|| format!("Site {}", n.site + 1));
            ui.text_fit_left(tr.x + 48.0, tr.y + 13.0, tr.w - 54.0, type_scale::CAPTION, rgb(palette::TEXT, 0.9 + 0.1 * tile.glow), &bp.name);
            let line = if raising {
                format!("{site}  \u{b7}  rising {:.0}%", n.raised * 100.0)
            } else {
                format!("{site}  \u{b7}  {} printed", n.printed)
            };
            ui.text_fit_left(tr.x + 48.0, tr.y + 29.0, tr.w - 54.0, type_scale::MICRO, rgb(palette::DIM, 1.0), &line);
            if raising {
                let b = Rect::new(tr.x + 4.0, tr.bottom() - 4.0, (tr.w - 8.0) * n.raised, 2.0);
                ui.fill(b, rgb(VIOLET, 0.9));
            }
            if tile.clicked {
                ui.audio.play(Sfx::Select);
                hud.actions.push(HudAction::LookAt(Vec2::new(n.pos[0], n.pos[1])));
            }
            bottom = tr.bottom();
        }
        if status.nodes.is_empty() {
            bottom = y;
        }
    }
    Some(bottom + GAP)
}

/// On the minimap: the fronts as flowing dashes toward the defenders, the
/// engine, and every node (pulsing while the ray raises it).
pub fn minimap(hud: &mut Hud, ui: &mut Ui, s: &Scene, at: &dyn Fn(Vec2) -> Vec2) {
    let Some(status) = s.view.status.survival.as_ref() else {
        return;
    };
    let t = ui.time;
    for (domain, path) in &hud.survival_fronts {
        let tone = match domain {
            mc_data::survival::Domain::Land => LAND,
            mc_data::survival::Domain::Air => AIR,
            mc_data::survival::Domain::Naval => NAVY,
        };
        let pts: Vec<Vec2> = path.iter().map(|p| at(*p)).collect();
        // Dashes that crawl along the path, the way the front comes.
        let mut run = -(t * 14.0) % 10.0;
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = a.distance(b);
            let dir = (b - a) / len.max(1e-3);
            let mut d = run;
            while d < len {
                let s0 = d.max(0.0);
                let s1 = (d + 5.0).min(len);
                if s1 > s0 {
                    ui.stroke(a + dir * s0, a + dir * s1, 1.4, rgb(tone, 0.55));
                }
                d += 10.0;
            }
            run = d - len;
        }
        if let (Some(&tip), Some(&before)) = (pts.last(), pts.get(pts.len().saturating_sub(2))) {
            let dir = (tip - before).normalize_or_zero();
            icons::arrow_head(ui, tip, dir, 5.0, 1.4, rgb(tone, 0.8));
        }
    }
    // The engine: a violet hexagon.
    let e = at(Vec2::new(status.engine[0], status.engine[1]));
    let r = 6.0;
    for k in 0..6 {
        let a0 = std::f32::consts::TAU * k as f32 / 6.0;
        let a1 = std::f32::consts::TAU * (k + 1) as f32 / 6.0;
        ui.stroke(e + Vec2::new(a0.cos(), a0.sin()) * r, e + Vec2::new(a1.cos(), a1.sin()) * r, 1.6, rgb(VIOLET, 1.0));
    }
    ui.disc(e, 2.2, rgb(VIOLET, 1.0));
    for n in &status.nodes {
        let c = at(Vec2::new(n.pos[0], n.pos[1]));
        let a = if n.raised < 1.0 { 0.4 + 0.6 * (t * 5.0).sin().abs() } else { 1.0 };
        let d = 4.0;
        ui.triangle(c + Vec2::new(0.0, -d), c + Vec2::new(d, 0.0), c + Vec2::new(0.0, d), rgb(VIOLET, a));
        ui.triangle(c + Vec2::new(0.0, -d), c + Vec2::new(0.0, d), c + Vec2::new(-d, 0.0), rgb(VIOLET, a));
        if n.raised < 1.0 {
            // The ray, from the engine to the site.
            ui.stroke(e, c, 1.0, rgb(VIOLET, 0.35 + 0.3 * a));
        }
    }
}

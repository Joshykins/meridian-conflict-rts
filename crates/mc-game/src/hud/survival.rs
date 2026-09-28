//! Survival's card, in the right column under the minimap: which round it is
//! and how many are left, what the Progenitor is doing (counting down,
//! forging, sending the round), what is coming by domain and tier, and the
//! Shapers standing, the bonus objectives, one row per site that takes the
//! camera to it.

use super::icons;
use super::style::{AIR, LAND, NAVY};
use super::{Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::BlueprintId;
use mc_sim::survival::{NodeStatus, Phase};

/// Replication violet: the engine, its ray, its nodes.
pub const VIOLET: u32 = 0xA070FF;
const PAD: f32 = 14.0;
const HEAD_H: f32 = 74.0;
const FORECAST_H: f32 = 50.0;
const ROW_H: f32 = 38.0;
const ROW_GAP: f32 = 4.0;
const DOMAINS: [u32; 3] = [LAND, AIR, NAVY];

fn clock(secs: f32) -> String {
    let s = secs.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn diamond(ui: &mut Ui, c: Vec2, d: f32, color: crate::ui::Color) {
    ui.triangle(
        c + Vec2::new(0.0, -d),
        c + Vec2::new(d, 0.0),
        c + Vec2::new(0.0, d),
        color,
    );
    ui.triangle(
        c + Vec2::new(0.0, -d),
        c + Vec2::new(0.0, d),
        c + Vec2::new(-d, 0.0),
        color,
    );
}

/// Draws the card if this is a survival match, `w` wide from (`right` - `w`,
/// `top`), never past `bottom`. Returns its rect.
pub fn draw(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    right: f32,
    top: f32,
    w: f32,
    bottom: f32,
) -> Option<Rect> {
    let status = s.view.status.survival.as_ref()?;
    if !hud.survival_read {
        hud.survival_read = true;
        if let Some(layout) = crate::survival::layout(s.map) {
            hud.survival_sites = layout.node_sites.iter().map(|n| n.name.clone()).collect();
            hud.survival_fronts = layout
                .fronts
                .iter()
                .map(|f| {
                    (
                        f.domain,
                        f.path.iter().map(|p| Vec2::new(p.0, p.1)).collect(),
                    )
                })
                .collect();
        }
    }
    let t = ui.time;

    // The Shapers by site, in the order they rose.
    let mut sites: Vec<(u8, Vec<&NodeStatus>)> = Vec::new();
    for n in &status.nodes {
        match sites.iter_mut().find(|(k, _)| *k == n.site) {
            Some((_, v)) => v.push(n),
            None => sites.push((n.site, vec![n])),
        }
    }
    let show_shapers = !status.nodes.is_empty() || status.nodes_destroyed > 0;
    let fixed = HEAD_H + FORECAST_H + if show_shapers { 30.0 } else { 0.0 } + PAD;
    let room = ((bottom - top - fixed) / (ROW_H + ROW_GAP))
        .floor()
        .max(1.0) as usize;
    let rows = if sites.is_empty() && show_shapers {
        1
    } else {
        sites.len().min(room)
    };
    let h = fixed + rows as f32 * (ROW_H + ROW_GAP) - if rows > 0 { ROW_GAP } else { 0.0 };
    let r = Rect::new(right - w, top, w, h);
    hud.glass(ui, r);
    let (x, cw) = (r.x + PAD, r.w - 2.0 * PAD);

    // The round, big, with how many there are.
    let y = r.y + 18.0;
    ui.text(x, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), "Round");
    let big = if status.round == 0 {
        "\u{2014}".to_string()
    } else {
        status.round.to_string()
    };
    let end = ui.text(
        x - 1.0,
        y + 26.0,
        type_scale::TITLE,
        rgb(palette::TEXT, 1.0),
        &big,
    );
    let of = if status.rounds == 0 {
        "Endless".to_string()
    } else {
        format!("of {}", status.rounds)
    };
    ui.text(
        end + 6.0,
        y + 31.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        &of,
    );

    // What the Progenitor is doing, right of it, with the tech it forges at.
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
                "Forging",
                format!("{done} / {of}"),
                Some(done as f32 / of.max(1) as f32),
                VIOLET,
            )
        }
        Phase::Final => (
            "Final Round",
            format!("{} Left", status.remaining),
            None,
            palette::WARN,
        ),
        Phase::Won => ("Held", "The Line Stands".to_string(), None, super::HEALTHY),
    };
    let rx = x + cw;
    let chip = Rect::new(rx - 26.0, y - 8.0, 26.0, 16.0);
    ui.frame(chip, rgb(VIOLET, 0.6));
    ui.text_centred(
        chip.x + chip.w * 0.5,
        chip.mid_y(),
        type_scale::MICRO,
        rgb(VIOLET, 1.0),
        &format!("T{}", status.tier),
    );
    ui.text_right(
        chip.x - 8.0,
        y,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        label,
    );
    let urgent = matches!(status.phase, Phase::Grace | Phase::Launched) && status.next_in < 300;
    let pulse = if urgent {
        0.6 + 0.4 * (t * 6.0).sin().abs()
    } else {
        1.0
    };
    let value_tone = if urgent { palette::WARN } else { tone };
    ui.text_right(
        rx,
        y + 27.0,
        type_scale::VALUE,
        rgb(value_tone, pulse),
        &value,
    );

    // A bar the width of the card: the forge filling, or the wait running out
    // (its last two and a half minutes).
    let bar = Rect::new(x, r.y + HEAD_H - 14.0, cw, 3.0);
    ui.fill(bar, rgb(palette::LINE, 0.1));
    match share {
        Some(k) => {
            ui.fill(Rect::new(bar.x, bar.y, bar.w * k, bar.h), rgb(VIOLET, 0.95));
            let head = bar.x + bar.w * k * (t * 0.7).fract();
            ui.fill(
                Rect::new(head, bar.y - 1.0, 2.0, bar.h + 2.0),
                rgb(0xFFFFFF, 0.5),
            );
        }
        None if matches!(status.phase, Phase::Grace | Phase::Launched) => {
            let k = (status.next_in as f32 / 1500.0).min(1.0);
            ui.fill(
                Rect::new(bar.x + bar.w * (1.0 - k), bar.y, bar.w * k, bar.h),
                rgb(
                    if urgent { palette::WARN } else { palette::TEXT },
                    if urgent { pulse } else { 0.8 },
                ),
            );
        }
        _ => {}
    }

    // What is coming, by domain, and what is out there now.
    let y = r.y + HEAD_H + 4.0;
    let heading = if status.phase == Phase::Printing {
        "Inbound"
    } else {
        "Forecast"
    };
    ui.section(x, y, cw, heading);
    let y = y + 22.0;
    let mut cx = x;
    for (d, tone) in DOMAINS.iter().enumerate() {
        let n: u16 = status.incoming[d].iter().sum();
        let top_tier = status.incoming[d]
            .iter()
            .rposition(|&c| c > 0)
            .map_or(1, |i| i + 1) as u8;
        let a = if n == 0 { 0.3 } else { 1.0 };
        let kind = match d {
            0 => mc_data::IconKind::Tank,
            1 => mc_data::IconKind::Fighter,
            _ => mc_data::IconKind::Ship,
        };
        icons::strategic(
            ui,
            kind,
            top_tier,
            Vec2::new(cx + 7.0, y),
            7.0,
            rgb(*tone, a),
            ink(0.9),
        );
        let text = n.to_string();
        let end = ui.text(
            cx + 19.0,
            y,
            type_scale::VALUE,
            rgb(palette::TEXT, a),
            &text,
        );
        cx = end + 14.0;
    }
    // A diamond for each Shaper that rises with the round.
    for k in 0..(status.shapers_next as usize).min(4) {
        diamond(
            ui,
            Vec2::new(cx + 5.0 + k as f32 * 12.0, y),
            5.0,
            rgb(VIOLET, 0.95),
        );
    }
    if status.shapers_next > 4 {
        ui.text(
            cx + 52.0,
            y,
            type_scale::MICRO,
            rgb(VIOLET, 1.0),
            &format!("+{}", status.shapers_next - 4),
        );
    }
    let reclaim_tone = if status.reclaim >= 0.5 {
        super::HEALTHY
    } else {
        palette::DIM
    };
    ui.text_right(
        rx,
        y - 6.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!("{} in the field", status.hostile),
    );
    ui.text_right(
        rx,
        y + 8.0,
        type_scale::MICRO,
        rgb(reclaim_tone, 0.9),
        &format!("+{:.0}/s reclaim", status.reclaim),
    );

    // The Shapers: bonus objectives, one row per site.
    if show_shapers {
        let y = r.y + HEAD_H + FORECAST_H + 8.0;
        let count = format!(
            "{} standing  \u{b7}  {} down",
            status.nodes.len(),
            status.nodes_destroyed
        );
        let count_w = ui.text_width(type_scale::MICRO, &count);
        ui.section(x, y, cw - count_w - 10.0, "Shapers");
        ui.text_right(rx, y, type_scale::MICRO, rgb(VIOLET, 1.0), &count);
        let mut ry = y + 16.0;
        if sites.is_empty() {
            ui.text(
                x,
                ry + ROW_H * 0.5,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                "None standing. The next rises with a round.",
            );
        }
        let hidden = sites.len().saturating_sub(rows);
        for (i, (site, nodes)) in sites.iter().take(rows).enumerate() {
            let tr = Rect::new(x - 4.0, ry, cw + 8.0, ROW_H);
            ry += ROW_H + ROW_GAP;
            if hidden > 0 && i + 1 == rows {
                // No room for the rest: say how many.
                let more = format!("+{} more sites", hidden + 1);
                ui.text(
                    tr.x + 10.0,
                    tr.mid_y(),
                    type_scale::MICRO,
                    rgb(palette::DIM, 1.0),
                    &more,
                );
                break;
            }
            let tile = hud.tile(ui, id("survival-site", *site as usize), tr, false, true);
            let raised = nodes.iter().map(|n| n.raised).sum::<f32>() / nodes.len() as f32;
            let raising = raised < 1.0;
            let a = if raising {
                0.55 + 0.45 * (t * 4.0).sin().abs()
            } else {
                1.0
            };
            diamond(ui, Vec2::new(tr.x + 13.0, tr.y + 13.0), 5.0, rgb(VIOLET, a));
            // What each Shaper here prints, as icons on the right.
            let mut products: Vec<(BlueprintId, usize)> = Vec::new();
            for n in nodes {
                match products.iter_mut().find(|(p, _)| *p == n.product) {
                    Some((_, c)) => *c += 1,
                    None => products.push((n.product, 1)),
                }
            }
            let mut ix = tr.right() - 14.0;
            for (p, _) in products.iter().rev() {
                let bp = s.blueprints.unit(BlueprintId(p.0));
                let tone = match super::style::Domain::of(bp) {
                    super::style::Domain::Air => AIR,
                    super::style::Domain::Navy => NAVY,
                    _ => LAND,
                };
                icons::strategic(
                    ui,
                    bp.visual.icon,
                    bp.tech,
                    Vec2::new(ix, tr.y + 13.0),
                    6.0,
                    rgb(tone, 1.0),
                    ink(0.9),
                );
                ix -= 18.0;
            }
            let name = hud
                .survival_sites
                .get(*site as usize)
                .cloned()
                .unwrap_or_else(|| format!("Site {}", site + 1));
            ui.text_fit_left(
                tr.x + 26.0,
                tr.y + 13.0,
                ix - tr.x - 30.0,
                type_scale::CAPTION,
                rgb(palette::TEXT, 0.9 + 0.1 * tile.glow),
                &name,
            );
            let line = if raising {
                format!("Rising  {:.0}%", raised * 100.0)
            } else {
                let names: Vec<String> = products
                    .iter()
                    .map(|(p, c)| {
                        let n = &s.blueprints.unit(BlueprintId(p.0)).name;
                        if *c > 1 {
                            format!("{n} \u{d7}{c}")
                        } else {
                            n.clone()
                        }
                    })
                    .collect();
                let printed: u32 = nodes.iter().map(|n| n.printed).sum();
                format!("{}  \u{b7}  {printed} printed", names.join(", "))
            };
            ui.text_fit_left(
                tr.x + 26.0,
                tr.y + 28.0,
                tr.w - 36.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &line,
            );
            if raising {
                ui.fill(
                    Rect::new(tr.x + 6.0, tr.bottom() - 4.0, (tr.w - 12.0) * raised, 2.0),
                    rgb(VIOLET, 0.9),
                );
            }
            if tile.clicked {
                ui.audio.play(Sfx::Select);
                let n = nodes[nodes.len() / 2];
                hud.actions
                    .push(HudAction::LookAt(Vec2::new(n.pos[0], n.pos[1])));
            }
        }
    }
    Some(r)
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
        // Dashes that crawl along the path, the way the front comes: the
        // first dash starts up to one period behind the path's start and
        // moves forward with time.
        let mut run = (t * 14.0) % 10.0 - 10.0;
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
        ui.stroke(
            e + Vec2::new(a0.cos(), a0.sin()) * r,
            e + Vec2::new(a1.cos(), a1.sin()) * r,
            1.6,
            rgb(VIOLET, 1.0),
        );
    }
    ui.disc(e, 2.2, rgb(VIOLET, 1.0));
    for n in &status.nodes {
        let c = at(Vec2::new(n.pos[0], n.pos[1]));
        let a = if n.raised < 1.0 {
            0.4 + 0.6 * (t * 5.0).sin().abs()
        } else {
            1.0
        };
        let d = 4.0;
        ui.triangle(
            c + Vec2::new(0.0, -d),
            c + Vec2::new(d, 0.0),
            c + Vec2::new(0.0, d),
            rgb(VIOLET, a),
        );
        ui.triangle(
            c + Vec2::new(0.0, -d),
            c + Vec2::new(0.0, d),
            c + Vec2::new(-d, 0.0),
            rgb(VIOLET, a),
        );
        if n.raised < 1.0 {
            // The ray, from the engine to the site.
            ui.stroke(e, c, 1.0, rgb(VIOLET, 0.35 + 0.3 * a));
        }
    }
}

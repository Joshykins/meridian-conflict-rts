//! Survival's card, in the right column under the minimap: which round it is
//! and how many are left, what the Progenitor is doing (counting down,
//! forging, sending the round), what is coming by domain and tier, its heavy
//! wave or the next tier, and the Shapers, the bonus objectives: one row per
//! site, its Shaper's level, tier and kind (a click selects it and looks at it),
//! or the round an empty site is reseeded in. A new tier also gets a banner
//! across the middle of the screen.

use super::icons;
use super::style::{AIR, LAND, NAVY};
use super::{Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, style, type_scale, Rect, Style, Ui};
use glam::Vec2;
use mc_data::survival::SiteKind;
use mc_data::BlueprintId;
use mc_sim::survival::{NodeStatus, Phase};

/// Replication violet: the engine, its ray, its nodes.
pub const VIOLET: u32 = 0xA070FF;
/// Heavy waves (T4/T5): pale gold, as on the set-up screen's forecast.
const HEAVY: u32 = 0xFFD27A;
const PAD: f32 = 14.0;
const HEAD_H: f32 = 74.0;
const FORECAST_H: f32 = 66.0;
/// The tier banner's title.
const BANNER: Style = style(mc_render::Face::Light, 64.0, 10.0);
/// Seconds the banner shows, and the T chip pulses after a change.
const BANNER_SECS: f32 = 5.5;
const CHIP_SECS: f32 = 10.0;
const ROW_H: f32 = 38.0;
const ROW_GAP: f32 = 4.0;
const DOMAINS: [u32; 3] = [LAND, AIR, NAVY];

/// Survival's part of the HUD: what it read from the map, its card this
/// frame, and the tier banner while it shows.
#[derive(Default)]
pub struct SurvivalHud {
    /// Names of the map's node sites, by site index.
    pub sites: Vec<String>,
    /// Where each site's Shaper stands, by site index.
    site_at: Vec<Vec2>,
    /// The map's fronts (domain, path), read once with the sites.
    fronts: Vec<(mc_data::survival::Domain, Vec<Vec2>)>,
    read: bool,
    /// The card this frame, which the range key keeps left of.
    pub(super) card: Option<Rect>,
    /// The tier the banner announces, and when it started (`Ui::time`; None until first drawn).
    banner: Option<(u8, Option<f32>)>,
    /// The tier the chip shows and when it changed, for its pulse.
    chip: (u8, f32),
    /// The Shaper whose selection card the player closed: it stays closed until
    /// another is selected (`selection::shaper`).
    pub(super) shaper_closed: Option<u32>,
    /// The Shaper's rings the card points at (`rings::SHAPER_*` bits), last frame's.
    pub(crate) shaper_focus: u8,
}

impl SurvivalHud {
    /// Puts the banner up for a tier the engine has just reached.
    pub fn announce_tier(&mut self, tier: u8) {
        self.banner = Some((tier, None));
    }
}

/// What a new tier means, under its banner.
fn tier_line(tier: u8) -> &'static str {
    match tier {
        1 => "The Progenitor prints tech 1",
        2 => "The Progenitor prints tech 2",
        3 => "The Progenitor prints tech 3",
        4 => "Experimentals  \u{b7}  heavy waves from now on",
        _ => "Tech 5  \u{b7}  titans join the heavy waves",
    }
}

/// The tier banner across the middle of the screen: fades in, holds, fades out.
fn banner(hud: &mut Hud, ui: &mut Ui) {
    let Some((tier, start)) = hud.survival.banner else {
        return;
    };
    let t = ui.time;
    let start = start.unwrap_or(t);
    hud.survival.banner = Some((tier, Some(start)));
    let age = t - start;
    if age > BANNER_SECS {
        hud.survival.banner = None;
        return;
    }
    // It comes in already part-lit, so it is there on its first frame.
    let a = (0.3 + age / 0.35).min(1.0) * ((BANNER_SECS - age) / 1.2).min(1.0);
    let cx = ui.size.x * 0.5;
    let y = ui.size.y * 0.27;
    let title = format!("TIER {tier}");
    let tone = if tier >= 4 { HEAVY } else { VIOLET };
    // The rule either side opens out from the middle as it comes in.
    let half = ui.text_width(BANNER, &title) * 0.5 + 28.0;
    let reach = 220.0 * (age / 0.6).min(1.0);
    ui.fill(
        Rect::new(cx - half - reach, y - 1.0, reach, 2.0),
        rgb(tone, 0.8 * a),
    );
    ui.fill(
        Rect::new(cx + half, y - 1.0, reach, 2.0),
        rgb(tone, 0.8 * a),
    );
    ui.text_centred(cx, y + 1.0, BANNER, rgb(tone, a), &title);
    ui.text_centred(
        cx,
        y + 46.0,
        type_scale::OVERLINE,
        rgb(palette::TEXT, 0.9 * a),
        tier_line(tier),
    );
}

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
    banner(hud, ui);
    if !hud.survival.read {
        hud.survival.read = true;
        if let Some(layout) = crate::survival::layout(s.map) {
            hud.survival.sites = layout.node_sites.iter().map(|n| n.name.clone()).collect();
            hud.survival.site_at = layout
                .node_sites
                .iter()
                .map(|n| Vec2::new(n.at.0, n.at.1))
                .collect();
            hud.survival.fronts = layout
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

    // One row per site: its Shaper, in the order they rose, then the sites
    // standing empty until the ray reseeds them.
    let sites: Vec<Site> = status
        .nodes
        .iter()
        .map(Site::Standing)
        .chain(
            status
                .reseeding
                .iter()
                .map(|&(site, round)| Site::Empty { site, round }),
        )
        .collect();
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
    // The tier chip: it lights up and pulses for a while after the tier changes.
    if hud.survival.chip.0 != status.tier {
        hud.survival.chip = (
            status.tier,
            if hud.survival.chip.0 == 0 {
                -CHIP_SECS
            } else {
                t
            },
        );
    }
    let fresh = (1.0 - (t - hud.survival.chip.1) / CHIP_SECS).clamp(0.0, 1.0);
    let chip_tone = if status.tier >= 4 { HEAVY } else { VIOLET };
    let chip = Rect::new(rx - 26.0, y - 8.0, 26.0, 16.0);
    if fresh > 0.0 {
        let beat = 0.5 + 0.5 * (t * 5.0).sin().abs();
        ui.fill(chip, rgb(chip_tone, 0.35 * fresh * beat));
    }
    ui.frame(chip, rgb(chip_tone, 0.6 + 0.4 * fresh));
    ui.text_centred(
        chip.x + chip.w * 0.5,
        chip.mid_y(),
        type_scale::MICRO,
        rgb(chip_tone, 1.0),
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
    if status.held {
        // The field cap holds the printing: say so, not just slow down.
        let pulse = 0.6 + 0.4 * (t * 3.0).sin().abs();
        ui.text_right(
            rx,
            y - 6.0,
            type_scale::MICRO,
            rgb(palette::WARN, pulse),
            &format!("Field full ({}): printing waits", status.hostile),
        );
    } else {
        ui.text_right(
            rx,
            y - 6.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("{} in the field", status.hostile),
        );
    }
    ui.text_right(
        rx,
        y + 8.0,
        type_scale::MICRO,
        rgb(reclaim_tone, 0.9),
        &format!("+{:.0}/s reclaim", status.reclaim),
    );
    // Under it: the round's heavy wave, squad by squad, else when the next tier comes.
    let y = y + 18.0;
    if !status.heavy.is_empty() {
        let squads: Vec<String> = status
            .heavy
            .iter()
            .map(|(b, n)| format!("{n} \u{d7} {}", s.blueprints.unit(*b).name))
            .collect();
        let end = ui.text(x, y, type_scale::MICRO, rgb(HEAVY, 1.0), "Heavy wave");
        ui.text_fit_left(
            end + 8.0,
            y,
            rx - end - 8.0,
            type_scale::MICRO,
            rgb(palette::TEXT, 0.9),
            &squads.join("  \u{b7}  "),
        );
    } else if let Some((tier, round)) = status.next_tier {
        let next = if status.phase == Phase::Printing {
            status.round
        } else {
            status.round + 1
        };
        let k = round.saturating_sub(next);
        let when = match k {
            0 => "next round".to_string(),
            1 => "in 1 round".to_string(),
            k => format!("in {k} rounds"),
        };
        let tone = if tier >= 4 { HEAVY } else { VIOLET };
        ui.text(
            x,
            y,
            type_scale::MICRO,
            rgb(tone, if k == 0 { 1.0 } else { 0.75 }),
            &format!("T{tier} {when}  \u{b7}  round {round}"),
        );
    }

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
        for (i, site) in sites.iter().take(rows).enumerate() {
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
            site_row(hud, ui, s, site, status.catch_up, tr);
        }
    }
    Some(r)
}

/// A row of the Shapers section: a site's Shaper, or a site standing empty.
enum Site<'a> {
    Standing(&'a NodeStatus),
    /// Its Shaper was destroyed: empty until the ray reseeds it in `round`.
    Empty {
        site: u8,
        round: u16,
    },
}

/// What a site kind's Shaper does, short.
fn kind_word(kind: SiteKind) -> &'static str {
    match kind {
        SiteKind::Printer => "Printer",
        SiteKind::Artillery => "Artillery",
        SiteKind::Nuke => "Nuke",
    }
}

/// The line under a site's name: rising, its level (and the level it is evolving
/// to), tier and kind, and what it has printed; or when an empty site is reseeded.
fn site_line(site: &Site, catch_up: u16) -> String {
    match site {
        Site::Standing(n) if n.raised < 1.0 => format!("Rising  {:.0}%", n.raised * 100.0),
        Site::Standing(n) => {
            let level = if n.level < catch_up {
                format!("Level {} \u{2192} {catch_up}", n.level)
            } else {
                format!("Level {}", n.level)
            };
            let mut line = format!(
                "{level}  \u{b7}  T{}  \u{b7}  {}",
                n.tier,
                kind_word(n.kind)
            );
            if n.kind == SiteKind::Printer {
                line.push_str(&format!("  \u{b7}  {} printed", n.printed));
            }
            line
        }
        Site::Empty { round, .. } => format!("Empty  \u{b7}  reseeds round {round}"),
    }
}

/// One site's row in `tr`: its mark, its name, what it prints, its state. A
/// Shaper's row selects it and looks at it; an empty site's looks at the site.
fn site_row(hud: &mut Hud, ui: &mut Ui, s: &Scene, site: &Site, catch_up: u16, tr: Rect) {
    let t = ui.time;
    let (index, node) = match site {
        Site::Standing(n) => (n.site, Some(*n)),
        Site::Empty { site, .. } => (*site, None),
    };
    let tile = hud.tile(ui, id("survival-site", index as usize), tr, false, true);
    let raising = node.is_some_and(|n| n.raised < 1.0);
    let a = if raising {
        0.55 + 0.45 * (t * 4.0).sin().abs()
    } else if node.is_none() {
        0.35
    } else {
        1.0
    };
    // Its mark: a printer's in the colour of the domain it prints for, an
    // auxiliary Shaper's in the engine's violet (a nuke's in the heavy gold).
    let tone = match node.map(|n| (n.kind, n.domain)) {
        Some((SiteKind::Printer, mc_data::survival::Domain::Land)) => LAND,
        Some((SiteKind::Printer, mc_data::survival::Domain::Air)) => AIR,
        Some((SiteKind::Printer, mc_data::survival::Domain::Naval)) => NAVY,
        Some((SiteKind::Nuke, _)) => HEAVY,
        _ => VIOLET,
    };
    let c = Vec2::new(tr.x + 13.0, tr.y + 13.0);
    if node.is_some() {
        diamond(ui, c, 5.0, rgb(tone, a));
    } else {
        // An empty site: the diamond's outline.
        for (p, q) in [
            (0.0, -1.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (-1.0, 0.0),
            (0.0, -1.0),
        ]
        .windows(2)
        .map(|w| (Vec2::from(w[0]), Vec2::from(w[1])))
        {
            ui.stroke(c + p * 5.0, c + q * 5.0, 1.2, rgb(VIOLET, 0.6));
        }
    }
    // What it prints, the newest kinds, as icons on the right.
    let mut ix = tr.right() - 14.0;
    for p in node
        .map_or(&[][..], |n| &n.products[..])
        .iter()
        .rev()
        .take(4)
    {
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
        .survival
        .sites
        .get(index as usize)
        .cloned()
        .unwrap_or_else(|| format!("Site {}", index + 1));
    ui.text_fit_left(
        tr.x + 26.0,
        tr.y + 13.0,
        ix - tr.x - 30.0,
        type_scale::CAPTION,
        rgb(
            palette::TEXT,
            if node.is_some() { 0.9 } else { 0.5 } + 0.1 * tile.glow,
        ),
        &name,
    );
    let catching = node.is_some_and(|n| n.raised >= 1.0 && n.level < catch_up);
    ui.text_fit_left(
        tr.x + 26.0,
        tr.y + 28.0,
        tr.w - 36.0,
        type_scale::MICRO,
        rgb(if catching { VIOLET } else { palette::DIM }, 1.0),
        &site_line(site, catch_up),
    );
    if let Some(n) = node.filter(|n| n.raised < 1.0) {
        ui.fill(
            Rect::new(tr.x + 6.0, tr.bottom() - 4.0, (tr.w - 12.0) * n.raised, 2.0),
            rgb(VIOLET, 0.9),
        );
    }
    if tile.clicked {
        ui.audio.play(Sfx::Select);
        match node {
            Some(n) => hud.actions.push(HudAction::Select {
                units: vec![n.unit.0],
                focus: true,
            }),
            None => {
                if let Some(at) = hud.survival.site_at.get(index as usize) {
                    hud.actions.push(HudAction::LookAt(*at));
                }
            }
        }
    }
}

/// On the minimap: the fronts as flowing dashes toward the defenders, the
/// engine, and every node (pulsing while the ray raises it).
pub fn minimap(hud: &mut Hud, ui: &mut Ui, s: &Scene, at: &dyn Fn(Vec2) -> Vec2) {
    let Some(status) = s.view.status.survival.as_ref() else {
        return;
    };
    let t = ui.time;
    for (domain, path) in &hud.survival.fronts {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn node(level: u16, raised: f32, kind: SiteKind) -> NodeStatus {
        NodeStatus {
            unit: mc_sim::Handle(7),
            site: 2,
            kind,
            pos: [0.0; 2],
            domain: mc_data::survival::Domain::Land,
            products: Vec::new(),
            level,
            tier: mc_sim::survival::shaper_tier(level),
            raised,
            printed: 40,
        }
    }

    #[test]
    fn a_site_row_says_how_its_shaper_stands() {
        let rising = node(1, 0.4, SiteKind::Printer);
        assert_eq!(site_line(&Site::Standing(&rising), 1), "Rising  40%");
        let up = node(9, 1.0, SiteKind::Printer);
        assert_eq!(
            site_line(&Site::Standing(&up), 6),
            "Level 9  \u{b7}  T3  \u{b7}  Printer  \u{b7}  40 printed"
        );
        // Below the round's level it is evolving up to it.
        let behind = node(3, 1.0, SiteKind::Artillery);
        assert_eq!(
            site_line(&Site::Standing(&behind), 6),
            "Level 3 \u{2192} 6  \u{b7}  T1  \u{b7}  Artillery"
        );
        assert_eq!(
            site_line(&Site::Empty { site: 1, round: 9 }, 6),
            "Empty  \u{b7}  reseeds round 9"
        );
    }
}

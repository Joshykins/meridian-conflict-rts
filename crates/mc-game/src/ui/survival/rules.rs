//! Survival's rules: rounds, intensity, timing, the fronts that attack, the
//! tech ceiling and the Shapers, with the forecast of the rounds under them.
//! Survival's set-up and the co-op lobby both draw it.

use super::marks::*;
use crate::audio::Sfx;
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::survival::Domain;
use mc_sim::SurvivalRules;

/// Domains that will attack: switched on, and with lanes (`counts`) on the theatre.
pub fn attacking(rules: &SurvivalRules, counts: [usize; 3]) -> Vec<Domain> {
    Domain::ALL
        .into_iter()
        .zip(counts)
        .filter(|(d, n)| *n > 0 && rules.has(*d))
        .map(|(d, _)| d)
        .collect()
}

/// Flips a domain's fronts. Refused (false) when the domain has no lanes
/// here or it is the last one attacking.
pub fn toggle_front(rules: &mut SurvivalRules, counts: [usize; 3], d: Domain) -> bool {
    if counts[domain_index(d)] == 0 {
        return false;
    }
    if rules.has(d) && attacking(rules, counts) == vec![d] {
        return false;
    }
    rules.fronts ^= d.bit();
    true
}

/// Keeps the rules possible on a theatre with these lanes: tiers and Shapers
/// in range, and at least one front that attacks.
pub fn settle(rules: &mut SurvivalRules, counts: [usize; 3]) {
    rules.tier_cap = rules.tier_cap.clamp(1, 5);
    rules.nodes = rules.nodes.min(3);
    if attacking(rules, counts).is_empty() {
        for (d, n) in Domain::ALL.iter().zip(counts) {
            if n > 0 {
                rules.fronts |= d.bit();
            }
        }
    }
}

/// Rounds, and how long: "12 Rounds", "Endless".
pub fn rounds_label(rules: &SurvivalRules) -> String {
    if rules.rounds == 0 {
        "Endless".to_owned()
    } else {
        format!("{} Rounds", rules.rounds)
    }
}

const ROUNDS: [u16; 7] = [5, 10, 15, 20, 30, 50, 0];
const ROUND_LABELS: [&str; 7] = ["5", "10", "15", "20", "30", "50", "Endless"];
const INTENSITY: [(&str, u16); 5] = [
    ("Skirmish", 600),
    ("Standard", 1000),
    ("Siege", 1500),
    ("Onslaught", 2200),
    ("Annihilation", 3200),
];
const GRACE: [(&str, u16); 4] = [
    ("2 min", 120),
    ("4 min", 240),
    ("6 min", 360),
    ("8 min", 480),
];
const INTERVAL: [(&str, u16); 3] = [("1.5 min", 90), ("2.5 min", 150), ("4 min", 240)];
const NODES: [&str; 4] = ["Off", "Rare", "Regular", "Frequent"];
const TIER_NAMES: [&str; 5] = ["Light", "Main Line", "Heavy", "Experimental", "Titan"];

fn nearest(values: impl Iterator<Item = u16>, v: u16) -> usize {
    values
        .enumerate()
        .min_by_key(|(_, x)| (*x as i32 - v as i32).abs())
        .map_or(0, |(i, _)| i)
}

const VALUE_W: f32 = 200.0;
const PITCH: f32 = 40.0;

fn pick_row(
    ui: &mut Ui,
    key: &str,
    r: Rect,
    label: &str,
    hint: &str,
    options: &[&str],
    selected: usize,
) -> Option<usize> {
    label_row(ui, r, label, hint);
    ui.dropdown(
        id(key, 0),
        Rect::new(r.right() - VALUE_W, r.mid_y() - 15.0, VALUE_W, 30.0),
        options,
        selected,
        true,
    )
}

/// The rules panel in `area`, with the forecast in what height is left. `counts`
/// are the theatre's lanes by domain; `hover` is set to a fronts chip under the pointer.
pub fn engagement(
    ui: &mut Ui,
    rules: &mut SurvivalRules,
    counts: [usize; 3],
    hover: &mut Option<Domain>,
    area: Rect,
) {
    ui.section(area.x, area.y + 6.0, area.w, "Engagement");
    let row = |k: f32| Rect::new(area.x, area.y + 22.0 + k * PITCH, area.w, PITCH - 4.0);

    let at = ROUNDS
        .iter()
        .position(|r| *r == rules.rounds)
        .unwrap_or_else(|| nearest(ROUNDS[..6].iter().copied(), rules.rounds));
    let hint = if rules.rounds == 0 {
        "Until you fall"
    } else {
        "Survive them all to win"
    };
    if let Some(i) = pick_row(ui, "sv-rounds", row(0.0), "Rounds", hint, &ROUND_LABELS, at) {
        rules.rounds = ROUNDS[i];
    }
    let at = nearest(INTENSITY.iter().map(|l| l.1), rules.intensity);
    let hint = format!("\u{d7}{:.1} round size", rules.intensity as f32 / 1000.0);
    let labels = INTENSITY.map(|l| l.0);
    if let Some(i) = pick_row(
        ui,
        "sv-intensity",
        row(1.0),
        "Intensity",
        &hint,
        &labels,
        at,
    ) {
        rules.intensity = INTENSITY[i].1;
    }
    let at = nearest(GRACE.iter().map(|l| l.1), rules.grace_secs);
    let labels = GRACE.map(|l| l.0);
    if let Some(i) = pick_row(
        ui,
        "sv-grace",
        row(2.0),
        "First Contact",
        "Time to build before round 1",
        &labels,
        at,
    ) {
        rules.grace_secs = GRACE[i].1;
    }
    let at = nearest(INTERVAL.iter().map(|l| l.1), rules.interval_secs);
    let labels = INTERVAL.map(|l| l.0);
    if let Some(i) = pick_row(
        ui,
        "sv-interval",
        row(3.0),
        "Between Rounds",
        "",
        &labels,
        at,
    ) {
        rules.interval_secs = INTERVAL[i].1;
    }
    // The economy, said plainly: the waves are the income.
    let r = row(4.0);
    ui.fill(
        Rect::new(r.x, r.y + 4.0, 2.0, r.h - 8.0),
        rgb(crate::hud::MASS, 0.8),
    );
    let note = "No mass is handed out: every unit the engine sends leaves a wreck worth most of its cost - reclaim the field.";
    for (k, l) in ui
        .wrap(type_scale::MICRO, note, r.w - 24.0)
        .iter()
        .take(2)
        .enumerate()
    {
        ui.text(
            r.x + 14.0,
            r.y + 11.0 + k as f32 * 15.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            l,
        );
    }

    // Fronts: a chip per domain, with the theatre's lanes of it.
    let y = area.y + 22.0 + 5.0 * PITCH + 14.0;
    ui.text(
        area.x + 2.0,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Attack Fronts",
    );
    ui.text_right(
        area.right(),
        y,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "Hover to see the lanes",
    );
    let gap = 10.0;
    let cw = (area.w - 2.0 * gap) / 3.0;
    *hover = None;
    for (k, d) in Domain::ALL.into_iter().enumerate() {
        let r = Rect::new(area.x + k as f32 * (cw + gap), y + 12.0, cw, 50.0);
        let lanes = counts[k];
        let on = rules.has(d) && lanes > 0;
        let res = ui.tile(id("sv-front", k), r, on, lanes > 0);
        if res.hovered {
            *hover = Some(d);
        }
        if res.clicked {
            if toggle_front(rules, counts, d) {
                ui.audio.play(if rules.has(d) {
                    Sfx::ToggleOn
                } else {
                    Sfx::ToggleOff
                });
            } else {
                ui.audio.play(Sfx::Deny);
            }
        }
        let a = if lanes == 0 {
            0.3
        } else if on {
            1.0
        } else {
            0.5
        };
        ui.fill(Rect::new(r.x + 1.0, r.y + 7.0, 2.0, r.h - 14.0), dcol(d, a));
        ui.gradient_h(
            Rect::new(r.x + 3.0, r.y + 3.0, r.w * 0.6, r.h - 6.0),
            dcol(d, 0.16 * a * res.glow.max(on as u8 as f32)),
            dcol(d, 0.0),
        );
        domain_glyph(
            ui,
            d,
            Vec2::new(r.x + 24.0, r.mid_y() - 2.0),
            9.0,
            dcol(d, a),
        );
        ui.text(
            r.x + 44.0,
            r.y + 18.0,
            type_scale::ITEM,
            rgb(palette::TEXT, 0.35 + 0.65 * a),
            d.label(),
        );
        let sub = match (lanes, on) {
            (0, _) => "No lanes here".to_owned(),
            (n, true) => format!("{n} lane{}", if n == 1 { "" } else { "s" }),
            (_, false) => "Off".to_owned(),
        };
        ui.text(
            r.x + 44.0,
            r.y + 36.0,
            type_scale::MICRO,
            rgb(if on { palette::DIM } else { palette::FAINT }, 1.0),
            &sub,
        );
        // A check box in the corner.
        let b = Rect::new(r.right() - 22.0, r.y + 10.0, 12.0, 12.0);
        ui.frame(b, rgb(palette::LINE, 0.4 * a));
        if on {
            ui.fill(b.inset(3.0), dcol(d, 1.0));
        }
    }

    // Tech ceiling: five tiers, planned beyond what has units.
    let y = y + 12.0 + 50.0 + 22.0;
    ui.text(
        area.x + 2.0,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Tech Ceiling",
    );
    let cap = rules.tier_cap.clamp(1, 5);
    let note = if cap >= 4 {
        "T4 and T5 come one at a time, every few rounds"
    } else {
        "The engine climbs to it over the rounds"
    };
    ui.text_right(
        area.right(),
        y,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        note,
    );
    let gap = 8.0;
    let tw = (area.w - 4.0 * gap) / 5.0;
    for k in 0..5u8 {
        let tier = k + 1;
        let r = Rect::new(area.x + k as f32 * (tw + gap), y + 12.0, tw, 56.0);
        let reached = tier <= cap;
        let res = ui.tile(id("sv-tier", k as usize), r, tier == cap, true);
        if res.clicked && tier != cap {
            rules.tier_cap = tier;
            ui.audio.play(Sfx::Tick);
        }
        let tone = if reached {
            palette::ACCENT
        } else {
            palette::FAINT
        };
        // The ladder: every tier up to the ceiling is lit.
        ui.fill(
            Rect::new(r.x + 6.0, r.y + 4.0, r.w - 12.0, 2.0),
            rgb(tone, if reached { 0.85 } else { 0.3 }),
        );
        ui.text(
            r.x + 12.0,
            r.y + 22.0,
            type_scale::ITEM,
            rgb(
                if reached { 0xFFFFFF } else { palette::DIM },
                0.85 + 0.15 * res.glow,
            ),
            &format!("T{tier}"),
        );
        for p in 0..tier {
            ui.fill(
                Rect::new(
                    r.right() - 12.0 - (tier - p) as f32 * 6.0,
                    r.y + 18.0,
                    4.0,
                    8.0,
                ),
                rgb(tone, if reached { 0.9 } else { 0.35 }),
            );
        }
        ui.text_fit_left(
            r.x + 12.0,
            r.y + 41.0,
            r.w - 18.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            TIER_NAMES[k as usize],
        );
    }

    // Replication nodes.
    let y = y + 12.0 + 56.0 + 12.0;
    let r = Rect::new(area.x, y, area.w, PITCH - 4.0);
    let hint = match rules.nodes {
        0 => "None".to_owned(),
        n => format!("Up to {} standing", rules.node_limit().max(n as usize)),
    };
    if let Some(i) = pick_row(
        ui,
        "sv-nodes",
        r,
        "Shapers",
        &hint,
        &NODES,
        rules.nodes.min(3) as usize,
    ) {
        rules.nodes = i as u8;
    }
    ui.text_fit_left(
        area.x + 16.0,
        r.bottom() + 14.0,
        area.w - 16.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "The Progenitor raises Shapers that print one unit type each - a destroyed Shaper leaves a rich wreck to reclaim",
    );

    // The forecast, when there is room for its bars.
    let y = r.bottom() + 44.0;
    if area.bottom() - y > 150.0 {
        forecast(
            ui,
            rules,
            counts,
            Rect::new(area.x, y, area.w, area.bottom() - y),
        );
    }
}

/// One bar per round (sampled when there are many): the engine's budget split
/// over the attacking domains, tier bands behind, a diamond where a node rises.
/// Diamonds stacked over one forecast bar before a "+" says there are more.
const MAX_DIAMONDS: usize = 4;
/// The heavies' (T4/T5) mark on the forecast: pale gold.
const HEAVY: u32 = 0xFFD27A;

fn forecast(ui: &mut Ui, rules: &SurvivalRules, counts: [usize; 3], area: Rect) {
    ui.section(area.x, area.y, area.w, "Forecast");
    let endless = rules.rounds == 0;
    let total = if endless { 30 } else { rules.rounds };
    let n = total.min(30) as usize;
    let round_of = |i: usize| -> u16 {
        if n <= 1 || total as usize == n {
            i as u16 + 1
        } else {
            1 + ((i as f32 * (total - 1) as f32 / (n - 1) as f32).round() as u16)
        }
    };
    let attacking = attacking(rules, counts);
    let weight = |d: Domain| match d {
        Domain::Land => 60,
        Domain::Air => 25,
        Domain::Naval => 20,
    };
    let wsum: i64 = attacking.iter().map(|d| weight(*d)).sum::<i64>().max(1);
    let peak = (0..n)
        .map(|i| rules.budget(round_of(i)))
        .max()
        .unwrap_or(1)
        .max(1);

    let plot = Rect::new(
        area.x + 34.0,
        area.y + 50.0,
        area.w - 34.0,
        area.h - 50.0 - 44.0,
    );
    // Axis and grid.
    for k in 0..=3 {
        let y = plot.bottom() - 0.9 * plot.h * k as f32 / 3.0;
        ui.hline(
            plot.x,
            y,
            plot.w,
            rgb(palette::LINE, if k == 0 { 0.3 } else { 0.06 }),
        );
        if k > 0 {
            ui.text_right(
                plot.x - 6.0,
                y,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                &mass(peak * k / 3),
            );
        }
    }
    let slot = plot.w / n as f32;
    let bw = (slot * 0.68).max(2.0);

    // Tier bands behind the bars.
    let mut i0 = 0;
    while i0 < n {
        let tier = rules.tier_at(round_of(i0));
        let mut i1 = i0;
        while i1 + 1 < n && rules.tier_at(round_of(i1 + 1)) == tier {
            i1 += 1;
        }
        let band = Rect::new(
            plot.x + i0 as f32 * slot,
            plot.y - 18.0,
            (i1 - i0 + 1) as f32 * slot,
            plot.h + 18.0,
        );
        ui.fill(band, rgb(0xFFFFFF, 0.012 + 0.018 * tier as f32));
        if i0 > 0 {
            ui.vline(band.x, band.y, band.h, rgb(palette::LINE, 0.18));
        }
        ui.text_fit_left(
            band.x + 5.0,
            band.y + 8.0,
            band.w - 8.0,
            type_scale::MICRO,
            rgb(palette::TEXT, 0.75),
            &format!("T{tier}"),
        );
        i0 = i1 + 1;
    }

    // Bars.
    let cursor = ui.cursor - ui.shift;
    let over = (ui.interactive
        && ui.mem.popup.is_none()
        && Rect::new(plot.x, plot.y - 18.0, plot.w, plot.h + 18.0).contains(cursor))
    .then(|| (((cursor.x - plot.x) / slot) as usize).min(n - 1));
    for i in 0..n {
        let round = round_of(i);
        let budget = rules.budget(round);
        let target = 0.9 * budget as f32 / peak as f32;
        let k = ui.ease(id("sv-bar", i), target, 9.0);
        let x = plot.x + i as f32 * slot + (slot - bw) * 0.5;
        let hot = over == Some(i);
        if hot {
            ui.fill(
                Rect::new(plot.x + i as f32 * slot, plot.y - 18.0, slot, plot.h + 18.0),
                rgb(palette::ACCENT, 0.08),
            );
        }
        let mut y = plot.bottom();
        let h_total = plot.h * k;
        for d in &attacking {
            let h = h_total * weight(*d) as f32 / wsum as f32;
            ui.fill(
                Rect::new(x, y - h, bw, h),
                dcol(*d, if hot { 0.95 } else { 0.72 }),
            );
            y -= h;
        }
        ui.fill(
            Rect::new(x, y - 1.0, bw, 1.5),
            rgb(0xFFFFFF, if hot { 0.9 } else { 0.45 }),
        );
        // The rounds this bar stands for (more than one when the rounds are sampled).
        let from = if i == 0 { 1 } else { round_of(i - 1) + 1 };
        let span = from..=round;
        // A heavy (T4/T5) in these rounds: a hexagon over the bar, bigger for a T5.
        let mut top = y - 4.0;
        if let Some(heavy) = span.clone().filter_map(|r| rules.heavy_at(r)).max() {
            let s = (bw * 0.45).clamp(3.5, 6.0) * if heavy >= 5 { 1.3 } else { 1.0 };
            let c = Vec2::new(x + bw * 0.5, top - s);
            fill_hex(
                ui,
                c,
                s,
                0.0,
                rgb(HEAVY, if heavy >= 5 { 0.95 } else { 0.4 }),
            );
            outline_hex(ui, c, s, 0.0, 1.2, rgb(HEAVY, 1.0));
            top -= 2.0 * s + 3.0;
        }
        // One diamond per Shaper the facility raises with these rounds.
        let raised: usize = span.map(|r| rules.nodes_raised(r)).sum();
        let s = (bw * 0.35).clamp(2.5, 4.5);
        // As many as fit under the band labels, a "+" for the rest.
        let room = ((top - (plot.y - 4.0)) / (2.0 * s + 2.0)).floor().max(0.0) as usize;
        let shown = raised.min(MAX_DIAMONDS).min(room);
        for k in 0..shown {
            diamond(
                ui,
                Vec2::new(x + bw * 0.5, top - s - k as f32 * (2.0 * s + 2.0)),
                s,
                engine(0.9),
                engine(1.0),
            );
        }
        if raised > shown {
            let yy = top - shown as f32 * (2.0 * s + 2.0) - 6.0;
            ui.text_centred(x + bw * 0.5, yy, type_scale::MICRO, engine(1.0), "+");
        }
    }
    // Round numbers under the axis.
    let label_every = if n > 20 {
        5
    } else if n > 10 {
        2
    } else {
        1
    };
    for i in 0..n {
        let r = round_of(i);
        if i == 0 || i == n - 1 || (r as usize).is_multiple_of(label_every) && i != n - 2 {
            ui.text_centred(
                plot.x + (i as f32 + 0.5) * slot,
                plot.bottom() + 11.0,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                &r.to_string(),
            );
        }
    }

    // The summary, or the hovered round.
    let y = area.bottom() - 12.0;
    let (max_tier, nodes) = if endless {
        (
            rules.tier_cap.min(5),
            (1..=30)
                .map(|r| rules.nodes_raised(r))
                .sum::<usize>()
                .min(rules.node_limit()),
        )
    } else {
        (
            (1..=total).map(|r| rules.tier_at(r)).max().unwrap_or(1),
            (1..=total)
                .map(|r| rules.nodes_raised(r))
                .sum::<usize>()
                .min(rules.node_limit()),
        )
    };
    let text = match over {
        Some(i) => {
            let round = round_of(i);
            let budget = rules.budget(round);
            let parts: Vec<String> = attacking.iter().map(|d| format!("{} {}", d.label(), mass(budget * weight(*d) / wsum))).collect();
            format!(
                "Round {round}  \u{b7}  T{}  \u{b7}  {} mass  \u{b7}  {}{}{}",
                rules.tier_at(round),
                mass(budget),
                parts.join("  "),
                match rules.nodes_raised(round) {
                    0 => String::new(),
                    1 => "  \u{b7}  a Shaper rises".to_string(),
                    k => format!("  \u{b7}  {k} Shapers rise"),
                },
                rules.heavy_at(round).map_or(String::new(), |t| format!("  \u{b7}  a T{t} heavy"))
            )
        }
        None if endless => format!(
            "Endless  \u{b7}  a tier every 6 rounds  \u{b7}  T{} by round {}  \u{b7}  {} Shapers in 30 rounds",
            rules.tier_cap.clamp(1, 5),
            1 + 6 * (rules.tier_cap.clamp(1, 5) as u32 - 1),
            nodes
        ),
        None => {
            let secs = rules.grace_secs as u32 + total as u32 * rules.interval_secs as u32;
            let heavies = (1..=total).filter(|r| rules.heavy_at(*r).is_some()).count();
            format!(
                "{total} rounds  \u{b7}  ~{} min  \u{b7}  tops out at T{max_tier}  \u{b7}  {nodes} Shaper{}{}  \u{b7}  last wave {} mass",
                (secs + 30) / 60,
                if nodes == 1 { "" } else { "s" },
                match heavies {
                    0 => String::new(),
                    1 => "  \u{b7}  1 heavy".to_owned(),
                    k => format!("  \u{b7}  {k} heavies"),
                },
                mass(rules.budget(total))
            )
        }
    };
    let tone = if over.is_some() {
        rgb(palette::TEXT, 1.0)
    } else {
        rgb(palette::DIM, 1.0)
    };
    ui.text_fit_left(area.x, y, area.w, type_scale::CAPTION, tone, &text);
    // Key for the stack colours.
    let mut x = area.right();
    for d in attacking.iter().rev() {
        let w = ui.text_width(type_scale::MICRO, d.label());
        x -= w;
        ui.text(
            x,
            area.y + 22.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            d.label(),
        );
        ui.fill(Rect::new(x - 12.0, area.y + 18.0, 8.0, 8.0), dcol(*d, 0.85));
        x -= 26.0;
    }
    // And the marks over the bars.
    if rules.tier_cap >= 4 {
        x -= ui.text_width(type_scale::MICRO, "Heavy") - 4.0;
        ui.text(
            x,
            area.y + 22.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Heavy",
        );
        outline_hex(
            ui,
            Vec2::new(x - 8.0, area.y + 22.0),
            4.5,
            0.0,
            1.2,
            rgb(HEAVY, 1.0),
        );
        x -= 30.0;
    }
    if rules.nodes > 0 {
        x -= ui.text_width(type_scale::MICRO, "Shaper") - 4.0;
        ui.text(
            x,
            area.y + 22.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Shaper",
        );
        diamond(
            ui,
            Vec2::new(x - 8.0, area.y + 22.0),
            4.0,
            engine(0.9),
            engine(1.0),
        );
    }
    let _ = max_tier;
}

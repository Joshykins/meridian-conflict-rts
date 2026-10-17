//! A survival Shaper's card over the unit panel, in place of the details card
//! when the selection is a Shaper or a piece of its base: its level and tier,
//! what it builds, and its Lance, shield, anti-nuke and base at that level, each
//! grown figure with what the next level adds. The figures come from the sim
//! (`SimStatus::shaper`, `World::shaper_card`); what a level adds is worked out
//! from the Shaper's blueprint (`mc_sim::growth::grown_*`).
//!
//! The card is a list of blocks laid into two columns (`Block`): a new kind of
//! Shaper's section (a Seedcaster's warp-ins, an Unmaker's stock, a Gatewright's
//! fleet) is one more block.

use super::*;
use crate::hud::survival::VIOLET;
use crate::rings::{SHAPER_COVER, SHAPER_LANCE, SHAPER_SHIELD};
use crate::sim_thread::ShaperPick;
use mc_data::survival::{Domain as SiteDomain, SiteKind};
use mc_sim::survival::{
    arrival_shock, fleet_gap_ticks, fleet_size, launch_gap_ticks, shaper_platforms, shaper_pylons,
    shaper_silos, squads_per_volley, volley_ticks, warp_in_ticks, ArtilleryCard, FleetCard,
    FleetStage, Hold, PieceKind, ShaperCard, WarheadCard,
};

const W: f32 = 540.0;
const PAD: f32 = 18.0;
const COL_GAP: f32 = 16.0;
const ROW_H: f32 = 18.0;
/// Width of the "+x/lvl" column at a row's right.
const GROW_W: f32 = 58.0;
const BLOCK_GAP: f32 = 12.0;

/// A figure: its label, its value now, and what the next level adds.
struct Row {
    label: &'static str,
    value: String,
    grow: Option<String>,
}

fn row(label: &'static str, value: String, grow: Option<String>) -> Row {
    Row { label, value, grow }
}

/// What a block shows above its figures.
enum Gadget {
    None,
    /// The Lance's charge, 0..1.
    Charge(f32),
    /// A bar filled `share` (a shield's charge, a volley's beat), with its figure
    /// on the title's line.
    Bar {
        share: f32,
        text: String,
    },
    /// Rounds: ready, the most held, the next one's assembly 0..1, and whether they
    /// are warheads (banded) or interceptors.
    Slots {
        stock: u32,
        capacity: u32,
        next: f32,
        warhead: bool,
    },
    /// Nothing of this kind: why, in place of the figures.
    Absent(&'static str),
}

/// One section of the card.
struct Block {
    title: &'static str,
    tone: u32,
    /// The ring it lights on the ground when hovered (`rings::SHAPER_*`), 0 for none.
    ring: u8,
    gadget: Gadget,
    rows: Vec<Row>,
}

impl Block {
    fn height(&self) -> f32 {
        let gadget = match self.gadget {
            Gadget::None => 0.0,
            Gadget::Charge(_) | Gadget::Bar { .. } => 10.0,
            Gadget::Slots { .. } => 30.0,
            Gadget::Absent(_) => ROW_H,
        };
        18.0 + gadget + self.rows.len() as f32 * ROW_H
    }
}

/// `v` with thousands marked, or in thousands (k) and millions (M) once long.
fn amount(v: f32) -> String {
    let a = v.abs();
    if a >= 9_999_500.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if a >= 99_950.0 {
        format!("{:.0}k", v / 1000.0)
    } else if a >= 10.0 || a.fract() == 0.0 {
        crate::hud::whole(v)
    } else {
        format!("{v:.1}")
    }
}

/// Seconds as "3.2 s", or "1:05" from a minute.
fn secs(s: f32) -> String {
    if s >= 59.95 {
        let t = s.round() as u32;
        format!("{}:{:02}", t / 60, t % 60)
    } else if s >= 9.95 {
        format!("{s:.0} s")
    } else {
        format!("{s:.1} s")
    }
}

/// What the next level adds to a figure `now` that will be `next`: "+35", "-0.2".
/// None when it does not change (a figure at its cap). The figure's own unit is left
/// to its value, for room.
fn gain(now: f32, next: f32, tenths: bool) -> Option<String> {
    let d = next - now;
    let shown = if tenths {
        (d * 10.0).round() / 10.0
    } else {
        d.round()
    };
    if shown == 0.0 {
        return None;
    }
    let sign = if shown > 0.0 { "+" } else { "\u{2212}" };
    let n = if tenths {
        format!("{:.1}", shown.abs())
    } else {
        amount(shown.abs())
    };
    Some(format!("{sign}{n}"))
}

/// What a Shaper of a site kind does, for its header.
fn kind_label(kind: SiteKind) -> &'static str {
    match kind {
        SiteKind::Printer => "Printer",
        SiteKind::Artillery => "Warp-In",
        SiteKind::Nuke => "Nuke",
        SiteKind::Fleet => "Fleet",
        SiteKind::Auxiliary => "Auxiliary",
    }
}

/// The line under the name: level, tier, kind and domain.
fn level_line(card: &ShaperCard) -> String {
    let mut line = format!(
        "Level {}  \u{b7}  T{}  \u{b7}  {}",
        card.level,
        card.tier,
        kind_label(card.kind)
    );
    if card.kind == SiteKind::Printer {
        line.push_str(&format!("  \u{b7}  {}", card.domain.label()));
    }
    line
}

/// A piece of a Shaper's base by name, at a site of `domain`.
fn piece_name(kind: PieceKind, domain: SiteDomain) -> &'static str {
    let sea = domain == SiteDomain::Naval;
    match kind {
        PieceKind::Platform if sea => "Print Slip",
        PieceKind::Platform => "Print Platform",
        PieceKind::GreatPlatform if sea => "Great Print Slip",
        PieceKind::GreatPlatform => "Great Print Platform",
        PieceKind::Pylon => "Shield Pylon",
        PieceKind::Silo => "Warhead Silo",
    }
}

/// Lance figures a Shaper of `bp` has at `level`: charge and reload seconds, damage,
/// splash and range.
fn lance_at(bp: &UnitBlueprint, level: u16) -> Option<[f32; 5]> {
    let l = mc_sim::growth::grown_lance(bp, level)?;
    let tps = mc_core::TICKS_PER_SECOND as f32;
    Some([
        l.charge_ticks as f32 / tps,
        l.reload_ticks as f32 / tps,
        l.damage.to_f32(),
        l.splash.to_f32(),
        l.range.to_f32(),
    ])
}

/// Strikes a minute and damage a second of a Lance that charges `charge` and
/// reloads `reload` seconds, as the sim counts them (`World::shaper_card`).
fn pace(charge: f32, reload: f32, damage: f32) -> (f32, f32) {
    let cycle = (charge + reload).max(0.1);
    (60.0 / cycle, damage / cycle)
}

/// The card's blocks, in order: an auxiliary Shaper's weapon (warp-ins, warheads or
/// fleets), the Lance, the shield, the anti-nuke, the base. `bp` is the Shaper's
/// blueprint, for what a level adds; `next_round` how far its next interceptor is
/// assembled, 0..1.
fn blocks(card: &ShaperCard, bp: Option<&UnitBlueprint>, next_round: f32) -> Vec<Block> {
    let mut out = Vec::new();
    out.extend(
        card.artillery
            .as_ref()
            .map(|a| artillery_block(a, card.level)),
    );
    out.extend(card.warheads.as_ref().map(|w| warhead_block(w, card.level)));
    out.extend(card.fleet.as_ref().map(|f| fleet_block(f, card.level)));
    out.push(lance_block(card, bp));
    out.push(shield_block(card, bp));
    out.push(anti_nuke_block(card, bp, next_round));
    out.push(base_block(card));
    out
}

fn lance_block(card: &ShaperCard, bp: Option<&UnitBlueprint>) -> Block {
    let Some(l) = &card.lance else {
        return absent("Lance", Reach::Direct.tone(), "No Lance");
    };
    let (now, next) = (card.level, card.level.saturating_add(1));
    let lance_gain = bp.and_then(|bp| Some((lance_at(bp, now)?, lance_at(bp, next)?)));
    let g = |i: usize, tenths: bool| lance_gain.and_then(|(a, b)| gain(a[i], b[i], tenths));
    let paced = lance_gain.map(|(a, b)| (pace(a[0], a[1], a[2]), pace(b[0], b[1], b[2])));
    Block {
        title: "Lance",
        tone: Reach::Direct.tone(),
        ring: SHAPER_LANCE,
        gadget: Gadget::Charge(l.charging),
        rows: vec![
            row("Charge", secs(l.charge_secs), g(0, true)),
            row("Reload", secs(l.reload_secs), g(1, true)),
            row(
                "Fire Rate",
                format!("{:.1} /min", l.fire_rate),
                paced.and_then(|(a, b)| gain(a.0, b.0, true)),
            ),
            row("Damage / Strike", amount(l.damage), g(2, false)),
            row(
                "Damage / s",
                amount(l.dps),
                paced.and_then(|(a, b)| gain(a.1, b.1, false)),
            ),
            row("Splash", format!("{:.0} m", l.splash), g(3, false)),
            row("Range", format!("{} m", amount(l.range)), g(4, false)),
        ],
    }
}

fn shield_block(card: &ShaperCard, bp: Option<&UnitBlueprint>) -> Block {
    let Some(sh) = &card.shield else {
        return absent("Shield", Reach::Shield.tone(), "No shield");
    };
    let shield_at = |level| bp.and_then(|bp| mc_sim::growth::grown_shield(bp, level));
    let (a, b) = (
        shield_at(card.level),
        shield_at(card.level.saturating_add(1)),
    );
    let g = |f: fn(&mc_data::Shield) -> f32| gain(f(a.as_ref()?), f(b.as_ref()?), false);
    Block {
        title: "Shield",
        tone: Reach::Shield.tone(),
        ring: SHAPER_SHIELD,
        gadget: Gadget::Bar {
            share: sh.charge / sh.health.max(1.0),
            text: format!("{} / {}", amount(sh.charge), amount(sh.health)),
        },
        rows: vec![
            row("Hit Points", amount(sh.health), g(|s| s.health.to_f32())),
            row(
                "Radius",
                format!("{:.0} m", sh.radius),
                g(|s| s.radius.to_f32()),
            ),
            row(
                "Regen",
                format!("{} /s", amount(sh.regen)),
                g(|s| s.regen.to_f32()),
            ),
            row("Pylon Feed", format!("{} /s", amount(sh.feed)), None),
        ],
    }
}

fn anti_nuke_block(card: &ShaperCard, bp: Option<&UnitBlueprint>, next_round: f32) -> Block {
    let Some(a) = &card.anti_nuke else {
        return absent("Anti-Nuke", Reach::AntiMissile.tone(), "None");
    };
    let cover_at = |level| {
        bp.and_then(|bp| mc_sim::growth::grown_launcher(bp, level))
            .map(|s| (s.stock as f32, s.coverage.to_f32()))
    };
    let (x, y) = (cover_at(card.level), cover_at(card.level.saturating_add(1)));
    let g = |pick: fn((f32, f32)) -> f32| gain(pick(x?), pick(y?), false);
    Block {
        title: "Anti-Nuke",
        tone: Reach::AntiMissile.tone(),
        ring: SHAPER_COVER,
        gadget: Gadget::Slots {
            stock: a.stock as u32,
            capacity: a.capacity as u32,
            next: next_round,
            warhead: false,
        },
        rows: vec![
            row(
                "Interceptors",
                format!("{} / {}", a.stock, a.capacity),
                g(|v| v.0),
            ),
            row("Coverage", format!("{} m", amount(a.coverage)), g(|v| v.1)),
            row("Each Takes", secs(a.round_secs), None),
        ],
    }
}

/// What holds an auxiliary Shaper's volley or fleet, for the bar over its block.
fn hold_text(hold: Hold) -> Option<&'static str> {
    match hold {
        Hold::None => None,
        Hold::Dampened => Some("Dampened: every target in a warp field"),
    }
}

/// A Seedcaster's warp-ins: its volley charging, and what a squad is.
fn artillery_block(a: &ArtilleryCard, level: u16) -> Block {
    let next = level.saturating_add(1);
    let tps = mc_core::TICKS_PER_SECOND as f32;
    let every = |l| volley_ticks(l) as f32 / tps;
    let warning = |l| warp_in_ticks(l) as f32 / tps;
    let (r0, d0) = arrival_shock(level);
    let (r1, d1) = arrival_shock(next);
    let heavy = if a.squad.1 > 0 {
        format!(" + {} heavy", a.squad.1)
    } else {
        String::new()
    };
    let mut rows = vec![
        row(
            "Volley Every",
            secs(a.interval_secs),
            gain(every(level), every(next), true),
        ),
        row(
            "Squads / Volley",
            a.squads_per_volley.to_string(),
            gain(
                squads_per_volley(level) as f32,
                squads_per_volley(next) as f32,
                false,
            ),
        ),
        row(
            "Squad",
            format!("{}{heavy}  T{}", a.squad.0, a.squad_tech),
            None,
        ),
        row(
            "Warp-In After",
            secs(a.warp_in_secs),
            gain(warning(level), warning(next), true),
        ),
        row(
            "Arrival Shock",
            format!("{:.0} m", a.shock_radius),
            gain(r0.to_f32(), r1.to_f32(), false),
        ),
        row(
            "Shock Damage",
            amount(a.shock_damage),
            gain(d0.to_f32(), d1.to_f32(), false),
        ),
    ];
    if a.marked > 0 {
        rows.push(row("Marked", a.marked.to_string(), None));
    }
    Block {
        title: "Warp-Ins",
        tone: VIOLET,
        ring: 0,
        gadget: Gadget::Bar {
            share: 1.0 - a.next_in_secs / a.interval_secs.max(0.1),
            text: match hold_text(a.hold) {
                Some(why) => why.to_string(),
                None if a.next_in_secs < 0.05 => "Volley now".to_string(),
                None => format!("Volley in {}", secs(a.next_in_secs)),
            },
        },
        rows,
    }
}

/// A Gatewright's fleets: the one printing or coming, and what a fleet is.
fn fleet_block(f: &FleetCard, level: u16) -> Block {
    let next = level.saturating_add(1);
    let tps = mc_core::TICKS_PER_SECOND as f32;
    let gap = |l| fleet_gap_ticks(l) as f32 / tps;
    let (light, capital) = fleet_size(level);
    let (light1, capital1) = fleet_size(next);
    let mut rows = vec![
        row(
            "Fleet",
            if f.size.1 > 0 {
                format!("{} + {} capital", f.size.0, f.size.1)
            } else {
                f.size.0.to_string()
            },
            gain((light + capital) as f32, (light1 + capital1) as f32, false),
        ),
        row("Classes", f.classes.len().to_string(), None),
        row("Rests", secs(f.gap_secs), gain(gap(level), gap(next), true)),
        row("Mark Shown", secs(f.lead_secs), None),
        row("In the Field", f.in_field.to_string(), None),
    ];
    if f.ships > 0 {
        rows.insert(2, row("Ships Now", f.ships.to_string(), None));
    }
    let (share, text) = match (hold_text(f.hold), f.stage) {
        (Some(why), FleetStage::Resting) => (0.0, why.to_string()),
        (_, FleetStage::Resting) if f.next_in_secs < 0.05 => (0.0, "Printing now".to_string()),
        (_, FleetStage::Resting) => (
            1.0 - f.next_in_secs / f.gap_secs.max(0.1),
            format!("Next fleet in {}", secs(f.next_in_secs)),
        ),
        (_, FleetStage::Building) => (
            f.build,
            format!(
                "Printing  {:.0}%  \u{b7}  {}",
                f.build * 100.0,
                secs(f.next_in_secs)
            ),
        ),
        (_, FleetStage::Marked) => (1.0, format!("Warp in {}", secs(f.next_in_secs))),
        (_, FleetStage::Warping) => (1.0, format!("Warping  {}", secs(f.next_in_secs))),
    };
    Block {
        title: "Fleet",
        tone: crate::hud::style::AIR,
        ring: 0,
        gadget: Gadget::Bar { share, text },
        rows,
    }
}

/// An Unmaker's warheads: its silos' stock, and the blast.
fn warhead_block(w: &WarheadCard, level: u16) -> Block {
    let tps = mc_core::TICKS_PER_SECOND as f32;
    let gap = |l| launch_gap_ticks(l) as f32 / tps;
    let mut rows = vec![
        row(
            "Silos",
            format!("{} / {}", w.silos, w.silos_wanted),
            gain(
                shaper_silos(level) as f32,
                shaper_silos(level.saturating_add(1)) as f32,
                false,
            ),
        ),
        row("Warheads", format!("{} / {}", w.stock, w.capacity), None),
        row("Each Takes", secs(w.round_secs), None),
        row(
            "Launch Every",
            secs(w.gap_secs),
            gain(gap(level), gap(level.saturating_add(1)), true),
        ),
        row("Blast", format!("{:.0} m", w.blast_radius), None),
        row("Blast Core", format!("{:.0} m", w.blast_core), None),
        row("Blast Damage", amount(w.blast_damage), None),
    ];
    if w.in_flight > 0 {
        rows.push(row("In Flight", w.in_flight.to_string(), None));
    }
    Block {
        title: "Warheads",
        tone: crate::hud::silo::WARHEAD,
        ring: 0,
        // No silo standing yet: no slots to show.
        gadget: if w.capacity == 0 {
            Gadget::None
        } else {
            Gadget::Slots {
                stock: u32::from(w.stock),
                capacity: u32::from(w.capacity),
                next: 0.0,
                warhead: true,
            }
        },
        rows,
    }
}

/// The base: what stands of each kind of piece, against what its level raises.
fn base_block(card: &ShaperCard) -> Block {
    let standing = |k: PieceKind| {
        card.pieces
            .iter()
            .filter(|p| p.kind == k && p.raised)
            .count()
    };
    let printing = card.pieces.iter().filter(|p| !p.raised).count();
    let sea = card.domain == SiteDomain::Naval;
    let mut rows = Vec::new();
    if card.kind == SiteKind::Printer {
        let (small, great) = shaper_platforms(card.level);
        rows.push(row(
            if sea { "Slips" } else { "Platforms" },
            format!("{} / {small}", standing(PieceKind::Platform)),
            None,
        ));
        rows.push(row(
            if sea {
                "Great Slips"
            } else {
                "Great Platforms"
            },
            format!("{} / {great}", standing(PieceKind::GreatPlatform)),
            None,
        ));
    }
    rows.push(row(
        "Pylons",
        format!(
            "{} / {}",
            standing(PieceKind::Pylon),
            shaper_pylons(card.level)
        ),
        None,
    ));
    if printing > 0 {
        rows.push(row("Printing", printing.to_string(), None));
    }
    Block {
        title: "Base",
        tone: VIOLET,
        ring: 0,
        gadget: Gadget::None,
        rows,
    }
}

fn absent(title: &'static str, tone: u32, why: &'static str) -> Block {
    Block {
        title,
        tone,
        ring: 0,
        gadget: Gadget::Absent(why),
        rows: Vec::new(),
    }
}

/// Lays `blocks` into two columns, each into the shorter: (column, y offset) per
/// block and the columns' height.
fn columns(blocks: &[Block]) -> (Vec<(usize, f32)>, f32) {
    let mut h = [0.0f32; 2];
    let at = blocks
        .iter()
        .map(|b| {
            let c = usize::from(h[1] < h[0]);
            let y = h[c];
            h[c] += b.height() + BLOCK_GAP;
            (c, y)
        })
        .collect();
    (at, h[0].max(h[1]) - BLOCK_GAP)
}

/// Product chips: icon and name, wrapped onto lines `w` wide. Returns each chip's
/// x, line, and width.
fn chip_layout(ui: &mut Ui, names: &[String], w: f32) -> Vec<(f32, usize, f32)> {
    let mut out = Vec::new();
    let (mut x, mut line) = (0.0, 0);
    for n in names {
        let cw = 22.0 + ui.text_width(type_scale::CAPTION, n);
        if x > 0.0 && x + cw > w {
            x = 0.0;
            line += 1;
        }
        out.push((x, line, cw));
        x += cw + 14.0;
    }
    out
}

/// The Shaper's card over the unit panel at `anchor`.
pub(super) fn card(ui: &mut Ui, hud: &mut Hud, s: &Scene, pick: &ShaperPick, anchor: Rect) {
    let c = &pick.card;
    let cw = W - 2.0 * PAD;
    let shaper = s
        .view
        .index_of
        .get(&c.unit.0)
        .map(|&i| &s.view.frame.units[i]);
    let bp = shaper.map(|u| s.bp(u));
    let name = bp.map_or("Shaper", |b| b.name.as_str());
    let next_round = shaper
        .and_then(crate::hud::silo::Launcher::of)
        .map_or(0.0, |l| l.progress);
    let blocks = blocks(c, bp, next_round);
    let (at, cols_h) = columns(&blocks);
    let names: Vec<String> = c
        .products
        .iter()
        .map(|p| s.blueprints.unit(*p).name.clone())
        .collect();
    let chips = chip_layout(ui, &names, cw);
    let lines = chips.last().map_or(0, |c| c.1 + 1);
    let builds_h = if c.products.is_empty() {
        0.0
    } else {
        18.0 + lines as f32 * 22.0 + 18.0 + 8.0
    };
    let catching = c.level < c.catch_up;
    let note = catching || c.raised < 1.0 || pick.piece.is_some();
    let head_h = if note { 86.0 } else { 72.0 };
    let h = head_h + builds_h + cols_h + PAD;
    let r = Rect::new(anchor.x, (anchor.y - 10.0 - h).max(14.0), W, h);
    hud.claim(ui, r);
    ui.panel(r);
    let x = r.x + PAD;

    header(ui, hud, pick, name, r, note);
    let mut y = r.y + head_h;

    if !c.products.is_empty() {
        builds(ui, s, c, &names, &chips, x, y);
        y += builds_h;
    }

    // The blocks, two columns. A block under the pointer lights its ring on the
    // ground; last frame's focus lights it here.
    let col_w = (cw - COL_GAP) * 0.5;
    let focus = hud.survival.shaper_focus;
    let mut hover = 0;
    for (i, (b, (col, dy))) in blocks.iter().zip(at).enumerate() {
        let bx = x + col as f32 * (col_w + COL_GAP);
        let br = Rect::new(bx - 6.0, y + dy - 9.0, col_w + 12.0, b.height());
        let res = ui.interact_with(id("shaper-block", i), br, true, false);
        if res.hovered && b.ring != 0 {
            hover = b.ring;
        }
        let lit = focus_of(
            ui,
            "shaper-block-focus",
            i,
            u64::from(focus),
            u64::from(b.ring),
        );
        draw_block(ui, b, bx, y + dy, col_w, lit);
    }
    hud.survival.shaper_focus = hover;
    hud.details_focus = 0;
    hud.reach_focus = None;
}

/// The card's head in `r`: the name, its site and the close cross; level, tier and
/// kind and how tall it stands; with `note`, whether it is rising or evolving and
/// which piece of its base is selected.
fn header(ui: &mut Ui, hud: &mut Hud, pick: &ShaperPick, name: &str, r: Rect, note: bool) {
    let c = &pick.card;
    let (x, cw) = (r.x + PAD, W - 2.0 * PAD);
    let t = ui.time;
    let catching = c.level < c.catch_up;
    // The name, its site, and the close cross.
    let site = hud
        .survival
        .sites
        .get(c.site as usize)
        .cloned()
        .unwrap_or_else(|| format!("Site {}", c.site + 1));
    let end = ui.text(x, r.y + 24.0, type_scale::ITEM, rgb(0xFFFFFF, 1.0), name);
    let close = Rect::new(r.right() - 34.0, r.y + 12.0, 22.0, 22.0);
    ui.text_fit_left(
        end + 10.0,
        r.y + 25.0,
        close.x - end - 20.0,
        type_scale::CAPTION,
        rgb(VIOLET, 1.0),
        &site,
    );
    let res = ui.interact(id("shaper-close", 0), close, true);
    let cc = Vec2::new(close.x + 11.0, close.mid_y());
    let tone = rgb(palette::TEXT, 0.6 + 0.4 * res.glow);
    ui.stroke(cc - Vec2::splat(5.0), cc + Vec2::splat(5.0), 1.4, tone);
    ui.stroke(
        cc + Vec2::new(-5.0, 5.0),
        cc + Vec2::new(5.0, -5.0),
        1.4,
        tone,
    );
    if res.clicked {
        ui.audio.play(Sfx::Tick);
        hud.survival.shaper_closed = Some(c.unit.0);
    }
    // Level, tier and kind; how tall it stands against its level's height.
    ui.text(
        x,
        r.y + 44.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &level_line(c),
    );
    let tall = format!("{:.0} / {:.0} m tall", c.height, c.full_height);
    ui.text_right(
        x + cw,
        r.y + 44.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &tall,
    );
    let growth = Rect::new(x, r.y + 53.0, cw, 2.0);
    ui.fill(growth, rgb(palette::LINE, 0.08));
    ui.fill(
        Rect::new(
            growth.x,
            growth.y,
            growth.w * (c.height / c.full_height.max(1.0)).clamp(0.0, 1.0),
            growth.h,
        ),
        rgb(VIOLET, 0.9),
    );
    if note {
        let pulse = 0.65 + 0.35 * (t * 3.0).sin().abs();
        let mut parts = Vec::new();
        if c.raised < 1.0 {
            parts.push(format!("Rising  {:.0}%", c.raised * 100.0));
        }
        if catching {
            parts.push(format!("Evolving to level {}", c.catch_up));
        }
        if let Some(k) = pick.piece {
            parts.push(format!("Selected: its {}", piece_name(k, c.domain)));
        }
        ui.text_fit_left(
            x,
            r.y + 68.0,
            cw,
            type_scale::MICRO,
            rgb(
                VIOLET,
                if catching || c.raised < 1.0 {
                    pulse
                } else {
                    1.0
                },
            ),
            &parts.join("  \u{b7}  "),
        );
    }
}

/// What it builds from (`x`, `y`): its products as chips (`chip_layout`), then the
/// formation and the next print.
fn builds(
    ui: &mut Ui,
    s: &Scene,
    c: &ShaperCard,
    names: &[String],
    chips: &[(f32, usize, f32)],
    x: f32,
    y: f32,
) {
    let cw = W - 2.0 * PAD;
    let lines = chips.last().map_or(0, |c| c.1 + 1);
    // What it builds: its products, then the formation and the next print (a
    // Seedcaster's products are what its squads are made of, a Gatewright's the classes
    // of its fleets).
    ui.section(x, y, cw - 180.0, "Builds");
    let heavy = if c.batch.1 > 0 {
        format!(" + {} heavy", c.batch.1)
    } else {
        String::new()
    };
    ui.text_right(
        x + cw,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &match c.kind {
            SiteKind::Printer => format!("Formation of {}{heavy}", c.batch.0),
            SiteKind::Fleet => "Its fleets warp in over you".to_string(),
            _ => "Its squads warp in beside you".to_string(),
        },
    );
    let y = y + 18.0;
    for ((p, n), (cx, line, _)) in c.products.iter().zip(names).zip(chips) {
        let pb = s.blueprints.unit(*p);
        let tone = match Domain::of(pb) {
            Domain::Air => crate::hud::style::AIR,
            Domain::Navy => crate::hud::style::NAVY,
            _ => crate::hud::style::LAND,
        };
        let cy = y + *line as f32 * 22.0;
        icons::strategic(
            ui,
            pb.visual.icon,
            pb.tech,
            Vec2::new(x + cx + 7.0, cy),
            6.5,
            rgb(tone, 1.0),
            ink(0.9),
        );
        ui.text(
            x + cx + 20.0,
            cy,
            type_scale::CAPTION,
            rgb(palette::TEXT, 0.92),
            n,
        );
    }
    let y = y + lines as f32 * 22.0;
    let printed = format!("{} printed", whole(c.printed as f32));
    let line = match c.kind {
        SiteKind::Printer if c.next_print > 0.05 => {
            format!("Next print in {}  \u{b7}  {printed}", secs(c.next_print))
        }
        SiteKind::Printer => format!("Prints with the round  \u{b7}  {printed}"),
        _ => printed,
    };
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &line);
}

/// One block at (`x`, `y`), `w` wide; `(lit, back)` ease its focus (`focus_of`).
fn draw_block(ui: &mut Ui, b: &Block, x: f32, y: f32, w: f32, (lit, back): (f32, f32)) {
    let area = Rect::new(x - 6.0, y - 9.0, w + 12.0, b.height());
    let t = ui.time;
    if lit > 0.01 {
        ui.fill(area, rgb(b.tone, 0.10 * lit));
        ui.fill(
            Rect::new(area.x, area.y + 2.0, 2.0, area.h - 4.0),
            rgb(b.tone, lit),
        );
    }
    let fade = ui.fade;
    ui.fade *= 1.0 - 0.6 * back;
    // A bar's figure on the title's line, right.
    let status = match &b.gadget {
        Gadget::Charge(k) if *k > 0.0 => Some(format!("Charging  {:.0}%", k * 100.0)),
        Gadget::Charge(_) => Some("Ready".to_string()),
        Gadget::Bar { text, .. } => Some(text.clone()),
        _ => None,
    };
    let status_w = status
        .as_ref()
        .map_or(0.0, |s| ui.text_width(type_scale::MICRO, s) + 10.0);
    ui.section(x, y, w - status_w, b.title);
    if let Some(st) = &status {
        ui.text_right(x + w, y, type_scale::MICRO, rgb(palette::DIM, 1.0), st);
    }
    let mut y = y + 18.0;
    match &b.gadget {
        Gadget::None => {}
        Gadget::Charge(k) => {
            let track = Rect::new(x, y - 3.0, w, 4.0);
            ui.fill(track, rgb(b.tone, 0.14));
            if *k > 0.0 {
                ui.fill(
                    Rect::new(track.x, track.y, track.w * k.clamp(0.0, 1.0), track.h),
                    rgb(b.tone, 0.95),
                );
                let head = track.x + track.w * k.clamp(0.0, 1.0);
                let a = 0.5 + 0.5 * (t * 8.0).sin().abs();
                ui.fill(
                    Rect::new(head - 1.0, track.y - 2.0, 2.0, track.h + 4.0),
                    rgb(0xFFFFFF, a),
                );
            }
            y += 10.0;
        }
        Gadget::Bar { share, .. } => {
            let track = Rect::new(x, y - 3.0, w, 4.0);
            ui.fill(track, rgb(b.tone, 0.14));
            ui.fill(
                Rect::new(track.x, track.y, track.w * share.clamp(0.0, 1.0), track.h),
                rgb(b.tone, 0.95),
            );
            y += 10.0;
        }
        Gadget::Slots {
            stock,
            capacity,
            next,
            warhead,
        } => {
            let gap = if *capacity > 8 { 3.0 } else { 5.0 };
            let room = w - 40.0;
            let fit = (((room + gap) / (7.0 + gap)).floor() as u32).max(1);
            let shown = (*capacity).min(fit);
            let slot_w = (room / shown.max(1) as f32 - gap).clamp(7.0, 14.0);
            for i in 0..shown {
                let fill = if i < *stock {
                    1.0
                } else if i == *stock {
                    *next
                } else {
                    0.0
                };
                let at = Rect::new(x + i as f32 * (slot_w + gap), y - 7.0, slot_w, 24.0);
                crate::hud::silo::round_slot(ui, at, fill, i < *stock, b.tone, *warhead);
            }
            if shown < *capacity {
                ui.text(
                    x + shown as f32 * (slot_w + gap) + 2.0,
                    y + 5.0,
                    type_scale::MICRO,
                    rgb(palette::DIM, 1.0),
                    &format!("+{}", capacity - shown),
                );
            }
            y += 30.0;
        }
        Gadget::Absent(why) => {
            ui.text(x, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), why);
        }
    }
    for r in &b.rows {
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), r.label);
        ui.text_right(
            x + w - GROW_W,
            y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &r.value,
        );
        if let Some(g) = &r.grow {
            ui.text_right(
                x + w,
                y,
                type_scale::MICRO,
                rgb(VIOLET, 0.85),
                &format!("{g}/lvl"),
            );
        }
        y += ROW_H;
    }
    ui.fade = fade;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_read_short() {
        assert_eq!(amount(0.0), "0");
        assert_eq!(amount(1.0), "1");
        assert_eq!(amount(4.26), "4.3");
        assert_eq!(amount(1500.0), "1,500");
        assert_eq!(amount(41_250.0), "41,250");
        assert_eq!(amount(184_000.0), "184k");
        assert_eq!(amount(12_400_000.0), "12.4M");
    }

    #[test]
    fn seconds_read_as_a_clock_from_a_minute() {
        assert_eq!(secs(3.0), "3.0 s");
        assert_eq!(secs(5.76), "5.8 s");
        assert_eq!(secs(14.2), "14 s");
        assert_eq!(secs(65.0), "1:05");
    }

    #[test]
    fn a_level_gain_is_signed_and_absent_at_a_cap() {
        assert_eq!(gain(800.0, 835.0, false).as_deref(), Some("+35"));
        assert_eq!(gain(6.0, 5.76, true).as_deref(), Some("\u{2212}0.2"));
        assert_eq!(gain(900.0, 900.0, false), None);
        // Too small to show at its precision is no change.
        assert_eq!(gain(10.0, 10.01, true), None);
        assert_eq!(gain(1500.0, 1800.0, false).as_deref(), Some("+300"));
    }

    #[test]
    fn a_lance_paces_as_the_sim_counts() {
        let (rate, dps) = pace(3.0, 6.0, 1500.0);
        assert!((rate - 60.0 / 9.0).abs() < 1e-4);
        assert!((dps - 1500.0 / 9.0).abs() < 1e-3);
    }

    #[test]
    fn blocks_fill_the_shorter_column() {
        let b = |rows: usize| Block {
            title: "",
            tone: 0,
            ring: 0,
            gadget: Gadget::None,
            rows: (0..rows).map(|_| row("", String::new(), None)).collect(),
        };
        let (at, h) = columns(&[b(7), b(4), b(3), b(3)]);
        assert_eq!(at[0].0, 0);
        assert_eq!(at[1].0, 1);
        // The third goes under the shorter second, the fourth under the first.
        assert_eq!(at[2].0, 1);
        assert_eq!(at[3].0, 0);
        let left = b(7).height() + BLOCK_GAP + b(3).height();
        let right = b(4).height() + BLOCK_GAP + b(3).height();
        assert!((h - left.max(right)).abs() < 1e-3);
    }
}

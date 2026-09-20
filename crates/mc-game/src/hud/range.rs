//! The test range's panel, down the left edge: pick the subject, spawn it for
//! either side, hurt it, scrub its build state, stage a scenario, reset.
//! Like the rest of the HUD it only reports what was asked for.

use super::{has_flag, Hud, HudAction, Scene, EDGE, GAP};
use crate::audio::Sfx;
use crate::game::Mode;
use crate::range::{Range, RangeAction, Scenario, Side, BLUE, RED};
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Id, Rect, Ui};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::flag;

pub const WIDTH: f32 = 340.0;
const PAD: f32 = 14.0;
const ROW: f32 = 28.0;

/// A HUD tile with a word on it. `tone` colours the word while it is lit.
#[allow(clippy::too_many_arguments)]
fn word_tile(
    hud: &mut Hud,
    ui: &mut Ui,
    id: Id,
    r: Rect,
    label: &str,
    lit: bool,
    enabled: bool,
    tone: u32,
) -> bool {
    let t = hud.tile(ui, id, r, lit, enabled);
    let live = if enabled { 1.0 } else { 0.3 };
    ui.text_centred(
        r.x + r.w * 0.5 + 1.5,
        r.mid_y() - 1.0,
        type_scale::MICRO,
        rgb(
            if lit { tone } else { palette::TEXT },
            (0.74 + 0.26 * t.glow) * live,
        ),
        label,
    );
    if t.clicked {
        ui.audio.play(Sfx::Select);
    }
    t.clicked
}

/// `n` equal rectangles across `r`, `gap` apart.
fn split(r: Rect, n: usize, gap: f32) -> impl Iterator<Item = Rect> {
    let w = (r.w - gap * (n as f32 - 1.0)) / n as f32;
    (0..n).map(move |i| Rect::new(r.x + i as f32 * (w + gap), r.y, w, r.h))
}

pub fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene, range: &Range, top: f32) {
    let view = s.view;
    let subject = s.blueprints.unit(range.subject);
    let acted = range.acted(&view.selection, &view.frame.units, s.blueprints, view.local);
    let targets: Vec<&UnitInstance> = acted
        .ids
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
        .collect();
    let any = !targets.is_empty();

    let panel = Rect::new(EDGE, top + GAP, WIDTH, 610.0);
    hud.glass(ui, panel);
    let (x, w) = (panel.x + PAD, panel.w - 2.0 * PAD);
    let mut y = panel.y + 22.0;
    let mut asked: Vec<RangeAction> = Vec::new();

    ui.text(
        x,
        y,
        type_scale::OVERLINE,
        rgb(palette::ACCENT, 1.0),
        "TEST RANGE",
    );
    y += 26.0;

    // -- subject ------------------------------------------------------------------
    ui.section(x, y, w, "SUBJECT");
    y += 16.0;
    let step = ui.stepper(
        id("range-subject", 0),
        Rect::new(x, y, w, 32.0),
        &subject.name.to_uppercase(),
        rgb(palette::TEXT, 1.0),
        true,
    );
    if step != 0 {
        asked.push(RangeAction::Subject(step));
    }
    y += 32.0 + 12.0;
    ui.text(
        x,
        y,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!("T{}  \u{b7}  {}", subject.tech, subject.role.to_uppercase()),
    );
    ui.text_right(
        x + w,
        y,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &subject.key,
    );
    y += 16.0;

    let count = ui.stepper(
        id("range-count", 0),
        Rect::new(x, y, 96.0, ROW),
        &format!("\u{d7} {}", range.count()),
        rgb(palette::TEXT, 1.0),
        subject.is_mobile(),
    );
    if count != 0 {
        asked.push(RangeAction::Count(count));
    }
    for (i, (side, r)) in Side::ALL
        .into_iter()
        .zip(split(Rect::new(x + 104.0, y, w - 104.0, ROW), 3, 4.0))
        .enumerate()
    {
        let tone = if side == Side::Blue {
            palette::ACCENT
        } else {
            palette::BAD
        };
        if word_tile(
            hud,
            ui,
            id("range-side", i),
            r,
            side.label(),
            range.side == side,
            true,
            tone,
        ) {
            asked.push(RangeAction::Side(side));
        }
    }
    y += ROW + 6.0;
    if word_tile(
        hud,
        ui,
        id("range-spawn", 0),
        Rect::new(x, y, w, ROW),
        "SPAWN AT THE POINTER   [ G ]",
        view.mode == Mode::Spawn,
        true,
        palette::ACCENT,
    ) {
        asked.push(RangeAction::ArmSpawn);
    }
    y += ROW + 22.0;

    // -- what happens to it ---------------------------------------------------------
    ui.section(x, y, w, "DO TO");
    ui.text_right(
        x + w,
        y,
        type_scale::MICRO,
        rgb(if any { palette::TEXT } else { palette::FAINT }, 1.0),
        &acted.label,
    );
    y += 16.0;
    let hurts: [(&str, i16, u32); 4] = [
        ("\u{2212}10%", 100, palette::WARN),
        ("\u{2212}25%", 250, palette::WARN),
        ("HEAL", -1000, palette::ACCENT),
        ("KILL", 1000, palette::BAD),
    ];
    for (i, ((label, permille, tone), r)) in hurts
        .into_iter()
        .zip(split(Rect::new(x, y, w, ROW), 4, 4.0))
        .enumerate()
    {
        if word_tile(hud, ui, id("range-hurt", i), r, label, false, any, tone) {
            asked.push(RangeAction::Damage(permille));
        }
    }
    y += ROW + 4.0;
    let all_have = |f: u16| any && targets.iter().all(|u| has_flag(u, f));
    let mut row = split(Rect::new(x, y, w, ROW), 3, 4.0);
    if word_tile(
        hud,
        ui,
        id("range-remove", 0),
        row.next().unwrap(),
        "REMOVE",
        false,
        any,
        palette::TEXT,
    ) {
        asked.push(RangeAction::Remove);
    }
    for (i, (label, f)) in [
        ("HOLD FIRE", flag::PASSIVE),
        ("CANNOT DIE", flag::INVULNERABLE),
    ]
    .into_iter()
    .enumerate()
    {
        let on = all_have(f);
        if word_tile(
            hud,
            ui,
            id("range-flag", i),
            row.next().unwrap(),
            label,
            on,
            any,
            palette::ACCENT,
        ) {
            asked.push(RangeAction::Flag(f, !on));
        }
    }
    y += ROW + 8.0;

    // Build state: drag anywhere along the track. 100% finishes the unit.
    let built = if any {
        targets.iter().map(|u| u.build).sum::<f32>() / targets.len() as f32
    } else {
        1.0
    };
    let track = Rect::new(x + 52.0, y, w - 52.0 - 44.0, ROW);
    let res = ui.interact(id("range-build", 0), track, any);
    let live = if any { 1.0 } else { 0.3 };
    ui.text(
        x,
        track.mid_y(),
        type_scale::MICRO,
        rgb(palette::DIM, live),
        "BUILT",
    );
    let line = Rect::new(track.x, track.mid_y() - 2.0, track.w, 4.0);
    ui.fill(line, rgb(palette::LINE, 0.16 * live));
    ui.fill(
        Rect::new(line.x, line.y, line.w * built, line.h),
        rgb(palette::ACCENT, 0.9 * live),
    );
    for i in 0..=10 {
        ui.vline(
            line.x + line.w * i as f32 / 10.0,
            line.bottom() + 4.0,
            if i % 5 == 0 { 5.0 } else { 3.0 },
            rgb(palette::LINE, 0.3 * live),
        );
    }
    let grow = 1.5 * res.glow + if res.held { 1.5 } else { 0.0 };
    ui.fill(
        Rect::new(
            line.x + line.w * built - 3.0 - grow * 0.5,
            track.mid_y() - 8.0 - grow * 0.5,
            6.0 + grow,
            16.0 + grow,
        ),
        rgb(0xFFFFFF, live),
    );
    ui.text_right(
        x + w,
        track.mid_y(),
        type_scale::VALUE,
        rgb(palette::TEXT, live),
        &format!("{:.0}%", built * 100.0),
    );
    if res.held {
        // 5% detents, and only a new detent is an order.
        let wanted = ((((ui.cursor.x - ui.shift.x - line.x) / line.w).clamp(0.0, 1.0) * 20.0)
            .round()
            * 50.0) as u16;
        let current = hud.range_built.unwrap_or((built * 1000.0).round() as u16);
        if wanted != current {
            hud.range_built = Some(wanted);
            ui.audio.play(Sfx::Tick);
            asked.push(RangeAction::Build(wanted));
        }
    } else {
        hud.range_built = None;
    }
    y += ROW + 22.0;

    // -- scenarios ------------------------------------------------------------------
    ui.section(x, y, w, "STAGE");
    y += 16.0;
    // Around the subject first, then what the subject is told to do itself.
    for (n, line) in [&Scenario::ALL[..4], &Scenario::ALL[4..]]
        .into_iter()
        .enumerate()
    {
        for (i, (scenario, r)) in line
            .iter()
            .zip(split(Rect::new(x, y, w, ROW + 4.0), line.len(), 4.0))
            .enumerate()
        {
            if word_tile(
                hud,
                ui,
                id("range-scenario", n * 4 + i),
                r,
                scenario.label(),
                false,
                true,
                palette::ACCENT,
            ) {
                asked.push(RangeAction::Scenario(*scenario));
            }
        }
        y += ROW + 4.0 + 4.0;
    }
    y += 2.0;
    if ui.button(
        id("range-reset", 0),
        Rect::new(x, y, w, 34.0),
        "RESET   [ F5 ]",
        ButtonKind::Primary,
        true,
    ) {
        asked.push(RangeAction::Reset);
    }
    y += 34.0 + 22.0;

    // -- the range itself -------------------------------------------------------------
    ui.section(x, y, w, "COMMANDING");
    y += 16.0;
    let mut row = split(Rect::new(x, y, w, ROW), 3, 4.0);
    for (i, (label, player, tone)) in [("BLUE", BLUE, palette::ACCENT), ("RED", RED, palette::BAD)]
        .into_iter()
        .enumerate()
    {
        if word_tile(
            hud,
            ui,
            id("range-control", i),
            row.next().unwrap(),
            label,
            view.local == player,
            true,
            tone,
        ) {
            asked.push(RangeAction::Control(player));
        }
    }
    if word_tile(
        hud,
        ui,
        id("range-free", 0),
        row.next().unwrap(),
        "FREE BUILD",
        range.free_build,
        true,
        palette::ACCENT,
    ) {
        asked.push(RangeAction::FreeBuild(!range.free_build));
    }
    y += ROW + 4.0;
    for (i, (label, r)) in ["CLOSE  F2", "MID  F3", "FAR  F4"]
        .into_iter()
        .zip(split(Rect::new(x, y, w, ROW), 3, 4.0))
        .enumerate()
    {
        if word_tile(
            hud,
            ui,
            id("range-zoom", i),
            r,
            label,
            false,
            true,
            palette::ACCENT,
        ) {
            asked.push(RangeAction::Zoom(i));
        }
    }
    y += ROW + 4.0;
    if word_tile(
        hud,
        ui,
        id("range-reload", 0),
        Rect::new(x, y, w, ROW),
        "RELOAD DATA/ AND RESTART   [ F9 ]",
        false,
        true,
        palette::ACCENT,
    ) {
        asked.push(RangeAction::Reload);
    }
    debug_assert!(
        y + ROW <= panel.bottom(),
        "the range panel outgrew its glass"
    );

    hud.actions.extend(asked.into_iter().map(HudAction::Range));
}

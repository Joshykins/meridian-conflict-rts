//! Warp on the unit card (`mc_sim::warp`): a ship's jump as its activity (charging with
//! how far along, in warp, coming out), a stun in its electric colour with the seconds
//! left, and a line under that for the drive (ready, or recharging) or a dampener's field
//! (up, or down and why). What is drawn in the world is `warp_marks.rs`.

use super::{has_flag, whole, Scene};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use mc_data::UnitBlueprint;
use mc_sim::mirror::{UnitInstance, WarpView};
use mc_sim::tables::{flag, WarpPhase};

/// A drive's jump and its charge bar: a cold violet, clear of the economy's
/// yellow and the interface's red-orange.
pub const WARP: u32 = 0x9D8CFF;
/// An EMP stun: electric cyan.
pub const STUN: u32 = 0x6FE8FF;
/// A warp dampener's field, on the ground and in the key: magenta.
pub const DAMPER: u32 = 0xE25BD0;

/// What a unit is doing, when its drive or a stun has the say: the card's activity line.
pub(super) struct Activity {
    pub label: String,
    /// How far along, for the bar under the line.
    pub progress: Option<f32>,
    /// The figure at the right of the line.
    pub value: String,
    pub tone: u32,
}

/// `u`'s jump under way, as `RenderFrame::warps` lists it.
fn jump_of<'a>(s: &'a Scene, u: &UnitInstance) -> Option<&'a WarpView> {
    s.view.frame.warps.iter().find(|w| w.unit_id == u.unit_id)
}

/// Seconds `u` stays stunned: `Some(Some(s))` for our own listed units, `Some(None)` for
/// anyone else stunned (the mirror only says it is, fading over its last seconds).
pub fn stun_left(view: &crate::game::View, u: &UnitInstance) -> Option<Option<f32>> {
    let listed = crate::sim_thread::queue_of(&view.status.queues, u.unit_id)
        .map(|q| q.stunned)
        .filter(|&t| t > 0.0);
    if listed.is_some() {
        return Some(listed);
    }
    (u.stun(1.0) > 0.01).then_some(None)
}

/// "18 s", rounded up: a stun with a moment left still has a second.
pub fn seconds(t: f32) -> String {
    format!("{} s", t.ceil().max(0.0) as u32)
}

/// The activity line for `u` while it is stunned or its drive is in a jump.
pub(super) fn activity(s: &Scene, u: &UnitInstance) -> Option<Activity> {
    if let Some(left) = stun_left(s.view, u) {
        // The longest stun a dampener gives, for how full the bar starts.
        let longest = s
            .blueprints
            .units
            .iter()
            .filter_map(|b| b.warp_damper)
            .map(|d| d.stun_ticks as f32 / 10.0)
            .fold(1.0, f32::max);
        return Some(Activity {
            label: "Stunned  \u{b7}  systems down".to_owned(),
            progress: left.map(|t| (t / longest).clamp(0.0, 1.0)),
            value: left.map_or_else(String::new, seconds),
            tone: STUN,
        });
    }
    let w = jump_of(s, u)?;
    let tone = if w.dampened { palette::BAD } else { WARP };
    let (label, progress, value) = match w.phase {
        WarpPhase::Spool if !w.aligned => (
            "Charging warp  \u{b7}  coming onto the mark",
            Some(w.charge),
            format!("{:.0}%", w.charge * 100.0),
        ),
        WarpPhase::Spool => (
            "Charging warp",
            Some(w.charge),
            format!("{:.0}%", w.charge * 100.0),
        ),
        WarpPhase::Transit => (
            if w.dampened {
                "In warp  \u{b7}  dampened"
            } else {
                "In warp"
            },
            Some(w.ticks as f32 / w.length.max(1) as f32),
            seconds(w.length.saturating_sub(w.ticks) as f32 / 10.0),
        ),
        WarpPhase::Emerge if w.dampened => ("Thrown out of warp", None, String::new()),
        WarpPhase::Emerge => ("Leaving warp", None, String::new()),
        WarpPhase::Idle => return None,
    };
    Some(Activity {
        label: label.to_owned(),
        progress,
        value,
        tone,
    })
}

/// One line of the band: label, figure, a bar's share, the bar's and the label's colours.
type Line = (String, String, Option<f32>, u32, u32);

/// The drive's line for our own ship: what the jump it is charging for costs, or, when no
/// jump is under way, the drive's price a kilometre (or its recharge).
pub(super) fn drive_line(s: &Scene, u: &UnitInstance, bp: &UnitBlueprint) -> Option<Line> {
    let d = bp.warp?;
    if (u.owner_flags & 0xFF) as u8 != s.view.local {
        return None;
    }
    if let Some(w) = jump_of(s, u) {
        if w.phase != WarpPhase::Spool {
            return None;
        }
        let far = glam::Vec2::new(w.to[0] - w.from[0], w.to[1] - w.from[1]).length();
        return Some((
            format!(
                "Jump {:.1} km  \u{b7}  {} E/km",
                far / 1000.0,
                whole(d.per_km.to_f32())
            ),
            format!("{} E", whole(w.energy)),
            None,
            WARP,
            WARP,
        ));
    }
    let recharge = s.queue_of(u).map_or(0.0, |q| q.warp_recharge);
    Some(if recharge > 0.0 {
        let full = d.cooldown_ticks.max(1) as f32 / 10.0;
        (
            "Warp drive recharging".to_owned(),
            seconds(recharge),
            Some(1.0 - recharge / full),
            WARP,
            palette::DIM,
        )
    } else {
        (
            "Warp drive ready  \u{b7}  O".to_owned(),
            format!("{} E/km", whole(d.per_km.to_f32())),
            None,
            WARP,
            WARP,
        )
    })
}

/// A dampener's line: its field up, or down and why. Only for a field the viewer is
/// shown: an enemy's never is.
pub(super) fn damper_line(s: &Scene, u: &UnitInstance, bp: &UnitBlueprint) -> Option<Line> {
    let spec = bp.warp_damper?;
    if has_flag(u, flag::UNDER_CONSTRUCTION) {
        return None;
    }
    let live = s
        .view
        .frame
        .dampers
        .iter()
        .find(|d| d.unit_id == u.unit_id)?
        .live;
    let radius = format!("{} m", whole(spec.radius.to_f32()));
    let (label, tone) = if live {
        ("Warp field up", DAMPER)
    } else if u.paused() {
        ("Warp field down  \u{b7}  powered down", palette::WARN)
    } else {
        ("Warp field down  \u{b7}  no power", palette::BAD)
    };
    Some((label.to_owned(), radius, None, tone, tone))
}

/// The drive's line (ready, or recharging with the seconds left) or a dampener's, under
/// the activity line. Returns where the next band starts.
pub(super) fn band(
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    bp: &UnitBlueprint,
    x: f32,
    y: f32,
    cw: f32,
) -> f32 {
    let Some((label, value, share, bar_tone, tone)) =
        drive_line(s, u, bp).or_else(|| damper_line(s, u, bp))
    else {
        return y;
    };
    ui.text_fit_left(x, y, cw - 110.0, type_scale::MICRO, rgb(tone, 1.0), &label);
    ui.text_right(
        x + cw,
        y,
        type_scale::VALUE,
        rgb(palette::TEXT, 0.9),
        &value,
    );
    match share {
        Some(p) => {
            let track = Rect::new(x, y + 9.0, cw, 3.0);
            ui.fill(track, rgb(bar_tone, 0.14));
            ui.fill(
                Rect::new(track.x, track.y, track.w * p.clamp(0.0, 1.0), track.h),
                rgb(bar_tone, 0.9),
            );
            y + 24.0
        }
        None => y + 20.0,
    }
}

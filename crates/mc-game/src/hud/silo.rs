//! A strategic launcher in the unit panel (`docs/NUKES.md`): the rounds it holds, the
//! one it is assembling, auto-build and the queue, and on a silo the launch button.
//!
//! A launcher assembles rounds like a factory builds units: on auto-build (the default)
//! it starts the moment it is finished and keeps going until it is full; with auto-build
//! off it assembles only what is queued (the - / + chips). Z pauses it, and engineers
//! assist it like a factory. An interceptor array fires by itself; a silo launches only
//! from its button (or N), at a point anywhere on the map or the minimap.

use super::{Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::game::{Mode, Targeting, View};
use crate::ui::{id, ink, palette, rgb, style, type_scale, Rect, Ui};
use glam::Vec2;
use mc_core::{Fx, FxVec3};
use mc_data::strategic::StrategicKind;
use mc_data::{BlueprintId, Blueprints, UnitBlueprint};
use mc_render::Face;
use mc_sim::mirror::UnitInstance;
use mc_sim::nukes::{self, WarheadPath};
use mc_sim::nukes::{
    LAUNCHER_CAPACITY_SHIFT, LAUNCHER_FIRING, LAUNCHER_MANUAL, LAUNCHER_MARK,
    LAUNCHER_PROGRESS_SHIFT, LAUNCHER_QUEUED_SHIFT, LAUNCHER_STOCK_MASK,
};

/// A warhead's colour: the hot red-orange of the launch controls and the warnings.
pub const WARHEAD: u32 = 0xFF3D1F;
/// An interceptor's colour.
pub const INTERCEPT: u32 = 0x5CD6FF;
/// Something being assembled (construction amber).
const BUILDING: u32 = 0xFFA928;

/// What a launcher's instance says about it (`nukes::LAUNCHER_*`).
#[derive(Clone, Copy, Debug)]
pub struct Launcher {
    pub stock: u32,
    pub capacity: u32,
    /// How far the next round is assembled, 0..1.
    pub progress: f32,
    pub firing: bool,
    pub manual: bool,
    pub queued: u32,
}

impl Launcher {
    pub fn of(u: &UnitInstance) -> Option<Launcher> {
        let p = u.status[2];
        (p & LAUNCHER_MARK != 0).then(|| Launcher {
            stock: p & LAUNCHER_STOCK_MASK,
            capacity: (p >> LAUNCHER_CAPACITY_SHIFT) & 0xFF,
            progress: ((p >> LAUNCHER_PROGRESS_SHIFT) & 0xFF) as f32 / 255.0,
            firing: p & LAUNCHER_FIRING != 0,
            manual: p & LAUNCHER_MANUAL != 0,
            queued: p >> LAUNCHER_QUEUED_SHIFT,
        })
    }

    /// Whether it is assembling a round now (it may still be starved or paused).
    pub fn assembling(&self) -> bool {
        self.stock < self.capacity && (!self.manual || self.queued > 0)
    }
}

fn bp_of<'a>(blueprints: &'a Blueprints, u: &UnitInstance) -> &'a UnitBlueprint {
    blueprints.unit(BlueprintId(u.blueprint as u16))
}

pub fn is_silo(blueprints: &Blueprints, u: &UnitInstance) -> bool {
    bp_of(blueprints, u)
        .strategic
        .as_ref()
        .is_some_and(|s| s.kind == StrategicKind::Nuke)
}

/// A launch sent and not yet seen in the frame: counted against the silo it should come
/// from until the sim shows it, so quick clicks do not spend a warhead twice.
#[derive(Clone, Copy, Debug)]
pub struct SentLaunch {
    pub silo: u32,
    pub at: Vec2,
    /// The frame tick it was sent on.
    pub tick: u32,
}

/// Ticks a sent launch is counted before it is given up on.
const SENT_PATIENCE: u32 = 30;

/// Drops the sent launches the frame now shows (as a launch waiting its turn or a warhead
/// in the air), and any the sim never took.
pub fn settle_sent(view: &mut View) {
    let frame = &view.frame;
    let mut planned: Vec<(u32, Vec2)> = frame
        .planned_launches
        .iter()
        .map(|p| (p.silo, Vec2::from(p.path.mark.xy().to_f32())))
        .collect();
    let mut flying: Vec<Vec2> = frame
        .warhead_tracks
        .iter()
        .filter(|t| t.owner == view.local)
        .map(|t| Vec2::from(t.path.mark.xy().to_f32()))
        .collect();
    view.nuke_sent.retain(|s| {
        if let Some(i) = planned
            .iter()
            .position(|&(silo, at)| silo == s.silo && at.distance(s.at) < 2.0)
        {
            planned.swap_remove(i);
            return false;
        }
        if let Some(i) = flying.iter().position(|at| at.distance(s.at) < 2.0) {
            flying.swap_remove(i);
            return false;
        }
        frame.tick < s.tick + SENT_PATIENCE
    });
}

/// Warheads `u` holds that no launch has spoken for: its stock, less the launches waiting
/// their turn on it and those sent to it and not yet seen.
pub fn free_warheads(view: &View, u: &UnitInstance) -> u32 {
    let Some(l) = Launcher::of(u) else { return 0 };
    let planned = view
        .frame
        .planned_launches
        .iter()
        .filter(|p| p.silo == u.unit_id)
        .count();
    let sent = view
        .nuke_sent
        .iter()
        .filter(|s| s.silo == u.unit_id)
        .count();
    l.stock.saturating_sub((planned + sent) as u32)
}

/// The player's own silos among the selection.
pub fn selected_silos<'a>(
    view: &'a View,
    blueprints: &'a Blueprints,
) -> impl Iterator<Item = &'a UnitInstance> {
    view.selection
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
        .filter(move |u| (u.owner_flags & 0xFF) as u8 == view.local && is_silo(blueprints, u))
}

/// The selected silos with a warhead free.
pub fn armed_silos(view: &View, blueprints: &Blueprints) -> Vec<u32> {
    selected_silos(view, blueprints)
        .filter(|u| free_warheads(view, u) > 0)
        .map(|u| u.unit_id)
        .collect()
}

/// The selected silo a launch at `at` would come from, by the sim's own rule
/// (`nukes::launch_nuke`): the most warheads free, then the nearest, then the lowest id.
pub fn next_silo<'a>(
    view: &'a View,
    blueprints: &'a Blueprints,
    at: Vec2,
) -> Option<&'a UnitInstance> {
    selected_silos(view, blueprints)
        .map(|u| (u, free_warheads(view, u)))
        .filter(|&(_, free)| free > 0)
        .min_by(|(a, fa), (b, fb)| {
            let d = |u: &UnitInstance| Vec2::new(u.pos[0], u.pos[1]).distance_squared(at);
            fb.cmp(fa)
                .then(d(a).total_cmp(&d(b)))
                .then(a.unit_id.cmp(&b.unit_id))
        })
        .map(|(u, _)| u)
}

/// The path a warhead from silo `u` (standing on its `pos`) to the ground at `ground`
/// flies: the sim's own (`nukes::WarheadPath`).
pub fn path_from(blueprints: &Blueprints, u: &UnitInstance, ground: glam::Vec3) -> WarheadPath {
    let fx = |v: glam::Vec3| FxVec3::new(Fx::from_f32(v.x), Fx::from_f32(v.y), Fx::from_f32(v.z));
    let apogee = bp_of(blueprints, u)
        .strategic
        .as_ref()
        .map_or(Fx::from_int(3000), |s| s.apogee);
    WarheadPath::new(
        nukes::warhead_start(fx(glam::Vec3::from(u.pos))),
        nukes::burst_point(fx(ground)),
        apogee,
    )
}

/// Seconds from a launch order at `u` to the burst: the doors, then the flight.
pub fn flight_seconds(blueprints: &Blueprints, u: &UnitInstance, path: &WarheadPath) -> f32 {
    let cruise = bp_of(blueprints, u)
        .strategic
        .as_ref()
        .map_or(Fx::from_int(30), |s| s.speed);
    let ticks = path.ticks_left(cruise, 0, Fx::ZERO) + nukes::SILO_DOOR_TICKS as u32;
    ticks as f32 / mc_core::TICKS_PER_SECOND as f32
}

/// Width of the launcher panel (`panel`), right of the order card.
pub const WIDTH: f32 = 372.0;

/// The first launcher among the selection, and what it says about itself.
pub fn launcher_of<'a>(
    s: &Scene,
    units: &[&'a UnitInstance],
) -> Option<(&'a UnitInstance, Launcher)> {
    units.iter().find_map(|u| {
        s.blueprints
            .unit(BlueprintId(u.blueprint as u16))
            .strategic
            .as_ref()?;
        Launcher::of(u).map(|l| (*u, l))
    })
}

/// Every silo of ours picked, taken together: a launch goes to whichever of them has the
/// most warheads free, so the panel counts them as one battery.
#[derive(Clone, Debug, Default)]
pub struct Battery {
    pub silos: u32,
    /// Each silo's rounds (`Launcher`) and its warheads free.
    pub launchers: Vec<(u32, Launcher, u32)>,
}

impl Battery {
    pub fn of(view: &View, blueprints: &Blueprints) -> Battery {
        let launchers: Vec<_> = selected_silos(view, blueprints)
            .filter_map(|u| Launcher::of(u).map(|l| (u.unit_id, l, free_warheads(view, u))))
            .collect();
        Battery {
            silos: launchers.len() as u32,
            launchers,
        }
    }

    pub fn ready(&self) -> u32 {
        self.launchers.iter().map(|(_, l, _)| l.stock).sum()
    }

    pub fn capacity(&self) -> u32 {
        self.launchers.iter().map(|(_, l, _)| l.capacity).sum()
    }

    /// Warheads no launch has spoken for.
    pub fn free(&self) -> u32 {
        self.launchers.iter().map(|(_, _, f)| f).sum()
    }

    /// Warheads given a mark and not yet away.
    pub fn targeted(&self) -> u32 {
        self.ready() - self.free()
    }

    /// Silos assembling a warhead now, and how far along the nearest to done is.
    pub fn assembling(&self) -> (u32, f32) {
        let busy = self.launchers.iter().filter(|(_, l, _)| l.assembling());
        (
            busy.clone().count() as u32,
            busy.map(|(_, l, _)| l.progress).fold(0.0, f32::max),
        )
    }

    /// The battery as one launcher, for what the panel draws of a single one.
    pub fn as_one(&self, own: &Launcher) -> Launcher {
        let (busy, progress) = self.assembling();
        Launcher {
            stock: self.ready(),
            capacity: self.capacity(),
            progress: if busy > 0 { progress } else { own.progress },
            firing: self.targeted() > 0 && self.free() == 0,
            manual: own.manual,
            queued: own.queued,
        }
    }
}

/// A launcher's panel over `r`, glass and all: its rounds, assembly, auto-build and the
/// queue, and on a silo the launch button. Several silos picked read as one battery.
pub fn panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, u: &UnitInstance, l: &Launcher, r: Rect) {
    let bp = s.blueprints.unit(BlueprintId(u.blueprint as u16));
    let Some(spec) = bp.strategic.as_ref() else {
        return;
    };
    hud.glass(ui, r);
    let silo = spec.kind == StrategicKind::Nuke;
    let tone = if silo { WARHEAD } else { INTERCEPT };
    let own = (u.owner_flags & 0xFF) as u8 == s.view.local && !s.view.observing;
    let battery = (silo && own)
        .then(|| Battery::of(s.view, s.blueprints))
        .filter(|b| b.silos > 0);
    let whole = battery.as_ref().map(|b| b.as_one(l));
    let l = whole.as_ref().unwrap_or(l);
    let silos = battery.as_ref().map_or(1, |b| b.silos);
    let free = battery.as_ref().map_or(l.stock, |b| b.free());
    let targeted = battery.as_ref().map_or(0, |b| b.targeted());
    let ids: Vec<mc_sim::Handle> = match &battery {
        Some(b) => b
            .launchers
            .iter()
            .map(|(id, ..)| mc_sim::Handle(*id))
            .collect(),
        None => vec![mc_sim::Handle(u.unit_id)],
    };
    let (x, cw) = (r.x + 16.0, r.w - 32.0);
    let mut y = r.y + 22.0;

    // Header: what it holds, how many, and auto-build on the right.
    let title = if silo { "Warheads" } else { "Interceptors" };
    ui.text(x, y, type_scale::ITEM, rgb(palette::TEXT, 1.0), title);
    let label_w = ui.text_width(type_scale::ITEM, title) + 12.0;
    let count = format!("{} / {}", l.stock, l.capacity);
    ui.text(
        x + label_w,
        y + 1.0,
        type_scale::VALUE,
        rgb(tone, 1.0),
        &count,
    );
    if silos > 1 {
        let sx = x + label_w + ui.text_width(type_scale::VALUE, &count) + 10.0;
        ui.text(
            sx,
            y + 1.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("\u{b7}  {silos} silos"),
        );
    }
    if own {
        let on = !l.manual;
        let value = if on { "On" } else { "Off" };
        let w = ui.text_width(type_scale::MICRO, "Auto-build")
            + ui.text_width(type_scale::VALUE, value)
            + 34.0;
        let (clicked, _) = hud.chip(
            ui,
            id("launcher-auto", 0),
            x + cw - w,
            y - 12.0,
            "Auto-build",
            value,
            if on { tone } else { palette::DIM },
        );
        if clicked {
            ui.audio
                .play(if on { Sfx::ToggleOff } else { Sfx::ToggleOn });
            hud.actions
                .push(HudAction::Send(mc_sim::Command::SetAutoBuild {
                    units: ids.clone(),
                    on: !on,
                }));
        }
    }
    y += 20.0;

    // The rounds: one slot each, ready ones lit (those given a mark first, ringed), the
    // ones being assembled filling up. A battery's slots narrow to fit, and past what fits
    // at their narrowest the rest are counted, not drawn.
    let gap = if l.capacity > 6 { 3.0 } else { 5.0 };
    let fit = |room: f32| (((room + gap) / (7.0 + gap)).floor() as u32).max(1);
    let shown = if l.capacity <= fit(cw * 0.5) {
        l.capacity
    } else {
        fit(cw * 0.5 - 34.0)
    };
    let slot_w = ((cw * 0.5) / shown.max(1) as f32 - gap).clamp(7.0, 16.0);
    let paused = u.paused();
    let fills: Vec<f32> = match &battery {
        Some(b) => {
            let mut building: Vec<f32> = b
                .launchers
                .iter()
                .filter(|(_, l, _)| l.assembling())
                .map(|(_, l, _)| l.progress)
                .collect();
            building.sort_by(|a, b| b.total_cmp(a));
            (0..l.capacity)
                .map(|i| {
                    if i < l.stock {
                        1.0
                    } else {
                        building.get((i - l.stock) as usize).copied().unwrap_or(0.0)
                    }
                })
                .collect()
        }
        None => (0..l.capacity)
            .map(|i| {
                if i < l.stock {
                    1.0
                } else if i == l.stock && l.assembling() {
                    l.progress
                } else {
                    0.0
                }
            })
            .collect(),
    };
    for i in 0..shown {
        let at = Rect::new(x + i as f32 * (slot_w + gap), y, slot_w, 32.0);
        round_slot(
            ui,
            at,
            fills[i as usize],
            i < l.stock,
            if i < l.stock { tone } else { BUILDING },
            silo,
        );
        if i < targeted {
            // Spoken for: a mark under it.
            ui.fill(
                Rect::new(at.x, at.bottom() + 3.0, at.w, 2.0),
                rgb(0xFFFFFF, 0.85),
            );
        }
    }
    let mut sx = x + shown as f32 * (slot_w + gap);
    if shown < l.capacity {
        // The rounds past the cut: how many, lit if any of them are ready.
        let more = format!("+{}", l.capacity - shown);
        let lit = l.stock > shown;
        ui.text(
            sx + 2.0,
            y + 10.0,
            type_scale::MICRO,
            rgb(if lit { tone } else { palette::DIM }, 1.0),
            &more,
        );
        sx += ui.text_width(type_scale::MICRO, &more) + 4.0;
    }
    // What it is doing, right of the slots.
    let sx = sx + 10.0;
    let seconds = (1.0 - l.progress) * spec.round_seconds();
    let (state, state_tone) = if targeted > 0 {
        (format!("{targeted} targeted  \u{b7}  {free} free"), WARHEAD)
    } else if l.firing {
        ("Launching".to_owned(), WARHEAD)
    } else if l.stock >= l.capacity {
        ("Full".to_owned(), tone)
    } else if paused {
        ("Paused".to_owned(), palette::WARN)
    } else if l.assembling() {
        (format!("Assembling  {:.0}%", l.progress * 100.0), BUILDING)
    } else {
        ("Idle: queue a round".to_owned(), palette::DIM)
    };
    ui.text(sx, y + 8.0, type_scale::VALUE, rgb(state_tone, 1.0), &state);
    let busy = battery
        .as_ref()
        .map_or(u32::from(l.assembling()), |b| b.assembling().0);
    let detail = if targeted > 0 && free > 0 {
        "Shift-click queues more marks; each silo fires its own in turn.".to_owned()
    } else if targeted > 0 {
        "Every warhead has a mark; each silo fires its own in turn.".to_owned()
    } else if busy > 1 {
        format!("{busy} assembling  \u{b7}  next in {}", clock(seconds))
    } else if l.assembling() && !paused && !l.firing {
        format!("{} left  \u{b7}  engineers can assist", clock(seconds))
    } else if paused {
        "Z resumes assembly".to_owned()
    } else if l.stock > 0 && silo {
        "Armed. Launch at any point on the map.".to_owned()
    } else if !silo && l.stock > 0 {
        "Ready. Fires by itself.".to_owned()
    } else {
        format!("{} a round", clock(spec.round_seconds()))
    };
    ui.text_fit_left(
        sx,
        y + 25.0,
        x + cw - sx,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &detail,
    );
    y += 38.0;

    // The assembly line: a thin bar, amber while it works.
    ui.fill(Rect::new(x, y, cw, 3.0), rgb(palette::LINE, 0.08));
    if l.assembling() && l.stock < l.capacity {
        let pulse = if paused {
            0.45
        } else {
            0.8 + 0.2 * (ui.time * 5.0).sin()
        };
        ui.fill(Rect::new(x, y, cw * l.progress, 3.0), rgb(BUILDING, pulse));
    }
    y += 16.0;

    // With auto-build off: how many are queued, and less / more.
    if l.manual && own {
        ui.text(
            x,
            y + 8.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Queued",
        );
        let qx = x + ui.text_width(type_scale::MICRO, "Queued") + 12.0;
        let room = l.capacity.saturating_sub(l.stock);
        let (minus, w1) = hud.chip(
            ui,
            id("launcher-queue", 0),
            qx,
            y - 4.0,
            "Less",
            "\u{2212}",
            palette::DIM,
        );
        let vx = qx + w1 + 10.0;
        ui.text(
            vx,
            y + 8.0,
            type_scale::VALUE,
            rgb(tone, 1.0),
            &format!("{} / {}", l.queued, room),
        );
        let (plus, _) = hud.chip(
            ui,
            id("launcher-queue", 1),
            vx + 44.0,
            y - 4.0,
            "More",
            "+",
            tone,
        );
        for (clicked, count) in [(minus, -1i16), (plus, 1)] {
            if clicked {
                ui.audio.play(Sfx::Tick);
                hud.actions
                    .push(HudAction::Send(mc_sim::Command::QueueRounds {
                        units: vec![mc_sim::Handle(u.unit_id)],
                        count,
                    }));
            }
        }
        y += 28.0;
    }

    let foot = r.bottom() - 16.0;
    if silo {
        let h = (foot - 20.0 - y).clamp(46.0, 70.0);
        let button = Rect::new(x, foot - 18.0 - h, cw, h);
        launch_button(hud, ui, s, l, free, paused, own, button);
    } else {
        let km = spec.coverage.to_f32() / 1000.0;
        let ay = foot - 44.0;
        ui.text(x, ay, type_scale::VALUE, rgb(INTERCEPT, 1.0), "Automatic");
        ui.text_fit_left(
            x,
            ay + 18.0,
            cw,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("Shoots down enemy warheads coming down within {km:.1} km"),
        );
    }
    let hint = if silo {
        "N arms the launch  \u{b7}  Z pauses assembly  \u{b7}  engineers assist"
    } else {
        "One interceptor a warhead  \u{b7}  Z pauses  \u{b7}  engineers assist"
    };
    ui.text_fit_left(
        x,
        foot - 4.0,
        cw,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        hint,
    );
}

/// A round's slot: a missile's outline, filled from the bottom as it is assembled.
pub(crate) fn round_slot(ui: &mut Ui, r: Rect, fill: f32, ready: bool, tone: u32, warhead: bool) {
    let body = Rect::new(r.x + r.w * 0.28, r.y + r.h * 0.26, r.w * 0.44, r.h * 0.64);
    let nose_top = Vec2::new(r.x + r.w * 0.5, r.y + 1.0);
    let nose_l = Vec2::new(body.x, body.y);
    let nose_r = Vec2::new(body.right(), body.y);
    let fins = Rect::new(
        r.x + r.w * 0.12,
        r.bottom() - r.h * 0.2,
        r.w * 0.76,
        r.h * 0.12,
    );
    // The empty shape, faint.
    ui.fill(body, rgb(palette::LINE, 0.08));
    ui.triangle(nose_top, nose_l, nose_r, rgb(palette::LINE, 0.08));
    ui.fill(fins, rgb(palette::LINE, 0.08));
    if fill <= 0.0 {
        ui.frame(body, rgb(palette::LINE, 0.18));
        return;
    }
    let glow = if ready { 1.0 } else { 0.75 };
    let color = rgb(tone, glow);
    // Filled from the tail up.
    let top = r.bottom() - (r.h - 1.0) * fill;
    ui.fill(
        Rect::new(
            fins.x,
            fins.y.max(top),
            fins.w,
            (fins.bottom() - fins.y.max(top)).max(0.0),
        ),
        color,
    );
    if top < body.bottom() {
        let y0 = body.y.max(top);
        ui.fill(Rect::new(body.x, y0, body.w, body.bottom() - y0), color);
    }
    if top < body.y {
        // The nose, cut level where the fill stops.
        let k = ((body.y - top) / (body.y - nose_top.y)).clamp(0.0, 1.0);
        let apex = nose_l.lerp(nose_top, k);
        let apex_r = nose_r.lerp(nose_top, k);
        ui.triangle(nose_l, nose_r, apex_r, color);
        ui.triangle(nose_l, apex_r, apex, color);
    }
    if ready && warhead {
        // A warhead's band.
        ui.fill(
            Rect::new(body.x, body.y + body.h * 0.3, body.w, 2.0),
            rgb(palette::INK, 0.6),
        );
    }
}

fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The launch button. Armed, it is the loudest thing on the card: hazard bands crawling
/// along its edges, a red core breathing behind a trefoil, brackets at its corners.
/// Otherwise it is dark, its foot band filling amber as the next warhead is assembled.
fn launch_button(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    l: &Launcher,
    free: u32,
    paused: bool,
    own: bool,
    r: Rect,
) {
    hud.claim(ui, r);
    let armed = own && free > 0;
    let targeting = s.view.mode == Mode::Target(Targeting::Nuke);
    let res = ui.interact(id("launch-button", 0), r, armed);
    let t = ui.time;
    let hot = ui.ease(id("launch-hot", 0), if armed { 1.0 } else { 0.0 }, 6.0);
    let sel = ui.ease(id("launch-sel", 0), if targeting { 1.0 } else { 0.0 }, 10.0);
    let sink = if res.held { 1.5 } else { 0.0 };
    let r = Rect::new(r.x, r.y + sink, r.w, r.h);

    // Glass, then the core glow breathing in the middle.
    ui.fill_cut(r, 8.0, ink(0.78));
    let breathe = 0.55 + 0.45 * (t * if targeting { 7.0 } else { 2.4 }).sin();
    let core = hot * (0.16 + 0.12 * breathe + 0.2 * res.glow + 0.2 * sel);
    ui.gradient_h(r.inset(2.0), rgb(WARHEAD, core), rgb(WARHEAD, core * 0.15));
    ui.gradient_v(r.inset(2.0), rgb(0xFFFFFF, 0.05 * hot), rgb(0xFFFFFF, 0.0));

    // Hazard bands along the top and the foot: diagonal stripes that crawl while armed.
    let band = 6.0;
    let speed = if targeting {
        60.0
    } else if armed {
        14.0 + 30.0 * res.glow
    } else {
        0.0
    };
    let stripe = 12.0;
    let offset = (t * speed) % (stripe * 2.0);
    let stripe_tone = if armed {
        rgb(WARHEAD, 0.85)
    } else {
        rgb(palette::LINE, 0.14)
    };
    let (lo, hi) = (r.x + 10.0, r.right() - 10.0);
    let stripes = |ui: &mut Ui, y0: f32, dir: f32, end: f32, tone| {
        let mut sx = lo - stripe * 2.0 + offset * dir.signum();
        while sx < end {
            let a = Vec2::new(sx.max(lo), y0 + band);
            let b = Vec2::new((sx + stripe).clamp(lo, end), y0 + band);
            let c = Vec2::new((sx + stripe + band).clamp(lo, end), y0);
            let d = Vec2::new((sx + band).clamp(lo, end), y0);
            if b.x > a.x + 0.5 {
                ui.triangle(a, b, c, tone);
                ui.triangle(a, c, d, tone);
            }
            sx += stripe * 2.0;
        }
    };
    let foot = r.bottom() - 2.0 - band;
    stripes(ui, r.y + 2.0, 1.0, hi, stripe_tone);
    stripes(ui, foot, -1.0, hi, stripe_tone);
    // Not armed: the foot band lights up amber as the next warhead is assembled.
    if !armed && l.assembling() && !l.firing {
        stripes(
            ui,
            foot,
            -1.0,
            lo + (hi - lo) * l.progress,
            rgb(BUILDING, if paused { 0.4 } else { 0.9 }),
        );
    }
    ui.outline_cut(
        r,
        8.0,
        rgb(
            if armed { WARHEAD } else { palette::LINE },
            0.25 + 0.5 * hot * (0.5 + 0.5 * res.glow.max(sel)),
        ),
        rgb(0xFFFFFF, 0.3 + 0.5 * hot),
    );

    // The trefoil on the left.
    let c = Vec2::new(r.x + 34.0, r.mid_y());
    let mark = if armed {
        rgb(WARHEAD, 0.75 + 0.25 * breathe)
    } else {
        rgb(palette::LINE, 0.25)
    };
    let spin = if targeting { t * 1.6 } else { 0.0 };
    for k in 0..3 {
        let a = spin + k as f32 * std::f32::consts::TAU / 3.0 - std::f32::consts::FRAC_PI_2;
        ui.arc(c, 9.5, a - 0.52, a + 0.52, 9.0, mark);
    }
    ui.disc(c, 3.2, mark);
    ui.arc(
        c,
        17.0,
        0.0,
        std::f32::consts::TAU,
        1.2,
        if armed {
            rgb(WARHEAD, 0.5)
        } else {
            rgb(palette::LINE, 0.12)
        },
    );

    // The words.
    let label = if l.firing && !armed {
        "Launching"
    } else if targeting {
        "Select target"
    } else if armed {
        "Launch"
    } else if !own {
        "Silo"
    } else if paused {
        "Assembly paused"
    } else if l.assembling() {
        "Assembling warhead"
    } else {
        "No warhead"
    };
    let big = style(Face::Bold, 22.0, 1.2);
    let blink = if l.firing {
        0.55 + 0.45 * (t * 9.0).sin().abs()
    } else {
        1.0
    };
    let ink_tone = if armed || l.firing {
        rgb(0xFFFFFF, blink)
    } else {
        rgb(palette::DIM, 0.8)
    };
    ui.text(r.x + 62.0, r.mid_y() - 6.0, big, ink_tone, label);
    let sub = if l.firing && !armed {
        "Blast doors open  \u{b7}  ignition".to_owned()
    } else if targeting && free > 1 {
        format!("{free} free  \u{b7}  shift-click queues each  \u{b7}  RMB cancels")
    } else if targeting {
        "Anywhere on the map or the minimap  \u{b7}  RMB cancels".to_owned()
    } else if armed && free < l.stock {
        format!("{free} of {} warheads free", l.stock)
    } else if armed {
        format!("{} warhead{} armed", free, if free == 1 { "" } else { "s" })
    } else if l.assembling() {
        format!("{:.0}% assembled", l.progress * 100.0)
    } else {
        "Queue a warhead, or turn on auto-build".to_owned()
    };
    ui.text_fit_left(
        r.x + 62.0,
        r.mid_y() + 13.0,
        r.w - 100.0,
        type_scale::MICRO,
        rgb(if armed { WARHEAD } else { palette::FAINT }, 0.9),
        &sub,
    );
    ui.key_cap(r.right() - 26.0, r.mid_y() - 7.5, "N", armed);
    if armed {
        ui.brackets(
            r.inset(-3.0),
            7.0,
            rgb(WARHEAD, 0.5 + 0.5 * res.glow.max(sel)),
        );
    }
    if res.clicked && armed {
        ui.audio.play(Sfx::Select);
        // Toggles: pressed again while choosing a target, it stands down.
        hud.actions.push(HudAction::Target(Targeting::Nuke));
    }
}

// ------------------------------------------------------------------ alerts

/// Which events have been turned into notes already.
#[derive(Default)]
pub struct Alerts {
    /// The last tick whose events were read.
    tick: u32,
}

/// Warheads in flight become one live card per side (an enemy's loud, ours quieter),
/// however many there are: the count, the soonest impact and the last, a tick for each
/// on a time strip, and a click steps through where they will land. What just happened
/// (a warhead intercepted, a warhead ready) goes up as a note that counts repeats.
pub fn alerts(hud: &mut Hud, s: &Scene) {
    use super::notices::{Glyph, Live};
    use mc_sim::mirror::{SimEvent, STRATEGIC_WARHEAD};
    let view = s.view;
    let local = view.local;
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    if view.frame.tick != hud.nuke_alerts.tick {
        hud.nuke_alerts.tick = view.frame.tick;
        for e in &view.frame.events {
            match e {
                SimEvent::WarheadIntercepted {
                    pos, killed: true, ..
                } => {
                    let at = Vec2::from(pos.xy().to_f32());
                    hud.notices.note(
                        "warhead-intercepted",
                        "Warhead intercepted",
                        INTERCEPT,
                        Glyph::Intercept,
                        Some(at),
                    );
                }
                SimEvent::RoundReady {
                    pos,
                    owner,
                    warhead: true,
                    ..
                } if *owner == local && !view.observing => {
                    let at = Vec2::from(pos.xy().to_f32());
                    hud.notices.note(
                        "warhead-ready",
                        "Warhead ready",
                        WARHEAD,
                        Glyph::Trefoil,
                        Some(at),
                    );
                }
                _ => {}
            }
        }
    }

    // Enemy, ours, allied: in that order down the screen.
    let mut sides: [Vec<(f32, Vec2, bool)>; 3] = Default::default();
    for m in view
        .frame
        .strategic
        .iter()
        .filter(|m| m.kind == STRATEGIC_WARHEAD)
    {
        let owner = m.owner as u8;
        let side = if !view.observing && team(owner) != team(local) {
            0
        } else if owner == local && !view.observing {
            1
        } else {
            2
        };
        sides[side].push((
            m.eta.max(0.0),
            Vec2::new(m.mark[0], m.mark[1]),
            m.boost > 0.5,
        ));
    }
    for (side, group) in sides.iter().enumerate() {
        let n = group.len();
        if n == 0 {
            continue;
        }
        let (key, title) = match (side, n) {
            (0, 1) => ("nuke-enemy", "Nuclear launch detected".to_owned()),
            (0, _) => ("nuke-enemy", format!("{n} nuclear launches detected")),
            (1, 1) => ("nuke-own", "Warhead away".to_owned()),
            (1, _) => ("nuke-own", format!("{n} warheads away")),
            (_, 1) => (
                "nuke-ally",
                if view.observing {
                    "Nuclear launch"
                } else {
                    "Allied launch"
                }
                .to_owned(),
            ),
            _ => (
                "nuke-ally",
                format!(
                    "{n} {}",
                    if view.observing {
                        "nuclear launches"
                    } else {
                        "allied launches"
                    }
                ),
            ),
        };
        let flying: Vec<f32> = group.iter().filter(|w| !w.2).map(|w| w.0).collect();
        let boosting = n - flying.len();
        let soonest = flying.iter().copied().fold(f32::INFINITY, f32::min);
        let last = flying.iter().copied().fold(0.0f32, f32::max);
        let mut sub = match flying.len() {
            0 => String::new(),
            1 => format!("Impact in {}", clock(soonest)),
            _ => format!(
                "Next impact {}  \u{b7}  last {}",
                clock(soonest),
                clock(last)
            ),
        };
        if boosting > 0 {
            if !sub.is_empty() {
                sub.push_str("  \u{b7}  ");
            }
            sub.push_str(&if n == 1 {
                "Leaving the silo".to_owned()
            } else {
                format!("{boosting} leaving the silo")
            });
        }
        let figure = soonest
            .is_finite()
            .then(|| format!("{:.0}", soonest.ceil()));
        hud.notices.live(Live {
            key,
            title,
            sub,
            tone: if side == 0 { palette::BAD } else { WARHEAD },
            glyph: Glyph::Trefoil,
            loud: side == 0,
            figure,
            marks: group.iter().map(|w| (w.0, w.1)).collect(),
        });
    }
}

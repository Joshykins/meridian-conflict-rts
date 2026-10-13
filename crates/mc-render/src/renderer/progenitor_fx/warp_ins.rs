//! The Progenitor's warp-ins (`mc_sim::survival`, `RenderFrame::warp_ins`): a
//! Seedcaster's squad or a Gatewright's fleet marked to come in beside the defenders.
//! Not a capital ship's rift: the Progenitor opens gates of hard light. Bigger, with
//! more blades and brighter, the higher its Shaper's level.
//!
//! **A squad** (a Seedcaster's):
//! - **Marked**: a ring of arcs kicked out round the Seedcaster's crown and lines of
//!   light thrown up toward the mark; for the whole countdown its Lens charges (a
//!   crystal of light growing in it, rings closing round it).
//! - **Counting down**: a gate forms on the ground round the mark. Blades of hard light
//!   rise out of the ground in a ring one after another, lines of light join their heads
//!   as they stand, and in the last seconds a lattice dome closes over it; the mark turns
//!   inside, beating faster, and a line of light stands up out of it.
//! - **Arrived**: at each unit's spot a tear of light opens, the unit unfolding out of
//!   it (the sim brings it two thirds whole and finishes it in a second); the gate's
//!   blades are flung down, a shock ring runs out to the sim's radius, and a mark is
//!   left burnt into the ground.
//! - **Cancelled** (a dampening field came up over it): the gate shatters, each blade
//!   breaking into lines of light thrown off it, the mark breaking outward.
//!
//! **A fleet** (a Gatewright's):
//! - **Printing** aloft before the Gatewright: under each ship its print beam reaches, a
//!   cradle of hard light, a ring of arcs and a scan ring running up and down the hull.
//! - **Marked**: a gate in the sky over the mark at the fleet's cruise height for the
//!   whole lead, two great rings of arcs turning against each other (one lying over the
//!   mark, one standing across the fleet's way in), lines of light standing up into
//!   them from the mark as it comes due.
//! - **The jump** is any ship's warp (`warp_fx.rs`); over it each Progenitor ship goes
//!   through a ring gate that closes behind it as it leaves, and comes out of one that
//!   opens in the sky, with lines of light streaking in after it.

use super::{lens_of, tier_of, Piece, HOT, LIGHT};
use crate::gpu_consts::progenitor_fx::{
    ARCS, CLOSE, DOME, DRAPE, LIGHT as SHARD, LINE, MARK, TARGET,
};
use crate::renderer::Renderer;
use glam::{Vec2, Vec3};
use mc_sim::mirror::{RenderFrame, SimEvent};
use mc_sim::survival::{WarpInInstance, WarpInKind, BEAM_PRINT};
use std::f32::consts::TAU;

/// Seconds before a squad comes in that the lattice dome closes over its gate.
const DOME_LEAD: f32 = 2.5;
/// The Gatewright's kind (`ShaperInstance::kind`, `SiteKind::Fleet`).
const GATEWRIGHT: u8 = 3;

/// The warp-ins the renderer knows of.
#[derive(Default)]
pub(super) struct WarpInFx {
    /// Marked this tick, and last tick (an arrival or a cancel is heard the tick its mark
    /// is gone).
    live: Vec<WarpInInstance>,
    before: Vec<WarpInInstance>,
    /// The Progenitor's player, from its Shapers (`UnitInstance::owner_flags`).
    owner: Option<u32>,
    /// Ships being printed aloft this tick: where, their radius and height.
    cradles: Vec<(Vec3, f32, f32)>,
}

impl WarpInFx {
    /// Mark `serial`, this tick's or last tick's.
    fn find(&self, serial: u32) -> Option<&WarpInInstance> {
        self.live
            .iter()
            .chain(self.before.iter())
            .find(|w| w.serial == serial)
    }
}

/// The gate round a squad's mark: its blades' count, how far out they stand, and how
/// tall they rise.
fn gate(w: &WarpInInstance) -> (usize, f32, f32) {
    let level = f32::from(w.level);
    let n = (8 + usize::from(w.level) / 2).min(28);
    let r = w.reach.max(w.radius) * 1.05 + 6.0;
    let tall = (12.0 + r * 0.25 + level * 0.8).min(140.0);
    (n, r, tall)
}

/// Where blade `k` of `n` stands, `r` out round `at` (`turn` its ring's turn), and the
/// way out from the middle there.
fn blade_foot(at: Vec3, k: usize, n: usize, r: f32, turn: f32) -> (Vec3, Vec3) {
    let a = turn + k as f32 / n as f32 * TAU;
    let out = Vec3::new(a.cos(), a.sin(), 0.0);
    (at + out * r, out)
}

/// The fleet's cruise height over its mark: its ships' spots, at their middle.
fn sky_height(w: &WarpInInstance) -> f32 {
    if w.spots.is_empty() {
        return w.at[2] + 300.0;
    }
    w.spots.iter().map(|s| s[2]).sum::<f32>() / w.spots.len() as f32
}

impl Renderer {
    /// Once a tick, before its events: the warp-ins marked now, a crown pulse for each
    /// one new, and the ships being printed aloft.
    pub(super) fn warp_in_tick(&mut self, frame: &RenderFrame, time: f32) {
        let fresh: Vec<WarpInInstance> = frame
            .warp_ins
            .iter()
            .filter(|w| {
                !self
                    .progenitor_fx
                    .warp
                    .live
                    .iter()
                    .any(|l| l.serial == w.serial)
            })
            .cloned()
            .collect();
        let warp = &mut self.progenitor_fx.warp;
        warp.before = std::mem::replace(&mut warp.live, frame.warp_ins.clone());
        warp.owner = frame
            .shapers
            .iter()
            .find_map(|s| frame.units.iter().find(|u| u.unit_id == s.unit))
            .map(|u| u.owner_flags & 0xFF);
        let mut cradles = Vec::new();
        for b in frame.beams.iter().filter(|b| b.kind == BEAM_PRINT) {
            let to = Vec3::from(b.to);
            if to.z - self.progenitor_ground(to) > 40.0 {
                cradles.push((to, b.radius.max(4.0), b.height.max(4.0)));
            }
        }
        self.progenitor_fx.warp.cradles = cradles;
        for w in fresh {
            self.warp_in_marked(&w, time);
        }
    }

    /// Every frame: each warp-in's gate, mark and charge, and the cradles under ships
    /// being printed aloft.
    pub(super) fn warp_in_frame(&mut self, out: &mut Vec<Piece>, time: f32, alpha: f32) {
        let tick = self.tick_seconds.max(0.02);
        let live = self.progenitor_fx.warp.live.clone();
        for w in &live {
            let whole = (w.age + w.eta).max(0.1);
            let left = (w.eta - alpha * tick).max(0.0);
            let near = 1.0 - (left / whole).clamp(0.0, 1.0);
            match w.kind {
                WarpInKind::Squad => self.squad_gate(out, w, near, left, time),
                WarpInKind::Fleet => self.sky_gate(out, w, near, time),
            }
        }
        for (at, r, h) in &self.progenitor_fx.warp.cradles {
            out.push(
                Piece::ring(*at - Vec3::Z * h * 0.5, Vec3::Z)
                    .size(r * 1.5, r * 1.5)
                    .width(r * 0.12 + 0.5)
                    .power(1.4)
                    .flags(ARCS)
                    .tier(3.0)
                    .seed(time * 0.05)
                    .now(time),
            );
            let scan = (time * 0.7 + at.x * 0.01).sin() * 0.5;
            out.push(
                Piece::ring(*at + Vec3::Z * h * scan, Vec3::Z)
                    .size(r * 1.15, r * 1.15)
                    .width(r * 0.05 + 0.3)
                    .power(1.0)
                    .now(time),
            );
        }
    }

    /// A squad's gate forming round its mark `near` of the way through its countdown,
    /// `left` seconds to go; its Seedcaster's Lens charging.
    fn squad_gate(
        &mut self,
        out: &mut Vec<Piece>,
        w: &WarpInInstance,
        near: f32,
        left: f32,
        time: f32,
    ) {
        let mark = Vec3::from(w.at);
        let level = f32::from(w.level);
        let (n, r, tall) = gate(w);
        let turn = (w.serial % 17) as f32 * 0.37;
        let beat = 0.8 + 0.2 * (time * (3.0 + 12.0 * near * near)).sin();
        out.push(
            Piece::ring(mark, Vec3::Z)
                .size(w.radius, w.radius)
                .power((0.6 + 1.6 * near * near) * beat)
                .flags(DRAPE | TARGET)
                .now(time),
        );
        // The blades rising out of the ground one after another, and the lines joining
        // the heads of those that stand.
        let mut heads = Vec::with_capacity(n);
        for k in 0..n {
            let (foot, outward) = blade_foot(mark, k, n, r, turn);
            let foot = foot.truncate().extend(self.progenitor_ground(foot));
            let up = (near * 1.35 - k as f32 / n as f32 * 0.35).clamp(0.0, 1.0);
            let h = tall * up * up * (3.0 - 2.0 * up);
            heads.push((foot + Vec3::Z * h, up));
            if h < 0.5 {
                continue;
            }
            out.push(
                Piece::body(foot + Vec3::Z * h, Vec3::Z, outward)
                    .size(tall * 0.07, h)
                    .power(0.8 + 1.4 * near + level * 0.02)
                    .flags(SHARD)
                    .now(time),
            );
        }
        for k in 0..n {
            let (a, ua) = heads[k];
            let (b, ub) = heads[(k + 1) % n];
            if ua >= 1.0 && ub >= 1.0 {
                out.push(
                    Piece::beam(a, b)
                        .size(0.8 + tall * 0.01, 0.8 + tall * 0.01)
                        .power(0.6 + 1.2 * near)
                        .flags(LINE)
                        .now(time),
                );
            }
        }
        if left < DOME_LEAD {
            // The lattice closing over it.
            let k = 1.0 - left / DOME_LEAD;
            out.push(
                Piece::shell(mark, 0.05 + 0.45 * k)
                    .size(r, r)
                    .power(0.5 + 1.2 * k)
                    .flags(DOME)
                    .seed(turn)
                    .now(time),
            );
        }
        if left < 3.0 {
            // A line of light standing up out of the mark to meet it.
            let k = 1.0 - left / 3.0;
            out.push(
                Piece::beam(mark, mark + Vec3::Z * (120.0 + 380.0 * k))
                    .size(1.2 + 2.0 * k, 0.3)
                    .power(0.6 + 1.6 * k)
                    .flags(LINE)
                    .now(time),
            );
        }
        // The Seedcaster's Lens charging through the countdown.
        if let Some((s, foot)) = self.progenitor_fx.shaper(w.source).cloned() {
            let lens = lens_of(&s, foot);
            let tier = tier_of(&s).min(6.0);
            let len = (10.0 + 3.0 * tier) * (0.3 + 0.7 * near);
            out.push(
                Piece::body(lens + Vec3::Z * len * 0.5, Vec3::Z, Vec3::X)
                    .size((2.0 + 0.5 * tier) * (0.4 + 0.6 * near), len)
                    .power(0.6 + 2.0 * near)
                    .flags(SHARD)
                    .now(time),
            );
            let reach = s.height.max(20.0) * 0.08 * (1.2 - near) + 10.0;
            for k in 0..2 {
                let spin = time * (0.6 + 0.4 * k as f32) * if k == 1 { -1.0 } else { 1.0 };
                let tilt = 0.5 * k as f32;
                let normal = Vec3::new(tilt * spin.cos(), tilt * spin.sin(), 1.0).normalize();
                out.push(
                    Piece::ring(lens, normal)
                        .size(reach, reach)
                        .width(1.5 + 0.3 * tier)
                        .power(0.5 + 1.8 * near)
                        .flags(ARCS)
                        .tier(tier)
                        .seed(turn + k as f32 * 0.3)
                        .now(time),
                );
            }
        }
    }

    /// A fleet's gate in the sky over its mark, `near` of the way through its lead.
    fn sky_gate(&mut self, out: &mut Vec<Piece>, w: &WarpInInstance, near: f32, time: f32) {
        let mark = Vec3::from(w.at);
        let high = Vec3::new(mark.x, mark.y, sky_height(w));
        let reach = w.reach.max(60.0);
        let r = reach * 1.15;
        let level = f32::from(w.level);
        let lit = if w.jumping { 1.0 } else { near };
        out.push(
            Piece::ring(mark, Vec3::Z)
                .size(reach, reach)
                .power(0.6 + 1.4 * lit)
                .flags(DRAPE | TARGET)
                .now(time),
        );
        // One ring lying over the mark, one standing across the fleet's way in.
        let way_in = (mark - Vec3::from(w.from))
            .truncate()
            .normalize_or(Vec2::X)
            .extend(0.0);
        for k in 0..2 {
            let way = if k == 0 { 1.0 } else { -1.0 };
            let rk = r * (1.0 - 0.18 * k as f32);
            let normal = if k == 0 { Vec3::Z } else { way_in };
            out.push(
                Piece::ring(high, normal)
                    .size(rk, rk)
                    .width(r * 0.08 + 4.0)
                    .power((1.5 + 3.5 * lit) * (1.0 + level * 0.02))
                    .flags(ARCS)
                    .tier(5.0)
                    .seed(time * 0.03 * way + k as f32 * 0.5)
                    .now(time),
            );
        }
        if lit > 0.5 {
            // Lines of light standing up out of the mark into the gate.
            let k = (lit - 0.5) * 2.0;
            for i in 0..6 {
                let a = i as f32 / 6.0 * TAU + time * 0.05;
                let foot = mark + Vec3::new(a.cos(), a.sin(), 0.0) * r * 0.9;
                let foot = foot.truncate().extend(self.progenitor_ground(foot));
                let top = Vec3::new(foot.x, foot.y, high.z);
                out.push(
                    Piece::beam(foot, foot.lerp(top, k))
                        .size(1.5, 0.8)
                        .power(0.6 + 1.2 * k)
                        .flags(LINE)
                        .now(time),
                );
            }
        }
    }

    /// Warp-in `w` was marked: a ring of arcs kicked out round its Shaper's crown and
    /// lines of light thrown up toward the mark.
    fn warp_in_marked(&mut self, w: &WarpInInstance, time: f32) {
        let (from, to) = (Vec3::from(w.from), Vec3::from(w.at));
        let level = f32::from(w.level);
        let seed = self.scatter.unit();
        let big = 1.0 + level * 0.04;
        self.lay_progenitor(
            Piece::ring(from, Vec3::Z)
                .size(8.0 * big, 55.0 * big)
                .width(4.0 * big)
                .time(time, 0.7, 0.5)
                .power(2.2)
                .flags(ARCS)
                .tier(3.0 + level / 4.0)
                .seed(seed),
        );
        let toward = (to - from).truncate().normalize_or(Vec2::X);
        for k in 0..6 {
            let a = k as f32 / 6.0 * TAU + seed * TAU;
            let out = (toward * 0.6 + Vec2::new(a.cos(), a.sin()) * 0.4).extend(1.0);
            self.lay_progenitor(
                Piece::beam(from, from + out.normalize() * 60.0 * big)
                    .size(1.6 * big, 0.4)
                    .time(time, 0.5, 0.3)
                    .power(1.8)
                    .flags(LINE),
            );
        }
        self.progenitor_fx
            .flash(from, HOT * 1.5e5 * big, 220.0 * big, time, 0.5);
    }

    /// A warp event the Progenitor draws: true when it draws it alone (a squad's
    /// arrival or cancel); a fleet ship's jump is drawn by `warp_fx` too, the gates laid
    /// over it.
    pub(super) fn warp_in_event(&mut self, event: &SimEvent, time: f32) -> bool {
        let ours = |owner: u8| Some(u32::from(owner)) == self.progenitor_fx.warp.owner;
        match *event {
            SimEvent::WarpInArrived {
                serial, at, radius, ..
            } => {
                self.squad_arrived(serial, Vec3::from(at.to_f32()), radius.to_f32(), time);
                true
            }
            SimEvent::WarpInCancelled { serial, at, .. } => {
                self.squad_cancelled(serial, Vec3::from(at.to_f32()), time);
                true
            }
            SimEvent::WarpJumped {
                from,
                to,
                owner,
                blueprint,
                ..
            } if ours(owner) => {
                let r = self.blueprints.unit(blueprint).radius.to_f32();
                let (from, to) = (Vec3::from(from.to_f32()), Vec3::from(to.to_f32()));
                self.fleet_gate(from, to, r, false, time);
                false
            }
            SimEvent::WarpArrived {
                at,
                owner,
                blueprint,
                dampened,
                ..
            } if ours(owner) => {
                let r = self.blueprints.unit(blueprint).radius.to_f32();
                let at = Vec3::from(at.to_f32());
                // Coming in from its Gatewright's side (the nearest standing).
                let from = self
                    .progenitor_fx
                    .shapers
                    .iter()
                    .filter(|(s, _)| s.kind == GATEWRIGHT)
                    .map(|(_, foot)| *foot)
                    .min_by(|a, b| a.distance_squared(at).total_cmp(&b.distance_squared(at)))
                    .unwrap_or(at - Vec3::X * 1000.0);
                // Thrown out of a dampened jump, its gate opens weakly.
                let r = if dampened { r * 0.6 } else { r };
                self.fleet_gate(from, at, r, true, time);
                false
            }
            _ => false,
        }
    }

    /// A Progenitor ship going into warp at `from` for `to` (`arriving` false: a ring
    /// closes behind it) or coming out at `to` from `from`'s way (a ring opens round it
    /// and lines of light streak in after it); `r` the ship's radius.
    fn fleet_gate(&mut self, from: Vec3, to: Vec3, r: f32, arriving: bool, time: f32) {
        let dir = (to - from).normalize_or(Vec3::X);
        let at = if arriving { to } else { from };
        let seed = self.scatter.unit();
        let r = r.max(8.0);
        let ring = Piece::ring(at, dir)
            .width(r * 0.15 + 1.0)
            .flags(ARCS)
            .tier(4.0)
            .seed(seed);
        let bright = (r / 20.0).max(1.0);
        if !arriving {
            self.lay_progenitor(
                ring.size(r * 3.5, r * 0.3)
                    .time(time, 0.9, 0.9)
                    .power(2.2)
                    .flags(CLOSE),
            );
            self.progenitor_fx
                .flash(at, LIGHT * 8.0e4 * bright, r * 6.0 + 80.0, time, 0.5);
            return;
        }
        self.lay_progenitor(ring.size(r * 0.6, r * 4.0).time(time, 1.2, 0.35).power(2.4));
        self.lay_progenitor(
            Piece::ring(at, dir)
                .size(r * 2.0, r * 2.6)
                .width(r * 0.06 + 0.5)
                .time(time + 0.05, 1.6, 0.6)
                .power(1.4)
                .seed(seed + 0.5),
        );
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let lift = side.cross(dir);
        for _ in 0..6 {
            let a = self.scatter.unit() * TAU;
            let off = (side * a.cos() + lift * a.sin()) * r * (0.6 + 1.6 * self.scatter.unit());
            let len = (r * (2.0 + 3.0 * self.scatter.unit())).min(160.0);
            self.lay_progenitor(
                Piece::beam(at - dir * len + off, at + off * 0.3)
                    .size(0.4, 1.4 + r * 0.04)
                    .time(time, 0.7, 0.2)
                    .power(1.8)
                    .flags(LINE),
            );
        }
        self.progenitor_fx
            .flash(at, HOT * 6.0e4 * bright, r * 5.0 + 80.0, time, 0.5);
    }

    /// Squad `serial` warped in at `at`, its shock `radius` metres: a tear of light at
    /// each spot, the gate's blades flung down, the shock ring and a burnt mark.
    fn squad_arrived(&mut self, serial: u32, at: Vec3, radius: f32, time: f32) {
        let w = self.progenitor_fx.warp.find(serial).cloned();
        let level = w.as_ref().map_or(0.0, |w| f32::from(w.level));
        let big = 1.0 + level * 0.04;
        let r = radius.max(10.0);
        let ground = Vec3::new(at.x, at.y, self.progenitor_ground(at));
        let seed = self.scatter.unit();
        let tier = 3.0 + (level / 4.0).min(3.0);
        // A tear of light at each unit's spot: a slit standing open and closing as the
        // unit unfolds out of it, a ring flashing round its foot.
        let spots: Vec<Vec3> = w.as_ref().map_or_else(
            || vec![ground],
            |w| w.spots.iter().map(|s| Vec3::from(*s)).collect(),
        );
        let tall = 14.0 + level * 0.6;
        for spot in &spots {
            let foot = spot.truncate().extend(self.progenitor_ground(*spot));
            let mid = foot + Vec3::Z * tall * 0.5;
            let wide = tall * 0.32;
            let start = time + self.scatter.unit() * 0.15;
            for (a, b, w0, w1) in [
                (foot, mid, 0.2, wide),
                (mid, foot + Vec3::Z * tall, wide, 0.2),
            ] {
                self.lay_progenitor(
                    Piece::beam(a, b)
                        .size(w0, w1)
                        .time(start, 1.0, 0.18)
                        .power(2.4),
                );
            }
            self.lay_progenitor(
                Piece::ring(foot, Vec3::Z)
                    .size(2.0, tall * 0.8)
                    .width(1.5)
                    .time(start, 0.8, 0.5)
                    .power(1.8)
                    .flags(DRAPE | ARCS)
                    .tier(tier)
                    .seed(seed),
            );
        }
        // The gate's blades flung down: lines thrown off their heads.
        if let Some(w) = &w {
            let (n, gr, gt) = gate(w);
            let turn = (w.serial % 17) as f32 * 0.37;
            for k in 0..n {
                let (foot, outward) = blade_foot(Vec3::from(w.at), k, n, gr, turn);
                let head = foot.truncate().extend(self.progenitor_ground(foot) + gt);
                self.lay_progenitor(
                    Piece::beam(head, head + outward * gt * 0.8 - Vec3::Z * gt * 0.7)
                        .size(1.5 + gt * 0.03, 0.3)
                        .time(time, 0.6, 0.3)
                        .power(1.8)
                        .flags(LINE),
                );
            }
        }
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r * 0.2, r)
                .width(r * 0.2)
                .time(time, 0.8, 0.5)
                .power(2.6 * big)
                .flags(DRAPE | ARCS)
                .tier(tier)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r, r * 1.8)
                .width(2.5)
                .time(time, 1.0, 0.9)
                .power(1.2 * big)
                .flags(DRAPE),
        );
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r, r)
                .time(time, 25.0, 0.0)
                .power(1.6 * big)
                .flags(DRAPE | MARK)
                .tier(tier)
                .seed(seed),
        );
        self.progenitor_fx.flash(
            ground + Vec3::Z * (8.0 + r * 0.2),
            LIGHT * 6.0e4 * big,
            r * 5.0 + 80.0,
            time,
            0.9,
        );
    }

    /// Squad `serial`'s mark at `at` was lost to a dampening field: its gate shatters.
    fn squad_cancelled(&mut self, serial: u32, at: Vec3, time: f32) {
        let Some(w) = self.progenitor_fx.warp.find(serial).cloned() else {
            return;
        };
        let (n, r, tall) = gate(&w);
        let whole = (w.age + w.eta).max(0.1);
        let up = (1.0 - w.eta / whole).clamp(0.0, 1.0);
        let turn = (w.serial % 17) as f32 * 0.37;
        let seed = self.scatter.unit();
        for k in 0..n {
            let (foot, _) = blade_foot(Vec3::from(w.at), k, n, r, turn);
            let foot = foot.truncate().extend(self.progenitor_ground(foot));
            let head = foot + Vec3::Z * tall * up.max(0.2);
            for _ in 0..4 {
                let dir = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.unit() * 0.8,
                )
                .normalize_or(Vec3::Z);
                let len = tall * (0.4 + 0.8 * self.scatter.unit());
                let along = self.scatter.unit();
                let from = foot.lerp(head, along);
                self.lay_progenitor(
                    Piece::beam(from, from + dir * len)
                        .size(1.2, 0.3)
                        .time(time + along * 0.1, 0.9, 0.3)
                        .power(1.8)
                        .flags(LINE),
                );
            }
        }
        let ground = at.truncate().extend(self.progenitor_ground(at));
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(w.radius, w.radius * 1.6)
                .width(3.0)
                .time(time, 0.9, 0.8)
                .power(2.0)
                .flags(DRAPE | ARCS)
                .tier(6.0)
                .seed(seed),
        );
        self.progenitor_fx.flash(
            ground + Vec3::Z * 20.0,
            LIGHT * 5.0e4,
            r * 3.0 + 60.0,
            time,
            0.6,
        );
    }
}

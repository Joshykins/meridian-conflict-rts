//! Strategic missiles: the nuclear silo, the interceptor array, and the blasts
//! (`docs/NUKES.md`).
//!
//! A launcher (`UnitBlueprint::strategic`) assembles its rounds one at a time out of its
//! side's income, paid like a build, and keeps up to its stock. A silo launches only when
//! ordered (`Command::LaunchNuke`, one warhead a mark; a silo fires the marks it is given
//! in turn): its blast doors open, the missile climbs straight out of the tube on its
//! boost, pitches over gradually onto a high arc and comes down steep on the mark anywhere
//! on the map, bursting a little above the ground (`WarheadPath`, the one path both the
//! sim and the interface use). An interceptor array fires by itself at any enemy warhead
//! coming down inside its coverage: one interceptor per warhead, which climbs hard, meets
//! the warhead along its path and bursts beside it.
//!
//! A detonation is not instant. Its front runs out over about a second (`NuclearBlast::
//! front_at`), and whatever it reaches that tick is hit then: the middle at once, the
//! edge a moment later. It hurts everything, its owner's too; a dome in the way takes it
//! instead. It kills every tree out past its damage radius (the renderer flattens the
//! near ones and sets the rest alight); the crater is the renderer's. A commander's
//! reactor going up is the same thing, smaller (`COMMANDER_BLAST`).
//!
//! Missiles here are not projectiles: nothing else can hit them, they ignore domes and
//! fog, and every side sees them (a launch is announced to everyone).

use std::collections::BTreeMap;

use mc_core::{Fx, FxVec2, FxVec3, StateHasher};
use mc_data::strategic::{NuclearBlast, StrategicKind};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::{flag, UnitId};
use crate::{SimError, World};

/// Ticks a silo's blast doors take to open (and to close).
pub const SILO_DOOR_TICKS: u16 = 40;
/// Ticks an interceptor array's doors take to open.
pub const ARRAY_DOOR_TICKS: u16 = 8;
/// Ticks after ignition before a silo's doors start to close behind the missile.
const SILO_CLEAR_TICKS: u16 = 60;
/// Ticks between two warheads out of one silo while its doors stay open for the next.
pub const SILO_NEXT_TICKS: u16 = 25;
/// Ticks an array keeps its doors open with nothing to shoot at.
const ARRAY_LINGER_TICKS: u16 = 30;
/// Ticks between two interceptors out of one array.
const ARRAY_GAP_TICKS: u16 = 6;

/// How deep in the tube a silo's missile starts, metres under the ground.
pub const TUBE_DEPTH: Fx = Fx::from_int(22);
/// Ticks of the warhead's boost: out of the tube slowly, picking up speed until it
/// reaches its cruise as the boost ends.
pub const BOOST_TICKS: u32 = 70;
/// A warhead's pace at ignition, as a share of its cruise.
const IGNITION_PACE: Fx = Fx::ratio(1, 25);
/// Metres a warhead climbs dead straight up from the bottom of its tube before it starts
/// to pitch over (about five seconds of the boost).
const STRAIGHT_CLIMB: Fx = Fx::from_int(380);
/// How far back from the mark, as a share of the span, the dive's control point stands:
/// the warhead comes down steep, not plumb.
const DIVE_LEAN: Fx = Fx::ratio(3, 20);
/// Metres short of the burst over which a warhead picks up its dive speed, and how much
/// faster than its cruise it comes in.
const DIVE_REACH: Fx = Fx::from_int(2500);
const DIVE_GAIN: Fx = Fx::ratio(3, 5);
/// Samples along the curve, to walk it by distance.
const PATH_SAMPLES: usize = 96;
/// Metres above the ground a warhead bursts.
pub const BURST_HEIGHT: Fx = Fx::from_int(55);
/// The lowest an arc tops out, metres over its ends, and its share of the span.
const ARC_FLOOR: Fx = Fx::from_int(900);
const ARC_SHARE: Fx = Fx::ratio(9, 20);

/// Ticks of an interceptor's boost straight up out of its cell.
const INTERCEPT_BOOST: u32 = 12;
/// Metres from the warhead at which an interceptor bursts and kills it.
const KILL_REACH: Fx = Fx::from_int(55);
/// Ticks ahead an interceptor looks along its warhead's path for where to meet it.
const LEAD_TICKS: i32 = 120;
/// Ticks an interceptor flies before it burns out.
const INTERCEPT_LIFE: u32 = 450;
/// Where an array's four cells stand on its deck (x, y), and how high they launch from.
const CELLS: [(i32, i32); 4] = [(-24, -24), (-24, 24), (24, -24), (24, 24)];
const CELL_TOP: Fx = Fx::from_int(3);

/// A commander's reactor going up: a nuclear blast, smaller than a warhead's.
pub const COMMANDER_BLAST: NuclearBlast = NuclearBlast {
    radius: Fx::from_int(300),
    core: Fx::from_int(90),
    damage: Fx::from_int(7000),
    edge: Fx::from_int(400),
    front_ticks: 8,
};

/// Trees die out to this share of a blast's damage radius (the thermal pulse carries
/// further than the pressure that breaks armour).
const TREE_REACH: Fx = Fx::ratio(13, 10);

/// `UnitInstance::_pad3[2]` on a launcher: rounds in stock (bits 0..8), how far the next
/// is assembled (8..16, 0..=255), the most it holds (16..24), and `LAUNCHER_FIRING` while
/// a silo has marks to fire at. Never set on anything else.
pub const LAUNCHER_STOCK_MASK: u32 = 0xFF;
pub const LAUNCHER_PROGRESS_SHIFT: u32 = 8;
pub const LAUNCHER_CAPACITY_SHIFT: u32 = 16;
pub const LAUNCHER_FIRING: u32 = 1 << 24;
/// Set on every launcher, so a zero stock still reads as a launcher.
pub const LAUNCHER_MARK: u32 = 1 << 25;
/// Auto-build is off (`Launcher::manual`).
pub const LAUNCHER_MANUAL: u32 = 1 << 26;
/// Bits 27..32: rounds queued by hand (`Launcher::queued`, at most 31 shown).
pub const LAUNCHER_QUEUED_SHIFT: u32 = 27;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Launcher {
    pub stock: u8,
    /// Build-time units into the next round.
    pub progress: Fx,
    /// A silo's launch under way (its doors opening for the first of `targets`): where it
    /// goes, and ticks since the doors began to open for it.
    pub launch: Option<(FxVec2, u16)>,
    /// Ticks since this launcher last fired; for an array, since it last had work.
    pub idle: u16,
    /// A silo's marks ordered and not yet away, in turn, each with its number among the
    /// orders given (`Strategic::orders`). Never more than its stock: each is a warhead
    /// spoken for. The first is the one `launch` opens the doors for.
    #[serde(default)]
    pub targets: Vec<(FxVec2, u32)>,
    /// Auto-build is off: it assembles only the rounds `queued` asks for. Off by default,
    /// so a launcher starts on its first round the moment it is finished.
    #[serde(default)]
    pub manual: bool,
    /// Rounds ordered by hand while auto-build is off.
    #[serde(default)]
    pub queued: u8,
}

impl Launcher {
    /// Rounds in stock that no launch has spoken for yet.
    pub fn free(&self) -> u8 {
        self.stock.saturating_sub(self.targets.len().min(255) as u8)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum MissileKind {
    Warhead,
    Interceptor,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StrategicMissile {
    /// Never zero, never reused in a match.
    pub serial: u32,
    pub kind: MissileKind,
    pub owner: u8,
    pub source: UnitId,
    pub blueprint: BlueprintId,
    pub pos: FxVec3,
    pub prev_pos: FxVec3,
    /// Metres a tick.
    pub vel: FxVec3,
    /// Ticks since launch.
    pub age: u32,
    /// Warhead: where it bursts, the bottom of the tube it left, and metres flown along
    /// its path (`WarheadPath`). Interceptor: the warhead it hunts.
    pub mark: FxVec3,
    #[serde(default)]
    pub origin: FxVec3,
    #[serde(default)]
    pub travelled: Fx,
    pub quarry: u32,
}

/// A detonation whose front is still running out.
#[derive(Clone, Serialize, Deserialize)]
pub struct Detonation {
    pub pos: FxVec3,
    pub owner: u8,
    pub source: UnitId,
    pub radius: Fx,
    pub core: Fx,
    pub damage: Fx,
    pub edge: Fx,
    pub front_ticks: u32,
    pub age: u32,
    /// Metres the front had reached last tick.
    pub reached: Fx,
    /// Units already hit, so one that moves out ahead of the front is not hit twice.
    pub hit: Vec<UnitId>,
    /// Domes that took this blast (middle, radius, what the dome held when it did): what
    /// stands under one takes only what got past it, even once the dome is down.
    #[serde(default)]
    pub screens: Vec<(FxVec3, Fx, Fx)>,
}

impl Detonation {
    fn blast(&self) -> NuclearBlast {
        NuclearBlast {
            radius: self.radius,
            core: self.core,
            damage: self.damage,
            edge: self.edge,
            front_ticks: self.front_ticks,
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Strategic {
    pub launchers: BTreeMap<UnitId, Launcher>,
    pub missiles: Vec<StrategicMissile>,
    pub detonations: Vec<Detonation>,
    pub serial: u32,
    /// Launches ordered so far, every side's: the number the next one takes.
    #[serde(default)]
    pub orders: u32,
}

impl Strategic {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.launchers.len() as u64 | (self.serial as u64) << 32);
        for (id, l) in &self.launchers {
            h.write_u64(id.0 as u64 | (l.stock as u64) << 32 | (l.idle as u64) << 40 | (l.queued as u64) << 56);
            h.write_u64(l.manual as u64);
            h.write_i64(l.progress.0);
            if let Some((at, t)) = l.launch {
                h.write_i64(at.x.0);
                h.write_i64(at.y.0);
                h.write_u64(t as u64);
            }
            h.write_u64(l.targets.len() as u64);
            for &(at, order) in &l.targets {
                h.write_i64(at.x.0);
                h.write_i64(at.y.0);
                h.write_u64(order as u64);
            }
        }
        h.write_u64(self.orders as u64);
        h.write_u64(self.missiles.len() as u64);
        for m in &self.missiles {
            h.write_u64(m.serial as u64 | (m.owner as u64) << 32 | (m.kind as u64) << 40);
            h.write_u64(m.age as u64 | (m.quarry as u64) << 32);
            for v in [m.pos, m.vel, m.mark, m.origin] {
                h.write_i64(v.x.0);
                h.write_i64(v.y.0);
                h.write_i64(v.z.0);
            }
            h.write_i64(m.travelled.0);
        }
        h.write_u64(self.detonations.len() as u64);
        for d in &self.detonations {
            h.write_i64(d.pos.x.0);
            h.write_i64(d.pos.y.0);
            h.write_u64(d.age as u64 | (d.hit.len() as u64) << 32);
            h.write_i64(d.reached.0);
        }
    }
}

/// The one path a warhead flies, from the bottom of its tube to its burst point, walked
/// by distance. The sim flies it and the interface draws it (the aim preview, a launch
/// waiting its turn, a warhead in flight), so what is drawn is what is flown.
///
/// It leaves the tube straight up and climbs dead straight for `STRAIGHT_CLIMB`, then
/// follows a cubic curve whose first control point stands straight over the top of that
/// climb (so it goes on heading up there and pitches over gradually, like a gravity
/// turn) and whose last stands high over the mark, a little back toward the silo (so it
/// comes down steep). The curve's middle tops out `rise` over its ends: 45% of the span,
/// no less than 900 m, no more than the silo's `apogee`. Position and heading are
/// continuous all the way; the pace along it comes from `pace`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarheadPath {
    /// The bottom of the tube.
    pub start: FxVec3,
    /// Where it bursts: `BURST_HEIGHT` over the mark.
    pub mark: FxVec3,
    /// The curve after the straight climb.
    curve: [FxVec3; 4],
    /// Metres along the curve at each of its samples.
    lengths: [Fx; PATH_SAMPLES + 1],
}

impl WarheadPath {
    /// The path from `start` (the bottom of the tube, `warhead_start`) to `mark` (the
    /// burst point, `burst_point`) for a silo that tops out at no more than `apogee`.
    pub fn new(start: FxVec3, mark: FxVec3, apogee: Fx) -> WarheadPath {
        let climb = start + FxVec3::new(Fx::ZERO, Fx::ZERO, STRAIGHT_CLIMB);
        let span = climb.xy().distance(mark.xy());
        let rise = (span * ARC_SHARE).max(ARC_FLOOR).min(apogee.max(ARC_FLOOR));
        // A cubic's middle stands three quarters of the way up to its control points.
        let reach = rise * Fx::ratio(4, 3);
        let back = (climb.xy() - mark.xy()) * DIVE_LEAN;
        let curve = [
            climb,
            climb + FxVec3::new(Fx::ZERO, Fx::ZERO, reach),
            (mark.xy() + back).extend(mark.z + reach),
            mark,
        ];
        let mut lengths = [Fx::ZERO; PATH_SAMPLES + 1];
        let mut last = climb;
        for (i, l) in lengths.iter_mut().enumerate().skip(1) {
            let p = cubic(&curve, (i as i128) << 32, PATH_SAMPLES as i128);
            *l = (p - last).length();
            last = p;
        }
        for i in 1..=PATH_SAMPLES {
            lengths[i] += lengths[i - 1];
        }
        WarheadPath { start, mark, curve, lengths }
    }

    /// Metres from the tube to the burst.
    pub fn length(&self) -> Fx {
        STRAIGHT_CLIMB + self.lengths[PATH_SAMPLES]
    }

    /// Where the warhead is `s` metres along.
    pub fn at(&self, s: Fx) -> FxVec3 {
        if s <= STRAIGHT_CLIMB {
            return self.start + FxVec3::new(Fx::ZERO, Fx::ZERO, s.max(Fx::ZERO));
        }
        let u = (s - STRAIGHT_CLIMB).min(self.lengths[PATH_SAMPLES]);
        let i = match self.lengths.binary_search(&u) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
        .min(PATH_SAMPLES - 1);
        let seg = self.lengths[i + 1] - self.lengths[i];
        // How far through this sample, finely: the curve is long and a coarse `t` jitters.
        let f = if seg > Fx::ZERO { (((u - self.lengths[i]).0 as i128) << 32) / seg.0 as i128 } else { 0 };
        cubic(&self.curve, ((i as i128) << 32) + f, PATH_SAMPLES as i128)
    }

    /// `at` for the interface, from metres as a float.
    pub fn at_f32(&self, s: f32) -> [f32; 3] {
        self.at(Fx::from_f32(s)).to_f32()
    }

    /// `count + 1` points evenly spaced along what is left of the path past `from`
    /// metres, ending on the burst: for drawing it.
    pub fn trace(&self, from: Fx, count: usize) -> Vec<[f32; 3]> {
        let (from, len) = (from.max(Fx::ZERO), self.length());
        let count = count.max(1);
        (0..=count)
            .map(|i| self.at(from + (len - from) * i as i32 / count as i32).to_f32())
            .collect()
    }

    /// Metres the warhead covers this tick, `age` ticks after ignition with `travelled`
    /// behind it, cruising at `cruise` metres a tick: slow out of the tube and faster
    /// through the boost until it reaches its cruise, then faster again in the dive.
    pub fn pace(&self, cruise: Fx, age: u32, travelled: Fx) -> Fx {
        let boost = if age < BOOST_TICKS {
            let f = Fx::from_int(age as i32) / BOOST_TICKS as i32;
            IGNITION_PACE + (Fx::ONE - IGNITION_PACE) * f * f
        } else {
            Fx::ONE
        };
        let left = self.length() - travelled;
        let d = ((DIVE_REACH - left) / DIVE_REACH).clamp(Fx::ZERO, Fx::ONE);
        let dive = d * d * (Fx::from_int(3) - d * 2);
        cruise * boost * (Fx::ONE + DIVE_GAIN * dive)
    }

    /// One tick on from (`age`, `travelled`): the new age and metres flown.
    pub fn step(&self, cruise: Fx, age: u32, travelled: Fx) -> (u32, Fx) {
        let age = age + 1;
        (age, (travelled + self.pace(cruise, age, travelled)).min(self.length()))
    }

    /// Ticks until the burst from (`age`, `travelled`), exactly as the sim flies it.
    pub fn ticks_left(&self, cruise: Fx, age: u32, travelled: Fx) -> u32 {
        let (mut age, mut s, len) = (age, travelled, self.length());
        let mut n = 0;
        while s < len && n < 100_000 {
            (age, s) = self.step(cruise.max(Fx::ONE), age, s);
            n += 1;
        }
        n
    }
}

/// The curve through `p` at `t = num / den`, `num` in 32-bit fixed point: evaluated in
/// wide integers, so a long curve walked a few metres a tick does not jitter.
fn cubic(p: &[FxVec3; 4], num: i128, den: i128) -> FxVec3 {
    const ONE: i128 = 1 << 32;
    let t = (num / den).clamp(0, ONE);
    let s = ONE - t;
    // The four weights, each in 32-bit fixed point.
    let w = [
        (s * s >> 32) * s >> 32,
        3 * ((s * s >> 32) * t >> 32),
        3 * ((s * t >> 32) * t >> 32),
        (t * t >> 32) * t >> 32,
    ];
    let axis = |f: fn(&FxVec3) -> Fx| {
        Fx(((0..4).map(|k| f(&p[k]).0 as i128 * w[k]).sum::<i128>() >> 32) as i64)
    };
    FxVec3::new(axis(|v| v.x), axis(|v| v.y), axis(|v| v.z))
}

/// The bottom of a silo's tube, standing on `ground`: where its warhead's path starts.
pub fn warhead_start(ground: FxVec3) -> FxVec3 {
    ground - FxVec3::new(Fx::ZERO, Fx::ZERO, TUBE_DEPTH)
}

/// Where a warhead aimed at `surface` (the ground there, or the sea over it) bursts.
pub fn burst_point(surface: FxVec3) -> FxVec3 {
    surface + FxVec3::new(Fx::ZERO, Fx::ZERO, BURST_HEIGHT)
}

/// A warhead in flight, for the interface: the path it flies and how far along it is.
#[derive(Clone, Copy, Debug)]
pub struct WarheadTrack {
    pub serial: u32,
    pub owner: u8,
    pub path: WarheadPath,
    pub travelled: Fx,
}

/// A launch ordered and not yet away, for its own side and observers only.
#[derive(Clone, Copy, Debug)]
pub struct PlannedLaunch {
    /// The silo that will fire it.
    pub silo: u32,
    pub owner: u8,
    /// Its number among the launches ordered (`Strategic::orders`): lower goes first.
    pub order: u32,
    /// The path the warhead will fly.
    pub path: WarheadPath,
    /// The silo's doors are opening for it now.
    pub opening: bool,
    /// Seconds until it bursts, if nothing stops it.
    pub eta: f32,
}

/// Closest two things come over a tick, each moving in a straight line from `a0` to
/// `a1` and `b0` to `b1`: the distance and where the first was then.
fn closest_over_tick(a0: FxVec3, a1: FxVec3, b0: FxVec3, b1: FxVec3) -> (Fx, FxVec3) {
    let r0 = a0 - b0;
    let dr = (a1 - b1) - r0;
    let dd = dr.dot(dr);
    let t = if dd > Fx::EPSILON {
        (-(r0.dot(dr)) / dd).clamp(Fx::ZERO, Fx::ONE)
    } else {
        Fx::ZERO
    };
    ((r0 + dr * t).length(), a0 + (a1 - a0) * t)
}

impl World {
    fn launcher_spec(&self, row: usize) -> Option<&mc_data::strategic::Strategic> {
        self.bp(row).strategic.as_ref()
    }

    /// `Command::SetAutoBuild`: `ids`' own launchers build rounds by themselves (`on`), or
    /// only those queued by hand. Turning it off keeps the round in hand as one queued.
    pub(crate) fn set_auto_build(&mut self, player: u8, ids: &[UnitId], on: bool) {
        for row in self.owned(player, ids, 0) {
            if self.launcher_spec(row).is_none() {
                continue;
            }
            let id = self.state.units.id(row);
            let l = self.state.strategic.launchers.entry(id).or_default();
            if l.manual == !on {
                continue;
            }
            l.manual = !on;
            l.queued = if !on && l.progress > Fx::ZERO { 1 } else { 0 };
        }
    }

    /// `Command::QueueRounds`: `count` more rounds (fewer, when negative) queued by hand on
    /// `ids`' own launchers; queuing turns auto-build off.
    pub(crate) fn queue_rounds(&mut self, player: u8, ids: &[UnitId], count: i16) {
        for row in self.owned(player, ids, 0) {
            let Some(cap) = self.launcher_spec(row).map(|s| s.stock) else {
                continue;
            };
            let id = self.state.units.id(row);
            let l = self.state.strategic.launchers.entry(id).or_default();
            if !l.manual {
                l.manual = true;
                l.queued = 0;
            }
            let room = cap.saturating_sub(l.stock) as i16;
            // Cancelling a round gives nothing back: what was spent stays in the next one.
            l.queued = (l.queued as i16 + count).clamp(0, room.max(0)) as u8;
        }
    }

    /// `Command::LaunchNuke`: one warhead among `ids`' own silos is given `pos`. It comes
    /// from the silo with the most warheads not yet spoken for (ties: the nearest to the
    /// mark, then the lowest id), which fires its marks in turn. With none free, nothing.
    pub(crate) fn launch_nuke(&mut self, player: u8, ids: &[UnitId], pos: FxVec2) {
        let pos = self.clamp_to_map(pos);
        let mut best: Option<(std::cmp::Reverse<u8>, Fx, UnitId)> = None;
        for row in self.owned(player, ids, 0) {
            let is_silo = self.launcher_spec(row).is_some_and(|s| s.kind == StrategicKind::Nuke);
            if !is_silo {
                continue;
            }
            let id = self.state.units.id(row);
            let free = self.state.strategic.launchers.get(&id).map_or(0, |l| l.free());
            if free == 0 {
                continue;
            }
            let key = (std::cmp::Reverse(free), self.state.units.pos[row].distance_sq(pos), id);
            if best.is_none_or(|b| key < b) {
                best = Some(key);
            }
        }
        let Some((_, _, id)) = best else {
            return;
        };
        let order = self.state.strategic.orders;
        self.state.strategic.orders += 1;
        let l = self.state.strategic.launchers.entry(id).or_default();
        l.targets.push((pos, order));
    }

    /// Every tick, before the economy: launchers that came or went, doors, launches,
    /// flights, interceptions and blast fronts.
    pub(crate) fn run_strategic(&mut self) -> Result<(), SimError> {
        // Launchers that are gone take their state with them.
        let units = &self.state.units;
        self.state
            .strategic
            .launchers
            .retain(|id, _| units.row(*id).is_some());
        self.run_silos();
        self.run_arrays();
        self.fly_warheads()?;
        self.fly_interceptors();
        self.run_detonations()?;
        Ok(())
    }

    fn run_silos(&mut self) {
        let rows: Vec<usize> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&row| {
                self.state.units.is_active(row)
                    && self.launcher_spec(row).is_some_and(|s| s.kind == StrategicKind::Nuke)
            })
            .collect();
        for row in rows {
            let id = self.state.units.id(row);
            let owner = self.state.units.owner[row];
            let bp = self.state.units.blueprint[row];
            let base = self.state.units.pos[row];
            let ground = self.terrain.height_at(base);
            let target = |at: FxVec2| burst_point(at.extend(self.terrain.height_at(at).max(self.terrain.water_level())));
            let l = self.state.strategic.launchers.entry(id).or_default();
            let units = &mut self.state.units;
            l.idle = l.idle.saturating_add(1);
            // Every mark is a warhead spoken for.
            l.targets.truncate(l.stock as usize);
            // The next mark in turn: the doors open for it, or, still open behind the last
            // warhead, it goes after a short gap.
            if l.launch.is_none() {
                if let Some(&(at, _)) = l.targets.first() {
                    let doors = units.deploy[row];
                    if doors < SILO_DOOR_TICKS || l.idle >= SILO_NEXT_TICKS {
                        l.launch = Some((at, 0));
                        if doors < SILO_DOOR_TICKS {
                            self.events.push(SimEvent::SiloOpening {
                                pos: base.extend(ground),
                                owner,
                            });
                        }
                    }
                }
            }
            let Some((mark, t)) = l.launch else {
                // Nothing under way: the doors close once the last missile is clear and
                // no mark is waiting.
                if l.targets.is_empty() && l.idle > SILO_CLEAR_TICKS {
                    units.deploy[row] = units.deploy[row].saturating_sub(1);
                }
                continue;
            };
            l.launch = Some((mark, t + 1));
            units.deploy[row] = (units.deploy[row] + 1).min(SILO_DOOR_TICKS);
            if units.deploy[row] < SILO_DOOR_TICKS {
                continue;
            }
            // Doors open: light it.
            l.launch = None;
            if !l.targets.is_empty() {
                l.targets.remove(0);
            }
            if l.stock == 0 {
                continue;
            }
            l.stock -= 1;
            l.idle = 0;
            self.state.strategic.serial += 1;
            let serial = self.state.strategic.serial;
            let start = warhead_start(base.extend(ground));
            let to = target(mark);
            self.state.strategic.missiles.push(StrategicMissile {
                serial,
                kind: MissileKind::Warhead,
                owner,
                source: id,
                blueprint: bp,
                pos: start,
                prev_pos: start,
                vel: FxVec3::ZERO,
                age: 0,
                mark: to,
                origin: start,
                travelled: Fx::ZERO,
                quarry: 0,
            });
            self.events.push(SimEvent::NuclearLaunch {
                from: base.extend(ground),
                to,
                owner,
                serial,
            });
        }
    }

    /// Whether the launcher in `row` is assembling a round now: finished, not paused, not
    /// full, and on auto-build or with rounds queued by hand. Engineers can help with it.
    pub(crate) fn launcher_wants_round(&self, row: usize) -> bool {
        let units = &self.state.units;
        let Some(spec) = self.launcher_spec(row) else {
            return false;
        };
        if !units.is_active(row) || self.work_paused(row) {
            return false;
        }
        let l = self.state.strategic.launchers.get(&units.id(row));
        let stock = l.map_or(0, |l| l.stock);
        let wanted = l.is_none_or(|l| !l.manual || l.queued > 0);
        stock < spec.stock && wanted
    }

    /// Paid out of `run_economy`: what every launcher still assembling a round wants this
    /// tick at full supply (mass, energy per tick), by row, with its build-time rate: its
    /// own power and that of every engineer assisting it.
    pub(crate) fn launcher_jobs(&mut self) -> Vec<(usize, Fx, [Fx; 2])> {
        let mut jobs = Vec::new();
        // Engineers on an assist whose work is a launcher's round (`run_assist`).
        let mut helpers: BTreeMap<UnitId, Fx> = BTreeMap::new();
        for row in self.state.units.slots.iter() {
            let units = &self.state.units;
            if units.flags[row] & flag::BUILDING == 0 || units.has_flag(row, flag::REPAIRING) {
                continue;
            }
            let Some(b) = self.bp(row).builder.as_ref() else {
                continue;
            };
            let target = units.build_target[row];
            if units.row(target).is_some_and(|t| self.launcher_spec(t).is_some()) {
                *helpers.entry(target).or_insert(Fx::ZERO) += b.power;
            }
        }
        for row in self.state.units.slots.iter() {
            if !self.launcher_wants_round(row) {
                continue;
            }
            let spec = self.launcher_spec(row).unwrap();
            let id = self.state.units.id(row);
            let l = self.state.strategic.launchers.get(&id);
            let done = l.map_or(Fx::ZERO, |l| l.progress);
            let power = spec.power + helpers.get(&id).copied().unwrap_or(Fx::ZERO);
            let rate = (power / mc_core::TICKS_PER_SECOND as i32).min(spec.round_time - done);
            if rate <= Fx::ZERO {
                continue;
            }
            let want = [
                spec.round_mass * rate / spec.round_time,
                spec.round_energy * rate / spec.round_time,
            ];
            jobs.push((row, rate, want));
        }
        jobs
    }

    /// The economy's answer to `launcher_jobs`: each job gets `efficiency` of its rate.
    pub(crate) fn advance_launchers(&mut self, row: usize, step: Fx) {
        let spec = self.launcher_spec(row).cloned().unwrap();
        let id = self.state.units.id(row);
        let l = self.state.strategic.launchers.entry(id).or_default();
        l.progress += step;
        if l.progress >= spec.round_time {
            l.progress = Fx::ZERO;
            l.stock = (l.stock + 1).min(spec.stock);
            if l.manual {
                l.queued = l.queued.saturating_sub(1);
            }
            let pos = self.state.units.pos[row];
            self.events.push(SimEvent::RoundReady {
                unit: id,
                pos: pos.extend(self.terrain.height_at(pos)),
                owner: self.state.units.owner[row],
                warhead: spec.kind == StrategicKind::Nuke,
            });
        }
    }

    fn run_arrays(&mut self) {
        let rows: Vec<usize> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&row| {
                self.state.units.is_active(row)
                    && self
                        .launcher_spec(row)
                        .is_some_and(|s| s.kind == StrategicKind::Interceptor)
            })
            .collect();
        for row in rows {
            let id = self.state.units.id(row);
            let owner = self.state.units.owner[row];
            let at = self.state.units.pos[row];
            let spec = self.launcher_spec(row).cloned().unwrap();
            // Warheads of an enemy bound for somewhere inside the coverage, already coming
            // down inside it (not merely aimed there from afar), and not yet hunted.
            let hunted: Vec<u32> = self
                .state
                .strategic
                .missiles
                .iter()
                .filter(|m| m.kind == MissileKind::Interceptor)
                .map(|m| m.quarry)
                .collect();
            let prey = self
                .state
                .strategic
                .missiles
                .iter()
                .filter(|m| {
                    m.kind == MissileKind::Warhead
                        && m.vel.z < Fx::ZERO
                        && self.are_enemies(owner, m.owner)
                        && m.mark.xy().distance(at) <= spec.coverage
                        && m.pos.xy().distance(at) <= spec.coverage
                        && !hunted.contains(&m.serial)
                })
                .min_by_key(|m| (m.mark.xy().distance(at), m.serial))
                .map(|m| m.serial);
            let grid_dark = self.state.players[owner as usize].upkeep_efficiency <= Fx::ZERO;
            let l = self.state.strategic.launchers.entry(id).or_default();
            let units = &mut self.state.units;
            let Some(serial) = prey.filter(|_| l.stock > 0 && !grid_dark) else {
                l.idle = l.idle.saturating_add(1);
                if l.idle > ARRAY_LINGER_TICKS {
                    units.deploy[row] = units.deploy[row].saturating_sub(1);
                }
                continue;
            };
            units.deploy[row] = (units.deploy[row] + 1).min(ARRAY_DOOR_TICKS);
            if units.deploy[row] < ARRAY_DOOR_TICKS || (l.idle < ARRAY_GAP_TICKS && l.launch.is_some()) {
                l.idle = l.idle.saturating_add(1);
                continue;
            }
            // The cell to fire from: the fullest one still holding a round.
            let cell = (spec.stock.saturating_sub(l.stock) as usize) % CELLS.len();
            l.stock -= 1;
            l.idle = 0;
            l.launch = Some((at, 0));
            let heading = units.heading[row];
            let (cx, cy) = CELLS[cell];
            let off = FxVec2::new(Fx::ratio(cx as i64, 10), Fx::ratio(cy as i64, 10)).rotate(heading);
            let ground = self.terrain.height_at(at);
            let start = (at + off).extend(ground + CELL_TOP);
            self.state.strategic.serial += 1;
            let own = self.state.strategic.serial;
            let bp = self.state.units.blueprint[row];
            self.state.strategic.missiles.push(StrategicMissile {
                serial: own,
                kind: MissileKind::Interceptor,
                owner,
                source: id,
                blueprint: bp,
                pos: start,
                prev_pos: start,
                vel: FxVec3::new(Fx::ZERO, Fx::ZERO, spec.speed / 6),
                age: 0,
                mark: start,
                origin: start,
                travelled: Fx::ZERO,
                quarry: serial,
            });
            self.events.push(SimEvent::InterceptorLaunch {
                from: start,
                owner,
                serial: own,
            });
        }
    }

    /// A warhead's path and its cruise, metres a tick.
    fn warhead_flight(&self, m: &StrategicMissile) -> (WarheadPath, Fx) {
        let spec = self.blueprints.unit(m.blueprint).strategic.as_ref();
        let cruise = spec.map_or(Fx::from_int(30), |s| s.speed);
        let apogee = spec.map_or(Fx::from_int(3000), |s| s.apogee);
        (WarheadPath::new(m.origin, m.mark, apogee), cruise)
    }

    fn fly_warheads(&mut self) -> Result<(), SimError> {
        let mut burst = Vec::new();
        for i in 0..self.state.strategic.missiles.len() {
            let m = &self.state.strategic.missiles[i];
            if m.kind != MissileKind::Warhead {
                continue;
            }
            let (path, cruise) = self.warhead_flight(m);
            let m = &mut self.state.strategic.missiles[i];
            m.prev_pos = m.pos;
            (m.age, m.travelled) = path.step(cruise, m.age, m.travelled);
            let next = path.at(m.travelled);
            m.vel = next - m.pos;
            m.pos = next;
            if m.travelled >= path.length() {
                burst.push(i);
            }
        }
        for &i in burst.iter().rev() {
            let m = self.state.strategic.missiles.swap_remove(i);
            let blast = self
                .blueprints
                .unit(m.blueprint)
                .strategic
                .as_ref()
                .and_then(|s| s.blast)
                .unwrap_or(COMMANDER_BLAST);
            self.detonate(m.mark, blast, m.owner, m.source, false)?;
        }
        Ok(())
    }

    fn fly_interceptors(&mut self) {
        let mut spent = Vec::new();
        let mut killed = Vec::new();
        for i in 0..self.state.strategic.missiles.len() {
            if self.state.strategic.missiles[i].kind != MissileKind::Interceptor {
                continue;
            }
            let (quarry, owner) = {
                let m = &self.state.strategic.missiles[i];
                (m.quarry, m.owner)
            };
            // Its warhead, or failing that another nobody is after.
            let mut prey = self
                .state
                .strategic
                .missiles
                .iter()
                .position(|w| w.kind == MissileKind::Warhead && w.serial == quarry && !killed.contains(&w.serial));
            if prey.is_none() {
                let hunted: Vec<u32> = self
                    .state
                    .strategic
                    .missiles
                    .iter()
                    .filter(|m| m.kind == MissileKind::Interceptor)
                    .map(|m| m.quarry)
                    .collect();
                let here = self.state.strategic.missiles[i].pos;
                prey = self
                    .state
                    .strategic
                    .missiles
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| {
                        w.kind == MissileKind::Warhead
                            && self.are_enemies(owner, w.owner)
                            && !hunted.contains(&w.serial)
                            && !killed.contains(&w.serial)
                            && w.pos.xy().distance(here.xy()) < Fx::from_int(6000)
                    })
                    .min_by_key(|(_, w)| (w.pos.xy().distance(here.xy()), w.serial))
                    .map(|(j, _)| j);
                if let Some(j) = prey {
                    let s = self.state.strategic.missiles[j].serial;
                    self.state.strategic.missiles[i].quarry = s;
                }
            }
            let speed = {
                let m = &self.state.strategic.missiles[i];
                self.blueprints.unit(m.blueprint).strategic.as_ref().map_or(Fx::from_int(90), |s| s.speed)
            };
            let target = prey.map(|j| {
                let w = &self.state.strategic.missiles[j];
                let (path, cruise) = self.warhead_flight(w);
                (w.pos, w.prev_pos, w.serial, path, cruise, w.age, w.travelled)
            });
            let m = &mut self.state.strategic.missiles[i];
            m.prev_pos = m.pos;
            m.age += 1;
            if m.age > INTERCEPT_LIFE || (target.is_none() && m.age > INTERCEPT_BOOST) {
                // Burnt out, or nothing left to hunt: it bursts where it is.
                spent.push(i);
                self.events.push(SimEvent::WarheadIntercepted {
                    pos: m.pos,
                    owner,
                    killed: false,
                });
                continue;
            }
            if m.age <= INTERCEPT_BOOST {
                let f = Fx::from_int(m.age as i32) / Fx::from_int(INTERCEPT_BOOST as i32);
                let climb = speed * (Fx::ratio(1, 6) + f * Fx::ratio(5, 6));
                // Up out of the cell, already leaning toward the warhead.
                let lean = target.map_or(FxVec3::ZERO, |(p, ..)| {
                    let d = (p - m.pos).xy().normalize();
                    d.extend(Fx::ZERO) * (climb * f * Fx::ratio(2, 5))
                });
                m.vel = FxVec3::new(Fx::ZERO, Fx::ZERO, climb) + lean;
            } else if let Some((p, _, _, path, cruise, age, travelled)) = target {
                // Lead the warhead along the path it flies: the first point of it this can
                // reach by the time the warhead gets there.
                let mut aim = p;
                let (mut a, mut s) = (age, travelled);
                for n in 1..=LEAD_TICKS {
                    (a, s) = path.step(cruise, a, s);
                    aim = path.at(s);
                    if (aim - m.pos).length() <= speed * n {
                        break;
                    }
                }
                let want = (aim - m.pos).normalize() * speed;
                let turned = m.vel + (want - m.vel) * Fx::ratio(2, 5);
                m.vel = turned.normalize() * speed;
            }
            m.pos += m.vel;
            if let Some((p, pp, serial, ..)) = target {
                let (gap, at) = closest_over_tick(m.prev_pos, m.pos, pp, p);
                if gap <= KILL_REACH {
                    m.pos = at;
                    killed.push(serial);
                    spent.push(i);
                    self.events.push(SimEvent::WarheadIntercepted {
                        pos: at,
                        owner,
                        killed: true,
                    });
                }
            }
        }
        // Remove the spent interceptors and the warheads they killed, highest index first.
        let mut gone: Vec<usize> = spent;
        for serial in killed {
            if let Some(j) = self
                .state
                .strategic
                .missiles
                .iter()
                .position(|w| w.kind == MissileKind::Warhead && w.serial == serial)
            {
                gone.push(j);
            }
        }
        gone.sort_unstable();
        gone.dedup();
        for &i in gone.iter().rev() {
            self.state.strategic.missiles.remove(i);
        }
    }

    /// A nuclear blast at `pos`: announced now, its front run out over the next ticks.
    pub(crate) fn detonate(
        &mut self,
        pos: FxVec3,
        blast: NuclearBlast,
        owner: u8,
        source: UnitId,
        commander: bool,
    ) -> Result<(), SimError> {
        self.events.push(SimEvent::NuclearDetonation {
            pos,
            radius: blast.radius,
            owner,
            commander,
        });
        self.state.strategic.detonations.push(Detonation {
            pos,
            owner,
            source,
            radius: blast.radius,
            core: blast.core,
            damage: blast.damage,
            edge: blast.edge,
            front_ticks: blast.front_ticks,
            age: 0,
            reached: Fx::ZERO,
            hit: Vec::new(),
            screens: Vec::new(),
        });
        // The first ring lands the tick it forms.
        self.run_detonation(self.state.strategic.detonations.len() - 1);
        Ok(())
    }

    fn run_detonations(&mut self) -> Result<(), SimError> {
        let mut i = 0;
        while i < self.state.strategic.detonations.len() {
            let d = &mut self.state.strategic.detonations[i];
            d.age += 1;
            if d.age > d.front_ticks {
                self.state.strategic.detonations.swap_remove(i);
                continue;
            }
            self.run_detonation(i);
            i += 1;
        }
        Ok(())
    }

    /// Everything the front of detonation `i` reaches this tick.
    fn run_detonation(&mut self, i: usize) {
        let d = &self.state.strategic.detonations[i];
        let blast = d.blast();
        let front = blast.front_at(d.age);
        // Trees go at the same pace, out to further than armour breaks.
        let tree_front = front * TREE_REACH;
        let (center, from, owner, source) = (d.pos.xy(), d.reached, d.owner, d.source);
        let origin = d.pos;
        let mut victims = Vec::new();
        self.index.query(center, front, kind::UNIT, |e| {
            let row = e.row as usize;
            if self.unit_entry_is_current(e) && !self.state.units.has_flag(row, flag::IN_FACTORY) {
                victims.push(row);
            }
            true
        });
        victims.sort_unstable();
        victims.dedup();
        let mut charged = Vec::new();
        for row in victims {
            let id = self.state.units.id(row);
            if self.state.strategic.detonations[i].hit.contains(&id) {
                continue;
            }
            let radius = self.bp(row).radius;
            let height = self.bp(row).height;
            let reach = (self.state.units.pos[row].distance(center) - radius).max(Fx::ZERO);
            if reach > front {
                continue;
            }
            self.state.strategic.detonations[i].hit.push(id);
            let damage = blast.damage_at(reach);
            let target = self.state.units.pos[row].extend(self.state.units.z[row] + height / 2);
            // A dome takes what it can hold and the rest goes through: nothing that
            // stands against a warhead's middle survives it, dome or no dome.
            let screen = self.state.strategic.detonations[i].screens.iter().find(|&&(c, r, _)| {
                use crate::combat::dome_space;
                dome_space(target - c, r).length_sq() < r * r
                    && dome_space(origin - c, r).length_sq() >= r * r
            });
            let damage = if let Some(&(_, _, held)) = screen {
                damage - held
            } else if let Some(shield) = self.blast_blocker(origin, target, Some(row)) {
                let held = if self.state.units.has_flag(shield, flag::INVULNERABLE) {
                    Fx::MAX
                } else {
                    self.state.units.shield_hp[shield]
                };
                // A dome takes the blast once (a hull field once per tick it is reached, as
                // it covers only its own hull); later ticks of the front find its screen.
                if self.bp(shield).shield.is_some_and(|s| !s.is_hull()) {
                    let c = self.state.units.pos[shield].extend(self.state.units.z[shield]);
                    let r = self.dome_radius(shield);
                    self.state.strategic.detonations[i].screens.push((c, r, held));
                }
                if !charged.contains(&shield) {
                    charged.push(shield);
                    self.damage_shield(shield, damage);
                }
                damage - held.min(damage)
            } else {
                damage
            };
            if damage <= Fx::ZERO {
                continue;
            }
            let by = if self.are_enemies(owner, self.state.units.owner[row]) {
                owner
            } else {
                u8::MAX
            };
            self.damage_unit(row, damage, by, source);
        }
        let tree_from = from * TREE_REACH;
        let mut felled = Vec::new();
        self.prop_index.query(center, tree_front, kind::PROP, |e| {
            let prop = e.row as usize;
            let dist = self.map.props[prop].pos.distance(center);
            if dist >= tree_from && dist <= tree_front {
                felled.push(prop);
            }
            true
        });
        for prop in felled {
            if self.map.props[prop].kind.is_tree() {
                self.state.props_dead[prop / 64] |= 1 << (prop % 64);
            }
        }
        let d = &mut self.state.strategic.detonations[i];
        d.reached = front;
    }
}

impl World {
    /// `UnitInstance::_pad3[2]` for a launcher (`LAUNCHER_*`); zero for anything else.
    pub(crate) fn launcher_pad(&self, row: usize) -> u32 {
        let Some(spec) = self.launcher_spec(row) else {
            return 0;
        };
        let id = self.state.units.id(row);
        let l = self.state.strategic.launchers.get(&id);
        let stock = l.map_or(0, |l| l.stock) as u32;
        let progress = l.map_or(Fx::ZERO, |l| l.progress);
        let part = if spec.round_time > Fx::ZERO {
            ((progress / spec.round_time).to_f32() * 255.0).clamp(0.0, 255.0) as u32
        } else {
            0
        };
        let firing = spec.kind == StrategicKind::Nuke && l.is_some_and(|l| l.launch.is_some() || !l.targets.is_empty());
        let manual = l.is_some_and(|l| l.manual);
        let queued = l.map_or(0, |l| l.queued.min(31)) as u32;
        stock
            | part << LAUNCHER_PROGRESS_SHIFT
            | (spec.stock as u32) << LAUNCHER_CAPACITY_SHIFT
            | if firing { LAUNCHER_FIRING } else { 0 }
            | LAUNCHER_MARK
            | if manual { LAUNCHER_MANUAL } else { 0 }
            | queued << LAUNCHER_QUEUED_SHIFT
    }

    /// Every strategic missile in flight, for the renderer and the interface.
    pub(crate) fn write_strategic(&self, out: &mut Vec<crate::mirror::StrategicInstance>) {
        use crate::mirror::{StrategicInstance, STRATEGIC_INTERCEPTOR, STRATEGIC_WARHEAD};
        out.clear();
        let tps = mc_core::TICKS_PER_SECOND as f32;
        for m in &self.state.strategic.missiles {
            let (kind, mark, eta) = match m.kind {
                MissileKind::Warhead => {
                    let (path, cruise) = self.warhead_flight(m);
                    let left = path.ticks_left(cruise, m.age, m.travelled);
                    (STRATEGIC_WARHEAD, m.mark.to_f32(), left as f32 / tps)
                }
                MissileKind::Interceptor => {
                    let quarry = self
                        .state
                        .strategic
                        .missiles
                        .iter()
                        .find(|w| w.serial == m.quarry)
                        .map_or(m.pos.to_f32(), |w| w.pos.to_f32());
                    (STRATEGIC_INTERCEPTOR, quarry, 0.0)
                }
            };
            out.push(StrategicInstance {
                prev_pos: m.prev_pos.to_f32(),
                kind,
                pos: m.pos.to_f32(),
                owner: m.owner as u32,
                mark,
                age: m.age as f32 / tps,
                serial: m.serial,
                eta,
                boost: if m.kind == MissileKind::Warhead && m.age <= BOOST_TICKS { 1.0 } else { 0.0 },
                quarry: m.quarry,
            });
        }
    }

    /// Every warhead's path with how far along it is, and the launches ordered and not yet
    /// away: `viewer`'s side's only (everyone's with no viewer), since a mark is secret
    /// until the warhead is in the air.
    pub(crate) fn write_warhead_plans(
        &self,
        viewer: Option<u8>,
        tracks: &mut Vec<WarheadTrack>,
        plans: &mut Vec<PlannedLaunch>,
    ) {
        tracks.clear();
        plans.clear();
        let tps = mc_core::TICKS_PER_SECOND as f32;
        for m in &self.state.strategic.missiles {
            if m.kind == MissileKind::Warhead {
                let (path, _) = self.warhead_flight(m);
                tracks.push(WarheadTrack { serial: m.serial, owner: m.owner, path, travelled: m.travelled });
            }
        }
        for (id, l) in &self.state.strategic.launchers {
            let Some(row) = self.state.units.row(*id) else { continue };
            let owner = self.state.units.owner[row];
            if l.targets.is_empty() || viewer.is_some_and(|v| self.are_enemies(v, owner)) {
                continue;
            }
            let Some(spec) = self.launcher_spec(row) else { continue };
            let base = self.state.units.pos[row];
            let start = warhead_start(base.extend(self.terrain.height_at(base)));
            // Ticks until each goes: the doors, or the gap behind the one before.
            let mut wait = match l.launch {
                Some(_) => (SILO_DOOR_TICKS - self.state.units.deploy[row].min(SILO_DOOR_TICKS)) as u32,
                None if self.state.units.deploy[row] >= SILO_DOOR_TICKS => {
                    SILO_NEXT_TICKS.saturating_sub(l.idle) as u32
                }
                None => SILO_DOOR_TICKS as u32,
            };
            for (i, &(at, order)) in l.targets.iter().enumerate() {
                let ground = self.terrain.height_at(at).max(self.terrain.water_level());
                let path = WarheadPath::new(start, burst_point(at.extend(ground)), spec.apogee);
                let flight = path.ticks_left(spec.speed, 0, Fx::ZERO);
                plans.push(PlannedLaunch {
                    silo: id.0,
                    owner,
                    order,
                    path,
                    opening: i == 0 && l.launch.is_some(),
                    eta: (wait + flight) as f32 / tps,
                });
                wait += SILO_NEXT_TICKS as u32;
            }
        }
        plans.sort_by_key(|p| p.order);
    }
}

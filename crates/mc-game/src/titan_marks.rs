//! Tier-5 titans on the map (the Behemoth): what the interface keeps track of and draws
//! in the world. The HUD's cards, calls, panel and minimap marks are in `hud/titan.rs`.
//!
//! A titan's great bore (`Bore::storm`, the AEB-3) is the thing to be warned about: when
//! it starts to charge on a mark the sim says so (`SimEvent::StormCharging`), and when its
//! bolt lands the storm it raises spreads out from there (`BoreDischarge`, then
//! `titan::DischargeStorm::reach`). `Titans` follows both from the events, so everything
//! drawn here is the sim's own timing:
//!
//! - A strike charging: the storm's full reach as a ring at the mark, a charging sweep
//!   round it, a crosshair and a countdown to the shot; the line from the titan to it.
//! - A storm raging: its spreading edge, where it will stop, and how long it has left.
//! - A titan selected: a frame round the whole machine with its name and tier, and its
//!   guns' reach labelled on the rings, dead zones included.
//! - Aiming an attack with a titan picked: the storm it would raise under the pointer, and
//!   whether the mark lies in its bore's reach, in the dead zone under it, or too far.
//!
//! The owner's side sees all of it; the other side sees a strike where it can see the
//! ground at the mark (or the titan itself), like any other shot.

use crate::game::{Mode, Targeting, View};
use crate::hud::has_flag;
use crate::orders::Field;
use crate::ui::{self, palette, rgb, type_scale, Ui};
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, Blueprints, UnitBlueprint, Weapon};
use mc_sim::mirror::{RenderFrame, UnitInstance, KIND_WRECK, STATE_UNIDENTIFIED};
use mc_sim::tables::flag;
use mc_sim::SimEvent;
use std::collections::HashMap;
use std::f32::consts::TAU;

/// A titan's storm and its strike warnings: electric violet.
pub const STORM: u32 = 0xA48CFF;
/// A titan's own mark: the pale platinum of its frame.
pub const TITAN: u32 = 0xDCE4FF;

const TPS: f32 = mc_core::TICKS_PER_SECOND as f32;
/// A charge whose bolt has not landed this long after it was due is given up on (the mark
/// died, the titan was turned away): ticks.
const BOLT_PATIENCE: i64 = 60;
/// A storm's rings linger this long after it dies away: ticks.
const STORM_FADE: i64 = 20;
/// How long a titan's sighting (or ours standing up) stays up as a live card, seconds.
pub const SIGHTED_CARD_SECONDS: f32 = 12.0;

/// Whether `bp` is a titan: a mobile machine of tier 5 or more.
pub fn is_titan(bp: &UnitBlueprint) -> bool {
    bp.tech >= 5 && bp.is_mobile()
}

/// A titan's great bore: its weapon index, and the weapon.
pub fn storm_weapon(bp: &UnitBlueprint) -> Option<(usize, &Weapon)> {
    bp.weapons
        .iter()
        .enumerate()
        .find(|(_, w)| w.bore.is_some_and(|b| b.storm.is_some()))
}

fn bp_of<'a>(blueprints: &'a Blueprints, u: &UnitInstance) -> &'a UnitBlueprint {
    blueprints.unit(BlueprintId(u.blueprint as u16))
}

fn owner_of(u: &UnitInstance) -> u8 {
    (u.owner_flags & 0xFF) as u8
}

/// Whether the viewer can see the ground at `p` now (always, without fog).
pub fn seen(frame: &RenderFrame, p: Vec2) -> bool {
    let (w, h) = frame.fog_dims;
    if frame.fog.is_empty() || w == 0 {
        return true;
    }
    let (x, y) = ((p.x / 64.0).floor(), (p.y / 64.0).floor());
    if x < 0.0 || y < 0.0 || x as u32 >= w || y as u32 >= h {
        return false;
    }
    frame
        .fog
        .get(((y as u32 * w + x as u32) * 2) as usize)
        .is_some_and(|&v| v > 0)
}

/// One strike of a titan's great bore: charging on its mark, then its storm.
#[derive(Clone, Debug)]
pub struct Strike {
    /// The titan firing it (`u32::MAX` when only the landing was seen).
    pub unit: u32,
    pub owner: u8,
    pub blueprint: BlueprintId,
    pub weapon: u8,
    /// Where it is aimed.
    pub target: Vec3,
    /// The unit it is on, if any: `target` follows it while it is in sight.
    pub on: Option<u32>,
    /// How far its storm reaches when fully spread, metres.
    pub radius: f32,
    /// The tick the charge began, and how many ticks it takes.
    pub charge_from: i64,
    pub charge_ticks: u16,
    /// The tick the bolt landed, and where: the storm's centre.
    pub struck: Option<(i64, Vec3)>,
    /// How long the storm grows, ticks.
    pub storm_ticks: u16,
}

/// Where a strike stands at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Charging: how far, 0..1, and seconds to the shot.
    Charging(f32, f32),
    /// Fired; the bolt is on its way.
    Bolt,
    /// Its storm: how far it has spread (metres), how far along its life (0..1), seconds left.
    Storm(f32, f32, f32),
    /// The storm has died away: fading out, 1 → 0.
    Spent(f32),
}

impl Strike {
    /// Its phase at `tick` (plus `alpha` of the next).
    pub fn phase(&self, tick: i64, alpha: f32) -> Phase {
        let now = tick as f32 + alpha;
        if let Some((at, _)) = self.struck {
            let age = now - at as f32;
            let life = self.storm_ticks.max(1) as f32;
            if age >= life {
                return Phase::Spent((1.0 - (age - life) / STORM_FADE as f32).clamp(0.0, 1.0));
            }
            let f = (age / life).clamp(0.0, 1.0);
            // `titan::DischargeStorm::reach`: a fifth at once, the rest over its life, quick at first.
            let reach = self.radius * (0.2 + 0.8 * (2.0 * f - f * f));
            return Phase::Storm(reach, f, (life - age) / TPS);
        }
        let due = (self.charge_from + self.charge_ticks as i64) as f32;
        if now < due {
            let k =
                ((now - self.charge_from as f32) / self.charge_ticks.max(1) as f32).clamp(0.0, 1.0);
            Phase::Charging(k, (due - now) / TPS)
        } else {
            Phase::Bolt
        }
    }

    /// The storm's centre: where the bolt landed, or else where it is aimed.
    pub fn centre(&self) -> Vec3 {
        self.struck.map_or(self.target, |s| s.1)
    }

    /// `centre`, drawn: a charge on a unit in sight rides with it between ticks.
    pub fn drawn_centre(&self, view: &View, alpha: f32) -> Vec3 {
        let followed = self
            .on
            .filter(|_| self.struck.is_none())
            .and_then(|id| view.index_of.get(&id))
            .and_then(|&i| view.frame.units.get(i))
            .filter(|u| u.owner_flags & (KIND_WRECK | STATE_UNIDENTIFIED) == 0);
        followed.map_or(self.centre(), |u| {
            Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), alpha)
        })
    }

    fn over(&self, tick: i64) -> bool {
        match self.struck {
            Some((at, _)) => tick > at + self.storm_ticks as i64 + STORM_FADE,
            None => tick > self.charge_from + self.charge_ticks as i64 + BOLT_PATIENCE,
        }
    }
}

/// A titan's great bore, as last heard: when its last charge began and how long its
/// charge and reload take (ticks). Enough to tell charging, recharging and ready apart.
#[derive(Clone, Copy, Debug)]
pub struct BoreClock {
    pub charge_from: i64,
    pub charge_ticks: u16,
    pub reload_ticks: u16,
}

/// A titan's great bore right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoreState {
    /// Charging: 0..1, seconds to the shot.
    Charging(f32, f32),
    /// Recharging after a shot: 0..1, seconds until it can charge again.
    Recharging(f32, f32),
    Ready,
}

impl BoreClock {
    pub fn state(&self, tick: i64) -> BoreState {
        let fire = self.charge_from + self.charge_ticks as i64;
        if tick < fire {
            let k = (tick - self.charge_from) as f32 / self.charge_ticks.max(1) as f32;
            return BoreState::Charging(k.clamp(0.0, 1.0), (fire - tick) as f32 / TPS);
        }
        // The next charge begins `charge_ticks` before the reload is out.
        let span = (self.reload_ticks as i64 - self.charge_ticks as i64).max(1);
        let ready = fire + span;
        if tick < ready {
            let k = (tick - fire) as f32 / span as f32;
            return BoreState::Recharging(k.clamp(0.0, 1.0), (ready - tick) as f32 / TPS);
        }
        BoreState::Ready
    }
}

/// Something about titans the player should be told of (`hud/titan.rs` turns these into
/// cards and calls).
#[derive(Clone, Debug, PartialEq)]
pub struct News {
    pub serial: u64,
    pub kind: NewsKind,
    pub unit: u32,
    pub owner: u8,
    /// The titan's name ("Behemoth").
    pub name: String,
    pub at: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewsKind {
    /// An enemy titan seen for the first time, walking.
    Sighted,
    /// An enemy titan's lot seen, the machine still going up on it.
    SiteSpotted,
    /// One of ours finished and stood up.
    Online,
    /// An enemy titan's bore started to charge on ground we can see and hold.
    StrikeWarning,
}

/// A bore's charge as the sim announced it (`StormCharging`, or `StormRetargeted` for one
/// whose start was not heard).
struct Charge {
    unit: u32,
    owner: u8,
    blueprint: BlueprintId,
    weapon: u8,
    target: Vec3,
    on: Option<u32>,
    radius: f32,
    charge_from: i64,
    charge_ticks: u16,
}

impl Charge {
    /// Marks the strike, starts its titan's bore clock and warns if it is coming down on
    /// ground of ours we can see (`warned`).
    fn begin(
        &self,
        t: &mut Titans,
        blueprints: &Blueprints,
        warned: &impl Fn(u8, Vec2, f32) -> bool,
    ) {
        let bp = blueprints.unit(self.blueprint);
        let Some(w) = bp.weapons.get(self.weapon as usize) else {
            return;
        };
        t.strikes.push(Strike {
            unit: self.unit,
            owner: self.owner,
            blueprint: self.blueprint,
            weapon: self.weapon,
            target: self.target,
            on: self.on,
            radius: self.radius,
            charge_from: self.charge_from,
            charge_ticks: self.charge_ticks,
            struck: None,
            storm_ticks: w.bore.and_then(|b| b.storm).map_or(90, |s| s.ticks),
        });
        t.bores.insert(
            self.unit,
            BoreClock {
                charge_from: self.charge_from,
                charge_ticks: self.charge_ticks,
                reload_ticks: w.reload_ticks,
            },
        );
        self.warn(t, blueprints, warned);
    }

    fn warn(
        &self,
        t: &mut Titans,
        blueprints: &Blueprints,
        warned: &impl Fn(u8, Vec2, f32) -> bool,
    ) {
        let at = self.target.truncate();
        if !warned(self.owner, at, self.radius) {
            return;
        }
        t.serial += 1;
        t.news.push(News {
            serial: t.serial,
            kind: NewsKind::StrikeWarning,
            unit: self.unit,
            owner: self.owner,
            name: blueprints.unit(self.blueprint).name.clone(),
            at,
        });
    }
}

/// What the interface knows of titans this match (`View::titans`).
#[derive(Default)]
pub struct Titans {
    pub strikes: Vec<Strike>,
    /// Great bores by titan, as last heard charging.
    pub bores: HashMap<u32, BoreClock>,
    /// Titans seen: still under construction when last seen?
    known: HashMap<u32, bool>,
    /// The news of the last while, oldest first, numbered from 1.
    pub news: Vec<News>,
    serial: u64,
    last_tick: Option<u32>,
}

impl Titans {
    fn tell(&mut self, kind: NewsKind, u: &UnitInstance, name: &str, owner: u8) {
        self.serial += 1;
        self.news.push(News {
            serial: self.serial,
            kind,
            unit: u.unit_id,
            owner,
            name: name.to_owned(),
            at: Vec2::new(u.pos[0], u.pos[1]),
        });
        if self.news.len() > 16 {
            self.news.remove(0);
        }
    }

    /// The great bore of titan `unit`, now.
    pub fn bore(&self, unit: u32, tick: u32) -> BoreState {
        self.bores
            .get(&unit)
            .map_or(BoreState::Ready, |c| c.state(tick as i64))
    }
}

/// Reads a fresh frame: strikes begun and landed, titans seen for the first time, ours
/// finished. Called once per frame pulled from the sim.
pub fn observe(view: &mut View, blueprints: &Blueprints) {
    let frame = &view.frame;
    let tick = frame.tick as i64;
    let local = view.local;
    let observing = view.observing;
    let teams: Vec<u8> = view.status.players.iter().map(|p| p.team).collect();
    let team = |p: u8| teams.get(p as usize).copied();
    let hostile = |p: u8| !observing && team(p) != team(local);
    let warned = |owner: u8, at: Vec2, radius: f32| {
        hostile(owner)
            && seen(frame, at)
            && frame.units.iter().any(|u| {
                u.owner_flags & KIND_WRECK == 0
                    && owner_of(u) == local
                    && Vec2::new(u.pos[0], u.pos[1]).distance(at) < radius + 150.0
            })
    };
    let t = &mut view.titans;
    // A new match, or the range reset: start over.
    if t.last_tick.is_some_and(|last| frame.tick < last) {
        *t = Titans::default();
    }
    t.last_tick = Some(frame.tick);

    for e in &frame.events {
        match e {
            SimEvent::StormCharging {
                unit,
                target,
                on,
                radius,
                ticks,
                owner,
                blueprint,
                weapon,
                ..
            } => {
                let charge = Charge {
                    unit: unit.0,
                    owner: *owner,
                    blueprint: *blueprint,
                    weapon: *weapon,
                    target: Vec3::from(target.to_f32()),
                    on: on.map(|u| u.0),
                    radius: radius.to_f32(),
                    charge_from: tick,
                    charge_ticks: *ticks,
                };
                // A new charge from the same titan replaces one that never fired.
                t.strikes
                    .retain(|s| !(s.unit == unit.0 && s.struck.is_none()));
                charge.begin(t, blueprints, &warned);
            }
            // The bore changed its mark mid-charge (its target died): the mark follows it.
            SimEvent::StormRetargeted {
                unit,
                target,
                on,
                left,
                radius,
                ticks,
                owner,
                blueprint,
                weapon,
            } => {
                let before = t
                    .strikes
                    .iter()
                    .position(|s| s.unit == unit.0 && s.struck.is_none());
                let Some(target) = target else {
                    if let Some(i) = before {
                        t.strikes.remove(i);
                    }
                    continue;
                };
                let charge = Charge {
                    unit: unit.0,
                    owner: *owner,
                    blueprint: *blueprint,
                    weapon: *weapon,
                    target: Vec3::from(target.to_f32()),
                    on: on.map(|u| u.0),
                    radius: radius.to_f32(),
                    charge_from: tick - (*ticks as i64 - *left as i64),
                    charge_ticks: *ticks,
                };
                match before {
                    Some(i) => {
                        let s = &mut t.strikes[i];
                        // Warned again only when it swings onto ground the old mark missed.
                        let far =
                            s.target.truncate().distance(charge.target.truncate()) > charge.radius;
                        s.target = charge.target;
                        s.on = charge.on;
                        if far {
                            charge.warn(t, blueprints, &warned);
                        }
                    }
                    None => charge.begin(t, blueprints, &warned),
                }
            }
            SimEvent::BoreDischarge {
                to,
                owner,
                blueprint,
                weapon,
                ..
            } => {
                let bp = blueprints.unit(*blueprint);
                let Some(storm) = bp
                    .weapons
                    .get(*weapon as usize)
                    .and_then(|w| w.bore)
                    .and_then(|b| b.storm)
                else {
                    continue;
                };
                let to = Vec3::from(to.to_f32());
                let mine = t
                    .strikes
                    .iter_mut()
                    .filter(|s| {
                        s.struck.is_none()
                            && s.owner == *owner
                            && s.blueprint == *blueprint
                            && s.weapon == *weapon
                    })
                    .min_by(|a, b| a.target.distance(to).total_cmp(&b.target.distance(to)));
                match mine {
                    Some(s) => s.struck = Some((tick, to)),
                    None => t.strikes.push(Strike {
                        unit: u32::MAX,
                        owner: *owner,
                        blueprint: *blueprint,
                        weapon: *weapon,
                        target: to,
                        on: None,
                        radius: storm.radius.to_f32(),
                        charge_from: tick,
                        charge_ticks: 0,
                        struck: Some((tick, to)),
                        storm_ticks: storm.ticks,
                    }),
                }
            }
            _ => {}
        }
    }
    t.strikes.retain(|s| !s.over(tick));
    // A charge on a unit follows it while it is in sight.
    for s in t.strikes.iter_mut().filter(|s| s.struck.is_none()) {
        let Some(id) = s.on else { continue };
        if let Some(u) = frame
            .units
            .iter()
            .find(|u| u.unit_id == id && u.owner_flags & (KIND_WRECK | STATE_UNIDENTIFIED) == 0)
        {
            s.target = Vec3::from(u.pos);
        }
    }

    // Titans seen: an enemy's first sighting, its lot, and ours standing up.
    let mut news: Vec<(NewsKind, &UnitInstance, u8)> = Vec::new();
    for u in &frame.units {
        if u.owner_flags & (KIND_WRECK | STATE_UNIDENTIFIED) != 0 {
            continue;
        }
        let bp = bp_of(blueprints, u);
        if !is_titan(bp) {
            continue;
        }
        let owner = owner_of(u);
        let building = has_flag(u, flag::UNDER_CONSTRUCTION);
        let before = t.known.insert(u.unit_id, building);
        if hostile(owner) {
            match (before, building) {
                (None, true) => news.push((NewsKind::SiteSpotted, u, owner)),
                (None, false) | (Some(true), false) => news.push((NewsKind::Sighted, u, owner)),
                _ => {}
            }
        } else if owner == local && !observing && before == Some(true) && !building {
            news.push((NewsKind::Online, u, owner));
        }
    }
    for (kind, u, owner) in news {
        let name = bp_of(blueprints, u).name.clone();
        t.tell(kind, u, &name, owner);
    }
}

/// Headless shots: what `observe` would have gathered over the ticks run without frames,
/// read off the world itself (bores charging, storms raging), plus the titans in sight.
/// `MERIDIAN_TITAN_NEWS=sighted|site|online|strike` adds that call for the first titan;
/// `MERIDIAN_TITAN_EYES=n` draws the interface as player n.
pub fn seed(view: &mut View, world: &mc_sim::World, blueprints: &Blueprints) {
    let tick = world.state.tick as i64;
    let units = &world.state.units;
    let mut strikes = Vec::new();
    let mut bores = HashMap::new();
    for row in units.slots.iter() {
        let bp = world.bp(row);
        let Some((w, weapon)) = storm_weapon(bp) else {
            continue;
        };
        let Some(storm) = weapon.bore.and_then(|b| b.storm) else {
            continue;
        };
        let id = units.id(row).0;
        let cd = units.weapon_cooldown[row][w] as i64;
        let charge = weapon.charge_ticks as i64;
        if cd == 0 {
            continue;
        }
        let charge_from = if cd <= charge {
            tick - (charge - cd)
        } else {
            tick + cd - weapon.reload_ticks as i64 - charge
        };
        bores.insert(
            id,
            BoreClock {
                charge_from,
                charge_ticks: weapon.charge_ticks,
                reload_ticks: weapon.reload_ticks,
            },
        );
        if cd <= charge {
            if let Some(target) = units.row(units.weapon_target[row][w]) {
                strikes.push(Strike {
                    unit: id,
                    owner: units.owner[row],
                    blueprint: bp.id,
                    weapon: w as u8,
                    target: Vec3::from(units.pos[target].extend(units.z[target]).to_f32()),
                    on: Some(units.id(target).0),
                    radius: storm.radius.to_f32(),
                    charge_from,
                    charge_ticks: weapon.charge_ticks,
                    struck: None,
                    storm_ticks: storm.ticks,
                });
            }
        }
    }
    for st in &world.state.storms {
        let from = units.row(st.source);
        let (blueprint, weapon) = from
            .and_then(|r| storm_weapon(world.bp(r)).map(|(w, _)| (world.bp(r).id, w as u8)))
            .unwrap_or((BlueprintId(0), 0));
        strikes.push(Strike {
            unit: st.source.0,
            owner: st.owner,
            blueprint,
            weapon,
            target: Vec3::from(st.pos.extend(st.z).to_f32()),
            on: None,
            radius: st.radius.to_f32(),
            charge_from: tick - st.age as i64,
            charge_ticks: 0,
            struck: Some((
                tick - st.age as i64,
                Vec3::from(st.pos.extend(st.z).to_f32()),
            )),
            storm_ticks: st.ticks,
        });
    }
    // `MERIDIAN_TITAN_EYES=n`: the interface as player n sees it (the other side's warnings).
    if let Some(p) = std::env::var("MERIDIAN_TITAN_EYES")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        view.local = p;
    }
    // The titans in sight, known already: no calls for them unless asked.
    observe(view, blueprints);
    view.titans.news.clear();
    view.titans.strikes = strikes;
    view.titans.bores = bores;
    if let Ok(kind) = std::env::var("MERIDIAN_TITAN_NEWS") {
        let kind = match kind.as_str() {
            "site" => NewsKind::SiteSpotted,
            "online" => NewsKind::Online,
            "strike" => NewsKind::StrikeWarning,
            _ => NewsKind::Sighted,
        };
        let first = view
            .frame
            .units
            .iter()
            .find(|u| u.owner_flags & KIND_WRECK == 0 && is_titan(bp_of(blueprints, u)));
        if let Some(u) = first {
            let name = bp_of(blueprints, u).name.clone();
            let owner = owner_of(u);
            view.titans.tell(kind, u, &name, owner);
        }
    }
}

// ------------------------------------------------------------------ drawing

fn surface(field: &Field, p: Vec2) -> f32 {
    field.renderer.surface_height(p)
}

fn project(ui: &Ui, field: &Field, p: Vec3) -> Option<Vec2> {
    field.camera.project(p).map(|q| q / ui.s)
}

fn ground(ui: &Ui, field: &Field, p: Vec2) -> Option<Vec2> {
    project(ui, field, p.extend(surface(field, p) + 2.0))
}

/// A ring on the ground: solid, or dashed (every other segment).
fn ring(
    ui: &mut Ui,
    field: &Field,
    c: Vec2,
    radius: f32,
    width: f32,
    color: ui::Color,
    dashed: bool,
    spin: f32,
) {
    arc(ui, field, c, radius, spin, spin + TAU, width, color, dashed);
}

/// Part of a ring on the ground, from angle `a0` to `a1`.
fn arc(
    ui: &mut Ui,
    field: &Field,
    c: Vec2,
    radius: f32,
    a0: f32,
    a1: f32,
    width: f32,
    color: ui::Color,
    dashed: bool,
) {
    let span = (a1 - a0).abs();
    let segments = (((radius / 14.0) * span / TAU) as usize).clamp(8, 240) & !1;
    let points: Vec<Option<Vec2>> = (0..=segments)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / segments as f32;
            ground(ui, field, c + Vec2::from_angle(a) * radius)
        })
        .collect();
    let step = if dashed { 2 } else { 1 };
    let mut last = points[0];
    for i in 0..segments {
        let next = points[i + 1];
        if i % step == 0 {
            if let (Some(a), Some(b)) = (last, next) {
                ui.stroke(a, b, width, color);
            }
        }
        last = next;
    }
}

/// A wash over the ground inside `radius` of `c`: a fan of triangles draped on the ground.
fn disc(ui: &mut Ui, field: &Field, c: Vec2, radius: f32, color: ui::Color) {
    let Some(mid) = ground(ui, field, c) else {
        return;
    };
    let segments = 64;
    let points: Vec<Option<Vec2>> = (0..=segments)
        .map(|i| {
            ground(
                ui,
                field,
                c + Vec2::from_angle(i as f32 / segments as f32 * TAU) * radius,
            )
        })
        .collect();
    for pair in points.windows(2) {
        if let (Some(a), Some(b)) = (pair[0], pair[1]) {
            ui.triangle(mid, a, b, color);
        }
    }
}

/// A label on a dark backing, centred on `at`, with a bar of `tone` on its left.
fn tag(ui: &mut Ui, at: Vec2, text: &str, tone: u32, alpha: f32) {
    let w = ui.text_width(type_scale::MICRO, text) + 14.0;
    let r = ui::Rect::new(at.x - w * 0.5, at.y - 9.0, w, 18.0);
    ui.fill(r, ui::ink(0.72 * alpha));
    ui.fill(ui::Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 0.9 * alpha));
    ui.text_centred(at.x + 1.0, at.y, type_scale::MICRO, rgb(tone, alpha), text);
}

fn seconds(s: f32) -> String {
    if s >= 10.0 {
        format!("{:.0} s", s.ceil())
    } else {
        format!("{:.1} s", s.max(0.0))
    }
}

/// A distance as a player reads it: metres, or kilometres past two.
pub fn metres(m: f32) -> String {
    if m >= 2000.0 {
        format!("{:.1} km", m / 1000.0)
    } else {
        format!("{m:.0} m")
    }
}

/// Where on screen a ring label may go: clear of the economy and range panels, the
/// notices, the minimap column and the deck. Labels reach right of their point, so the
/// right edge leaves room for one.
fn label_room(ui: &Ui) -> ui::Rect {
    ui::Rect::new(
        340.0,
        160.0,
        ui.size.x - 340.0 - 300.0 - 190.0,
        ui.size.y - 160.0 - 345.0,
    )
}

/// Where a titan's ring labels go: the world direction along which the most of `radii`
/// land on screen clear of the HUD's panels, leaning right and down.
fn label_bearing(ui: &Ui, field: &Field, c: Vec2, radii: &[f32]) -> f32 {
    let Some(at) = ground(ui, field, c) else {
        return 0.0;
    };
    let safe = label_room(ui);
    let score = |a: f32| -> f32 {
        let dir = Vec2::from_angle(a);
        let fits = radii
            .iter()
            .filter(|&&r| ground(ui, field, c + dir * r).is_some_and(|q| safe.contains(q)))
            .count() as f32;
        let lean = ground(ui, field, c + dir * 400.0).map_or(0.0, |q| {
            let d = (q - at).normalize_or_zero();
            d.x + 0.35 * d.y
        });
        fits * 10.0 + lean
    };
    (0..36)
        .map(|i| i as f32 / 36.0 * TAU)
        .max_by(|&a, &b| score(a).total_cmp(&score(b)))
        .unwrap_or(0.0)
}

/// Whether `strike` is shown to this viewer: always to its own side, else where its mark
/// or its titan can be seen.
pub fn shown(view: &View, s: &Strike) -> bool {
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    if view.observing || team(s.owner) == team(view.local) {
        return true;
    }
    seen(&view.frame, s.centre().truncate())
        || view
            .index_of
            .get(&s.unit)
            .is_some_and(|&i| view.frame.units[i].owner_flags & STATE_UNIDENTIFIED == 0)
}

/// Everything above, for this frame.
pub fn draw(ui: &mut Ui, field: &Field, alpha: f32, cursor: Option<Vec3>) {
    let view = field.view;
    let t = ui.time;
    let tick = view.frame.tick as i64;
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    let hostile = |p: u8| !view.observing && team(p) != team(view.local);

    // ---- strikes ---------------------------------------------------------------------
    for s in view.titans.strikes.iter().filter(|s| shown(view, s)) {
        let enemy = hostile(s.owner);
        let tone = if enemy { palette::BAD } else { STORM };
        let c = s.drawn_centre(view, alpha).truncate();
        match s.phase(tick, alpha) {
            Phase::Charging(k, left) => {
                // Faster as the shot comes.
                let pulse = 0.5 + 0.5 * (t * (3.0 + 9.0 * k)).sin();
                // The ground the storm will take, washed: the other side's warning to get out.
                disc(
                    ui,
                    field,
                    c,
                    s.radius,
                    rgb(tone, if enemy { 0.07 + 0.07 * pulse } else { 0.05 }),
                );
                ring(
                    ui,
                    field,
                    c,
                    s.radius,
                    2.2,
                    rgb(tone, 0.6 + 0.35 * pulse),
                    false,
                    0.0,
                );
                // The charge filling round just outside it.
                arc(
                    ui,
                    field,
                    c,
                    s.radius * 1.045,
                    -TAU * 0.25,
                    -TAU * 0.25 + TAU * k,
                    3.2,
                    rgb(STORM, 0.95),
                    false,
                );
                arc(
                    ui,
                    field,
                    c,
                    s.radius * 1.045,
                    -TAU * 0.25 + TAU * k,
                    TAU * 0.75,
                    1.0,
                    rgb(STORM, 0.3),
                    true,
                );
                ring(
                    ui,
                    field,
                    c,
                    s.radius * 0.34,
                    1.4,
                    rgb(tone, 0.5),
                    false,
                    0.0,
                );
                // The line from the titan to its mark.
                if let Some(u) = view.index_of.get(&s.unit).map(|&i| &view.frame.units[i]) {
                    let bp = bp_of(field.blueprints, u);
                    let arm = Vec3::from(u.pos) + Vec3::Z * bp.height.to_f32() * 0.6;
                    if let (Some(a), Some(b)) = (project(ui, field, arm), ground(ui, field, c)) {
                        crawl(ui, a, b, 1.4, rgb(STORM, 0.55), 80.0);
                    }
                }
                if let Some(g) = ground(ui, field, c) {
                    cross(ui, g, 16.0, rgb(tone, 1.0));
                    let who = if enemy {
                        "Incoming AEB-3 strike"
                    } else {
                        "AEB-3 charging"
                    };
                    tag(
                        ui,
                        g + Vec2::new(0.0, 30.0),
                        &format!("{who}  \u{b7}  {}", seconds(left)),
                        tone,
                        1.0,
                    );
                    tag(
                        ui,
                        g + Vec2::new(0.0, 50.0),
                        &format!("Storm {}", metres(s.radius)),
                        STORM,
                        0.85,
                    );
                }
            }
            Phase::Bolt => {
                ring(ui, field, c, s.radius, 2.4, rgb(tone, 0.9), true, t * 0.3);
                if let Some(g) = ground(ui, field, c) {
                    cross(ui, g, 16.0, rgb(0xFFFFFF, 1.0));
                    tag(ui, g + Vec2::new(0.0, 30.0), "AEB-3 fired", tone, 1.0);
                }
            }
            Phase::Storm(reach, f, left) => {
                // Where it will stop, and its edge now, flickering like the lightning in it.
                ring(
                    ui,
                    field,
                    c,
                    s.radius,
                    1.4,
                    rgb(tone, 0.45),
                    true,
                    -t * 0.05,
                );
                let flick = 0.75 + 0.25 * (t * 23.0).sin() * (t * 7.0).cos();
                ring(
                    ui,
                    field,
                    c,
                    reach,
                    3.0,
                    rgb(STORM, 0.95 * flick),
                    false,
                    0.0,
                );
                ring(
                    ui,
                    field,
                    c,
                    reach * 0.97,
                    1.2,
                    rgb(0xFFFFFF, 0.5 * flick),
                    false,
                    0.0,
                );
                if let Some(g) = ground(ui, field, c) {
                    let who = if enemy {
                        "Enemy lightning storm"
                    } else {
                        "Lightning storm"
                    };
                    // The storm's life as a bar under its name.
                    let bar = ui::Rect::new(g.x - 50.0, g.y + 42.0, 100.0, 3.0);
                    ui.fill(bar, ui::ink(0.6));
                    ui.fill(
                        ui::Rect::new(bar.x, bar.y, bar.w * (1.0 - f), bar.h),
                        rgb(STORM, 0.9),
                    );
                    tag(
                        ui,
                        g + Vec2::new(0.0, 30.0),
                        &format!("{who}  \u{b7}  {}", seconds(left)),
                        tone,
                        1.0,
                    );
                }
            }
            Phase::Spent(k) => {
                ring(ui, field, c, s.radius, 1.6, rgb(STORM, 0.6 * k), false, 0.0);
            }
        }
    }

    // ---- titans selected ----------------------------------------------------------------
    let picked: Vec<&UnitInstance> = view
        .selection
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
        .filter(|u| is_titan(bp_of(field.blueprints, u)))
        .collect();
    for u in &picked {
        frame(ui, field, u, alpha, true);
        reach_labels(ui, field, u, alpha);
    }

    // ---- aiming an attack --------------------------------------------------------------
    let aiming = matches!(
        view.mode,
        Mode::Target(Targeting::Attack | Targeting::AttackGround | Targeting::Strike)
    );
    let ours: Vec<&&UnitInstance> = picked
        .iter()
        .filter(|u| owner_of(u) == view.local && !view.observing)
        .collect();
    if !aiming || ours.is_empty() {
        return;
    }
    let Some(target) = cursor else { return };
    let c = target.truncate();
    for u in &ours {
        let bp = bp_of(field.blueprints, u);
        let Some((_, w)) = storm_weapon(bp) else {
            continue;
        };
        let Some(storm) = w.bore.and_then(|b| b.storm) else {
            continue;
        };
        let at = Vec2::new(u.pos[0], u.pos[1]);
        let d = at.distance(c);
        let (lo, hi) = (w.range_min.to_f32(), w.range_max.to_f32());
        let (verdict, good) = if d < lo {
            (
                format!("Too close: inside its dead zone ({})", metres(lo)),
                false,
            )
        } else if d > hi {
            (format!("Out of reach ({})", metres(hi)), false)
        } else {
            (format!("In reach  \u{b7}  {}", metres(d)), true)
        };
        let tone = if good { STORM } else { palette::WARN };
        let breathe = 0.5 + 0.5 * (t * 3.0).sin();
        let radius = storm.radius.to_f32();
        ring(
            ui,
            field,
            c,
            radius,
            2.4,
            rgb(tone, 0.7 + 0.25 * breathe),
            false,
            0.0,
        );
        ring(
            ui,
            field,
            c,
            radius * 0.2,
            1.4,
            rgb(tone, 0.8),
            true,
            t * 0.2,
        );
        ring(
            ui,
            field,
            c,
            w.splash.to_f32(),
            1.2,
            rgb(0xFFFFFF, 0.6),
            false,
            0.0,
        );
        // The band it can strike, round the titan, while aiming.
        ring(ui, field, at, hi, 1.8, rgb(STORM, 0.5), false, 0.0);
        ring(
            ui,
            field,
            at,
            lo,
            1.6,
            rgb(palette::WARN, 0.55),
            true,
            t * 0.02,
        );
        let arm = Vec3::from(u.pos) + Vec3::Z * bp.height.to_f32() * 0.6;
        if let (Some(a), Some(b)) = (project(ui, field, arm), ground(ui, field, c)) {
            crawl(ui, a, b, 1.4, rgb(tone, 0.6), 80.0);
        }
        if let Some(g) = project(ui, field, target + Vec3::Z * 2.0) {
            for k in 0..3 {
                let a = t * 1.2 + k as f32 * TAU / 3.0;
                ui.arc(g, 11.0, a - 0.45, a + 0.45, 3.0, rgb(tone, 0.9));
            }
            ui.disc(g, 2.5, rgb(tone, 1.0));
            let clock = view.titans.bore(u.unit_id, view.frame.tick);
            let state = match clock {
                BoreState::Ready => format!("Charge {:.0} s", w.charge_ticks as f32 / TPS),
                BoreState::Charging(_, s) => format!("Charging  \u{b7}  {}", seconds(s)),
                BoreState::Recharging(_, s) => format!("Recharging  \u{b7}  {}", seconds(s)),
            };
            tag(
                ui,
                g + Vec2::new(0.0, 32.0),
                &format!("AEB-3 storm {}  \u{b7}  {state}", metres(radius)),
                STORM,
                1.0,
            );
            tag(ui, g + Vec2::new(0.0, 53.0), &verdict, tone, 1.0);
        }
        // One titan's preview says it all.
        break;
    }
}

/// A crosshair of four ticks round `g`.
fn cross(ui: &mut Ui, g: Vec2, size: f32, color: ui::Color) {
    for d in [Vec2::X, Vec2::Y] {
        ui.stroke(g - d * size, g - d * size * 0.35, 1.8, color);
        ui.stroke(g + d * size * 0.35, g + d * size, 1.8, color);
    }
}

/// A dashed line from `a` to `b`, the dashes crawling toward `b`.
fn crawl(ui: &mut Ui, a: Vec2, b: Vec2, width: f32, color: ui::Color, speed: f32) {
    let len = a.distance(b);
    if len < 2.0 {
        return;
    }
    let dir = (b - a) / len;
    let (dash, gap) = (10.0, 8.0);
    let mut s = (ui.time * speed) % (dash + gap) - (dash + gap);
    while s < len {
        let (from, to) = (s.max(0.0), (s + dash).min(len));
        if to > from {
            ui.stroke(a + dir * from, a + dir * to, width, color);
        }
        s += dash + gap;
    }
}

/// Corner brackets round a titan's whole height on screen, with its name and tier over
/// it: the selection mark a machine this size needs.
pub fn frame(ui: &mut Ui, field: &Field, u: &UnitInstance, alpha: f32, selected: bool) {
    let bp = bp_of(field.blueprints, u);
    let pos = Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), alpha);
    let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
    // The screen box round its bounding cylinder, sampled at its foot and crown.
    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    let mut any = false;
    for i in 0..8 {
        let d = Vec2::from_angle(i as f32 / 8.0 * TAU) * r * 0.8;
        for z in [0.0, h] {
            if let Some(p) = project(ui, field, pos + d.extend(z)) {
                lo = lo.min(p);
                hi = hi.max(p);
                any = true;
            }
        }
    }
    if !any {
        return;
    }
    let size = hi - lo;
    // Too small to frame: the strategic icon does the work.
    if size.y < 40.0 || size.y > ui.size.y * 3.0 {
        return;
    }
    let pad = 8.0;
    let r = ui::Rect::new(
        lo.x - pad,
        lo.y - pad,
        size.x + pad * 2.0,
        size.y + pad * 2.0,
    );
    let arm = (r.w.min(r.h) * 0.14).clamp(10.0, 42.0);
    let k = if selected { 1.0 } else { 0.55 };
    let tone = TITAN;
    for (cx, cy, dx, dy) in [
        (r.x, r.y, 1.0, 1.0),
        (r.right(), r.y, -1.0, 1.0),
        (r.right(), r.bottom(), -1.0, -1.0),
        (r.x, r.bottom(), 1.0, -1.0),
    ] {
        let c = Vec2::new(cx, cy);
        ui.stroke(c, c + Vec2::new(dx * arm, 0.0), 2.0, rgb(tone, 0.85 * k));
        ui.stroke(c, c + Vec2::new(0.0, dy * arm), 2.0, rgb(tone, 0.85 * k));
    }
    // Its name plate under the foot of the frame, clear of the notices along the top.
    let name = bp.name.as_str();
    let tier = format!("T{}", bp.tech);
    let nw = ui.text_width(type_scale::VALUE, name);
    let tw = ui.text_width(type_scale::MICRO, &tier) + 12.0;
    let plate = ui::Rect::new(r.x, r.bottom() + 6.0, nw + tw + 26.0, 20.0);
    ui.fill(plate, ui::ink(0.72 * k));
    ui.fill(ui::Rect::new(plate.x, plate.y, 2.0, plate.h), rgb(tone, k));
    ui.text(
        plate.x + 9.0,
        plate.y + 14.5,
        type_scale::VALUE,
        rgb(0xFFFFFF, k),
        name,
    );
    let chip = ui::Rect::new(plate.x + nw + 17.0, plate.y + 3.0, tw, 14.0);
    ui.fill(chip, rgb(tone, 0.9 * k));
    ui.text_centred(
        chip.x + chip.w * 0.5,
        chip.mid_y(),
        type_scale::MICRO,
        ui::ink(k),
        &tier,
    );
}

/// On a selected titan's rings, where the guns reach and where they cannot: each band
/// named where it ends, and the ground right under it (short of every gun) called out.
fn reach_labels(ui: &mut Ui, field: &Field, u: &UnitInstance, alpha: f32) {
    let bp = bp_of(field.blueprints, u);
    let c = Vec3::from(u.prev_pos)
        .lerp(Vec3::from(u.pos), alpha)
        .truncate();
    // Only while the rings are readable: a titan's reach is kilometres.
    let span = match (
        ground(ui, field, c),
        ground(ui, field, c + Vec2::X * 1000.0),
    ) {
        (Some(a), Some(b)) => a.distance(b),
        _ => return,
    };
    if span < 25.0 {
        return;
    }
    // Bands: the great bore, the others by name, farthest first; twin guns once.
    let mut marks: Vec<(f32, String, u32)> = Vec::new();
    let mut shortest = f32::MAX;
    // Its flak is left to the anti-air ring: only the guns that reach the ground.
    for w in bp
        .weapons
        .iter()
        .filter(|w| w.target_mask & !mc_data::cat::AIR != 0)
    {
        let (lo, hi) = (w.range_min.to_f32(), w.range_max.to_f32());
        let short = w
            .name
            .split_whitespace()
            .next()
            .unwrap_or(&w.name)
            .to_owned();
        let tone = crate::rings::Reach::of(w).tone();
        if marks.iter().any(|m| m.0 == hi) {
            continue;
        }
        marks.push((hi, format!("{short}  {}", metres(hi)), tone));
        if lo > 0.0 {
            marks.push((lo, format!("{short} dead zone  {}", metres(lo)), tone));
        }
        shortest = shortest.min(if lo > 0.0 { lo } else { 0.0 });
    }
    // Spread the tags round a little so near rings do not stack.
    marks.sort_by(|a, b| b.0.total_cmp(&a.0));
    let radii: Vec<f32> = marks.iter().map(|m| m.0).collect();
    let bearing = label_bearing(ui, field, c, &radii);
    for (i, (radius, text, tone)) in marks.iter().enumerate() {
        let a = bearing + (i as f32 - (marks.len() as f32 - 1.0) * 0.5) * 0.09;
        let room = label_room(ui);
        if let Some(g) =
            ground(ui, field, c + Vec2::from_angle(a) * *radius).filter(|g| room.contains(*g))
        {
            ui.disc(g, 3.0, rgb(*tone, 0.95));
            let w = ui.text_width(type_scale::MICRO, text) + 14.0;
            tag(ui, g + Vec2::new(w * 0.5 + 8.0, 0.0), text, *tone, 0.95);
        }
    }
    // Under it, short of every gun: only its feet and its flak.
    if shortest > 0.0 && shortest < f32::MAX {
        let a = bearing + std::f32::consts::PI;
        let room = label_room(ui);
        if let Some(g) = ground(ui, field, c + Vec2::from_angle(a) * shortest * 0.7)
            .filter(|g| room.contains(*g))
        {
            tag(
                ui,
                g,
                "Under its guns: feet and flak only",
                palette::WARN,
                0.9,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_great_bore_charges_then_recharges_then_is_ready() {
        let clock = BoreClock {
            charge_from: 100,
            charge_ticks: 60,
            reload_ticks: 450,
        };
        assert!(
            matches!(clock.state(130), BoreState::Charging(k, s) if (k - 0.5).abs() < 1e-3 && (s - 3.0).abs() < 1e-3)
        );
        // Fired at 160; the next charge may begin 390 ticks later.
        assert!(
            matches!(clock.state(160), BoreState::Recharging(k, s) if k == 0.0 && (s - 39.0).abs() < 1e-3)
        );
        assert_eq!(clock.state(549), BoreState::Recharging(389.0 / 390.0, 0.1));
        assert_eq!(clock.state(550), BoreState::Ready);
    }

    #[test]
    fn a_storm_spreads_like_the_sims() {
        let s = Strike {
            unit: 7,
            owner: 0,
            blueprint: BlueprintId(0),
            weapon: 1,
            target: Vec3::ZERO,
            on: None,
            radius: 440.0,
            charge_from: 0,
            charge_ticks: 60,
            struck: None,
            storm_ticks: 90,
        };
        assert!(matches!(s.phase(30, 0.0), Phase::Charging(k, _) if (k - 0.5).abs() < 1e-3));
        assert_eq!(s.phase(70, 0.0), Phase::Bolt);
        assert!(!s.over(60 + BOLT_PATIENCE));
        assert!(s.over(61 + BOLT_PATIENCE));
        let s = Strike {
            struck: Some((100, Vec3::ZERO)),
            ..s
        };
        // A fifth at once, all of it by the end (`titan::DischargeStorm::reach`).
        assert!(matches!(s.phase(100, 0.0), Phase::Storm(r, _, _) if (r - 88.0).abs() < 1e-3));
        assert!(
            matches!(s.phase(145, 0.0), Phase::Storm(r, _, _) if (r - 440.0 * (0.2 + 0.8 * 0.75)).abs() < 1e-2)
        );
        assert!(matches!(s.phase(190, 0.0), Phase::Spent(k) if k == 1.0));
        assert!(s.over(190 + STORM_FADE + 1));
    }
}

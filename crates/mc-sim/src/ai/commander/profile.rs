//! What each unit can do, read from its blueprint (`docs/AI_COMMANDER.md`, "Profiles").
//!
//! Nothing here names a unit or a race: a new unit, a T5 included, is understood the
//! day its blueprint is written. Built once per think from the blueprints (a few
//! microseconds for the whole roster), never stored in `State`.
use super::super::ENERGY_PER_MASS;
use mc_core::{Fx, TICKS_PER_SECOND};
use mc_data::strategic::StrategicKind;
use mc_data::{cat, BlueprintId, Blueprints, MoveLayer, UnitBlueprint, Weapon};

/// What a weapon can be aimed at.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(usize)]
pub(in crate::ai) enum Target {
    Land = 0,
    Structure = 1,
    Air = 2,
    /// Ships afloat.
    Surface = 3,
    /// Dived submarines: torpedoes only.
    Submerged = 4,
    /// Capital spacecraft: hit by anti-air and by guns made for them.
    Space = 5,
}

pub(in crate::ai) const TARGETS: usize = 6;
pub(in crate::ai) const ALL_TARGETS: [Target; TARGETS] = [
    Target::Land,
    Target::Structure,
    Target::Air,
    Target::Surface,
    Target::Submerged,
    Target::Space,
];

/// How a unit gets about.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(in crate::ai) enum Domain {
    Static,
    Land,
    /// Hover or amphibious: crosses water.
    Hover,
    Naval,
    /// A ship that dives.
    Sub,
    Air,
    /// A capital spacecraft: flies, often warps.
    Space,
}

/// Roles, as bits, derived from what a unit can do.
pub(in crate::ai) mod role {
    /// Fights other armies on the ground.
    pub(in crate::ai) const LINE: u32 = 1 << 0;
    /// Fast and armed: soft targets and engineers.
    pub(in crate::ai) const RAIDER: u32 = 1 << 1;
    /// Shells from beyond what it is shelling can shoot back.
    pub(in crate::ai) const ARTILLERY: u32 = 1 << 2;
    pub(in crate::ai) const ANTI_AIR: u32 = 1 << 3;
    pub(in crate::ai) const ANTI_SHIP: u32 = 1 << 4;
    /// Hits dived submarines.
    pub(in crate::ai) const HUNTER: u32 = 1 << 5;
    /// Carries land units.
    pub(in crate::ai) const TRANSPORT: u32 = 1 << 6;
    pub(in crate::ai) const SCOUT: u32 = 1 << 7;
    /// Radar or sonar on the move.
    pub(in crate::ai) const SENSOR: u32 = 1 << 8;
    /// Bombs or shells structures well.
    pub(in crate::ai) const SIEGE: u32 = 1 << 9;
    /// A nuke launcher.
    pub(in crate::ai) const STRATEGIC: u32 = 1 << 10;
    /// Shoots down strategic missiles.
    pub(in crate::ai) const INTERCEPTOR: u32 = 1 << 11;
    pub(in crate::ai) const SHIELD: u32 = 1 << 12;
    pub(in crate::ai) const BUILDER: u32 = 1 << 13;
    pub(in crate::ai) const ECONOMY: u32 = 1 << 14;
    /// A structure that shells from many kilometres.
    pub(in crate::ai) const MAP_GUN: u32 = 1 << 15;
    /// Raised on a lot of its own: a big commitment, judged against income.
    pub(in crate::ai) const PROJECT: u32 = 1 << 16;
    /// Jumps through warp.
    pub(in crate::ai) const WARP: u32 = 1 << 17;
    pub(in crate::ai) const FACTORY: u32 = 1 << 18;
    /// A static gun guarding its ground.
    pub(in crate::ai) const DEFENSE: u32 = 1 << 19;
    /// An aircraft that strikes the ground.
    pub(in crate::ai) const STRIKE: u32 = 1 << 20;
    /// Hits spaceships hard.
    pub(in crate::ai) const ANTI_SPACE: u32 = 1 << 21;
}

/// A shot farther than this many times an ordinary defence's reach is a map gun's.
const MAP_GUN_RANGE: Fx = Fx::from_int(4000);
/// Faster than this (m/s) and armed: a raider.
const RAIDER_SPEED: Fx = Fx::from_int(40);

/// One weapon, summarised: what it reaches, how hard, between which distances.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::ai) struct Gun {
    /// Bit `t` set: it can be aimed at `Target` number `t`.
    pub at: u8,
    pub dps: Fx,
    pub min: Fx,
    pub max: Fx,
    /// Metres a second its shot flies; zero for a hit that lands at once (hitscan,
    /// beams) or a shot that steers itself onto its target (guided).
    pub flight: Fx,
    /// How wide its hit lands: splash or proximity fuse.
    pub spread: Fx,
}

#[derive(Clone, Debug, Default)]
pub(in crate::ai) struct Profile {
    pub guns: Vec<Gun>,
    pub id: BlueprintId,
    /// Mass plus energy at `ENERGY_PER_MASS`.
    pub cost: Fx,
    pub mass: Fx,
    pub tech: u8,
    /// Damage a second it puts on each target class.
    pub dps: [Fx; TARGETS],
    /// Its longest reach at each target class (zero: cannot).
    pub reach: [Fx; TARGETS],
    /// Share of its damage that splashes (0..1): good against crowds.
    pub splash: Fx,
    /// Health plus shield.
    pub ehp: Fx,
    pub speed: Fx,
    /// Hull radius: how big a mark it makes.
    pub radius: Fx,
    pub domain: Option<Domain>,
    /// What target class it is.
    pub is: Option<Target>,
    pub roles: u32,
    /// Lift room, and the room it takes in a hold.
    pub carry: u16,
    pub room: u16,
}

impl Profile {
    pub(in crate::ai) fn has(&self, roles: u32) -> bool {
        self.roles & roles == roles
    }

    /// Any weapon at all that reaches `t`.
    pub(in crate::ai) fn hits(&self, t: Target) -> bool {
        self.dps[t as usize] > Fx::ZERO
    }

    pub(in crate::ai) fn armed(&self) -> bool {
        self.dps.iter().any(|d| *d > Fx::ZERO)
    }

    /// Damage a second that lands on `target` (of class `t`) from `d` metres away. A
    /// slow shot that is not steered misses a moving target by however far it moved
    /// while the shot flew, less the shot's spread and the target's size.
    pub(in crate::ai) fn dps_at(&self, t: Target, d: Fx, target: &Profile) -> Fx {
        self.guns
            .iter()
            .filter(|g| g.at >> t as u8 & 1 != 0 && g.min <= d && d <= g.max)
            .map(|g| g.dps * hit_share(g, d, target))
            .sum()
    }

    pub(in crate::ai) fn mobile(&self) -> bool {
        !matches!(self.domain, None | Some(Domain::Static))
    }
}

/// The class a unit of `bp` is when shot at.
pub(in crate::ai) fn target_class(bp: &UnitBlueprint) -> Option<Target> {
    if bp.is_structure() {
        return Some(Target::Structure);
    }
    Some(match bp.motion?.layer {
        MoveLayer::Air if bp.has(cat::SPACE) => Target::Space,
        MoveLayer::Air => Target::Air,
        MoveLayer::Naval if bp.dive.is_some() => Target::Submerged,
        MoveLayer::Naval => Target::Surface,
        MoveLayer::Land | MoveLayer::Amphibious | MoveLayer::Hover => Target::Land,
    })
}

fn domain(bp: &UnitBlueprint) -> Option<Domain> {
    if bp.is_structure() {
        return Some(Domain::Static);
    }
    let m = bp.motion?;
    Some(match m.layer {
        MoveLayer::Air if bp.has(cat::SPACE) || bp.is_capital_ship() => Domain::Space,
        MoveLayer::Air => Domain::Air,
        MoveLayer::Naval if bp.dive.is_some() => Domain::Sub,
        MoveLayer::Naval => Domain::Naval,
        MoveLayer::Hover | MoveLayer::Amphibious => Domain::Hover,
        MoveLayer::Land => Domain::Land,
    })
}

/// Whether `w` can be aimed at targets of class `t`.
fn weapon_reaches(w: &Weapon, t: Target) -> bool {
    if w.intercepts {
        return false;
    }
    match t {
        Target::Land => w.target_mask & cat::LAND != 0 && !w.torpedo,
        Target::Structure => w.target_mask & cat::STRUCTURE != 0 && !w.torpedo,
        Target::Air => w.target_mask & cat::AIR != 0 && !w.torpedo,
        Target::Surface => w.target_mask & cat::NAVAL != 0,
        Target::Submerged => w.torpedo && w.target_mask & cat::NAVAL != 0,
        Target::Space => w.target_mask & (cat::AIR | cat::SPACE) != 0 && !w.torpedo,
    }
}

/// Damage a second of one weapon.
fn weapon_dps(w: &Weapon) -> Fx {
    let shots = w.salvo.max(1) as i32 * w.salvo_batch.max(1) as i32;
    w.damage * shots * TICKS_PER_SECOND as i32 / w.reload_ticks.max(1) as i32
}

impl Profile {
    pub(in crate::ai) fn of(bp: &UnitBlueprint) -> Profile {
        let mut p = Profile {
            id: bp.id,
            cost: bp.cost_mass + bp.cost_energy / ENERGY_PER_MASS,
            mass: bp.cost_mass,
            tech: bp.tech,
            ehp: bp.health + bp.shield.as_ref().map_or(Fx::ZERO, |s| s.health),
            speed: bp.motion.map_or(Fx::ZERO, |m| m.speed),
            radius: bp.radius,
            domain: domain(bp),
            is: target_class(bp),
            carry: bp.transport.map_or(0, |t| t.capacity),
            room: bp.cargo_room().unwrap_or(0),
            ..Profile::default()
        };
        let mut splash_dps = Fx::ZERO;
        let mut total = Fx::ZERO;
        for w in &bp.weapons {
            let dps = weapon_dps(w);
            let mut at = 0u8;
            for t in ALL_TARGETS {
                if weapon_reaches(w, t) {
                    at |= 1 << t as u8;
                    p.dps[t as usize] += dps;
                    p.reach[t as usize] = p.reach[t as usize].max(w.range_max);
                }
            }
            if at != 0 {
                let sure = w.hitscan || w.beam || w.guided || w.torpedo;
                p.guns.push(Gun {
                    at,
                    dps,
                    min: w.range_min,
                    max: w.range_max,
                    flight: if sure { Fx::ZERO } else { w.projectile_speed },
                    spread: w.splash.max(w.proximity),
                });
            }
            total += dps;
            if w.splash > Fx::from_int(4) || w.flak {
                splash_dps += dps;
            }
        }
        if total > Fx::ZERO {
            p.splash = splash_dps / total;
        }
        p.roles = roles(bp, &p);
        p
    }
}

fn roles(bp: &UnitBlueprint, p: &Profile) -> u32 {
    let mut r = 0;
    let mobile = p.mobile();
    let ground = p.hits(Target::Land);
    if bp.has(cat::COMMANDER) {
        return role::BUILDER;
    }
    if bp.builder.is_some() {
        r |= if bp.is_structure() {
            if bp.has(cat::FACTORY) {
                role::FACTORY
            } else {
                0
            }
        } else {
            role::BUILDER
        };
    }
    if bp.economy.mass_income > Fx::ZERO || bp.economy.energy_income > Fx::ZERO || bp.mine.is_some()
    {
        r |= role::ECONOMY;
    }
    if bp.is_site_built_unit() {
        r |= role::PROJECT;
    }
    if bp.warp.is_some() {
        r |= role::WARP;
    }
    if p.carry > 0 {
        r |= role::TRANSPORT;
    }
    if bp.has(cat::SCOUT) {
        r |= role::SCOUT;
    }
    if mobile && (bp.radar > Fx::ZERO || bp.sonar > Fx::ZERO) && !p.armed() {
        r |= role::SENSOR;
    }
    if bp.shield.is_some() {
        r |= role::SHIELD;
    }
    match bp.strategic.as_ref().map(|s| s.kind) {
        Some(StrategicKind::Nuke) => r |= role::STRATEGIC | role::PROJECT,
        Some(StrategicKind::Interceptor) => r |= role::INTERCEPTOR,
        None => {}
    }
    let longest = p.reach.iter().copied().max().unwrap_or(Fx::ZERO);
    if !mobile && longest >= MAP_GUN_RANGE {
        r |= role::MAP_GUN | role::PROJECT;
    } else if !mobile && p.armed() && bp.builder.is_none() {
        r |= role::DEFENSE;
    }
    if p.hits(Target::Air)
        && p.dps[Target::Air as usize] * 2 >= p.dps.iter().copied().max().unwrap_or(Fx::ZERO)
    {
        r |= role::ANTI_AIR;
    }
    if p.dps[Target::Space as usize] > p.dps[Target::Air as usize] {
        r |= role::ANTI_SPACE;
    }
    if p.domain == Some(Domain::Air)
        && (p.hits(Target::Land) || p.hits(Target::Structure))
        && !p.has(role::ANTI_AIR)
    {
        r |= role::STRIKE;
    }
    if p.hits(Target::Surface) {
        r |= role::ANTI_SHIP;
    }
    if p.hits(Target::Submerged) {
        r |= role::HUNTER;
    }
    if mobile && bp.has(cat::ARTILLERY) {
        r |= role::ARTILLERY;
    }
    if mobile
        && (bp.has(cat::ARTILLERY)
            || p.dps[Target::Structure as usize] > p.dps[Target::Land as usize] * 3 / 2)
    {
        r |= role::SIEGE;
    }
    // A gun that can only shoot up at spacecraft (the Regency's Spire) is no line unit: it
    // never marches with the army, and stays where it was made, guarding the sky over it.
    let fights_the_ground =
        !p.armed() || p.hits(Target::Land) || p.hits(Target::Structure) || p.hits(Target::Surface);
    if mobile && ground && fights_the_ground && !bp.has(cat::ARTILLERY) && !bp.has(cat::ENGINEER) {
        if matches!(p.domain, Some(Domain::Land | Domain::Hover)) {
            r |= role::LINE;
        }
        // A scout is fast and armed, but it scouts.
        if p.speed >= RAIDER_SPEED
            && matches!(p.domain, Some(Domain::Land | Domain::Hover))
            && !bp.has(cat::SCOUT)
        {
            r |= role::RAIDER;
        }
    }
    r
}

/// Share of shots of `g` fired from `d` metres that land on `target`.
fn hit_share(g: &Gun, d: Fx, target: &Profile) -> Fx {
    if g.flight == Fx::ZERO || target.speed == Fx::ZERO {
        return Fx::ONE;
    }
    // Targets dodge only part of the time: they turn, stop and bunch.
    let drift = target.speed * d / g.flight.max(Fx::ONE) / 2;
    let mark = g.spread + target.radius + Fx::from_int(4);
    if drift <= mark {
        Fx::ONE
    } else {
        (mark / drift).max(Fx::ratio(1, 10))
    }
}

/// Every blueprint's profile, indexed by its id.
pub(in crate::ai) struct Profiles {
    by_id: Vec<Profile>,
}

impl Profiles {
    pub(in crate::ai) fn build(blueprints: &Blueprints) -> Profiles {
        Profiles {
            by_id: blueprints.units.iter().map(Profile::of).collect(),
        }
    }

    pub(in crate::ai) fn get(&self, id: BlueprintId) -> &Profile {
        &self.by_id[id.0 as usize]
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;

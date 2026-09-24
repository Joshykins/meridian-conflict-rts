//! Range rings: what the selection reaches, as circles on the ground. One
//! colour per kind of reach; a weapon's dead zone is the dashed inner circle.
//! The renderer merges the rings of a kind, so an army shows one outline.
//!
//! A unit with guns of one kind but different reach shows each reach: the
//! farthest as the full line, the shorter ones in the same colour as a finer,
//! fainter solid line (dashes stay the dead zone's alone). `ranges.wgsl` reads
//! the rank from the group, and each rank merges on its own.
//!
//! Rings are honest about the sim: a weapon or builder reaches whatever has
//! its edge inside the circle (`combat.rs` measures to the target's hull).

use mc_data::{cat, Blueprints, Trajectory, UnitBlueprint, Weapon};
use mc_render::{RangeRing, MAX_RANGES};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK};

/// A kind of reach. Each is a colour, and a group the renderer merges.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Reach {
    Direct,
    Indirect,
    Missile,
    AntiAir,
    Torpedo,
    Radar,
    Build,
    /// A reclaimer tower's beam.
    Reclaim,
    /// A shield generator's dome. Hull wraps do not draw a ring.
    Shield,
    /// Hostile missiles this unit can burn. `ranges.wgsl` draws group 9 thinner.
    AntiMissile,
    /// Sonar: where dived hulls are found.
    Sonar,
    /// An airbase's reach: the ground it calls idle aircraft home from and may guard.
    Airbase,
}

impl Reach {
    pub fn label(self) -> &'static str {
        match self {
            Reach::Direct => "DIRECT FIRE",
            Reach::Indirect => "ARTILLERY",
            Reach::Missile => "MISSILES",
            Reach::AntiAir => "ANTI-AIR",
            Reach::Torpedo => "TORPEDOES",
            Reach::Radar => "RADAR",
            Reach::Build => "BUILD",
            Reach::Reclaim => "RECLAIM",
            Reach::Shield => "SHIELD",
            Reach::AntiMissile => "ANTI-MISSILE",
            Reach::Sonar => "SONAR",
            Reach::Airbase => "AIRBASE",
        }
    }

    /// sRGB, for the HUD's key.
    pub fn tone(self) -> u32 {
        match self {
            Reach::Direct => 0xFF4B3A,
            Reach::Indirect => 0xFFD23C,
            Reach::Missile => 0xFF8A1E,
            Reach::AntiAir => 0x5CC8FF,
            Reach::Torpedo => 0x39E05A,
            Reach::Radar => 0x4F7DFF,
            Reach::Build => 0xE6F2F0,
            Reach::Reclaim => 0xFFB38A,
            Reach::Shield => 0x7AD4FF,
            Reach::AntiMissile => 0xFF7A1A,
            Reach::Sonar => 0x1D7A3A,
            Reach::Airbase => 0x9DB4FF,
        }
    }

    pub fn linear(self) -> [f32; 3] {
        let c = self.tone();
        [16, 8, 0].map(|shift| (((c >> shift) & 0xFF) as f32 / 255.0).powf(2.2))
    }

    /// What a weapon's ring says about it: what it can hit first, then how the shot flies.
    pub fn of(weapon: &Weapon) -> Reach {
        let hits = |mask: u32| weapon.target_mask & mask != 0;
        if hits(cat::AIR) && !hits(cat::LAND | cat::STRUCTURE | cat::NAVAL) {
            Reach::AntiAir
        } else if hits(cat::NAVAL) && !hits(cat::LAND | cat::STRUCTURE | cat::AIR) {
            Reach::Torpedo
        } else if weapon.missile {
            Reach::Missile
        } else if weapon.trajectory == Trajectory::Ballistic {
            Reach::Indirect
        } else {
            Reach::Direct
        }
    }
}

/// A ring's group holds its kind in the low byte and its rank above.
pub const RANK_SHIFT: u32 = 8;
/// Ranks past the last are all drawn alike.
pub const RANKS: u8 = 3;

/// The kind of a ring's group.
pub fn kind_of(group: u32) -> u32 {
    group & ((1 << RANK_SHIFT) - 1)
}

/// Where a gun that cannot turn all the way round can shoot: a wedge centred on
/// the hull's nose, or its tail for a rear gun, as `combat.rs` limits it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub aft: bool,
    /// Where the wedge is centred, radians off the nose, to the left (`Weapon::facing`).
    pub turn: f32,
    /// Radians either side of the centre line.
    pub half: f32,
}

impl Arc {
    fn of(weapon: &Weapon) -> Option<Arc> {
        (weapon.half_arc < 0x8000).then(|| Arc {
            aft: weapon.rear || weapon.facing.0 == 0x8000,
            turn: weapon.facing.0 as i16 as f32 / 32768.0 * std::f32::consts::PI,
            half: weapon.half_arc as f32 / 32768.0 * std::f32::consts::PI,
        })
    }

    /// The whole arc in degrees, and which way it faces: "150\u{b0} Aft".
    pub fn label(self) -> String {
        format!(
            "{:.0}\u{b0} {}",
            self.half.to_degrees() * 2.0,
            match self.turn.to_degrees().round() as i32 {
                _ if self.aft => "Aft",
                45..=135 => "Port",
                -135..=-45 => "Starboard",
                _ => "Fore",
            }
        )
    }
}

/// One circle pair a blueprint puts on the ground, and what the HUD calls it.
#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    pub reach: Reach,
    /// Zero for the farthest ring of its kind on this blueprint, one for the next, and so on.
    pub rank: u8,
    /// The dead zone's radius, zero without one.
    pub inner: f32,
    pub outer: f32,
    /// `None`: all the way round.
    pub arc: Option<Arc>,
    pub name: String,
}

impl Projection {
    pub fn group(&self) -> u32 {
        self.reach as u32 | (self.rank.min(RANKS - 1) as u32) << RANK_SHIFT
    }

    fn round(reach: Reach, outer: f32, name: &str) -> Projection {
        Projection { reach, rank: 0, inner: 0.0, outer, arc: None, name: name.into() }
    }
}

/// Every ring `bp` projects, by kind and then farthest first. Twin weapons are one ring.
pub fn projections(bp: &UnitBlueprint) -> Vec<Projection> {
    let mut all: Vec<Projection> = bp
        .weapons
        .iter()
        .map(|w| Projection {
            reach: Reach::of(w),
            rank: 0,
            inner: w.range_min.to_f32(),
            outer: w.range_max.to_f32(),
            arc: Arc::of(w),
            name: w.name.clone(),
        })
        .collect();
    all.push(Projection::round(Reach::Radar, bp.radar.to_f32(), "Radar"));
    all.push(Projection::round(Reach::Sonar, bp.sonar.to_f32(), "Sonar"));
    all.push(Projection::round(
        Reach::Airbase,
        bp.airbase.as_ref().map_or(0.0, |a| a.reach.to_f32()),
        "Base Reach",
    ));
    all.push(Projection::round(Reach::AntiMissile, bp.anti_missile.to_f32(), "Missile Defence"));
    // A factory builds inside itself: its builder has no range.
    all.push(Projection::round(
        Reach::Build,
        bp.builder.as_ref().map_or(0.0, |b| b.range.to_f32()),
        "Build Range",
    ));
    all.push(Projection::round(
        Reach::Reclaim,
        bp.reclaimer.map_or(0.0, |r| r.range.to_f32()).max(bp.drone_radius.to_f32()),
        "Reclaim Reach",
    ));
    all.push(Projection::round(
        Reach::Shield,
        bp.shield
            .filter(|s| s.is_dome())
            .map_or(0.0, |s| s.radius.to_f32()),
        "Shield Dome",
    ));
    all.retain(|p| p.outer > 0.0);
    // Farthest first; of two alike, the one that reaches round first.
    let width = |p: &Projection| p.arc.map_or(f32::MAX, |a| a.half);
    all.sort_by(|a, b| {
        a.reach
            .cmp(&b.reach)
            .then(b.outer.total_cmp(&a.outer))
            .then(a.inner.total_cmp(&b.inner))
            .then(width(b).total_cmp(&width(a)))
    });
    let mut out: Vec<Projection> = Vec::new();
    for mut p in all {
        if let Some(same) = out
            .iter_mut()
            .find(|o| o.reach == p.reach && o.inner == p.inner && o.outer == p.outer && o.arc == p.arc)
        {
            if !same.name.split(" \u{b7} ").any(|n| n == p.name) {
                same.name = format!("{} \u{b7} {}", same.name, p.name);
            }
            continue;
        }
        p.rank = match out.last() {
            Some(o) if o.reach == p.reach => o.rank + u8::from(o.outer > p.outer),
            _ => 0,
        };
        out.push(p);
    }
    out
}

/// One circle pair of a blueprint, as the renderer takes it: kind, group, dead
/// zone (zero without one), reach, the arc's centre off the nose and half-width
/// in radians (a half-width of pi or more is all the way round).
type Span = (Reach, u32, f32, f32, f32, f32);

fn spans(bp: &UnitBlueprint) -> Vec<Span> {
    projections(bp)
        .iter()
        .map(|p| {
            let (turn, half) = p.arc.map_or((0.0, FULL_ARC), |a| {
                (if a.aft { std::f32::consts::PI } else { a.turn }, a.half)
            });
            (p.reach, p.group(), p.inner, p.outer, turn, half)
        })
        .collect()
}

/// `RangeRing::half_arc` of a ring that goes all the way round.
const FULL_ARC: f32 = 4.0;

/// Directions asked about when deciding whether a ring shows at all.
const PROBES: usize = 32;

/// Which rings lie wholly inside what others of their kind reach: those are not
/// worth drawing, though they still mask. In a blob of tanks that leaves the few
/// on its edge.
///
/// A ring counts as hidden only when every probe is covered by more than the
/// arc to the next probe, so what falls between two probes is covered too. That
/// margin is also why the answer holds until the next tick moves the units.
fn hidden(rings: &[RangeRing]) -> Vec<bool> {
    // A handful of rings costs the GPU nothing.
    if rings.len() <= 16 {
        return vec![false; rings.len()];
    }
    let dirs: [[f32; 2]; PROBES] = std::array::from_fn(|k| {
        let a = k as f32 / PROBES as f32 * std::f32::consts::TAU;
        [a.cos(), a.sin()]
    });
    let buried = |i: usize, radius: f32| {
        let ring = &rings[i];
        let margin = radius * std::f32::consts::PI / PROBES as f32;
        dirs.iter().all(|d| {
            let p = [
                ring.center[0] + d[0] * radius,
                ring.center[1] + d[1] * radius,
            ];
            rings.iter().enumerate().any(|(j, other)| {
                let (near, far) = (other.inner + margin, other.outer - margin);
                let d2 = (p[0] - other.center[0]).powi(2) + (p[1] - other.center[1]).powi(2);
                j != i
                    && other.group == ring.group
                    && other.half_arc >= std::f32::consts::PI
                    && far > 0.0
                    && d2 >= near * near
                    && d2 <= far * far
            })
        })
    };
    (0..rings.len())
        .map(|i| {
            // A wedge's edges are not probed: it is always drawn.
            rings[i].half_arc >= std::f32::consts::PI
                && buried(i, rings[i].outer)
                && (rings[i].inner <= 0.0 || buried(i, rings[i].inner))
        })
        .collect()
}

pub struct Rings {
    /// By blueprint id.
    spans: Vec<Vec<Span>>,
    /// `hidden` of the rings last collected, and which units those were.
    hidden: Vec<bool>,
    hidden_of: u64,
}

impl Rings {
    pub fn new(blueprints: &Blueprints) -> Rings {
        Rings {
            spans: blueprints.units.iter().map(spans).collect(),
            hidden: Vec::new(),
            hidden_of: 0,
        }
    }

    /// The rings of `units` at `alpha` between the last two ticks, where the renderer draws
    /// them, and how many from the front are to be drawn (`FrameInput::ranges_drawn`).
    /// `fresh` says the units have moved since the last call: a new tick arrived.
    pub fn collect<'a>(
        &mut self,
        units: impl Iterator<Item = &'a UnitInstance>,
        alpha: f32,
        fresh: bool,
    ) -> (Vec<RangeRing>, usize) {
        let mut out = Vec::new();
        let mut of = 0xcbf29ce484222325u64;
        // What is being placed follows the pointer, not the ticks: it is always drawn.
        let mut placed = 0;
        for u in units {
            let Some(spans) = self.spans.get(u.blueprint as usize) else {
                continue;
            };
            if u.owner_flags & KIND_WRECK != 0 || out.len() + spans.len() > MAX_RANGES {
                continue;
            }
            of = (of ^ ((u.unit_id as u64) << 32 | u.blueprint as u64)).wrapping_mul(0x100000001b3);
            if u.owner_flags & KIND_GHOST != 0 {
                placed = out.len() + spans.len();
            }
            let center = [
                u.prev_pos[0] + (u.pos[0] - u.prev_pos[0]) * alpha,
                u.prev_pos[1] + (u.pos[1] - u.prev_pos[1]) * alpha,
            ];
            // Arcs turn with the hull, the short way round between ticks.
            let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let heading = u.prev_heading + turn * alpha;
            out.extend(spans.iter().map(|&(reach, group, inner, outer, off, half)| RangeRing {
                center,
                inner,
                outer,
                color: reach.linear(),
                group,
                facing: heading + off,
                half_arc: half,
                _pad: [0.0; 2],
            }));
        }
        // Sorting out what is hidden is the costly part, and only a tick or another selection changes it.
        if fresh || of != self.hidden_of || self.hidden.len() != out.len() {
            self.hidden = hidden(&out);
            self.hidden_of = of;
        }
        self.hidden[..placed].fill(false);
        let (mut shown, masks): (Vec<_>, Vec<_>) = out
            .iter()
            .zip(&self.hidden)
            .partition(|(_, hidden)| !**hidden);
        let drawn = shown.len();
        shown.extend(masks);
        (shown.into_iter().map(|(ring, _)| *ring).collect(), drawn)
    }

    /// The HUD's key to `rings`: each kind and rank drawn, with its farthest reach and that ring's dead zone.
    pub fn key(rings: &[RangeRing]) -> Vec<(Reach, u8, f32, f32)> {
        const ALL: [Reach; 12] = [
            Reach::Direct,
            Reach::Indirect,
            Reach::Missile,
            Reach::AntiAir,
            Reach::AntiMissile,
            Reach::Torpedo,
            Reach::Radar,
            Reach::Sonar,
            Reach::Airbase,
            Reach::Build,
            Reach::Reclaim,
            Reach::Shield,
        ];
        ALL.into_iter()
            .flat_map(|reach| (0..RANKS).map(move |rank| (reach, rank)))
            .filter_map(|(reach, rank)| {
                let group = reach as u32 | (rank as u32) << RANK_SHIFT;
                let farthest = rings
                    .iter()
                    .filter(|r| r.group == group)
                    .max_by(|a, b| a.outer.total_cmp(&b.outer))?;
                Some((reach, rank, farthest.inner, farthest.outer))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blueprints() -> Blueprints {
        Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap()
    }

    /// Kind, rank, dead zone and reach of each ring.
    fn of(b: &Blueprints, key: &str) -> Vec<(Reach, u8, f32, f32)> {
        projections(b.unit(b.id_of(key).unwrap()))
            .into_iter()
            .map(|p| (p.reach, p.rank, p.inner, p.outer))
            .collect()
    }

    #[test]
    fn rings_follow_the_data() {
        let b = blueprints();
        assert_eq!(
            of(&b, "aster_t1_scout"),
            vec![(Reach::Direct, 0, 0.0, 140.0), (Reach::Radar, 0, 0.0, 800.0)]
        );
        assert_eq!(of(&b, "aster_t1_tank"), vec![(Reach::Direct, 0, 0.0, 180.0)]);
        // A howitzer has a dead zone, a missile rack is its own kind.
        assert_eq!(
            of(&b, "aster_t1_artillery"),
            vec![(Reach::Indirect, 0, 60.0, 320.0)]
        );
        assert_eq!(
            of(&b, "aster_t2_missile"),
            vec![(Reach::Missile, 0, 120.0, 620.0)]
        );
        // The Paladin's twin projectors are one ring; its shin tubes another.
        assert_eq!(
            of(&b, "aster_t3_assault_bot"),
            vec![(Reach::Direct, 0, 0.0, 280.0), (Reach::Torpedo, 0, 0.0, 360.0)]
        );
        assert_eq!(
            of(&b, "aster_t2_support"),
            vec![
                (Reach::Radar, 0, 0.0, 2800.0),
                (Reach::AntiMissile, 0, 0.0, 300.0),
            ]
        );
        assert_eq!(of(&b, "aster_t1_radar"), vec![(Reach::Radar, 0, 0.0, 2000.0)]);
        assert_eq!(of(&b, "aster_t2_radar"), vec![(Reach::Radar, 0, 0.0, 4000.0)]);
        assert_eq!(of(&b, "aster_t3_radar"), vec![(Reach::Radar, 0, 0.0, 8000.0)]);
        assert_eq!(of(&b, "aster_t2_shield"), vec![(Reach::Shield, 0, 0.0, 100.0)]);
        assert_eq!(of(&b, "aster_t3_shield"), vec![(Reach::Shield, 0, 0.0, 180.0)]);
        assert_eq!(
            of(&b, "aster_commander"),
            vec![(Reach::Direct, 0, 0.0, 250.0), (Reach::Build, 0, 0.0, 70.0)]
        );
        // Factories build inside themselves; a reactor reaches nothing.
        assert!(of(&b, "aster_t1_land_factory").is_empty());
        assert!(of(&b, "aster_t1_air_factory").is_empty());
        assert!(of(&b, "aster_t1_power").is_empty());
        assert_eq!(
            of(&b, "aster_t1_interceptor"),
            vec![(Reach::AntiAir, 0, 0.0, 220.0)]
        );
        assert_eq!(
            of(&b, "aster_t1_bomber"),
            vec![(Reach::Indirect, 0, 0.0, 320.0)]
        );
    }

    /// A second gun of the same kind with less reach is its own ring, one rank down.
    #[test]
    fn shorter_guns_of_a_kind_rank_below_the_farthest() {
        let b = blueprints();
        let mut bp = b.unit(b.id_of("aster_t1_tank").unwrap()).clone();
        let scout = b.unit(b.id_of("aster_t1_scout").unwrap());
        bp.weapons.push(scout.weapons[0].clone());
        bp.weapons.push(bp.weapons[0].clone());
        let all = projections(&bp);
        let got: Vec<_> = all.iter().map(|p| (p.reach, p.rank, p.inner, p.outer)).collect();
        assert_eq!(
            got,
            vec![(Reach::Direct, 0, 0.0, 180.0), (Reach::Direct, 1, 0.0, 140.0)]
        );
        // The twin is not named twice; the ranks merge apart.
        assert!(!all[0].name.contains('\u{b7}'));
        assert_ne!(all[0].group(), all[1].group());
        assert_eq!(kind_of(all[1].group()), Reach::Direct as u32);
    }

    fn unit_at(x: f32, y: f32) -> UnitInstance {
        UnitInstance {
            prev_pos: [x, y, 0.0],
            prev_heading: 0.0,
            pos: [x, y, 0.0],
            heading: 0.0,
            blueprint: 0,
            owner_flags: 0,
            health: 1.0,
            build: 1.0,
            turret_yaw: 0.0,
            radius: 3.0,
            unit_id: 1,
            _pad: 0,
            gait: [0.0; 3],
            upgrade: 0.0,
            arm_pitch: [0.0; 4],
            prev_turret_yaw: 0.0,
            weld: [0.0; 3],
            recoil: 0.0,
            prev_recoil: 0.0,
            weld_first: 0,
            weld_count: 0,
            deploy: 0.0,
            prev_deploy: 0.0,
            _pad2: [0.0; 2],
            refit_modules: 0,
            _pad3: [0; 3],
            mount: [0.0; 4],
            spin_recoil: [0.0; 4],
        }
    }

    /// A gun that cannot turn all the way round projects a wedge: the Hellkite's tail gun faces aft.
    #[test]
    fn limited_guns_project_a_wedge() {
        let b = blueprints();
        let hellkite = projections(b.unit(b.id_of("aster_t2_fire_bomber").unwrap()));
        let tail = hellkite.iter().find(|p| p.name == "Tail AA").unwrap();
        let arc = tail.arc.unwrap();
        assert!(arc.aft && (arc.half.to_degrees() - 75.0).abs() < 0.5, "{arc:?}");
        assert_eq!(arc.label(), "150\u{b0} Aft");
        let dorsal = hellkite.iter().find(|p| p.name == "Dorsal AA").unwrap();
        assert_eq!((dorsal.arc, dorsal.rank), (None, 0));

        // The wedge turns with the hull: a unit facing north puts its aft arc south.
        let mut rings = Rings::new(&b);
        let bp = b.id_of("aster_t2_fire_bomber").unwrap().0 as u32;
        let north = std::f32::consts::FRAC_PI_2;
        let unit = UnitInstance { blueprint: bp, prev_heading: north, heading: north, ..unit_at(0.0, 0.0) };
        let (all, _) = rings.collect([&unit].into_iter(), 1.0, true);
        let aft = all.iter().find(|r| r.half_arc < std::f32::consts::PI).unwrap();
        assert!((aft.facing - (north + std::f32::consts::PI)).abs() < 1e-4);
        assert!(all.iter().filter(|r| r.half_arc >= std::f32::consts::PI).count() >= 2);
    }

    #[test]
    fn rings_sit_between_ticks_and_never_outgrow_the_renderer() {
        let b = blueprints();
        let mut rings = Rings::new(&b);
        let tank = b.id_of("aster_t1_tank").unwrap().0 as u32;
        let unit = UnitInstance {
            prev_pos: [10.0, 20.0, 0.0],
            prev_heading: 0.0,
            pos: [20.0, 40.0, 0.0],
            heading: 0.0,
            blueprint: tank,
            owner_flags: 0,
            health: 1.0,
            build: 1.0,
            turret_yaw: 0.0,
            radius: 3.0,
            unit_id: 1,
            _pad: 0,
            gait: [0.0; 3],
            upgrade: 0.0,
            arm_pitch: [0.0; 4],
            prev_turret_yaw: 0.0,
            weld: [0.0; 3],
            recoil: 0.0,
            prev_recoil: 0.0,
            weld_first: 0,
            weld_count: 0,
            deploy: 0.0,
            prev_deploy: 0.0,
            _pad2: [0.0; 2],
            refit_modules: 0,
            _pad3: [0; 3],
            mount: [0.0; 4],
            spin_recoil: [0.0; 4],
        };
        let (one, drawn) = rings.collect([&unit].into_iter(), 0.5, true);
        assert_eq!((one.len(), drawn), (1, 1));
        assert_eq!(one[0].center, [15.0, 30.0]);
        assert_eq!(Rings::key(&one), vec![(Reach::Direct, 0, 0.0, 180.0)]);

        let wreck = UnitInstance {
            owner_flags: KIND_WRECK,
            ..unit
        };
        assert!(rings.collect([&wreck].into_iter(), 0.5, true).0.is_empty());
        let army = vec![unit; MAX_RANGES + 40];
        assert_eq!(rings.collect(army.iter(), 0.0, true).0.len(), MAX_RANGES);
    }

    /// A block of tanks draws the rings on its edge and keeps the rest as masks.
    #[test]
    fn rings_buried_in_a_blob_are_not_drawn() {
        let b = blueprints();
        let mut rings = Rings::new(&b);
        let tank = b.id_of("aster_t1_tank").unwrap().0 as u32;
        let at = |x: f32, y: f32| UnitInstance {
            prev_pos: [x, y, 0.0],
            prev_heading: 0.0,
            pos: [x, y, 0.0],
            heading: 0.0,
            blueprint: tank,
            owner_flags: 0,
            health: 1.0,
            build: 1.0,
            turret_yaw: 0.0,
            radius: 3.0,
            unit_id: 1,
            _pad: 0,
            gait: [0.0; 3],
            upgrade: 0.0,
            arm_pitch: [0.0; 4],
            prev_turret_yaw: 0.0,
            weld: [0.0; 3],
            recoil: 0.0,
            prev_recoil: 0.0,
            weld_first: 0,
            weld_count: 0,
            deploy: 0.0,
            prev_deploy: 0.0,
            _pad2: [0.0; 2],
            refit_modules: 0,
            _pad3: [0; 3],
            mount: [0.0; 4],
            spin_recoil: [0.0; 4],
        };
        let block: Vec<UnitInstance> = (0..15)
            .flat_map(|x| {
                (0..15).map(move |y| at(1000.0 + x as f32 * 12.0, 1000.0 + y as f32 * 12.0))
            })
            .collect();
        let (all, drawn) = rings.collect(block.iter(), 1.0, true);
        assert_eq!(all.len(), 225);
        assert!(drawn >= 4 && drawn < 120, "{drawn} of 225 drawn");
        // The corners are on the outline, and the block's middle is not.
        let is_drawn = |x: f32, y: f32| all[..drawn].iter().any(|r| r.center == [x, y]);
        assert!(is_drawn(1000.0, 1000.0) && is_drawn(1168.0, 1168.0));
        assert!(!is_drawn(1084.0, 1084.0));

        // Spread out of each other's reach, every ring shows.
        let line: Vec<UnitInstance> = (0..40)
            .map(|i| at(1000.0 + i as f32 * 400.0, 1000.0))
            .collect();
        assert_eq!(rings.collect(line.iter(), 1.0, true).1, 40);

        // Between ticks the last answer stands; something being placed is drawn wherever it is.
        assert_eq!(rings.collect(block.iter(), 0.5, false).1, drawn);
        let ghost = UnitInstance {
            owner_flags: KIND_GHOST,
            unit_id: u32::MAX,
            ..at(1084.0, 1084.0)
        };
        let (with_ghost, _) = rings.collect([&ghost].into_iter().chain(block.iter()), 1.0, true);
        assert_eq!(with_ghost[0].center, [1084.0, 1084.0]);
    }
}

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

use mc_data::{cat, BlueprintId, Blueprints, Trajectory, UnitBlueprint, Weapon};
use mc_render::{RangeRing, MAX_RANGES};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK};

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
    /// A warp dampener's field: an enemy jump that ends inside it is snagged.
    Damper,
    /// A nanite repair field: friendly units inside it slowly mend.
    Repair,
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
            Reach::Damper => "WARP FIELD",
            Reach::Repair => "REPAIR FIELD",
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
            Reach::Reclaim => crate::hud::MASS,
            Reach::Shield => 0x7AD4FF,
            Reach::AntiMissile => 0xFF7A1A,
            Reach::Sonar => 0x1D7A3A,
            Reach::Damper => crate::hud::warp::DAMPER,
            // The nanites' violet.
            Reach::Repair => 0xB07CFF,
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
        } else if weapon.trajectory == Trajectory::Ballistic && !weapon.flat_fire {
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

/// Set in the group of the ring the HUD points at (a hovered weapon card): it merges
/// only with the same ring on other units, and `ranges.wgsl` draws it lit.
pub const FOCUS: u32 = 1 << 16;

/// The kind of a ring's group.
#[cfg(test)]
fn kind_of(group: u32) -> u32 {
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
        Projection {
            reach,
            rank: 0,
            inner: 0.0,
            outer,
            arc: None,
            name: name.into(),
        }
    }
}

/// Every ring `bp` projects, by kind and then farthest first. Twin weapons are one ring.
pub fn projections(bp: &UnitBlueprint) -> Vec<Projection> {
    let mut all: Vec<Projection> = bp
        .weapons
        .iter()
        .enumerate()
        .map(|(i, w)| Projection {
            reach: Reach::of(w),
            rank: 0,
            inner: w.range_min.to_f32(),
            outer: w.range_max.to_f32(),
            // A spinal gun is laid by turning the whole ship: it reaches all the way round.
            arc: if i == 0 && spinal(bp) {
                None
            } else {
                Arc::of(w)
            },
            name: w.name.clone(),
        })
        .collect();
    all.push(Projection::round(Reach::Radar, bp.radar.to_f32(), "Radar"));
    all.push(Projection::round(Reach::Sonar, bp.sonar.to_f32(), "Sonar"));
    all.push(Projection::round(
        Reach::AntiMissile,
        bp.anti_missile.to_f32(),
        "Missile Defence",
    ));
    // A factory builds inside itself: its builder has no range.
    all.push(Projection::round(
        Reach::Build,
        bp.builder.as_ref().map_or(0.0, |b| b.range.to_f32()),
        "Build Range",
    ));
    all.push(Projection::round(
        Reach::Reclaim,
        bp.reclaimer
            .map_or(0.0, |r| r.range.to_f32())
            .max(bp.drone_radius.to_f32()),
        "Reclaim Reach",
    ));
    all.push(Projection::round(
        Reach::Shield,
        bp.shield
            .filter(|s| s.is_dome())
            .map_or(0.0, |s| s.radius.to_f32()),
        "Shield Dome",
    ));
    all.push(Projection::round(
        Reach::Damper,
        bp.warp_damper.map_or(0.0, |d| d.radius.to_f32()),
        "Warp Field",
    ));
    all.push(Projection::round(
        Reach::Repair,
        bp.repair_field.map_or(0.0, |f| f.radius.to_f32()),
        "Repair Field",
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
        if let Some(same) = out.iter_mut().find(|o| {
            o.reach == p.reach && o.inner == p.inner && o.outer == p.outer && o.arc == p.arc
        }) {
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

/// A span whose dead zone depends on how high the hull is (`Rings::dives`).
#[derive(Clone, Copy, Debug)]
struct Dive {
    span: usize,
    /// The bore's height over the hull's origin (a spinal gun), or the house's pivot's.
    height: f32,
    /// A gun house's `depression` limit in degrees; `None`: a spinal gun.
    depression: Option<f32>,
}

/// Whether `bp`'s first weapon is a warship's spinal gun (`combat::spinal_gun`).
fn spinal(bp: &UnitBlueprint) -> bool {
    bp.weapons.first().is_some_and(|gun| {
        bp.is_capital_ship()
            && bp.transport.is_none()
            && gun.turret_turn == 0
            && !gun.guided
            && !gun.missile
            && !gun.vertical_launch
    })
}

/// Which of `all` (`projections(bp)`) is the ring of `bp`'s weapon `i`.
fn span_of(bp: &UnitBlueprint, all: &[Projection], i: usize) -> Option<usize> {
    let w = bp.weapons.get(i)?;
    let arc = if i == 0 && spinal(bp) {
        None
    } else {
        Arc::of(w)
    };
    all.iter().position(|p| {
        p.reach == Reach::of(w)
            && p.outer == w.range_max.to_f32()
            && p.inner == w.range_min.to_f32()
            && p.arc == arc
    })
}

/// Which of `projections(bp)` is the ring of `bp`'s weapon `i`.
pub fn projection_of(bp: &UnitBlueprint, i: usize) -> Option<usize> {
    span_of(bp, &projections(bp), i)
}

/// `bp`'s spans with a dead zone under the hull.
fn dives(bp: &UnitBlueprint) -> Vec<Dive> {
    let all = projections(bp);
    let mut out = Vec::new();
    for (i, w) in bp.weapons.iter().enumerate() {
        let depression = (w.depression.0 > 0).then(|| w.depression.to_radians_f32().to_degrees());
        if !(i == 0 && spinal(bp)) && depression.is_none() {
            continue;
        }
        let Some(span) = span_of(bp, &all, i) else {
            continue;
        };
        let height = w.pivot.unwrap_or(w.muzzle).z.to_f32();
        out.push(Dive {
            span,
            height: if depression.is_some() {
                height
            } else {
                w.muzzle.z.to_f32()
            },
            depression,
        });
    }
    out
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

/// A cover a side builds up post by post: placing a post shows the side's network of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cover {
    Radar,
    Sonar,
    AntiMissile,
}

impl Cover {
    /// A post with more than one cover is placed for the first of these it has.
    const ALL: [Cover; 3] = [Cover::AntiMissile, Cover::Sonar, Cover::Radar];

    pub fn reach(self) -> Reach {
        match self {
            Cover::Radar => Reach::Radar,
            Cover::Sonar => Reach::Sonar,
            Cover::AntiMissile => Reach::AntiMissile,
        }
    }

    /// How far `bp` covers of this kind if it is a post of it: a structure that has it.
    pub fn of(self, bp: &UnitBlueprint) -> Option<f32> {
        let r = match self {
            Cover::Radar => bp.radar,
            Cover::Sonar => bp.sonar,
            Cover::AntiMissile => bp.anti_missile,
        }
        .to_f32();
        (bp.is_structure() && r > 0.0).then_some(r)
    }

    /// The cover `bp` is a post of, and its reach.
    pub fn post(bp: &UnitBlueprint) -> Option<(Cover, f32)> {
        Cover::ALL.into_iter().find_map(|c| Some((c, c.of(bp)?)))
    }
}

/// While a post (`Cover::post`) is being placed: the ring of that cover for each of
/// `owner`'s standing posts of it, in its group, so the renderer merges them and the new
/// site's own ring into one outline of the side's cover. Empty while placing anything
/// else. `site` adds the new site's ring too, for a caller that draws no ghost of it (a
/// headless shot).
pub fn cover_network<'a>(
    blueprints: &Blueprints,
    placing: BlueprintId,
    site: Option<[f32; 2]>,
    owner: u8,
    units: impl Iterator<Item = &'a UnitInstance>,
) -> Vec<RangeRing> {
    let Some((cover, reach)) = Cover::post(blueprints.unit(placing)) else {
        return Vec::new();
    };
    let ring = |center: [f32; 2], outer: f32| RangeRing {
        center,
        inner: 0.0,
        outer,
        color: cover.reach().linear(),
        group: cover.reach() as u32,
        facing: 0.0,
        half_arc: FULL_ARC,
        _pad: [0.0; 2],
    };
    units
        .filter(|u| {
            u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
                && (u.owner_flags & 0xFF) as u8 == owner
                && u.build >= 1.0
        })
        .filter_map(|u| {
            let r = cover.of(blueprints.unit(BlueprintId(u.blueprint as u16)))?;
            Some(ring([u.pos[0], u.pos[1]], r))
        })
        .chain(site.map(|at| ring(at, reach)))
        .collect()
}

/// `extra` in front of `rings` (as `Rings::collect` gives them), all drawn.
pub fn prepend(rings: &mut Vec<RangeRing>, drawn: &mut usize, extra: Vec<RangeRing>) {
    *drawn += extra.len();
    rings.splice(0..0, extra);
}

pub struct Rings {
    /// By blueprint id.
    spans: Vec<Vec<Span>>,
    /// By blueprint id: the spans whose dead zone under the hull grows with its height, so
    /// is worked out per frame (a spinal gun, or a gun house with a `depression` limit).
    dives: Vec<Vec<Dive>>,
    /// `hidden` of the rings last collected, and which units those were.
    hidden: Vec<bool>,
    hidden_of: u64,
    /// The rings the HUD points at: a blueprint id and its projections (`projections`), as
    /// bits by index. They are drawn lit on every unit of the blueprint, every other ring dimmed.
    pub focus: Option<(u32, u64)>,
}

impl Rings {
    pub fn new(blueprints: &Blueprints) -> Rings {
        Rings {
            spans: blueprints.units.iter().map(spans).collect(),
            dives: blueprints.units.iter().map(dives).collect(),
            hidden: Vec::new(),
            hidden_of: 0,
            focus: None,
        }
    }

    /// The rings of `units` at `alpha` between the last two ticks, where the renderer draws
    /// them, and how many from the front are to be drawn (`FrameInput::ranges_drawn`).
    /// `fresh` says the units have moved since the last call: a new tick arrived.
    /// `ground` gives the ground's height at a point, for the dead zone under a warship.
    pub fn collect<'a>(
        &mut self,
        units: impl Iterator<Item = &'a UnitInstance>,
        alpha: f32,
        fresh: bool,
        ground: &dyn Fn([f32; 2]) -> f32,
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
            let first = out.len();
            let focus = self.focus;
            out.extend(spans.iter().enumerate().map(
                |(i, &(reach, group, inner, outer, off, half))| {
                    let (group, lit) = match focus {
                        Some((bp, spans)) if bp == u.blueprint && i < 64 && spans >> i & 1 != 0 => {
                            (group | FOCUS, 1.0)
                        }
                        Some(_) => (group, -1.0),
                        None => (group, 0.0),
                    };
                    // A ring of another kind with the same reach would lie right on top of this
                    // one (a frigate's turrets and its anti-air, both 700 m): it is drawn that
                    // many line-widths further out (ranges.wgsl `nudge`), so both show.
                    let under = spans[..i]
                        .iter()
                        .filter(|s| s.0 != reach && (s.3 - outer).abs() < 1.0)
                        .count();
                    RangeRing {
                        center,
                        inner,
                        outer,
                        color: reach.linear(),
                        group,
                        facing: heading + off,
                        half_arc: half,
                        _pad: [under as f32, lit],
                    }
                },
            ));
            let dives = self
                .dives
                .get(u.blueprint as usize)
                .map_or(&[][..], |d| &d[..]);
            if !dives.is_empty() {
                let z = u.prev_pos[2] + (u.pos[2] - u.prev_pos[2]) * alpha;
                let drop = z - ground(center);
                for dive in dives {
                    let dead = match dive.depression {
                        None => mc_sim::combat::spinal_dead_zone(drop, dive.height),
                        Some(d) => mc_sim::combat::depression_dead_zone(drop + dive.height, d),
                    };
                    // Whole metres, so a hovering hull's bob does not keep re-sorting the rings.
                    let ring = &mut out[first + dive.span];
                    ring.inner = ring.inner.max(dead.round()).min(ring.outer);
                }
            }
        }
        // Sorting out what is hidden is the costly part, and only a tick, another selection
        // or another ring in focus changes it (a lit ring is a group of its own).
        if let Some((bp, spans)) = self.focus {
            of = (of ^ (bp as u64) << 48 ^ spans).wrapping_mul(0x100000001b3);
            of = (of ^ 1).wrapping_mul(0x100000001b3);
        }
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
        const ALL: [Reach; 13] = [
            Reach::Direct,
            Reach::Indirect,
            Reach::Missile,
            Reach::AntiAir,
            Reach::AntiMissile,
            Reach::Torpedo,
            Reach::Radar,
            Reach::Sonar,
            Reach::Build,
            Reach::Reclaim,
            Reach::Shield,
            Reach::Damper,
            Reach::Repair,
        ];
        ALL.into_iter()
            .flat_map(|reach| (0..RANKS).map(move |rank| (reach, rank)))
            .filter_map(|(reach, rank)| {
                let group = reach as u32 | (rank as u32) << RANK_SHIFT;
                let farthest = rings
                    .iter()
                    .filter(|r| r.group & !FOCUS == group)
                    .max_by(|a, b| a.outer.total_cmp(&b.outer))?;
                Some((reach, rank, farthest.inner, farthest.outer))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;

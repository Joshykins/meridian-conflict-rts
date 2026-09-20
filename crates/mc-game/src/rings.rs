//! Range rings: what the selection reaches, as circles on the ground. One
//! colour per kind of reach; a weapon's dead zone is the dashed inner circle.
//! The renderer merges the rings of a kind, so an army shows one outline.
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
        }
    }

    /// sRGB, for the HUD's key.
    pub fn tone(self) -> u32 {
        match self {
            Reach::Direct => 0xFF4B3A,
            Reach::Indirect => 0xFFD23C,
            Reach::Missile => 0xFF8A1E,
            Reach::AntiAir => 0x5CC8FF,
            Reach::Torpedo => 0x2EE6A8,
            Reach::Radar => 0x4F7DFF,
            Reach::Build => 0xE6F2F0,
            Reach::Reclaim => 0xFFB38A,
        }
    }

    fn linear(self) -> [f32; 3] {
        let c = self.tone();
        [16, 8, 0].map(|shift| (((c >> shift) & 0xFF) as f32 / 255.0).powf(2.2))
    }

    /// What a weapon's ring says about it: what it can hit first, then how the shot flies.
    fn of(weapon: &Weapon) -> Reach {
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

/// One circle pair of a blueprint: the kind, the dead zone's radius (zero without one) and the reach.
type Span = (Reach, f32, f32);

fn spans(bp: &UnitBlueprint) -> Vec<Span> {
    let mut out: Vec<Span> = bp
        .weapons
        .iter()
        .map(|w| (Reach::of(w), w.range_min.to_f32(), w.range_max.to_f32()))
        .collect();
    out.push((Reach::Radar, 0.0, bp.radar.to_f32()));
    // A factory builds inside itself: its builder has no range.
    out.push((
        Reach::Build,
        0.0,
        bp.builder.as_ref().map_or(0.0, |b| b.range.to_f32()),
    ));
    out.push((
        Reach::Reclaim,
        0.0,
        bp.reclaimer.map_or(0.0, |r| r.range.to_f32()),
    ));
    out.retain(|s| s.2 > 0.0);
    // Twin weapons are one ring.
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out.dedup();
    out
}

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
                    && far > 0.0
                    && d2 >= near * near
                    && d2 <= far * far
            })
        })
    };
    (0..rings.len())
        .map(|i| buried(i, rings[i].outer) && (rings[i].inner <= 0.0 || buried(i, rings[i].inner)))
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
            out.extend(spans.iter().map(|&(reach, inner, outer)| RangeRing {
                center,
                inner,
                outer,
                color: reach.linear(),
                group: reach as u32,
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

    /// The HUD's key to `rings`: each kind drawn, with its farthest reach and that ring's dead zone.
    pub fn key(rings: &[RangeRing]) -> Vec<(Reach, f32, f32)> {
        const ALL: [Reach; 8] = [
            Reach::Direct,
            Reach::Indirect,
            Reach::Missile,
            Reach::AntiAir,
            Reach::Torpedo,
            Reach::Radar,
            Reach::Build,
            Reach::Reclaim,
        ];
        ALL.into_iter()
            .filter_map(|reach| {
                let farthest = rings
                    .iter()
                    .filter(|r| r.group == reach as u32)
                    .max_by(|a, b| a.outer.total_cmp(&b.outer))?;
                Some((reach, farthest.inner, farthest.outer))
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

    fn of(b: &Blueprints, key: &str) -> Vec<Span> {
        spans(b.unit(b.id_of(key).unwrap()))
    }

    #[test]
    fn rings_follow_the_data() {
        let b = blueprints();
        assert_eq!(
            of(&b, "aster_t1_scout"),
            vec![(Reach::Direct, 0.0, 140.0), (Reach::Radar, 0.0, 800.0)]
        );
        assert_eq!(of(&b, "aster_t1_tank"), vec![(Reach::Direct, 0.0, 180.0)]);
        // A howitzer has a dead zone, a missile rack is its own kind.
        assert_eq!(
            of(&b, "aster_t1_artillery"),
            vec![(Reach::Indirect, 60.0, 320.0)]
        );
        assert_eq!(
            of(&b, "aster_t2_missile"),
            vec![(Reach::Missile, 120.0, 620.0)]
        );
        // The Paladin's twin projectors are one ring.
        assert_eq!(
            of(&b, "aster_t3_assault_bot"),
            vec![(Reach::Direct, 0.0, 280.0)]
        );
        assert_eq!(of(&b, "aster_t1_radar"), vec![(Reach::Radar, 0.0, 1150.0)]);
        assert_eq!(of(&b, "aster_t2_radar"), vec![(Reach::Radar, 0.0, 2000.0)]);
        assert_eq!(of(&b, "aster_t3_radar"), vec![(Reach::Radar, 0.0, 4000.0)]);
        assert_eq!(
            of(&b, "aster_commander"),
            vec![(Reach::Direct, 0.0, 330.0), (Reach::Build, 0.0, 70.0)]
        );
        // Factories build inside themselves; a reactor reaches nothing.
        assert!(of(&b, "aster_t1_land_factory").is_empty());
        assert!(of(&b, "aster_t1_power").is_empty());
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
        };
        let (one, drawn) = rings.collect([&unit].into_iter(), 0.5, true);
        assert_eq!((one.len(), drawn), (1, 1));
        assert_eq!(one[0].center, [15.0, 30.0]);
        assert_eq!(Rings::key(&one), vec![(Reach::Direct, 0.0, 180.0)]);

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

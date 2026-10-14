//! The selection panel's figures for many units (`Summary`), counted a share of the
//! units at a time when a selection of tens of thousands would otherwise count them all
//! in the frame after each of the sim's ticks (`Kept::summary`).

use super::kept::Shown;
use super::ordered_as;
use crate::hud::economy::{self, Flow};
use crate::hud::priority::Mix;
use crate::hud::Scene;
use mc_data::{BlueprintId, UnitBlueprint};
use mc_sim::mirror::UnitInstance;
use mc_sim::FireState;

/// The selection panel's figures for many units (`info`).
#[derive(Clone, Default)]
pub(super) struct Summary {
    /// The one lift ship among them, when every other unit was taken out of a hold.
    pub(super) base: Option<u32>,
    /// A tile per blueprint: how many, and whether any of them is paused; most numerous
    /// first, then by blueprint.
    pub(super) types: Vec<(u32, usize, bool)>,
    pub(super) can_save: bool,
    pub(super) kills: u32,
    pub(super) reclaimed: f32,
    /// The mean.
    pub(super) health: f32,
    /// The emptiest tank among them.
    pub(super) low_fuel: Option<f32>,
    pub(super) economy: (Flow, Flow),
    /// Every one of them is a core mine with its view: the mine panel instead of tiles.
    pub(super) all_mines: bool,
    /// Of those that shoot (they or what they make, `ordered_as`), how many hold their
    /// fire, and how many there are: the order card's Hold Fire.
    pub(super) held: (usize, usize),
    /// Their priorities, for the order card's Priority.
    pub(super) mix: Mix,
}

/// A `Summary` being counted, and how far it has got.
pub(super) struct Tally {
    /// What it is counted for; `None`: all at once, for a deck worked out every frame.
    key: Option<Shown>,
    /// How many of the units are counted.
    done: usize,
    out: Summary,
    health: f32,
    /// What they make and spend together.
    materials: Flow,
    energy: Flow,
    /// For each blueprint met, as `out.types`: whether it shoots (it or what it makes),
    /// takes a priority, and sends reclaimers, asked once.
    traits: Vec<(bool, bool, bool)>,
    /// The tile of the last unit counted: units come in runs of one blueprint, and the
    /// tile is looked for only when the run ends.
    at: usize,
}

impl Tally {
    pub(super) fn new(key: Option<Shown>) -> Tally {
        Tally {
            key,
            done: 0,
            out: Summary::default(),
            health: 0.0,
            materials: Flow::default(),
            energy: Flow::default(),
            traits: Vec::new(),
            at: usize::MAX,
        }
    }

    pub(super) fn key(&self) -> Option<Shown> {
        self.key
    }

    pub(super) fn done(&self) -> usize {
        self.done
    }

    /// Counts `units` up to `upto`, from where it got to; the same units each time.
    pub(super) fn count(&mut self, s: &Scene, units: &[&UnitInstance], upto: usize) {
        let upto = upto.min(units.len());
        for u in &units[self.done.min(upto)..upto] {
            let out = &mut self.out;
            if out.types.get(self.at).is_none_or(|t| t.0 != u.blueprint) {
                self.at = match out.types.iter().position(|t| t.0 == u.blueprint) {
                    Some(i) => i,
                    None => {
                        let bp = s.bp(u);
                        out.types.push((u.blueprint, 0, false));
                        self.traits.push((
                            ordered_as(s.blueprints, bp).any(|b| !b.weapons.is_empty()),
                            mc_sim::focus::prioritizable(s.blueprints, bp),
                            bp.sends_reclaimers(),
                        ));
                        out.types.len() - 1
                    }
                };
            }
            let tile = &mut out.types[self.at];
            tile.1 += 1;
            tile.2 |= u.paused();
            let (shoots, prioritizable, reclaims) = self.traits[self.at];
            out.kills += u.kill_count();
            self.health += u.health;
            if let Some(f) = s.view.fuel(u.unit_id) {
                let fill = f.fill.clamp(0.0, 1.0);
                out.low_fuel = Some(out.low_fuel.map_or(fill, |low: f32| low.min(fill)));
            }
            if reclaims {
                out.reclaimed += s.queue_of(u).map_or(0.0, |q| q.reclaimed);
            }
            let (m, e) = economy::flows(s, u, s.bp(u));
            self.materials.add(m);
            self.energy.add(e);
            if shoots {
                out.held.0 += usize::from(u.fire_state() == FireState::HoldFire);
                out.held.1 += 1;
            }
            out.mix.add(s, u, prioritizable);
        }
        self.done = self.done.max(upto);
    }

    /// The summary of every unit counted: all of `units`.
    pub(super) fn finish(self, s: &Scene, units: &[&UnitInstance], kinds: &[u32]) -> Summary {
        let any = |test: &dyn Fn(&UnitBlueprint) -> bool| {
            (kinds.iter()).any(|&k| test(s.blueprints.unit(BlueprintId(k as u16))))
        };
        let Tally {
            mut out,
            health,
            materials,
            energy,
            ..
        } = self;
        out.types
            .sort_by_key(|&(bp, n, _)| (std::cmp::Reverse(n), bp));
        out.health = health / units.len().max(1) as f32;
        out.economy = (materials, energy);
        // A lift ship with units picked out of its hold: the ship's panel stays up.
        if any(&|bp| bp.transport.is_some()) {
            let mut bases = units.iter().filter(|u| s.bp(u).transport.is_some());
            if let (Some(base), None) = (bases.next(), bases.next()) {
                if units
                    .iter()
                    .all(|u| u.unit_id == base.unit_id || u.stored())
                {
                    out.base = Some(base.unit_id);
                }
            }
        }
        let structures = any(&|bp| bp.built_on_site() && !bp.is_mobile());
        out.can_save = crate::hud::prefabs::can_save(s, units, structures);
        // A mine's view comes only with a mine's blueprint (`World::mine_view`).
        out.all_mines = !units.is_empty()
            && kinds
                .iter()
                .all(|&k| s.blueprints.unit(BlueprintId(k as u16)).mine.is_some())
            && units
                .iter()
                .all(|u| s.queue_of(u).is_some_and(|q| q.mine.is_some()));
        out
    }
}

/// `Summary` of `units`, all at once.
pub(super) fn summary(s: &Scene, units: &[&UnitInstance], kinds: &[u32]) -> Summary {
    let mut tally = Tally::new(None);
    tally.count(s, units, units.len());
    tally.finish(s, units, kinds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::View;
    use mc_data::Blueprints;
    use mc_render::Camera;

    #[test]
    fn counted_in_shares_it_comes_to_the_same() {
        let blueprints = Blueprints::load(&Blueprints::locate_data_dir().expect("data dir"))
            .expect("blueprints");
        let map = mc_map::MapFile::open(crate::ui::test_maps::field()).expect("map opens");
        let camera = Camera::new(glam::Vec2::splat(4000.0), glam::Vec2::new(1920.0, 1080.0));
        let mut view = View::new(0, crate::setup::TEAM_COLORS, false);
        let kinds = ["aster_t1_interceptor", "aster_t1_bomber"]
            .map(|key| blueprints.id_of(key).expect("blueprint").0 as u32);
        for i in 0..50u32 {
            let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
            u.unit_id = i;
            // Runs of one blueprint, then the other, and back.
            u.blueprint = kinds[(i / 7 % 2) as usize];
            u.health = (i % 10) as f32 / 10.0;
            u.packed = UnitInstance::pack_veterancy(i % 3, 0, 0.0);
            view.frame.units.push(u);
            view.frame.fuel.push(mc_sim::mirror::FuelView {
                unit_id: i,
                fill: 1.0 - i as f32 / 100.0,
                dry_in: None,
            });
        }
        let stats = mc_render::FrameStats::default();
        let s = Scene {
            view: &view,
            blueprints: &blueprints,
            map: &map,
            camera: &camera,
            gpu: &stats,
            hover: None,
            show_reclaim: false,
            placing: None,
            placing_open: None,
            net: None,
            net_notices: &[],
            prefabs: crate::prefabs::Library::none(),
        };
        let units: Vec<&UnitInstance> = view.frame.units.iter().collect();
        let mut kinds = kinds.to_vec();
        kinds.sort_unstable();
        let whole = summary(&s, &units, &kinds);
        let mut shares = Tally::new(None);
        for upto in [0, 13, 13, 40, 99] {
            shares.count(&s, &units, upto);
        }
        assert_eq!(shares.done(), units.len());
        let shares = shares.finish(&s, &units, &kinds);
        let figures = |m: &Summary| {
            (
                m.types.clone(),
                m.kills,
                m.health.to_bits(),
                m.low_fuel.map(f32::to_bits),
                m.held,
            )
        };
        assert_eq!(figures(&whole), figures(&shares));
        assert_eq!(whole.types.len(), 2);
        assert_eq!(whole.kills, (0..50).map(|i| i % 3).sum::<u32>());
        assert_eq!(whole.low_fuel, Some(0.51));
    }
}

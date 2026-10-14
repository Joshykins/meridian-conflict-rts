//! What the deck works out from many units, kept while they and the frame stay the same.
//!
//! The selection panel's tiles and figures, the order card and the panels beside it each
//! looked at every unit of the selection, every frame. Sixty thousand fighters made that
//! tens of milliseconds a frame for figures that only change with the sim's tick, ten
//! times a second, or when the selection does. Here they are worked out once for each
//! (`Shown`), and asked of the blueprints once each where the blueprint alone decides
//! (`kinds`). Nothing shown changes: the same functions give the same answers, less often.

use super::{families, Family, Order};
use crate::game::Mode;
use crate::hud::economy::{self, Flow};
use crate::hud::priority::Mix;
use crate::hud::{cargo, elevator, gravitic, silo, titan, Scene};
use mc_data::{BlueprintId, UnitBlueprint};
use mc_sim::mirror::{CargoView, ElevatorView, UnitInstance};

/// What the deck shows, as far as what is kept for it goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::hud) enum Shown {
    /// The selection, by `Picked::stamp` (which changes with every pull and selection).
    Selection(u64),
    /// What it showed last, going away: by the pull, and which units those were.
    Closing { pulls: u64, units: u64 },
}

/// The order card: its families and their orders.
type Card = Vec<(Family, Vec<Order>)>;

/// A value and what it was worked out for. With no key, it is worked out each time.
struct Memo<K, V> {
    key: Option<K>,
    value: Option<V>,
}

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Memo {
            key: None,
            value: None,
        }
    }
}

impl<K: PartialEq, V> Memo<K, V> {
    fn get(&mut self, key: Option<K>, work: impl FnOnce() -> V) -> &V {
        if key.is_none() || self.key != key {
            self.value = None;
            self.key = key;
        }
        self.value.get_or_insert_with(work)
    }
}

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
}

/// The panels beside the order card, and the first unit each is for (by id) and how many.
#[derive(Clone, Default)]
pub(in crate::hud) struct Panels {
    pub(in crate::hud) launcher: Option<(u32, silo::Launcher)>,
    pub(in crate::hud) titan: Option<(u32, usize)>,
    pub(in crate::hud) engine: Option<(u32, usize)>,
    pub(in crate::hud) elevator: Option<(u32, ElevatorView, usize)>,
    pub(in crate::hud) ship: Option<(u32, CargoView)>,
}

#[derive(Default)]
pub(in crate::hud) struct Kept {
    /// What the deck shows this frame; `None` while it is something worked out afresh
    /// each frame (the one unit under the pointer).
    shown: Option<Shown>,
    /// The `Picked::stamp` the deck last compared what it shows against (`Hud::draw`).
    pub(in crate::hud) compared: Option<u64>,
    /// Counts the changes of the units the deck last showed (`Hud::deck_units`).
    pub(in crate::hud) closing: u64,
    kinds: Memo<Shown, Vec<u32>>,
    summary: Memo<Shown, Summary>,
    card: Memo<(Shown, Mode, bool), Card>,
    mix: Memo<Shown, Mix>,
    panels: Memo<Shown, Panels>,
}

/// The blueprints of `units`, each once, ascending.
pub(in crate::hud) fn kinds_of(units: &[&UnitInstance]) -> Vec<u32> {
    let mut kinds: Vec<u32> = Vec::new();
    for u in units {
        if kinds.last() != Some(&u.blueprint) {
            kinds.push(u.blueprint);
        }
    }
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

fn bp_of<'a>(s: &Scene<'a>, kind: u32) -> &'a UnitBlueprint {
    s.blueprints.unit(BlueprintId(kind as u16))
}

impl Kept {
    /// Sets what the deck shows this frame, before anything asks.
    pub(in crate::hud) fn show(&mut self, shown: Option<Shown>) {
        self.shown = shown;
    }

    /// The blueprints of `units`, each once (`kinds_of`).
    pub(in crate::hud) fn kinds(&mut self, s: &Scene, units: &[&UnitInstance]) -> &[u32] {
        let shown = self.shown;
        self.kinds.get(shown, || match shown {
            Some(Shown::Selection(_)) => s.view.picked().kinds().to_vec(),
            _ => kinds_of(units),
        })
    }

    /// Whether any of `units` is of a blueprint `test` takes.
    pub(in crate::hud) fn any_kind(
        &mut self,
        s: &Scene,
        units: &[&UnitInstance],
        test: impl Fn(&UnitBlueprint) -> bool,
    ) -> bool {
        self.kinds(s, units).iter().any(|&k| test(bp_of(s, k)))
    }

    pub(super) fn summary(&mut self, s: &Scene, units: &[&UnitInstance]) -> Summary {
        let kinds = self.kinds(s, units).to_vec();
        (self.summary.get(self.shown, || summary(s, units, &kinds))).clone()
    }

    /// The order card's families (`families`); the lit orders follow the mode.
    pub(super) fn card(&mut self, s: &Scene, units: &[&UnitInstance]) -> Card {
        let kinds = self.kinds(s, units).to_vec();
        let key = self
            .shown
            .map(|shown| (shown, s.view.mode, s.view.formation_panel));
        (self.card.get(key, || families(s, units, &kinds))).clone()
    }

    pub(in crate::hud) fn mix(&mut self, s: &Scene, units: &[&UnitInstance]) -> Mix {
        *self.mix.get(self.shown, || Mix::of(s, units))
    }

    pub(in crate::hud) fn panels(&mut self, s: &Scene, units: &[&UnitInstance]) -> Panels {
        let kinds = self.kinds(s, units).to_vec();
        let any = |test: &dyn Fn(&UnitBlueprint) -> bool| kinds.iter().any(|&k| test(bp_of(s, k)));
        let work = || Panels {
            launcher: any(&|bp| bp.strategic.is_some())
                .then(|| silo::launcher_of(s, units))
                .flatten()
                .map(|(u, l)| (u.unit_id, l)),
            titan: any(&|bp| crate::titan_marks::is_titan(bp))
                .then(|| titan::titan_of(s, units))
                .flatten()
                .map(|(u, n)| (u.unit_id, n)),
            engine: any(&|bp| bp.gravitic.is_some())
                .then(|| gravitic::engine_of(s, units))
                .flatten()
                .map(|(u, n)| (u.unit_id, n)),
            elevator: any(&|bp| bp.elevator.is_some())
                .then(|| elevator::elevator_of(s, units))
                .flatten()
                .map(|(u, v, n)| (u.unit_id, v, n)),
            // Only a lift ship has a hold (`World::cargo_view`).
            ship: any(&|bp| bp.transport.is_some())
                .then(|| cargo::ship_of(s, units))
                .flatten()
                .map(|(u, v)| (u.unit_id, v)),
        };
        self.panels.get(self.shown, work).clone()
    }
}

/// `Summary`, in a pass or two over `units`.
fn summary(s: &Scene, units: &[&UnitInstance], kinds: &[u32]) -> Summary {
    let any = |test: &dyn Fn(&UnitBlueprint) -> bool| kinds.iter().any(|&k| test(bp_of(s, k)));
    let mut out = Summary::default();
    // A lift ship with units picked out of its hold: the ship's panel stays up.
    if any(&|bp| bp.transport.is_some()) {
        let mut bases = units.iter().filter(|u| s.bp(u).transport.is_some());
        if let (Some(base), None) = (bases.next(), bases.next()) {
            if units
                .iter()
                .all(|u| u.unit_id == base.unit_id || u.stored())
            {
                out.base = Some(base.unit_id);
                return out;
            }
        }
    }
    let reclaimers = any(&|bp| bp.sends_reclaimers());
    let mut health = 0.0;
    // Units come in runs of one blueprint: the tile is looked for only when the run ends.
    let mut at = usize::MAX;
    for u in units {
        if out.types.get(at).is_none_or(|t| t.0 != u.blueprint) {
            at = match out.types.iter().position(|t| t.0 == u.blueprint) {
                Some(i) => i,
                None => {
                    out.types.push((u.blueprint, 0, false));
                    out.types.len() - 1
                }
            };
        }
        let tile = &mut out.types[at];
        tile.1 += 1;
        tile.2 |= u.paused();
        out.kills += u.kill_count();
        health += u.health;
        if let Some(f) = s.view.fuel(u.unit_id) {
            let fill = f.fill.clamp(0.0, 1.0);
            out.low_fuel = Some(out.low_fuel.map_or(fill, |low: f32| low.min(fill)));
        }
        if reclaimers && s.bp(u).sends_reclaimers() {
            out.reclaimed += s.queue_of(u).map_or(0.0, |q| q.reclaimed);
        }
    }
    out.types
        .sort_by_key(|&(bp, n, _)| (std::cmp::Reverse(n), bp));
    out.health = health / units.len().max(1) as f32;
    out.can_save = crate::hud::prefabs::can_save(s, units);
    out.economy = economy::total(s, units);
    // A mine's view comes only with a mine's blueprint (`World::mine_view`).
    out.all_mines = !units.is_empty()
        && kinds.iter().all(|&k| bp_of(s, k).mine.is_some())
        && units
            .iter()
            .all(|u| s.queue_of(u).is_some_and(|q| q.mine.is_some()));
    out
}

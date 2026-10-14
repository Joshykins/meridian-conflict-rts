//! What the deck works out from many units, kept while they and the frame stay the same.
//!
//! The selection panel's tiles and figures, the order card and the panels beside it each
//! looked at every unit of the selection, every frame. Sixty thousand fighters made that
//! tens of milliseconds a frame for figures that only change with the sim's tick, ten
//! times a second, or when the selection does. Here they are worked out once for each
//! (`Shown`), and asked of the blueprints once each where the blueprint alone decides
//! (`kinds`). Nothing shown changes: the same functions give the same answers, less often.
//! The panel's figures for a selection of thousands are counted a share a frame
//! (`tally.rs`), the last tick's standing meanwhile: a tick late at most.

use super::tally::{self, Summary, Tally};
use super::{families, Family, Order};
use crate::game::Mode;
use crate::hud::priority::Mix;
use crate::hud::{cargo, elevator, gravitic, silo, titan, Scene};
use mc_data::{BlueprintId, UnitBlueprint};
use mc_sim::mirror::{CargoView, ElevatorView, UnitInstance};

/// What the deck shows, as far as what is kept for it goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::hud) enum Shown {
    /// The selection, by `Picked::stamp` (which changes with every pull and selection), and
    /// `Picked::units` (with every selection).
    Selection { stamp: u64, units: u64 },
    /// What it showed last, going away: by the pull, and which units those were.
    Closing { pulls: u64, units: u64 },
}

impl Shown {
    /// Which units, whatever the tick.
    fn units(self) -> (bool, u64) {
        match self {
            Shown::Selection { units, .. } => (false, units),
            Shown::Closing { units, .. } => (true, units),
        }
    }
}

/// Units past which the panel's figures are counted a share a frame (`SHARES` of them).
const SHARED_FROM: usize = 8192;
const SHARES: usize = 3;

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
    /// The last summary finished, and what for.
    summary: Option<(Shown, Summary)>,
    /// The one being counted.
    tally: Option<Tally>,
    /// A share was counted this frame: the deck asks for the figures several times.
    counted: bool,
    /// Counts the summaries finished: the order card's Hold Fire goes by them.
    summaries: u64,
    card: Memo<(Shown, Mode, bool, u64), Card>,
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
        self.counted = false;
    }

    /// The blueprints of `units`, each once (`kinds_of`).
    pub(in crate::hud) fn kinds(&mut self, s: &Scene, units: &[&UnitInstance]) -> &[u32] {
        let shown = self.shown;
        self.kinds.get(shown, || match shown {
            Some(Shown::Selection { .. }) => s.view.picked().kinds().to_vec(),
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

    /// The selection panel's figures. For thousands of units, the frames after a tick
    /// count a share each and show the last tick's meanwhile.
    pub(super) fn summary(&mut self, s: &Scene, units: &[&UnitInstance]) -> Summary {
        let kinds = self.kinds(s, units).to_vec();
        let Some(shown) = self.shown else {
            return tally::summary(s, units, &kinds);
        };
        if let Some((_, sum)) = self.summary.as_ref().filter(|(key, _)| *key == shown) {
            return sum.clone();
        }
        // The same units' figures, a tick old, to show while these are counted.
        let meanwhile = (self.summary.as_ref()).filter(|(key, _)| key.units() == shown.units());
        if let Some((_, sum)) = meanwhile.filter(|_| self.counted) {
            return sum.clone();
        }
        // A count the next tick overtook before it was done: this one is done at once, so
        // the figures never fall more than a tick behind.
        let (mut count, overtaken) = match self.tally.take() {
            Some(t) if t.key() == Some(shown) => (t, false),
            last => (Tally::new(Some(shown)), last.is_some()),
        };
        let upto = match meanwhile {
            Some(_) if !overtaken && units.len() > SHARED_FROM => {
                count.done() + units.len().div_ceil(SHARES)
            }
            _ => units.len(),
        };
        count.count(s, units, upto);
        self.counted = true;
        if count.done() < units.len() {
            if let Some((_, sum)) = meanwhile {
                let sum = sum.clone();
                self.tally = Some(count);
                return sum;
            }
            count.count(s, units, units.len());
        }
        let sum = count.finish(s, units, &kinds);
        self.summaries += 1;
        self.summary = Some((shown, sum.clone()));
        sum
    }

    /// The order card's families (`families`); the lit orders follow the mode.
    pub(super) fn card(&mut self, s: &Scene, units: &[&UnitInstance]) -> Card {
        let kinds = self.kinds(s, units).to_vec();
        let (held, of) = self.summary(s, units).held;
        let key = (self.shown).map(|shown| {
            let panel = s.view.formation_panel;
            (shown, s.view.mode, panel, self.summaries)
        });
        let holding = held * 2 > of;
        (self
            .card
            .get(key, || families(s, units, &kinds, Some(holding))))
        .clone()
    }

    pub(in crate::hud) fn mix(&mut self, s: &Scene, units: &[&UnitInstance]) -> Mix {
        self.summary(s, units).mix
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

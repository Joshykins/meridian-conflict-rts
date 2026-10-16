//! What an engineer assisting a unit can do for it. One list, read both by an Assist
//! order (`run_assist`) and by an engineer looking round its Area Assist ring
//! (`area_work.rs`), so a kind of work an engineer can help with is never helpable by
//! hand and missed by the ring.

use crate::tables::*;
use crate::World;

/// Work an assisting engineer can do on or through a unit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AssistWork {
    /// The unit is itself a site going up.
    Raise,
    /// What the unit is building: a factory's product, an upgrade's or a refit's new
    /// body, or a builder's site.
    Product(usize),
    /// The unit is hurt.
    Mend,
    /// A silo's or an interceptor array's next round (`nukes.rs`).
    Round,
    /// A live dome missing charge (`shields.rs`).
    Shield,
}

impl World {
    /// What there is to do for `t`, in the order an assist takes it up: finish `t`
    /// itself, help with whatever it is building, mend it, assemble its next round, or
    /// feed its shield.
    pub(crate) fn assist_work(&self, t: usize) -> Option<AssistWork> {
        let units = &self.state.units;
        if units.has_flag(t, flag::UNDER_CONSTRUCTION) {
            Some(AssistWork::Raise)
        } else if let Some(p) = units
            .row(units.build_target[t])
            .filter(|&p| units.has_flag(p, flag::UNDER_CONSTRUCTION))
        {
            Some(AssistWork::Product(p))
        } else if units.health[t] < self.unit_max_health(t) {
            Some(AssistWork::Mend)
        } else if self.launcher_wants_round(t) {
            Some(AssistWork::Round)
        } else if self.shield_assistable(t) {
            Some(AssistWork::Shield)
        } else {
            None
        }
    }
}

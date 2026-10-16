//! The Saker (`regency_saker`): the Regency's tech 4 surge artillery. A low arrowhead
//! hull slung between four legs that reach out low to plant far out at its corners on
//! spades and toes, one very long split pinch bore laid over it from a turret whose
//! housing sits at the back.
//!
//! It plants to fire (`deploy`): each leg steps out in turn to brace wider and the hull
//! settles low between them (`set_crawl_brace`); packing runs it back, the feet standing
//! in and the hull riding higher to walk.
//!
//! The gun is the Regency's bore (`gun`): split down its length into two plated halves,
//! one above the other, that hold the charge between their tips; its `muzzle` is the
//! middle of that charge. Its fusion is housed in the plated hump on the turret's back;
//! the light is in the plates (`pattern::EMBER_SLOT`), never a lit piece.
//!
//! Authored planted, [`SCALE`] times smaller than the unit file builds it
//! (`regency_t4_surge_artillery`, `data/factions/regency/units/experimental.ron`): its
//! size, pivot and muzzle are this file's, its gun's [`gun::Line`], times `SCALE` (tests).

mod feet;
mod gun;
mod hull;
mod legs;
#[cfg(test)]
mod tests;
mod turret;

use crate::builder::MeshBuilder;
use crate::part;

/// Authored size, planted (the unit file's is `SCALE` times it).
pub(super) const RADIUS: f32 = 26.0;
pub(super) const HEIGHT: f32 = 14.5;
/// How much bigger the unit file builds it.
#[cfg(test)]
const SCALE: f32 = 1.3;

/// The hull's top, where the turret's race sits, planted.
const DECK: f32 = 6.6;

pub(super) fn saker(b: &mut MeshBuilder, _tech: u8) {
    b.set_dust_line(hull::HULL.belly + 1.5);
    gun::BORE.line.rig(b, DECK);
    legs::legs(b, &legs::LEGS);
    hull::hull(b, &hull::HULL);
    b.with_part(part::TURRET, |b| {
        turret::turret(b, gun::BORE.line.pivot);
        gun::gun(b, &gun::BORE);
    });
}

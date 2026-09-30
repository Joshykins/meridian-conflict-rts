//! What the interface sees of a factory's batch (`crate::batch`): a mark on the factory
//! for its own side (`UNIT_BATCH`), and for a factory it asks about, the muster block.

use crate::World;

/// Units' `status[0]`: a factory with batch on. Sent to its own side, allies and
/// observers only: how a factory sends its units out is an order, not something seen.
pub const UNIT_BATCH: u32 = 1 << 14;

/// A factory's batch as the interface shows it (`UnitOrders::batch`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchView {
    /// Which batch it is in: factories with the same number are linked.
    pub group: u32,
    /// The batch's count so far and what it leaves at: products out of the laps' total,
    /// or, with a size set (`fixed`), units waiting of the size.
    pub count: u16,
    pub size: u16,
    pub fixed: bool,
    /// Each place in this factory's muster block, and whether a unit stands in it: those
    /// taken first, then those still to fill.
    pub places: Vec<([f32; 2], bool)>,
    /// The way the block faces (the factory's), a unit vector.
    pub facing: [f32; 2],
    /// Distance between places, metres.
    pub spacing: f32,
    /// Where each linked factory's block starts, in the batch's order; this one is `index`.
    pub linked: Vec<[f32; 2]>,
    pub index: usize,
}

impl World {
    pub(super) fn batch_view(&self, row: usize) -> Option<BatchView> {
        let m = self.muster(row)?;
        let a = self.state.units.heading[row];
        Some(BatchView {
            group: m.group,
            count: m.count,
            size: m.size,
            fixed: m.fixed,
            places: m
                .places
                .iter()
                .map(|&(p, here)| (p.to_f32(), here))
                .collect(),
            facing: [a.cos().to_f32(), a.sin().to_f32()],
            spacing: m.spacing.to_f32(),
            linked: m.linked.iter().map(|p| p.to_f32()).collect(),
            index: m.index,
        })
    }

    /// Whether the factory in `row` is in a batch.
    pub(super) fn batching(&self, row: usize) -> bool {
        self.batch_of(self.state.units.id(row)).is_some()
    }
}

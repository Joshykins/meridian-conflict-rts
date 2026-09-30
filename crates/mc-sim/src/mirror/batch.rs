//! What the interface sees of a factory's batch (`crate::batch`): a mark on the factory
//! for its own side (`UNIT_BATCH`), and for a factory it asks about, the muster block.

use crate::World;

/// Units' `status[0]`: a factory with batch on. Sent to its own side, allies and
/// observers only: how a factory sends its units out is an order, not something seen.
pub const UNIT_BATCH: u32 = 1 << 14;

/// A factory's batch as the interface shows it (`UnitOrders::batch`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchView {
    /// Products this batch has had so far, and how many it will have.
    pub made: u16,
    pub size: u16,
    /// Each place in the muster block, and whether a unit stands in it: those taken first,
    /// then those still to fill.
    pub places: Vec<([f32; 2], bool)>,
    /// The way the block faces (the factory's), a unit vector.
    pub facing: [f32; 2],
    /// Distance between places, metres.
    pub spacing: f32,
}

impl World {
    pub(super) fn batch_view(&self, row: usize) -> Option<BatchView> {
        let m = self.muster(row)?;
        let a = self.state.units.heading[row];
        Some(BatchView {
            made: m.made,
            size: m.size,
            places: m
                .places
                .iter()
                .map(|&(p, here)| (p.to_f32(), here))
                .collect(),
            facing: [a.cos().to_f32(), a.sin().to_f32()],
            spacing: m.spacing.to_f32(),
        })
    }

    /// Whether the factory in `row` has batch on.
    pub(super) fn batching(&self, row: usize) -> bool {
        self.state.batches.contains_key(&self.state.units.id(row))
    }
}

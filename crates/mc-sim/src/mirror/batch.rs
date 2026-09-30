//! What the interface sees of a factory's batch (`crate::batch`): a mark on the factory
//! for its own side (`UNIT_BATCH`), and for a factory it asks about, who waits there.

use crate::World;

/// Units' `status[0]`: a factory with batch on. Sent to its own side, allies and
/// observers only: how a factory sends its units out is an order, not something seen.
pub const UNIT_BATCH: u32 = 1 << 14;

/// A factory's batch as the interface shows it (`UnitOrders::batch`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchView {
    /// Which batch it is in: factories with the same number are linked.
    pub group: u32,
    /// Units waiting in the whole batch, and how many it leaves at.
    pub count: u16,
    pub size: u16,
    /// The units waiting at this factory: to the player, one group.
    pub units: Vec<u32>,
    /// Where the next one out will stand, while more are to come.
    pub next: Option<[f32; 2]>,
    /// The factories linked in the batch, this one among them.
    pub linked: Vec<u32>,
}

impl World {
    pub(super) fn batch_view(&self, row: usize) -> Option<BatchView> {
        let m = self.muster(row)?;
        Some(BatchView {
            group: m.group,
            count: m.count,
            size: m.size,
            units: m.units.iter().map(|u| u.0).collect(),
            next: m.next.map(|p| p.to_f32()),
            linked: m.linked.iter().map(|f| f.0).collect(),
        })
    }

    /// Whether the factory in `row` is in a batch.
    pub(super) fn batching(&self, row: usize) -> bool {
        self.batch_of(self.state.units.id(row)).is_some()
    }
}

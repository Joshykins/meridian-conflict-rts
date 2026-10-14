//! Shares a tick's cosmetic emitters out when there are too many to draw: a sky of a
//! hundred thousand fighters trailing, smoking and falling would write millions of
//! puffs a tick into a ring of tens of thousands, each overwriting the last before it
//! is drawn. Past a budget only every `stride`th emitter emits, the same ones tick after
//! tick (by unit id), so what is drawn stays whole: a trail unbroken, a column steady.
//! A deliberate cap: the puffs are cosmetic.

#[derive(Clone, Copy)]
pub(super) struct Share {
    stride: u32,
}

impl Share {
    /// For `emitters` this tick, of which about `budget` may emit.
    pub(super) fn new(emitters: usize, budget: usize) -> Share {
        Share {
            stride: emitters.div_ceil(budget.max(1)).max(1) as u32,
        }
    }

    /// Whether the unit `unit_id` emits this tick.
    pub(super) fn takes(self, unit_id: u32) -> bool {
        self.stride == 1 || (unit_id.wrapping_mul(0x9E37_79B9) >> 16) % self.stride == 0
    }
}

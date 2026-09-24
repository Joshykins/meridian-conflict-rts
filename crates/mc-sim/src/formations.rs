//! Stable, centered command layouts. Local X is forward, local Y is left.
use mc_core::{Angle, Fx, FxVec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub anchor: FxVec2,
    pub heading: Angle,
    /// 0 waits for its queued leg, 1 forms during travel, 2 travels, 3 passes an obstruction.
    pub phase: u8,
    pub speed: Fx,
}

pub(crate) fn slots(count: usize, spacing: Fx, air: bool) -> Vec<FxVec2> {
    if count == 0 {
        return Vec::new();
    }
    let n = count as i32;
    let cols = Fx::from_int(n).sqrt().ceil_int().max(1);
    let flights = (n + 4) / 5;
    let flight_cols = Fx::from_int(flights).sqrt().ceil_int().max(1);
    let mut out: Vec<_> = (0..n)
        .map(|i| {
            if air {
                // Five-ship V: leader, inner pair, outer pair. Repeat the whole
                // motif across and behind, leaving one hull-spacing between Vs.
                let flight = i / 5;
                let wing = (i % 5 + 1) / 2;
                let side = if i % 5 == 0 {
                    0
                } else if (i % 5) % 2 == 0 {
                    -1
                } else {
                    1
                };
                FxVec2::new(
                    -spacing * ((flight / flight_cols) * 4 + wing),
                    spacing * ((flight % flight_cols) * 6 + side * wing),
                )
            } else {
                FxVec2::new(-spacing * (i / cols), spacing * (i % cols))
            }
        })
        .collect();
    let sum = out.iter().copied().fold(FxVec2::ZERO, |a, b| a + b);
    let mean = FxVec2::new(sum.x / n, sum.y / n);
    for p in &mut out {
        *p = *p - mean;
    }
    out
}

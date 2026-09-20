//! Deterministic foundations shared by every crate that touches game state.
//!
//! Nothing in here uses floating point on a path that feeds the simulation.
//! `to_f32`/`from_f32` exist for the renderer, UI and offline tools only.

pub mod fx;
pub mod hash;
pub mod rng;
pub mod trig;
pub mod vec;

pub use fx::Fx;
pub use hash::StateHasher;
pub use rng::Rng;
pub use trig::Angle;
pub use vec::{FxVec2, FxVec3};

/// Simulation rate. One tick is 100 ms of game time.
pub const TICKS_PER_SECOND: u32 = 10;

/// Most players a match supports.
pub const MAX_PLAYERS: usize = 8;

/// Index of a player slot, `0..MAX_PLAYERS`.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct PlayerId(pub u8);

impl PlayerId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

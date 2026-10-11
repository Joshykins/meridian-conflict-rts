//! What a Commander derives from the data and the map and keeps between thinks, so
//! no think builds it again. Never saved or hashed: it is rebuilt, the same, after
//! a snapshot is restored.
use super::profile::Profiles;
use crate::World;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(in crate::ai) struct Scratch {
    /// Work units the last think spent (world-model cells and contacts walked):
    /// a count the tests can budget, unlike time.
    pub work: u32,
    /// Its fronts at the last think, as read then: for the overlay.
    pub fronts: Vec<super::fronts::Front>,
}

// Derived data is never part of what two states compare.
impl PartialEq for Scratch {
    fn eq(&self, _: &Scratch) -> bool {
        true
    }
}
impl Eq for Scratch {}

impl std::fmt::Debug for Scratch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scratch")
            .field("work", &self.work)
            .finish_non_exhaustive()
    }
}

impl World {
    /// Every blueprint's profile: built on the first think, then shared by every side.
    pub(in crate::ai) fn profiles(&self) -> Arc<Profiles> {
        self.ai_shared
            .profiles
            .get_or_init(|| Arc::new(Profiles::build(&self.blueprints)))
            .clone()
    }
}

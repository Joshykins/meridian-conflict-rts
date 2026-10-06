//! What a Commander derives from the data and the map and keeps between thinks, so
//! no think builds it again. Never saved or hashed: it is rebuilt, the same, after
//! a snapshot is restored.
use super::profile::Profiles;
use super::zones::Zones;
use crate::World;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Default)]
pub(in crate::ai) struct Scratch {
    profiles: OnceLock<Arc<Profiles>>,
    zones: OnceLock<Arc<Zones>>,
    /// Work units the last think spent (world-model cells and contacts walked):
    /// a count the tests can budget, unlike time.
    pub work: u32,
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

impl Scratch {
    /// Zones the side plays on and how many hold ore; zeros before they are made.
    pub(in crate::ai) fn zone_counts(&self) -> (u32, u32) {
        self.zones.get().map_or((0, 0), |z| {
            (
                z.zones.len() as u32,
                z.zones.iter().filter(|z| z.ore > 0).count() as u32,
            )
        })
    }
}

impl World {
    /// Every blueprint's profile: built on the first think, then shared.
    pub(in crate::ai) fn profiles(&self, player: u8) -> Arc<Profiles> {
        self.state.ai[player as usize]
            .commander
            .scratch
            .profiles
            .get_or_init(|| Arc::new(Profiles::build(&self.blueprints)))
            .clone()
    }

    /// The side's zones (`zones.rs`), from its reach, its start and the map's ore.
    pub(in crate::ai) fn zones(&self, player: u8, reach: &super::reach::Reach) -> Arc<Zones> {
        self.state.ai[player as usize]
            .commander
            .scratch
            .zones
            .get_or_init(|| {
                let starts: Vec<_> = std::iter::once(self.state.players[player as usize].start)
                    .chain(
                        self.state
                            .players
                            .iter()
                            .enumerate()
                            .filter(|&(i, _)| i != player as usize)
                            .map(|(_, p)| p.start),
                    )
                    .collect();
                Arc::new(Zones::build(reach, &starts, &self.ore_centres()))
            })
            .clone()
    }
}

//! The state hash every machine reports each tick, built from named sections.
//!
//! Lockstep peers compare the whole hash; when it differs (a desync), comparing
//! the sections says which part of the state went apart first, before anyone
//! has to diff a snapshot.

use crate::{PlayerCommand, SimError, World};
use mc_core::StateHasher;

/// The sections, in the order they are hashed. Names show in desync reports.
pub const SECTIONS: [&str; SECTION_COUNT] = [
    "clock",
    "players",
    "units",
    "orders",
    "projectiles",
    "debris",
    "giants",
    "strategic",
    "mines",
    "factories",
    "survival",
    "terrain",
    "ai",
    "navigation",
    "fog",
];
pub const SECTION_COUNT: usize = 15;

/// One hash per entry of [`SECTIONS`].
pub type SectionHashes = [u64; SECTION_COUNT];

impl World {
    /// Applies `commands`, steps the simulation once, and returns the state hash.
    pub fn tick(&mut self, commands: &[PlayerCommand]) -> Result<u64, SimError> {
        Ok(combine(&self.tick_sections(commands)?))
    }

    /// Hash of the whole game state. Equal hashes on every machine, every tick, or it is a desync.
    pub fn hash(&self) -> u64 {
        combine(&self.hash_sections())
    }

    /// The state hash by section; [`combine`] folds them into [`World::hash`].
    pub fn hash_sections(&self) -> SectionHashes {
        let s = &self.state;
        let section = |write: &dyn Fn(&mut StateHasher)| {
            let mut h = StateHasher::new();
            write(&mut h);
            h.finish()
        };
        [
            section(&|h| {
                h.write_u64(s.tick as u64);
                h.write_u64(s.rng.state());
                h.write_u64(s.winner.map_or(u64::MAX, |w| w as u64));
            }),
            section(&|h| {
                for p in &s.players {
                    p.hash(h);
                }
            }),
            section(&|h| s.units.hash(h)),
            section(&|h| {
                s.orders.hash(h);
                h.write_u64(s.formation_serial);
                for (&id, g) in &s.formations {
                    h.write_u64(id);
                    h.write_i64(g.anchor.x.0);
                    h.write_i64(g.anchor.y.0);
                    h.write_i64(g.speed.0);
                    h.write_u64(g.heading.0 as u64 | (g.phase as u64) << 16);
                }
            }),
            section(&|h| s.projectiles.hash(h)),
            section(&|h| {
                s.wrecks.hash(h);
                h.write_u64(s.aircraft_crashes.len() as u64);
                for crash in &s.aircraft_crashes {
                    crash.hash(h);
                }
                h.write_u64(s.sinking.len() as u64);
                for hull in &s.sinking {
                    hull.hash(h);
                }
                s.stains.hash(h);
                s.fires.hash(h);
            }),
            section(&|h| crate::titan::hash_giants(s, h)),
            section(&|h| {
                s.strategic.hash(h);
                s.pads.hash(h);
            }),
            section(&|h| s.mines.hash(h)),
            section(&|h| {
                h.write_u64(s.rollouts.len() as u64);
                for (&id, r) in &s.rollouts {
                    h.write_u64(id.0 as u64);
                    h.write_u64(r.factory.0 as u64);
                    h.write_i64(r.exit.x.0);
                    h.write_i64(r.exit.y.0);
                }
                h.write_u64(s.batches.len() as u64);
                for (&id, b) in &s.batches {
                    h.write_u64(id.0 as u64);
                    h.write_u64(b.made as u64);
                    h.write_u64(b.held.len() as u64);
                    for (u, at) in &b.held {
                        h.write_u64(u.0 as u64);
                        h.write_i64(at.x.0);
                        h.write_i64(at.y.0);
                    }
                }
            }),
            section(&|h| {
                if let Some(survival) = &s.survival {
                    survival.hash(h);
                }
            }),
            section(&|h| {
                h.write_u64(s.terrain_edits.len() as u64);
                if let Some(e) = s.terrain_edits.last() {
                    h.write_u64(e.min.0 as u64 | (e.min.1 as u64) << 32);
                    h.write_u64(e.max.0 as u64 | (e.max.1 as u64) << 32);
                    h.write_u64(e.sample as u64);
                }
                h.write_u64s(&s.props_dead);
            }),
            section(&|h| {
                for ai in &s.ai {
                    ai.hash(h);
                }
                h.write_u64(s.ai_pending.len() as u64);
            }),
            section(&|h| self.nav.hash(h)),
            section(&|h| self.fog.hash_memory(h)),
        ]
    }
}

/// Folds section hashes into the one state hash.
pub fn combine(sections: &SectionHashes) -> u64 {
    let mut h = StateHasher::new();
    for &v in sections {
        h.write_u64(v);
    }
    h.finish()
}

/// The names of the sections that differ between two machines' hashes of one tick.
pub fn differing(a: &SectionHashes, b: &SectionHashes) -> Vec<&'static str> {
    SECTIONS
        .iter()
        .zip(a.iter().zip(b))
        .filter(|(_, (x, y))| x != y)
        .map(|(name, _)| *name)
        .collect()
}

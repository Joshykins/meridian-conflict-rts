//! Surge shells (`Weapon::surge`, the Regency Saker's mortar): a shell that strikes as
//! any shell does where it lands, and then lets go a wake that rolls out all round from
//! the crater over a wide disc and burns lightly over all of it for a while.
//!
//! - **The strike** is the shell's own: its `damage` over its `splash` (`impacts.rs`).
//! - **The wake** (`State::surges`) starts on the tick the shell lands, wherever it
//!   bursts (on a unit, a dome or the ground), and rolls out `Surge::speed` metres a
//!   second to `Surge::radius`. It is its own thing once let go: it rolls on if the gun
//!   dies.
//! - **The burn:** each tick, everything of the enemy's whose hull reaches into the disc
//!   the wake has rolled over takes `Surge::damage` a second, the same anywhere in it and
//!   however long it has stood there: the crush every gravity well deals
//!   (`well_area.rs`), never to aircraft or spacecraft. Shields first: a live enemy dome
//!   reaching into the disc takes it, and what stands under one is spared while it holds;
//!   a hull field takes its unit's share. Wrecks and city blocks in it wear down too.
//! - It burns `Surge::linger` seconds after the wake has reached its full radius, then is
//!   gone.
//!
//! Friends are never touched. What is drawn and heard comes from the shell's `Impact`
//! event (`blueprint`, `weapon`); the sim keeps no more than what it strikes with.

use mc_core::{Fx, FxVec2, StateHasher, TICKS_PER_SECOND};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

use crate::tables::UnitId;
use crate::well_area::{Aircraft, Sides, WellArea};
use crate::world::State;
use crate::World;

/// A surge shell's wake, rolling out from where it landed (`surge.rs`).
#[derive(Clone, Serialize, Deserialize)]
pub struct SurgeWake {
    /// Where the shell landed, and the ground there (the sea's surface over water).
    pub pos: FxVec2,
    pub z: Fx,
    pub owner: u8,
    pub source: UnitId,
    /// The gun that fired it: how far, how fast, how hard and how long.
    pub blueprint: BlueprintId,
    pub weapon: u8,
    /// Ticks since the shell landed.
    pub age: u16,
}

impl World {
    /// The shell of weapon `weapon` of `blueprint` landed at `at`: if it is a surge shell,
    /// its wake starts rolling out from there (`run_surges`).
    pub(crate) fn surge_struck(
        &mut self,
        at: FxVec2,
        blueprint: BlueprintId,
        weapon: u8,
        owner: u8,
        source: UnitId,
    ) {
        let surge = self
            .blueprints
            .unit(blueprint)
            .weapons
            .get(weapon as usize)
            .and_then(|w| w.surge);
        if surge.is_none() {
            return;
        }
        let at = self.clamp_to_map(at);
        let z = self.terrain.height_at(at).max(self.terrain.water_level());
        self.state.surges.push(SurgeWake {
            pos: at,
            z,
            owner,
            source,
            blueprint,
            weapon,
            age: 0,
        });
    }

    /// Every surge's wake rolls a tick further and burns what it has rolled over; those
    /// that have burnt out are gone.
    pub(crate) fn run_surges(&mut self) {
        let surges = std::mem::take(&mut self.state.surges);
        let mut left = Vec::with_capacity(surges.len());
        for mut wake in surges {
            let Some(surge) = self
                .blueprints
                .unit(wake.blueprint)
                .weapons
                .get(wake.weapon as usize)
                .and_then(|w| w.surge)
            else {
                continue;
            };
            wake.age = wake.age.saturating_add(1);
            let area = WellArea {
                mark: wake.pos,
                z: wake.z,
                radius: surge.reach(wake.age),
                owner: wake.owner,
                source: wake.source,
                per_tick: surge.damage / Fx::from_int(TICKS_PER_SECOND as i32),
                aircraft: Aircraft::Spared,
                edge: Fx::ONE,
                sides: Sides::Enemies,
            };
            self.crush_area(&area);
            if wake.age < surge.life_ticks() {
                left.push(wake);
            }
        }
        // Shells that landed while these burned (none do from here, but keep any).
        left.append(&mut self.state.surges);
        self.state.surges = left;
    }
}

/// The surges' wakes, for the state hash.
pub(crate) fn hash_surges(s: &State, h: &mut StateHasher) {
    h.write_u64(s.surges.len() as u64);
    for w in &s.surges {
        for v in [w.pos.x, w.pos.y, w.z] {
            h.write_i64(v.0);
        }
        h.write_u64(
            w.owner as u64
                | (w.weapon as u64) << 8
                | (w.age as u64) << 16
                | (w.blueprint.0 as u64) << 32,
        );
        h.write_u64(w.source.0 as u64);
    }
}

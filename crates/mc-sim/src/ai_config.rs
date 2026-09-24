//! Per-player AI tuning. These inputs are part of snapshots and the state hash.
use mc_core::StateHasher;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Doctrine {
    #[default]
    Adaptive,
    Aggressive,
    Economic,
    Defensive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub difficulty: Difficulty,
    pub doctrine: Doctrine,
    /// How strongly observed enemies affect production (0..=100).
    pub adaptation: u8,
    /// Retreat below this percentage of maximum health (0..=80).
    pub retreat_health: u8,
    /// Production preferences for land, air, and naval units (0..=200).
    /// Zero disables combat production in that domain.
    pub domain_weights: [u8; 3],
}

/// The decisions a difficulty level changes; see [`AiConfig::skill`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skill {
    /// Idle builders given a job in one think.
    pub builders_per_think: usize,
    /// Metres from the start a builder is sent to put down a mine.
    pub mine_travel: i32,
    /// Percent of its reach's worth a mine on bare ground must keep, shared
    /// with the mines already standing, to be worth building.
    pub bare_mine_efficiency: i32,
    /// Seconds a mine upgrade may take to pay back its cost (its energy
    /// counted as mass); doubled while materials pile up.
    pub upgrade_payback: i32,
    /// Energy income kept at this many times mass income.
    pub power_ratio: i32,
    /// Most factories it builds, however much material goes unspent.
    pub factory_cap: i32,
    /// Mass a second before factories are upgraded.
    pub tech_income: i32,
    /// Factories are upgraded once a few hundred mass are in store, not only
    /// once materials pile up.
    pub eager_tech: bool,
    /// Units added to (or taken from) every attack wave.
    pub wave_delta: i32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            difficulty: Difficulty::Normal,
            doctrine: Doctrine::Adaptive,
            adaptation: 75,
            retreat_health: 30,
            domain_weights: [100, 100, 100],
        }
    }
}

impl AiConfig {
    pub fn normalized(mut self) -> Self {
        self.adaptation = self.adaptation.min(100);
        self.retreat_health = self.retreat_health.min(80);
        self.domain_weights = self.domain_weights.map(|w| w.min(200));
        self
    }

    pub fn think_period(self) -> u32 {
        match self.difficulty {
            Difficulty::Easy => 30,
            Difficulty::Normal => 20,
            // Thinking faster than this made the AI play worse, not better.
            Difficulty::Hard => 15,
        }
    }

    pub fn memory_ticks(self) -> u32 {
        match self.difficulty {
            Difficulty::Easy => 300,
            Difficulty::Normal => 900,
            Difficulty::Hard => 1500,
        }
    }

    /// How well this AI plays. No level gets resources the others do not:
    /// Hard plays as well as the AI can, and the levels below it are handicapped
    /// in how they spend (measured with `tests/zz_ai_duel_probe.rs`).
    pub fn skill(self) -> Skill {
        match self.difficulty {
            Difficulty::Easy => Skill {
                builders_per_think: 3,
                mine_travel: 2200,
                bare_mine_efficiency: 30,
                upgrade_payback: 180,
                power_ratio: 5,
                factory_cap: 3,
                tech_income: 25,
                eager_tech: false,
                wave_delta: -2,
            },
            Difficulty::Normal => Skill {
                builders_per_think: 5,
                mine_travel: 2600,
                bare_mine_efficiency: 40,
                upgrade_payback: 240,
                power_ratio: 7,
                factory_cap: 6,
                tech_income: 18,
                eager_tech: false,
                wave_delta: -1,
            },
            Difficulty::Hard => Skill {
                builders_per_think: 8,
                mine_travel: 3000,
                bare_mine_efficiency: 50,
                upgrade_payback: 300,
                power_ratio: 8,
                factory_cap: 10,
                tech_income: 14,
                eager_tech: false,
                wave_delta: 0,
            },
        }
    }

    pub(crate) fn hash(self, h: &mut StateHasher) {
        h.write_u64(
            self.difficulty as u64
                | (self.doctrine as u64) << 8
                | (self.adaptation as u64) << 16
                | (self.retreat_health as u64) << 24,
        );
        for w in self.domain_weights {
            h.write_u64(w as u64);
        }
    }
}

//! How a survival map is laid out: where the Replication Engine stands, where
//! a defender may start, which way each front attacks from, and where the
//! engine may raise replication nodes. Read from the `survival` block of a
//! map's `maps/<stem>.ron`; a map without one is not offered for survival.
//!
//! Positions are metres on the map, x east, y north. Converted exactly to
//! fixed point when a match is set up, so the simulation sees them as part of
//! the match description (and a replay carries them).
//!
//! ```ron
//! survival: (
//!     engine: (12800, 12400),
//!     engine_start: 3,
//!     harbor: (13900, 9800),
//!     spawns: [
//!         (name: "Bastion", start: 0, blurb: "High ground, every front reaches it late."),
//!     ],
//!     fronts: [
//!         (name: "Northern Pass", domain: Land, path: [(11800, 12900), (7000, 11000)]),
//!         (name: "Sound", domain: Naval, path: [(13900, 9800), (6000, 4200)]),
//!         (name: "High Corridor", domain: Air, path: [(12000, 12000), (4200, 4200)]),
//!     ],
//!     node_sites: [
//!         (name: "Cinder Ridge", at: (9800, 11800), domain: Land),
//!     ],
//! )
//! ```

use serde::{Deserialize, Serialize};

/// Which kind of force comes down a front, and what a node site may print.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Domain {
    #[default]
    Land,
    Air,
    Naval,
}

impl Domain {
    pub const ALL: [Domain; 3] = [Domain::Land, Domain::Air, Domain::Naval];

    pub fn label(self) -> &'static str {
        match self {
            Domain::Land => "Land",
            Domain::Air => "Air",
            Domain::Naval => "Naval",
        }
    }

    /// The bit this domain takes in a front mask (`SurvivalRules::fronts`).
    pub fn bit(self) -> u8 {
        match self {
            Domain::Land => 1,
            Domain::Air => 2,
            Domain::Naval => 4,
        }
    }
}

/// A place a defender may begin.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpawnZone {
    pub name: String,
    /// Index into the baked map's start positions.
    pub start: u8,
    /// One line on what holding it is like.
    pub blurb: String,
}

/// One way the engine's forces come. `path` runs from the engine's side
/// toward the defenders; the last leg to wherever they started is added when
/// the match is set up.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FrontLayout {
    pub name: String,
    pub domain: Domain,
    pub path: Vec<(f32, f32)>,
}

/// Somewhere the engine may raise a replication node.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NodeSiteLayout {
    pub name: String,
    pub at: (f32, f32),
    /// Land sites print land (or air) units; naval sites stand in water and
    /// print ships (or air).
    pub domain: Domain,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SurvivalLayout {
    /// Middle of the Replication Engine.
    pub engine: (f32, f32),
    /// The start position reserved for the engine's side (not offered to defenders).
    pub engine_start: u8,
    /// Open water near the engine where it prints ships. None: no naval fronts.
    pub harbor: Option<(f32, f32)>,
    pub spawns: Vec<SpawnZone>,
    pub fronts: Vec<FrontLayout>,
    pub node_sites: Vec<NodeSiteLayout>,
}

impl SurvivalLayout {
    /// Fronts of one domain.
    pub fn fronts_of(&self, domain: Domain) -> impl Iterator<Item = &FrontLayout> {
        self.fronts.iter().filter(move |f| f.domain == domain)
    }

    /// Something wrong with the layout, for the map author.
    pub fn problem(&self) -> Option<String> {
        if self.spawns.is_empty() {
            return Some("survival: no spawns".into());
        }
        if self.spawns.iter().any(|s| s.start == self.engine_start) {
            return Some("survival: a spawn uses the engine's start".into());
        }
        if self.fronts.iter().any(|f| f.path.is_empty()) {
            return Some("survival: a front has an empty path".into());
        }
        if self.fronts_of(Domain::Naval).next().is_some() && self.harbor.is_none() {
            return Some("survival: naval fronts need a harbor".into());
        }
        None
    }
}

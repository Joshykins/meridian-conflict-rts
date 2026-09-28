//! How a survival map is laid out: the Precursor facility's heart (where the
//! ray that raises Shapers leaves), the print bays built into its halls and
//! its sea gate, the guns guarding it, where a defender may start, which way
//! each front attacks from, and the cradles where Shapers form. Read from the
//! `survival` block of a map's `maps/<stem>.ron`; a map without one is not
//! offered for survival.
//!
//! Positions are metres on the map, x east, y north. Converted exactly to
//! fixed point when a match is set up, so the simulation sees them as part of
//! the match description (and a replay carries them).
//!
//! ```ron
//! survival: (
//!     engine: (12800, 12400),
//!     ray_height: 720,
//!     engine_start: 3,
//!     harbor: (13900, 9800),
//!     bays: [
//!         (at: (11200, 12000), facing: 180, emitter: (11275, 12000, 110)),
//!         (at: (13900, 9800), facing: 200, emitter: (13980, 9830, 140), domain: Naval),
//!     ],
//!     guards: [(key: "aster_t2_point_defense", at: (11000, 12100), facing: 180)],
//!     spawns: [
//!         (name: "Bastion", start: 0, blurb: "High ground, every front reaches it late."),
//!     ],
//!     fronts: [
//!         (name: "Northern Pass", domain: Land, path: [(11800, 12900), (7000, 11000)]),
//!         (name: "Sound", domain: Naval, path: [(13900, 9800), (6000, 4200)]),
//!         (name: "High Corridor", domain: Air, path: [(12000, 12000), (4200, 4200)]),
//!     ],
//!     node_sites: [
//!         (name: "Cinder Ridge", at: (9800, 11800), domain: Land, facing: Some(180)),
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

/// Somewhere the engine may raise a replication node: a cradle holding a row
/// of Shapers.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NodeSiteLayout {
    pub name: String,
    pub at: (f32, f32),
    /// Land sites print land (or air) units; naval sites stand in water and
    /// print ships (or air).
    pub domain: Domain,
    /// Degrees (0 east, counter-clockwise) the Shapers face, their row square
    /// to it: a cradle's facing. None: toward the defenders.
    pub facing: Option<f32>,
}

/// A print bay built into the facility: a printed unit stands at `at` while
/// the beam from `emitter` (x, y, metres over the ground at `at`) builds it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BayLayout {
    pub at: (f32, f32),
    /// Degrees the printed unit faces (0 east, counter-clockwise).
    pub facing: f32,
    pub emitter: (f32, f32, f32),
    /// Land bays print land units (and aircraft when no air bay is free); air
    /// bays (aeries) aircraft; naval bays (slips) ships.
    pub domain: Domain,
    /// The biggest unit (collision radius, metres) the bay takes. None: any.
    /// Units go to the smallest free bay that fits, so the great bays are kept
    /// for the great units.
    pub max_radius: Option<f32>,
}

/// A gun raised for the facility's side when the match begins.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GuardLayout {
    /// Blueprint key.
    pub key: String,
    pub at: (f32, f32),
    pub facing: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SurvivalLayout {
    /// The facility's heart: where its replication ray leaves, and what the
    /// HUD marks as the enemy.
    pub engine: (f32, f32),
    /// Metres over the ground at `engine` the ray leaves from.
    pub ray_height: f32,
    /// The start position reserved for the engine's side (not offered to defenders).
    pub engine_start: u8,
    /// The sea gate, for the set-up screen's marker. None: no naval fronts.
    pub harbor: Option<(f32, f32)>,
    pub bays: Vec<BayLayout>,
    pub guards: Vec<GuardLayout>,
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
        if self.fronts_of(Domain::Naval).next().is_some()
            && !self.bays.iter().any(|b| b.domain == Domain::Naval)
        {
            return Some("survival: naval fronts need a naval bay".into());
        }
        if !self.bays.iter().any(|b| b.domain == Domain::Land) {
            return Some("survival: no land print bays".into());
        }

        None
    }
}

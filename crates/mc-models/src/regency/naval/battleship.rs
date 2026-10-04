//! The Flamberge, the Regency's tech 3 battleship (`regency_t3_battleship`): the
//! Leviathan's place, but it dives. A capital ship drawn to go under: low and long, its
//! superstructure swept back, dark plate lapped over bronze workings (docs/STYLE.md "The
//! Regency look", "The navy").
//!
//! The origin is the waterline; the keel is drawn below it and the sea closes over the
//! whole hull when it dives. The hull is a whaleback arrowhead under one long swept fin
//! ([`body`]); the main houses carry the Sunspear's rails made twin ([`guns`]). It draws
//! the unit file's weapon points:
//!
//! - Houses 0, 1, 2: three twin Pinch-fusion Cannon ([`MAIN`]): fore, a second raised over
//!   it, and aft (`rear`, authored facing forward like every house). Each gathers two
//!   charges side by side ahead of its bores ([`charges`]), held between projectors.
//! - House 3: the Plasmeric AA Repeater on the superstructure's crown ([`AA`]).
//! - Houses 4 to 7: four twin plasmeric repeaters on the beam ([`SECONDARY`]), resting
//!   trained outboard.
//! - 8: four gravitic torpedo doors in the bow's blunt face under the water ([`TUBES`]).
//! - Four counter-seeker heads ([`DEFENCE`], the unit's `anti_missile_mounts`).

use glam::Vec3;

use crate::library::ModelDef;

mod body;
mod guns;
mod hull;
#[cfg(test)]
mod tests;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new(
    "regency_battleship",
    RADIUS,
    HEIGHT,
    body::build,
)];

const RADIUS: f32 = 64.0;
const HEIGHT: f32 = 16.0;

/// The three main houses' pivots (weapons 0, 1, 2): fore, second (raised) and aft.
const MAIN: [Vec3; 3] = [
    Vec3::new(38.0, 0.0, 8.0),
    Vec3::new(20.0, 0.0, 11.0),
    Vec3::new(-36.0, 0.0, 8.0),
];
/// How far ahead of its pivot, and over it, a main gun holds its charges, and how far
/// either side of the house's middle each one is.
const REACH: f32 = 15.0;
const RISE: f32 = 0.4;
const TWIN: f32 = 1.6;

/// The middle of main gun `i`'s pair of charges (the unit file's `muzzle`).
fn charge(i: usize) -> Vec3 {
    MAIN[i] + Vec3::new(REACH, 0.0, RISE)
}

/// Main gun `i`'s two charges (the unit file's `muzzles`), starboard first.
#[cfg(test)]
fn charges(i: usize) -> [Vec3; 2] {
    let c = charge(i);
    [c - Vec3::Y * TWIN, c + Vec3::Y * TWIN]
}

/// The AA house's pivot and the middle of its tube mouths, the bore level (the sim holds
/// it up at the sky at rest).
const AA: Vec3 = Vec3::new(-6.0, 0.0, 15.0);
const AA_MUZZLE: Vec3 = Vec3::new(-3.2, 0.0, 15.0);

/// The beam secondaries' pivots, in weapon order (4 to 7): port fore, port aft, starboard
/// fore, starboard aft. Each holds two muzzles [`SECONDARY_REACH`] ahead, `SECONDARY_TWIN`
/// either side.
const SECONDARY: [Vec3; 4] = [
    Vec3::new(6.0, 9.0, 7.0),
    Vec3::new(-18.0, 9.0, 7.0),
    Vec3::new(6.0, -9.0, 7.0),
    Vec3::new(-18.0, -9.0, 7.0),
];
const SECONDARY_REACH: Vec3 = Vec3::new(5.0, 0.0, 0.2);
const SECONDARY_TWIN: f32 = 0.5;

/// The torpedo doors in the bow's face, as in the unit file, and the face itself.
const TUBES: [[f32; 3]; 4] = [
    [62.0, -1.2, -1.7],
    [62.0, 1.2, -1.7],
    [62.0, -1.2, -2.9],
    [62.0, 1.2, -2.9],
];
const BOW_FACE: f32 = 61.9;

/// The counter-seeker heads: a pair forward on the superstructure, a pair aft.
const DEFENCE: [Vec3; 4] = [
    Vec3::new(-1.0, 5.0, 13.0),
    Vec3::new(-1.0, -5.0, 13.0),
    Vec3::new(-20.0, 5.0, 10.5),
    Vec3::new(-20.0, -5.0, 10.5),
];

//! The Regent Yard (`regency_t5_sky_yard`, mesh "regency_sky_yard"): the tech 5 structure
//! the Regent is raised from, authored at its unit file's size (metres; a 24 x 24 lot of
//! 12 m cells, 288 m square, 300 m high).
//!
//! The Regent is built in the sky over the yard, at its cruise height, on one great
//! nanite beam the yard casts straight up from [`EMITTER`] (the unit file's first
//! emitter; the renderer draws the beam). Helpers' streams pour into four intakes round
//! its foot ([`INTAKE_R`], [`INTAKE_Z`]: its other emitters). The yard puts its emitter
//! and intakes there.
//!
//! The design (`bore.rs`, the user's pick): a bore ringed by a low wall, eight
//! blade-shards flaring outward round it, and a mast of rotor drums rising out of it to
//! the emitter.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plate over dark graphite works, red
//! optics where it looks out (`pattern::EMBER_SLOT`), violet only on what builds: the
//! emitter's crown and the intakes' rims (`pattern::NANITE_RIM`, `NANITE_SLOT`), light in
//! the plates and never a lit piece of mesh.

mod bore;
mod ground;
#[cfg(test)]
mod tests;

use glam::Vec3;

pub(super) use bore::bore;

/// The unit file's size.
pub(super) const RADIUS: f32 = 140.0;
pub(super) const HEIGHT: f32 = 300.0;
/// Where the great beam leaves for the hull, model metres: the unit file's first emitter.
pub(super) const EMITTER: Vec3 = Vec3::new(0.0, 0.0, 292.0);
/// The four intakes helpers pour into: how far out from the middle (along x and y, both
/// ways) and how high. The unit file's other emitters.
pub(super) const INTAKE_R: f32 = 58.0;
pub(super) const INTAKE_Z: f32 = 46.0;

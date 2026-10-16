//! The Lens (`regency_gravitic_lens`): the Regency's tech 3 gravitic sniper, a small mobile
//! cousin of the Fallen, their tech 5 gravitic engine, and their answer to ARC's Arbalest.
//! It crouches on six great black shards planted wide round it like a crab's legs
//! ([`legs`]). The body hangs low between them ([`body`]), an open well in its back where
//! rotor rings turn, and three tall prongs rise off the well's lip to hold a knot of
//! gravity between their points at [`CORE`].
//!
//! It walks in two tripods, three feet always down, its body rolling a little over the
//! side whose feet are down. It plants to fire (`deploy`): each leg steps out in turn,
//! then the body settles to the ground between them; packing runs it back. Once down, it
//! charges the core and throws one front of gravity from it to its mark.
//!
//! Authored planted, at the size the unit file gives it, the gravity held at [`CORE`] (the
//! unit file's `muzzle` and `pivot`). The gravity itself is not part of the model (renderer
//! `lens_fx`): nothing comes within [`CORE_CLEAR`] of it, and the light is in the plates
//! (`pattern::NANITE_SLOT` on the shards' inner faces, the prongs and the well), never a
//! lit piece.

mod body;
mod legs;
#[cfg(test)]
mod tests;

use glam::Vec3;

use crate::builder::MeshBuilder;

/// The unit file's size, planted.
pub(super) const RADIUS: f32 = 8.5;
pub(super) const HEIGHT: f32 = 7.5;

/// Where the Lens holds its gravity, planted: high over its middle, between the prongs.
pub(super) const CORE: Vec3 = Vec3::new(0.0, 0.0, 5.9);
/// How far round the core the model leaves open for the gravity.
#[cfg(test)]
pub(super) const CORE_CLEAR: f32 = 0.85;

pub(super) fn lens(b: &mut MeshBuilder, _tech: u8) {
    b.set_dust_line(0.4);
    legs::legs(b);
    body::body(b);
}

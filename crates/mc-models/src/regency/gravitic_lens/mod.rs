//! The Lens (`regency_gravitic_lens`): the Regency's tech 3 gravitic sniper, a small mobile
//! cousin of the Fallen, their tech 5 gravitic engine, and their answer to ARC's Arbalest.
//! Its legs are its crown: four great black shards stand on their boots round it and lean
//! in to meet over its middle ([`legs`]), and between their points, under the knees, it
//! holds a knot of gravity at [`CORE`]. The body hangs low between them ([`body`]), an
//! open well in its back where rotor rings turn, three prongs leaning up off its lip
//! toward the core.
//!
//! It walks one foot at a time, three always down, its body rolling over the side whose
//! feet are down and dipping to each as it lands. It plants to fire (`deploy`): each leg
//! steps out in turn, then the body settles to the ground between them; packing runs it
//! back. Once down, it throws a
//! gravitic link from the core to its mark and holds a small well there.
//!
//! Authored planted, at the size the unit file gives it, the gravity held at [`CORE`] (the
//! unit file's `muzzle` and `pivot`). The gravity itself is not part of the model (renderer
//! `lens_fx`): nothing comes within [`CORE_CLEAR`] of it, and the light is in the plates
//! (`pattern::NANITE_SLOT` on the shards' inner faces and the well), never a lit piece.

mod body;
mod legs;
#[cfg(test)]
mod tests;

use glam::Vec3;

use crate::builder::MeshBuilder;

/// The unit file's size, planted.
pub(super) const RADIUS: f32 = 8.5;
pub(super) const HEIGHT: f32 = 7.5;

/// Where the Lens holds its gravity, planted: high over its middle, under the knees.
pub(super) const CORE: Vec3 = Vec3::new(0.0, 0.0, 5.9);
/// How far round the core the model leaves open for the gravity.
#[cfg(test)]
pub(super) const CORE_CLEAR: f32 = 0.85;

pub(super) fn lens(b: &mut MeshBuilder, _tech: u8) {
    b.set_dust_line(0.4);
    legs::legs(b);
    body::body(b);
}

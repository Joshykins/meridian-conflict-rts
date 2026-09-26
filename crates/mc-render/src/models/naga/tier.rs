//! Tiers of a Naga structure that upgrades in place (`upgrades_to`): each tier adds its
//! machinery to the last, and the next tier's pieces go up during the refit.

use crate::models::builder::MeshBuilder;

/// Runs `f` for tier `tier`'s pieces of a structure drawn at `tech`: as they stand once
/// that tier is built, or, for the next tier up, as refit pieces hidden until the upgrade
/// begins and raised `at` (zero to one) of the way through it. Only at full detail: far
/// off, the refit shows as the finished tier.
pub(super) fn tier(
    b: &mut MeshBuilder,
    tech: u8,
    tier: u8,
    at: f32,
    f: impl FnOnce(&mut MeshBuilder),
) {
    if tier <= tech {
        f(b);
    } else if tier == tech + 1 && b.fine() {
        b.upgrade(at, f);
    }
}

//! Design A (`fabricator`): one tall pressure vessel in the middle of the lot, stacked a
//! stage higher at each tier, its field coils round it; the capacitor banks on the grid
//! side (-x) and the hopper on the far side (+x).
//! - Tech 1: the vessel, two coils, one bank, the hopper.
//! - Tech 2: an upper stage with its own coils inside a braced containment frame, banks
//!   on both flanks, radiators either side of the hopper.
//! - Tech 3: a third stage and a crown, four corner pylons feeding it, a second bank row
//!   and more cooling.

use glam::Vec3;

use super::super::parts::*;
use super::*;

/// The vessel's stages: (radius, foot, top of the straight body). Each stands on the
/// hatch of the one below.
const STAGES: [(f32, f32, f32); 3] = [(6.0, DECK, 9.5), (4.5, 12.3, 17.0), (3.2, 19.35, 23.6)];

/// Where a stage's hatch lid is: its shoulder plus the collar.
fn hatch(stage: usize) -> f32 {
    let (r, _, z1) = STAGES[stage];
    z1 + r * 0.35 + 0.7
}

pub(in crate::aster) fn vessel_plant(b: &mut MeshBuilder, tech: u8) {
    standalone(b, tech, plant);
}

/// The plant drawn with every tier's machinery up to `tech`, at the 4x4 lot's scale.
fn plant(b: &mut MeshBuilder, tech: u8) {
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    slab(b);
    deck_marks(b, 19.6);

    // ---- Tech 1: the vessel, its coils, one bank and the hopper.
    let (r, z0, z1) = STAGES[0];
    vessel(b, Vec3::ZERO, r, z0, z1);
    for z in [3.6, 7.0] {
        coil(b, v3(0.0, 0.0, z), r + 0.02, 0.9, 1.0);
    }
    bank(b, v3(-15.5, 0.0, DECK), 4, 6.5);
    for y in [-2.6, 2.6] {
        conduit(
            b,
            &[v3(-13.6, y, 3.0), v3(-9.0, y, 3.0), v3(-6.6, y * 0.6, 3.0)],
            0.9,
        );
    }
    hopper(b, v3(6.2, 0.0, 5.4), v3(14.5, 0.0, DECK), 3.4);
    if b.fine() {
        relief(b, v3(0.0, 0.0, hatch(0)), 1.5);
    }

    // ---- Tech 2: the upper stage in its containment frame, banks on the flanks, cooling.
    fitted(b, tech, 2, |b| {
        b.radial(4, |b| {
            b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
                b.paint(PLATING_DARK);
                b.chamfered_box(v3(15.0, 0.0, DECK + 0.5), v3(3.0, 3.0, 1.0), 0.5);
                brace(b, v3(15.0, 0.0, DECK + 0.8), v3(5.0, 0.0, 17.6), 1.1);
            });
        });
    });
    fitted(b, tech, 2, |b| {
        let (r, z0, z1) = STAGES[1];
        vessel(b, Vec3::ZERO, r, z0, z1);
        for z in [13.8, 15.8] {
            coil(b, v3(0.0, 0.0, z), r + 0.02, 0.8, 0.9);
        }
        // The frame's collar round the stage's top.
        b.paint(ACCENT);
        b.at(Vec3::ZERO, |b| {
            annulus_on(b, Vec3::ZERO, Vec3::Z, 8, 4.9, 5.9, 17.2, 18.2)
        });
    });
    fitted(b, tech, 2, |b| {
        for s in [-1.0f32, 1.0] {
            b.yawed(Vec3::ZERO, s * std::f32::consts::FRAC_PI_2, |b| {
                bank(b, v3(-15.5, 0.0, DECK), 3, 7.5);
                conduit(b, &[v3(-13.6, 0.0, 3.0), v3(-6.6, 0.0, 3.0)], 0.9);
            });
        }
    });
    fitted(b, tech, 2, |b| {
        for y in [-7.0, 7.0] {
            radiator(b, v3(9.0, y, DECK), Vec3::X, 10.5, 2.6);
        }
    });

    // ---- Tech 3: a third stage under a crown, corner pylons feeding it, more banks.
    fitted(b, tech, 3, |b| {
        b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
            b.radial(4, |b| pylon(b, 23.0));
        });
    });
    fitted(b, tech, 3, |b| {
        let (r, z0, z1) = STAGES[2];
        vessel(b, Vec3::ZERO, r, z0, z1);
        coil(b, v3(0.0, 0.0, 20.6), r + 0.02, 0.8, 0.9);
        coil(b, v3(0.0, 0.0, 22.2), r + 0.02, 0.8, 0.9);
        // The crown: a heavy ring the pylons' feeds come in to.
        b.paint(PLATING_DARK);
        annulus_on(b, Vec3::ZERO, Vec3::Z, 8, 3.6, 6.4, 24.6, 26.0);
        b.paint(GLOW).pattern(pattern::CHARGE);
        annulus_on(b, Vec3::ZERO, Vec3::Z, 8, 6.4, 6.48, 24.9, 25.7);
        if b.fine() {
            relief(b, v3(0.0, 0.0, hatch(2)), 2.4);
        }
    });
    fitted(b, tech, 3, |b| {
        b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
            b.radial(4, |b| {
                conduit(b, &[v3(21.6, 0.0, 25.0), v3(6.3, 0.0, 25.0)], 0.8);
            });
        });
    });
    fitted(b, tech, 3, |b| {
        bank(b, v3(-19.4, 0.0, DECK), 5, 9.0);
        for y in [-7.0, 7.0] {
            radiator(b, v3(-12.0, y, DECK), Vec3::X, 5.5, 2.4);
        }
    });
}

/// A corner pylon out along +x to `out` (on the diagonal): a plated column with a lit
/// band near the top, standing on a dark foot, and its feed down from the head.
fn pylon(b: &mut MeshBuilder, out: f32) {
    let at = v3(out, 0.0, DECK);
    b.paint(PLATING_DARK);
    b.chamfered_box(at + Vec3::Z * 0.6, v3(3.4, 3.4, 1.2), 0.6);
    b.paint(PLATING);
    b.frustum(
        at + Vec3::Z * 1.2,
        v2(2.6, 2.6),
        v2(2.0, 2.0),
        22.6,
        glam::Vec2::ZERO,
    );
    b.paint(GLOW).pattern(pattern::NONE);
    b.cuboid(at + Vec3::Z * 20.6, v3(2.18, 2.18, 0.5));
    b.paint(ACCENT);
    b.chamfered_box(at + Vec3::Z * 24.4, v3(2.8, 2.8, 1.2), 0.4);
}

/// Pressure relief on a hatch at `top`: two short stacks with dark caps, `h` tall.
fn relief(b: &mut MeshBuilder, top: Vec3, h: f32) {
    for y in [-0.9, 0.9] {
        b.paint(METAL);
        b.prism(top + v3(0.0, y, 0.0), 6, 0.35, 0.35, h);
        b.paint(ACCENT);
        b.prism(top + v3(0.0, y, h), 6, 0.5, 0.45, 0.3);
    }
}

/// The coarse level: the slab, the vessel as one stepped block to the tier's height, the
/// bank and the hopper as boxes.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    slab(b);
    deck_marks(b, 19.6);
    let top = hatch(tech as usize - 1);
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, DECK),
        v2(13.0, 13.0),
        v2(
            STAGES[tech as usize - 1].0 * 1.3,
            STAGES[tech as usize - 1].0 * 1.3,
        ),
        top - DECK,
        glam::Vec2::ZERO,
    );
    team_panel(b, v3(0.0, 0.0, top), v2(2.4, 2.4));
    bank(b, v3(-15.5, 0.0, DECK), 4, 6.5);
    hopper(b, v3(6.2, 0.0, 5.4), v3(14.5, 0.0, DECK), 3.4);
}

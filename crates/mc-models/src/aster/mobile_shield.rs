//! Mobile shield generator (tech 2): the Aegis's field carried with the army.
//! Unarmed. It speaks the Aegis's language at vehicle scale: a white hexagonal
//! column, a blue faceted crystal turning inside a dark cage, a wreath of lit
//! pods orbiting it, coil drums feeding it from the deck.
//!
//! The dome's shaft is born at the crystal's crown: the unit file's
//! `effects: (shield_projector: ..)` sits there.

use super::parts::*;
use super::structures::projector_wreath;
use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

/// The Javelin's tracked chassis, a little longer.
const TRACKED: Chassis = Chassis {
    rear: -4.7,
    front: 4.7,
    track: (1.95, 3.25, 1.32),
    split_tracks: false,
    deck: 1.85,
    dark: false,
    lit: false,
};

/// A row of coil drums across the deck at `x`, under a white lid, with a dark
/// conduit trunk running back to `toward` (the generator's x).
fn coil_bank(b: &mut MeshBuilder, x: f32, z: f32, toward: f32) {
    b.paint(ACCENT);
    b.chamfered_box(v3(x, 0.0, z + 0.45), v3(1.5, 3.0, 0.9), 0.15);
    if b.coarse() {
        return;
    }
    b.paint(METAL);
    let sides = b.sides(10);
    for dx in [-0.42, 0.42] {
        b.cylinder_between(
            v3(x + dx, -1.3, z + 1.05),
            v3(x + dx, 1.3, z + 1.05),
            0.3,
            0.3,
            sides,
        );
    }
    b.paint(PLATING);
    b.plate(v3(x, 0.0, z + 1.3), v2(1.3, 1.6), 0.08, 0.03);
    b.paint(PLATING_DARK).pattern(pattern::CONDUIT);
    let (x0, x1) = if toward < x {
        (toward, x - 0.75)
    } else {
        (x + 0.75, toward)
    };
    b.block(v3(x0, -0.42, z), v3(x1, 0.42, z + 0.5));
    if b.fine() {
        b.paint(GLOW);
        b.mirror_y(|b| b.block(v3(x - 0.5, 1.5, z + 0.25), v3(x + 0.5, 1.53, z + 0.45)));
    }
}

/// A caged crystal column on the hull origin, from the deck at `z0` to `top`:
/// a dark hex plinth, white column shoulders with slit windows onto the crystal,
/// and a crown of petals aiming the field up. The crystal turns (`SPINNER`) about
/// the column's axis, the wreath with it. Far off it is a white column and a lit tip.
fn caged_column(b: &mut MeshBuilder, z0: f32, top: f32, r: f32) {
    let crown = top - r * 0.9;
    b.set_spinner_pivot(v3(0.0, 0.0, z0));
    if b.coarse() {
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, z0), 4, r, r * 0.8, crown - z0);
        b.paint(GLOW);
        b.prism(v3(0.0, 0.0, crown), 3, r * 0.5, r * 0.2, top - crown);
        return;
    }
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, z0), 6, r * 1.45, r * 1.3, 0.45);
    b.with_part(part::SPINNER, |b| {
        b.paint(GLOW);
        b.prism(
            v3(0.0, 0.0, z0 + 0.4),
            6,
            r * 0.55,
            r * 0.45,
            crown - z0 - 0.3,
        );
    });
    // Lower shoulder and upper collar in white; between them the cage is open.
    b.paint(PLATING);
    b.loft_z(
        &ngon(6, r),
        &[Section::new(z0 + 0.45, 1.0), Section::new(z0 + 1.1, 0.95)],
    );
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, crown - 0.2), 6, r * 0.95, r, 0.25);
    b.radial(6, |b| {
        b.paint(ACCENT);
        b.beam(
            v3(r * 0.9, 0.0, z0 + 1.0),
            v3(r * 0.86, 0.0, crown - 0.1),
            v2(0.16, 0.12),
            v2(0.12, 0.1),
        );
    });
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, crown + 0.05), 6, r * 0.95, r * 0.7, 0.55);
    b.paint(GLOW);
    b.prism(
        v3(0.0, 0.0, crown + 0.6),
        6,
        r * 0.42,
        r * 0.22,
        top - crown - 0.6,
    );
    if b.fine() {
        b.radial(6, |b| {
            b.paint(PLATING);
            b.pitched(v3(r * 0.62, 0.0, crown + 0.5), 0.9, |b| {
                b.extrude_y_chamfered(
                    &[[-0.1, 0.0], [0.7, 0.0], [0.55, 0.4], [-0.05, 0.5]],
                    0.18,
                    0.05,
                );
            });
        });
    }
}

// The Javelin's tracked hull carrying a caged crystal column on its middle, the
// wreath orbiting at its waist and a coil bank aft feeding it. The column stands
// on the hull origin: every dome's shaft rises from there.

pub(super) fn mobile_shield(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(b, &TRACKED);
    team_panel(b, deck.at(0.92, 0.0), v2(0.8, 2.2));
    caged_column(b, deck.z, 6.4, 1.15);
    if b.coarse() {
        return;
    }
    coil_bank(b, -3.0, deck.z, 0.0);
    b.with_part(part::SPINNER, |b| projector_wreath(b, 3.9, 1.85, 0.32));
    side_pods(b);
}

/// Electronics pods over the track sponsons.
fn side_pods(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.block(v3(-3.6, 2.05, 1.35), v3(-0.6, 2.42, 2.1));
        if b.fine() {
            vent(b, v3(-2.1, 2.24, 2.1), v2(1.2, 0.48), 3, METAL);
            antenna_unlit(b, v3(-3.3, 2.2, 2.1), 1.0, 0.15);
        }
    });
}

//! The Censer, the Regency's tech 3 field mender: a low armoured hover hull on four lift
//! bells, carrying no gun, only a nanite spire over its middle that lets out the repair
//! field (`repair_field`, `mc-sim` `repair_field.rs`). The field's motes leave from high
//! over the unit's middle (`mirror::mend`), so the spire stands there.
//!
//! The hull is a faceted lozenge: two courses of plates swept back down each flank, a
//! glacis and chin plate over the nose, skirts over the lift bells, a plated housing aft
//! of the spire with bronze feeds running up to it. The spire is a plated core wrapped in
//! keeled courses pointing up, each fanning a wing up and out either side, longest at
//! the foot, under a split crest. Four keeled plates are lapped round the spire's race,
//! a violet gasket round each. The violet is the plates' own (`kit::nanite_plate`,
//! `pattern::NANITE_RIM`), the red optics are ember slots: no lit pieces.
//!
//! Rig: a hovercraft (`MeshBuilder::set_hover`): the shader heaves the hull over its lift
//! bells (`part::LOCOMOTION`), each marked for its red plasma (`add_lift`). Nothing turns.
//! The numbers match `regency_t3_field_mender` in `data/factions/regency/units/land_t3.ron`.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

use super::commander::form::{blade, ring, sleeve, KEEL, OCT};
use super::kit::{cable, dark_plate, metal, nanite_plate, seam, slab, v3};
use super::machine::{armour, swept, Frame};
pub(super) const RADIUS: f32 = 6.0;
pub(super) const HEIGHT: f32 = 7.0;
/// The deck the emitter stands on.
const DECK: f32 = 2.4;
/// The lift drums under the left side (x, y); the right's are their mirrors.
const LIFTS: [(f32, f32); 2] = [(3.0, 2.3), (-3.3, 2.5)];
/// Where each lift bell's mouth opens.
const MOUTH: f32 = 0.25;

/// The hull's plan: a lozenge, pointed fore, its stern cut square between two corners.
const HULL: [[f32; 2]; 9] = [
    [5.8, 0.0],
    [3.6, 2.9],
    [-1.2, 3.6],
    [-4.6, 3.0],
    [-5.4, 1.2],
    [-5.4, -1.2],
    [-4.6, -3.0],
    [-1.2, -3.6],
    [3.6, -2.9],
];

pub(super) fn censer(b: &mut MeshBuilder, _tech: u8) {
    b.set_hover();
    b.set_dust_line(0.8);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull(b);
    b.mirror_y(flank);
    lifts(b);
    stern(b);
    deck(b);
    spire(b);
}

/// Far off: the lozenge, the team colour on its nose, the spire a slotted post.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    let lozenge = [[5.8, 0.0], [-1.2, 3.6], [-5.4, 0.0], [-1.2, -3.6]];
    b.loft_z(&lozenge, &[Section::new(0.9, 0.9), Section::new(DECK, 0.7)]);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(4.0, 0.0, 0.5), v3(-4.0, -2.6, 0.5), v3(-4.0, 2.6, 0.5)]);
    });
    b.paint(TEAM);
    b.face(&[
        v3(3.6, 0.0, DECK + 0.02),
        v3(1.6, 1.0, DECK + 0.02),
        v3(1.6, -1.0, DECK + 0.02),
    ]);
    nanite_plate(b);
    b.beam(
        v3(0.0, 0.0, DECK),
        v3(0.0, 0.0, HEIGHT),
        Vec2::new(1.6, 1.6),
        Vec2::new(0.4, 0.4),
    );
}

/// The hull: a flat-faceted lozenge over a seam-dark belly, a glacis over the nose and a
/// chin plate under it, a red optic either side, the team colour on the deck.
fn hull(b: &mut MeshBuilder) {
    seam(b);
    b.loft_z(&HULL, &[Section::new(0.75, 0.82), Section::new(1.0, 0.86)]);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &HULL,
            &[
                Section::new(0.95, 0.88),
                Section::new(1.5, 1.0),
                Section::new(2.1, 0.88),
                Section::new(DECK, 0.6).shifted(-0.4, 0.0),
            ],
        );
    });
    blade(
        b,
        v3(5.4, 0.0, 1.7),
        v3(-1.0, 0.0, 0.3),
        v3(0.35, 0.0, 1.0),
        3.2,
        1.4,
        0.0,
        0.32,
    );
    // The chin: a keeled plate under the glacis, its point forward.
    blade(
        b,
        v3(3.4, 0.0, 1.05),
        v3(1.0, 0.0, 0.12),
        v3(0.3, 0.0, -1.0),
        2.6,
        1.2,
        0.0,
        0.28,
    );
    // An optic either side of the nose: a small plate with a red slot recessed in it.
    b.mirror_y(|b| {
        b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
        let out = v3(0.6, 1.0, 0.0).normalize();
        let along = v3(-1.0, 0.6, 0.0).normalize();
        let at = v3(3.85, 2.42, 1.45);
        slab(
            b,
            [
                at - along * 0.7 - Vec3::Z * 0.22,
                at + along * 0.7 - Vec3::Z * 0.22,
                at + along * 0.7 + Vec3::Z * 0.22,
                at - along * 0.7 + Vec3::Z * 0.22,
            ],
            out * 0.14,
        );
    });
    b.paint(TEAM);
    b.face(&[
        v3(2.9, 0.0, DECK + 0.03),
        v3(1.8, 0.75, DECK + 0.03),
        v3(1.8, -0.75, DECK + 0.03),
    ]);
}

/// The left flank's plates: two courses swept back, the upper running out past the stern
/// into a spike.
fn flank(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(3.6, 2.9, 1.95),
        v3(-1.0, 0.12, -0.04),
        v3(0.0, 1.0, 0.8),
        4.6,
        0.7,
        0.2,
        0.3,
    );
    blade(
        b,
        v3(1.2, 3.55, 1.6),
        v3(-1.0, 0.08, -0.05),
        v3(0.0, 1.0, 0.6),
        7.0,
        0.75,
        0.3,
        0.32,
    );
}

/// Four bronze collars under the hull, a lift bell in each marked for its plasma, a skirt
/// plate over each's outboard side.
fn lifts(b: &mut MeshBuilder) {
    let drum = |b: &mut MeshBuilder, x: f32, y: f32, r: f32| {
        b.add_lift(v3(x, y, MOUTH), r * 0.7);
        metal(b);
        let sides = b.sides(12);
        b.prism(v3(x, y, 0.62), sides, r, r * 0.95, 0.36);
        b.with_part(part::LOCOMOTION, |b| {
            seam(b);
            b.prism(v3(x, y, 0.34), sides, r * 0.86, r * 0.84, 0.5);
            b.prism(v3(x, y, MOUTH), sides, r * 0.55, r * 0.7, 0.09);
        });
        if b.fine() {
            // The skirt: a short plate hung down the bell's outboard side.
            dark_plate(b);
            blade(
                b,
                v3(x + 0.9, y + 1.0, 1.15),
                v3(-1.0, 0.0, -0.1),
                v3(0.0, 1.0, -0.35),
                1.9,
                0.42,
                0.4,
                0.2,
            );
        }
    };
    b.mirror_y(|b| {
        for (x, y) in LIFTS {
            drum(b, x, y, 0.95);
        }
    });
}

/// Aft of the spire: a low plated housing across the deck, its back cut with a red slot,
/// two bronze drums out of it and a feed either side running up to the spire's race.
fn stern(b: &mut MeshBuilder) {
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [-2.2, 0.0],
                [-2.6, 1.5],
                [-4.4, 1.7],
                [-4.9, 0.0],
                [-4.4, -1.7],
                [-2.6, -1.5],
            ],
            &[
                Section::new(DECK - 0.4, 1.0),
                Section::new(DECK + 0.45, 0.95),
                Section::new(DECK + 0.75, 0.72).shifted(-0.25, 0.0),
            ],
        );
    });
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
    slab(
        b,
        [
            v3(-4.75, -1.1, DECK - 0.15),
            v3(-4.75, 1.1, DECK - 0.15),
            v3(-4.75, 1.1, DECK + 0.35),
            v3(-4.75, -1.1, DECK + 0.35),
        ],
        v3(-0.14, 0.0, 0.0),
    );
    if b.fine() {
        b.mirror_y(|b| {
            metal(b);
            b.cylinder_between(
                v3(-3.0, 0.95, DECK + 0.9),
                v3(-4.4, 0.95, DECK + 0.75),
                0.32,
                0.3,
                10,
            );
            seam(b);
            b.cylinder_between(
                v3(-4.4, 0.95, DECK + 0.75),
                v3(-4.55, 0.95, DECK + 0.74),
                0.26,
                0.2,
                10,
            );
            metal(b);
            cable(
                b,
                &[
                    v3(-2.9, 1.75, DECK + 0.1),
                    v3(-2.0, 1.8, DECK + 0.05),
                    v3(-1.0, 1.75, DECK + 0.15),
                ],
                0.12,
            );
        });
    }
}

/// The deck: four keeled plates lapped round the race on the diagonals, their
/// points outboard, each with a violet gasket round its face.
fn deck(b: &mut MeshBuilder) {
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        let out = v3(a.cos(), a.sin(), 0.0);
        b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
        blade(
            b,
            out * 1.75 + Vec3::Z * (DECK + 0.12),
            out - Vec3::Z * 0.22,
            (Vec3::Z + out * 0.22).normalize(),
            2.3,
            0.85,
            0.15,
            0.26,
        );
    }
}

/// A bronze race the spire stands on.
fn race(b: &mut MeshBuilder, r: f32) {
    metal(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, r, r * 0.96, 0.3);
}

/// A keeled prong from `root` to a point at `tip`, keeled toward `out`, slotted.
fn prong(b: &mut MeshBuilder, root: Vec3, tip: Vec3, out: Vec3, w: f32) {
    nanite_plate(b);
    sleeve(
        b,
        &[
            ring(root, out, w, w * 0.8),
            ring(root.lerp(tip, 0.55), out, w * 0.7, w * 0.6),
            ring(tip, out, 0.03, 0.03),
        ],
        &KEEL,
    );
}

/// The spire: a plated core up the middle, three keeled courses laid round it fore and
/// aft pointing up, each fanning a slotted wing up and out either side, longest at the
/// foot, and a split crest on top.
fn spire(b: &mut MeshBuilder) {
    race(b, 1.9);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(0.0, 0.0, DECK + 0.2), Vec3::X, 1.3, 1.1),
            ring(v3(0.0, 0.0, 4.4), Vec3::X, 1.0, 0.85),
            ring(v3(0.0, 0.0, 5.9), Vec3::X, 0.6, 0.5),
        ],
        &OCT,
    );
    let courses = if b.fine() { 3 } else { 2 };
    for k in 0..courses {
        let kf = k as f32;
        let z = 3.0 + 1.1 * kf;
        let half = 0.55 - 0.05 * kf;
        let r = 1.05 - 0.15 * kf;
        for s in [-1.0f32, 1.0] {
            // Fore and aft faces of the stack, leaning in to the core as they rise:
            // long and straight-sided, so the slot runs up the spike, not across it.
            nanite_plate(b);
            armour(
                b,
                &Frame::new(v3(r * s, 0.0, z), v3(-0.2 * s, 0.0, 1.0), v3(s, 0.0, 0.2)),
                &swept(1.9, half, 0.0, 0.7),
                0.36,
            );
        }
        b.mirror_y(|b| {
            let root = v3(0.0, r * 0.75, z + 0.25);
            let u = v3(0.0, 0.8, 0.6).normalize();
            let n = v3(0.3, -0.6, 0.8).normalize();
            nanite_plate(b);
            armour(
                b,
                &Frame::new(root, u, n),
                &swept(2.4 - 0.35 * kf, 0.38, 0.0, 0.7),
                0.3,
            );
        });
    }
    for s in [-1.0f32, 1.0] {
        prong(
            b,
            v3(0.0, 0.25 * s, 5.8),
            v3(-0.3, 0.65 * s, HEIGHT),
            Vec3::Y * s,
            0.32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check("regency_censer", RADIUS, HEIGHT, None, &[]);
    }

    #[test]
    fn the_unit_file_is_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t3_field_mender").unwrap());
        assert_eq!(bp.visual.mesh, "regency_censer");
        assert!(bp.weapons.is_empty(), "it carries no gun");
        assert!(bp.repair_field.is_some(), "it carries the field");
        assert_eq!(
            bp.motion.map(|m| m.layer),
            Some(mc_data::MoveLayer::Hover),
            "it rides its lift"
        );
        let model = build_model("regency_censer").unwrap();
        assert!(model.hover);
        assert_eq!(model.lifts.len(), 4);
    }
}

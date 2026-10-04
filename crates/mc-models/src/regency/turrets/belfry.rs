//! The Belfry (tech 3 anti-air, a 2 x 2 lot): a Gravitic Seeker Silo. From above a star:
//! four keeled arms swept out from an octagonal core, two of them carrying a raised spine
//! of four hatched cells (`CellBlock`s, dark lids over red rims, a seeker in each with its
//! head lit), the other two a tall swept plate. On the core its fire-control head: a
//! stepped cap, a sensor drum with a red optic on each diagonal, and plates lapped up round
//! it into a point. Nothing yaws: the seekers leave their cells straight up.
//!
//! Its muzzles are the unit file's (`data/factions/regency/units/structures.ron`), each
//! under its cell's deck in firing order.

use glam::{Vec2, Vec3};

use crate::builder::{chamfered_rect, CellGrid, MeshBuilder, Section};
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::*;

const THICK: f32 = 0.35;
/// How far under its cell's deck a seeker is launched from: half its body down.
#[cfg(test)]
const MUZZLE_DROP: f32 = 2.0;
/// A row's firing order: the ends first, then the middle, so a salvo ripples along it.
const FIRE: [(u8, u8); 4] = [(0, 0), (3, 0), (1, 0), (2, 0)];
/// The two rows of four along x, the one at +x first (the unit file's muzzles).
const CELLS: [Cells; 2] = [Cells::new(6.6), Cells::new(-6.6)];
/// The core's top, where the fire-control head stands.
const CORE_TOP: f32 = 8.9;

/// A row of four cells along x: its middle, the spacing, the hatches' deck and the
/// armoured spine's foot under it.
#[derive(Clone, Copy)]
struct Cells {
    centre: Vec2,
    pitch: f32,
    deck: f32,
    foot: f32,
}

impl Cells {
    const fn new(x: f32) -> Self {
        Self {
            centre: Vec2::new(x, 0.0),
            pitch: 1.6,
            deck: 6.6,
            foot: 4.4,
        }
    }

    fn half(&self) -> Vec2 {
        Vec2::new(self.pitch * 2.0 + 0.45, self.pitch * 0.5 + 0.45)
    }

    fn grid(&self) -> CellGrid {
        CellGrid {
            centre: self.centre,
            deck: self.deck,
            pitch: self.pitch,
            half: self.pitch * 0.42,
            nx: 4,
            ny: 1,
            hinge_y: true,
        }
    }
}

pub(crate) fn belfry(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse_silo(b);
        return;
    }
    star(b);
    for c in &CELLS {
        cells(b, c);
    }
    head(b);
}

/// Far off: a square plinth, the core up to its top, the head's point over it, the
/// owner's colour on the core.
fn coarse_silo(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, 14.6, 13.8, 1.0);
    b.prism(Vec3::Z * 1.0, 4, 7.0, 5.0, CORE_TOP - 1.0);
    b.prism(Vec3::Z * CORE_TOP, 4, 3.6, 0.3, 4.0);
    team_patch(b, -2.0, 2.0, 1.2, CORE_TOP + 0.01);
}

/// A plate standing at `o` on a wall facing `out`, leaning out by `lean` as it rises (in,
/// when negative), `len` tall and `half` wide, swept up into a point.
fn blade(b: &mut MeshBuilder, o: Vec3, out: Vec3, lean: f32, len: f32, half: f32, thick: f32) {
    dark_plate(b);
    armour(
        b,
        &Frame::new(o, Vec3::Z + out * lean, out),
        &swept(len, half, 0.0, 0.5),
        thick,
    );
}

/// The star: an octagonal core on four keeled arms swept out on the square, two of them
/// carrying the armoured spine of a row of cells, the other two a tall swept plate standing
/// on each; red slots on the core, the owner's colour round its top.
fn star(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    b.prism(Vec3::ZERO, 8, 5.4, 4.2, CORE_TOP - 0.3);
    seam(b);
    b.prism(Vec3::Z * (CORE_TOP - 0.3), 8, 4.2, 4.0, 0.3);
    for k in 0..4 {
        b.yawed(Vec3::ZERO, (90.0 * k as f32).to_radians(), |b| {
            buttress(b, 3.0, 4.4, 5.0, 11.6, 1.4, 1.0);
            if k % 2 == 1 {
                blade(b, v3(5.4, 0.0, 3.6), Vec3::X, 0.25, 6.6, 1.3, THICK);
            }
        });
        let a = (45.0 + 90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        if fine {
            slit(b, d * 4.85 + Vec3::Z * 4.6, d, Vec3::Z, 3.0, 0.26);
        }
    }
    for c in &CELLS {
        let half = c.half() + Vec2::new(0.3, 0.4);
        dark_plate(b);
        b.at(c.centre.extend(0.0), |b| {
            b.loft_z(
                &chamfered_rect(half, 0.8),
                &[Section::new(2.0, 1.15), Section::new(c.foot + 0.1, 1.0)],
            );
        });
    }
    b.paint(TEAM);
    hoop(
        b,
        Vec3::Z * (CORE_TOP + 0.02),
        3.5,
        0.6,
        0.05,
        if fine { 16 } else { 8 },
    );
}

/// The fire-control head on the core: a stepped cap, a sensor drum with a red optic on
/// each diagonal and red slots between, eight plates lapped up round the drum leaning in,
/// long and short in turn, and over them a faceted point with a red band round its foot.
fn head(b: &mut MeshBuilder) {
    let fine = b.fine();
    let z = CORE_TOP;
    dark_plate(b);
    b.prism(Vec3::Z * z, 8, 3.8, 3.3, 0.6);
    seam(b);
    b.prism(Vec3::Z * (z + 0.6), 8, 3.3, 3.1, 0.2);
    // The sensor drum.
    metal(b);
    b.prism(Vec3::Z * (z + 0.8), 8, 2.6, 2.4, 1.6);
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        // A red optic in a dark hood.
        let at = d * 2.3 + Vec3::Z * (z + 1.6);
        dark_plate(b);
        b.cylinder_between(at - d * 0.2, at + d * 0.35, 0.42, 0.38, b.sides(8));
        b.paint(GLOW_LASER);
        b.cylinder_between(at + d * 0.35, at + d * 0.42, 0.26, 0.22, b.sides(8));
        let a = (90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        if fine {
            slit(b, d * 2.33 + Vec3::Z * (z + 1.6), d, Vec3::Z, 1.0, 0.18);
        }
    }
    // The plates lapped up round it, leaning in over the drum.
    for k in 0..8 {
        let a = (22.5 + 45.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let len = if k % 2 == 0 { 3.4 } else { 2.5 };
        blade(b, d * 3.0 + Vec3::Z * (z + 0.7), d, -0.32, len, 0.85, 0.22);
    }
    // The point over it.
    dark_plate(b);
    b.prism(Vec3::Z * (z + 2.4), 8, 2.0, 1.3, 0.6);
    b.paint(GLOW_LASER);
    b.prism(Vec3::Z * (z + 3.0), 8, 1.32, 1.28, 0.12);
    dark_plate(b);
    b.prism(Vec3::Z * (z + 3.12), 8, 1.3, 0.12, 1.8);
}

/// A row's armoured top (a seam course under its deck) and on it a lid for each cell
/// hinged on its edge, over a red rim and the seeker standing in it.
fn cells(b: &mut MeshBuilder, c: &Cells) {
    let plan = chamfered_rect(c.half(), 0.6);
    b.at(c.centre.extend(0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &plan,
            &[Section::new(c.foot, 1.06), Section::new(c.deck - 0.3, 1.0)],
        );
        seam(b);
        b.loft_z(
            &plan,
            &[Section::new(c.deck - 0.3, 1.0), Section::new(c.deck, 0.97)],
        );
    });
    let fine = b.fine();
    let lid = c.pitch * 0.8;
    for m in b.cell_block(c.grid(), &FIRE) {
        if fine {
            b.paint(GLOW_LASER);
            b.plate(m.extend(c.deck), Vec2::splat(c.pitch * 0.9), 0.04, 0.01);
        }
        b.with_part(part::CELL_HATCH, |b| {
            dark_plate(b);
            b.plate(m.extend(c.deck + 0.03), Vec2::splat(lid), 0.2, 0.06);
            if fine {
                // A swept ridge across the lid, and the knuckle it swings on.
                dark_plate(b);
                b.beam(
                    v3(m.x - lid * 0.4, m.y, c.deck + 0.28),
                    v3(m.x + lid * 0.3, m.y - lid * 0.25, c.deck + 0.28),
                    Vec2::new(0.22, 0.12),
                    Vec2::new(0.06, 0.1),
                );
                metal(b);
                b.cylinder_between(
                    v3(m.x - lid * 0.3, m.y + lid * 0.46, c.deck + 0.1),
                    v3(m.x + lid * 0.3, m.y + lid * 0.46, c.deck + 0.1),
                    0.09,
                    0.09,
                    4,
                );
            }
        });
        if !fine {
            continue;
        }
        // The seeker: a dark body and its lit head, all that shows over the rim when the
        // lid swings open.
        b.with_part(part::CELL_ROUND, |b| {
            let top = c.deck - 0.25;
            metal(b);
            b.cylinder_between(m.extend(top - 2.4), m.extend(top - 0.8), 0.48, 0.48, 6);
            b.paint(GLOW_LASER);
            b.cylinder_between(m.extend(top - 0.8), m.extend(top), 0.48, 0.14, 6);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where each seeker is launched from (the unit file's muzzles), in firing order.
    fn muzzles() -> Vec<Vec3> {
        CELLS
            .iter()
            .flat_map(|c| {
                let block = crate::CellBlock {
                    centre: c.centre.to_array(),
                    deck: c.deck,
                    pitch: c.pitch,
                    half: c.pitch * 0.42,
                    nx: 4,
                    ny: 1,
                    hinge_y: true,
                    first: 0,
                    order: [0; crate::CellBlock::MAX_CELLS],
                };
                FIRE.iter().map(move |&(i, j)| {
                    Vec2::from(block.grid_centre(i as usize, j as usize))
                        .extend(c.deck - MUZZLE_DROP)
                })
            })
            .collect()
    }

    /// Fits its lot; nothing yaws (the cells' muzzles are held by `tests/cells.rs`).
    #[test]
    fn belfry_fits() {
        super::super::super::check("regency_seeker_silo", 12.0, 14.0, Some(2), &[]);
    }

    #[test]
    fn the_unit_files_muzzles_are_the_cells() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let unit = bp
            .units
            .iter()
            .find(|u| u.visual.mesh == "regency_seeker_silo")
            .unwrap();
        let data: Vec<Vec3> = unit.weapons[0]
            .muzzles
            .iter()
            .map(|p| Vec3::from(p.to_f32()))
            .collect();
        let model = muzzles();
        assert_eq!(data.len(), model.len());
        for (k, (d, m)) in data.iter().zip(&model).enumerate() {
            assert!(
                d.distance(*m) < 0.02,
                "muzzle {k}: {d} in the file, {m} drawn"
            );
        }
    }
}

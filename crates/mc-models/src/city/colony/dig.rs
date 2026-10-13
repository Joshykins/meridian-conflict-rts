//! The way into a dig: its head works.

use glam::Vec2;

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// A tall heavy hall built over the pit, the lift tower up through its roof, a
/// covered conveyor out of its side to a heap of spoil.
pub(super) fn dig(b: &mut MeshBuilder, _tech: u8) {
    let r = Rect::new(-21.0, -15.0, 13.0, 15.0);
    let lift = Rect::new(-6.0, -4.0, 2.0, 4.0);
    let roof = 14.0;
    if !b.mid() {
        solid(b, r, -2.0, roof + 1.0, pat::PREFAB, pat::ROOF_FLAT);
        solid(b, lift, roof, 22.0, pat::PREFAB, pat::ROOF_FLAT);
        return;
    }
    let cut = 1.2;
    skirt(b, r, cut, 1.0, 1.0, pat::CONCRETE);
    cut_walls(b, r, cut, 1, 1.0, 9.5, all(pat::PREFAB), pat::PREFAB);
    recess(b, r, cut, 1, 9.5, 10.1, 0.35, pat::TRIM);
    cut_walls(b, r, cut, 1, 10.1, roof, all(pat::HAB_BAND), pat::PREFAB);
    paint(b, pat::PREFAB);
    b.loft(
        &[
            cut_ring(r, cut, 1, roof),
            cut_ring(r.grow(0.3), cut + 0.1, 1, roof),
            cut_ring(r.grow(0.3), cut + 0.1, 1, roof + 0.35),
            cut_ring(r.grow(-0.7), cut, 1, roof + 1.0),
        ],
        false,
        false,
    );
    deck(b, r.grow(-0.7), roof + 1.0, pat::ROOF_FLAT);
    // The lift tower through the roof, the sheave house on it.
    cut_walls(
        b,
        lift,
        0.6,
        1,
        roof + 1.0,
        20.5,
        all(pat::PREFAB),
        pat::PREFAB,
    );
    recess(b, lift, 0.6, 1, 20.5, 21.0, 0.25, pat::TRIM);
    cut_slab(b, lift.grow(0.4), 0.8, 1, 21.0, 22.4, pat::ACCENT);
    // The door to the street, big enough for the haul trucks.
    paint(b, pat::STEEL);
    panel(b, r, Side::Front, -10.0, -2.0, 1.0, 7.5, 0.06);
    hood(b, r, Side::Front, -10.6, -1.4, 7.7, 1.6, pat::TRIM);
    // The conveyor out of the east wall to the heap.
    paint(b, pat::TRIM);
    b.beam(
        v3(13.0, -6.0, 10.0),
        v3(18.0, -10.0, 8.0),
        Vec2::new(1.6, 1.4),
        Vec2::new(1.6, 1.4),
    );
    spoil(b, v3(19.5, -11.0, 0.0), Vec2::new(5.5, 6.5), 8.0, 73);
    if b.fine() {
        plant_with(
            b,
            Rect::new(r.min.x + 2.0, r.min.y + 2.0, -8.0, r.max.y - 2.0),
            roof + 1.0,
            75,
            3,
            false,
            false,
        );
        lamp_mast(b, v3(16.0, 12.0, 0.0), 10.0);
    }
}

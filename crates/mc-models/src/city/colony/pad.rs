//! A landing pad for shuttles, flush with the ground.

use glam::Vec3;

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// An octagon raised half a metre on sloped edges a vehicle rolls up, lights let
/// into its rim, a kiosk on a corner and two lamp masts.
pub(super) fn pad(b: &mut MeshBuilder, _tech: u8) {
    let r = Rect::new(-29.0, -29.0, 29.0, 29.0);
    let deck_z = 0.5;
    let top = cut_ring(r, 8.5, 1, deck_z);
    paint(b, pat::PAD);
    facing(b, top.clone(), Vec3::Z);
    if !b.mid() {
        return;
    }
    let foot = r.grow(2.0);
    paint(b, pat::CONCRETE);
    b.loft(
        &[
            cut_ring(foot, 9.3, 1, -0.6),
            cut_ring(foot, 9.3, 1, 0.0),
            top,
        ],
        false,
        false,
    );
    if b.fine() {
        // Rim lights every few metres round the deck's edge.
        let ring = cut_ring(r.grow(-0.5), 8.3, 1, deck_z);
        for k in 0..ring.len() {
            let (p, q) = (ring[k], ring[(k + 1) % ring.len()]);
            let n = ((q - p).length() / 6.0).round().max(1.0) as usize;
            for i in 0..n {
                let c = p.lerp(q, (i as f32 + 0.5) / n as f32);
                boxed(
                    b,
                    c - v3(0.25, 0.25, 0.0),
                    c + v3(0.25, 0.25, 0.12),
                    pat::LED,
                );
            }
        }
    }
    // The kiosk on the north-east corner.
    let kiosk = Rect::new(24.0, 24.0, 30.0, 30.0);
    cut_walls(b, kiosk, 1.0, 1, 0.0, 3.0, all(pat::PREFAB), pat::PREFAB);
    if b.fine() {
        // Its window: dark, no glass to break on scenery nothing hits.
        paint(b, pat::TRIM);
        panel(b, kiosk, Side::West, 25.2, 28.8, 1.2, 2.4, 0.03);
        panel(b, kiosk, Side::Back, 25.2, 28.8, 1.2, 2.4, 0.03);
    }
    cut_slab(b, kiosk.grow(0.4), 1.2, 1, 3.0, 3.4, pat::ACCENT);
    for (x, y) in [(-26.0, 27.0), (27.0, -26.0)] {
        lamp_mast(b, v3(x, y, 0.0), 9.0);
    }
}

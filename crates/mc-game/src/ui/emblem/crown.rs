//! The Regency's marks as ARC draws them: a crown reduced to blades, flat
//! bronze inside a broken double ring, with the red light every Regency
//! machine carries framed at its heart. Everything inside the rings reaches
//! out to about the same radius, so the gap to the ring is even all round,
//! and every gap between pieces, round the frame and between the blades, is
//! one width along its whole length.
//! Design space as the eagle's: 1000 units square, centred on (500, 500).

use super::paint::{circle, hex, poly, Canvas};
use glam::Vec2;
use std::f32::consts::{FRAC_PI_2, TAU};
use tiny_skia::{Path, PathBuilder};

const BRONZE: u32 = 0xA4703C;
const RED: u32 = 0xE61E24;
const CENTRE: Vec2 = Vec2::new(500.0, 500.0);

/// The cavity the light sits in: a diamond `A` wide and `B` tall (half
/// sizes). The spikes' inner edges and the two shards lie on it.
const A: f32 = 80.0;
const B: f32 = 107.0;
/// Half the slot between the two centre spikes.
const SLOT: f32 = 32.0;
/// The one gap round the frame: to the spikes, the shards and the light.
const GAP: f32 = 16.0;
/// The frame's width.
const FRAME: f32 = 18.0;

/// The cavity diamond scaled by `s` about the centre.
fn diamond(s: f32) -> Vec<Vec2> {
    vec![
        CENTRE + Vec2::new(0.0, -B * s),
        CENTRE + Vec2::new(A * s, 0.0),
        CENTRE + Vec2::new(0.0, B * s),
        CENTRE + Vec2::new(-A * s, 0.0),
    ]
}

/// A right-hand outline and its mirror image.
fn mirrored(right: &[(f32, f32)]) -> [Vec<Vec2>; 2] {
    [1.0f32, -1.0].map(|side| {
        right
            .iter()
            .map(|&(x, y)| Vec2::new(CENTRE.x + (x - CENTRE.x) * side, y))
            .collect()
    })
}

/// The tall centre spike's outer edge, tip to foot.
const SPIKE_EDGE: [(f32, f32); 4] = [
    (546.0, 178.0),
    (634.0, 500.0),
    (572.0, 700.0),
    (532.0, 792.0),
];

/// The tall centre spikes, cut in round the cavity.
fn spikes() -> [Vec<Vec2>; 2] {
    // Where the cavity's edge meets the slot.
    let meet = CENTRE.y - (A - SLOT) * B / A;
    let edge = CENTRE.x + SLOT;
    let [tip, knee, foot, base] = SPIKE_EDGE;
    mirrored(&[
        base,
        (edge, 2.0 * CENTRE.y - meet),
        (CENTRE.x + A, CENTRE.y),
        (edge, meet),
        (edge, 290.0),
        tip,
        knee,
        foot,
    ])
}

/// Each side's short spike and long blade as one shape, reaching in under the
/// centre spike; `cut_sides` splits it and clears it off the spike.
fn sides() -> [Vec<Vec2>; 2] {
    mirrored(&[
        (566.0, 350.0),
        (684.0, 236.0),
        (668.0, 360.0),
        (712.0, 452.0),
        (792.0, 500.0),
        (700.0, 560.0),
        (590.0, 712.0),
        (540.0, 760.0),
        (540.0, 360.0),
    ])
}

/// Cuts every gap in the sides as a straight band: `GAP` between the short
/// spike and the long blade, and `GAP` off the centre spike's edge (a band
/// twice as wide, centred on the edge, which the spike then covers half of).
/// Bands keep their width, so every gap is even along its length.
fn cut_sides(c: &mut Canvas) {
    for split in mirrored(&[(620.0, 478.0), (760.0, 440.0)]) {
        c.erase_stroke(&open_line(&split), GAP);
    }
    for edge in mirrored(&SPIKE_EDGE) {
        c.erase_stroke(&open_line(&edge), 2.0 * GAP);
    }
}

/// The shards above and below the frame: in the slot, their inner edge on the
/// cavity's line, `GAP` from the spikes either side.
fn shards() -> [Vec<Vec2>; 2] {
    let w = SLOT - GAP;
    let tip = CENTRE.y - B;
    let side = tip + w * B / A;
    let depth = 26.0;
    [1.0f32, -1.0].map(|dir| {
        let p = |x: f32, y: f32| Vec2::new(x, CENTRE.y + dir * (y - CENTRE.y));
        vec![
            p(CENTRE.x, tip),
            p(CENTRE.x + w, side),
            p(CENTRE.x + w, side - depth),
            p(CENTRE.x, tip - depth),
            p(CENTRE.x - w, side - depth),
            p(CENTRE.x - w, side),
        ]
    })
}

/// An open polyline through `points`.
fn open_line(points: &[Vec2]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let (first, rest) = points.split_first()?;
    pb.move_to(first.x, first.y);
    for p in rest {
        pb.line_to(p.x, p.y);
    }
    pb.finish()
}

/// A ring of `radius`, open across the top by `gap` radians each side.
fn ring(radius: f32, gap: f32) -> (Option<Path>, [Vec2; 2]) {
    let at = |a: f32| CENTRE + Vec2::new(a.cos(), a.sin()) * radius;
    let from = -FRAC_PI_2 + gap;
    let sweep = TAU - 2.0 * gap;
    let mut pb = PathBuilder::new();
    let start = at(from);
    pb.move_to(start.x, start.y);
    for i in 1..=120 {
        let q = at(from + sweep * i as f32 / 120.0);
        pb.line_to(q.x, q.y);
    }
    (pb.finish(), [start, at(from + sweep)])
}

/// The whole mark, in `bronze` with the light in `light`.
fn mark(c: &mut Canvas, bronze: u32, light: u32) {
    let (outer, ends) = ring(388.0, 0.42);
    c.stroke(&outer, 24.0, 1.0, hex(bronze, 1.0));
    for end in ends {
        c.fill(&circle(end, 19.0), hex(bronze, 1.0));
    }
    let (inner, _) = ring(352.0, 1.05);
    c.stroke(&inner, 8.0, 1.0, hex(bronze, 1.0));
    for side in sides() {
        c.fill(&poly(&side), hex(bronze, 1.0));
    }
    cut_sides(c);
    for piece in spikes().iter().chain(shards().iter()) {
        c.fill(&poly(piece), hex(bronze, 1.0));
    }
    let k = (1.0 / (A * A) + 1.0 / (B * B)).sqrt();
    let frame = 1.0 - GAP * k;
    let hole = frame - FRAME * k;
    c.fill(&poly(&diamond(frame)), hex(bronze, 1.0));
    c.erase(&poly(&diamond(hole)));
    c.fill(&poly(&diamond(hole - GAP * k)), hex(light, 1.0));
}

pub fn crest(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    mark(&mut c, BRONZE, RED);
    c.into_rgba()
}

/// The same mark in white, for tinting.
pub fn badge(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    mark(&mut c, 0xFFFFFF, 0xFFFFFF);
    c.into_rgba()
}

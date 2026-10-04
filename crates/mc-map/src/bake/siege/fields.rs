//! The outskirts' fields: a Voronoi of jittered points, one field per point,
//! its borders straight like surveyed fields. The terrain shader draws the
//! same cells (`field_cell` in `shaders/streets.wgsl`, from the same hash)
//! to lay each field's furrows its own way, so the bake only has to say
//! where farmland is; the bake plants hedgerows along some borders.

use super::grid::P;

use crate::format::FIELD_CELL_M as FIELD_CELL;

/// A small integer hash, the same in WGSL (`field_hash`).
pub(in crate::bake) fn field_hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// A field's seed point in grid cell `(x, y)`.
fn seed_point(x: i32, y: i32) -> P {
    let h = field_hash(x, y);
    let u = (h & 0xFFFF) as f64 / 65_536.0;
    let v = (h >> 16) as f64 / 65_536.0;
    (
        (x as f64 + 0.15 + 0.7 * u) * FIELD_CELL,
        (y as f64 + 0.15 + 0.7 * v) * FIELD_CELL,
    )
}

/// The field `p` lies in (its hash) and how far `p` is from its border
/// with the nearest other field, metres; and that neighbour's hash.
pub(in crate::bake) fn field_at(p: P) -> (u32, f64, u32) {
    let (cx, cy) = (
        (p.0 / FIELD_CELL).floor() as i32,
        (p.1 / FIELD_CELL).floor() as i32,
    );
    let mut best = (f64::MAX, (0, 0));
    let mut second = (f64::MAX, (0, 0));
    for y in cy - 1..=cy + 1 {
        for x in cx - 1..=cx + 1 {
            let s = seed_point(x, y);
            let d = (p.0 - s.0).powi(2) + (p.1 - s.1).powi(2);
            if d < best.0 {
                second = best;
                best = (d, (x, y));
            } else if d < second.0 {
                second = (d, (x, y));
            }
        }
    }
    // The distance to the bisector between the two nearest seeds.
    let (a, b) = (
        seed_point(best.1 .0, best.1 .1),
        seed_point(second.1 .0, second.1 .1),
    );
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy).max(1e-6);
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let edge = ((mid.0 - p.0) * dx + (mid.1 - p.1) * dy) / len;
    (
        field_hash(best.1 .0, best.1 .1),
        edge.abs(),
        field_hash(second.1 .0, second.1 .1),
    )
}

/// Whether the border between two fields carries a hedge.
pub(in crate::bake) fn hedged(a: u32, b: u32) -> bool {
    (a ^ b).wrapping_mul(0x9E37_79B9) >> 30 < 2
}

/// Whether a field is ploughed or sown (the rest are pasture).
pub(in crate::bake) fn tilled(h: u32) -> bool {
    (h >> 8) % 10 < 7
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_border_is_where_two_fields_meet() {
        let mut seen_near = 0;
        for i in 0..2_000 {
            let p = (i as f64 * 7.3, 3_000.0 + (i as f64 * 3.1).sin() * 400.0);
            let (h, edge, other) = field_at(p);
            assert_ne!(h, other);
            assert!((0.0..FIELD_CELL * 1.5).contains(&edge));
            if edge < 3.0 {
                seen_near += 1;
                // Just across the border is the other field.
                let q = (p.0 + 6.0, p.1);
                let r = (p.0 - 6.0, p.1);
                let (hq, _, _) = field_at(q);
                let (hr, _, _) = field_at(r);
                assert!(hq == h || hq == other || hr == h || hr == other);
            }
        }
        assert!(seen_near > 0);
    }
}

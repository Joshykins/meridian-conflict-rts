//! Decks: level spans of walkable surface standing over the ground, a bridge's
//! hard-light road. Land units cross on top while ships sail under, so a deck
//! makes its cells both land and water to the pathfinder; a unit on a deck
//! stands at its `top`, a hull under it at the water.
//!
//! A deck is a strip `half` metres either side of the segment from `a` to `b`,
//! flat at `top` metres over the map's zero. Every query is integer-only, on
//! the raw fixed-point values, so every peer agrees on where a deck is.
//!
//! In the map file (`format.rs`) a deck is a record of six raw `i64` values:
//! `a.x, a.y, b.x, b.y, half, top`.

use crate::format::{MapError, Reader};
use crate::CELL_SIZE_M;
use mc_core::{Fx, FxVec2, FxVec3, StateHasher};

/// Most decks a map may carry.
pub const MAX_DECKS: usize = 64;
/// How high over a deck's top a shot may leave and still count as fired from on it:
/// the tallest gun a unit carries.
pub const ON_DECK: Fx = Fx::from_int(60);
/// Bytes per deck record in the map file.
pub const DECK_RECORD_LEN: usize = 48;

/// One deck.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Deck {
    pub a: FxVec2,
    pub b: FxVec2,
    /// Metres either side of the line from `a` to `b`.
    pub half: Fx,
    /// Height of the walking surface over the map's zero (as the heightfield's).
    pub top: Fx,
}

impl Deck {
    /// Whether `p` lies on the deck: within `half` of the line and between its ends.
    pub fn covers(&self, p: FxVec2) -> bool {
        let (dx, dy) = (
            (self.b.x - self.a.x).0 as i128,
            (self.b.y - self.a.y).0 as i128,
        );
        let (vx, vy) = ((p.x - self.a.x).0 as i128, (p.y - self.a.y).0 as i128);
        let along = vx * dx + vy * dy;
        let len2 = dx * dx + dy * dy;
        if along < 0 || along > len2 {
            return false;
        }
        let across = vx * dy - vy * dx;
        let half = self.half.0 as i128;
        across * across <= half * half * len2
    }

    /// The deck's bounding box.
    pub fn bounds(&self) -> (FxVec2, FxVec2) {
        let lo = FxVec2::new(self.a.x.min(self.b.x), self.a.y.min(self.b.y));
        let hi = FxVec2::new(self.a.x.max(self.b.x), self.a.y.max(self.b.y));
        let h = FxVec2::new(self.half, self.half);
        (lo - h, hi + h)
    }

    /// Whether the deck is well formed and lies inside a map of `size`.
    pub fn valid(&self, size: FxVec2) -> bool {
        let inside =
            |p: FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size.x && p.y <= size.y;
        inside(self.a)
            && inside(self.b)
            && self.a != self.b
            && self.half > Fx::ZERO
            && self.half <= Fx::from_int(2_000)
            && self.top.abs() <= Fx::from_int(4_000)
    }

    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        for v in [self.a.x, self.a.y, self.b.x, self.b.y, self.half, self.top] {
            out.extend_from_slice(&v.0.to_le_bytes());
        }
    }

    pub(crate) fn parse(r: &mut Reader<'_>) -> Result<Deck, MapError> {
        let mut v = [Fx::ZERO; 6];
        for x in &mut v {
            *x = Fx(r.i64()?);
        }
        Ok(Deck {
            a: FxVec2::new(v[0], v[1]),
            b: FxVec2::new(v[2], v[3]),
            half: v[4],
            top: v[5],
        })
    }

    pub(crate) fn hash(&self, h: &mut StateHasher) {
        for v in [self.a.x, self.a.y, self.b.x, self.b.y, self.half, self.top] {
            h.write_i64(v.0);
        }
    }
}

/// A map's decks, for the queries the simulation makes of them.
#[derive(Clone, Default, Debug)]
pub struct Decks {
    decks: Vec<Deck>,
    /// Each deck's bounding box, to rule it out quickly.
    bounds: Vec<(FxVec2, FxVec2)>,
}

impl Decks {
    pub fn new(decks: Vec<Deck>) -> Decks {
        let bounds = decks.iter().map(Deck::bounds).collect();
        Decks { decks, bounds }
    }

    pub fn list(&self) -> &[Deck] {
        &self.decks
    }

    pub fn is_empty(&self) -> bool {
        self.decks.is_empty()
    }

    /// The top of the highest deck over `p`, if any.
    pub fn top_at(&self, p: FxVec2) -> Option<Fx> {
        let mut top: Option<Fx> = None;
        for (d, &(lo, hi)) in self.decks.iter().zip(&self.bounds) {
            if p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y && d.covers(p) {
                top = Some(top.map_or(d.top, |t| t.max(d.top)));
            }
        }
        top
    }

    /// Whether a deck covers the middle of cell `(cx, cy)`: a cell the
    /// pathfinder takes as land as well as whatever water lies under it.
    pub fn covers_cell(&self, cx: u32, cy: u32) -> bool {
        !self.decks.is_empty() && self.top_at(cell_middle(cx, cy)).is_some()
    }

    /// Whether a deck covers the middle of any cell in `min..=max`.
    pub fn covers_any_cell(&self, min: (u32, u32), max: (u32, u32)) -> bool {
        if self.decks.is_empty() {
            return false;
        }
        let (lo, hi) = (cell_middle(min.0, min.1), cell_middle(max.0, max.1));
        let near = self
            .bounds
            .iter()
            .any(|&(a, b)| lo.x <= b.x && lo.y <= b.y && hi.x >= a.x && hi.y >= a.y);
        near && (min.1..=max.1).any(|y| (min.0..=max.0).any(|x| self.covers_cell(x, y)))
    }

    /// Where the segment from `from` to `to` comes down through a deck's
    /// surface from above, the first such point along it, and how far along
    /// the segment that is (0 to 1): a shot meets the deck there. A segment
    /// climbing, or level, passes; so does a shot fired from on the deck
    /// (`fired_from` over it, at most `ON_DECK` over its top), which a unit on
    /// a bridge fires off it.
    pub fn crossing(&self, from: FxVec3, to: FxVec3, fired_from: FxVec3) -> Option<(FxVec3, Fx)> {
        if from.z <= to.z {
            return None;
        }
        let mut best: Option<(Fx, FxVec3)> = None;
        for d in &self.decks {
            if from.z < d.top || to.z > d.top {
                continue;
            }
            let fired_on = d.covers(fired_from.xy())
                && fired_from.z >= d.top - Fx::ONE
                && fired_from.z <= d.top + ON_DECK;
            if fired_on {
                continue;
            }
            // How far along the segment it reaches the deck's height.
            let t = (from.z - d.top) / (from.z - to.z);
            let point = from + (to - from) * t;
            if d.covers(point.xy()) && best.is_none_or(|(b, _)| t < b) {
                best = Some((t, FxVec3::new(point.x, point.y, d.top)));
            }
        }
        best.map(|(t, p)| (p, t))
    }
}

/// The middle of cell `(cx, cy)`.
fn cell_middle(cx: u32, cy: u32) -> FxVec2 {
    let half = CELL_SIZE_M / 2;
    FxVec2::from_ints(
        cx as i32 * CELL_SIZE_M + half,
        cy as i32 * CELL_SIZE_M + half,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck() -> Deck {
        Deck {
            a: FxVec2::from_ints(100, 100),
            b: FxVec2::from_ints(500, 100),
            half: Fx::from_int(40),
            top: Fx::from_int(50),
        }
    }

    #[test]
    fn a_deck_covers_its_strip_and_nothing_past_its_ends() {
        let d = deck();
        assert!(d.covers(FxVec2::from_ints(300, 139)));
        assert!(d.covers(FxVec2::from_ints(100, 60)));
        assert!(!d.covers(FxVec2::from_ints(300, 141)));
        assert!(!d.covers(FxVec2::from_ints(99, 100)));
        assert!(!d.covers(FxVec2::from_ints(501, 100)));
        // Turned: the same strip from the other end, and on a slant.
        let slant = Deck {
            a: FxVec2::from_ints(100, 100),
            b: FxVec2::from_ints(400, 500),
            ..d
        };
        assert!(slant.covers(FxVec2::from_ints(250, 300)));
        assert!(!slant.covers(FxVec2::from_ints(350, 300)));
    }

    #[test]
    fn decks_answer_for_cells_and_for_shots_coming_down() {
        let decks = Decks::new(vec![deck()]);
        assert_eq!(
            decks.top_at(FxVec2::from_ints(300, 100)),
            Some(Fx::from_int(50))
        );
        assert_eq!(decks.top_at(FxVec2::from_ints(300, 200)), None);
        assert!(decks.covers_cell(37, 12));
        assert!(!decks.covers_cell(37, 20));
        assert!(decks.covers_any_cell((30, 15), (40, 25)));
        assert!(!decks.covers_any_cell((30, 20), (40, 25)));
        let sky = FxVec3::new(Fx::from_int(300), Fx::from_int(-400), Fx::from_int(400));
        let down = decks.crossing(
            FxVec3::new(Fx::from_int(300), Fx::from_int(100), Fx::from_int(80)),
            FxVec3::new(Fx::from_int(300), Fx::from_int(100), Fx::from_int(20)),
            sky,
        );
        assert_eq!(down.map(|p| p.0.z), Some(Fx::from_int(50)));
        assert_eq!(down.map(|p| p.1), Some(Fx::HALF));
        // A gun on the deck fires off it, down at the water.
        let off = decks.crossing(
            FxVec3::new(Fx::from_int(300), Fx::from_int(100), Fx::from_int(55)),
            FxVec3::new(Fx::from_int(320), Fx::from_int(100), Fx::from_int(45)),
            FxVec3::new(Fx::from_int(290), Fx::from_int(100), Fx::from_int(55)),
        );
        assert_eq!(off, None);
        // From under the deck, or past its side, a shot goes on.
        let up = decks.crossing(
            FxVec3::new(Fx::from_int(300), Fx::from_int(100), Fx::from_int(20)),
            FxVec3::new(Fx::from_int(300), Fx::from_int(100), Fx::from_int(80)),
            sky,
        );
        assert_eq!(up, None);
        let beside = decks.crossing(
            FxVec3::new(Fx::from_int(300), Fx::from_int(300), Fx::from_int(80)),
            FxVec3::new(Fx::from_int(300), Fx::from_int(300), Fx::from_int(20)),
            sky,
        );
        assert_eq!(beside, None);
    }

    #[test]
    fn a_deck_round_trips_through_its_record() {
        let d = deck();
        let mut bytes = Vec::new();
        d.write(&mut bytes);
        assert_eq!(bytes.len(), DECK_RECORD_LEN);
        assert_eq!(Deck::parse(&mut Reader::new(&bytes)).unwrap(), d);
    }
}

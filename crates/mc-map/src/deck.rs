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
use crate::landmark::HARDLIGHT_BRIDGE as PLAN;
use crate::CELL_SIZE_M;
use mc_core::{Fx, FxVec2, FxVec3, StateHasher};

/// Most decks a map may carry.
pub const MAX_DECKS: usize = 64;
/// How high over a deck's top a shot may leave and still count as fired from on it:
/// the tallest gun a unit carries.
pub const ON_DECK: Fx = Fx::from_int(60);
/// Bytes per deck record in the map file.
pub const DECK_RECORD_LEN: usize = 48;
/// A deck's bridge holds it and a projector past each end (`HARDLIGHT_BRIDGE`'s
/// head and ramp, a little wider than the road): how far past each end, and how
/// wide, in millionths of the deck's length and of its half-width. Worked out
/// once, at compile time, so every peer has the same integers.
const PROJECTOR_REACH: i128 = ((PLAN.head + PLAN.run) / PLAN.span * 1e6) as i128;
const PROJECTOR_WIDTH: i128 = ((PLAN.half + PLAN.edge) / PLAN.half * 1e6) as i128;
const MILLION: i128 = 1_000_000;

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

    /// Whether `p` lies on the deck's bridge: the deck, or a projector past
    /// either end of it. Nothing is built there.
    pub fn holds(&self, p: FxVec2) -> bool {
        let (dx, dy) = (
            (self.b.x - self.a.x).0 as i128,
            (self.b.y - self.a.y).0 as i128,
        );
        let (vx, vy) = ((p.x - self.a.x).0 as i128, (p.y - self.a.y).0 as i128);
        let along = (vx * dx + vy * dy) * MILLION;
        let len2 = dx * dx + dy * dy;
        let reach = len2 * PROJECTOR_REACH;
        if along < -reach || along > len2 * MILLION + reach {
            return false;
        }
        let across = vx * dy - vy * dx;
        let half = self.half.0 as i128 * PROJECTOR_WIDTH / MILLION;
        across * across <= half * half * len2
    }

    /// The bounding box of what the deck's bridge holds (`holds`), a little
    /// generous.
    fn held_bounds(&self) -> (FxVec2, FxVec2) {
        let (lo, hi) = self.bounds();
        let len = (self.b.x - self.a.x).abs() + (self.b.y - self.a.y).abs();
        let out = Fx((len.0 as i128 * PROJECTOR_REACH / MILLION) as i64)
            + Fx((self.half.0 as i128 * PROJECTOR_WIDTH / MILLION) as i64);
        let out = FxVec2::new(out, out);
        (lo - out, hi + out)
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
    /// Each deck's bridge's (`Deck::held_bounds`).
    held: Vec<(FxVec2, FxVec2)>,
}

impl Decks {
    pub fn new(decks: Vec<Deck>) -> Decks {
        let bounds = decks.iter().map(Deck::bounds).collect();
        let held = decks.iter().map(Deck::held_bounds).collect();
        Decks {
            decks,
            bounds,
            held,
        }
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

    /// Whether a bridge holds the middle of cell `(cx, cy)` (`Deck::holds`): its
    /// deck or a projector, where nothing is built.
    pub fn holds_cell(&self, cx: u32, cy: u32) -> bool {
        let p = cell_middle(cx, cy);
        self.decks.iter().zip(&self.held).any(|(d, &(lo, hi))| {
            p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y && d.holds(p)
        })
    }

    /// Whether a bridge holds the middle of any cell in `min..=max`.
    pub fn holds_any_cell(&self, min: (u32, u32), max: (u32, u32)) -> bool {
        if self.decks.is_empty() {
            return false;
        }
        let (lo, hi) = (cell_middle(min.0, min.1), cell_middle(max.0, max.1));
        let near = self
            .held
            .iter()
            .any(|&(a, b)| lo.x <= b.x && lo.y <= b.y && hi.x >= a.x && hi.y >= a.y);
        near && (min.1..=max.1).any(|y| (min.0..=max.0).any(|x| self.holds_cell(x, y)))
    }

    /// Where the segment from `from` to `to` passes through a deck's surface,
    /// the first such point along it, and how far along the segment that is (0
    /// to 1): a shot meets the deck there, coming down onto it, or climbing into
    /// its underside when it was fired from under it (`fired_from` over the
    /// deck and below its top). A level segment passes; so does a shot coming
    /// down that was fired from on the deck (at most `ON_DECK` over its top),
    /// which a unit on a bridge fires off it, and one climbing that was fired
    /// from beside the deck, which comes up over its side at a unit on it.
    pub fn crossing(&self, from: FxVec3, to: FxVec3, fired_from: FxVec3) -> Option<(FxVec3, Fx)> {
        if from.z == to.z {
            return None;
        }
        let down = from.z > to.z;
        let mut best: Option<(Fx, FxVec3)> = None;
        for d in &self.decks {
            let through = if down {
                from.z >= d.top && to.z <= d.top
            } else {
                from.z < d.top && to.z >= d.top
            };
            if !through {
                continue;
            }
            let over = d.covers(fired_from.xy());
            let passes = if down {
                over && fired_from.z >= d.top - Fx::ONE && fired_from.z <= d.top + ON_DECK
            } else {
                !over || fired_from.z >= d.top
            };
            if passes {
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
    fn decks_answer_for_cells_and_for_shots_through_them() {
        let decks = Decks::new(vec![deck()]);
        assert_eq!(
            decks.top_at(FxVec2::from_ints(300, 100)),
            Some(Fx::from_int(50))
        );
        assert_eq!(decks.top_at(FxVec2::from_ints(300, 200)), None);
        assert!(decks.covers_cell(37, 12));
        assert!(!decks.covers_cell(37, 20));
        assert!(decks.holds_any_cell((30, 15), (40, 25)));
        assert!(!decks.holds_any_cell((30, 20), (40, 25)));
        // Its bridge holds a projector past each end too (51.5 m on a 400 m deck).
        assert!(!decks.covers_cell(8, 12));
        assert!(decks.holds_cell(8, 12));
        assert!(!decks.holds_cell(4, 12));
        assert!(decks.holds_cell(66, 12));
        assert!(!decks.holds_cell(70, 12));
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
        // Fired from under the deck, a shot climbing meets its underside; fired
        // from beside it, it comes up over the side; past its side it goes on.
        let under = FxVec3::new(Fx::from_int(300), Fx::from_int(110), Fx::from_int(15));
        let up = decks.crossing(
            FxVec3::new(Fx::from_int(300), Fx::from_int(100), Fx::from_int(20)),
            FxVec3::new(Fx::from_int(300), Fx::from_int(100), Fx::from_int(80)),
            under,
        );
        assert_eq!(up.map(|p| p.0.z), Some(Fx::from_int(50)));
        assert_eq!(up.map(|p| p.1), Some(Fx::HALF));
        let over_the_side = decks.crossing(
            FxVec3::new(Fx::from_int(300), Fx::from_int(130), Fx::from_int(45)),
            FxVec3::new(Fx::from_int(300), Fx::from_int(120), Fx::from_int(55)),
            FxVec3::new(Fx::from_int(300), Fx::from_int(400), Fx::from_int(15)),
        );
        assert_eq!(over_the_side, None);
        let past = decks.crossing(
            FxVec3::new(Fx::from_int(300), Fx::from_int(300), Fx::from_int(20)),
            FxVec3::new(Fx::from_int(300), Fx::from_int(300), Fx::from_int(80)),
            under,
        );
        assert_eq!(past, None);
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

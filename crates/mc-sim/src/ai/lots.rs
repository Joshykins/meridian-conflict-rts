//! Lanes between the AI's buildings.
//!
//! Placement only forbids overlap, so over a match the AI packed storage,
//! shields and power flush against its factories. One factory's exit ended up
//! facing a 16 m pocket walled in by a storage and a shield: everything it made
//! rolled out into the pocket and stayed there, idle, for the rest of the match,
//! while the AI ordered it to the front every think.
use super::*;

/// Clear ground kept between two buildings, in build cells (24 m): room for a
/// large hull, which needs three 8 m path cells.
pub(super) const LANE_CELLS: i32 = 2;
/// Clear ground kept in front of a factory's exit, in build cells.
const APRON_CELLS: i32 = 4;
/// Buildings this small (2x2 and less) may stand shoulder to shoulder with
/// each other, as power farms do: a block of them is small enough to go around.
pub(super) const SMALL_FOOT: i32 = 2;

/// A lot in whole metres: `(x0, y0, x1, y1)`.
type Rect = (i32, i32, i32, i32);

fn lot(foot: (i32, i32), pos: FxVec2) -> Rect {
    let (hw, hh) = (
        foot.0 * mc_map::BUILD_CELL_M / 2,
        foot.1 * mc_map::BUILD_CELL_M / 2,
    );
    let (cx, cy) = (pos.x.round_int(), pos.y.round_int());
    (cx - hw, cy - hh, cx + hw, cy + hh)
}

/// The strip a factory's units roll out onto, in front of the side it faces.
fn apron(foot: (i32, i32), pos: FxVec2, heading: Angle) -> Rect {
    let (x0, y0, x1, y1) = lot(foot, pos);
    let depth = APRON_CELLS * mc_map::BUILD_CELL_M;
    let d = FxVec2::from_angle(heading);
    if d.x.abs() >= d.y.abs() {
        if d.x > Fx::ZERO {
            (x1, y0, x1 + depth, y1)
        } else {
            (x0 - depth, y0, x0, y1)
        }
    } else if d.y > Fx::ZERO {
        (x0, y1, x1, y1 + depth)
    } else {
        (x0, y0 - depth, x1, y0)
    }
}

/// Gap between two lots, negative where they overlap.
fn gap(a: Rect, b: Rect) -> i32 {
    let gx = (b.0 - a.2).max(a.0 - b.2);
    let gy = (b.1 - a.3).max(a.1 - b.3);
    gx.max(gy)
}

/// The lane two buildings keep between them, in metres.
fn lane(a: i32, b: i32) -> i32 {
    if a <= SMALL_FOOT && b <= SMALL_FOOT {
        0
    } else {
        LANE_CELLS * mc_map::BUILD_CELL_M
    }
}

impl World {
    /// Whether a building of `bp` at `site` keeps its lanes: clear of every
    /// factory's apron, a lane off every other building, and, for a factory,
    /// with its own apron clear. `claimed` are this think's plans.
    pub(super) fn keeps_lanes(&self, bp: &UnitBlueprint, site: FxVec2, claimed: &[Claim]) -> bool {
        let foot = (bp.footprint.0 as i32, bp.footprint.1 as i32);
        let size = foot.0.max(foot.1);
        let me = lot(foot, site);
        // The AI builds every structure facing the same way.
        let own_apron = bp
            .has(cat::FACTORY)
            .then(|| apron(foot, site, AI_BUILD_HEADING));
        // An apron keeps a lane around it as well: buildings just clear of it
        // on three sides would still pen in what rolls out onto it.
        let full = LANE_CELLS * mc_map::BUILD_CELL_M;
        let clear = |other: Rect, other_size: i32, other_apron: Option<Rect>| {
            gap(me, other) >= lane(size, other_size)
                && other_apron.is_none_or(|a| gap(me, a) >= full)
                && own_apron.is_none_or(|a| gap(a, other) >= full)
        };
        for c in claimed {
            let other = (c.foot, c.foot);
            let other_apron = c.factory.then(|| apron(other, c.pos, AI_BUILD_HEADING));
            if !clear(lot(other, c.pos), c.foot, other_apron) {
                return false;
            }
        }
        let reach = Fx::from_int((size + 8 + APRON_CELLS + LANE_CELLS) * mc_map::BUILD_CELL_M);
        let mut ok = true;
        self.index.query(site, reach, kind::UNIT, |e| {
            let row = e.row as usize;
            if !self.unit_entry_is_current(e) {
                return true;
            }
            let other = self.bp(row);
            if !other.is_structure() {
                return true;
            }
            let of = (other.footprint.0 as i32, other.footprint.1 as i32);
            let pos = self.state.units.pos[row];
            let other_apron = other
                .has(cat::FACTORY)
                .then(|| apron(of, pos, self.state.units.heading[row]));
            ok = clear(lot(of, pos), of.0.max(of.1), other_apron);
            ok
        });
        ok
    }
}

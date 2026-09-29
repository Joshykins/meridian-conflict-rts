//! The construction strip's layout: where each tile sits, shelf after shelf
//! (Factories, Economy, Defense, Anti-Air, ... Units) with a divider in the
//! room between one shelf and the next, and the stops the strip scrolls
//! between when it runs past the panel.

use super::{Purpose, SHELF_H, STRIP_H, TILE_GAP, TILE_W};
use crate::ui::{palette, rgb, Rect, Ui};
use mc_data::UnitBlueprint;

/// Room between the last tile of one shelf and the first of the next, where
/// the divider stands.
const SHELF_GAP: f32 = 22.0;
/// The arrow buttons at the strip's ends while it runs past the panel.
pub(super) const ARROW_W: f32 = 24.0;

/// A tile's place along the strip.
pub(super) struct Slot<'a> {
    pub(super) item: &'a UnitBlueprint,
    pub(super) shelf: Purpose,
    /// Its place on its shelf, which picks its item key.
    pub(super) n: usize,
    pub(super) at: f32,
    /// A tier upgrade: its step along the builder's line of successors.
    pub(super) climb: Option<usize>,
}

pub(super) struct Layout<'a> {
    pub(super) slots: Vec<Slot<'a>>,
    /// Each shelf: what it is, where it starts and ends along the strip, and its count.
    pub(super) shelves: Vec<(Purpose, f32, f32, usize)>,
    pub(super) length: f32,
    pub(super) tile_w: f32,
    /// The strip is longer than the panel: it scrolls, between arrows.
    pub(super) overflow: bool,
    /// Where the tiles show.
    pub(super) view: Rect,
}

/// The tier's upgrades (`climbs`) lead, then `items` by shelf, in the panel `grid`.
pub(super) fn lay_out<'a>(
    items: &[&'a UnitBlueprint],
    climbs: &[(usize, &'a UnitBlueprint)],
    is_factory: bool,
    grid: Rect,
) -> Layout<'a> {
    let lead = items.first().map(|b| Purpose::of(b, !is_factory));
    let lay = |tile_w: f32| {
        let mut slots: Vec<Slot> = Vec::new();
        let mut shelves: Vec<(Purpose, f32, f32, usize)> = Vec::new();
        let mut at = 0.0;
        for &(i, item) in climbs {
            let shelf = lead.unwrap_or_else(|| Purpose::of(item, !is_factory));
            slots.push(Slot {
                item,
                shelf,
                n: usize::MAX,
                at,
                climb: Some(i),
            });
            at += tile_w + TILE_GAP;
        }
        for p in Purpose::ALL {
            let on: Vec<&UnitBlueprint> = items
                .iter()
                .copied()
                .filter(|b| Purpose::of(b, !is_factory) == p)
                .collect();
            if on.is_empty() {
                continue;
            }
            if !shelves.is_empty() || !climbs.is_empty() {
                at += SHELF_GAP - TILE_GAP;
            }
            let start = at;
            for (n, item) in on.iter().enumerate() {
                slots.push(Slot {
                    item,
                    shelf: p,
                    n,
                    at,
                    climb: None,
                });
                at += tile_w + TILE_GAP;
            }
            shelves.push((p, start, at - TILE_GAP, on.len()));
        }
        (slots, shelves, at - TILE_GAP)
    };
    let strip = Rect::new(grid.x, grid.y + SHELF_H + 8.0, grid.w, STRIP_H);
    let (slots, shelves, length) = lay(TILE_W);
    if length <= strip.w {
        return Layout {
            slots,
            shelves,
            length,
            tile_w: TILE_W,
            overflow: false,
            view: strip,
        };
    }
    let view = Rect::new(
        strip.x + ARROW_W + 6.0,
        strip.y,
        strip.w - 2.0 * (ARROW_W + 6.0),
        STRIP_H,
    );
    // Past the panel, tiles are sized so a whole number of them fill the view,
    // and the strip stops only where a tile starts (`snap`): it never rests on
    // half a tile.
    let fit = ((view.w + TILE_GAP) / (TILE_W + TILE_GAP)).round().max(1.0);
    let w = (view.w + TILE_GAP) / fit - TILE_GAP;
    // Never wider than a tile and a quarter: one more, smaller, fits better.
    let fit = if w > TILE_W * 1.25 { fit + 1.0 } else { fit };
    let tile_w = (view.w + TILE_GAP) / fit - TILE_GAP;
    let (slots, shelves, length) = lay(tile_w);
    Layout {
        slots,
        shelves,
        length,
        tile_w,
        overflow: true,
        view,
    }
}

/// Where the strip may rest: at a tile's start, or at its far end.
fn stops<'a>(slots: &'a [Slot], max_scroll: f32) -> impl Iterator<Item = f32> + 'a {
    slots
        .iter()
        .map(move |t| t.at.min(max_scroll))
        .chain([max_scroll])
}

/// The rest nearest `x` (anywhere, when the strip fits the panel).
pub(super) fn snap(slots: &[Slot], overflow: bool, max_scroll: f32, x: f32) -> f32 {
    if !overflow {
        return x.clamp(0.0, max_scroll);
    }
    stops(slots, max_scroll)
        .min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))
        .unwrap_or(0.0)
}

/// The next rest from `from` toward `dir` (+1 on along the strip, -1 back).
pub(super) fn step(slots: &[Slot], max_scroll: f32, from: f32, dir: f32) -> f32 {
    let next = if dir > 0.0 {
        stops(slots, max_scroll)
            .filter(|&a| a > from + 0.5)
            .min_by(f32::total_cmp)
    } else {
        stops(slots, max_scroll)
            .filter(|&a| a < from - 0.5)
            .max_by(f32::total_cmp)
    };
    next.unwrap_or(from)
}

/// A divider between one shelf and the next (and after the upgrade that leads):
/// a line down the gap, capped top and bottom.
pub(super) fn dividers(
    ui: &mut Ui,
    shelves: &[(Purpose, f32, f32, usize)],
    after_climb: bool,
    view: Rect,
    shown: f32,
) {
    for &(_, start, ..) in shelves.iter().skip(if after_climb { 0 } else { 1 }) {
        let x = view.x + start - shown - SHELF_GAP * 0.5;
        if x <= view.x + 1.0 || x >= view.right() - 1.0 {
            continue;
        }
        let (top, h) = (view.y + 4.0, STRIP_H - 8.0);
        ui.vline(x, top, h, rgb(palette::LINE, 0.5));
        for y in [top, top + h - 1.0] {
            ui.hline(x - 3.0, y, 7.0, rgb(palette::LINE, 0.8));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_data::Blueprints;

    #[test]
    fn a_long_strip_rests_only_where_a_tile_starts() {
        let bps = Blueprints::load(&Blueprints::locate_data_dir().expect("data dir"))
            .expect("blueprints");
        let commander = bps.unit(bps.id_of("aster_commander").expect("commander"));
        let items: Vec<&UnitBlueprint> = commander
            .builder
            .as_ref()
            .expect("builds")
            .builds
            .iter()
            .map(|b| bps.unit(*b))
            .filter(|b| b.tech == 1)
            .collect();
        let grid = Rect::new(0.0, 0.0, 400.0, 200.0);
        let lay = lay_out(&items, &[], false, grid);
        assert!(lay.overflow && lay.shelves.len() > 1);
        // Between shelves there is room for the divider, more than between tiles.
        let (_, _, end, _) = lay.shelves[0];
        let (_, start, ..) = lay.shelves[1];
        assert!(start - end >= SHELF_GAP - 0.01);
        let max = (lay.length - lay.view.w).max(0.0);
        let starts: Vec<f32> = lay.slots.iter().map(|t| t.at.min(max)).collect();
        let mut x = 0.0;
        let mut seen = 0;
        loop {
            let next = step(&lay.slots, max, x, 1.0);
            if next == x {
                break;
            }
            assert!(starts.contains(&next) || next == max);
            x = next;
            seen += 1;
        }
        assert_eq!(x, max);
        assert!(seen >= 2);
        assert!(step(&lay.slots, max, x, -1.0) < x);
        // A stop between two starts goes to the nearer.
        let (a, b) = (lay.slots[1].at, lay.slots[2].at);
        assert_eq!(snap(&lay.slots, true, max, a + (b - a) * 0.3), a);
    }
}

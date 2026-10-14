//! What the presentation sees of aircraft fuel (`crate::staging`) and of carriers' air
//! wing queues (`crate::air_wings`).

use crate::World;
use mc_core::Fx;

/// An aircraft's tank, for the line under its health bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuelView {
    pub unit_id: u32,
    /// Zero (dry) to one (full).
    pub fill: f32,
    /// Seconds until it runs dry at the pace it burns now; `None` while it is not
    /// burning (set down, aboard a base) or already dry.
    pub dry_in: Option<f32>,
}

impl FuelView {
    /// The tank of `unit_id` in a frame's list (`RenderFrame::fuel`, in unit id order).
    pub fn of(fuel: &[FuelView], unit_id: u32) -> Option<&FuelView> {
        fuel.binary_search_by_key(&unit_id, |f| f.unit_id)
            .ok()
            .map(|i| &fuel[i])
    }
}

impl World {
    /// Every tank on `viewer`'s side and its allies' (everyone's with no viewer), in unit
    /// id order so the selection's bars find theirs by search (`FuelView::of`): an enemy
    /// is not told how far its aircraft can still go.
    pub(super) fn write_fuel(&self, viewer: Option<u8>, out: &mut Vec<FuelView>) {
        out.clear();
        let units = &self.state.units;
        for row in units.slots.iter() {
            let Some(tank) = self.bp(row).fuel else {
                continue;
            };
            if viewer.is_some_and(|v| self.are_enemies(v, units.owner[row])) {
                continue;
            }
            out.push(FuelView {
                unit_id: units.id(row).0,
                fill: (units.fuel[row] / tank.range.max(Fx::ONE)).to_f32(),
                dry_in: self
                    .burn_pace(row)
                    .filter(|&pace| pace > Fx::ZERO && units.fuel[row] > Fx::ZERO)
                    .map(|pace| (units.fuel[row] / pace).to_f32()),
            });
        }
        out.sort_by_key(|f| f.unit_id);
    }
}

/// A carrier's air wing queue (`crate::air_wings`), for its panel.
#[derive(Clone, Debug, PartialEq)]
pub struct WingView {
    pub unit_id: u32,
    /// Wings queued, by their index in the carrier's `air_wings`; the first is under way.
    pub queue: Vec<u8>,
    /// How far the first is built, zero to one.
    pub progress: f32,
    /// Aircraft of the last wing still aboard or waiting for the rest to go off the deck.
    pub launching: u8,
}

impl World {
    /// Every wing queue on `viewer`'s side (everyone's with no viewer).
    pub(super) fn write_wings(&self, viewer: Option<u8>, out: &mut Vec<WingView>) {
        out.clear();
        let units = &self.state.units;
        for (id, bay) in &self.state.wings.bays {
            let Some(row) = units.row(*id) else {
                continue;
            };
            if viewer.is_some_and(|v| v != units.owner[row]) {
                continue;
            }
            let time = bay
                .queue
                .first()
                .and_then(|&w| self.wing_time(row, w))
                .unwrap_or(Fx::ONE);
            out.push(WingView {
                unit_id: id.0,
                queue: bay.queue.clone(),
                progress: (bay.progress / time.max(Fx::ONE)).to_f32(),
                launching: bay.launching.len().min(u8::MAX as usize) as u8,
            });
        }
    }
}

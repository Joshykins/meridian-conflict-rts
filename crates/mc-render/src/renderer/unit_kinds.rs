//! The frame's units by blueprint, made once a tick (`Renderer::upload_sim`).
//!
//! Many of the renderer's effects belong to a few kinds of unit (a star core, an
//! elevator, a ship's gun): each walked every unit of the frame for its own, and with
//! a hundred thousand fighters on the field twenty such walks were tens of
//! milliseconds a tick. Each now asks for the units of the blueprints it draws for.

use mc_sim::mirror::UnitInstance;

#[derive(Default)]
pub(super) struct UnitKinds {
    /// Per blueprint id, where its units start in `rows` (one more entry at the end).
    start: Vec<u32>,
    /// Frame indices of the units, by blueprint, in frame order within one.
    rows: Vec<u32>,
    /// The blueprints with a unit in the frame, ascending.
    present: Vec<u32>,
}

impl UnitKinds {
    /// Sorts `units` by blueprint (`blueprints` of them).
    pub(super) fn rebuild(&mut self, units: &[UnitInstance], blueprints: usize) {
        self.start.clear();
        self.start.resize(blueprints + 2, 0);
        for u in units {
            let b = (u.blueprint as usize).min(blueprints);
            self.start[b + 2] += 1;
        }
        for b in 2..self.start.len() {
            self.start[b] += self.start[b - 1];
        }
        self.rows.clear();
        self.rows.resize(units.len(), 0);
        for (i, u) in units.iter().enumerate() {
            let b = (u.blueprint as usize).min(blueprints);
            let at = &mut self.start[b + 1];
            self.rows[*at as usize] = i as u32;
            *at += 1;
        }
        self.start.pop();
        self.present.clear();
        self.present.extend(
            (0..blueprints as u32).filter(|&b| self.start[b as usize] < self.start[b as usize + 1]),
        );
    }

    /// Frame indices of the units of the blueprints `wanted` takes, by blueprint.
    pub(super) fn rows_where(&self, mut wanted: impl FnMut(u32) -> bool) -> Vec<usize> {
        let mut out = Vec::new();
        for &b in &self.present {
            if wanted(b) {
                let (a, z) = (self.start[b as usize], self.start[b as usize + 1]);
                out.extend(
                    self.rows[a as usize..z as usize]
                        .iter()
                        .map(|&i| i as usize),
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_come_back_by_blueprint_in_frame_order() {
        let unit = |blueprint| UnitInstance {
            blueprint,
            ..bytemuck::Zeroable::zeroed()
        };
        let units: Vec<_> = [3, 1, 3, 0, 1, 3].into_iter().map(unit).collect();
        let mut kinds = UnitKinds::default();
        kinds.rebuild(&units, 5);
        assert_eq!(kinds.rows_where(|b| b == 3), vec![0, 2, 5]);
        assert_eq!(kinds.rows_where(|b| b == 1 || b == 0), vec![3, 1, 4]);
        assert!(kinds.rows_where(|b| b == 4).is_empty());
    }
}

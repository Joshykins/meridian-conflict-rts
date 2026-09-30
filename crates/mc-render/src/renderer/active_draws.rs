//! Skip models absent from the CPU render mirror before submitting indirect draws.
//! GPU culling still decides visibility and LOD. Keep every LOD of an occupied
//! model, including the fourth prop LOD and blueprints sharing a mesh.

use mc_sim::mirror::UnitInstance;
use std::ops::Range;

pub(super) struct ActiveDraws {
    models: Vec<[u32; 2]>,
    statics: Vec<bool>,
    units: Vec<bool>,
    frame: Vec<bool>,
    pub(super) ranges: Vec<Range<u32>>,
}

impl ActiveDraws {
    pub(super) fn new(
        mut models: Vec<[u32; 2]>,
        prop_base: u32,
        slots: u32,
        statics: &[UnitInstance],
    ) -> Self {
        for model in &mut models[prop_base as usize..] {
            model[1] = crate::gpu_consts::lod::FAR + 1;
        }
        let mut out = Self {
            models,
            statics: vec![false; slots as usize],
            units: vec![false; slots as usize],
            frame: vec![false; slots as usize],
            ranges: Vec::new(),
        };
        Self::mark(&out.models, &mut out.statics, statics.iter());
        out
    }

    fn mark<'a>(
        models: &[[u32; 2]],
        occupied: &mut [bool],
        entities: impl Iterator<Item = &'a UnitInstance>,
    ) {
        for entity in entities {
            let [start, count] = models[entity.blueprint as usize];
            occupied[start as usize..(start + count) as usize].fill(true);
        }
    }

    pub(super) fn set_units(&mut self, units: &[UnitInstance]) {
        self.units.copy_from_slice(&self.statics);
        Self::mark(&self.models, &mut self.units, units.iter());
    }

    pub(super) fn update<'a>(&mut self, extras: impl Iterator<Item = &'a UnitInstance>) {
        self.frame.copy_from_slice(&self.units);
        // Static scenery must also work before the first simulation upload.
        for (used, fixed) in self.frame.iter_mut().zip(&self.statics) {
            *used |= fixed;
        }
        Self::mark(&self.models, &mut self.frame, extras);
        self.ranges.clear();
        let mut start = None;
        for (i, &used) in self.frame.iter().chain(std::iter::once(&false)).enumerate() {
            match (start, used) {
                (None, true) => start = Some(i as u32),
                (Some(first), false) => {
                    self.ranges.push(first..i as u32);
                    start = None;
                }
                _ => {}
            }
        }
    }

    /// The first model drawing `slot` (a unit blueprint, or past them a prop kind),
    /// for naming it in a breadcrumb.
    pub(super) fn owner(&self, slot: u32) -> Option<usize> {
        self.models
            .iter()
            .position(|&[start, count]| (start..start + count).contains(&slot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(blueprint: u32) -> UnitInstance {
        UnitInstance {
            blueprint,
            ..bytemuck::Zeroable::zeroed()
        }
    }

    #[test]
    fn scenery_keeps_far_lod_and_aliases_keep_shared_mesh() {
        let mut draws = ActiveDraws::new(vec![[0, 3], [0, 3], [3, 3], [6, 3]], 3, 10, &[entity(3)]);
        draws.update(std::iter::empty());
        assert_eq!(draws.ranges, vec![6..10]);
        draws.set_units(&[entity(1)]);
        draws.update(std::iter::empty());
        assert_eq!(draws.ranges, vec![0..3, 6..10]);
        draws.set_units(&[entity(0)]);
        draws.update(std::iter::empty());
        assert_eq!(draws.ranges, vec![0..3, 6..10]);
    }

    #[test]
    fn ghosts_and_falling_trees_are_current_but_units_persist_between_ticks() {
        let mut draws = ActiveDraws::new(vec![[0, 3], [3, 3], [6, 3]], 2, 10, &[]);
        draws.set_units(&[entity(0)]);
        draws.update([entity(1), entity(2)].iter());
        assert_eq!(draws.ranges, vec![0..10]);
        draws.update(std::iter::empty());
        assert_eq!(draws.ranges, vec![0..3]);
        draws.set_units(&[]);
        draws.update(std::iter::empty());
        assert!(draws.ranges.is_empty());
    }
}

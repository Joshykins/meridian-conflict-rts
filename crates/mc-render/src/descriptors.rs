//! Descriptor pools sized from the layouts they serve.
//!
//! A pool must hold every descriptor each set's layout declares, whether or not a
//! binding is ever written. NVIDIA's driver lets an allocation past a pool's sizes
//! through; AMD's refuses it with `ERROR_OUT_OF_POOL_MEMORY`, so a hand-counted pool
//! that is one short works on one machine and stops the game on the other. A
//! `SetPool` is sized from the binding lists themselves, and counts what it hands
//! out: a set it has no room for is refused here, with the line that asked, on
//! every GPU.

use crate::gpu::{Gpu, GpuError};
use ash::vk;
use std::panic::Location;

/// One binding of a set layout: its number and descriptor type (one descriptor).
pub(crate) type Binding = (u32, vk::DescriptorType);

/// A descriptor pool with room for a known list of sets.
pub(crate) struct SetPool {
    raw: vk::DescriptorPool,
    sets_left: u32,
    /// Descriptors of each type still free.
    left: Vec<(vk::DescriptorType, u32)>,
}

/// The pool sizes `sets` need: for each layout, how many sets of it.
fn sizes(sets: &[(&[Binding], u32)]) -> Vec<(vk::DescriptorType, u32)> {
    let mut sizes: Vec<(vk::DescriptorType, u32)> = Vec::new();
    for &(bindings, count) in sets {
        for &(_, ty) in bindings {
            match sizes.iter_mut().find(|(t, _)| *t == ty) {
                Some((_, n)) => *n += count,
                None => sizes.push((ty, count)),
            }
        }
    }
    sizes
}

impl SetPool {
    /// A pool with room for exactly `sets`: each layout's bindings, and how many sets
    /// of it will be allocated.
    #[track_caller]
    pub(crate) fn new(gpu: &Gpu, sets: &[(&[Binding], u32)]) -> Result<SetPool, GpuError> {
        let left = sizes(sets);
        let sets_left = sets.iter().map(|(_, n)| n).sum();
        let pool_sizes: Vec<_> = left
            .iter()
            .map(|&(ty, descriptor_count)| vk::DescriptorPoolSize {
                ty,
                descriptor_count,
            })
            .collect();
        // SAFETY: the device is alive and `pool_sizes` lives to the end of the call.
        let raw = unsafe {
            gpu.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(sets_left)
                    .pool_sizes(&pool_sizes),
                None,
            )
        }?;
        Ok(SetPool {
            raw,
            sets_left,
            left,
        })
    }

    /// A set of `layout`, whose bindings are `bindings`. Refused with
    /// `GpuError::PoolTooSmall`, naming the caller's line, when the pool was not made
    /// with room for it.
    #[track_caller]
    pub(crate) fn alloc(
        &mut self,
        gpu: &Gpu,
        layout: vk::DescriptorSetLayout,
        bindings: &[Binding],
    ) -> Result<vk::DescriptorSet, GpuError> {
        let at = Location::caller();
        take(&mut self.sets_left, &mut self.left, bindings)
            .map_err(|ty| GpuError::pool_too_small(ty, at))?;
        let layouts = [layout];
        // SAFETY: the pool and layout are this device's, the pool has room for the set
        // (counted just above), and `layouts` lives to the end of the call.
        Ok(unsafe {
            gpu.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(self.raw)
                    .set_layouts(&layouts),
            )
        }?[0])
    }

    /// The Vulkan pool, for its owner to destroy (which frees its sets).
    pub(crate) fn into_raw(self) -> vk::DescriptorPool {
        self.raw
    }
}

/// Takes one set of `bindings` out of what is left, or names the descriptor type that
/// ran out (`None`: the pool's count of sets).
fn take(
    sets_left: &mut u32,
    left: &mut [(vk::DescriptorType, u32)],
    bindings: &[Binding],
) -> Result<(), Option<vk::DescriptorType>> {
    if *sets_left == 0 {
        return Err(None);
    }
    let need = sizes(&[(bindings, 1)]);
    for &(ty, n) in &need {
        let room = left.iter().find(|(t, _)| *t == ty).map_or(0, |(_, r)| *r);
        if room < n {
            return Err(Some(ty));
        }
    }
    for (ty, n) in need {
        if let Some((_, room)) = left.iter_mut().find(|(t, _)| *t == ty) {
            *room -= n;
        }
    }
    *sets_left -= 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vk::DescriptorType as T;

    const SCREEN: &[Binding] = &[
        (0, T::SAMPLED_IMAGE),
        (1, T::SAMPLED_IMAGE),
        (2, T::SAMPLER),
        (3, T::UNIFORM_BUFFER),
    ];

    #[test]
    fn a_pool_holds_every_binding_of_its_sets() {
        let left = sizes(&[(SCREEN, 3), (&[(0, T::STORAGE_BUFFER)], 2)]);
        assert_eq!(
            left,
            vec![
                (T::SAMPLED_IMAGE, 6),
                (T::SAMPLER, 3),
                (T::UNIFORM_BUFFER, 3),
                (T::STORAGE_BUFFER, 2),
            ]
        );
    }

    /// The terrain lighting's pool had room for two sampled images and was asked for a
    /// whole screen set: AMD refused it at start-up. It is refused here first.
    #[test]
    fn a_set_bigger_than_the_room_left_is_refused() {
        let mut left = vec![(T::SAMPLED_IMAGE, 2)];
        let mut sets = 1;
        assert_eq!(take(&mut sets, &mut left, SCREEN), Err(Some(T::SAMPLER)));
        assert_eq!(sets, 1, "a refused set takes nothing");

        let mut left = sizes(&[(SCREEN, 1)]);
        assert_eq!(take(&mut sets, &mut left, SCREEN), Ok(()));
        assert_eq!(
            take(&mut sets, &mut left, SCREEN),
            Err(None),
            "one set was asked for"
        );
    }
}

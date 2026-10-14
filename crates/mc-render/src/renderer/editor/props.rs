//! Replace scenery at a frame boundary; failed allocations keep the old scene.
use super::super::{city_fx, cliff_rocks, cull_lists::CullLists, map_props, Renderer};
use crate::gpu::{Buffer, Gpu, GpuError};
use ash::vk;
use mc_map::{edit::EditMap, placed::Placed, PropKind};

/// Own both candidate and retired buffers until the swap has completed.
struct Pending<'a> {
    gpu: &'a Gpu,
    statics: Buffer,
    dead: Buffer,
    city: Option<city_fx::CityFx>,
    cull: Option<CullLists>,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.gpu
            .destroy_buffer(std::mem::replace(&mut self.statics, Buffer::null()));
        self.gpu
            .destroy_buffer(std::mem::replace(&mut self.dead, Buffer::null()));
        if let Some(city) = &mut self.city {
            city.destroy(self.gpu);
        }
        if let Some(cull) = &mut self.cull {
            cull.destroy(self.gpu);
        }
    }
}
impl Renderer {
    /// Rebuild authored props, their collision presentation and cliff scenery.
    /// Models, pipelines, camera and terrain residency stay in place.
    #[track_caller]
    pub fn edit_props(&mut self, map: &EditMap, placed: &Placed) -> Result<(), GpuError> {
        let components: std::collections::BTreeMap<_, _> = placed
            .doodads
            .iter()
            .map(|d| (d.prop_index as usize, d))
            .collect();
        let component = |i| components.get(&i).copied();
        let mut instances = map_props::build(
            &map.props,
            |i| {
                component(i)
                    .map(|d| d.tint)
                    .or_else(|| map.tints.get(i).copied())
            },
            &self.tile_cache,
            &[],
            self.tree_model_base,
        );
        let first = instances.len();
        instances.extend(cliff_rocks::edited(
            map,
            self.editor.cliff_base,
            first as u32,
        ));
        let mut cliffs = cliff_rocks::CliffRocks::new(&instances, first);
        if let Some((look, _, _)) = &self.editor.environment {
            cliffs.set_look(look);
        }
        let rubble = PropKind::ALL
            .iter()
            .position(|k| *k == PropKind::CityRubble)
            .unwrap_or(0) as u32;
        let (heaps, list) = city_fx::heaps_for(
            &map.props,
            component,
            instances.len() as u32,
            self.tree_model_base + rubble,
            |p| self.tile_cache.overview_height(p),
        );
        instances.extend(heaps);
        let count = instances.len() as u32;
        let mut pending = Pending {
            gpu: &self.gpu,
            statics: Buffer::null(),
            dead: Buffer::null(),
            city: None,
            cull: None,
        };
        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        let bytes = bytemuck::cast_slice(&instances);
        pending.statics = self.gpu.buffer_with_data(
            if bytes.is_empty() {
                &[0; std::mem::size_of::<mc_sim::mirror::UnitInstance>()]
            } else {
                bytes
            },
            storage,
        )?;
        pending.dead = self
            .gpu
            .host_buffer((count as u64).div_ceil(32).max(1) * 4, storage)?;
        pending.dead.write(0, &vec![0; pending.dead.size as usize]);
        pending.city = Some(city_fx::CityFx::from_props(
            &self.gpu,
            glam::Vec2::from(map.info.size_metres().to_f32()),
            &map.props,
            component,
            0,
            list,
        )?);
        pending.cull = Some(self.cull.for_statics(&self.gpu, &instances)?);
        // A new scenery buffer starts with live structures and hidden rubble,
        // even if its caller has no new simulation frame to upload yet.
        let mut hidden = cliffs.dead_props(&[]).to_vec();
        hidden.resize(pending.dead.size as usize / 4, 0);
        if let Some(city) = &mut pending.city {
            city.hide_heaps(&[], &mut hidden);
        }
        pending.dead.write(0, bytemuck::cast_slice(&hidden));
        self.gpu.wait_idle();
        std::mem::swap(&mut self.statics, &mut pending.statics);
        std::mem::swap(&mut self.props_dead, &mut pending.dead);
        if let Some(city) = &mut pending.city {
            std::mem::swap(&mut self.city_fx, city);
        }
        if let Some(cull) = &mut pending.cull {
            std::mem::swap(&mut self.cull, cull);
        }
        drop(pending);
        self.static_count = count;
        self.prop_instances = instances;
        self.cliff_rocks = cliffs;
        self.previous_dead.clear();
        self.bind_edited_props();
        self.edit_scenery_cover(map)?;
        Ok(())
    }
    fn bind_edited_props(&self) {
        let write = |set, binding, buffer: &Buffer| {
            let info = [buffer.info()];
            let writes = [vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&info)];
            // SAFETY: the GPU is idle; these live buffers match the existing
            // storage-buffer bindings, and the slices outlive this call.
            unsafe {
                self.gpu.device.update_descriptor_sets(&writes, &[]);
            }
        };
        for (binding, buffer) in [
            (2, &self.statics),
            (4, &self.cull.visible),
            (31, &self.cull.sway),
            (33, self.city_fx.buffer()),
            (40, &self.cull.commands),
        ] {
            write(self.scene_set, binding, buffer);
        }
        write(self.cull_set, 2, &self.statics);
        for (i, buffer) in self.cull.bindings().iter().enumerate() {
            write(self.cull_set, 5 + i as u32, buffer);
        }
        for (binding, buffer) in [
            (9, &self.props_dead),
            (11, &self.cull.sway),
            (12, &self.cull.pulled),
        ] {
            write(self.cull_set, binding, buffer);
        }
    }
}

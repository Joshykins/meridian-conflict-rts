//! Every model's gun houses (`mc_models::House`) in one list, scene set binding 43: each
//! house's pivot, how far its recoiling pieces kick back, and the gun slot it is bound to
//! (`mc_data::UnitBlueprint::gun_slot`). `ModelInfo::houses` finds a model's run; a house
//! vertex finds its slot in that run from its rig bits (`gpu_consts::house`).

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::house;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use mc_models::House;

// The gun slots a `HousePose` holds and a model's houses are the sim's and the data's.
const _: () = assert!(
    mc_data::MAX_HOUSES == house::GUNS_PER_RECORD as usize
        && mc_data::MAX_GUNS == house::MAX as usize
);

/// One house as the shaders read it (common.wgsl `ModelHouse`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuHouse {
    pub(crate) pivot: [f32; 3],
    pub(crate) travel: f32,
    pub(crate) gun: u32,
    pub(crate) _pad: [u32; 3],
}

/// The houses gathered model by model while the renderer builds its model table.
#[derive(Default)]
pub(crate) struct HouseList {
    houses: Vec<GpuHouse>,
}

impl HouseList {
    /// Adds a model's houses; returns its `ModelInfo::houses` word (first | count <<
    /// `house::COUNT_SHIFT`). A model has at most `house::MAX` (`MeshBuilder::with_house`
    /// refuses more).
    pub(crate) fn add(&mut self, houses: &[House]) -> u32 {
        let first = self.houses.len() as u32;
        let count = houses.len().min(house::MAX as usize);
        self.houses.extend(houses[..count].iter().map(|h| GpuHouse {
            pivot: h.pivot,
            travel: h.travel,
            gun: h.gun as u32,
            _pad: [0; 3],
        }));
        first | (count as u32) << house::COUNT_SHIFT
    }
}

/// The houses' buffer, made once with the model table.
pub(crate) struct ModelHouses {
    buffer: Buffer,
}

impl ModelHouses {
    pub(crate) fn new(gpu: &Gpu, list: &HouseList) -> Result<ModelHouses, GpuError> {
        // A model table without houses still binds one record.
        let table: Vec<GpuHouse> = if list.houses.is_empty() {
            vec![GpuHouse::zeroed()]
        } else {
            list.houses.clone()
        };
        let bytes: &[u8] = bytemuck::cast_slice(&table);
        let buffer = gpu.host_buffer(bytes.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER)?;
        buffer.write(0, bytes);
        Ok(ModelHouses { buffer })
    }

    /// What the scene set's binding 43 reads.
    pub(crate) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Frees the buffer. The renderer calls this from its `Drop`, after the device has gone
    /// idle.
    pub(crate) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
    }
}

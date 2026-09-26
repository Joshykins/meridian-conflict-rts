//! Named GPU timing scopes, with optional pipeline statistics (triangles,
//! shader invocations) for the leaf scopes that draw.
//!
//! ```ignore
//! self.timers.scope(device, cmd, "scene");        // time only; may nest, may span passes
//! self.timers.draws(device, cmd, "scene.terrain"); // time + triangles/fragments
//! ...
//! self.timers.end(device, cmd);                   // closes the innermost open scope
//! ```
//!
//! `draws` scopes must not nest in each other and must begin and end in the
//! same render pass (or both outside one): Vulkan allows one active pipeline
//! statistics query per command buffer and forbids it crossing a pass edge.
//! Results arrive one frame late, after the fence, as [`GpuScope`]s.

use std::cell::RefCell;

use ash::vk;

/// Timestamps per frame, two per scope.
const MAX_SCOPES: u32 = 96;

/// The statistics we ask for, in the order Vulkan returns them (bit order).
const STATS: vk::QueryPipelineStatisticFlags = vk::QueryPipelineStatisticFlags::from_raw(
    vk::QueryPipelineStatisticFlags::INPUT_ASSEMBLY_PRIMITIVES.as_raw()
        | vk::QueryPipelineStatisticFlags::VERTEX_SHADER_INVOCATIONS.as_raw()
        | vk::QueryPipelineStatisticFlags::CLIPPING_PRIMITIVES.as_raw()
        | vk::QueryPipelineStatisticFlags::FRAGMENT_SHADER_INVOCATIONS.as_raw()
        | vk::QueryPipelineStatisticFlags::COMPUTE_SHADER_INVOCATIONS.as_raw(),
);
const STAT_COUNT: usize = 5;

/// What one draw scope did on the GPU.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawStats {
    /// Primitives the input assembler built (triangles submitted).
    pub triangles_in: u64,
    pub vertex_invocations: u64,
    /// Primitives that survived clipping (roughly: on screen).
    pub triangles_out: u64,
    /// Fragment shader invocations; over the pixel count this is overdraw.
    pub fragments: u64,
    pub compute_invocations: u64,
}

/// One measured scope of the previous frame.
#[derive(Clone, Debug)]
pub struct GpuScope {
    pub name: &'static str,
    /// Nesting depth, 0 for top level.
    pub depth: u8,
    pub ms: f32,
    pub stats: Option<DrawStats>,
}

#[derive(Default)]
struct Recording {
    /// Scope name, depth and statistics slot, by timestamp pair.
    scopes: Vec<(&'static str, u8, Option<u32>)>,
    open: Vec<u32>,
    /// The scope holding the active statistics query.
    stats_open: Option<u32>,
    stats_used: u32,
    overflow: bool,
}

pub struct GpuTimers {
    timestamps: vk::QueryPool,
    stats: Option<vk::QueryPool>,
    period: f32,
    want_stats: bool,
    rec: RefCell<Recording>,
    valid: bool,
    /// `MERIDIAN_GPU_TIMERS=0` records nothing, to A/B the timers' own cost.
    on: bool,
}

impl GpuTimers {
    pub fn new(gpu: &crate::gpu::Gpu) -> Result<GpuTimers, vk::Result> {
        let timestamps = unsafe {
            gpu.device.create_query_pool(
                &vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    .query_count(MAX_SCOPES * 2),
                None,
            )
        }?;
        let stats = if gpu.pipeline_stats {
            Some(unsafe {
                gpu.device.create_query_pool(
                    &vk::QueryPoolCreateInfo::default()
                        .query_type(vk::QueryType::PIPELINE_STATISTICS)
                        .pipeline_statistics(STATS)
                        .query_count(MAX_SCOPES),
                    None,
                )
            }?)
        } else {
            None
        };
        Ok(GpuTimers {
            timestamps,
            stats,
            period: gpu.limits.timestamp_period,
            want_stats: std::env::var("MERIDIAN_GPU_STATS").is_ok_and(|v| v == "1"),
            rec: RefCell::new(Recording::default()),
            valid: false,
            on: std::env::var("MERIDIAN_GPU_TIMERS").map_or(true, |v| v != "0"),
        })
    }

    /// Turns pipeline statistics on or off from the next frame (the profiler
    /// panel and `--perf` turn them on; they cost a little).
    pub fn set_stats(&mut self, on: bool) {
        self.want_stats = on;
    }

    /// Call once per frame right after the command buffer begins.
    pub fn reset(&self, device: &ash::Device, cmd: vk::CommandBuffer) {
        unsafe {
            device.cmd_reset_query_pool(cmd, self.timestamps, 0, MAX_SCOPES * 2);
            if let Some(pool) = self.stats {
                device.cmd_reset_query_pool(cmd, pool, 0, MAX_SCOPES);
            }
        }
        *self.rec.borrow_mut() = Recording::default();
    }

    fn open(&self, device: &ash::Device, cmd: vk::CommandBuffer, name: &'static str, stats: bool) {
        if !self.on {
            return;
        }
        let mut rec = self.rec.borrow_mut();
        let index = rec.scopes.len() as u32;
        if index >= MAX_SCOPES {
            rec.overflow = true;
            rec.open.push(u32::MAX);
            return;
        }
        let depth = rec.open.len() as u8;
        let slot = match self.stats {
            Some(pool) if stats && self.want_stats && rec.stats_open.is_none() => {
                let slot = rec.stats_used;
                rec.stats_used += 1;
                rec.stats_open = Some(index);
                unsafe {
                    device.cmd_begin_query(cmd, pool, slot, vk::QueryControlFlags::empty())
                };
                Some(slot)
            }
            _ => None,
        };
        debug_assert!(!stats || slot.is_some() || !self.want_stats || self.stats.is_none(),
            "draws scope {name} nested in another draws scope");
        rec.scopes.push((name, depth, slot));
        rec.open.push(index);
        unsafe {
            device.cmd_write_timestamp(
                cmd,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.timestamps,
                index * 2,
            )
        };
    }

    /// Opens a timing scope. Scopes nest and may cross render pass edges.
    pub fn scope(&self, device: &ash::Device, cmd: vk::CommandBuffer, name: &'static str) {
        self.open(device, cmd, name, false);
    }

    /// Opens a timing scope that also counts triangles and shader invocations.
    pub fn draws(&self, device: &ash::Device, cmd: vk::CommandBuffer, name: &'static str) {
        self.open(device, cmd, name, true);
    }

    /// Closes the innermost open scope.
    pub fn end(&self, device: &ash::Device, cmd: vk::CommandBuffer) {
        if !self.on {
            return;
        }
        let mut rec = self.rec.borrow_mut();
        let Some(index) = rec.open.pop() else {
            debug_assert!(false, "gpu timer end without begin");
            return;
        };
        if index == u32::MAX {
            return;
        }
        unsafe {
            device.cmd_write_timestamp(
                cmd,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.timestamps,
                index * 2 + 1,
            )
        };
        if rec.stats_open == Some(index) {
            rec.stats_open = None;
            let slot = rec.scopes[index as usize].2.expect("stats slot");
            unsafe { device.cmd_end_query(cmd, self.stats.expect("stats pool"), slot) };
        }
    }

    /// Call after the frame's commands are submitted.
    pub fn submitted(&mut self) {
        debug_assert!(self.rec.borrow().open.is_empty(), "gpu timer scope left open");
        self.valid = true;
    }

    /// Forget the in-flight frame (after a resize rebuilt the command state).
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Reads the last submitted frame. Call after its fence has signalled.
    pub fn read(&self, device: &ash::Device) -> Option<Vec<GpuScope>> {
        if !self.valid || self.period <= 0.0 {
            return None;
        }
        let rec = self.rec.borrow();
        let n = rec.scopes.len();
        if n == 0 {
            return Some(Vec::new());
        }
        let mut ticks = vec![0u64; n * 2];
        unsafe {
            device.get_query_pool_results(
                self.timestamps,
                0,
                &mut ticks,
                vk::QueryResultFlags::TYPE_64,
            )
        }
        .ok()?;
        let mut stats = vec![[0u64; STAT_COUNT]; rec.stats_used as usize];
        if let (Some(pool), true) = (self.stats, rec.stats_used > 0) {
            let read = unsafe {
                device.get_query_pool_results(pool, 0, &mut stats, vk::QueryResultFlags::TYPE_64)
            };
            if read.is_err() {
                stats.clear();
            }
        }
        Some(
            rec.scopes
                .iter()
                .enumerate()
                .map(|(i, &(name, depth, slot))| GpuScope {
                    name,
                    depth,
                    ms: ticks[i * 2 + 1].saturating_sub(ticks[i * 2]) as f32 * self.period / 1e6,
                    stats: slot.and_then(|s| stats.get(s as usize)).map(|v| DrawStats {
                        triangles_in: v[0],
                        vertex_invocations: v[1],
                        triangles_out: v[2],
                        fragments: v[3],
                        compute_invocations: v[4],
                    }),
                })
                .collect(),
        )
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_query_pool(self.timestamps, None);
            if let Some(pool) = self.stats {
                device.destroy_query_pool(pool, None);
            }
        }
    }
}

/// Puts a frame's scopes into a perf frame: `gpu.<name>` spans, and for draw
/// scopes `gpu.<name>.tris_in`, `.tris_out`, `.fragments`, `.compute` counters.
pub fn to_perf(scopes: &[GpuScope], frame: &mut mc_core::perf::Frame) {
    use mc_core::perf::intern;
    let mut total = 0.0f64;
    for s in scopes {
        if s.depth == 0 {
            total += s.ms as f64;
        }
        frame.push(intern(&format!("gpu.{}", s.name)), 1, Some((s.ms as f64 * 1e6) as u64));
        if let Some(st) = s.stats {
            for (suffix, v) in [
                ("tris_in", st.triangles_in),
                ("tris_out", st.triangles_out),
                ("fragments", st.fragments),
                ("vertices", st.vertex_invocations),
                ("compute", st.compute_invocations),
            ] {
                if v > 0 {
                    frame.push(intern(&format!("gpu.{}.{suffix}", s.name)), v, None);
                }
            }
        }
    }
    frame.push("gpu", 1, Some((total * 1e6) as u64));
}

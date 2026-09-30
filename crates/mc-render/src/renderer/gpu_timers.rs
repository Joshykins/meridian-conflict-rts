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
//!
//! Every scope's edges are also breadcrumbs (`breadcrumbs.rs`), so a lost device
//! can say where the GPU stopped.

use std::cell::RefCell;

use super::breadcrumbs::Breadcrumbs;

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

pub(super) struct GpuTimers {
    timestamps: vk::QueryPool,
    stats: Option<vk::QueryPool>,
    period: f32,
    want_stats: bool,
    rec: RefCell<Recording>,
    valid: bool,
    /// `MERIDIAN_GPU_TIMERS=0` records nothing, to A/B the timers' own cost.
    on: bool,
    breadcrumbs: Option<Breadcrumbs>,
}

impl GpuTimers {
    pub(super) fn new(gpu: &crate::gpu::Gpu) -> Result<GpuTimers, vk::Result> {
        // SAFETY: the device is alive and the create info lives to the end of the call.
        let timestamps = unsafe {
            gpu.device.create_query_pool(
                &vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    .query_count(MAX_SCOPES * 2),
                None,
            )
        }?;
        let stats = if gpu.pipeline_stats {
            // SAFETY: pipeline statistics queries are enabled on the device
            // (`gpu.pipeline_stats` is set only when the feature is), and the create info lives
            // to the end of the call.
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
        // SAFETY: `physical` belongs to the live instance and `queue_family` is the
        // family its queue was created in, so it indexes the returned list.
        let timestamp_bits = unsafe {
            gpu.instance
                .get_physical_device_queue_family_properties(gpu.physical)
        }[gpu.queue_family as usize]
            .timestamp_valid_bits;
        let breadcrumbs = Breadcrumbs::new(gpu).map_err(|e| match e {
            crate::gpu::GpuError::Vk(e) => e,
            _ => vk::Result::ERROR_OUT_OF_DEVICE_MEMORY,
        })?;
        Ok(GpuTimers {
            breadcrumbs,
            timestamps,
            stats,
            period: gpu.limits.timestamp_period,
            want_stats: std::env::var("MERIDIAN_GPU_STATS").is_ok_and(|v| v == "1"),
            rec: RefCell::new(Recording::default()),
            valid: false,
            // A queue without timestamp bits cannot write timestamps at all.
            on: timestamp_bits > 0
                && std::env::var("MERIDIAN_GPU_TIMERS").map_or(true, |v| v != "0"),
        })
    }

    /// Turns pipeline statistics on or off from the next frame (the profiler
    /// panel and `--perf` turn them on; they cost a little).
    pub(super) fn set_stats(&mut self, on: bool) {
        self.want_stats = on;
    }

    /// Call once per frame right after the command buffer begins.
    pub(super) fn reset(&self, device: &ash::Device, cmd: vk::CommandBuffer) {
        // SAFETY: the renderer calls this right after `cmd` begins, outside any render pass,
        // with its own device; the ranges are exactly the pools' sizes, and the GPU is done
        // with the last frame's queries (its fence was waited on).
        unsafe {
            device.cmd_reset_query_pool(cmd, self.timestamps, 0, MAX_SCOPES * 2);
            if let Some(pool) = self.stats {
                device.cmd_reset_query_pool(cmd, pool, 0, MAX_SCOPES);
            }
        }
        *self.rec.borrow_mut() = Recording::default();
        if let Some(b) = &self.breadcrumbs {
            b.next_frame();
        }
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
                // SAFETY: `cmd` is recording (callers pass the renderer's frame buffer); `slot`
                // < `MAX_SCOPES` since at most one stats slot is taken per scope, the slot was
                // reset this frame, the pool exists only with the feature on, and no other
                // statistics query is active (`stats_open` is none). Callers keep `draws`
                // scopes un-nested and inside one pass, as the module doc says (debug-
                // asserted).
                unsafe { device.cmd_begin_query(cmd, pool, slot, vk::QueryControlFlags::empty()) };
                Some(slot)
            }
            _ => None,
        };
        debug_assert!(
            !stats || slot.is_some() || !self.want_stats || self.stats.is_none(),
            "draws scope {name} nested in another draws scope"
        );
        rec.scopes.push((name, depth, slot));
        rec.open.push(index);
        if let Some(b) = &self.breadcrumbs {
            b.point(cmd, format!("{}{name}", "  ".repeat(depth as usize)).into());
        }
        // SAFETY: `cmd` is recording; `index * 2` < `MAX_SCOPES * 2` (checked above), that
        // query was reset this frame, and `new` turns the timers off on a queue without
        // timestamp bits.
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
    pub(super) fn scope(&self, device: &ash::Device, cmd: vk::CommandBuffer, name: &'static str) {
        self.open(device, cmd, name, false);
    }

    /// Opens a timing scope that also counts triangles and shader invocations.
    pub(super) fn draws(&self, device: &ash::Device, cmd: vk::CommandBuffer, name: &'static str) {
        self.open(device, cmd, name, true);
    }

    /// Closes the innermost open scope.
    pub(super) fn end(&self, device: &ash::Device, cmd: vk::CommandBuffer) {
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
        // SAFETY: `cmd` is recording; `index` is an opened scope below `MAX_SCOPES`, so `index
        // * 2 + 1` is in the pool, reset this frame and not yet written.
        unsafe {
            device.cmd_write_timestamp(
                cmd,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.timestamps,
                index * 2 + 1,
            )
        };
        if let Some(b) = &self.breadcrumbs {
            let (name, depth, _) = rec.scopes[index as usize];
            b.point(
                cmd,
                format!("{}end {name}", "  ".repeat(depth as usize)).into(),
            );
        }
        if rec.stats_open == Some(index) {
            rec.stats_open = None;
            let slot = rec.scopes[index as usize].2.expect("stats slot");
            // SAFETY: this closes the statistics query `open` began for this scope in the same
            // command buffer, and callers end a `draws` scope in the pass it began in.
            unsafe { device.cmd_end_query(cmd, self.stats.expect("stats pool"), slot) };
        }
    }

    /// Call after the frame's commands are submitted.
    pub(super) fn submitted(&mut self) {
        debug_assert!(
            self.rec.borrow().open.is_empty(),
            "gpu timer scope left open"
        );
        self.valid = true;
    }

    /// Forget the in-flight frame (after a resize rebuilt the command state).
    pub(super) fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Reads the last submitted frame. Call after its fence has signalled.
    pub(super) fn read(&self, device: &ash::Device) -> Option<Vec<GpuScope>> {
        if !self.valid || self.period <= 0.0 {
            return None;
        }
        let rec = self.rec.borrow();
        let n = rec.scopes.len();
        if n == 0 {
            return Some(Vec::new());
        }
        let mut ticks = vec![0u64; n * 2];
        // SAFETY: the renderer calls this after waiting on the fence of the frame that wrote
        // these queries; `ticks` holds exactly `n * 2` 64-bit results, the queries recorded
        // then, and no WAIT flag is used, so unwritten queries only return NOT_READY.
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
            // SAFETY: as above: after the frame's fence, `stats` holds `stats_used` results of
            // `STAT_COUNT` 64-bit values each, one per statistic the pool was made with.
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

    /// After the device is lost: logs where the GPU stopped (`breadcrumbs.rs`).
    pub(super) fn lost(&self) {
        match &self.breadcrumbs {
            Some(b) => b.report(),
            None => log::error!("device lost; no breadcrumbs (no VK_AMD_buffer_marker)"),
        }
    }

    /// A breadcrumb before one draw, named by `label`, when fine crumbs are on
    /// (`MERIDIAN_GPU_CRUMBS=1`).
    pub(super) fn crumb(&self, cmd: vk::CommandBuffer, label: impl FnOnce() -> String) {
        if let Some(b) = self.breadcrumbs.as_ref().filter(|b| b.fine) {
            b.point(cmd, label().into());
        }
    }

    /// Fine crumbs are on: models are drawn a slot at a time, each with its crumb.
    pub(super) fn fine(&self) -> bool {
        self.breadcrumbs.as_ref().is_some_and(|b| b.fine)
    }

    pub(super) fn destroy(&mut self, gpu: &crate::gpu::Gpu) {
        // SAFETY: the pools were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle.
        unsafe {
            gpu.device.destroy_query_pool(self.timestamps, None);
            if let Some(pool) = self.stats {
                gpu.device.destroy_query_pool(pool, None);
            }
        }
        if let Some(b) = self.breadcrumbs.take() {
            b.destroy(gpu);
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
        frame.push(
            intern(&format!("gpu.{}", s.name)),
            1,
            Some((s.ms as f64 * 1e6) as u64),
        );
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

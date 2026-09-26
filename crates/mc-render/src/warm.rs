//! Compiling many pipelines at once. With a warm driver cache a pipeline is
//! made in a millisecond; after a shader changes every one compiles again, the
//! big ones for seconds each, and made one after another that came to over a
//! minute before the first picture. `warmed` runs a constructor once to list
//! the pipelines it asks for (compiling nothing), compiles that list on
//! several threads into one pipeline cache, then runs it for real against the
//! cache.

use crate::gpu::{Gpu, GpuError};
use crate::pipelines::{self, Blend, Depth, PipelineDesc, VertexKind};
use ash::vk;
use std::cell::{Cell, RefCell};
use std::ffi::CString;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A pipeline a constructor asked for while being listed.
pub(crate) enum Wanted {
    Graphics {
        module: vk::ShaderModule,
        vs: CString,
        fs: CString,
        layout: vk::PipelineLayout,
        pass: vk::RenderPass,
        vertex: VertexKind,
        blend: Blend,
        depth: Depth,
        cull: vk::CullModeFlags,
    },
    Compute {
        module: vk::ShaderModule,
        entry: CString,
        layout: vk::PipelineLayout,
    },
}

thread_local! {
    /// Set while a constructor is being listed.
    static LISTING: RefCell<Option<Vec<Wanted>>> = const { RefCell::new(None) };
    /// The cache the real run finds its pipelines in.
    static CACHE: Cell<vk::PipelineCache> = const { Cell::new(vk::PipelineCache::null()) };
}

/// While a constructor is being listed, notes what it wants and says so: the
/// caller then hands back a null pipeline instead of compiling one.
pub(crate) fn listed(wanted: impl FnOnce() -> Wanted) -> bool {
    LISTING.with(|l| match &mut *l.borrow_mut() {
        Some(list) => {
            list.push(wanted());
            true
        }
        None => false,
    })
}

/// The cache pipelines are made against on this thread (null outside `warmed`).
pub(crate) fn cache() -> vk::PipelineCache {
    CACHE.with(Cell::get)
}

/// Runs `build` to list its pipelines, compiles them all at once, and runs
/// `build` again for real. `build` must only make objects, not use them: its
/// listing run gets null pipelines, and `destroy` takes that run's result down.
pub(crate) fn warmed<T>(
    gpu: &Gpu,
    build: impl Fn() -> Result<T, GpuError>,
    destroy: impl FnOnce(T),
) -> Result<T, GpuError> {
    // SAFETY: plain object creation on a live device.
    let cache = unsafe { gpu.device.create_pipeline_cache(&vk::PipelineCacheCreateInfo::default(), None) }?;
    LISTING.with(|l| *l.borrow_mut() = Some(Vec::new()));
    let shell = build();
    let wanted = LISTING.with(|l| l.borrow_mut().take()).unwrap_or_default();
    let result = shell.and_then(|shell| {
        let started = std::time::Instant::now();
        compile_all(gpu, cache, &wanted);
        log::debug!(
            "{} pipelines compiled in parallel in {:.0} ms",
            wanted.len(),
            started.elapsed().as_secs_f32() * 1000.0
        );
        // The listing run's shader modules stay alive until the list is compiled.
        destroy(shell);
        CACHE.with(|c| c.set(cache));
        let real = build();
        CACHE.with(|c| c.set(vk::PipelineCache::null()));
        real
    });
    // SAFETY: every pipeline made against the cache has been made; it is used no more.
    unsafe { gpu.device.destroy_pipeline_cache(cache, None) };
    result
}

/// Makes each wanted pipeline against `cache` and throws it away: what is kept
/// is the compiled code in the cache. A failure is left for the real run to report.
fn compile_all(gpu: &Gpu, cache: vk::PipelineCache, wanted: &[Wanted]) {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 8)
        .min(wanted.len().max(1));
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                while let Some(w) = wanted.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let made = match w {
                        Wanted::Graphics { module, vs, fs, layout, pass, vertex, blend, depth, cull } => {
                            let desc = PipelineDesc {
                                module: *module,
                                vs,
                                fs,
                                layout: *layout,
                                pass: *pass,
                                vertex: *vertex,
                                blend: *blend,
                                depth: *depth,
                                cull: *cull,
                            };
                            pipelines::create_graphics(gpu, cache, &desc)
                        }
                        Wanted::Compute { module, entry, layout } => {
                            pipelines::create_compute(gpu, cache, *module, entry, *layout)
                        }
                    };
                    if let Ok(pipeline) = made {
                        // SAFETY: never bound; nothing else holds it.
                        unsafe { gpu.device.destroy_pipeline(pipeline, None) };
                    }
                }
            });
        }
    });
}

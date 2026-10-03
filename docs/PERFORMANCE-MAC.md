# M3 Pro rendering performance — September 28, 2026

Measured on an Apple M3 Pro (18 GPU cores, 36 GB unified memory), macOS 26.5.1,
using Vulkan through MoltenVK. Baseline: `1e9cf9ec19811845d294a60ec9ff9beb2de5c806`.

## Results

Paired headless captures used the same scene, simulation ticks, camera, output size
and Low-equivalent settings. Each capture contains 120 GPU frame samples. Values
below are milliseconds; lower is better. These are GPU timings, not measured
windowed presentation rates. Background activity and thermal state can affect them.

| Scene | Median before | Median after | Reduction | p95 before | p95 after |
|---|---:|---:|---:|---:|---:|
| Battle, medium camera | 20.85 | 14.34 | 31% | 21.55 | 14.91 |
| Battle, map overview | 42.34 | 34.49 | 19% | 46.33 | 37.28 |
| Overcast, low camera | 44.70 | 37.44 | 16% | 45.74 | 38.17 |
| Twin Shoals, map overview | 31.72 | 29.03 | 8% | 33.38 | 30.80 |

The battle capture now fits within a 16.67 ms GPU budget. The overview and overcast
captures still exceed that budget and need more work for consistent 60 FPS.
Twin Shoals uses the headless overview camera, not the normal commander view.
Windows/Linux runtime performance has not been measured in this pass.

## Changes and tradeoffs

- Low and Medium use fewer terrain texture samples, hardware shadow filtering,
  and one quarter of the full-resolution cloud-shadow texels per frame after
  initialization. Texture repetition and sharper shadow edges may be more visible.
  High and Ultra retain their original shading quality.
- Skip indirect draw slots for absent models while retaining every LOD for live
  units, static scenery, build ghosts and falling/burning trees. The battle case
  submits 100 occupied model slots instead of all 587. GPU visibility culling stays
  active, and shared meshes are retained.
- Skip negligible blast influence outside six radii in the visual weather field.
- Extend the performance suite to native macOS/Linux and add active-slot counters.

Existing saved Low/Medium (formerly Balanced) settings gain the shading changes automatically;
custom resolution and antialiasing settings remain intact. The deterministic
simulation is unchanged.

## Reproduce

Build the baseline and updated revisions separately, retaining a copy of each
binary. The reported captures used the `shot` profile for both builds. Then run
the suite once per binary (absolute paths required for `PERF_EXE`):

```sh
PERF_EXE=/absolute/path/to/meridian \
PERF_SIZE=2560x1600 PERF_FOLLOW=60 \
PERF_ENV='MERIDIAN_RENDER_SCALE=0.5 MERIDIAN_AA=off MERIDIAN_PROP_DETAIL=6,4,8 MERIDIAN_CLOUD_RES=4 MERIDIAN_SIMPLE_SHADING=1' \
scripts/perf-suite.sh artifacts/perf-mac/after battle_mid overview_far overcast_low
```

For Twin Shoals, use the same environment overrides with:

```sh
/absolute/path/to/meridian --map twin_shoals --ticks 100 --follow 60 \
  --size 2560x1600 --screenshot artifacts/perf-mac/after/twin_shoals.png \
  --perf artifacts/perf-mac/after/twin_shoals.json
```

Direct binary invocations on macOS also need the Vulkan library/driver environment
that `play.sh` and the suite normally set. Raw logs, JSON timing reports and images
from this machine are in the local, ignored `artifacts/perf-mac/{before,after}`
directories.

## Verification

- Workspace formatting and Clippy with warnings denied passed.
- The complete workspace test suite passed with the `gate` profile. A final
  renderer/game test run also passed after adding counters and compatibility tests.
- Shader compilation and generated Rust/WGSL layout tests passed.
- Windowed menu → match → menu smoke test passed with a temporary settings folder.
  Khronos validation layers are not installed on this machine, so API validation
  was unavailable.
- Before/after screenshots were inspected for missing objects and rendering defects.
- Native benchmark runner smoke checks covered output paths with spaces and error
  propagation. Hardware captures exercised the native MoltenVK path.

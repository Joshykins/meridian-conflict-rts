# Rendering performance switches

Headless screenshots and performance captures use these environment overrides.
Interactive play takes its graphics settings from Settings → Display instead.

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_SIMPLE_SHADING` | `1` to enable, otherwise off | Single-patch terrain textures, hardware shadow filtering and staggered cloud-shadow updates. Low and Balanced enable this automatically in interactive play. |
| `MERIDIAN_RENDER_SCALE` | `0.5`–`2.0` | Scene resolution relative to output; UI stays at output resolution. |
| `MERIDIAN_AA` | `off`, `smaa` | Edge smoothing. |
| `MERIDIAN_PROP_DETAIL` | `minimum radius,LOD bias,shadow radius` | Scenery detail; Low is `6,4,8`, Balanced is `4,3,6`. |
| `MERIDIAN_CLOUD_RES` | `1`–`4` | Divide output dimensions by this for the cloud march. |
| `MERIDIAN_GPU_TIMERS` | `0` to disable | A/B profiling overhead. GPU timing values are unavailable when off; compare `cpu.render` instead. |

`scripts/perf-suite.sh OUT_DIR [CASE...]` runs native macOS/Linux or Windows via WSL.
`PERF_SIZE`, `PERF_FOLLOW`, and whitespace-separated `PERF_ENV` assignments set the
capture size, followed ticks and overrides. Native `PERF_EXE` selects an existing
binary; otherwise the suite uses `play.sh`. Commands and shell expansions are not
supported inside `PERF_ENV`.

## Interface shots

Headless screenshots (`--screenshot`, `scripts/shot.sh run`) can show an order being aimed.

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_AIM` | `1`, `ground`, `reclaim`, `warp`, `formation:DEG` | The order in hand, aimed at `--cursor`: a warhead launch (`1`, docs/NUKES.md), a titan's strike, Reclaim, a warp jump (the line from each selected ship to its exit in formation, and the energy card), or a move held on the right button (the selection's formation at `--cursor`, turned to face DEG degrees; `formation` alone faces the way it goes). |
| `MERIDIAN_ISSUE_NOTE` | any text | The F1 report card's note, as if typed (the field wraps and grows with a long one). |
| `MERIDIAN_GROUPS` | `KEY,KEY,...` | Control groups 2, 3, ... hold player 0's units whose blueprint key contains each KEY (group 1 is the selection): the groups card and the numbers by the units. |

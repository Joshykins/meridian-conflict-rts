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
| `MERIDIAN_GPU_CRUMBS` | `0` to disable | Finding a lost device on AMD GPUs (`VK_AMD_buffer_marker`), on by default there: a breadcrumb before every scope, ground decal, grass band, missile and model draw (models drawn one draw slot at a time), so the error log lists which draws the GPU reached and finished. Costs some frame time; `0` keeps only the scope edges. Other GPUs have no breadcrumbs. |

`scripts/perf-suite.sh OUT_DIR [CASE...]` runs native macOS/Linux or Windows via WSL.
`PERF_SIZE`, `PERF_FOLLOW`, and whitespace-separated `PERF_ENV` assignments set the
capture size, followed ticks and overrides. Native `PERF_EXE` selects an existing
binary; otherwise the suite uses `play.sh`. Commands and shell expansions are not
supported inside `PERF_ENV`.

## Climate and weather

For shots and tests; a match takes both from the map's `maps/<stem>.ron` and skirmish set-up
(docs/ARCHITECTURE.md, "Sky, light and weather").

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_CLIMATE` | `temperate`, `tropical`, `desert` | Draws the whole map in that climate, whatever its file says. A map with a climate divide loses it: one climate, one weather, no light along the wall's foot. |
| `MERIDIAN_WEATHER` | `clear`, `fair`, `cloudy`, `stormy`, `overcast`, `storm` | The weather preset a renderer starts in (`storm` also parks a storm over the middle). A headless shot of a map with a climate divide shows the map's two weathers unless this is set, which plays the one preset over both sides. |

## Interface shots

Headless screenshots (`--screenshot`, `scripts/shot.sh run`) can show an order being aimed.
`--report PAGE[@M:SS]` draws the battle report over the match instead (overview, economy,
military, battlefield, timeline; the battlefield replay stopped at M:SS). It reports on
the `--ticks` played: an `--observe` match run long enough to be decided (dev16, two AIs:
about 45 minutes) has its verdict. `scripts/shot.sh run` stops a shot at 3 minutes, so a
match that long is run with the built exe directly.

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_AIM` | `1`, `ground`, `reclaim`, `warp`, `formation:DEG[:SHAPE]` | The order in hand, aimed at `--cursor`: a warhead launch (`1`, docs/NUKES.md), a titan's strike, Reclaim, a warp jump (the line from each selected ship to its exit in formation, and the energy card), or a move held on the right button (the selection's formation at `--cursor`, turned to face DEG degrees, shown as a hologram of each unit; `formation` alone faces the way it goes; SHAPE is wheel notches wider, negative longer, -6 to 6). |
| `MERIDIAN_ISSUE_NOTE` | any text | The F1 report card's note, as if typed (the field wraps and grows with a long one). |
| `MERIDIAN_GROUPS` | `KEY,KEY,...` | Control groups 2, 3, ... hold player 0's units whose blueprint key contains each KEY (group 1 is the selection): the groups card and the numbers by the units. |
| `MERIDIAN_HISTORY_REPORT` | `N[:PAGE]`, 1 = the newest | With `--ui history`: match N's battle report open over Match History on PAGE (`overview`, `economy`, `military`, `battlefield`, `timeline`), fully drawn in. A replay without a kept record (`replays/<id>.mcreport`) is played through first, which the shot waits for. |
| `MERIDIAN_VISION` | player slot | An `--observe` shot is drawn through that player's eyes, as if its vision chip were picked (and shows its AI Mind card when a Commander plays it, docs/AI_COMMANDER.md). |

## AI probes and tournaments

Read by the ignored probe tests in mc-sim, never by the game (docs/AI_COMMANDER.md).

| Variable | Values | Purpose |
|---|---|---|
| `TOURNEY` | `map:players:minutes:seed:A:B[:difficulty]` | One tournament match (`zz_ai_tournament`); A and B are `brain/doctrine`. |
| `TOURNEY_EVERY` | minutes | Print each side's economy, build speed, mines, worth and plans every N minutes. |
| `TOURNEY_ROSTER` | any | Print each player's units and where its mass stands at the end. |
| `TOURNEY_DEATHS` | any | Print every unit that dies: when, how far from home, the nearest enemy. |
| `TOURNEY_OPS` | player slot | Print that Commander's operations whenever one changes. |
| `TOURNEY_ARMY` | `key*n,key*n` | Give every side the same army at its start (a mirror of fighting alone). |
| `TOURNEY_SNAP` | tick | Print each side's armed mobile units at that tick. |
| `TOURNEY_MAPS`, `TOURNEY_SEEDS`, `TOURNEY_SIDES`, `TOURNEY_MINUTES`, `TOURNEY_DIFF`, `TOURNEY_JOBS` | see `scripts/ai-tournament.sh` | What the tournament script plays. |
| `MATCHUP` | `SHARD/SHARDS` | The share of unit pairs `zz_matchup_probe` stages. |
| `MATCHUP_ONLY` | unit key | Only that unit's pairs. |

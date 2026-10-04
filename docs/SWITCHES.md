# Rendering performance switches

Interactive play takes its graphics settings from Settings → Display (Auto, Low,
Medium, High, Ultra; `crates/mc-game/src/settings/quality.rs`). The scenery overrides
below win over the preset everywhere, in play and in headless captures, so an A/B
measurement holds whatever the settings say; render scale and AA override only
headless captures.

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_SIMPLE_SHADING` | `1` on, `0` off | Single-patch terrain textures, hardware shadow filtering and staggered cloud-shadow updates. Low and Medium use it. |
| `MERIDIAN_RENDER_SCALE` | `0.5`–`2.0` | Scene resolution relative to output; UI stays at output resolution. |
| `MERIDIAN_AA` | `off`, `smaa` | Edge smoothing. |
| `MERIDIAN_PROP_DETAIL` | `minimum radius,LOD bias,shadow radius` | Scenery detail; Low is `6,4,8`, Medium `3,3,5`, High `1.2,2,0`, Ultra `1.2,1,0`. |
| `MERIDIAN_CLOUD_RES` | `1`–`4` | Divide output dimensions by this for the cloud march. |
| `MERIDIAN_SHADOW_SIZE` | `512`–`4096`, a power of two | Texels along each sun shadow cascade; Low 1024, Medium/High 2048, Ultra 4096. |
| `MERIDIAN_SHADOW_DISTANCE` | metres, `1000`–`30000` | Camera distance at which sun shadows have faded out and the shadow pass stops; Low 4000, Medium 6000, High 8000, Ultra 12000. |
| `MERIDIAN_GTAO` | `0` off, `1` on | Ground-truth ambient occlusion; High and Ultra. |
| `MERIDIAN_GRASS` | `0` off | No grass at all. |
| `MERIDIAN_GRASS_DENSITY` | `0`–`1` | Fraction of the grass field grown; Low 0, Medium 0.5, High/Ultra 1. |
| `MERIDIAN_WATER_REFLECTIONS` | `0` off, `1` on | Screen-space reflections on water; off on Low. |
| `MERIDIAN_GPU_TIMERS` | `0` to disable | A/B profiling overhead. GPU timing values are unavailable when off; compare `cpu.render` instead. |
| `MERIDIAN_GPU_CRUMBS` | `0` to disable, `1` to enable | Finding a lost device (`VK_AMD_buffer_marker`), on by default on AMD GPUs: a breadcrumb before every scope, ground decal, grass band, missile and model draw (models drawn one draw slot at a time), so the error log lists which draws the GPU reached and finished. Costs frame time (2 ms GPU and 4 ms CPU a frame on a large map); `0` keeps only the scope edges. Other GPUs whose driver has the extension (NVIDIA) mark only the scope edges unless `1`; GPUs without it have no breadcrumbs. |

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
| `MERIDIAN_CLIMATE` | `temperate`, `tropical`, `desert` | Draws the whole map in that climate, whatever its file says. A map with regions loses them: one climate, region 0's weather over all of it, no light along the walls' foot. |
| `MERIDIAN_WEATHER` | `clear`, `fair`, `cloudy`, `stormy`, `overcast`, `storm` | The weather preset a renderer starts in (`storm` also parks a storm over the middle). A headless shot of a map with regions shows each region in its own weather unless this is set, which plays the one preset in every region. |
| `MERIDIAN_HOUR` | `0`–`24` | The time of day a renderer starts at (24-hour clock; the sun's height and colour follow it). Headless shots keep it; a match in a window then takes its map's hour. |

## Interface shots

Headless screenshots (`--screenshot`, `scripts/shot.sh run`) can show an order being aimed.
`--report PAGE[@M:SS]` draws the battle report over the match instead (overview, economy,
military, battlefield, timeline; the battlefield replay stopped at M:SS). It reports on
the `--ticks` played: an `--observe` match run long enough to be decided (crosswater, two AIs:
about 45 minutes) has its verdict. `scripts/shot.sh run` stops a shot at 3 minutes, so a
match that long is run with the built exe directly.

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_AIM` | `1`, `ground`, `reclaim`, `warp`, `formation:DEG[:SHAPE]` | The order in hand, aimed at `--cursor`: a warhead launch (`1`, docs/NUKES.md), a titan's strike, Reclaim, a warp jump (the line from each selected ship to its exit in formation, and the energy card), or a move held on the right button (the selection's formation at `--cursor`, turned to face DEG degrees, shown as a hologram of each unit; `formation` alone faces the way it goes; SHAPE is wheel notches wider, negative longer, -6 to 6). |
| `MERIDIAN_ISSUE_NOTE` | any text | The F1 report card's note, as if typed (the field wraps and grows with a long one). |
| `MERIDIAN_GROUPS` | `KEY,KEY,...` | Control groups 2, 3, ... hold player 0's units whose blueprint key contains each KEY (group 1 is the selection): the groups card and the numbers by the units. |
| `MERIDIAN_HISTORY_REPORT` | `N[:PAGE]`, 1 = the newest | With `--ui history`: match N's battle report open over Match History on PAGE (`overview`, `economy`, `military`, `battlefield`, `timeline`), fully drawn in. A replay without a kept record (`replays/<id>.mcreport`) is played through first, which the shot waits for. |
| `MERIDIAN_VISION` | player slot | An `--observe` shot is drawn through that player's eyes, as if its vision chip were picked (and shows its AI Mind card when a Commander plays it, docs/AI_COMMANDER.md). |

## Crash reports

Whatever ends the game badly is written beside the settings file (`%APPDATA%\meridian-conflict`
on Windows, `~/.config/meridian-conflict` elsewhere; `crates/mc-game/src/crash.rs`): a panic or
native fault to `crash-<unix secs>.log` (a native fault also to `crash-<secs>.dmp`, a minidump
for Visual Studio or WinDbg with the build's `.pdb`), an error to `error-<secs>.log`. A
player's run (no arguments, or one that opens the window) also shows the crash screen, and
writes its whole log to `meridian.log` (the run before it: `meridian-previous.log`). The crash
screen is the game's own interface in a process of its own (the game started again with
`--crash-screen`; `crates/mc-game/src/crash/screen.rs`): what happened, the report, and Copy
details, Open folder, Restart and Quit. When it cannot come up, a system task dialog with Copy
details and Open folder stands in. Tool runs (headless shots, the shot server, bots) write the
reports and never wait on a window.

| Switch | Values | Purpose |
|---|---|---|
| `--crash-screen FILE` | a report's path, then `--crash-title T`, `--crash-message M`, `--crash-hint H` | The crash screen for a saved report, as a crashed run starts it. With `--screenshot OUT.png` it is drawn headless (at `--size`, `--cursor` hovering), and `OUT-copied.png` just after Copy details. |
| `--crash-test` | `panic`, `sim`, `native`, `error` | Fail on purpose 8 s after start, to check the crash window over the running game: a panic on the main thread, a panic on a thread the game needs (as the sim thread is), an access violation (Windows), an error that ends the game. |

## Builds and channels

See `docs/RELEASES.md`.

| Switch | Values | Purpose |
|---|---|---|
| `MERIDIAN_CHANNEL` (compile time) | `dev` (default), `playtest`, `release` | Who the build is for. Part of the build's name (`0.1.0-playtest+<commit>`; a release is `0.1.0+<commit>`), which replays record and network players must share. |
| `--version` | | Print the build's name, number, channel, commit and simulation fingerprint, one `key: value` a line, and exit. |

## AI probes and tournaments

Read by the ignored probe tests in mc-sim, never by the game (docs/AI_COMMANDER.md).

| Variable | Values | Purpose |
|---|---|---|
| `TOURNEY` | `map:players:minutes:seed:A:B[:difficulty]` | One tournament match (`zz_ai_tournament`); A and B are doctrines (`adaptive`, `aggressive`, `economic`, `defensive`). |
| `TOURNEY_EVERY` | minutes | Print each side's economy, build speed, mines, worth and plans every N minutes. |
| `TOURNEY_ROSTER` | any | Print each player's units and where its mass stands at the end. |
| `TOURNEY_DEATHS` | any | Print every unit that dies: when, how far from home, the nearest enemy. |
| `TOURNEY_OPS` | player slot | Print that Commander's operations whenever one changes. |
| `TOURNEY_ARMY` | `key*n,key*n` | Give every side the same army at its start (a mirror of fighting alone). |
| `TOURNEY_SNAP` | tick | Print each side's armed mobile units at that tick. |
| `TOURNEY_MAPS`, `TOURNEY_SEEDS`, `TOURNEY_SIDES`, `TOURNEY_MINUTES`, `TOURNEY_DIFF`, `TOURNEY_JOBS` | see `scripts/ai-tournament.sh` | What the tournament script plays. |
| `MATCHUP` | `SHARD/SHARDS` | The share of unit pairs `zz_matchup_probe` stages. |
| `MATCHUP_ONLY` | unit key | Only that unit's pairs. |

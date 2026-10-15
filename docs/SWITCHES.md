# Rendering performance switches

Interactive play takes its graphics settings from Settings → Display (Auto, Low,
Medium, High, Ultra; `crates/mc-game/src/settings/quality.rs`). The scenery overrides
below win over the preset everywhere, in play and in headless captures, so an A/B
measurement holds whatever the settings say; render scale and AA override only
headless captures.

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_SIMPLE_SHADING` | `1` on, `0` off | Single-patch terrain textures and hardware shadow filtering. Low and Medium use it. |
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
| `MERIDIAN_FOLLOW_FRAMES` | `2`–`16` | Headless: frames each followed tick is drawn in (default 2), so a `--perf` capture shows a tick's work spread over the frames after it as play does. |
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
| `MERIDIAN_CLIMATE` | `temperate`, `tropical`, `desert`, `polar` (also `antarctic`, `arctic`), `autumn` (also `fall`) | Draws the whole map in that climate, whatever its file says. A map with regions loses them: one climate, region 0's weather over all of it, no light along the walls' foot. |
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
| `MERIDIAN_AIM` | `1`, `ground`, `reclaim`, `warp`, `dash`, `fracture`, `edict`, `formation:DEG[:SHAPE]` | The order in hand, aimed at `--cursor`: a warhead launch (`1`, docs/NUKES.md), a titan's strike, Reclaim, a warp jump (the line from each selected ship to its exit in formation, and the energy card), a dash by hand (the Seraph's wave where it would come down, `dash_marks.rs`), a gravitic engine's rift (the path from each picked engine and the well at the point, `gravitic_marks.rs`), a ship's Edict (its reach round the hull, the line from the firing point and the blast and wave rings at the point, `edict_marks.rs`), or a move held on the right button (the selection's formation at `--cursor`, turned to face DEG degrees, shown as a hologram of each unit; `formation` alone faces the way it goes; SHAPE is wheel notches wider, negative longer, -6 to 6). |
| `MERIDIAN_ISSUE_NOTE` | any text | The F1 report card's note, as if typed (the field wraps and grows with a long one). |
| `MERIDIAN_GROUPS` | `KEY,KEY,...` | Control groups 2, 3, ... hold player 0's units whose blueprint key contains each KEY (group 1 is the selection): the groups card and the numbers by the units. |
| `MERIDIAN_HISTORY_REPORT` | `N[:PAGE]`, 1 = the newest | With `--ui history`: match N's battle report open over Match History on PAGE (`overview`, `economy`, `military`, `battlefield`, `timeline`), fully drawn in. A replay without a kept record (`replays/<id>.mcreport`) is played through first, which the shot waits for. |
| `MERIDIAN_VISION` | player slot | An `--observe` shot is drawn through that player's eyes, as if its vision chip were picked (and shows its AI Mind card when a Commander plays it, docs/AI_COMMANDER.md). |
| `MERIDIAN_MIND_MAP` | `mind` or `truth` | With `MERIDIAN_VISION` on an `--observe` shot: the AI Mind card's map switch is set so, and the watched Commander's zones are drawn on the ground (`mind_map.rs`, docs/AI_COMMANDER.md). |

| Switch | Values | Purpose |
|---|---|---|
| `--prefabs FILE` | a `prefabs.ron` | The player's prefabs read from FILE instead of beside the settings, the selected builder's construction panel turned to prefabs (`hud/prefabs`). |
| `--place @NAME` | a prefab's name | With `--prefabs`: that prefab in hand at `--cursor`, its holograms and the card by the pointer (`--place KEY` places one structure). |
| `--place-turn Q[m]` | `0`..`3`, then `m` to mirror | With `--place`: what is in hand turned Q quarter turns anticlockwise (as R turns it, clockwise, in the game), and with `m` (a prefab) mirrored first, as F does. |
| `--save-prefab` | | The Save Prefab box open on the selection: its structures, and with `--plans` the commander's planned ones. |

## Films

Headless camera moves over a match, for trailers and the website's background
(`crates/mc-game/src/film.rs`). `scripts/film.sh REEL` films each cut of a reel
(`scripts/film/hero.reel`) and cuts them into one WebM, MP4 and poster frame;
`scripts/film.sh --help`.

| Switch | Values | Purpose |
|---|---|---|
| `--film DIR` | a folder | With `--screenshot`: after `--ticks`, film a camera move into DIR as numbered PNGs, no interface; the screenshot is the last frame. The camera aims at the middle of the mobile units, followed smoothly, or at `--camera X,Y`. |
| `--film-secs S` | seconds (default 8) | The clip's length. |
| `--film-fps N` | frames a second (default 30) | |
| `--film-speed X` | match seconds per film second (default 1) | 0.5 is half-speed slow motion. |
| `--film-from RIG`, `--film-to RIG` | `YAW,PITCH,DIST[,FOV[,LIFT[,SIDE]]]` | Where the camera starts and ends, eased between: the bearing it looks along and degrees below the horizon, metres from the aim point, lens degrees (default 34), the aim point's height above the ground (default 4) and offset to the camera's right (0). |
| `--film-on KEY` | part of a blueprint key | Aim at the units whose key contains KEY (`t5_titan`) instead of them all. |

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
| `--crash-test` | `panic`, `sim`, `native`, `error` | Fail on purpose 8 s after start, to check the crash window over the running game: a panic on the main thread, a panic on a thread the game needs (as the sim thread is), an access violation (Windows), an error that ends the game (raised as a failed Vulkan call is, so its report lists it under "errors raised" with its stack). |

## Survival

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_SURVIVAL` | `rounds:grace:interval:intensity:fronts:tier:nodes` | The rules a `--scene survival` run plays (rounds 0 is endless; seconds; intensity in thousandths; fronts as land 1 + air 2 + naval 4; tech ceiling; nodes 0-3). |
| `MERIDIAN_SURVIVAL_SPAWN` | defender spawn index | Which of the map's spawns the defender takes. |
| `MERIDIAN_SURVIVAL_PLAY_ON` | `1` | Debug: the engine plays on after the defenders fall, its rounds coming (printing nothing more) and its Shapers growing, so a headless `--observe` run can shoot grown Shapers (`SurvivalRules::play_on`). |
| `MERIDIAN_SURVIVAL_WHY` | `1`, or `pos` | Headless runs list where the hostiles are each minute; `pos` prints every hostile's position, motion, printing and order. |
## Builds and channels

See `docs/RELEASES.md`.

| Switch | Values | Purpose |
|---|---|---|
| `MERIDIAN_CHANNEL` (compile time) | `dev` (default), `playtest`, `release` | Who the build is for. Part of the build's name (`0.1.0-playtest+<commit>`; a release is `0.1.0+<commit>`), which replays record and network players must share. A release build has no unit or map marked `playtest: true`. |
| `MERIDIAN_STORE` (compile time) | an `https://` URL | The build store the launcher updates from and fetches old builds from (`publish/config.sh`'s `STORE_URL`, set by `scripts/release.sh`). Unset: no updates, and Match History names the build an old replay needs instead of fetching it. |
| `MERIDIAN_LAUNCHER` | the launcher's path | Set by the launcher for the game it starts: Match History's Watch in Its Build asks it to fetch and run an old replay's build. |
| `MeridianConflict --replay FILE` | a replay | The launcher plays FILE in the build that recorded it (fetching it first), then starts the current build on Match History. |
| `MeridianConflict --uninstall` | | Removes the install: the shortcuts, the Settings > Apps entry, the builds and the launcher; the replays stay. Settings > Apps runs it. |
| `MeridianConflictSetup*.exe` | the launcher under that name | Installs itself to `%LOCALAPPDATA%\Programs\Meridian Conflict` with shortcuts and an Apps entry, then starts the installed launcher (the website's download). |
| `MeridianConflict --screenshot DIR` | a folder | The launcher draws its window in each of its states into DIR as PNGs (1x and 1.5x) and exits. |
| `MERIDIAN_TEST_JOBS` | a number (default 6) | How many test binaries `scripts/check.sh` runs at once. |
| `MERIDIAN_TEST_LIMIT` | seconds (default 300) | How long `scripts/check.sh` lets one test binary run before it fails the check as hung. |
| `MERIDIAN_RELEASE_STORE` | a folder | Where `scripts/release.sh` keeps its copy of the build store (default `~/.local/share/meridian-release/store`). |
| `--replay-only` | with `--replay` | An old build playing a replay for a newer one: leaving the replay quits, and the settings file is never written. |
| `--open SCREEN` | `menu`, `skirmish`, `survival`, `multiplayer`, `history`, `settings` | The front end opens on that screen. |
| `--version` | | Print the build's name, number, channel, commit and simulation fingerprint, one `key: value` a line, and exit. |
| `MERIDIAN_SERVER_PLAYTEST` (compile time) | `host[:port]` | The server a playtest build plays on; players cannot change it. Unset: none (the multiplayer screen opens on the local network). |
| `MERIDIAN_SERVER_RELEASE` (compile time) | `host[:port]` | The same for a release build. A dev build reads neither and has no server of its own. |
| `MERIDIAN_MP_SERVER` | `host[:port]` | The game plays on that server instead of the one it was built with (a dev build has none). Also for `--ui` shots of the multiplayer screen. |

## AI probes and tournaments

Read by the ignored probe tests in mc-sim, never by the game (docs/AI_COMMANDER.md).

| Variable | Values | Purpose |
|---|---|---|
| `TOURNEY` | `map:players:minutes:seed:A:B[:difficulty]` | One tournament match (`zz_ai_tournament`); A and B are doctrines (`adaptive`, `aggressive`, `economic`, `defensive`). |
| `TOURNEY_INCOME` | thousandths | Every seat's share of normal income (`AiConfig::income`, as the set-up's AI income): the sim earns more, unlike `TOURNEY_RESOURCES`. Default 1000. |
| `TOURNEY_EVERY` | minutes | Print each side's economy, build speed, mines, worth and plans every N minutes. |
| `TOURNEY_ROSTER` | any | Print each player's units and where its materials stand at the end. |
| `TOURNEY_DEATHS` | any | Print every unit that dies: when, how far from home, the nearest enemy. |
| `TOURNEY_OPS` | player slot | Print that Commander's operations whenever one changes. |
| `TOURNEY_FACTIONS` | `A,B` | Each team's faction (`Aster`, `Regency`); Aster by default. |
| `TOURNEY_FORCES_B` | `land,air,naval` | Team B's force weights (0..=200, 100 even), e.g. `40,200,100` for an air-heavy side. |
| `TOURNEY_ARMY` | `key*n,key*n` | Give every side the same army at its start (a mirror of fighting alone). |
| `TOURNEY_RESOURCES` | whole number | Multiply every player's materials and energy income (the harness tops the stores up; default 1). |
| `TOURNEY_SNAP` | tick | Print each side's armed mobile units at that tick. |
| `TOURNEY_MAPS`, `TOURNEY_SEEDS`, `TOURNEY_SIDES`, `TOURNEY_MINUTES`, `TOURNEY_DIFF`, `TOURNEY_JOBS` | see `scripts/ai-tournament.sh` | What the tournament script plays. |
| `MATCHUP` | `SHARD/SHARDS` | The share of unit pairs `zz_matchup_probe` stages. |
| `MATCHUP_ONLY` | unit key | Only that unit's pairs. |

## Map editor

| Variable | Values | Purpose |
|---|---|---|
| `MERIDIAN_DEV` | `1` | A developer's run (`play.sh` sets it): the main menu also offers The Contract, Continue Campaign and Map Editor. Without it a player sees neither the campaign nor the editor (`ui/menu.rs`). |

- --editor MAP: open a map name or .mcmap path in the editor.
- The editor mailbox lives in the settings directory under editor/<process-id>.
  Requests are bounded to 256 KiB; each edit batch is one undo step.

--editor-apply FILE applies a JSON action array to the --editor map folder through the editor command layer, saves, and exits unless --screenshot is also given.

- `--mission-pilot`: with `--bench` or `--screenshot` on Vela, execute the
  authoring smoke route using ordinary Company orders. No teleports or forced
  triggers; does not control normal interactive play.

- `--editor-headless`: with `--editor MAP`, serve the editor mailbox and render
  the real editor offscreen at ten frames per second. Use `op=close` to stop.

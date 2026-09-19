# Meridian Conflict

A Supreme Commander: Forged Alliance-style RTS on a custom engine: Rust, raw Vulkan, one
continuous zoom from a tank's tracks to an 80 km x 80 km map. The requirements are in
[docs/SPEC.md](docs/SPEC.md); how the code meets them is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Run it

Maps are baked files and are not checked in. Bake them once:

```bash
cargo run --release -p mc-map --bin mc-bake -- --size-km 16 --seed 7 --name "Dev Basin 16" -o maps/dev16.mcmap
cargo run --release -p mc-map --bin mc-bake -- --size-km 80 --seed 7 --name "Meridian Basin" -o maps/meridian_basin.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout islands --size-km 10 --seed 46 --name "Twin Shoals" -o maps/twin_shoals.mcmap
```

Then, from the repository root (the game looks for `data/` and `maps/` there):

```bash
cargo run --release -p mc-game                                   # the front end: main menu, skirmish set-up, settings
cargo run --release -p mc-game -- --map dev16                    # straight into a skirmish against the AI
cargo run --release -p mc-game -- --map meridian_basin --players 8   # 8-way on the 80 km map
cargo run --release -p mc-game -- --scene battle                 # two pre-built armies (test scene)
cargo run --release -p mc-game -- --scene stress --map meridian_basin --players 8 --army 500   # full load
cargo run --release -p mc-game -- --scene showcase               # one of every unit
```

With no match options the game opens its front end. Its backdrop is the game itself: a scripted
battle on Twin Shoals (or the smallest map there is) running in the real simulation, filmed by a
few camera moves. Skirmish set-up lists every map in `maps/` with a chart of it; click a landing
zone on the chart to start there. Any of `--map`, `--scene`, `--players`, `--seed` or `--connect`
skips the front end, as before. Settings live in `%APPDATA%\meridian-conflict\settings.ron`
(`~/.config/meridian-conflict/` elsewhere).

Windows is the primary target: run the same commands from a Windows shell. No Vulkan SDK is
needed; shaders are WGSL compiled to SPIR-V at build time by naga. Under WSL the game runs on
the CPU rasteriser (lavapipe), which is fine for tests and screenshots but not for playing.
Sound needs a system audio API: Windows and macOS builds have it; on Linux build with
`--features alsa` (needs ALSA's headers), otherwise the game runs silent.

Multiplayer goes through the relay. The first player to join hosts, and their `--map`,
`--players` (total slots; empty ones become AI) and `--seed` define the match:

```bash
cargo run --release -p mc-net --bin mc-relay -- --bind 0.0.0.0:7777 --players 2 --auto-start
cargo run --release -p mc-game -- --connect HOST:7777 --name Alice --players 4
cargo run --release -p mc-game -- --connect HOST:7777 --name Bob
```

Headless tools use the same runtime:

```bash
cargo run --release -p mc-game -- --scene stress --map meridian_basin --players 8 --army 500 --bench 1500
cargo run --release -p mc-game -- --scene battle --ticks 150 --screenshot shot.png --camera 8192,8192,420,35
cargo run --release -p mc-game -- --ui skirmish --screenshot ui.png      # a front-end screen: menu | skirmish | settings
cargo run --release -p mc-game -- --smoke              # unattended: front end, a short skirmish, back, exit
cargo run --release -p mc-game -- --dump-sounds sounds/   # the synthesised sound set as WAV files
```

## Controls

| input | action |
|---|---|
| left click / drag | select unit / box select (shift adds) |
| right click | move; attack an enemy; assist or repair a friend; reclaim a wreck (engineers); set a factory's rally point |
| shift + order | queue |
| build menu (bottom) | place a structure (left click; shift keeps placing; right click cancels) or queue a unit in a factory (shift: five) |
| F, then left click | attack-move |
| X / U | stop / upgrade the selected structure |
| Ctrl+Delete | self-destruct |
| Ctrl+0-9 / 0-9 | set / recall a control group |
| Home | jump to your commander |
| wheel, WASD or arrows, Q/E, PgUp/PgDn, middle drag | zoom to cursor, pan, rotate, tilt, pan |
| F1 | profiler: every sim phase and GPU pass, table sizes |
| Esc (nothing selected) / F10 | the command menu: resume, volume, leave the match, exit. A single-player match pauses while it is open |

## Layout

| path | what |
|---|---|
| `crates/mc-core` | fixed point, integer trig, RNG, state hashing |
| `crates/mc-jobs` | worker pool, job graph, deterministic parallel-for, background tasks |
| `crates/mc-map` | `.mcmap` format, tile streaming, sim heightfield, `mc-bake` (basin and islands layouts) |
| `crates/mc-path` | hierarchical flow fields with deterministic background builds |
| `crates/mc-sim` | the simulation: state tables, spatial index, commands, economy, combat, AI, snapshots |
| `crates/mc-data` | blueprint loader; `data/factions/aster/` is the Aster faction |
| `crates/mc-net` | lockstep protocol, relay (`mc-relay`), sessions, replays |
| `crates/mc-render` | Vulkan renderer, WGSL shaders, procedural models and textures, the 2D overlay and its type |
| `crates/mc-game` | the `meridian` binary: front end (`ui/`), the match (`game.rs`), sound (`audio.rs`), tools |

## Status

Playable: a land war with the Aster faction (commander, three engineer tiers, nine combat units,
sixteen structures across three tech tiers), flow economy, construction and assisting, factories
with queues and rally points, in-place upgrades, reclaimable wreckage, persistent scorch marks,
fog of war and radar, strategic icons, a skirmish AI, and LAN/internet multiplayer through the
relay with per-tick desync detection. A front end (main menu, skirmish set-up, settings) with a
synthesised interface sound set and ambience. Three maps: the 80 km Meridian Basin, a 16 km dev
basin, and Twin Shoals, a 10 km 1v1 island map (both commanders on the main island around a
central lake, a ridge in each passage, two town islands reachable by amphibious units and hovers). Local matches are recorded to `last-match.mcreplay`; the
relay records with `--replay-dir`.

Implemented and tested at the crate level, but not yet exercised end to end from the game
binary: late join and reconnect (the relay's snapshot hand-off in `mc-net`, exact state
snapshots including in-flight flow fields in `mc-sim`/`mc-path`).

Measured on the 80 km map with 8 players and ~4,000 units in combat: simulation 4 ms per tick on
average (budget 25 ms); GPU frame about 1 ms at 2560x1440 on an RTX 3080 Ti with 260,000 props.
In an unpaced `--bench` run the first cross-map flow field can take longer than the two ticks it
is given, because ticks run back to back instead of 100 ms apart; at game speed it has 200 ms.

Not built yet: air, naval and experimental units; the Naga faction (the data format and engine
are faction-agnostic); battle sound (the interface has sound, weapons and explosions do not);
formations beyond block move orders; replay playback from the game
binary (`mc-net` has `ReplaySession`; the binary only records); observers in the UI; a
multiplayer lobby UI (network matches are still set up from the command line); patrol and guard orders; tree reclaim; authored art
(all models and textures are procedural).

# Meridian Conflict

A Supreme Commander: Forged Alliance-style RTS on a custom engine: Rust, raw Vulkan, one
continuous zoom from a tank's tracks to an 80 km x 80 km map. The requirements are in
[docs/SPEC.md](docs/SPEC.md); how the code meets them is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Run it

`./play.sh` detects the host: macOS and Linux build and run natively; WSL builds
with Windows Cargo and launches on the Windows GPU. `./play.sh --build-only`
builds without launching. All other arguments pass through to the game.

On macOS, install [Homebrew](https://brew.sh), Xcode command-line tools
(`xcode-select --install` if needed), and the runtime dependencies once:

```bash
brew install rustup vulkan-loader molten-vk spirv-tools
export PATH="$(brew --prefix rustup)/bin:$PATH"  # for direct cargo commands below
```

The repository pins Rust; rustup downloads that toolchain on the first build.
Mac builds run `spirv-opt` from `spirv-tools` after Naga to inline and simplify
shader array copies before MoltenVK translates them to Metal.
The launcher finds Homebrew's Rust and Vulkan libraries automatically on Apple
Silicon and Intel Macs. Rendering uses [MoltenVK](https://github.com/KhronosGroup/MoltenVK)
over Metal, including the headless screenshot tools. This is a development and
asset-capture path; performance parity with Windows is not assumed. Existing
`VK_DRIVER_FILES`, `VK_ICD_FILENAMES` and library-path overrides are preserved
for developers using their own Vulkan SDK.

**Settings → Display → Quality** groups render scale, anti-aliasing, scenery
detail and cloud resolution into presets. Changes apply immediately in the menu
and in a match, and are saved for the next launch.

| Preset | Render scale | Anti-aliasing | Scenery | Clouds | Shading |
|---|---|---|---|---|---|
| Low | 50% | Off | Least detail | Quarter resolution | Simple |
| Balanced | 75% | SMAA | Reduced distant detail | Quarter resolution | Simple |
| High | 100% | SMAA | Full detail | Third resolution | Full |
| Ultra | 150% | SMAA | Finer geometry | Half resolution | Full |

Simple shading keeps material colours, normal maps, weather and shadows, but uses
one terrain texture patch instead of three anti-tiling patches, hardware shadow
filtering instead of nine taps, and refreshes one quarter of the cloud-shadow
field per frame. Full shading retains the original detail. These paths work on
all supported GPUs; existing Low/Balanced settings gain the cheaper shading on
next launch without resetting preferences.

The performance suite also runs natively on macOS and Linux:

```bash
PERF_SIZE=2560x1600 PERF_FOLLOW=60 \
PERF_ENV="MERIDIAN_RENDER_SCALE=0.5 MERIDIAN_AA=off MERIDIAN_PROP_DETAIL=6,4,8 MERIDIAN_CLOUD_RES=4 MERIDIAN_SIMPLE_SHADING=1" \
scripts/perf-suite.sh artifacts/perf-mac battle_mid
```

It saves images, logs and CPU/GPU timing reports. `PERF_EXE=/absolute/path/to/meridian`
skips building and uses that binary; omit it to build through `play.sh`. Compare
runs at the same size, camera and settings with other GPU applications idle.
Headless captures take environment overrides, independently of saved settings.

New Mac settings default to Balanced; other platforms default to High. Existing
saved render scale and anti-aliasing choices are preserved. Adjusting either
manually displays **Custom**, retaining the base preset's scenery and clouds;
the first Quality arrow click restores that base preset. The interface stays at
native resolution at every quality level. Use Low or a custom 50% render scale
for additional relief on Retina displays.

The launcher no longer forces graphics settings through environment variables.
`MERIDIAN_PROP_DETAIL` and `MERIDIAN_CLOUD_RES` still work for headless match
captures; interactive play and front-end captures use the Settings presets.
The main menu runs a live 3D battle and shares its GPU cost. These adjustments
improve frame time but do not guarantee 60 FPS on Mac.

Maps are baked files and are not checked in. Bake them once:

```bash
cargo run --release -p mc-map --bin mc-bake -- --size-km 16 --seed 7 --name "Dev Basin 16" -o maps/dev16.mcmap
cargo run --release -p mc-map --bin mc-bake -- --size-km 80 --seed 7 --name "Meridian Basin" -o maps/meridian_basin.mcmap
cargo run --release -p mc-map --bin mc-bake -- --size-km 80 --seed 7 --players 32 --name "Meridian Crown" -o maps/meridian_crown.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout islands --size-km 10 --seed 46 --name "Twin Shoals" -o maps/twin_shoals.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout threshold --size-km 16 --seed 31 --name "The Threshold" -o maps/threshold.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout alpine --size-km 8 --seed 3 --name "Serac Divide" -o maps/serac_divide.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout alpine-teams --size-km 12 --seed 5 --name "Serac Sound" -o maps/serac_sound.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout archipelago --size-km 20 --seed 23 --players 8 --name "The Axis" -o maps/the_axis.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout twin-bays --size-km 16 --seed 7 --players 8 --name "Halden's Grip" -o maps/haldens_grip.mcmap
```

Every layout but the survival ones comes out with starting wreckage (`mc_map::wreckage`). To lay it on a map already
baked without baking the terrain again, give `--wreckage-only` with the layout and seed it was baked with, e.g.
`mc-bake --wreckage-only --layout alpine --seed 3 -o maps/serac_divide.mcmap`.

Then, from the repository root (the game looks for `data/` and `maps/` there):

```bash
./play.sh                              # main menu
./play.sh --map twin_shoals            # skirmish
mkdir -p artifacts
./play.sh --scene battle --map twin_shoals --ticks 150 --size 1920x1080 --screenshot artifacts/mac-battle.png
./play.sh --smoke                      # unattended window/menu/match/exit check
```

Direct Cargo commands also work when the Vulkan runtime is on your library path
(on macOS, prefer `./play.sh` for its runtime setup):

```bash
cargo run --release -p mc-game                                   # the front end: main menu, skirmish set-up, settings
cargo run --release -p mc-game -- --map dev16                    # straight into a skirmish against the AI
cargo run --release -p mc-game -- --map meridian_basin --players 8   # 8-way on the 80 km map
cargo run --release -p mc-game -- --map meridian_crown --players 32 --teams 8 --observe   # 32 AI commanders, eight teams of four
cargo run --release -p mc-game -- --scene battle                 # two pre-built armies (test scene)
cargo run --release -p mc-game -- --scene stress --map meridian_basin --players 8 --army 500   # full load
cargo run --release -p mc-game -- --scene showcase               # one of every unit
cargo run --release -p mc-game -- --range                        # the test range, a Warden on the pad
cargo run --release -p mc-game -- --range --unit aster_t1_artillery --scenario targets
cargo run --release -p mc-game -- --map twin_shoals --observe     # watch two AIs fight
```

With no match options the game opens its front end. Its backdrop is the game itself: a scripted
battle on Twin Shoals (or the smallest map there is) running in the real simulation, filmed by a
few camera moves. Skirmish set-up lists every map in `maps/` with a chart of it; click a landing
zone on the chart to start there. On skirmish set-up, set your control from YOU to OBSERVE to
watch the AIs fight. Any of `--map`, `--scene`, `--players`, `--seed`, `--observe` or
`--connect` skips the front end, as before. Settings live in
`%APPDATA%\meridian-conflict\settings.ron` (`~/.config/meridian-conflict/` elsewhere).

Windows is the primary target: run the Cargo commands from a Windows shell or
`./play.sh` from WSL. No Vulkan SDK is needed; shaders are WGSL compiled to SPIR-V
at build time by naga. Direct Linux Cargo runs under WSL may use the CPU rasteriser
(lavapipe); the launcher uses Windows instead.
Sound needs a system audio API: Windows and macOS builds have it; on Linux build with
`--features alsa` (needs ALSA's headers), otherwise the game runs silent.

**Skirmish** and **Survival** on the main menu open one set-up screen (the mode is
switched in its Match Settings), and **Open to Others** at its foot takes the match as
set up to a lobby friends join: over the internet or on your own network, public or
private by code. The lobby is the same screen with chat, ready and start; leaving it goes
back to the set-up. **Multiplayer** on the main menu finds games on a server or on your
own network, and its Host Game leads to the same set-up screen. Internet games go
through `meridian-server`: it lists open games, hosts rooms by code, checks names against
each player's device key and relays every match. `docs/SERVER.md` is the guide to running
one (on a small VPS, or at home on a Mac); `docs/MULTIPLAYER.md` explains the design. Games
on your own network need no server: the host's game runs the relay and announces itself.
Start a server locally with:

```bash
./server.sh
```

This builds the server as needed, listens on TCP port 7777, and keeps names and
replays in `meridian-data/`. On macOS it also keeps the Mac awake while running.
Stop with Ctrl+C. Use `./server.sh --help` for options, or see
[the server guide](docs/SERVER.md) to let players connect from outside your home.

A single match can also run on the plain relay, from the command line. The first player to join hosts, and their
`--map`, `--players` (total slots; empty ones become AI) and `--seed` define the match:

```bash
cargo run --release -p mc-net --bin mc-relay -- --bind 0.0.0.0:7777 --players 2 --auto-start
cargo run --release -p mc-game -- --connect HOST:7777 --name Alice --players 4
cargo run --release -p mc-game -- --connect HOST:7777 --name Bob
```

Headless tools use the same runtime:

```bash
cargo run --release -p mc-game -- --scene stress --map meridian_basin --players 8 --army 500 --bench 1500
cargo run --release -p mc-game -- --scene battle --ticks 150 --screenshot shot.png --camera 8192,8192,420,35
cargo run --release -p mc-game -- --map twin_shoals --ticks 3 --plans --screenshot plans.png --camera 7350,7340,480   # planned structures and order lines; --cursor X,Y --drag X,Y drags one
cargo run --release -p mc-game -- --ui skirmish --screenshot ui.png      # a front-end screen: menu | skirmish | settings
cargo run --release -p mc-render --example structure_shots -- maps/twin_shoals.mcmap shots wharf:aster_t3_naval_factory@water,0,0:140,0,0.9,0,-22   # structures on the map, no match: KEY@x,y,heading (x `land` or `water` finds a spot)
cargo run --release -p mc-game -- --smoke              # unattended: front end, a short skirmish, back, exit
cargo run --release -p mc-game -- --dump-sounds sounds/   # the synthesised sound set as WAV files
cargo run --release -p mc-game -- --dump-cursors cursors.png   # every mouse pointer, over dark, grass and bright ground
```

## Working on it

The rules every change follows, and how several sessions share this tree and commit
safely, are in [CLAUDE.md](CLAUDE.md). The gate a commit must pass:

```bash
scripts/check.sh            # rustfmt, clippy with warnings as errors, every test (release)
scripts/check.sh --head     # the same on the last commit alone, in its own worktree
scripts/commit.sh -m "message" path...   # commit exactly these paths, nothing else
```

Turn the pre-commit hook on once per clone with `git config core.hooksPath scripts/hooks`.
`scripts/determinism-cross.sh` compares the simulation's hash between the Windows and
Linux builds.

## Controls

| input | action |
|---|---|
| left click / drag | select unit / box select (shift adds); double-click a unit to take every one of its type on screen; a box takes combat units first, then builders, then structures |
| right click | move (hold the button a moment, or drag, to see the formation where it will stand, and drag to turn it to face the way you drag; it moves when you let go; a left click or Esc drops it); attack an enemy; assist or repair a friend; reclaim a wreck (engineers, reclaimer towers), or an enemy when nothing selected is armed; on a factory, the way out for what it makes (shift queues more) |
| the mouse pointer | says what a click would do. Over an enemy with anything armed selected: attack; over a wreck with builders: reclaim; over a friend with builders: assist; over one of yours otherwise: select. An armed order shows its own pointer, or the barred circle where it cannot apply, like a structure that does not fit. With shift over an order: pick it up. Open ground keeps the plain arrow: a right click there moves, as ever |
| shift + order | queue |
| hold shift | every order of your side shows on the map, and every planned structure as a ghost (the selection's show without shift). Drag a waypoint or a planned structure to move it: a shared waypoint moves for the whole group, a structure turns red where it cannot go. Right click or Esc puts it back |
| construction panel (bottom right) | T1/T2/T3 tabs; a tile places a structure (left click; shift keeps placing; right click cancels) or queues a unit in a factory (shift: five; right click takes one out). Hover a tile for its data card |
| queue strip (over the construction panel) | what the selected factory or engineer is working through; on a factory, click adds one, right click removes one. Batch, Pause and Repeat switches sit at its right end (on a narrow screen, as glyphs alone) |
| Shift+L / BATCH | toggle a factory's batch: what it makes forms up just outside it and waits until the batch is full, then the whole batch leaves together on the factory's orders, as one group, or for its rally point. Full means SIZE units ready (the -/+ stepper beside the switch, 10 to start; wheel steps it, Shift+wheel by 5), or every queue in it run dry. Several factories selected: Batch links them into one batch, drawn on the ground as blue brackets round each factory joined by blue lines; each forms up at its own door, and they share one set of orders and one rally point (an order to any of them is given to all), so the batch leaves on one order line. SEND lets the ones ready go now; turning batch off sends them too. The units waiting are a group: click their badge to select them; a unit you order away leaves the batch |
| order card: M / Attack / F / C / R, then left click | move / attack (no key: A pans; right-click an enemy does the same) / attack-move / assist / reclaim; right click or Esc cancels. Reclaim on open ground sends the reclaimers there, taking the wrecks along the way; pressed and dragged out, it clears every wreck in the circle (shift queues either). Reclaim takes a wreck, one of your own units or structures, or an enemy: a live unit loses health as it is taken apart and gives a fifth of its mass back, and what is reclaimed to nothing is gone with no blast and no wreck. An idle engineer mends wounded allies in reach (a quarter of the build cost) before it clears wrecks |
| P, then left click | patrol: the selection sets off at once, out to that point and back to where it stands (with shift, where its queue ends), walking the loop in formation and fighting what it meets. Keep shift held and click on to add posts to the loop; let go of shift to finish |
| group badge (a number on a waypoint) | a group ordered together walks as one: one line from its middle to its waypoint, and the waypoint wears how many are in it. Click the number to select that group again (shift adds it); order some away and they get a badge of their own |
| O, then left click | warp (the Courier, Bastion, Resolute and Dominion): each ship charges its drive off the grid and jumps as far as it reaches toward the point (shift queues). With the order in hand every ship shows its reach, the line to where it comes out and the energy the jumps take against your store; an exit inside a known enemy Undertow's field is marked DAMPENED. The card shows the charge, the transit, the drive's recharge and a stun's seconds |
| X / U / L | stop (a factory clears its queue and its orders) / upgrade the selection to its next tier (structures, the commander's engineering suites) / toggle a factory's repeat (Shift+L: its batch) |
| G / FORMATION | open formation controls: together/free movement, compact/standard/wide spacing, and form up here |
| minimap | left click or drag: look there (or aim the armed order); right click: order the selection there |
| selection panel | several units: one tile per type, click keeps only that type, shift-click drops it |
| idle engineers / idle factories chips | select them all and bring the camera |
| Pause | pause (single player) |
| + / - | game speed, 0.1x to 12x (single player) |
| Delete / Ctrl+Delete | self-destruct the selection after a 5 second countdown, shown as a draining red ring round each unit (Delete again calls it off) / at once |
| Ctrl+0-9 / 0-9 | set / recall a control group; twice quickly also brings the camera. Groups show as chips over the selection panel |
| Home | jump to your commander (or, watching, to a living one) |
| wheel, WASD or arrows, Q/E, PgUp/PgDn, middle drag | zoom to cursor, pan, rotate, tilt, pan |
| hold Alt, move the mouse | orbit around the unit under the pointer (or the tracked unit, or the ground); release to put the camera back |
| Ctrl+Alt | free camera for pictures and recordings: the interface folds away (Ctrl+Alt or Esc brings it back). Pressed during an Alt-orbit it keeps the angle and stays on the unit. Right-drag looks, WASD flies where you look, E/Q rise and sink, Shift faster, wheel dollies (right button + wheel: flight speed), Alt orbits the aim, Z/X lens, click locks on, F frames, T follows, Ctrl+1-9 saves a shot, 1-9 glides to it (Shift cuts), P plays the shots in order, L locks the camera, N smoothing, G thirds grid, B cinema bars, H keys |
| T | track the unit under the pointer, or the selection; pan to stop |
| F1 | profiler: every sim phase and GPU pass, table sizes |
| Esc (nothing selected) / F10 / MENU | the command menu: resume, settings, volume, leave the match, exit. A single-player match pauses while it is open |

## The test range

`--range`, or TEST RANGE in the main menu: a real match with cheats on, one unit (the *subject*)
on a pad, and a panel down the left edge that does things to it. Everything the panel does is a
`Debug*` command through the ordinary command stream, so what you see is what a match would show.
The rest of the HUD works as usual: select the subject and order it about, build with a factory
or an engineer (building is free on the range unless FREE BUILD is switched off).

| panel | what it does |
|---|---|
| SUBJECT | click the name to browse every unit and structure. Type a name or role, filter by category and tech, then click a card or press Enter. Arrow keys browse, the wheel or Previous / Next turns pages, and Escape cancels. A new subject gets a clean range; the small arrows still step adjacent units |
| SPAWN | click the name to pick what the next click on the ground places, using the same searchable browser without wiping the range. ALL / MOBILE / STRUCT walks that list faster. x N, BLUE / RED / DUMMY, SPAWN AT THE POINTER (G) put N of that type there, as yours, as a hostile, or as a dummy: hostile, holds its fire, cannot die. Shift keeps placing |
| DO TO: -10% / -25% / HEAL / KILL / REMOVE | applies to the selection, or with nothing selected to every unit of the subject's type on the side you command. KILL is a real death (explosion, wreck, scorch); REMOVE just takes it away |
| HOLD FIRE / CANNOT DIE | toggles on the same units |
| BUILT | drag to make the unit a construction site that far along; it stays there until an engineer is told to assist it, or the slider is put back to 100% |
| STAGE: UNDER FIRE | two hostiles spawn inside their weapon range and open fire on the pad. Switch CANNOT DIE on first to watch it take hits for as long as you like |
| STAGE: TARGETS | dummies at short, medium and long range of the subject's first weapon |
| STAGE: BUILD IT | whatever builds the subject (a factory, or an engineer for a structure) appears and builds it |
| RESET (F5) | clears units, wrecks, shots and scorch marks and puts the subject back on the pad |
| COMMANDING: BLUE / RED | which side your orders go to. Selecting a unit of the other side switches too, so red units can be told to attack like any of yours |
| CLOSE / MID / FAR (F2 / F3 / F4) | the three zoom levels everything has to read at |
| RELOAD DATA/ (F9) | reads `data/` again (stats, weapons, costs) and restarts the range on the same subject; a file that does not parse is reported and nothing changes |

Slow motion is the ordinary game speed control: `-` goes down to 0.1x.

## Layout

| path | what |
|---|---|
| `brand/` | collected brand assets, colors, writing guidance, and Discord setup |
| `crates/mc-core` | fixed point, integer trig, RNG, state hashing |
| `crates/mc-jobs` | worker pool, job graph, deterministic parallel-for, background tasks |
| `crates/mc-map` | `.mcmap` format, tile streaming, sim heightfield, `mc-bake` (basin, islands, alpine, alpine-teams, archipelago, twin-bays and threshold layouts) |
| `crates/mc-path` | hierarchical flow fields with deterministic background builds |
| `crates/mc-sim` | the simulation: state tables, spatial index, commands, economy, combat, AI, snapshots |
| `crates/mc-data` | blueprint loader; `data/factions/aster/` is the Aster faction |
| `crates/mc-net` | lockstep protocol, relay (`mc-relay`), sessions, replays, directory and identity, LAN discovery |
| `crates/mc-server` | `meridian-server`: the public game server (directory, rooms, names) |
| `crates/mc-render` | Vulkan renderer, WGSL shaders, procedural models and textures, the 2D overlay and its type |
| `crates/mc-game` | the `meridian` binary: front end (`ui/`), the match (`game.rs`) and its HUD (`hud/`), sound (`audio.rs`), tools |
| `site/` | the marketing site and unit directory (Next.js), built from `data/factions/`; see `site/README.md` |

## Status

Playable: a land war with the Aster faction (commander, three engineer tiers, nine combat units,
nineteen structures across three tech tiers), flow economy, construction and assisting, factories
with queues and standing orders (move, attack-move, patrol, attack, assist, guard, ground fire: every unit
made takes them, and a factory still going up takes both its queue and its orders), in-place upgrades, reclaim (wrecks, your own units, enemies; idle engineers
clear the wrecks within their reach while there is room for the mass, and so does the Scavenger reclaim
tower (tech 1 to 3) over a much wider one, though its head is slow to aim and has to charge before the beam comes on;
salvage vehicles, the Trawler boat and the Argus clear what they pass while they move;
nothing reclaims a live unit without an order, and a builder that carries weapons
leaves wrecks alone while an enemy is near; a builder raises a reclaim field over a lot before it lays a
structure down, and four waves running out from the middle vaporize the trees on it (3 s, for nothing), and walkers as big as a commander knock over the trees they walk through), persistent scorch marks,
fog of war and radar, strategic icons, a skirmish AI, and LAN/internet multiplayer through the
relay with per-tick desync detection. A match HUD in the front end's style (economy, clock,
game speed and pause, minimap, selection, order card, tech-tabbed construction with data cards,
build queues, order lines and planned-structure ghosts that can be dragged, idle-builder and control-group chips, alerts), and range rings on the
ground for whatever is selected or being placed (weapons by kind with their dead zones, radar, build range). A front end (main menu, skirmish set-up, settings) with a
synthesised interface sound set. Skirmish control can be OBSERVE: the AIs fight, the map opens
from orbit, pause and game speed still work, and a click on the roster jumps to that commander.
Three maps: the 80 km Meridian Basin, a 16 km dev
basin, and Twin Shoals, a 10 km 1v1 island map (both commanders on the main island around a
central lake, a ridge in each passage, two town islands reachable by amphibious units and hovers). Local matches are recorded to `replays/<id>.mcreplay` (the relay records with `--replay-dir`);
the main menu's **Replays** watches them with a timeline to scrub, and F1's **Mark Issue** flags a
moment with a note, the timings and a screenshot for later: see `docs/REPLAYS.md`.

Implemented and tested at the crate level, but not yet exercised end to end from the game
binary: late join and reconnect (the relay's snapshot hand-off in `mc-net`, exact state
snapshots including in-flight flow fields in `mc-sim`/`mc-path`).

Measured on the 80 km map with 8 players and ~4,000 units in combat: simulation 4 ms per tick on
average (budget 25 ms); GPU frame about 1 ms at 2560x1440 on an RTX 3080 Ti with 260,000 props.
In an unpaced `--bench` run the first cross-map flow field can take longer than the two ticks it
is given, because ticks run back to back instead of 100 ms apart; at game speed it has 200 ms.

Not built yet: the Regency's fighting units and their tech 2+ structures (they have their own
commander, engineer and tech 1 structures, and field ARC's units from their factories until then); battle sounds beyond the first library in `data/sounds` and `data/factions/aster/sounds.ron`
(unit files name their sounds; only the Warden's have been reviewed by ear, see `docs/STYLE.md`);
patrol and guard orders; reclaiming trees on an order; authored art
(all models and textures are procedural).

### Aircraft and formations

Select units and press **G**, or click **FORMATION** in the Orders card.
Choose **TOGETHER** / **FREE MOVE** and **COMPACT** / **STANDARD** / **WIDE** spacing
for subsequent orders. **FORM UP HERE** gathers them at their current location.
Selected orders show both moving slots and assigned destinations.

Group move/attack-move orders assign repeating five-aircraft Vs in each cruise
altitude band and rectangular ground blocks (including mixed land/amphibious units). Queued
legs face their direction of travel. Units move immediately and fall into ranks around a shared moving anchor at the group's pace, and completed orders preserve their pose.
Aircraft brake into a stationary hover, roll into turns, and pitch on climbs;
they do not lean with the terrain. Fighters lead moving targets and keep flying
through dogfights; bombers lead moving targets, commit past them before turning for another pass, and run along a map edge rather than at it.
Combat releases aircraft from formation; attack-move survivors resume their slots.

Cruise height remains per-blueprint: `motion.altitude` in metres above the local
land/water surface. The current T1 interceptor/bomber use 22/18 m. Future T2/T3
fighters and recon can use independent heights without changing the flight code.
Only aircraft whose vertical hull bands overlap push one another apart.
Idle fighters automatically intercept hostile aircraft; air factories are ground
targets, so fighters cannot attack them. Ground hulls collide across locomotion
types, including tanks and amphibious commanders.

Use `cargo run -p mc-game -- --scene formations --map dev16` to watch ten fighters,
five bombers, and a nine-tank block travel two legs and settle. The scene also
works with `--ticks 300 --screenshot formations.png`. This movement/state update
requires matching protocol/replay version 3; older replays are rejected.

## Complete air and AA roster

The tiered air roster, aircraft circling on guard, shielded reclaim carrier and drones, amphibious AA defenses, mobile AA, and redesigned Aerie factory are described in [Air roster and anti-air defenses](docs/AIR_ROSTER.md).

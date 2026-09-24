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
cargo run --release -p mc-map --bin mc-bake -- --layout survival --size-km 14 --seed 11 --name "The Crucible" -o maps/crucible.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout alpine --size-km 8 --seed 3 --name "Serac Divide" -o maps/serac_divide.mcmap
cargo run --release -p mc-map --bin mc-bake -- --layout alpine-teams --size-km 12 --seed 5 --name "Serac Sound" -o maps/serac_sound.mcmap
```

Then, from the repository root (the game looks for `data/` and `maps/` there):

```bash
cargo run --release -p mc-game                                   # the front end: main menu, skirmish set-up, settings
cargo run --release -p mc-game -- --map dev16                    # straight into a skirmish against the AI
cargo run --release -p mc-game -- --map meridian_basin --players 8   # 8-way on the 80 km map
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
cargo run --release -p mc-game -- --map twin_shoals --ticks 3 --plans --screenshot plans.png --camera 7350,7340,480   # planned structures and order lines; --cursor X,Y --drag X,Y drags one
cargo run --release -p mc-game -- --ui skirmish --screenshot ui.png      # a front-end screen: menu | skirmish | settings
cargo run --release -p mc-render --example structure_shots -- maps/twin_shoals.mcmap shots wharf:aster_t3_naval_factory@water,0,0:140,0,0.9,0,-22   # structures on the map, no match: KEY@x,y,heading (x `land` or `water` finds a spot)
cargo run --release -p mc-game -- --smoke              # unattended: front end, a short skirmish, back, exit
cargo run --release -p mc-game -- --dump-sounds sounds/   # the synthesised sound set as WAV files
cargo run --release -p mc-game -- --dump-cursors cursors.png   # every mouse pointer, over dark, grass and bright ground
```

## Controls

| input | action |
|---|---|
| left click / drag | select unit / box select (shift adds); double-click a unit to take every one of its type on screen; a box takes combat units first, then builders, then structures |
| right click | move; attack an enemy; assist or repair a friend; reclaim a wreck (engineers, reclaimer towers), or an enemy when nothing selected is armed; on a factory, the way out for what it makes (shift queues more) |
| the mouse pointer | says what a click would do. Over an enemy with anything armed selected: attack; over a wreck with builders: reclaim; over a friend with builders: assist; over one of yours otherwise: select. An armed order shows its own pointer, or the barred circle where it cannot apply, like a structure that does not fit. With shift over an order: pick it up. Open ground keeps the plain arrow: a right click there moves, as ever |
| shift + order | queue |
| hold shift | every order of your side shows on the map, and every planned structure as a ghost (the selection's show without shift). Drag a waypoint or a planned structure to move it: a shared waypoint moves for the whole group, a structure turns red where it cannot go. Right click or Esc puts it back |
| construction panel (bottom right) | T1/T2/T3 tabs; a tile places a structure (left click; shift keeps placing; right click cancels) or queues a unit in a factory (shift: five; right click takes one out). Hover a tile for its data card |
| queue strip (over the construction panel) | what the selected factory or engineer is working through; on a factory, click adds one, right click removes one |
| order card: M / A / F / C / R, then left click | move / attack / attack-move / assist / reclaim; right click or Esc cancels. Reclaim takes a wreck, one of your own units or structures, or an enemy: a live unit loses health as it is taken apart and gives a fifth of its mass back, and what is reclaimed to nothing is gone with no blast and no wreck. An idle engineer mends wounded allies in reach (a quarter of the build cost) before it clears wrecks |
| P, then left click | patrol: the selection sets off at once, out to that point and back to where it stands (with shift, where its queue ends), walking the loop in formation and fighting what it meets. Keep shift held and click on to add posts to the loop; let go of shift to finish |
| group badge (a number on a waypoint) | a group ordered together walks as one: one line from its middle to its waypoint, and the waypoint wears how many are in it. Click the number to select that group again (shift adds it); order some away and they get a badge of their own |
| X / U / L | stop (a factory clears its queue and its orders) / upgrade the selection to its next tier (structures, the commander's engineering suites) / toggle a factory's repeat |
| G / FORMATION | open formation controls: together/free movement, compact/standard/wide spacing, and form up here |
| minimap | left click or drag: look there (or aim the armed order); right click: order the selection there |
| selection panel | several units: one tile per type, click keeps only that type, shift-click drops it |
| idle engineers / idle factories chips | select them all and bring the camera |
| Pause | pause (single player) |
| + / - | game speed, 0.05x to 10x (single player) |
| Ctrl+Delete | self-destruct |
| Ctrl+0-9 / 0-9 | set / recall a control group; twice quickly also brings the camera. Groups show as chips over the selection panel |
| Home | jump to your commander (or, watching, to a living one) |
| wheel, W/S/D or arrows, Q/E, PgUp/PgDn, middle drag | zoom to cursor, pan, rotate, tilt, pan |
| hold Alt, move the mouse | orbit around the unit under the pointer (or the tracked unit, or the ground); release to put the camera back |
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

Slow motion is the ordinary game speed control: `-` goes down to 0.05x.

## Layout

| path | what |
|---|---|
| `crates/mc-core` | fixed point, integer trig, RNG, state hashing |
| `crates/mc-jobs` | worker pool, job graph, deterministic parallel-for, background tasks |
| `crates/mc-map` | `.mcmap` format, tile streaming, sim heightfield, `mc-bake` (basin, islands, survival and alpine layouts) |
| `crates/mc-path` | hierarchical flow fields with deterministic background builds |
| `crates/mc-sim` | the simulation: state tables, spatial index, commands, economy, combat, AI, snapshots |
| `crates/mc-data` | blueprint loader; `data/factions/aster/` is the Aster faction |
| `crates/mc-net` | lockstep protocol, relay (`mc-relay`), sessions, replays |
| `crates/mc-render` | Vulkan renderer, WGSL shaders, procedural models and textures, the 2D overlay and its type |
| `crates/mc-game` | the `meridian` binary: front end (`ui/`), the match (`game.rs`) and its HUD (`hud/`), sound (`audio.rs`), tools |

## Status

Playable: a land war with the Aster faction (commander, three engineer tiers, nine combat units,
nineteen structures across three tech tiers), flow economy, construction and assisting, factories
with queues and standing orders (move, attack-move, patrol, attack, assist, orbit, ground fire: every unit
made takes them, and a factory still going up takes both its queue and its orders), in-place upgrades, reclaim (wrecks, your own units, enemies; idle engineers
clear the wrecks within their reach while there is room for the mass, and so does the tech 2 Scavenger
tower over a much wider one, though its turret is slow to aim and has to charge before the beam comes on;
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
central lake, a ridge in each passage, two town islands reachable by amphibious units and hovers). Local matches are recorded to `last-match.mcreplay`; the
relay records with `--replay-dir`.

Implemented and tested at the crate level, but not yet exercised end to end from the game
binary: late join and reconnect (the relay's snapshot hand-off in `mc-net`, exact state
snapshots including in-flight flow fields in `mc-sim`/`mc-path`).

Measured on the 80 km map with 8 players and ~4,000 units in combat: simulation 4 ms per tick on
average (budget 25 ms); GPU frame about 1 ms at 2560x1440 on an RTX 3080 Ti with 260,000 props.
In an unpaced `--bench` run the first cross-map flow field can take longer than the two ticks it
is given, because ticks run back to back instead of 100 ms apart; at game speed it has 200 ms.

Not built yet: naval and experimental units; the Naga faction (the data format and engine
are faction-agnostic); battle sounds beyond the first library in `data/sounds` and `data/factions/aster/sounds.ron`
(unit files name their sounds; only the Warden's have been reviewed by ear, see `docs/STYLE.md`);
replay playback from the game
binary (`mc-net` has `ReplaySession`; the binary only records); a
multiplayer lobby UI (network matches are still set up from the command line); patrol and guard orders; reclaiming trees on an order; authored art
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

The tiered air roster, orbit command (**O**), shielded reclaim carrier and drones, amphibious AA defenses, mobile AA, and redesigned Aerie factory are described in [Air roster and anti-air defenses](docs/AIR_ROSTER.md).

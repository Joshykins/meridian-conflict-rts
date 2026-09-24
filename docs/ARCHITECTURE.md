# Meridian Conflict — architecture

The requirements live in the design note (`docs/SPEC.md`). This file records how the
code meets them and the rules every crate follows.

## Crates

| crate       | role                                                                              | may use floats |
|-------------|-----------------------------------------------------------------------------------|----------------|
| `mc-core`   | `Fx` fixed point, `Angle`, vectors, `Rng`, `StateHasher`, `PlayerId`               | presentation helpers only |
| `mc-jobs`   | persistent worker pool, job graph, deterministic `parallel_for`, background tasks  | n/a |
| `mc-map`    | baked `.mcmap` files, tile streaming, sim heightfield, procedural baker            | baker and render-side only |
| `mc-path`   | hierarchical flow fields over a sparse nav grid                                    | no |
| `mc-sim`    | all game state (flat tables), spatial index, commands, tick, hash, snapshot, replay| no |
| `mc-data`   | factions and blueprints loaded from `data/` into flat blueprint tables             | load time only: decimal literals are converted once, exactly, to fixed point; peers compare a content hash |
| `mc-net`    | lockstep protocol, relay server, client session, replay files                      | no |
| `mc-render` | raw Vulkan (ash) GPU-driven renderer                                               | yes |
| `mc-game`   | the `meridian` binary: window, input, camera, UI, tools and test scenes            | yes |

Dependencies point downward only: `mc-sim` never sees the renderer or the network.

## World conventions

- One world unit is one metre. The ground plane is XY, Z is up, right-handed.
- A full-size map is 81 920 m on a side (80 km). Smaller maps are allowed; larger are not.
- Height/path cell: 8 m. A full map is 10 240 × 10 240 cells.
- Build cell: 12 m. Structures snap to it. Pathing blocks only the hull (`UnitBlueprint::hull`, half-extents in the structure's own frame, default the radius; `world::hull_cells`): an 8 m cell is blocked when the hull covers more than 3 m of it across. The rest of the lot is walkable apron. Lots still may not overlap: `Nav` keeps a separate lot-occupancy layer (`set_lot`/`lots_free`, rebuilt from the units on restore, never snapshotted) that `can_place` checks. Economy structures stand at least 5 m back from their lot edge, so any two packed lot to lot leave an 8 m lane at either 8 m alignment of the 12 m grid (`tests/place.rs`).
- Map tile: 256 cells (2 048 m). A full map is 40 × 40 tiles.
- Nav sector: 32 cells (256 m).
- Angles are `mc_core::Angle`: `u16`, 65 536 per turn, zero along +X, counter-clockwise.
- Simulation rate is 10 ticks per second. Rendering interpolates between the last two ticks.

## Determinism rules (apply to mc-sim, mc-path, and anything they call)

1. No `f32`/`f64`. Use `Fx`, `Angle`, integers.
2. No `HashMap`/`HashSet` iteration, no pointer-derived ordering, no wall-clock time, no
   thread ids. Use `Vec`, sorted vectors, `BTreeMap`, or index tables.
3. Parallel work is split into chunks whose boundaries depend only on the data size. Each chunk
   writes its own output; outputs are merged in chunk order. Results never depend on the worker
   count or on which job finished first.
4. Background results (flow fields) are adopted on a tick chosen when the request is made, never
   "when they happen to finish". If a result is late the sim joins it; it never proceeds without it.
5. Every limit is explicit. Exceeding one returns an error that reaches the player; nothing is
   dropped silently.
6. No unbounded work in a tick: loops are bounded by table sizes, per-tick budgets or fixed
   iteration caps, and budgets are counted in deterministic work units, not time.

## Game state

All state is in flat struct-of-arrays tables owned by `mc_sim::World`. Rows are dense; ids are
`(index, generation)` handles resolved through a slot table. Anything that is not in a table is
either immutable input (map, blueprints) or derived and rebuildable from tables (spatial index,
flow-field cache, render mirror).

A replay is `(map id, blueprint hash, seed, player setup, command log)`.

**Materials and core mines** (`mc_sim::mines`). Maps carry ore fields as polygons
(`mc_map::OreRegion`, format v2). A blueprint with `mine` is a core mine: every hectare of
land within `reach` (1800 m, the same for every tier) pays `ground` materials a second and
every hectare of ore `per_hectare`, counted on grids (`OreGrid`: land at 32 m, the overview's
pitch, ore at 8 m). Overlapping mines, anyone's, divide the ground as a power diagram: a cell
goes to the mine with the least `distance^2 - reach^2`, so the border between two is the
straight line through the points where their circles cross. `State::mines` keeps each mine's
territory (`Share`: land and ore it has, and would have alone; efficiency is the ratio),
re-counted only when the set of mines changes. There is no feed or investment: the tiers
(`aster_core_mine` -> `_t2` -> `_t3`, upgraded in place, same unit id) are the investment and
raise the per-hectare yield; `_t4`, the deep core, adds only to the shaft's `base`. Mines may
stand anywhere a structure fits, on open water too (`water_build`). Ore lies deep
(`OreRegion::depth`, 140-400 m, hashed from the outline): a mine sinks its main shaft at
`SHAFT_SPEED` and drives a drift to each field at `DRIFT_SPEED` once the shaft reaches that
depth; a field's ore pays only from `Vein::reached_at` (mine age, kept through upgrades).
The renderer draws the ore as real geometry (`ore_vein_mesh`, `fs_vein`, additive, no depth
test) during the survey; the HUD draws territories, shafts and drifts, cut to the screen. The AI puts mines on
the nearest free fields a reach apart, then on bare ground once it has defence and energy.

**The core mine's pit and pipe** (`models/aster/mine.rs`, `models::Pit`). The model digs a
real hole below ground level, and the terrain is drawn across it, so the vertex shader pulls
what is inside the pit and below its opening up in depth, to just behind where the eye's ray
crosses the opening (`PIT_SQUEEZE`), keeping its own order; where that ray crosses outside
the opening the geometry is behind rock and keeps its true depth. The pieces down there are
cut short so no triangle spans much of that change-over. The mine works on a beat: the mirror
publishes blows struck in `UnitInstance::gait` (`mines::hammer_gait`, from the mine's age, one
blow every `hammer_ticks(tech)`), and `pipe_offset` in entity.wgsl poses the driver
(`part::RAM`), the pipe string (`part::STRING`, driven down one `Pit::section` a blow; it
repeats every section, so the loop is seamless) and the next section (`part::FEED`, rising out
of the magazine at `Pit::rack` and swinging in). `renderer/mine_fx.rs` puts dust, sparks, spray
and the tech 4 shockwave on the same beat, and the game plays the unit's `step` sound on it the
way it does footfalls. Standing in water (the terrain under it below the sea), the shader
raises the rig by `Pit::afloat_lift`, shows `part::AFLOAT` (stilts, moon pool) and hides
`part::ASHORE` (the pit, the broken ground). `--scene offshore-mine --map twin_shoals` stages two.

**Refits** (`mc_data::refit`). A unit file's `refits` lists slots, each holding one module;
a module with `after` is the next tier of another and replaces it, the rest of a slot are
alternatives. Loading compiles every loadout to a blueprint of its own (key `base+mod+mod`)
and every module to a hidden kit blueprint carrying its price, so the sim only ever sees
ordinary blueprints: `Command::Refit { kit }` queues an `Upgrade` order whose blueprint is the
kit, it is assembled like any in-place upgrade, and on completion the unit's blueprint becomes
`Blueprints::refit_result(current, kit)`. `Blueprints::is_listed` hides loadouts and kits from
menus and lists. The renderer builds one model per refit family (`build_model_fitted`, pieces
tagged by module key through `MeshBuilder::module`/`until`); `ModelInfo::modules` holds each
loadout's look bits and `UnitInstance::refit_modules` the loadout being fitted, so the vertex
shader shows, raises or fades each piece. The HUD's refit tab (`hud/refit.rs`) is drawn from
the data alone: any unit given `refits` gets it.

## Frame / tick decoupling

The sim runs on its own job-graph thread group. At the end of a tick it publishes a compact
render mirror (one packed record per entity). The renderer uploads the mirror once per tick and
the GPU interpolates, culls, picks LOD and builds its own indirect draws, so the render thread
does no per-entity work per frame and frame rate does not depend on sim load.

## Application stages (mc-game)

The binary is one window and three stages: the **front end** over its backdrop, a **loading
card**, and the **match**. A renderer is built for one map, so every stage change drops the
renderer and builds a new one (about half a second on a discrete GPU); that is also what
guarantees nothing of the last battle (scorch marks, terrain edits, fog) reaches the next. The
card is drawn by the outgoing renderer before the swap blocks the thread.

The front end's backdrop is a real match: the `backdrop` scene scripts two armies onto contested
ground, and `ui::backdrop::Director` films it. Leaving a stage drops its `SimHandle`, which stops
that simulation's thread. `--smoke` walks every stage change unattended.

## Interface toolkit (mc-game `ui/`, mc-render `overlay`)

Immediate mode. Screens are laid out in points on a canvas 1080 points tall and
scaled to the window. `Overlay` owns one RGBA atlas: the 8x8 bitmap font (no longer used by the game), outline
glyphs rasterised on first use at exactly the pixel size they are drawn at (so type is crisp at
any scale; Barlow, SIL OFL, embedded), and four 512 px image slots (map previews). The renderer
uploads the rows that changed. Blending is linear-light into an sRGB target, so dark glass uses
`ui::ink`, which bends opacity to what it looks like rather than what it multiplies by.

Controls make their own sounds, so nothing interactive can be mute. Sound is synthesised at
start-up on a background thread at the device's rate (`audio.rs`: a small mixer behind `cpal`);
there are no audio files. The tests check that no sound clicks or is cut off.

## Match HUD (mc-game `hud/`)

Built from the same toolkit as the front end, so it shares its type, palette, hover and press
behaviour and sounds. `Hud::draw` lays the panels out, records the screen areas it covered
(`covers`, which is how the match knows a press was not meant for the battlefield) and returns
`HudAction`s; only `game.rs` turns those into `Command`s, camera moves or mode changes. Unit
symbols are the strategic icons redrawn as overlay geometry (`hud/icons.rs`), so a tile reads
like the battlefield does.

What the HUD needs beyond the render mirror comes from the sim thread, never from the sim's
state directly: `SimHandle::watch` names the selected units, and each publish carries their order
queues (`World::write_orders`), which feed the queue strip, the "producing" line and the order
lines on the ground. A changed selection is answered at once, so it also works while paused.
While shift is held `Watch::everyone` asks for the queues of the whole side, and every publish
carries the side's planned structures (`World::write_plans`: build orders not yet begun).

Orders on the map are `orders.rs`: the lines, a ghost for every planned structure (the
selection's; everyone's while shift is held or a structure is being placed), and dragging.
With shift held a press on a waypoint or a planned structure picks it up, and the drop leaves
as one `Command::RelocateOrder { units, kind, from, to }`. The sim finds the order again by
kind and exact position, not by its place in a queue, so it does not matter that queues have
moved on while the command was in flight, and a waypoint a group shares moves for the whole
group. The sim refuses a structure planned across another plan of the same player, whether
it arrives as a `Build` or as a drag (`World::plan_blocks`); the same plan in another
builder's queue is allowed, because whoever arrives second joins in. The interface applies
the same rule to the preview's colour (`orders::site`), and shows a dropped order where it was
dropped until the sim says the same. A click-and-drag while placing lays a line of that
structure, spaced a footprint apart (`orders::line_centres`); the first `Build` replaces the
queue unless shift is held, and the rest append. Core mines stay a single site.

The mouse pointer (`pointer.rs`) says what a click would do. `Game::pointer_for` asks the very
functions a click goes through (`context_command`, `targeted_command`, `placement`, the order
map) and maps the command that would leave to a pointer, so the two cannot disagree. The
pointers are the HUD's vector glyphs with a dark rim, drawn through `Ui` into an overlay of
their own and rasterised on the CPU (4x supersampled, linear-light) into hardware cursors:
the window system draws them, so the pointer never waits for a frame. They are rebuilt when
the interface scale changes; where custom cursors are not to be had, system cursors stand in.
`STATE_IDLE` in the mirror's `owner_flags` marks units with no orders (idle-builder chips).

Range rings (`rings.rs`) are what the selection, and the structure being placed, reach: one
colour per kind of reach (direct fire, artillery, missiles, anti-air, torpedoes, radar, build),
read off the blueprint (a weapon's targets, then its trajectory), with a weapon's dead zone as a
dashed inner circle. The game hands the renderer a list of `RangeRing`s with centres already
interpolated to the frame; `ranges.wgsl` generates each circle from the vertex index, drapes it
over the height map at a constant width on screen, and drops every piece that lies inside
another ring of the same kind, so an army shows the outline of what it covers together. That
test is every vertex against every ring, so once a tick the game sorts out the rings that lie
wholly inside their kind's reach (`rings::hidden`): they are uploaded to mask the others but not
drawn, which leaves a blob of tanks with the few rings on its edge. 499 selected units cost
1.4 ms of GPU time on an RTX 3080 Ti; a usual selection costs nothing measurable. The HUD prints
a key to the colours on the ground above the deck (`View::reaches`).

Pause and game speed belong to the session's clock (`Session::set_paused`, `set_speed`): only a
single-machine session owns one, which `SimStatus::owns_clock` reports and the HUD's controls
follow. Render interpolation spans `0.1 s / speed`, the gap ticks really arrive at.
`hud` has tests that click through the real panels with synthetic pointer input.

## Combat presentation (mc-sim `mirror`, mc-render, mc-game `audio`)

What a fight looks and sounds like is decided outside the simulation; `docs/STYLE.md` says what it should be.

- The mirror's `ShotFired` and `Impact` events name the weapon (`blueprint`, `weapon`), so the renderer and the audio
  pick flashes, debris and sounds from the weapon's data. `Impact::after` is how far into the tick the shot landed.
- A shot that hits something leaves the projectile table in the same tick. `World::spent` (not state, cleared each
  tick) keeps its last stretch for the mirror, which emits it as a projectile that ends part-way through the tick
  (`PROJECTILE_ENDS_SHIFT`); `PROJECTILE_FRESH` marks a shot whose `prev_pos` is the muzzle. So a shell is drawn from
  muzzle to target even when its whole flight fits in one tick. Hits are shown on the target's skin (`Hit::seen`),
  while damage and blasts still use the sweep's own point.
- Renderer rings, written once and animated on the GPU from a birth time: flashes (`sprites.wgsl`), puffs
  (`puffs.wgsl`: dust, smoke, clods, sparks; premultiplied alpha so one pipeline covers and adds light), and track
  marks (`ground.wgsl`). `Renderer::ground_contact` lays marks and dust for moving models that report `Treads`
  (set by `tracked_chassis`), only near the camera.
- Reclaim is `mc-sim/src/reclaim.rs`. A wreck gives mass at the reclaimer's power; a live unit
  (`Command::ReclaimUnit`, `OrderKind::ReclaimUnit`: the player's own or a detected enemy's, never an ally's) is
  unbuilt at the rate the same power would build it, pays `UNIT_YIELD` of its mass scaled by how built it is (a fifth;
  less than a repair's quarter, so there is no loop to profit from), and when its health runs out gets `flag::RECLAIMED`:
  `despawn_unit` then emits `SimEvent::Reclaimed` instead of `UnitDied` and leaves no wreck or stain (commanders
  excepted). A unit with no orders runs `idle_repair` then `idle_reclaim`: it never moves and holds no order (so it still counts as idle
  to the HUD and the AI). A mobile builder mends wounded allies within reach (`flag::REPAIRING`) unless a friend is already taking the hull apart; a reclaimer
  clears wrecks within reach while the player's mass store has room. A reclaimer tower
  (`UnitBlueprint::reclaimer`) is the same with a longer reach, a slow turret (`turn`) and a charge
  (`charge_ticks`) before the beam comes on (`World::reclaim_ready`, `Units::reclaim_charge`). Live units are only reclaimed on an order, and a
  builder with weapons does nothing of the sort while it has a target or a detected enemy stands within 1.25 times
  its guns' reach (`enemy_in_gun_range`), so its torso is never on a wreck or a hull between two targets. Repair
  costs a quarter of the target's build (`repair_scale`); `World::reclaims` (not state) lists the tick's reclaim
  work, and `flag::REPAIRING` drives the repair beams. The mirror turns both into `RenderFrame::beams` (+ `beam_sources`). The sim says
  nothing of a beam starting or stopping: `Renderer::upload_beams` and the game's `battle_sounds` both work that out
  from who is on the list from one tick to the next, the renderer keeping a shut-off beam for `BEAM_LINGER` seconds
  so the bits already in flight arrive (a bit is drawn only if it was born while the beam was on). `beams.wgsl`
  draws each beam as 32 instanced quads: the
  ribbon, a glow at either end, and 29 shards whose flight along the beam is a pure function of time and a seed taken
  from the emitter's position. `BeamInstance::kind` 0 is reclaim, 1 is kept for construction, 2 is repair
  (mint-green patches leaving the emitter and seating on the hull). `--scene reclaim` and `--scene repair` stage them.
- Bloom is a five-level down/up chain in `screen.wgsl` (thresholded, firefly-weighted first level). Its render
  passes carry explicit subpass dependencies: without them an NVIDIA driver reads a level while it is still being
  written. The unit shader clamps its output, because a NaN or an infinity in the scene would spread through the chain.
- Battle sounds are data: `mc_data::sounds` loads the library (`data/sounds/*.ron`, each faction's `sounds.ron`),
  recipes of layers (`Tone`, `Stack`, `Fm`, `Burst`, `Hiss`, `Sweep`, `Rumble`, `Drone`, `Drive`) with `like`/`size` for
  variants, and checks at start-up that every name in a unit file's `sounds` blocks exists. None of it is in the
  content hash. `audio::from_recipe` synthesises them; the interface set is still code. `Game::sound_table` turns
  names into ids (again when F9 reloads the library), `Game::battle_sounds` places one-shots by the camera
  (`Audio::play_world`, loudest few per tick) and hears moving units as a few loops per movement sound
  (split across the view, slightly detuned, so a column is not one machine),
  reclaim beams into `reclaim_beam`, repair beams into `repair_beam`, and construction beams into `build_beam`
  (`Audio::set_loops`). `SimEvent::WeaponCharging` is raised `charge_time` before a salvo for weapons that name a
  `charge` sound. A unit answers being selected the same way (`data/sounds/responses.ron`): a unit's
  `sounds.select`, or the library's `defaults.select` for its `icon` kind. `Game::answer_selection` compares the
  selection with the one it last answered, once a frame, so every way of selecting is covered and a death in the
  selection is not one; the units new to it answer in the voice most of them share (every tier of a unit alike),
  through `Audio::play_response` (interface volume, no place in the world).
- `--follow N [--alpha A]` plays N more ticks through a headless renderer before the screenshot, so smoke, dust, track
  marks and shells in flight can be checked without a window. With `--ticks 1` the scene's orders are given on the
  first followed tick, so what they set off (`--scenario destruct`) happens where the renderer sees it.
- **Upgrades** are one mechanism for everything. A blueprint names its successor (`upgrades_to`); the successor's
  `cost` is the price. `Command::Upgrade` queues `OrderKind::Upgrade` behind the unit's other orders, the unit builds a
  hidden successor (`flag::UPGRADE | IN_FACTORY`, which is what assisting engineers help with). Only `Stop` and
  `Command::CancelUpgrade` (a right-click on it in the queue strip) scrap it: a mobile unit being refitted is pinned
  (`give` keeps the refit at the front and puts the new order behind it), so it neither walks nor builds until it is
  done, though it still shoots at what comes near. A
  structure is replaced by its successor; a mobile unit and an intel tower (`run_upgrade`, `upgrades_in_place`) stand still, and when the successor
  is done take its blueprint and the health it adds and stay the same row and id, so selections, control groups and
  `Player::commander` survive. The mirror never shows the successor: a refit is reported on the unit itself
  (`UnitInstance::upgrade`); a factory or other structure is drawn as a construction site (fill-all-over, waves from the weld, the successor's progress)
  so it looks like it is being rebuilt. Assisting engineers print on the upgrade
  the same way they print on an open site.
- **Build arms.** `builder.arm: (turn, emitter)` in a unit file says the unit builds with an arm on its turret.
  A reclaimer tower's `reclaimer.turn` is the same for its head. `World::face_work` turns `weapon_yaw[0]` to the
  work and sets the transient `flag::WORKING`; `BUILDING` (and
  reclaiming) is only set once it points there — and, for a charging reclaimer, once `reclaim_ready` has waited
  out `charge_ticks` — and `step_weapon` does nothing for a unit that is `WORKING`.   The
  mirror turns every mobile builder's `BUILDING` into a beam record in `RenderFrame::projectiles`
  (`PROJECTILE_BEAM`, `COLOR_BUILD`), drawn by `sprites.wgsl`, aimed at that builder's weld on the near
  side of the work (`UnitInstance::weld` is the closest of those, in the work's model space, and only
  while someone is printing); reclaim beams are separate
  (`RenderFrame::beams`). `build_sources` lists who is printing and where, so the game can
  hear a beam starting and stopping the same way it does reclaim.
- **Arms that pitch.** A weapon's or a build arm's `pivot` in the unit file is its elbow. `Units::arm_pitch`
  (gun arm, build arm) turns toward the target's or the work's middle at the arm's rate, within 35 degrees, and
  `world::pitched` carries the muzzle and the emitter round the pivot, so it is state and hashed. The mirror sends last
  tick's and this tick's pitch, and `prev_turret_yaw` beside `turret_yaw`; `entity.wgsl` interpolates both and pitches
  `rig::ARM_GUN` / `ARM_TOOL` vertices about `Model::arm_pivot` (`MeshBuilder::set_arm_pivot`) before the turret yaw.
- **Surfaces.** Unit texturing is procedural and per face. When `MeshBuilder` emits a planar face it also works out
  that face's frame and stores it on the vertices (`MeshVertex::face`: position on the face in metres, and the face's
  half size; level on walls, square to the model on decks, along itself for a raked beam; a tube's facets share one
  frame that wraps, marked by a negative half width; a face that fills too little of its rectangle gets none).
  `MeshVertex::surface` holds the face's `pattern` and a random byte (mirrored halves share it). `entity.wgsl` hands
  these to `shaders/surface.wgsl` (prepended by `build.rs` for shaders with `//!use surface`), which returns relief as
  a slope (a height function differenced over about a pixel, so it is filtered at any zoom), cavity, paint, team mask,
  bare steel, soot and HDR emission. The old tiled `panel_map` now only serves dirt/wear and wrecks. `MeshVertex` is
  64 bytes (locations 6 and 7 are new). Burn marks are placed by integer arithmetic on the unit id, written twice and
  kept line for line alike: `surf_ihash`/`surf_damage` in the shader paint them, `models::burns` (`burn_hash`,
  `burn_marks`) gives the renderer the same marks, and `BurnGrid` (a 32x32 top-surface height grid baked per model at
  load, noting which cells ride the turret) stands emitters on the hull under them. `Renderer::damage_fires` runs once
  a tick beside the other emitters. Plate size and burn marks scale with `Model::surface_reach` (the bounds with guns
  and arms at rest; `bounds_radius` also holds a barrel swung straight up, which coarsened long-gunned units), and T1
  field dust climbs to `Model::dust_line` (62% of the height unless `MeshBuilder::set_dust_line` says; a low hull under
  a tall mount sets it to its deck). Both ride `ModelInfo::surface`; the dust line reaches the fragment stage as
  `VsOut::dust`.
- **Rigs.** `MeshVertex::rig` carries what `part` cannot: the leg bone a vertex rides (`rig::THIGH/SHIN/FOOT`) and
  whether it is an upgrade piece and when in the refit it goes up. A model with `Model::legs` (`MeshBuilder::set_legs`:
  hip, knee, ankle at rest, stride, stance, lift) is walked by `entity.wgsl::walk_leg`: two-bone IK in the fore-and-aft
  plane toward a foot that is planted for `stance` of the cycle and swung forward for the rest, the right leg half a
  cycle behind the left (a stance under a half is a run, and the body bob inverts). `game.rs` plays a unit's `step`
  sound from the same counter, delayed to where in the tick the foot lands (`Audio::play_world_after`). The cycle comes from `Units::gait`, ground covered in 1/256 m including turning on the spot
  (`UnitInstance::gait`: the total and the last two ticks' steps, so both the phase and the ease in and out are
  interpolated). A stride is a power of two metres so the wrapping counter never breaks it.
- **Replicators (Survival).** Models in `models/replicator.rs` (contract points as consts there: ray emitter
  `(0,0,140)`, bay k at k*45 degrees with its projector at radius 100 m, z 55 and the unit printed at radius 150 m;
  node ray catch `(0,0,46)`, print emitter `(0,0,40)`). `ShieldInstance::packed` bit 27 (`mirror::SHIELD_VEIL`, set
  for `flag::INVULNERABLE` units) draws a dome as the veil (`veil_glass` at the end of `shields.wgsl`), never
  CSG-joined. `BeamInstance::kind` 4 is the replication ray (`radius` core metres, `height` node raise 0..1, lands
  46 m over `to`), 5 the print beam (`radius`/`height` the printed unit's), drawn by `replicator_vertex` /
  `replicator_fragment` in `beams.wgsl` from the two end points only (a far end behind the eye is clipped to the near
  plane), lit by `Lights::replication_light`. `UnitInstance::_pad3[1]` bit 0 (`UNIT_REPLICATING`) draws a site's fill in
  violet (`replication_tint` in `entity.wgsl`). `survival_shots` (mc-render, ignored) stages all of it on dev16.

## Sky, light and weather (mc-render `sky.rs`, `clouds.wgsl`, `clouds_sim.wgsl`; mc-data `weather.rs`)

**Light.** The sun follows the hour (`sky::sun_at_hour`: up at 6 from -x, down at 18 to +x; the afternoon default lights relief from the right of the default view); after dark a cool moon (`MOON`) lights the scene day-for-night with stars in the sky pass. `sky.rs` works out the sun's colour after the air, the sky's irradiance and the land's bounce from single Rayleigh/Mie scattering and writes them to the `Atmosphere` uniform (scene set binding 22, 496 bytes, stride-checked in build.rs). Every lit shader goes through `shade_pbr`/`shade_pbr_vis` and `apply_haze` in `bindings.wgsl`; terrain adds sky visibility (how far the land around rises above the local slope). Low sun is lifted a little for readability.

**Weather.** `mc_data::weather`: presets Clear/Fair/Cloudy/Stormy/Overcast (cover, storms, rain, towering, scale, wind, lightning) and `TimeOfDay`. A map names both in `maps/<stem>.ron` (`MapConfig`: `weather`, optional `tweaks`, `time` or `hour`); skirmish set-up's Sky rows (`ui/sky.rs`, shared with the test range's panel) pick a preset, rain, storms, wind and time of day over it (`SkyChoice`, Settings `skirmish_sky`); `App::enter` applies them with `Renderer::set_weather`/`set_hour` (`setup::map_config` finds the file by map content id). The range keeps its own (`Settings::range_sky`), plus "Storm Overhead" (`Renderer::park_storm` parks a raging storm over the pad); the panel edits a draft (`Hud::range_sky`) that only reaches the sky with its Apply button (`Game::frame` applies it), since each change starts the sky over. Each value is a `Ui::dropdown`: the arrows step, clicking the value opens its list, which `Ui::popups` draws last, over everything, and which takes the pointer while open (the HUD claims the whole screen then). Rain set past 0.6 lets any thick cloud shower, not just storm cores.

**The field.** A 1024² weather state on the GPU (binding 21: cover, storm, churn, rain) and a flow field (binding 23: local wind, plus the height and freshness of the last aircraft wake), advected by the wind and relaxed toward the air mass (`cloud_climate`, which also covers the sky past the map) and the storms `sky.rs` runs. Disturbers: aircraft leave wakes at their own height (a ragged tube thinned in the cloud, reaching 180 m above them, and broad churn that twists the billows). Wakes never set the air moving: a flight on patrol passes the same lanes again and again, and any push (an along-path vortex shear, air shoved aside) carried the cover out of those lanes into long hard-edged ribbons. Only things of radius 24+ are hulls; stirred flow is capped at 12 m/s, its bend of the billows at ~70 m, and divergence thinning at ~1%/s. Only what is near the layer stirs it: a wake fades out 60 m outside the layer (low gunships leave it alone), and a blast below the base fades out over a gap of half its reach (`Sky::blast`), so ground fighting leaves the cloud alone and only aircraft blowing up in it, reactors and nukes reach it. Big deaths and splash impacts shove the cloud outward (`Renderer::stir_clouds`; `Renderer::cloud_blast` for anything bigger, e.g. nukes), `Sky::hull` parts the cloud round anything vast. Cloud heights are measured from a cloud floor (binding 24: the land smoothed over a few hundred metres), base 190 m above it, so fixed-wing aircraft (200-300 m up) fly through the bottom of the layer; towering weather lifts cumulus tops where a drifting lift field says so. `sun_shadow` multiplies in `cloud_shadow`; the terrain darkens and gains a sheen where it rains.

**Drawing.** `fs_sky` at the end of the scene pass; `fs_march` at a third of the screen size (`MERIDIAN_CLOUD_RES`), stopped by scene depth, with rain shafts hanging under the cloud (`with_rain`); `fs_resolve` folds each frame into a reprojected history (per-texel hash start offsets stepped by the golden ratio each frame; loose clip and 6% blend when still, tight clip and 30% when moving); `vs_rain`/`fs_rain` draw falling streaks in three layers of ground-fixed tiles (60/240/960 m, the one fitting the zoom drawn, out to ~4 km), each streak at least 1.5 px wide and falling its column in 1.2-1.8 s; `fs_composite` lays the clouds over the scene before the icons. Clouds are a veil at command zooms (55% up to 3 km, solid by 10 km), clear in a bubble round the eye when the camera is down among them, and with something selected the middle of the screen shows them as a see-through cyan hologram (outline and density contours) while the selection's own clearings thin them away.

**Sound.** `data/sounds/weather.ron`: `rain_light`/`rain_heavy` loops crossfaded by the rain at the camera (read back from the weather map each frame, one frame late: `Renderer::rain_here`), `thunder_near`/`thunder_far` after each flash, delayed by distance at 343 m/s (`Renderer::take_thunder`, `Game::thunder`).

Knobs (environment): `MERIDIAN_WEATHER=clear|fair|cloudy|stormy|overcast|storm` (storm also parks one over the middle), `MERIDIAN_STORM_HERE=1` (headless shots: a storm parked over the camera's focus), `MERIDIAN_HOUR`, `MERIDIAN_CLOUDS=0`, `MERIDIAN_CLOUD_RES=1..4`. Headless GPU shots: `cargo test --release -p mc-render --lib sky_shots -- --ignored --nocapture` with `SKY_SHOTS="name:x,y,dist,yaw,tilt[,seconds];..."`, `SKY_OUT`, `SKY_HOUR`, `SKY_STIR=1` (a jet at aircraft height and a blast; `SKY_STIR=patrol`: eight aircraft up and down lanes plus two slow hulls, the case that used to cut ribbons), `SKY_PAN` (m/s scroll), `SKY_FLICKER=1` (frame-to-frame difference), `SKY_BIG=1` (1440p); prints 20-frame mean pass times.

## Local lights (mc-render `lights.rs`, `shaders/lights.wgsl`; mc-data `LightMount`)

Everything that glows lights what is around it: the ground, hulls, trees, water, and the smoke and dust in the air. One list per frame, built on the CPU from:

- **Effects**: every `push_effect` (muzzle flashes, hits, blasts, reactors, missile-defence kills) adds a timed light (`Lights::effect`). The effect's kind gives the colour, its radius the reach and brightness, its life how long the light lasts. Flashes die at once; blasts (kinds 2, 4, 5) start with a white-hot instant, then a flickering fireball. Effects pushed together at one spot merge into one light.
- **Per tick** (`Lights::tick`, from `upload_sim`): missiles' motors, energy slugs, tracers (small), construction beams and reclaim beams (line lights), napalm/ground fires, and hulls on fire.
- **Per frame** (`Renderer::upload_lights`): burning trees and fading hitscan beams.
- **Lamps**: `Visual::lights` in a blueprint (RON `lights: [ (kind: Headlights|Spot|Point, at, aim, color, intensity, range, cone, spread, night_only) ]`, hull-local, x forward). Left out, a unit gets the usual set: headlights on ground and sea vehicles, a yard lamp on small structures, four corner floodlights on big ones, nothing on aircraft or walls; `lights: []` means none. `night_only` lamps come on through dusk (`Sky::darkness`), each at its own moment; an unpowered structure's are off, and a badly hurt unit's stutter.

The list is culled to the view, dropped when its reach covers under 2.5 px, ranked (brightness × screen size, at most 2048) and binned into clusters: a 32×18 screen grid × 24 log slices of distance from the eye (4 m to 40 km). A narrow cone that spills nothing (headlights) is binned by the sphere round its cone, not its full reach. Each cluster lists at most 64 lights, weakest dropped. The list and the grid live in device-local buffers (scene set bindings 25, 26) filled by one copy from a staging buffer each frame (host-visible storage read per pixel cost several ms). Shaders call `local_lights(m, world, n, v)` (Cook-Torrance, windowed inverse square with a source size so nothing blows out up close); the terrain, units, ground decals and water use it, and puffs take `local_light_volume` per vertex so fires and blasts glow in their own smoke.

Lights are point, spot, line (a segment) or pair (two beams side by side, lateral spacing in metres). `MERIDIAN_LIGHTS=0` turns all of them off to compare looks and GPU cost; `MERIDIAN_HOUR=23` for a night shot.

## Test range (mc-game `range.rs`, `hud/range.rs`; mc-sim `debug.rs`)

The range is not a separate tool: it is a two-slot local match (`Scene::Range`, blue against red,
cheats on, fog off, no commanders) with one more HUD panel. The panel returns `RangeAction`s, and
`game.rs` turns them into the sim's `Debug*` commands, which `World::apply_debug` honours only
when the match was created with `cheats`: spawn with flags and a build state, damage or heal by a
share of full health, remove, set `flag::PASSIVE` / `flag::INVULNERABLE`, set a build state, clear
the map, free building per player, and `DebugControl`, which makes the issuing slot's later
commands apply as another slot's (`Player::acts_as`). Because all of it is commands, a range
session is as deterministic as any match, and the same code stages it headless
(`--range --scenario NAME --screenshot ...`). Scenarios either stand something around the subject
(`under-fire`, `close`, `targets`, `build`) or are an order to the subject itself (`work`, `upgrade`,
`march`, `destruct`).

A scenario that needs a builder (BUILD IT) spawns it and keeps the order it owes in
`Range::pending` until the unit shows up in the render mirror. RELOAD DATA/ is an application
event (`GameEvent::ReloadRange`): `app.rs` loads the blueprints again and enters a fresh range
through the usual loading card, so the renderer and the sim both get the new tables.

## Survival (mc-sim `survival.rs`; mc-game `survival.rs`, `game_survival.rs`, `hud/survival.rs`; mc-data `survival.rs`)

One side holds out against round after round from the **Replication Engine**, an indestructible foundry
(`replication_engine`, `flag::INVULNERABLE`, its dome the **veil**: `damage_shield` ignores hits on an
invulnerable unit, and the mirror marks it `SHIELD_VEIL` so it is drawn apart from ordinary shields).

- **Map layout.** A survival map's sidecar (`maps/<stem>.ron`) has a `survival:` block
  (`mc_data::survival::SurvivalLayout`): the engine's spot and start index, the harbor where it prints ships,
  the spawns a defender may pick (start index, name, blurb), the fronts (domain Land/Air/Naval and a path from the
  engine's side toward the defenders), and the node sites. `maps/crucible` ("The Crucible", `mc-bake --layout survival`)
  is the first; `crates/mc-map/tests/crucible.rs` checks its paths are drivable/sailable, its pads level, its
  corridors walled apart.
- **Match description.** The set-up screen's `SurvivalRules` (rounds or endless, grace, gap between rounds,
  intensity, which fronts, tech ceiling, node rate) plus the layout become `SurvivalConfig`. It travels in
  the start message's options *after* the bincode `MatchConfig` (`survival::encode_options`/`from_start`), so
  `MatchConfig` is unchanged and replays and every peer carry it. `World::begin_survival` runs right after
  `World::new`: it removes the engine side's commander, sets it to free build, raises the engine and a ring of
  guard turrets, and puts `State::survival` in place (hashed).
- **Rounds.** `run_survival` (after the AI each tick): at `next_at` a round is planned from its budget
  (`SurvivalRules::budget_against`: 800 mass at the first round and a quarter more each round, or 15%, rising 6
  points a round, of what the defenders take in from mines and reclaim over one gap, whichever is more) over
  the enabled domains (land 60 / air 25 / naval 20), mostly at `tier_at(round)` with some lower tiers, from every
  listed fighting unit of any faction (so new tiers and factions join by data alone). Nothing appears: each unit is
  spawned as a construction site on one of the engine's eight bays (ships at the harbor's slips) and built up over
  `(3 + 2·tech)` s under a print beam (`BeamInstance::kind` 5), then walks to the head of its front. When the last is
  out, the round goes down its front together (`AttackMove` along the path, then at the defenders). Idle hostiles are
  re-sent every 5 s. After the last round, the defenders win once none of the rounds' units are left.
- **Nodes.** On node rounds (`raises_node`) the engine fires its ray (`kind` 4) at a free site for 20 s, building up a
  `replication_node` there; online, it prints one unit type (chosen by site domain and the current tier) on a beat and
  sends each straight in. A node can be destroyed even while it rises; once up, it dies into a rich wreck.
  Events: `RoundPrinting`, `RoundLaunched`, `NodeRaising`, `NodeOnline`, `NodeDestroyed`, `SurvivalWon`.
- **Economy.** Nothing is handed out. The engine's rounds are the defenders' mass: every unit it prints dies into a
  wreck worth most of its cost, so a survival map has little ore and reclaim is the economy. The engine and its nodes are always revealed to the defenders
  (`survival_reveal`). The skirmish AI skips the engine side; an AI *defender* only attacks nodes.
- **Interface.** `SimStatus::survival` (`World::survival_status`) feeds the HUD's top-centre panel (round, what the
  engine is doing, what is coming per domain, reclaim income) and the node objective tiles (click to look); the minimap draws
  the fronts, the engine and the nodes; toasts and stingers (`data/sounds/survival.ron`) announce rounds and nodes; the
  ray and print beams are loops.
- **Headless.** `--scene survival --map maps/crucible.mcmap [--observe] --bench N` prints rounds and node events and a
  line a minute (`MERIDIAN_SURVIVAL=rounds:grace:interval:intensity:fronts:tier:nodes`,
  `MERIDIAN_SURVIVAL_SPAWN=i`, `MERIDIAN_SURVIVAL_WHY=1` lists where the hostiles are). Tests: `mc-sim/tests/survival.rs`.

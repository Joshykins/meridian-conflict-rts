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
- Build cell: 12 m. Structures snap to it. Pathing covers the lot by rounding out to 8 m cells.
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
any scale; Rajdhani, SIL OFL, embedded), and four 512 px image slots (map previews). The renderer
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
queue unless shift is held, and the rest append. Extractors stay a single site.

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
  unbuilt at the rate the same power would build it, pays `UNIT_YIELD` of its mass scaled by how built it is (less
  than a repair costs, so there is no loop to profit from), and when its health runs out gets `flag::RECLAIMED`:
  `despawn_unit` then emits `SimEvent::Reclaimed` instead of `UnitDied` and leaves no wreck or stain (commanders
  excepted). A unit with no orders runs `idle_reclaim`: it never moves and holds no order (so it still counts as idle
  to the HUD and the AI) and clears wrecks within reach while the player's mass store has room; a reclaimer tower
  (`UnitBlueprint::reclaimer`) is the same with a longer reach, a slow turret (`turn`) and a charge
  (`charge_ticks`) before the beam comes on (`World::reclaim_ready`, `Units::reclaim_charge`). Live units are only reclaimed on an order, and a
  builder with weapons does nothing of the sort while it has a target or a detected enemy stands within 1.25 times
  its guns' reach (`enemy_in_gun_range`), so its torso is never on a wreck between two targets. `World::reclaims`
  (not state) lists the tick's work; the mirror turns it into `RenderFrame::beams` (+ `beam_sources`). The sim says
  nothing of a beam starting or stopping: `Renderer::upload_beams` and the game's `battle_sounds` both work that out
  from who is on the list from one tick to the next, the renderer keeping a shut-off beam for `BEAM_LINGER` seconds
  so the bits already in flight arrive (a bit is drawn only if it was born while the beam was on). `beams.wgsl`
  draws each beam as 32 instanced quads: the
  ribbon, a glow at either end, and 29 shards whose flight up the beam is a pure function of time and a seed taken
  from the emitter's position. `BeamInstance::kind` 1 is kept for construction beams. `--scene reclaim` stages it.
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
  reclaim beams into `reclaim_beam`, and construction beams into `build_beam`
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
  structure is replaced by its successor; a mobile unit and a mass extractor (`run_upgrade`, `upgrades_in_place`) stand still, and when the successor
  is done take its blueprint and the health it adds and stay the same row and id, so selections, control groups and
  `Player::commander` survive. The mirror never shows the successor: a refit is reported on the unit itself
  (`UnitInstance::upgrade`); a factory or other structure is drawn as a construction site (grow-from-weld, the successor's progress)
  so it looks like it is being rebuilt. Assisting engineers print on the upgrade
  the same way they print on an open site.
- **Build arms.** `builder.arm: (turn, emitter)` in a unit file says the unit builds with an arm on its turret.
  A reclaimer tower's `reclaimer.turn` is the same for its head. `World::face_work` turns `weapon_yaw[0]` to the
  work and sets the transient `flag::WORKING`; `BUILDING` (and
  reclaiming) is only set once it points there — and, for a charging reclaimer, once `reclaim_ready` has waited
  out `charge_ticks` — and `step_weapon` does nothing for a unit that is `WORKING`.   The
  mirror turns every mobile builder's `BUILDING` into a beam record in `RenderFrame::projectiles`
  (`PROJECTILE_BEAM`, `COLOR_BUILD`), drawn by `sprites.wgsl`, aimed at the weld on the near
  side of the work (`UnitInstance::weld` in the work's model space); reclaim beams are separate
  (`RenderFrame::beams`). `build_sources` lists who is printing and where, so the game can
  hear a beam starting and stopping the same way it does reclaim.
- **Arms that pitch.** A weapon's or a build arm's `pivot` in the unit file is its elbow. `Units::arm_pitch`
  (gun arm, build arm) turns toward the target's or the work's middle at the arm's rate, within 35 degrees, and
  `world::pitched` carries the muzzle and the emitter round the pivot, so it is state and hashed. The mirror sends last
  tick's and this tick's pitch, and `prev_turret_yaw` beside `turret_yaw`; `entity.wgsl` interpolates both and pitches
  `rig::ARM_GUN` / `ARM_TOOL` vertices about `Model::arm_pivot` (`MeshBuilder::set_arm_pivot`) before the turret yaw.
- **Rigs.** `MeshVertex::rig` carries what `part` cannot: the leg bone a vertex rides (`rig::THIGH/SHIN/FOOT`) and
  whether it is an upgrade piece and when in the refit it goes up. A model with `Model::legs` (`MeshBuilder::set_legs`:
  hip, knee, ankle at rest, stride, stance, lift) is walked by `entity.wgsl::walk_leg`: two-bone IK in the fore-and-aft
  plane toward a foot that is planted for `stance` of the cycle and swung forward for the rest, the right leg half a
  cycle behind the left (a stance under a half is a run, and the body bob inverts). `game.rs` plays a unit's `step`
  sound from the same counter, delayed to where in the tick the foot lands (`Audio::play_world_after`). The cycle comes from `Units::gait`, ground covered in 1/256 m including turning on the spot
  (`UnitInstance::gait`: the total and the last two ticks' steps, so both the phase and the ease in and out are
  interpolated). A stride is a power of two metres so the wrapping counter never breaks it.

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

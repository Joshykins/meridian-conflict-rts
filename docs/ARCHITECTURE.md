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
- Build cell: 16 m (2 × 2 path cells). Structures snap to it.
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

Immediate mode, like the HUD. Screens are laid out in points on a canvas 1080 points tall and
scaled to the window. `Overlay` owns one RGBA atlas: the 8x8 bitmap font (HUD, profiler), outline
glyphs rasterised on first use at exactly the pixel size they are drawn at (so type is crisp at
any scale; Rajdhani, SIL OFL, embedded), and four 512 px image slots (map previews). The renderer
uploads the rows that changed. Blending is linear-light into an sRGB target, so dark glass uses
`ui::ink`, which bends opacity to what it looks like rather than what it multiplies by.

Controls make their own sounds, so nothing interactive can be mute. Sound is synthesised at
start-up on a background thread at the device's rate (`audio.rs`: a small mixer behind `cpal`);
there are no audio files. Every frequency in the ambience loop is a whole number of cycles per
loop and its echoes wrap, so it has no seam; the tests check that, and that no sound clicks.

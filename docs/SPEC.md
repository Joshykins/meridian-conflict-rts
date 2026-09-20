# Meridian Conflict RTS

A Supreme Commander: Forged Alliance-style RTS. Native desktop app, custom engine in Rust on raw Vulkan, Windows first.

Zoom out: the whole 80 km × 80 km map and the entire battle on screen. Zoom in: beautiful terrain and highly detailed, textured units. One continuous zoom.

Target: 60 fps on a mid-range PC. 8-player matches.

## Requirements

**Scale**

- 80 km × 80 km maps.
- Up to 40,000 entities on the map at once: about 4,000 live units, the rest wreckage, crater stains, and projectiles. maps will can support props too like cities.
- 8 players per match.
- No limits smaller than these. Hitting a limit is an error, never a silent drop.

**Graphics**

- Raw Vulkan.
- 60 fps at every zoom level. At full zoom-out the whole map and every entity are visible.
- Close-up quality: PBR materials, normal maps, moving turrets, treads and legs, shadows, effects.
- GPU-driven: no per-entity CPU work per frame. Level of detail runs from full mesh down to strategic icons.
- Wreckage, crater stains, and trees persist for the whole match.
- Frame rate never depends on simulation load.

**Simulation**

- 10 ticks per second. A tick at full load takes 25 ms or less.
- Deterministic: identical results on every machine and any core count.
- Game state lives in flat data tables. Nothing else holds it.
- One spatial index for all proximity queries. No all-pairs loops.
- Projectiles are simulated and collide with terrain and units.
- Every player action is a command. A replay is a seed plus the command log.
- No unbounded work inside a tick.

**Threading**

- Every core is used: sim, rendering, pathfinding, networking, streaming, audio.
- A job graph over a persistent worker pool. No fibers.
- Results never depend on thread count or timing.

**Pathfinding**

- Hierarchical flow fields: one shared field per destination, not a path per unit.
- Fields build in the background and never stall a tick.
- Placing a structure repairs only nearby fields.
- Structures snap to a 12 m build grid. Pathing covers each lot by rounding out to 8 m cells.
- Unit size classes. Land, naval and amphibious movement. Crowd avoidance and formations.

**Networking**

- Lockstep: only commands are sent.
- Desync detection by per-tick state hash.
- Relay server. Late join and reconnect from a snapshot. Replays and observers.

**World**

- Maps are baked files, streamed as tiles.
- Units and projectiles collide with the same terrain the player sees.
- Placing a structure flattens the ground under it. Nothing else deforms terrain. Craters are stains on the ground, not holes.

**Game**

- Commander, mass and energy flow economy, engineers, factories, tech tiers, reclaimable wreckage, fog of war, strategic icons, AI opponents, multiplayer.
- Factions are data. The engine supports many. Two are defined: Aster and the Naga. Aster is built first.
- First playable: a land war. Then air. Then naval and experimentals.

**The first faction: Aster**

- Full name: Asterian Reach Command (ARC).
- "Aster" is the name used in code, in asset names and for short labels such as the faction picker.
- Humans. The military of a large nation.
- UEF-like, but more advanced.
- Shapes are more angular and less boxy than UEF.
- Stark-white plating mixed with black and dark grey.
- Highlights are a bright, near-white blue, more highlights on the more advanced guns/units.
- Most weapons fire blue. Some are orange/conventional weapons.

**The second faction: the Naga (Build later not now)**

- Biomechanical: their technology looks grown as much as built. Very advanced.
- Black and red.
- "Naga" is the name used in code, in asset names and for short labels.
- In feel, somewhere between Aeon, Cybran and Seraphim.

**Engineering**

- One engine, one renderer. Tools and test scenes run on the same runtime.
- Timing for every sim phase and GPU pass, visible in-game.
- Tests compare state hashes across machines and thread counts.

## Open

- Mid-range spec. Proposed: 6 cores, RTX 3060-class GPU, 16 GB RAM, 1440p.

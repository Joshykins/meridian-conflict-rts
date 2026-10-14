# Editor and Vela Mission 1 implementation

Branch: codex/vela-mission-editor. Review target: dev; no direct landing.

The editor and The Contract were developed together. Vela is a complete playable
mission and a reproducible editor fixture. The native screenshot record is in
[EDITOR-SCREENSHOTS.md](EDITOR-SCREENSHOTS.md).

## Implemented

- Lossless terrain and optional-layer editing, continuous strokes, ramps and
  heightmap stamps; live cliff/scenery rebuilds and grounded object previews.
- Ground, snow, ice, trail and road painting; live water, time, climate, weather
  and climate boundaries; transactional map crop/extend.
- Stable placement identities; unit/doodad catalogues, outliner and properties;
  move/rotate/scale handles, ground/grid snapping and atomic undo/redo.
- Player names, factions, teams, starts and controllers, selected test player,
  mission selection, free camera and saved complete camera poses.
- Save/reopen and repeated Test/Stop with private streaming snapshots; simulation
  terrain flattening and prop damage do not leak into the authoring document.
- Playtest pause/speed, reveal/fog, catalogue spawning and deterministic forward
  jumps. Live data, shader and model reload preserve working state on failure.
- Shared UI/mailbox commands, bounded requests and validated atomic batches.
- Deterministic mission events, objectives, queued dialogue, camera cues and
  full-world checkpoints. Mission-only Company units stay out of skirmish production.
- Authored destructible/reclaimable props, custom collision geometry, salvage
  beams, storage limits and snapshot-safe remaining materials.
- The Contract: Bastion cargo landing, coast road, miners, betrayal, breakout,
  physical ARC arrival/unloading, optional holdouts, harbour push, spaceport,
  Mara's escape and the quarry ending. Thirteen destructible propaganda boards
  have localized captions and optional one-shot dialogue.
- The 176-object recipe includes both route refinement and excavation stamps.
  scripts/vela.sh rebuilds terrain and applies the recipe in a private staging
  folder before publishing the completed map.

## Final verification

Rebased without conflicts onto dev b40fc0fb. The implementation was verified at
a7c7b9d7; subsequent delivery changes are documentation, screenshots and a bake
destination guard.

- Full scripts/check.sh passes in 209 seconds, including 867 simulation
  integration tests and all five enabled performance budget tests.
- Windows and Linux determinism tests pass at 0, 1, 3 and 8 workers and across
  snapshot restore: every-domain e88dc67e63991b2c, commander AI b914c756d95136a4.
- The ordinary-order pilot wins at tick 6182, hash 42e6841fa3ec0983, after
  restoring all six checkpoints at ticks 465, 1958, 3000, 3740, 5050 and 5632.
  Collateral combat also exercises two optional propaganda reactions.
- An independent fresh recipe bake reproduces terrain identity 2e161c07b4a1e672
  and the canonical placement/map configuration (normalizing platform newlines).
- Native Windows Vulkan inspection covers terrain/paint, environment, bounds,
  player slots, catalogues, transforms, camera, reload success/failure, salvage,
  repeated Test/Stop and mission beats. Undo/Save restores the fixture terrain
  SHA-256 7b9bd1cda5cebac60301a25b808b7018c8f66983e715236b633ccfa524c99c31.
- The playtest toolbox reaches exactly tick 600 while paused, preserves 400%
  speed, reveals six spawned units or three local units under fog, and rejects
  invalid jump/spawn requests.

## Review boundaries

The ordinary-order pilot establishes progression and checkpoint correctness;
newcomer pacing and encounter balance still require player feedback. Native
transform evidence covers rendered handles, shared commands and Undo/Save;
pointer-drag interaction has not had an automated native input pass.

The separate model-authoring/glTF module work remains deferred. Propaganda uses
existing billboard geometry with authored tint, health and localized captions;
bespoke banner, mural and loudspeaker art is not included.

See [EDITOR.md](EDITOR.md) for editor controls and API examples and
[MISSION-SDK.md](MISSION-SDK.md) for mission/checkpoint authoring.

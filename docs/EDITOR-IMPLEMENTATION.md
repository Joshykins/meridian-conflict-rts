# Editor and Vela Mission 1 implementation

Branch: codex/vela-mission-editor. Target: dev through a pull request.

The editor and The Contract are developed together. The deliverable is the full
playable mission, authored and revised through the editor and its API.

## Acceptance
- Lossless editable terrain document, persistent object identities and undo.
- Main-menu editor with terrain, placement, regions, points and cameras.
- Save/reopen and repeated test/stop preserve the authored document.
- Shared commands for UI and mailbox API; bounded requests and clear errors.
- Deterministic mission state, objectives, dialogue and beat-boundary checkpoints.
- The Contract: landing, road, prisoners, betrayal, breakout, ARC intervention,
  holdouts, harbour push, Bastion escape and the spaceport ending.
- Data/model/shader reload retains working state on failure.
- Native Windows screenshots and playable inspection, focused tests and full gate.
- PR to dev with the validation evidence. Do not land directly on dev.

## Work log
- Created an isolated worktree from 7503fae7.
- Implemented canonical height samples and transactional terrain saves preserving
  all existing terrain sections. Focused tests cover no-op roundtrip, tile seams,
  undo and failure preservation.
- Added a versioned placement schema with persistent IDs, named units/groups,
  regions, points, camera positions and doodad properties.

- Saved the first 84 mission placements through the running Windows editor API,
  then added points/cameras through the same headless command layer.
- Added deterministic compiled mission events, full-world checkpoint resume,
  objectives/comms, campaign entry points, and the real Bastion cargo landing.
- Focused validation passed: terrain roundtrip/seams (3), mission/decoder (6),
  document transaction/undo (2), match options (2), local resume clock (1), and
  complete authored references/dialogue (1). Quick format/clippy gate passed.
- Native Windows landing inspection exposed cinematic HUD clipping; the shared
  HUD correction is verified in the Company landing screenshot.

- Added optional backward-compatible prop tints and Company skin variants.
- Fixed campaign units appearing in the skirmish production inventory.
- Real-order route smoke check reaches victory at tick 5347, without teleports
  or forced triggers; encounter pacing and remaining choreography need work.

- Native Company landing screenshot verifies corrected cinematic HUD.
- Expanded real-order route wins at tick 6150 with all six checkpoints restored.
- Added warp emergence, civilian evacuation cars, physical allied transport
  unloading, base structures, queued dialogue waits, radio cues and camera pans.
- Protected story actors and mission AI settings survive snapshot restoration.

- Full gate passed on 477e6641. Windows and Linux determinism hashes match for
  commander AI and the every-domain matrix, including mission events.
- Added asynchronous data/sound, shader and model reload with cache rollback,
  an offscreen native editor service, and responsive authoring controls.
- Reload failure and model-cache recovery regression tests and quick gate pass.
  Native data and shader reload preserve camera and document; malformed model
  rejection, subsequent undo and native screenshots are verified.

- Added free camera, searchable unit catalogue, stable-ID property editing,
  repeated unit stamping and labelled authoring guides behind the panels.

- Full gate passes on 3e809574 (438 s). Native catalogue/free camera screenshots
  and successful rebuilt-model reload are verified.
- Added cliff/noise terrain brushes, two-point ramps and bounded heightmap stamp
  assets, including a crater example. Ramp/stamp atomicity and undo tests pass;
  native live terrain preview and Undo are verified.

- Native repeated save exposed Windows/WSL streaming-handle locks during folder
  replacement. The disposable backup was recovered. Editor rendering, reloads and
  tests now read unique private snapshots, owned until their last file reader
  exits. Six consecutive native saves pass, and Undo + Save reproduces the
  original terrain bytes exactly. Snapshot lifetime/cleanup regression passes.

- Added shared edit/history refresh, including grounded unit previews after
  terrain changes and all supported player slots. Regression passes.
- Added sparse undoable ground/snow/ice/trail paint and two-point roads, compact
  tool families, and live cover subrectangle/mip uploads. Saved-map texture
  equality and optional-layer undo tests pass. Native paint, high-slot unit
  placement, Save and Undo are verified; the restored terrain hash is identical.
- Native evacuation, ARC carrier arrival and ending dialogue screenshots are
  captured. Full human encounter and cinematic composition review remain.

- Saved cameras now preserve altitude, pitch and orbit/free mode. Native capture,
  move, exact recall, Save and Undo pass. Cinematic pans interpolate complete
  poses, and default mission stills use the authored cue. Full gate passes in
  257 s, including 867 simulation integration tests and 348 game tests.

- Fixed the optional holdouts encounter so rescue and loss cannot both fire.
  Objective/AliveGroup predicates and checkpoint regressions cover either order;
  the pilot continues past a failed optional rescue. The full six-checkpoint
  route still wins at tick 6150. Windows/Linux determinism hashes match
  (commander AI b914c756d95136a4, every-domain b01c6a3bb0badef6).

- Added undoable live water, time, climate, weather and climate-wall authoring.
  Configuration serialization preserves every checked-in map's settings, including
  survival layouts. Environment changes share UI/API commands and private playtests.
  Native water/climate-wall/dusk screenshots, repeated saves and Undo pass; the
  restored document environment and terrain hash match the original exactly.

- Map loading now captures validated placements and binds them into map identity.
  Missions and the editor share that immutable snapshot; legacy empty authoring
  retains its original identity. Placement changes invalidate incompatible saves.

- Added simulation use of authored doodad health, blocking and rotated custom
  footprints. Weapon traces and destruction use the same geometry; collapse opens
  cells and snapshots restore health/collision. Authored trees bypass decorative
  clearing. Four focused regressions and existing city/tree tests pass.
- The full gate passes in 185 s. Windows/Linux match with an authored billboard
  destroyed in the matrix: commander AI b914c756d95136a4, every-domain
  b9e9c78fcc8683a3. Live prop authoring and reclaim are still pending.

- Added live doodad placement and components, shared simulation/render geometry,
  dynamic prop buffers, RGB/scale/rotation controls, catalogue, selection and
  deletion. Stable identity survives deletion remapping and sorted saves.
- Terrain edits now regenerate cliff rocks, cliff relief/refinement and canopy
  from the document. Continuous terrain/paint drags publish one undo entry and
  defer scenery regeneration until release. Focused transaction, footprint and
  stroke regressions pass. Native add/move/delete, tint, unit visibility after
  scenery refresh, catalogue and generated cliff rocks are verified. Undo/Save
  restores the original terrain hash and environment exactly.
- Native review caught prop tint entering the unit warp animation and stale
  visibility masks after scenery replacement; both are fixed and rechecked.
  The full gate passed all editor/render/simulation tests; an existing network
  clock timing test failed under concurrent load, then passed alone.

- Authored salvage now uses builder/reclaimer tools, storage limits, beams and
  area orders. Remaining materials are hashed and restored; collection removes
  props, rubble and collision. Nine focused regressions pass. Windows/Linux
  match with salvage in the matrix (every-domain 3c1cff9125b9e53e).
- Ordinary matches now spawn authored units before the first tick; missions
  retain staged group spawning. Test/Stop reuses the editor renderer, camera and
  history, and is available in the offscreen service through the same Game loop.
- Full gate passes in 230 s. Native salvage beams, the exact 80-material credit,
  prop/rubble removal, two Test/Stop cycles and exact terrain hash restoration
  after Undo/Save are verified. Native review caught a missing reclaimed flag
  in the render mirror; it is fixed and covered by a mirror regression.
- A terrain-cache regression now covers discarding simulation flattening before
  reapplying authored overview heights. This final ordering fix awaits the next
  native build. The suspected blank minimap was re-inspected in the original
  captures and is correctly rendered; no minimap defect was reproduced.

- Added authored player names, factions, teams, starts and controllers, with a
  chosen playtest slot and mission key in map.ron. Native second-slot testing
  and Undo/Save preserve the baseline document.
- Added transactional crop/extend with clipped regions, preserved stable IDs,
  exact overlap samples and full Undo/Redo. Native resize and restored-baseline
  captures pass; full gate passes in 193 seconds.
- Added reveal/fog, pause, speed, catalogue spawning and forward time jumps to
  playtests. Native checks reach exactly tick 600 paused at 400%, reveal six
  spawned units or only the three local units under fog, reject invalid inputs,
  and preserve the authoring document on Stop. The local session regression
  verifies all intervening tick bundles and commands are retained.

- Added move-axis, rotation-ring and uniform-scale handles, ground/grid snapping,
  right-click cancellation and one Undo per drag. Shared transform/scale commands
  preserve components and reject invalid batches atomically. Native handle and
  transformed-object screenshots plus exact Undo/Save baseline verification pass.

- Added 13 destructible propaganda boards with optional, gated dialogue and
  localized captions; destruction state and one-shot reactions survive snapshots.
- Authored a fenced, lit quarry excavation through the editor and corrected the
  ending camera. The complete 176-object recipe includes the original route
  terrain refinement and excavation stamps. An independent fresh bake matches
  terrain identity 2e161c07b4a1e672, placements and map settings.
- The ordinary-order route wins at tick 6182 and restores all six checkpoints.
  Collateral combat triggers two optional propaganda reactions. Newcomer pacing
  and encounter balance still need player feedback; this is a regression pilot.
- Full gate passes in 125 seconds, including 867 simulation integration tests.

- Final Windows/Linux matrix hashes match with prop-death triggers:
  every-domain 9009efe2c42c3ec2, commander AI b914c756d95136a4.

## Still required before final delivery
- Full checks, determinism matrix/cross-platform verification, final native
  screenshots, docs reconciliation, and the pull request to dev.

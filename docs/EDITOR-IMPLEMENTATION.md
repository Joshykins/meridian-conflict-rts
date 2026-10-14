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

## Still required before final delivery
- Component doodads, general destructible/reclaimable props.
- Decorative cliff regeneration and resize.
- Remaining gizmo/snapping improvements.
- Propaganda components and encounter balance; human playtest and cinematic review.
- Full checks, determinism matrix/cross-platform verification, final native
  screenshots, docs reconciliation, and the pull request to dev.

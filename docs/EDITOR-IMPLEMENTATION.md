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
  native terrain-tool review is next.

## Still required before final delivery
- Component doodads, general destructible/reclaimable props.
- Ground cover painting, water/climate controls, decorative cliff regeneration and resize.
- Remaining gizmo/snapping improvements and persistent full camera poses.
- Propaganda components and encounter balance; native review of evacuation,
  allied arrivals and later mission beats.
- Full checks, determinism matrix/cross-platform verification, final native
  screenshots, docs reconciliation, and the pull request to dev.

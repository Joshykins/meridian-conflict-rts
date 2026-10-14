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
- Native Windows landing inspection exposed cinematic HUD clipping; shared HUD
  correction is implemented and awaits the next screenshot pass.

## Still required before final delivery
- Prop tint layer, component doodads, general destructible/reclaimable props.
- Ground cover painting, water/climate controls, remaining brushes and resize.
- Free camera, catalogue/gizmo improvements and reload UI/API.
- Company appearance, miners leaving, propaganda, allied transport arrivals,
  battle balance, complete real-order playthrough and checkpoint recovery.
- Full checks, determinism matrix/cross-platform verification, final native
  screenshots, docs reconciliation, and the pull request to dev.

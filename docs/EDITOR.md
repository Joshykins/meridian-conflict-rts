# Map editor

Open Map Editor from the main menu, or launch with --editor MAP. A shipped
flat map opens as a source document; Save writes a new folder beneath the
user's meridian-conflict/maps directory. Open lists both shipped and user maps.

WASD pans, Q/E orbits, and the wheel zooms. The bottom panel selects a tool.
Raise, Lower, Flatten and Smooth stamp terrain at the cursor. Unit, Region,
Point and Camera create named placements. A region uses two opposite corners.
Select picks an object in the viewport or outliner; Move places that selection
at the cursor, and the heading button rotates a selected unit.

Named objects have stable numeric IDs. Reapplying a named placement updates
that object; deleting and creating a new object allocates another ID. Names
are unique within each kind, so a region and a camera may both be called square.

Save and Save As publish the terrain and placement files together, preserving
other assets in the folder. Save As refuses an existing unrelated destination.
Undo and Redo operate on command batches. Test exports a private copy; leaving
the match restores the retained authoring document, including unsaved changes
and undo history. This first implementation rebuilds the renderer at that
transition, so a loading curtain is currently visible.

## File layout

A map folder contains terrain.mcmap, map.ron, placed.ron, and optional
mission.ron and strings.en.ron. Terrain remains the existing lossless baked
format. Placement metadata is versioned and validated before use.

To bake the checked-in Vela mission terrain, run scripts/vela.sh. The generated
terrain is ignored by Git; the authored placement, mission manifest and dialogue
are tracked. The Contract and Continue Campaign appear in the main menu.

## Editor API

scripts/editor.sh accepts a JSON request or the path to a JSON request file.
It finds the single active editor. With several editors, use --mailbox DIR.
The editor publishes its mailbox under the configuration directory's
editor/<process-id> folder. Requests and replies are published by rename.

Examples:

    scripts/editor.sh '{"op":"state"}'
    scripts/editor.sh '{"op":"edit","actions":[{"action":"unit","name":"scout","group":"company","blueprint":"aster_t1_scout","owner":0,"at":[3400,7900],"heading":90}]}'
    scripts/editor.sh '{"op":"undo"}'
    scripts/editor.sh '{"op":"save","name":"my_mission"}'
    scripts/editor.sh '{"op":"camera","at":[3368,7988],"zoom":1200,"yaw":0}'
    scripts/editor.sh '{"op":"shot"}'
    scripts/editor.sh '{"op":"test"}'

An edit request has 1–512 actions and is one atomic undo step. Invalid actions
leave the entire request unapplied. Requests are limited to 256 KiB. Supported
actions are terrain, unit, region, point, camera, transform and delete; the
authoritative schema is editor/document.rs. Coordinates are world metres,
headings/yaw are degrees, and owner is the zero-based player slot.

A headless batch uses the same command layer:

    meridian --editor maps/vela_m1/terrain.mcmap --editor-apply maps/vela_m1/authoring.json

The batch file is a JSON action array. Add --screenshot OUT.png to inspect the
result with the native renderer. scripts/editor.sh --mcp serves the same API
over MCP stdio. A request timeout means the outcome is unknown; inspect state
before retrying an edit.

## Mission checkpoints

Compiled Rust authoring in mc-missions produces deterministic event definitions.
Async authoring records explicit dependencies; a suspended Rust future is never
persisted. Conditions, fired events, named runtime handles, objectives, dialogue
queues, control state and camera cues are in the simulation snapshot and hash.

Each checkpoint stores the entire world at its tick boundary. A local session
restores that snapshot before releasing its next tick. Saves bind to the terrain
content ID and blueprint hash. Incompatible saves produce a menu notice.
Completed progress is stored separately from checkpoint snapshots.

Dialogue text is presentation data keyed by stable IDs. Mission actions never
read floating-point camera or UI data back into the simulation. A Rust script
change requires rebuilding and relaunching; reopening the map retains its saved
authored content.

## Implementation status

The editor shell, height editing, named placement, undo, atomic save/export,
playtest return, API/MCP, mission runtime, checkpoint storage and Vela route
definition are implemented. Optional RGB prop tints survive save/reopen and render
without changing legacy map hashes. Mission-only Company variants have a separate
skin and remain outside skirmish production lists.

Run the real-order route check with:

    cargo run --profile gate -p mc-missions --example play_contract

It requires the baked Vela terrain and issues ordinary player movement/attack
orders, without forcing triggers or changing unit state. The first route completed
at tick 5347. This is a smoke check; full encounter balance, miners and allied
transport choreography are still being developed. Component prop editing, the remaining terrain
and paint tools, resizing, free camera and reload controls are tracked in
EDITOR-IMPLEMENTATION.md and are not yet available in this slice.

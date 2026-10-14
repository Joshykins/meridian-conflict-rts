# Map editor

Open Map Editor from the main menu, or launch with --editor MAP. A shipped
flat map opens as a source document; Save writes a new folder beneath the
user's meridian-conflict/maps directory. Open lists both shipped and user maps.

WASD pans, Q/E orbits, Page Up/Down tilts, and the wheel zooms. Free camera
keeps the current eye position: WASD flies, Q/E changes height, arrows look,
Shift boosts movement and the wheel changes speed. Toggle it off to restore
the previous orbit view. The bottom panel groups Terrain, Paint, Objects, Environment and
Camera / Reload tools.
Raise, Lower, Flatten, Smooth, Cliff and Noise edit terrain at the cursor.
Drag to paint a continuous stroke; releasing commits one undo step.
Ramp connects the existing heights at two clicked endpoints, with the brush
radius as its half-width. Stamp reads a named JSON heightmap from the map's
stamps folder (Vela includes crater.json); strength is its height in metres. Unit, Region,
Point creates a named placement. Camera captures the current composed view;
its Capture current view button also works without clicking the terrain. A
region uses two opposite corners.
Select picks an object in the viewport or outliner; Move places that selection
at the cursor, and the heading button rotates a selected unit. Browse opens a
searchable unit catalogue including mission-only variants. Apply to selection
changes an object's name and, for units, blueprint, group and player without
changing its ID. Repeated Unit clicks place separate units.

Named objects have stable numeric IDs. Reapplying a named placement updates
that object; deleting and creating a new object allocates another ID. Names
are unique within each kind, so a region and a camera may both be called square.

Renderers and playtests stream from private snapshots, released when their last
reader finishes, so native file handles cannot lock the authored folder.
Save and Save As publish the terrain and placement files together, preserving
other assets in the folder. Save As refuses an existing unrelated destination.
Undo and Redo operate on command batches. Test exports a private copy; leaving
the match restores the retained authoring document, including unsaved changes
and undo history. This first implementation rebuilds the renderer at that
transition, so a loading curtain is currently visible.

## File layout

A map folder contains terrain.mcmap, map.ron, placed.ron, and optional
mission.ron and strings.en.ron. Terrain remains the existing lossless baked
format. Placement metadata is versioned, validated and captured when a map opens.
Loaded maps retain those placements even if the source sidecar is edited later.
Map identity includes canonical placement values; empty authoring keeps legacy
terrain identities unchanged.

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
actions are water, time, climate, weather, climate_regions, terrain, ramp, stamp, paint, road, unit, doodad, region, point, camera, transform, properties and delete; the
authoritative schema is editor/document.rs. Coordinates are world metres,
headings/yaw are degrees, and owner is the zero-based player slot.

A headless batch uses the same command layer:

    meridian --editor maps/vela_m1/terrain.mcmap --editor-apply maps/vela_m1/authoring.json

The batch file is a JSON action array. Add --screenshot OUT.png to inspect the
result with the native renderer. scripts/editor.sh --mcp serves the same API
over MCP stdio. A request timeout means the outcome is unknown; inspect state
before retrying an edit.

For a native editor without opening a window, use:

    meridian --editor vela_m1 --editor-headless --size 1280x720

It serves the same mailbox and renders at ten frames per second. State, edit,
undo, save, camera, screenshot and reload work normally. Playtest requires the
windowed editor. Close with {"op":"close"}; unsaved work refuses to close unless
{"op":"close","discard":true} is explicitly requested.

A heightmap asset contains width, height and a row-major values array. Each side
has 2–257 samples, each value is between -1 and 1, and the file is at most 1 MiB.
Stamp samples it bilinearly as signed height changes; brush radius is its
horizontal half-width and its original aspect ratio is retained.

    {"op":"edit","actions":[{"action":"ramp","from":[400,400],"to":[800,400],"width":64}]}
    {"op":"edit","actions":[{"action":"stamp","at":[600,600],"radius":128,"strength":24,"asset":"crater.json"}]}

All terrain tools share sample-level undo and the live tile/overview upload path.
Cliff edits make a steep plateau in the heightfield. Decorative cliff rocks,
terrain refinement and relief rebuild when a stroke ends, using the edited heights.
Tree placement also refreshes forest canopy and ground cover.

## Paint

Ground paints Natural, Paving, Lawn, Yard, Ballast, Field, Rubble, Earth or
Spaceport apron, preserving road data beneath it. Snow, Ice and Trail paint
coverage from 0 (erase) to 255, with feathered circular brushes. Trail heading
controls the direction of its texture. Road uses two endpoints, a width in
metres and a type: erase, Street, Avenue, Highway, Country lane or Railway.

Paint changes upload only the affected cover texture rectangle and its ice
mips. Undo restores individual changed bytes and removes a newly introduced
layer when necessary. Introducing a snow layer changes the automatic-snow
sentinel across the map, so that first edit uploads the entire snow layer.

    {"op":"tool","name":"ground"}
    {"op":"edit","actions":[{"action":"paint","paint":"ground","at":[4000,8500],"radius":160,"value":3}]}
    {"op":"edit","actions":[{"action":"paint","paint":"snow","at":[4200,8500],"radius":160,"value":255}]}
    {"op":"edit","actions":[{"action":"paint","paint":"trail","at":[4500,8500],"radius":64,"value":220,"heading":90}]}
    {"op":"edit","actions":[{"action":"road","from":[3900,8500],"to":[4500,8500],"width":16,"kind":1}]}

Ground values are 0–8 in the order above; road kinds are 0–5. Tool names use
lowercase labels with underscores, such as free_camera and reload_data.

## Doodads

Objects > Doodad opens an authoring tool for every built-in prop kind. Browse the
searchable catalogue, then click the terrain to place separate named objects.
Pick a prop or use the outliner to edit it. Appearance controls rotation, uniform
scale and RGB tint. Components controls blocking, health, reclaim value and an
optional local box (half width, half depth, height in metres at scale one).
Apply to selection preserves its ID, including when renaming it. Move and Delete
use the same selection; undo restores the original components and prop table.

Health zero means indestructible. A blocking indestructible object stops shots
as well as movement. A prop without native collision geometry needs a custom
footprint before health, blocking or reclaim can be enabled. Destruction releases
blocked cells and uses the same footprint for impact and collapse presentation.
Reclaim values are authored and preserved; collection is still being implemented.

    {"op":"choose_doodad","kind":"CityBillboard"}
    {"op":"edit","actions":[{"action":"doodad","name":"district_sign","kind":"CityBillboard","at":[4000,8500],"heading":90,"scale":1.5,"tint":[255,120,60],"blocking":true,"health":400,"reclaim":20,"footprint":[8,2,15]}]}
    {"op":"select","id":1}

The component side table remains keyed to baked prop indices; saves and deletions
remap those indices while named identities stay fixed. Props, tint, destruction
geometry and cliff scenery update live without rebuilding models or pipelines.

## Environment

Water changes the sea level live, including water picking, ship previews and the
terrain-following cloud floor. Climate and Weather select a region; Add region
creates another and Remove region also removes its incident walls. Climate Wall
takes two clicked endpoints and explicit left/right region numbers. Draw walls
to the map edge or another wall; use the blend controls for softer transitions.
Time selects a day period or adjusts the clock in fifteen-minute steps.

Environment changes are atomic undoable commands, included in saves and private
playtests. Saving map.ron preserves survival layouts, browser metadata, weather
tweaks in untouched regions and all other configuration fields. A weather preset
explicitly replaces that region's prior tweaks. Water is stored in terrain.mcmap.

    {"op":"edit","actions":[{"action":"water","level":30},{"action":"time","hour":17.5}]}
    {"op":"edit","actions":[{"action":"climate","region":0,"value":"Tropical"},{"action":"weather","region":0,"preset":"Clear"}]}
    {"op":"edit","actions":[{"action":"climate_regions","regions":[{"name":"West","climate":"Desert"},{"name":"East","climate":"Polar"}],"walls":[{"line":[[8000,0],[8000,16384]],"left":0,"right":1}],"blend":100}]}

The state response includes environment and water_level. Region indices are
zero-based in the API. There are at most eight climate regions and 32 wall
segments. Map configuration input is bounded to 1 MiB; malformed files are
reported instead of silently replacing them with defaults.

## Reload

Reload data and Reload shaders are in the editor tool panel. The current renderer
stays visible while its replacement builds. The camera can still move, while
edits and playtests wait for completion. Invalid data, shaders or model files
report an error in the editor and leave the document, undo history and current
view intact. Successful data reload also updates sound definitions.

    scripts/editor.sh '{"op":"reload","resources":{"data":true}}'
    scripts/editor.sh '{"op":"reload","resources":{"shaders":true}}'
    scripts/editor.sh '{"op":"state"}'

State includes reloading and message; wait for reloading to become false, and
inspect message to distinguish completion from failure. The initial reload
reply only accepts the request.

For rebuilt models, request the exact calls for a rendered mesh, run the existing
mc-models helper with that binary request, and send its binary response:

    scripts/editor.sh '{"op":"model_calls","key":"tank_light"}'
    scripts/editor.sh '{"op":"reload","resources":{"calls":"CALLS.bin","models":"MODELS.bin"}}'

Paths are on the editor host (Windows for the native editor). The model files use
the same bounded protocol as the shot server. Every requested model must resolve
before replacements are applied. Rust gameplay changes still require a rebuild.

The catalogue and free camera are also available to tools:

    scripts/editor.sh '{"op":"catalogue","query":"company"}'
    scripts/editor.sh '{"op":"choose_unit","blueprint":"company_t1_tank"}'
    scripts/editor.sh '{"op":"camera","at":[7738,8008],"zoom":25,"yaw":0,"height":120,"pitch":15}'

Omit height and pitch to return to orbit. Both are required for a free pose;
height is the focus altitude in metres and pitch is degrees below the horizon.
Properties uses {"action":"properties","id":1,"name":"lead","group":"company",
"owner":0,"blueprint":"company_merrick"}. Omit unit-only fields when renaming a
point, region or camera. Invalid or duplicate names leave the batch unchanged.

Saved cameras preserve focus altitude, distance, yaw, pitch and orbit/free mode.
Select a camera and press View saved camera to restore its view. Legacy cameras
without a pose continue to use ground-level orbit views.

    {"op":"capture_camera","name":"street_closeup"}
    {"op":"recall_camera","name":"street_closeup"}
    {"op":"edit","actions":[{"action":"camera","name":"street_closeup","at":[7738,8008],"distance":25,"yaw":0,"height":120,"pitch":15,"free":true}]}

Cinematic pans interpolate authored height and pitch as well as position and
zoom. Mission screenshots without an explicit --camera use the active authored
cue after its pan settles.

## Mission checkpoints

Compiled Rust authoring in mc-missions produces deterministic event definitions.
Async authoring records explicit dependencies; a suspended Rust future is never
persisted. Conditions, fired events, named runtime handles, objectives, dialogue
queues, control state and camera cues are in the simulation snapshot and hash.

Each checkpoint stores the entire world at its tick boundary. A local session
restores that snapshot before releasing its next tick. Saves bind to the terrain
and captured placement content ID, blueprint hash and authored mission program hash. Separate mission
keys have separate save folders even when their terrain is identical. Incompatible saves produce a menu notice.
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
at tick 5347 before the next choreography pass. The expanded route includes
civilian evacuation cars, physical ARC cargo drops, queued comms and checkpoint
recovery. Add --resume to restore every checkpoint into a fresh world during the
route. Use --mission-pilot with a headless screenshot to inspect later beats.
Full encounter balance and visual review remain in progress. Reclaim collection,
resizing and remaining gizmos are tracked in
EDITOR-IMPLEMENTATION.md and are not yet available in this slice.

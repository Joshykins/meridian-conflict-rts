# Map editor

Open Map Editor from the main menu of a run from play.sh (`MERIDIAN_DEV=1`;
a packaged build run on its own does not offer it), or launch with --editor MAP. A shipped
flat map opens as a source document; Save writes a new folder beneath the
user's meridian-conflict/maps directory. Open lists both shipped and user maps.

WASD pans, Q/E orbits, Page Up/Down tilts, and the wheel zooms. Free camera
keeps the current eye position: WASD flies, Q/E changes height, arrows look,
Shift boosts movement and the wheel changes speed. Toggle it off to restore
the previous orbit view. The bottom panel groups Terrain, Paint, Objects, Environment,
Map and Camera / Reload tools.
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
and undo history. Test and Stop borrow the existing renderer without a loading
curtain. The terrain and prop buffers are restored on Stop, including structures
flattened or destroyed during the test. Ordinary maps spawn their authored units
at match start; missions stage their named groups through the mission program.

## File layout

A map folder contains terrain.mcmap, map.ron, placed.ron, and optional
strings/en.ron. The map configuration stores its mission key and player slots. Terrain remains the existing lossless baked
format. Placement metadata is versioned, validated and captured when a map opens.
Loaded maps retain those placements even if the source sidecar is edited later.
Map identity includes canonical placement values; empty authoring keeps legacy
terrain identities unchanged.

To bake the checked-in Vela mission terrain, run scripts/vela.sh. The generated
terrain is ignored by Git; the authored placement, map configuration and dialogue
are tracked. The Contract and Continue Campaign appear in the main menu of a run from
play.sh.

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
actions are bounds, players, mission, water, time, climate, weather, climate_regions, terrain, ramp, stamp, paint, road, unit, doodad, region, point, camera, transform, properties and delete; the
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
undo, save, camera, screenshot, reload, test and stop work normally. A headless
playtest runs the real simulation and HUD; inspect its tick, player resources,
reclaim beams and prop counts through state. Close with {"op":"close"}; unsaved work refuses to close unless
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
Builders and reclaimers collect authored salvage with their normal tools, including
area reclaim and idle work within reach. Full stores leave the remaining material
in the prop. Destruction preserves its salvage; finishing collection removes both
the prop and its rubble, opens its cells, and survives checkpoint restoration.

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

## Verification and iteration

The editor supports editable terrain and layers, component doodads, stable object
IDs, transform handles, slots, map bounds, reloads and repeated Test/Stop.
The Contract is a complete playable mission with six beat-boundary checkpoints.

Run the ordinary-order route check after baking:

    scripts/vela.sh
    cargo run --profile gate -p mc-missions --example play_contract -- --resume

The bake stages a fresh base map and applies the complete tracked authoring
recipe through the same commands as the editor before publishing the folder.
Use --output artifacts/vela-check for an independent bake. After editor changes,
update authoring.json and map-local stamps so a clean checkout reproduces them;
terrain.mcmap is intentionally not tracked.

The route pilot issues ordinary movement and attack orders, without forcing
triggers or teleporting units. The current route wins in about ten minutes of
simulation time; this is a fast regression route, not a measured newcomer
playtime or a substitute for player balance feedback. Native captures cover the
landing, evacuation, ARC intervention and quarry ending.

Propaganda doodads use named PropDead conditions. Their reactions are optional
dialogue only, with no objectives, rewards, tallies or campaign consequences.
Vask's reactions wait for ARC arrival; Mara's wait for the betrayal. Localized
prop captions use prop.<doodad-name> entries in strings/en.ron, respect fog and
disappear when destroyed.

Offscreen editors also support {"op":"test"} and {"op":"stop"}. While
testing, state.playtest reports tick, units, beams, dead/reclaimed props and player
materials; shot captures the game HUD. Stop before editing or saving the document.


## Player slots and mission

Map > Players edits the selected slot's name, faction, team, start position and
human/script or computer controller. Save roster publishes the current slots;
each change participates in Undo/Redo. Removing a slot used by a placed unit is
rejected. Test as chooses the local player without changing the saved roster.
A computer slot selected for testing is controlled by you during that test.

    {"op":"edit","actions":[{"action":"players","slots":[{"name":"Company","faction":"Aster","team":0,"start":0,"computer":false},{"name":"Opposition","faction":"Regency","team":1,"start":0,"computer":true}]}]}
    {"op":"edit","actions":[{"action":"mission","key":"vela_m1"}]}
    {"op":"test","slot":1}
    {"op":"stop"}

Slot, team and start indices are zero-based. Names are limited to 64 bytes.
A null mission key disables the mission; an empty players array uses the
match setup roster. Playtests never write normal campaign progress.


## Crop and extend

Map > Bounds sets the width, height and origin in 2048-metre tiles. Positive
origin values crop from the west/south; negative values add space there.
Choose the elevation for newly added ground, then Apply bounds. Each edge can
have 1–40 tiles. The current scene remains usable while the replacement builds;
a failed rebuild leaves the current document, camera and undo history intact.

    {"op":"edit","actions":[{"action":"bounds","origin":[-1,-1],"tiles":[8,8],"fill":-20}]}

Bounds must be the only action in that request. Poll state until reloading is
false, then inspect message and bounds to confirm completion. Undo and Redo
rebuild the scene in the same way.

The overlap retains exact height samples and paint bytes. Objects outside a
crop are removed; retained objects keep their IDs and names. Ore polygons,
regions and bridge spans are clipped. Player starts keep their indices and
are clamped inside the new map. Camera markers and climate walls move with
the terrain. A crop that removes a required survival location is rejected.
The full before/after document is retained for this infrequent undo operation.


## Playtest toolbox

Test opens a small Playtest panel; Hide folds it away and Stop test returns to
the authored document. Reveal toggles between full battlefield visibility and
the selected player's fog. Pause and Speed use the local match clock.
Advance 60 sec or Jump to seconds executes every intervening simulation tick,
including mission triggers and combat, then returns to the prior pause/speed.
Jumps run forward and are limited to one hour from match start.

Search or scroll the unit catalogue, choose a player and count, and Spawn at
camera places a temporary group in the test. Counts are capped at 64 per API
request. These units disappear on Stop; they do not change placed.ron.

    {"op":"debug","action":{"tool":"reveal","on":false}}
    {"op":"debug","action":{"tool":"pause","on":true}}
    {"op":"debug","action":{"tool":"speed","percent":400}}
    {"op":"debug","action":{"tool":"jump","tick":600}}
    {"op":"debug","action":{"tool":"spawn","blueprint":"aster_t1_tank","owner":0,"at":[3400,7900],"heading":90,"count":5}}

Debug requests require a running editor playtest. Time targets use ticks
(10 per second); state reports the current tick, local slot, pause/speed,
and reveal/jump status. Pausing before a jump keeps the simulation stopped
at exactly the target so a screenshot or inspection can follow.

## Transform handles

Select an object, then choose Move, Rotate or Scale in the bottom panel.
Drag the centre square to move on the ground, or the red/green endpoints to
constrain movement to a world axis. Rotate uses the gold ring; scale uses the
diagonal blue handle. A guide previews the target during the drag and release
publishes one undo step. Right click cancels. Scaling applies to doodads and
regions; rotation applies to doodads, units and cameras.

Grid cycles off, 1, 4 and 16 metres and also snaps new object placement. With
grid enabled, rotation snaps to 15 degrees and scale to quarter increments.
Objects remain grounded after movement and terrain edits.

    {"op":"gizmo","mode":"rotate","grid":4}
    {"op":"edit","actions":[{"action":"transform","id":1,"at":[3400,7900],"heading":90},{"action":"scale","id":1,"factor":1.5}]}

Scale is relative to the current doodad or region size. The command preserves
its stable ID and all components; invalid transforms reject the whole batch.

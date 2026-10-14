# Mission authoring

Missions live in mc-missions and compile into the game. A map folder contains
terrain.mcmap, placed.ron, map.ron and strings/en.ron. The map configuration
selects a registered Rust mission and can supply its player slots:

    (mission: Some("vela_m1"), players: [
        (name: "Company", faction: "Aster", team: 0, start: 0, computer: false),
        (name: "Rebels", faction: "Aster", team: 1, start: 0, computer: false),
    ])

The checked-in Vela roster has six slots. A human/script slot stays idle until
the player or mission issues orders; a computer slot runs the standard AI.
Editor playtests enable debugging and never write campaign checkpoints or progress.

Named objects are authored through the editor or its shared command API. Unit,
region, point and camera names have separate namespaces. Persistent numeric IDs
belong to the editor; mission code addresses names, never array positions.

## Programs and waits

The private Script builder inside mc-missions lets a mission use async functions
for readable sequences. These futures compile event dependencies immediately;
the simulation executes and saves the resulting explicit program. No suspended
Rust future or captured local variable needs to be reconstructed after loading.

    s.say("m1.road.01", 45).await;
    s.wait("roadblock_clear", Condition::DeadGroup("roadblock".into())).await;
    s.then("next_beat", Condition::Elapsed(0), vec![Action::Checkpoint(2)]);

then waits for the previous sequential event as well as its own condition.
event creates an independent trigger, useful for optional objectives or an
arrival unfolding alongside dialogue. Every event fires once. say waits until
the actual comms queue drains, so concurrent radio interruptions cannot make
the next story beat run ahead of the text.

Conditions include elapsed time, time after another event, a named unit inside
a region, group death or surviving members, unit death or low health, unloaded
cargo, objective status, named destructible-prop death, dialogue idle,
and bounded All/Any combinations. Times are simulation ticks (10 per second).
Condition decoding bounds nesting before recursive values are constructed.
Use an Active objective condition on mutually exclusive success/failure events;
a rescue also requires AliveGroup so a dead force cannot be rescued later.
Once either branch changes the objective status, the other branch stays closed.
Wire enum numbers are explicit and new variants are appended.

## Actions

- Spawn a named group; the placed blueprint, owner, heading and location apply.
- Load a group into a named carrier. This validates ownership, physical fit and
  total capacity. Land/unload and TakeOff use the normal transport orders.
- Arrive starts a warp-capable ship in transit at its authored arrival point;
  the normal emergence phase and effects follow. Schedule its first landing
  order after emergence, as Vela does.
- Move or attack-move a group to a named point through normal player commands.
- Change a slot's team; allies and shared vision are recomputed.
- Enable/disable a slot's AI and set its validated difficulty, doctrine,
  production preferences, income and build multipliers.
- Protect a named story actor from weapon damage without adding a debug shield.
  Despawn remains available for scripted departures.
- Queue a string ID, change objective status, request a camera, hand controls
  over, create a checkpoint, win or lose.

Text and speakers come from strings/en.ron. Radio squelch and camera pans are
client presentation; they cannot change a simulation wait or outcome.

## Checkpoints and compatibility

The entire deterministic world is saved at the end of a checkpoint tick. This
includes cargo, orders, projectiles, alliances, mission program, pending comms,
fired triggers and protected actors. Resume installs the snapshot before the
local session releases another tick. Every mission field participates in the
state hash and restored values are validated.

Checkpoints bind to terrain content, blueprint content and the authored program.
Editing the script or placements requires restarting the mission. Cosmetic text
changes do not invalidate saves. Distinct mission keys use distinct save folders
even when their terrain bytes match. Completion progress is saved separately.

## Verification

Run scripts/vela.sh once to bake the checked-in Vela layout, then:

    cargo test --profile gate -p mc-sim --lib mission::
    cargo test --profile gate -p mc-missions
    cargo run --profile gate -p mc-missions --example play_contract -- --resume

The pilot uses ordinary movement and combat orders. It never teleports units,
kills enemies directly, or forces a trigger. --resume reconstructs a fresh world
at every checkpoint and continues the same route. It is a smoke test, not a
substitute for playing and balancing the mission.

For native visual inspection of a later beat:

    scripts/shot.sh -o vela-betrayal run --map vela_m1 --mission-pilot --ticks 3100 --camera 7738,8008,1200,0

The standard determinism matrix includes pending mission dialogue, an objective
transition, named-prop destruction and checkpoints across its snapshot. Run scripts/determinism-cross.sh
after numeric simulation changes.

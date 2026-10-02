# Air roster and anti-air defenses

All units are available through the existing tiered factories and engineer/commander build menus. The existing Petrel light bomber remains available. Shrike now displays the Fighter role; its internal key remains `aster_t1_interceptor`.

| Tier | Aircraft | Role |
| --- | --- | --- |
| 1 | Swift | Fast, fragile scout with radar; circles on guard |
| 1 | Shrike | Fighter |
| 1 | Wasp | Low-altitude helicopter, light machine gun and unguided rockets |
| 1 | Magpie | Salvage aircraft: swept wing, tilting engine pods, one reclaim beam that works wrecks within 450 m as it flies |
| 2 | Osprey | Reclaim carrier with four salvage drones |
| 2 | Kestrel | Four-engine tilt-jet gunship with a chin autocannon and volley rocket pods |
| 2 | Hellkite | Four-engine flying fortress; 24 scattered incendiaries and three independent AA guns |
| 2 | Peregrine | Fast guided-missile interceptor |
| 1 | Courier | Fast unarmed transport; eight cargo slots; built on site |
| 2 | Bastion | Capital assault transport; carries the T1-T3 land roster; built on site |
| 3 | Raptor | Fast, highly maneuverable air-superiority fighter |
| 3 | Eclipse | Fast strategic bomber with a large blast |
| 3 | Thunderhead | Armored, shielded assault aircraft; forward rotary cannon and forward AA |
| 3 | Argus | High-flying radar, sonar, missile interception and a 1,700 m salvage ray as strong as a Scavenger III; guards a point or friendly unit |

Aircraft circle on the **Guard** order (Ctrl+G; see [The guard order](#the-guard-order)): press on a point or a friendly unit and drag out the area. They fly halfway between its centre and its edge, and follow the ally if one was picked. Shift queues a guard; Stop cancels it. If the ally is destroyed, the aircraft keep circling its last position. Move and attack commands replace a guard normally. (The separate Orbit order and its **O** key were merged into Guard on 2026-09-26.)

Osprey (tech 2, from the tech 2 air factory) builds up to four salvage drones, one at a time, each on its own pylon under the wing: a drone costs 3 mass and 30 energy over 3 seconds, paid like any build, in the tier the side's materials priority (`Focus::mines`) puts it, so Materials First fields a starved side's drones first and Materials Last leaves them waiting. The drone going up shows its construction on the pylon. Drones recover visible wreckage within **800 m of the carrier**, including while it flies (it never waits for them); a drone left outside that reach is called home, and drones fly faster (100 m/s) than the Osprey (65 m/s) so they catch up. They return when there is no work, and wait when mass storage is full. Losses are replaced. Drones depend on their parent carrier and are removed when it is destroyed. The selection shows the recovery radius.

Argus (tech 3) cruises at 450 m, above every other aircraft, at 165 m/s. It has 6,000 m radar, 900 m sonar, 2,000 m sight and a hull shield, and burns hostile missiles with two lasers within 650 m while powered (a sphere: from 450 m up it still covers about 470 m of ground). A salvage ray under its belly pulls 200 mass a second from wrecks up to 1,700 m across the ground from it, the same as a Scavenger III tower, while it flies (`reclaimer.mobile`). A light rocket fails in one tick; heavier missiles take a longer burst, then the laser waits 0.3 seconds before the next. It cannot intercept shells or bombs. Guided AA missiles retain their own target handle; vertical launch stays upright for 0.6 seconds before curving into pursuit. Losing a target leaves a finite-lived unguided missile.

Incendiary bombs spread across consecutive releases and inflict six seconds of burning damage, with persistent flame and smoke. Flak and missile splash use altitude and weapon target masks, so an airburst cannot damage ground units underneath it. Dome and hull shields intercept impacts.

| Tier | Structure | Mobile AA |
| --- | --- | --- |
| 1 | Sparrow: single-barrel rapid AA gun | Gnat: tracked AA gun |
| 2 | Barrage: heavy twin flak, two scattered shells a shot | Squall: conventional flak |
| 3 | Skyguard: long-range SAM, four hatched cells; each missile is boosted up, turns over on thrusters, then lights, and a salvo spreads over the targets in range | |

All three AA structures accept land and water, with floating bases at the water surface; occupancy and cliff restrictions still apply. Mobile AA remains land-based.

Aerie is an open aircraft assembly hangar with a lift, side machining rails, control tower and launch apron. T2 adds the rear equipment house; T3 adds taller assembly gantries. Upgrades reveal only the new modules. Models have three detail levels. Rotor, radar, engine exhaust, barrel elevation, plasma and fire effects are integrated into the renderer.

Dedicated synthesized sounds cover the helicopter rotor, heavy engines, assault engine, plasma minigun, flak and its airburst (`flak_burst`), and incendiary explosions. Missile weapons use rocket launch sounds.

Network and replay versions are **8** because orbit commands and drone, burning and guided-projectile state are serialized and hashed.

## Inspect

```sh
./play.sh --range --unit aster_t3_assault_aircraft --scenario targets --map dev16
./play.sh --range --unit aster_t2_fire_bomber --scenario targets --map dev16
./play.sh --range --unit aster_t3_air_factory --map dev16
cargo test -p mc-sim --test sim -- air_roster::
cargo test -p mc-models complete_air_roster_models_meet_lod_budgets
```

The new mechanics have 14 focused integration tests. Existing flight, combat, factory, crash, fog and network/replay regressions also pass. The broader suite still has unrelated failures for the existing land-factory reduced LOD, shield model height, and shield-upgrade presentation.

## Flight refinements

Gunships translate independently of their heading and circle targets while
facing inward. Wasp and Kestrel chin guns yaw and elevate around their mounts;
unguided rockets wait for the hull to face the target. Kestrel has four animated
thrust-vectoring engines. Wasp retains its tail, rotor and chin gun through its
lower LODs and keeps full detail farther away. Hellkite gains nacelle armor,
wing panels and a sensor spine.

Osprey remains airborne even without orders. Salvage drones cannot be selected
or directly ordered, prefer separate wrecks, and keep circling while reclaiming.
The flock hangs from four pylons under the wing, two a side, each drone's lugs in
its pylon's clamp jaws (`drone_sockets` in `air.ron`, matched by the model's
`PYLONS`). Each drone keeps its own pylon (`drone_socket` in the unit table).
Letting go, a drone drops 3 m clear (`drone_approach`) and flies; coming home it
glides in under its pylon, slowing all the way, and rises onto the clamp
(`air_support::seat_drones`). Docked drones are placed after movement, so they
keep pace with the flying Osprey, and the shader draws them in the Osprey's own
drawn frame (`mirror::UNIT_RIDING`, `entity.wgsl` `riding_frame`), so they ride
its heave, sway and lean. The commander's drone port lands its drones on its
back pads the same way, from above.

## Kestrel and Osprey (2026-09-23 rework)

Both had been built from the shared fuselage helpers with plain white barrels for
engines and no engine effects at all when hovering. Each now has a file of its own
(`models/aster/air/kestrel.rs`, `osprey.rs`, the Salvage Drone in the latter):

- **Kestrel** (reworked 2026-09-28): a two-nacelle tilt-jet. A chisel nose over
  the chin gun steps out into armoured cheeks, a hump sits under a high straight
  wing, and the body pinches into a slim boom ending in an H tail. A faceted jet
  nacelle with a raked inlet turns on a trunnion at each wing tip; like a real
  tiltrotor it needs no tail rotor, turning by tilting one nacelle against the
  other. The jets burn blue. A ball turret under the chin carries one long
  autocannon (`pivot (4.75, 0, 0.56)`, muzzle `(7.3, 0, 0.56)`); a six-tube
  launcher under each wing ripples a 12-rocket volley off in left-right pairs
  (muzzles round `(1.45, ±2.7, 1.7)`).
- **Osprey** (reworked 2026-09-29): a twin-boom tilt-jet. A deep pod fuselage
  with a glazed nose and a chin window, a long straight shoulder wing with a
  blue-burning jet nacelle at each tip, twin booms back to a tailplane and two
  fins, an amber-slotted salvage hopper on the back. Four drone pylons hang under
  the wing, each with a clamp beam, a jaw over each lug and an amber lamp.
- **Salvage Drone** (reworked 2026-09-29): a grapple, a flat faceted wedge with a
  lift fan through its middle (blades on `part::ROTOR`), two claw arms down to
  the reclaim emitter, two lugs on posts for the pylon's jaws, two steering jets
  behind.
- **Engine effects**: a vector-thrust jet leaves a flame cone and a white-hot
  bloom at each nozzle that reach past the pod's rim (a jet pointing straight
  down is otherwise hidden under its pod from the camera), and a thin haze when
  under way; a lift fan throws a wide blue field into its wash. Every hovering
  aircraft raises a downwash on the ground under it (`renderer::air_downwash`):
  a ring of dust driven outward, spray over water, strong at the Osprey's 22 m
  and only a stir at the Kestrel's 65 m. The pods' nozzles carry lamps after dark
  (`lights` in `air.ron`). Hovering aircraft heave gently on their lift
  (`entity.wgsl`). The Osprey and its drones no longer call `set_hover()`, which
  had been raising a hovercraft's ground dust around them in mid-air.
- **Range**: `--scenario salvage` leaves six medium-tank wrecks 48 m east of the
  pad for any reclaimer (builder, tower or carrier), so the flock can be watched
  going out: `--range --unit aster_t2_reclaim_carrier --scenario salvage --ticks
  140 --follow 12 --alpha 0.5 --camera 0,0,110,200`. Hover shots need `--follow`
  of ten or so ticks for the short-lived engine puffs to settle;
  `MERIDIAN_HOUR=23` shows the nozzle lamps.
Idle aircraft reserve landing clearance against other descending or parked
aircraft. An idle aircraft stopped where it cannot set down (water, cliffs,
structures, a pad another aircraft took) flies to the nearest clear ground
within 1.5 km, searched in widening rings nose side first, and lands there;
only the Osprey, its drones and the Magpie (salvage aircraft, whose beams keep working) stay up. Persistent velocity smooths VTOL movement and terrain-following altitude;
that velocity is serialized and hashed.

Bombing return distances include turn radius, fall time and carpet duration.
Map-edge setups retain enough approach distance, including through fog.
Tracking with T follows the aircraft's interpolated altitude.

Exhaust is emitted at every zoom level and has reduced opacity. Particle type
IDs use flat interpolation so contrails cannot become emissive plasma particles.
AA missiles turn more firmly toward their intercept and leave white smoke.
Flak (`flak: true`; the Squall, the Barrage's Heavy Twin Flak, the Behemoth's Heavy Twin Flak and the commander's AA Flak Cannon) is a slow shell (320-380 m/s) on two fuses. The proximity fuse sets it off beside a hull; the timed fuse bursts it where it was laid, the lead on the aircraft (`mc-sim/src/flak.rs`), so a near miss still catches the flight in its 30-48 m splash. In flight it is a small hot round with a thin smoke wake. The burst (renderer/flak_fx.rs) is a quick knot of burning gas (blast_fx `fireball`), the charge burning on for a moment inside a hard-edged black puff (puff kind 39, puffs.wgsl `flak_smoke`) that hangs on the wind for about five seconds, a sphere of hot shrapnel streaks (puff kind 37) flung out to the edge of the splash, and burning scraps falling away trailing thin smoke. The gun's report is a tongue of flame and a ring of powder smoke punched out round the muzzle. The Barrage and the commander fire both barrels on the same tick (`salvo: 2, salvo_batch: 2`) with a few degrees of `spread`, so each shot lays two bursts apart across a flight. Reach (2026-09-28, +1/3 on all flak): Squall 480 m, Barrage and commander 640 m, Behemoth 1,200 m. The Shatter and the Sunder (rail flak) were removed on 2026-09-27.

Thunderhead has 4,700 hull health, wider wings, no rockets, and forward-only
weapons. Its rotary cannon has four-degree spread, bright conventional tracers,
a stronger muzzle flash, and dedicated explosive impact reports. Strafing runs
descend from 95 m cruise toward a 32 m minimum clearance, level across the target,
then climb back to cruise. Vertical acceleration eases the transition, with
terrain look-ahead; the hull, muzzle and exhaust follow the actual flight slope.
The cannon keeps firing below cruise altitude. Its heavier rapid report sits
above a quieter engine bed, with no separate pullout sound. Bomb release is quieter.

## The guard order

The Roost airbase and the logistics network (Gates, Moorages, Junctions) were
removed on 2026-09-25. The Roost model is parked in `models/aster/airbase.rs`, and
no unit uses it.

**Guard** (`Command::Guard`, `OrderKind::Guard`) is for any armed mobile unit, and
for any aircraft, armed or not.
The group holds its spot (`pos + offset`, keeping its spread) and goes after
enemies it can strike that come within `radius` of `pos`. The chase is an
`Attack` pushed in front of the guard, leashed to the area plus a gun's reach.
After it the unit walks back. It only leaves its spot on the Engage stance.
Aircraft fight what is in the area, then circle it at half its radius
(`orbit.rs`); a group flies the circle as a V and re-forms with the formation
settings. Pressed on a friendly unit, the area goes with that unit (`target`),
and stays where it was last if the unit is lost. A guard never ends by itself:
anything queued behind it takes over at once. Factories keep guard as a standing
order. Shift-drag the centre to move a guard area (it then stops following).

Tests: `crates/mc-sim/tests/guard.rs`, `crates/mc-sim/tests/air_guard.rs`.

## Bastion assault transport (2026-09-24)

A 300 m spacecraft that keeps station in the cloud deck (560 m) and comes down to put an
army on the ground. It is not made in a factory: Mason II and III place it like a
structure on a 26 x 10 lot and build it there (2400 mass, 36000 energy, 14400 build
time: twelve minutes for one Mason II). A finished ship stays on its lot with the
ramp down, ready to load.

- **Orders.** Right-click the ship with land units selected: they board (`Command::Board`;
  the pointer turns to a boarding glyph and a note says how much room they take and what
  will not fit). An idle ship up in the sky comes down where it is for them. The order
  card has a Transport column: **Load** (L, click the ground: set down on the nearest
  ground big and flat enough, 6 m of rise under the hull, searched out to 1.6 km, and
  lower the ramp), **Unload** (U, click: the same and let the hold out,
  `Command::Land { unload }`) and, once down, **Take Off** (Shift+L, `Command::TakeOff`). Idle on the ground it stays down with the ramp open; idle in
  the air it stays up; any move raises the ramp and lifts it off.
- **Landing.** Told to set down it glides in rather than stopping overhead and dropping:
  from three cruise heights (1.7 km) out the height it holds eases down a smoothstep to
  12 m over the site, and it slows so it is over the site no sooner than it can come down
  (`lift_speed_cap`), braking to a near stop just over the ground before it settles. All
  its climbs and descents gather speed at an eighth of `descent` per second, top out at
  `descent` and brake to arrive at rest (`lift_vertical`), slower still near the ground.
  Taking off, it raises the ramp, rises straight up for 40 m, then gathers way as it
  climbs, reaching full speed half-way to cruise height.
- **Hold panel.** A status line (In flight, Setting down, Ramp opening, Ready, Unloading
  n left, Ramp closing, Taking off), room taken, and a tile per unit: click one to let
  just that unit out (`Command::Unload`; the ship sets down where it is first), Shift-click
  for every unit of that kind, Ctrl-click to pick it alongside the ship and give it orders
  for when it is off.
- **Hold.** Room 96, 40 m wide and 34 m clear height; a commander takes eight slots; other land units take their size class plus one.
  Units walk round the hull (never in under it from the side) to a point on the centre
  line behind the stern, wait there in file while the ramp is up, then walk straight up
  the centre line over the lip and are stowed at the far end of the hold (they keep their
  row, `IN_FACTORY`, `hangar` naming the ship, as below an airbase). Unloading lets one
  out every 0.4 s when the way is clear; each walks straight down the centre line to
  behind the stern before it goes to its row of four behind the ship, or round the ship's
  side to an order given in transit. The Courier uses the same routes. The renderer lifts what walks on the ramp and hold floor onto the
  deck (`transport::Deck`). What is in the hold dies with the ship.
- **Guns.** Four rotary cannon turrets, one to each side, at `bastion::TURRETS`: nose
  (150, 0, 40), facing ahead, 160 degree cone, 300 m; port and starboard sponsons
  (-40, +-56, 50), facing 90 degrees off the nose, 170 degree cones, 280 m and quicker to
  traverse; stern (-156, 0, 70), facing aft, 160 degree cone, 300 m. `Weapon::facing`
  (degrees, positive to port) sets where a house rests and centres its `arc`; a house
  with a limited arc takes no target outside it (`World::in_arc`; ships excepted, since
  they turn to bring guns to bear). A lift ship's guns measure reach from their own pivot
  (`gun_origin`). Muzzles are authored as the house faces the nose (pivot + 12 m along x);
  the house turns them. They shoot air and ground; `slant: true` measures reach to
  anything not in the air along the line of sight, so from the clouds they cannot touch
  the ground; below about 280 m they start to sweep the landing site. The ship never
  chases anything. On the ground it can be hit by land weapons as well as anti-air
  (`World::target_layers`).
- **Model** (`models/aster/air/bastion.rs`): the underside houses a recessed vehicle ramp (`part::RAMP`,
  hinge x -16 z 34, swung up 0.46365 rad to fold flush with the belly). Its legs, drives, lift jets and gun
  houses come from the shared spacecraft rig (`models/aster/air/capital.rs`): `bastion::RIG` says where they
  are, `models::capital_rig` hands it to the renderer as `ModelInfo::capital`, and `entity.wgsl` animates from
  that alone. The gear stages with the hull's height over the ground (out by touchdown, stowed from 60 m, smooth
  between ticks): flush keel doors swing open on the bay edges, the heavy legs swing down, the struts telescope,
  the pads unfold; on the ground the hull sinks onto its shock struts, rebounds and settles (timed by the ramp's
  deploy), and the hover heave stops. The drives' bells glow with speed, the lift jets with the climb or descent.
  The four rotary cannons (chin beak, flank sponsons, stern boom) spin while firing and do not kick; their
  posts are slim and their decks sit 6.5 m under the pivot so the barrels clear the hull at the pitch limit.
  Lamps (`bastion::LAMPS`, `models::capital_lamps`) sit in fittings. Strategic icon: `Transport`.
- **Atmosphere.** Cruises at 560 m and 78 m/s, accelerates at 9 m/s squared, turns
  at 12 degrees/s and climbs/descends at 40 m/s. Flight slope, acceleration trim
  and damped banking feed the hull and its exhaust. It levels for touchdown.
  Capital hulls and their selection never clear or deform clouds. Moisture-gated
  wisps drift behind a moving ship for 5.5 seconds. Its vectoring nozzles swing on
  their gimbals into a turn and open their slotted petals with thrust, and each plume
  is one tube out of the whole mouth that bends back through a turn: long and bright
  under throttle, a short glow hovering. Landing, it presses flat the trees under its
  hull and its wash bends those round it.
  The engine loop is a slow sub-bass reactor chord with no blade beat.
- **Fit.** Boarding checks cargo room, hull diameter against the door width, and unit
  height against the clear hangar height. Every current T3 land unit fits, including Paladin.

Inspect:

```sh
./play.sh --range --unit aster_t2_lift_ship --scenario lift
cargo test -p mc-sim --test sim -- lift_ship::
cargo test -p mc-models --lib the_lift_ship
```

Not done: a landed ship does not block pathing (boarding and unloading units route round
it themselves, but other traffic still walks under it); the AI neither builds nor uses it.

## Courier light transport

The T1 Courier (`aster_t1_lift_ship`) is built on a 10 x 6 lot by the commander or T1-T3 engineers. It carries
8 weighted cargo slots (one commander or eight T1 light tanks), cruises at 150 m/s at 140 m altitude,
accelerates at 42 m/s squared, and climbs/descends at 60 m/s. It has 400 health,
no weapons, no shield, and costs 160 mass / 2400 energy / 800 build time.

It is built in the Bastion's language at a third of the size (`models/aster/air/courier.rs`):
a dark gunmetal hull of two chined shoulders with pale armour brows (hatch runs, team
stripes), a service belt with a lit amber rail down each flank, landing skids, a radiator
terrace on each shoulder, a wedge prow with split jaws, a dark cockpit hood with a raked
window slit and a chin sensor keel, a dorsal spine with a lit radiator bank, a glazed
sensor house and a mast with a turning radar bar, and two drive nacelles carrying the
shared capital drives (`capital::drive` at 0.55: finned can, gimbal collar, a
vectoring nozzle of slotted petals round a glowing throat) either side of a hazard-striped door portal. Four lift jets (under the
nacelles and the prow's chin) and the lamp fittings (floods, nav lights, strobes, door
beacons, hold lamp) are the `LIFT_JETS` / `LAMPS` tables the renderer's capital effects
read; `RIG` is its `CapitalRig` (no legs, no ramp). Its sounds are `data/sounds/courier.ron`
(`aster_courier` hum, the capital set made smaller, and door slides for `ramp_*`).
Two plug doors slide into the shoulders in 0.9 s after landing. Units walk into a
28 m wide, 26 m tall ground-level tunnel and are stowed deep inside the hull.
There is no landing ramp. Unloading walks each unit straight through the stern
before it spreads out or resumes orders issued in transit. Right-click to board;
L lands and U lands/unloads, with one unit released every 0.25 s when clear.
Cargo uses the same weighted capacity, fit checks and loss-on-destruction as Bastion.

Preview: `./play.sh --range --unit aster_t1_lift_ship --scenario lift`.
Close-ups (GPU, doors open, loading/unloading frames):
`cargo run --release -p mc-render --example courier_shots -- maps/dev16.mcmap artifacts/courier`.

## Hover flight (2026-09-28)

Hover aircraft fly rather than slide (`mc-sim/src/hover_flight.rs`): they make
full speed only ahead and drift sideways or astern at 3/10 of it, so they turn
their nose to go anywhere fast, and the hull leans with its lift. The nose drops
to speed up and to hold a cruise and comes up to brake; the hull banks into a
turn or a drift. Roll is `Units::bank`, pitch is slot 1 of `Units::arm_pitch`,
both eased each tick, and muzzles are placed on the leaned hull.

A VTOL's pods are declared on the model (`MeshBuilder::set_vtol`, `Model::vtol`,
`ModelInfo::vtol`): pivots, nozzle and jets or fans. The pods tilt with the lean
(`models::vtol_tilt`, and `vtol_tilt` in `entity.wgsl` over the `VTOL_*`
constants): forward to drive the aircraft on, back past upright to brake, the
outer pair forward and the inner pair back in a turn. Jet pods burn a blue drive
plume (`PUFF_THRUST`) out of each nozzle, longer and hotter the harder the
aircraft is driven (`renderer/aircraft_trails.rs`). A VTOL may have one pair of
pods or two (`Vtol::pairs`).

## Warp drives and the Undertow dampener (2026-09-29)

The Courier, the Bastion, the Resolute and the Dominion carry a warp drive (`warp:` in their unit
entries; any aircraft may be given one). `Command::Warp` (`crates/mc-sim/src/warp.rs`):

1. **Charge.** The ship waits until its drive has recharged and it is up at 3/4 of its
   cruise height. Then it stops, turns its nose onto the mark and charges the drive. The
   charge is `energy`, drawn off the grid over `spool` seconds at full power and paid
   with the side's upkeep (`economy.rs`). A grid that cannot pay charges it more slowly
   (never the last sliver), and a big charge can stall the grid, dropping its shields.
   The drive charges only two thirds while the nose is still coming round; the last
   third charges once it is on the mark, so a ship always charges a moment after it
   lines up (the card reads `Charging warp · coming onto the mark` until then).
   Any other order, or a stun, calls the jump off, and the charge is lost.
2. **Jump.** Once fully charged and on the mark, the ship leaves: it is out of the
   world (`IN_FACTORY`: not seen, hit or ordered) and already where it will come out.
   The transit lasts `distance / speed`, and never less than 1.5 s. A jump reaches
   anywhere on the map. Ships sent together keep their formation: each comes out as far
   and as the same way from the mark as it stood from the middle of the group.
3. **Exit.** Back in the world, it holds still while the drive winds down (1.2 s), then
   recharges for `cooldown` seconds. The order was done when it jumped, so the next one
   is taken up as it comes out.

| ship     | charge          | cooldown | speed      |
|----------|-----------------|----------|------------|
| Vigil    | 1 200 E in 3 s  | 40 s     | 3 000 m/s  |
| Courier  | 1 500 E in 3 s  | 40 s     | 3 000 m/s  |
| Bastion  | 8 000 E in 4 s  | 60 s     | 3 500 m/s  |
| Resolute | 20 000 E in 5 s | 75 s     | 4 000 m/s  |
| Dominion | 45 000 E in 7 s | 110 s    | 4 000 m/s  |

**Undertow** (`aster_t2_warp_damper`, T2, 300 E/s upkeep, `warp_damper:`): an enemy
jump that ends within 1 600 m of a finished, powered Undertow is snagged. The transit
drags on 3x longer (a field raised mid-jump snags what is left of it), and if the
Undertow still stands and has power when the ship comes out, the ship loses a quarter
of its health and is stunned for 25 s. Destroying or starving it first spares the
ship. Pausing it powers the field down, as with shields. Its own side's jumps pass.
It is a trap: its enemies are never shown its field, and to the side whose ship it
snags the jump looks, sounds and counts down as a clean one until the ship comes out
hurt and stunned (`mirror/warp.rs`, `hides_damping`). Everyone else sees it torn.

**EMP stun** (`Units::stun`, `World::stun`): a stunned unit takes no orders (they wait
for it), picks no targets, fires nothing, does not move and its shield is off. A stunned
capital ship heels 14° over, dips its nose 4°, drifts to a stop and sinks toward 85% of
its cruise height, righting itself once the stun wears off.

Presentation: `UnitInstance::fx` carries the warp stretch and the stun, `status[0]`
`UNIT_WARP_DAMPED` / `UNIT_IN_WARP`, and `RenderFrame::warps` / `dampers` list the jumps
and fields a viewer may see (`mirror/warp.rs`). A ship is drawn for one more tick where
it left, stretching into a streak, and comes out of a streak on its first tick back.

Check: `cargo test --profile gate -p mc-sim --test sim -- warp::` (and the determinism
matrix, which plays a Courier's jump into an Undertow). Headless:
`scripts/shot.sh run --range --unit aster_t1_lift_ship --scenario warp` or
`--scenario warp-dampened` (a red Undertow beside the exit).

## Warp: the interface (2026-09-29)

The Courier, Bastion, Resolute and Dominion carry warp drives (`UnitBlueprint::warp`, `mc_sim::warp`).
What the player sees of them:

- **Order card.** A **Warp** button (key **O**) in the Movement column of any selection that
  holds a ship with a drive. Click a point: the ships charge and jump there in the formation
  they stand in; shift queues. The pointer is the warp pointer (an exit ring with streaks
  running into it) wherever a click would send the jump.
- **With the order in hand** (`warp_marks.rs`): a line from each ship to where it comes out
  (its place in the formation about the mark, kept on the map, as the sim does), and a
  ghost ring there. Enemy Undertow fields are never drawn. Next to the pointer a card totals the energy the jumps take
  (`Warp 9,500 E`) with a bar of the store against it, the store itself (warning-coloured,
  with how much it falls short, when it cannot cover the charge: the jump still goes, the
  charge just runs slower), and how long the charge takes at full power.
- **In the world.** A charge bar over a ship while it spools (`Charging warp 64%`), a pulsing
  ring and the seconds left where one of ours will come out while it is in warp, and over a
  stunned unit an electric bolt and **STUNNED 18 s** (the seconds only for our own).
- **Unit card.** The activity line reads `Charging warp 64%` with a bar, `In warp 3 s`,
  `Leaving warp` (or `Thrown out of warp`), or `Stunned · systems down 18 s` in electric cyan.
  Under it the drive's line: `Warp drive recharging 32 s` with a bar, or `Warp drive ready ·
  O` with its charge. Our own Undertow's card says `Warp field up 1,600 m`, or `Warp field
  down · powered down` / `· no power`.
- **The Undertow** has an icon of its own (`IconKind::Damper`: a ring broken on the
  diagonals, four chevrons pulling in on its middle), and selected or being placed its field
  is a magenta range ring (`Reach::Damper`, WARP FIELD in the key).
- **A jump of ours an enemy Undertow snags** shows nothing of it: no red, no **Dampened**,
  the countdown of a clean transit (it sits at 0 s while the drag holds it).

Shots: `MERIDIAN_AIM=warp scripts/shot.sh run --range --unit aster_t1_lift_ship --select
lift_ship --cursor X,Y ...` aims the order; `--scenario warp` / `warp-dampened` jumps.

## Dominion dreadnought (2026-09-29, reworked 2026-09-30)

`aster_t4_dreadnought`, mesh `space_dreadnought`, T4, about 570 m, built on a lot by
tech 3 engineers (the commander's Engineering Suite III, Mason III). The Resolute's
rules hold (capital ship, lands only when told, never moves unless ordered); on top it
fights broadside: engaged and stopped it lays its beam on the mark (`motion.broadside:
90`, the Leviathan's rule, which any layer may use), since no gun of weight bears dead
ahead.

- **Arc Cannon Casemates** (0..=5, three a side, port first, fore to aft): the
  Leviathan's charged shells, laid direct (a `Ballistic` gun on an aircraft is a bomb
  bay to the sim), 2 500 m, each bearing from 10 degrees off the nose round its own
  beam to 10 off the stern. Their reach stays under the Zenith's 3 200
  (`tests/dreadnought.rs` checks it).
- **Twin Bolt Rifles** (6, 7): houses on the stacked hull, one forward facing ahead,
  one aft facing astern, 1 600 m.
- **Long Range SAM** (8): two hatched blocks of 8 as one launcher, 3 000 m.
- **Hull field**: 60 000, regen 400; it draws the ship's 800 E/s upkeep and drops on
  a stall like every shield.

Nine weapons: `mc_data::MAX_WEAPONS` is 10 for it. The model (`mc-models
aster/air/dominion/`) is an aft block, a pinched waist with a lit hangar recess and a
forward block, a ventral hull, a terraced prow, and "ARC" and "DOMINION" painted on the
walls (`lettering.rs`, flat stencil glyphs).

## The Regency's air force: tech 1 (2026-10-02)

The Regency build their own aircraft (`data/factions/regency/units/air.ron`, models
`crates/mc-models/src/regency/air/`). Their jets are drones in the Cybertronian-jet manner:
a nose blade, down-turned fins, dark plates lapped back over bronze workings, red optics and
red heat in the exhausts. What hovers hangs on lift bells with red plasma under them: no
rotors, no jet plumes. Each is anchored on ARC's unit of the same job.

| Unit (key) | Job | ARC counterpart | How it differs |
| --- | --- | --- | --- |
| Flechette (`regency_t1_air_scout`) | Air scout, unarmed | Swift | The fastest thing in the air (270 m/s), a wider turn, thinner skin |
| Quarrel (`regency_t1_fighter`) | Fighter: Twin Plasmeric Repeater, aircraft only | Shrike | About the same damage a second in heavier bolts, a little less reach; crescent wing |
| Petard (`regency_t1_bomber`) | Light bomber: a stick of three Plasmeric Bombs | Petrel | 270 a pass in three lighter bombs strung along the run; a cleaver of a flying wing |
| Coffer (`regency_t1_transport`) | Light transport, eight slots, raised on a 7x5 lot by the Exarch and Artificers | Courier | No warp drive, a little quicker; an open-sterned box hold between two sponsons on six lift bells |
| Sickle (`regency_t1_air_reclaimer`) | Salvage drone: one nanite head under it, works wrecks as it flies | Magpie | Takes apart faster (power 5 to 4) at a shorter reach (380 m to 450 m); a pod with two hooked blades on three lift bells |

The Skyforge (all three tiers) makes the Quarrel, Petard, Flechette and Sickle; there is no
tech 1 gunship. The Sickle's stream is drawn as the Regency's nanite strands, as the
Breaker's is. The Commander AI fields them by role like any other unit
(`ai::commander::profile` test `the_regency_builds_its_own_air_force`). Sounds are still
ARC stand-ins (`aster_jet`, `aster_hover`).

## The Regency's gunships: Quiver and Reaper (2026-10-02)

The Regency's hovering combat craft (`data/factions/regency/units/air_gunships.ron`,
models `crates/mc-models/src/regency/gunships/`) take the Kestrel's and Thunderhead's
places at the Skyforge. Neither has rotors or jets: each floats on gravity lift bells, red
plasma crackling under them.

- **Quiver** (`regency_t2_drone_carrier`, tech 2): a manta-shaped hull with six bays down
  its back, a **Wick** (`regency_wick`) in each. Its one weapon `launches` the Wicks: on a
  mark in reach it lets the first Wick home in its bay go, a salvo of six 0.35 s apart every
  9 s. Each flies at the mark, diving onto it over its last ~200 m, and bursts there: the
  burst is a shot of the Quiver's weapon fired from the Wick over its last metres, so it
  hits, splashes and meets shields like any shot (mc-sim `strike_drones.rs`). A Wick whose
  mark dies first takes the nearest enemy within 160 m or comes home unspent. The bays are
  the magazine: with none home the launcher waits, and the racked Wicks on the hull show
  it. Spent Wicks are built again in their bays, one at a time (2 mass, 20 energy, 1.5 s
  each), as the Osprey's drones are (`air_support.rs`). Wicks are aircraft: flak and
  fighters shoot them down on the way in, and they die with their carrier.
- **Reaper** (`regency_t3_assault_aircraft`, tech 3): a scythe-winged craft on six bells
  that does not strafe. It `hangs`: it flies in to two fifths of its reach from its mark
  and holds still there, moving again only when the mark drifts out of that band
  (`orders.rs` `air_hang`). Its Pinch-fusion Beam (a `beam`, like the Harrow's) `walk`s
  14 m either side of the mark across the line of fire, back and forth every 4 s, so the
  stream glasses a swath and splashes everything standing in it.

Check: `cargo test -p mc-sim --test sim -- regency_gunships::`; both are in the
determinism match.

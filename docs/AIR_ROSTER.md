# Air roster and anti-air defenses

All units are available through the existing tiered factories and engineer/commander build menus. The existing Petrel light bomber remains available. Shrike now displays the Fighter role; its internal key remains `aster_t1_interceptor`.

| Tier | Aircraft | Role |
| --- | --- | --- |
| 1 | Swift | Fast, fragile scout with radar; circles on guard |
| 1 | Shrike | Fighter |
| 1 | Wasp | Low-altitude helicopter, light machine gun and unguided rockets |
| 1 | Osprey | Cheap reclaim carrier with four salvage drones |
| 2 | Argus | Radar, sonar, missile interception and a 1,200 m salvage ray; guards a point or friendly unit |
| 2 | Kestrel | Four-engine tilt-jet gunship with a chin autocannon and volley rocket pods |
| 2 | Hellkite | Four-engine flying fortress; 24 scattered incendiaries and three independent AA guns |
| 2 | Peregrine | Fast guided-missile interceptor |
| 1 | Courier | Fast unarmed transport; eight cargo slots; built on site |
| 2 | Bastion | Capital assault transport; carries the T1-T3 land roster; built on site |
| 3 | Raptor | Fast, highly maneuverable air-superiority fighter |
| 3 | Eclipse | Fast strategic bomber with a large blast |
| 3 | Thunderhead | Armored, shielded assault aircraft; forward rotary cannon and forward AA |

Aircraft circle on the **Guard** order (Ctrl+G; see [The guard order](#the-guard-order)): press on a point or a friendly unit and drag out the area. They fly halfway between its centre and its edge, and follow the ally if one was picked. Shift queues a guard; Stop cancels it. If the ally is destroyed, the aircraft keep circling its last position. Move and attack commands replace a guard normally. (The separate Orbit order and its **O** key were merged into Guard on 2026-09-26.)

Osprey (tech 1, from the first air factory) builds up to four salvage drones, one at a time, each on its own pylon under the wing: a drone costs 3 mass and 30 energy over 3 seconds, paid like any build, in the tier the side's materials priority (`Focus::mines`) puts it, so Materials First fields a starved side's drones first and Materials Last leaves them waiting. The drone going up shows its construction on the pylon. Drones recover visible wreckage within **600 m of the carrier**, return when there is no work, and wait when mass storage is full. Losses are replaced. Drones depend on their parent carrier and are removed when it is destroyed. The selection shows the recovery radius.

Argus has 2,800 m radar, 900 m sonar, 1,200 m sight and a small hull shield, and burns hostile missiles with two lasers within 450 m while powered. A salvage ray under its nose clears wrecks up to 1,200 m across the ground from it while it flies (`reclaimer.mobile`). A light rocket fails in one tick; heavier missiles take a longer burst, then the laser waits 0.3 seconds before the next. It cannot intercept shells or bombs. Guided AA missiles retain their own target handle; vertical launch stays upright for 0.6 seconds before curving into pursuit. Losing a target leaves a finite-lived unguided missile.

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
  going out: `--range --unit aster_t1_reclaim_carrier --scenario salvage --ticks
  140 --follow 12 --alpha 0.5 --camera 0,0,110,200`. Hover shots need `--follow`
  of ten or so ticks for the short-lived engine puffs to settle;
  `MERIDIAN_HOUR=23` shows the nozzle lamps.
Idle aircraft reserve landing clearance against other descending or parked
aircraft. An idle aircraft stopped where it cannot set down (water, cliffs,
structures, a pad another aircraft took) flies to the nearest clear ground
within 1.5 km, searched in widening rings nose side first, and lands there;
only the Osprey and its drones stay up. Persistent velocity smooths VTOL movement and terrain-following altitude;
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
  card has a Transport column: **Land** (L, click the ground: set down on the nearest
  ground big and flat enough, 6 m of rise under the hull, searched out to 1.6 km),
  **Unload** (U, click: the same and let the hold out, `Command::Land { unload }`),
  **Unload Here** (Shift+U) and **Take Off** (Shift+L, `Command::TakeOff`; while aloft the
  button is **Land Here**). Idle on the ground it stays down with the ramp open; idle in
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
  wisps drift behind a moving ship for 5.5 seconds. Animated magnetic iris vanes,
  pulsing ion cores and directed blue-white plumes make its propulsion visible.
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
shared capital drives (`capital::drive` at 0.55: finned can, gimbal, glow-lined bell,
turning iris) either side of a hazard-striped door portal. Four lift jets (under the
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

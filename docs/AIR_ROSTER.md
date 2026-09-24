# Air roster and anti-air defenses

All units are available through the existing tiered factories and engineer/commander build menus. The existing Petrel light bomber remains available. Shrike now displays the Fighter role; its internal key remains `aster_t1_interceptor`.

| Tier | Aircraft | Role |
| --- | --- | --- |
| 1 | Swift | Fast, fragile scout with radar and orbit |
| 1 | Shrike | Fighter |
| 1 | Wasp | Low-altitude helicopter, light machine gun and unguided rockets |
| 2 | Argus | Radar and missile interception; orbit a point or friendly unit |
| 2 | Osprey | Hull-shielded reclaim carrier |
| 2 | Kestrel | Four-engine vector-thrust gunship with cannon and rockets |
| 2 | Hellkite | Four-engine flying fortress; 24 scattered incendiaries and three independent AA guns |
| 2 | Peregrine | Fast guided-missile interceptor |
| 1 | Courier | Fast unarmed transport; eight cargo slots; built on site |
| 2 | Bastion | Capital assault transport; carries the T1-T3 land roster; built on site |
| 3 | Raptor | Fast, highly maneuverable air-superiority fighter |
| 3 | Eclipse | Fast strategic bomber with a large blast |
| 3 | Thunderhead | Armored, shielded assault aircraft; forward rotary cannon and forward AA |

Select Swift, Argus or Osprey and press **O**, or choose **ORBIT**, then click a point or friendly unit. The unit circles that position or follows that ally. Shift queues an orbit; Stop cancels it. If the ally is destroyed, the aircraft keeps circling its last position. Move and attack commands replace orbit normally.

Osprey automatically pays for and assembles up to four salvage drones. Each costs 12 mass and 288 energy and takes 4.8 seconds at full resources. Drones recover visible wreckage within **420 m of the carrier**, return when there is no work, and wait when mass storage is full. Losses are replaced. Drones depend on their parent carrier and are removed when it is destroyed. The selection shows the recovery radius.

Argus has 2,800 m radar and burns hostile missiles with a laser within 300 m while powered. A light rocket fails in one tick; heavier missiles take a longer burst, then the laser waits 0.3 seconds before the next. It cannot intercept shells or bombs. Guided AA missiles retain their own target handle; vertical launch stays upright for 0.6 seconds before curving into pursuit. Losing a target leaves a finite-lived unguided missile.

Incendiary bombs spread across consecutive releases and inflict six seconds of burning damage, with persistent flame and smoke. Flak and missile splash use altitude and weapon target masks, so an airburst cannot damage ground units underneath it. Dome and hull shields intercept impacts.

| Tier | Structure | Mobile AA |
| --- | --- | --- |
| 1 | Sparrow: single-barrel rapid AA gun | Gnat: tracked AA gun |
| 2 | Tempest: 16-missile vertical volley | Squall: conventional flak |
| 3 | Skyguard: slow, long-range high-damage SAM | Sunder: weaker plasma shatter gun |
| 3 | Shatter: advanced plasma flak | |

All four AA structures accept land and water, with floating bases at the water surface; occupancy and cliff restrictions still apply. Mobile AA remains land-based.

Aerie is an open aircraft assembly hangar with a lift, side machining rails, control tower and launch apron. T2 adds the rear equipment house; T3 adds taller assembly gantries. Upgrades reveal only the new modules. Models have three detail levels. Rotor, radar, engine exhaust, barrel elevation, plasma and fire effects are integrated into the renderer.

Dedicated synthesized sounds cover the helicopter rotor, heavy engines, assault engine, plasma minigun, shatter discharge and impact, conventional flak and incendiary explosions. Missile weapons use rocket launch sounds.

Network and replay versions are **8** because orbit commands and drone, burning and guided-projectile state are serialized and hashed.

## Inspect

```sh
./play.sh --range --unit aster_t3_assault_aircraft --scenario targets --map dev16
./play.sh --range --unit aster_t2_fire_bomber --scenario targets --map dev16
./play.sh --range --unit aster_t3_air_factory --map dev16
cargo test -p mc-sim --test air_roster
cargo test -p mc-render complete_air_roster_models_meet_lod_budgets
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
The flock stows in a hold under the Osprey's midbody, two abreast and two deep
(`air_support::drone_socket`, matched by the model's `CRADLES`); opening the hold
swings its two door leaves down and lowers the cradles 1.9 m, so the drones drop
out under the hull before they fly.

## Kestrel and Osprey (2026-09-23 rework)

Both had been built from the shared fuselage helpers with plain white barrels for
engines and no engine effects at all when hovering. Each now has a file of its own
(`models/aster/air/kestrel.rs`, `osprey.rs`, the Salvage Drone in the latter):

- **Kestrel**: hard-chined white hull over a graphite belly with a dark chisel
  nose, a generator hump down the back with lit louvres, a sponson each side fore
  and aft carrying a faceted engine pod on its tip. The pods stand up to hover and
  lie down to cruise (about `kestrel::NACELLES`, mirrored in `entity.wgsl` and
  `renderer::aircraft_trails`); a turbine face spins in each intake and the nozzle
  ring is hot. The twin assault cannon hangs in a yawing cradle under the chin
  (`pivot (3.4, 0, 0.95)`, muzzles `(6.2, ±0.38, 0.95)`); a seven-tube rocket pod
  hangs under each forward sponson (muzzles `(2.5, ±3.1, 0.75)`).
- **Osprey**: a broad lifting body, white shoulders with a graphite salvage deck
  saddling the roof over the hold, four big ducted lift fans in pods on stub
  wings (`osprey::NACELLES`), their five-blade fans turning in the intakes, which
  face the sky in the hover so the camera looks down into them. Amber running
  strips and hold louvres mark it as economy.
- **Salvage Drone**: a ring lift fan (its blades on `part::ROTOR`) with a claw
  arm ahead carrying the reclaim emitter, two steering jets behind.
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
only the Osprey and its drones stay up. Persistent velocity smooths VTOL movement and terrain-following altitude;
that velocity is serialized and hashed.

Bombing return distances include turn radius, fall time and carpet duration.
Map-edge setups retain enough approach distance, including through fog.
Tracking with T follows the aircraft's interpolated altitude.

Exhaust is emitted at every zoom level and has reduced opacity. Particle type
IDs use flat interpolation so contrails cannot become emissive plasma particles.
AA missiles turn more firmly toward their intercept and leave white smoke.
Shatter beams airburst visibly short of the target. Forward-moving plasma fragments fill the target area and detonate individually across the weapon's splash radius (60 m for the T3 emplacement, 42 m for Sunder). The beam is a brief, dim filament so the larger ragged airburst and fragment explosions dominate. Each fragment explosion carries a blue expanding pressure front around its ragged plasma core. The T3 emplacement has a continuous bearing and trunnion-supported black breech over a braced service casemate, with a 1.35 m firing recoil stroke. The firing sound layers a sharp electrical crack, midrange cannon bark and breech clack over restrained bass; delayed impact cracks follow the fragment arrivals.

Thunderhead has 4,700 hull health, wider wings, no rockets, and forward-only
weapons. Its rotary cannon has four-degree spread, bright conventional tracers,
a stronger muzzle flash, and dedicated explosive impact reports. Strafing runs
descend from 95 m cruise toward a 32 m minimum clearance, level across the target,
then climb back to cruise. Vertical acceleration eases the transition, with
terrain look-ahead; the hull, muzzle and exhaust follow the actual flight slope.
The cannon keeps firing below cruise altitude. Its heavier rapid report sits
above a quieter engine bed, with no separate pullout sound. Bomb release is quieter.

Sunder uses a low black chassis, separate white track skirts, rear power packs,
and a compact shatter turret with a supported elevation saddle and 0.85 m recoil.
Its mobile shot emits 14 fragments versus the emplacement's 24. Airburst and
fragment explosions are about 56% of the emplacement's size, with a thinner
beam, shorter fragment streaks and one smaller pressure wave at the split;
secondary detonations do not spawn overlapping pressure fronts. Its 26 m splash
radius, range and firing cadence are unchanged. Damage is 110.

Both shatter guns lead the airburst and each fragment detonation using the
struck aircraft's measured motion, including sideways travel and climbs.
Prediction includes each fragment's 0.16–0.33 s flight and the renderer's tick
interpolation. Ground and shield impacts remain at their interception point.

Shatter's emplacement deals 300 damage per burst; Sunder deals 175. Both now
spread fragments through a deeper, wider volume and give every fragment explosion
a blue shockwave. Direct AA mounts track through vertical elevation and wait for
the barrel to align before firing. The initial Shatter beam follows the fired
barrel axis; fragment trajectories alone lead a moving aircraft.

## Airbases and the guard order

**Roost / Roost II / Roost III** (`aster_t1_airbase` → `_t2` → `_t3`) is an
underground airbase on a 6×6 lot. Each upgrade happens in place: the same
building, the same stored aircraft, and a guard that covered the old reach
grows with it.

| Tier | Berths | Tunnels | Reach | Mends | Launch every |
| --- | --- | --- | --- | --- | --- |
| Roost | 6 | 2 | 1200 m | 3%/s | 1.6 s |
| Roost II | 10 | 4 | 1800 m | 4.5%/s | 1.2 s |
| Roost III | 16 | 4 | 2600 m | 7%/s | 0.8 s |

- **Landing.** Right-click the Roost with aircraft selected (`Command::Dock`).
  Four thick hatch leaves telescope apart (the outer pair comes to rest on the
  deck's arms) while aircraft come in. Arrivals
  are laid out as a cluster: as many as fit side by side in the square shaft (a
  3 m hex grid, nearest the middle first) go down together, and the rest hold in rings over the base
  until there is room. They sink all the way to the lift 21 m down, and are seen
  through the hatch on the way (`mirror::UNIT_IN_SHAFT`, squeezed in depth like
  the model's pit). Stored, they are `IN_FACTORY` with `Units::hangar` naming the base.
- **Mending.** Stored aircraft heal for free (table above).
- **The roster.** Selecting a Roost shows every aircraft below as its own tile in
  the hangar panel, with empty berths after them. Click a tile to pick that
  aircraft (the base stays selected so the roster stays up); PICK ALL picks them
  all; right-click a tile launches just that one. Orders given to picked aircraft
  go to them, not the base: stored aircraft take orders (`owned` includes them),
  are called out (`SORTIE_ORDERED`), and go through a tunnel to carry them out.
  They are listed only for their own side (`mirror::UNIT_STORED`) and never drawn.
- **Reach and auto-land.** An idle aircraft that needs to land goes to the
  nearest Roost of its side that has room, whose reach it is in, and that has
  **AUTO-LAND** on (the chip in the hangar panel, `Command::SetAutoLand`, on by
  default). Aircraft just fired out stay out for 30 s first.
- **Launching.** Use LAUNCH, or click a type in the hangar panel (right-click
  launches one). Each aircraft is put at the back of a tunnel and runs 12 m down
  it, speeding up, then leaves the mouth at the tier's launch speed. That is one
  aircraft per tunnel each launch interval. Called-out aircraft wait 330 m out.
- **Guarding.** A new Roost guards its whole reach (`Units::guard`, a setting,
  not an order, so upgrades still queue). GUARD (Ctrl+G) sets a centre and size
  inside the reach; right-clicking the ground moves it; Stop ends it. Intruders
  get every stored aircraft that can strike them (at a third of health or more)
  fired out with a guard order carrying the base. These aircraft fight in the
  area, come home once it is clear, and go home early below a quarter of health.
- **Loss.** When the Roost is destroyed, what it holds dies with it.

**Guard** (`Command::Guard`, `OrderKind::Guard`) is for any armed mobile unit.
The group holds its spot (`pos + offset`, keeping its spread) and goes after
enemies it can strike that come within `radius` of `pos`. The chase is an
`Attack` pushed in front of the guard, leashed to the area plus a gun's reach.
After it the unit walks back. It only leaves its spot on the Engage stance.
Aircraft without a base circle (fixed wing) or hover over the spot. Factories
keep guard as a standing order. Shift-drag the centre to move a guard area.

Sounds are in `data/sounds/airbase.ron` (hatch open/close, aircraft stored,
tunnel launch). Headless: `--scene airbase --no-fog` (dev16); `--scene airbase-hangar` is the same without the intruder, so the wing stays below (`--ticks 400 --select aster_t2_airbase` shows the roster). `--ticks 90 --follow 1 --alpha 0.5
--camera 12380,12368,190,25` shows a cluster going down the shaft;
`--ticks 109 --follow 1 --alpha 0.5 --camera 12398,12398,70,225` shows a Wasp
racing out of the north-east tunnel. Tests: `crates/mc-sim/tests/airbase.rs`.
Network/replay versions are **12**.

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
cargo test -p mc-sim --test lift_ship
cargo test -p mc-render --lib the_lift_ship
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

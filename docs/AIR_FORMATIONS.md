# Aircraft movement and formations

Select mobile units and open **FORMATION** (or press **G**) in the Orders card.
The panel sets **TOGETHER** / **FREE MOVE** and **COMPACT** / **STANDARD** / **WIDE**
spacing for subsequent move/attack-move orders. **FORM UP HERE** immediately
gathers the selection around its current center. Selected orders draw individual
destination rings and squares at their current moving slots.

Together orders have a serialized group identity, anchor, heading, convergence
phase, and speed. Units move immediately and converge on moving slots in repeating
five-aircraft Vs or a ground block. Land, amphibious, and hover units can share a
block; naval units and separate air altitude bands get separate groups. The group
travels below its slowest member's maximum speed, leaving room for correction,
and slows for stragglers. Open-ground travel uses a smooth heading rather than
flow-grid staircase headings.

Ground members steer to their moving slots whenever the swept path is clear.
When an obstacle or narrow passage prevents the layout from fitting, existing
per-hull navigation carries units through, and they regroup on open ground.
Mobile contact resolution uses proposed positions and applies across ground
locomotion types: amphibious commanders are solid to tanks. Aircraft do not
participate in crowd shoves or hull-contact resolution, even at the same altitude.
Their formation slots provide spacing without collision-induced bouncing.

Queued legs and dragged destinations retain independent group identities.
Completion keeps the assigned pose. Combat temporarily releases air participants;
attack-move survivors return to their group's slots.

Idle armed aircraft automatically pursue compatible detected enemies and return to
their original position afterward, landing when the ground is clear. Weapon matching uses physical target categories,
so an Air factory's production category does not make it a fighter target.
Explicit attack orders and the UI use the same target compatibility check.

Aircraft cruise along their heading with damped steering and bank into turns.
Only the final 24 metres at most one-third cruise speed use precise hover
docking. Transit formation slots do not pull aircraft sideways at flight speed.
Climbs pitch the model; air models skip ground-normal leaning.
Fighters use bounded velocity prediction, throttle-dependent turn radius, and
brief extensions when sustained pursuit cannot produce a firing line.
Bombers correct their inbound line, commit through the target, then make one
continuous turn directly into the next pass.

A bombing pass has two halves, latched in `air_turn_ticks`. Lining up, the
aircraft steers through where the target will be when the bombs land (a bomb
keeps the aircraft's ground speed, so that is simply the flight time to the
point); flying at a crossing target's present position left the release line
beside it and the bay shut. Inside the release distance, or when the target is
inside the (widened) turning circle with no straight leg left, it stops steering
and flies the line out rather than winding round the target. The outbound leg is
measured from where the target is now, so one driving the same way does not end
up too close behind to line up on again. Short of room near a map edge, the
aircraft breaks off to the nearest point that still has a full approach from
inside the map, which beside an edge means running along it; the leg ends as
soon as there is room, and the turn in goes round the open side.

The bomb sight opens the bay on the tick nearest the release line, from the
unrounded fall time and half the rack's real duration, so the middle of the
carpet lands on the target (it was about two ticks of flight short).

Fixed-wing formation members aim two turn radii ahead of themselves, not half a
second: a wing still rolling in when its aim point crosses the nose weaves all
the way. A flight whose members are mostly far from their slots (coming off
attack runs) re-seats its anchor on the aircraft and forms up on the way.
Orders run before movement in the tick, where `prev_pos == pos`: target velocity
there comes from `air_velocity` (last tick's displacement, kept for every hull).
Regressions are in `crates/mc-sim/tests/air_bombing.rs`.

Height stays in the unit blueprint's motion.altitude, above the local land/water
surface. Both T1 interceptors and bombers cruise at 200 m, above the 180 m heavy dome. Future fighter tiers
and recon can each specify their own altitude. Aircraft can overlap in plan view
without changing one another's flight path.
Idle aircraft land on clear dry ground and lift clear before departing. Water,
steep terrain and occupied sites keep them aloft.
This does not introduce additional aircraft blueprints.

## Try it

From the repository root:

    cargo run -p mc-game -- --scene formations --map dev16

Ten fighters, five bombers, and nine tanks follow two legs, turn, and settle.
The ordinary unit commands remain usable afterward. To render the final pose:

    cargo run -p mc-game -- --scene formations --map dev16 --ticks 300 --screenshot formations.png

## Validation

The combat and formation integration suites cover arrival, queued turns, altitude
separation, sustained flight, target retention, bomb salvos, shield interception,
edge attacks and deterministic snapshot continuation with one and four workers.
The network/replay suite covers the version-6 protocol. Aircraft model checks
cover nozzle placement and geometry/LOD budgets.

The broader suite currently has three failures outside aircraft:
shield model height, shield upgrade construction presentation, and the air
factory reduced-LOD budget.

See [Aircraft research and tuning](AIRCRAFT.md) for sources, flight constants,
bomb release, exhaust, and a shield bombing demonstration.

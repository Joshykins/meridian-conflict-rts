# Aircraft research and tuning

## Reference research

The implementation uses the published FA/FAF simulation data as a mechanical
reference, rather than claiming to reproduce the closed engine exactly.

- [FAF UEF T1 bomber blueprint](https://github.com/FAForever/fa/blob/develop/units/UEA0103/UEA0103_unit.bp):
  minimum and maximum airspeed are both 10, with damped move/turn/roll coefficients,
  explicit engage/break-off distances, bomb prediction and computed release,
  and two exhaust bones. This supports sustained forward passes, time to line
  up a return, and inherited bomb momentum. These are current FAF data, not
  verified original retail FA balance values.
- [FAF air superiority fighter blueprint](https://github.com/FAForever/fa/blob/develop/units/UEA0303/UEA0303_unit.bp):
  separate minimum/maximum speeds, steering and roll damping, forward weapon
  arcs, contrails and cruise exhaust.
- [Square Enix SC2 UEF aircraft](https://www.supremecommander2.com/na/units/air/uef.html)
  distinguishes air-only fighters, area bombers and gunships.
  [Cybran aircraft](https://www.supremecommander2.com/na/units/air/cybran.html)
  additionally include a fighter/bomber. These official descriptions do not
  publish steering constants; SC2-identical physics is not claimed here.

## Changes

The stationary bombing bug came from idle bombers acquiring weapon targets
without starting the flight controller. All armed aircraft now launch an attack
flight. Attack-move retains a detected target throughout a return leg; death or
a failed search of the last seen position releases it. Temporary fog during a
return leg does not cancel the attack or reveal hidden target movement.

| Aircraft | Speed | Acceleration | Maximum turn | Cruise above surface |
| --- | ---: | ---: | ---: | ---: |
| Shrike | 42–76 m/s in combat | 20 m/s² | 60°/s at cruise, up to 108°/s slow | 200 m |
| Petrel | 70 m/s | 18 m/s² | 32°/s | 200 m |

These values fit Meridian's scale. The altitude specifically implements the
request to clear shields; it is not copied from FA's coordinate scale. The
largest current dome has a 180 m radius. Both T1 aircraft share the lower 200 m
cruise band. Idle aircraft brake, level, and descend onto clear dry ground at up to 12 m/s,
easing to 2 m/s through the final 24 m before touchdown.
They remain aloft over water, steep ground, buildings or occupied landing sites.
New orders lift a landed aircraft 16 m clear before it accelerates horizontally.

Fighters pursue a predicted target position instead of a fixed combat center.
During an engagement they climb or dive to the detected target's actual altitude,
with smooth vertical acceleration and 24 m clearance over terrain and water ahead.
Their weapons remain available below the normal cruise band. When no detected
target remains, they resume normal cruise/landing behavior. This simulation
change requires network and replay version 10.
They shed speed as the opponent moves off the nose, gain turn authority at lower
speed, then accelerate as they line up. After several seconds of unsuccessful
turning they briefly roll out and regain separation before a new intercept;
staggered breakaways prevent matched opponents from mirroring a perpetual circle.
The pursuit and breakaway counters are serialized and included in the sync hash.

Yaw ramps with the serialized bank state and eases near the desired heading.
Cruising formation members move forward along their heading. Low-speed docking
is limited to the final 24 m and stays at or below one-third cruise speed. Off-axis
approaches limit speed by the available turn radius, so slow-turning aircraft
can close on their slots instead of orbiting outside the docking zone. Bombers extend past the target, then curve directly
into the next attack line in one continuous turn. There is no intermediate
setup waypoint in ordinary passes. Egress gives the aircraft room to finish
its turn before the high-altitude bomb-release point. A map-boundary escape
still redirects into usable airspace when a normal pass would leave the map.
Aircraft ignore ground crowd steering and hull-contact displacement, including
other aircraft at the same altitude; formation slots provide flight spacing.

Aircraft weapons require an attack flight and sufficient altitude. Bombs require
at least 75% cruise speed; fighter guns remain available down to 50% cruise.
Bombs release when their predicted landing carpet crosses the target, inherit
the bomber's forward velocity, and fall under gravity; they cannot launch
upward or steer sideways like artillery shells. Once started, the rack finishes
its eight drops through the pass. A target acquired too close requires another
approach. Bomb acquisition range and vision are 320 m to accommodate release from 200 m
at 70 m/s with the longer salvo. Vision must cover the release point so an
unassisted bomber can reacquire its target before dropping on a return pass.
Direct projectile lifetimes account for slant distance up to aircraft.

The bomber drops eight bombs in four symmetric pairs, 0.3 seconds between pairs.
Release prediction uses four drop times to keep the carpet centered. Each bomb
leaves a faint continuous white ribbon for 1.2 seconds, and each release has a
soft descending whistle. Falling bombs use matte black casings with no glowing
tracer; the impact retains its explosion color. The impact uses its own 1.4 visual scale, independent
of the suppressed muzzle flash; damage and splash radius are unchanged.

The interceptor has swept wings and a canopy; the bomber has broad, nearly
straight wings, an enclosed armoured nose, twin nacelles and a belly bomb rack.
Exhaust samples the actual interpolated heading, pitch and bank, leaving soft
broad, low-opacity clouds that expand, drift and fade over roughly 2.8 seconds. Parked, unfinished, factory-held and radar-only aircraft do not
emit trails. Mesh guns remain unlit conventional weapons.

Simulation behavior changed, so network and replay versions are 8.

## Inspect

    cargo run -p mc-game -- --scene aircraft --map dev16

Three bombers repeatedly attack a powered heavy shield. This exercises release,
shield interception, formation spacing, egress and return in the ordinary simulation.

    cargo run -p mc-game -- --scene aircraft --map dev16 --ticks 60 --follow 10 --screenshot aircraft-pass.png

The formations scene still exercises queued turns and arrival. Focused tests
cover idle close-target bombing, complete repeated salvos, forward-only bomb
velocity, gradual turn entry, no sideways cruise motion, shield-roof hits,
direct AA projectile lifetime, and deterministic combat snapshot continuation, fog-of-war returns, and flight-state
hashing.


## Verification

The final build passes. The 28 combat, 33 formation/flight, two fog and 28
network/replay tests pass, including the new fog return and flight-state hash
checks. Return-path regression rejects an extra turn after a straight setup
leg, and crossing-aircraft regression compares both paths against clear-air
controls. Sustained fighter combat checks speed variation, firing opportunities
and snapshot continuation during a breakaway. Landing speed and touchdown easing, close-slot capture from crosswise and
reverse headings, takeoff, and water/obstacle clearance have focused regressions. Aircraft nozzle placement and LOD tests pass. The broader suite still
reports the shield-height, shield-upgrade presentation and air-factory LOD
failures documented in AIR_FORMATIONS.md.

Both model previews and an in-game shield-pass overview were inspected; the
overview visibly shows three engine wakes. The close-up in-game capture timed
out on software Vulkan. A small overview is substantially faster:

    cargo run -p mc-game -- --scene aircraft --map dev16 --ticks 65 --follow 1 --size 400x250 --screenshot aircraft-small.png

For close inspection on a hardware renderer, add --select aster_t1_bomber
without --camera; the aircraft scene then focuses the selected plane.

## Aircraft destruction

Destroyed airborne aircraft burst into flame, retain their forward momentum,
and tumble under gravity with a continuous dark smoke and ember trail. Impact
on terrain or water triggers a second explosion and leaves the normal
reclaimable wreck at the landing position. Falling hulls cannot fight, be
selected, obstruct navigation or be reclaimed. Reclaimed aircraft and hidden
factory products do not produce crashes.

Crash position, velocity, age and original attitude are part of serialized,
hashed simulation state. Replay and network versions are 8 for this state change.

Inspect the full death sequence with:

    cargo run -p mc-game -- --scene aircraft-crash --map dev16

For a rendered falling-frame capture with smoke history:

    cargo run -p mc-game -- --scene aircraft-crash --map dev16 --ticks 1 --follow 25 --size 400x250 --screenshot aircraft-crash.png

On the current Dev Basin setup, --follow 30 captures the impact; --follow 50 shows the smoking wreck.

See AIR_ROSTER.md, Flight refinements, for the current VTOL, trails, altitude, and bombing-return behavior.

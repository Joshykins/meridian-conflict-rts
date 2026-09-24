# Configurable skirmish AI

In **Skirmish**, click an AI callsign to edit that opponent. Settings remain attached
to the slot when its team or landing zone changes, and survive map changes for shared slots. Observer slots have the
same controls. Each opponent has independent difficulty, doctrine, adaptation,
and force preference.

- **Easy / Normal / Hard**: decisions every 30 / 20 / 15 ticks, with 30 / 90 / 150
  seconds of scouting memory. Hard plays as well as the AI can; the levels below are
  handicapped in how they spend (`AiConfig::skill`): fewer builders given work per
  decision, shorter trips for mines, weaker bare-ground mines accepted, shorter upgrade
  paybacks, less power, fewer factories, later factory upgrades, smaller waves. Easy
  also skips outnumbered-force retreats. All levels use the same resources and
  construction rules. Thinking every 8 ticks made the AI play worse, so Hard does not.
- **Adaptive**: switches between expansion, pressure, and defense as the situation changes.
- **Aggressive**: earlier, smaller attacks and more forward production.
- **Economic**: more engineers, factories, and expansion before committing.
- **Defensive**: larger waves and a stronger preference for holding territory.
- **Adaptation**: how strongly remembered enemy capabilities and fortified objectives
  influence production and target selection. Zero disables those production bonuses;
  immediate defense, scouting, and health-based retreat still work.
- **Force preference**: balanced, land, air, or naval emphasis. Preferences are weights,
  not promises to build unavailable units. Naval preference becomes useful as ship and
  shipyard blueprints are added.

From WSL, build and play on Windows:

```bash
./play.sh --map dev16 --ai-difficulty hard --ai-doctrine adaptive
./play.sh --map dev16 --observe --ai-doctrine aggressive --ai-domains 100,140,80
```

CLI tuning applies to every AI in the launched match. Use skirmish setup for different
opponents. `--ai-adaptation 0..100` and `--ai-domains LAND,AIR,NAVAL` (each 0..200)
provide finer tuning; a zero domain weight disables its combat production.
`PlayerSetup.ai: AiConfig` also exposes the wounded-unit retreat threshold.
Inputs are normalized once on world creation, serialized in match setup and snapshots,
and included in the simulation hash.

## Decisions and information

The AI sends ordinary commands with no income multiplier. It knows enemy landing
zones, as the original AI did. Unit composition comes only from detected, identified
contacts. Unknown radar blips do not disclose blueprints; unseen movement never updates
a remembered position. Sightings expire and revisiting an empty position clears them.
Memory is capped at 256 contacts per opponent.

Production ranks legal build-menu entries by weapon coverage against remembered
targets, damage per cost, current composition, resources, siege needs, and domain
preference. Small deterministic tie variations prevent identical rotations. The AI
counts active scouts, includes air combatants in army strength, adds anti-air defenses
when aircraft are observed, and buys limited support units once it has an army to escort.

Armies gather and attack, raid economic targets, keep a small reserve for larger
waves, and consider scouted defenses when choosing objectives. Nearby busy units can
respond to raids. Wounded units and badly outmatched expeditionary units withdraw;
recovering units stay out of new waves until healed or their recovery interval expires.
Low-health commanders try to withdraw toward friendly repair support.
Construction limits unfinished projects, finishes urgent power, and reassigns builders
after assistance is complete. Upgrades wait for a functioning energy economy.
Tactical reassignment runs at a bounded cadence and considers at most 128 combatants
per pass, rotating through larger forces.

## Economy, turrets and waves

- The first three mines come right after the first factory, on bare ground if no ore
  is near. Later bare-ground mines go only where they would get at least the skill's
  share of a whole circle of land, so the gaps between mines are left alone.
- Mine upgrades go to the mine whose next tier pays back soonest (energy counted at 6
  per mass), within the skill's payback, doubled while materials pile up. They start
  only while energy flows (efficiency 90%+), and one at a time plus one per 60 mass/s.
  The Deep Core only qualifies for Hard with materials piling up.
- When materials pile up unspent (store over 40% and spending below income), another
  factory comes before new mines and power, at the best tier the builder and income
  allow, up to the skill's cap. At most one factory in four upgrades at a time.
- Builders more than 1.5 km from home build no power (a firebase gets its two plants),
  help only with sites near them, and leave the base's factories and yard buildings to
  builders at home. Plants dropped where a builder stood used to litter the map, and each
  one then seeded a little farm that home builders grew around it.
- Base layout (`ai/layout.rs`): power and storage go in farms. Farm centres come from
  the ground alone (the widest open home ground 240-420 m behind the base, or out to
  its flanks, never toward the enemy and clear of the factory yard), so a farm keeps its
  place all match, and a farm next to water no longer strings plants along the shore.
  Each farm fills from its middle out on a grid one plant apart: 2x2 plants stand flush
  in a block, bigger plants and storage start on the next farm and keep their lanes. A
  shield goes only where it covers at least 1.5 times its own mass cost in factories,
  power and storage that no other shield covers, on the free lot near there that
  covers the most. Without such a spot the builder moves on to its next job.
- Turrets: one per raid spot while defending, a few at the base's front (2 + half the
  factories), and one per mine. Materials no longer go into turret piles.
- Waves leave only from the staging point as one group, so they move in formation. A
  wave out in the field presses on to the next target while half a wave or more is out
  there; a remnant falls back to join the next. Outmatched units fall back together.
- A wave out in the field moves on only as a group: each 600 m cluster of it, busy
  units counted, waits until two thirds of it are idle, then goes on together (or
  back to staging if too few are left). Sending each unit on as it went idle turned
  the wave into a stream that arrived, and died, a unit at a time.
- Aircraft (`ai/groups.rs`) gather at the staging point and strike as a wing: four
  at first, more as waves go by, up to ten. Aircraft that come back idle regroup
  there first. Gathered fighters fly with a strike as escort; fighters hunt enemy
  aircraft within 1.5 km of home at once, whatever their number. Bombers that only
  hit ships (the Gannet) strike ships as their own wing and are never sent at mines.
- Ships gather at the fleet's anchorage (its water nearest home) and sail as a fleet
  of four or more, or at once against a contact within 800 m of it. A fleet out at
  sea moves on the way a land wave does.
- A mine being planned claims its deposit like a built one: no other mine is planned
  within its reach. (Its site can stand off the deposit's middle, and a check near the
  site alone sent every idle builder to the same deposit, piling mines up there.)
- Buildings keep lanes (`ai/lots.rs`): 24 m between buildings (2x2s such as power may
  pack together), and a 48 m apron in front of every factory's exit kept clear with a
  lane around it. Placement itself only forbids overlap; before this a factory could
  end up facing a pocket walled in by storage and shields, and everything it made sat
  there for the rest of the match.
- Base buildings go only on ground the army can walk to from the start (`ai/staging.rs`
  floods the terrain around it, stopped by cliffs and water, not by buildings), and the
  staging point moves onto that ground when the default one is below a cliff or across
  a river. Units made on another shelf go with the wave from where they are.
- Factories set no rally point: finished units roll out idle and the army sends them to
  the staging point with the rest. (A rally among the base's buildings jammed: units a
  few metres short of it in the crowd never finished the move and never joined a wave.)
  Hurt or outmatched units fall back to a point 220 m short of the start on their own
  side, not into the base, and hurt units already near home stay and fight.
- The navy only targets water its fleet can sail to (`ai/sea.rs` floods the sea on a
  128 m grid); with nothing seen it heads for its water nearest an enemy start.
- Threats the land army answers are land and hover units only: a gunship over a mine or
  a boat off an offshore one used to hold the whole army at home. A raid on an outlying
  mine takes the six nearest idle units; the rest carry on.
- A land or hover threat counts only when there is walkable ground within 150 m of it
  that home can reach (`land_can_answer` in `ai/staging.rs`). A hover tank parked on a
  lake by the base used to put the side in Defend and send every idle unit at it each
  think: the army piled up on the shore out of range, never went idle and never left,
  and died there a unit at a time to whatever it could not answer.
- Builders keep off ground under an enemy's guns and ground where the side just lost a
  building (`ai/danger.rs`). An armed enemy ground unit or turret seen in the last 10 s
  covers its weapon range plus 60 m; a building destroyed (finished or not) keeps
  builders 220 m clear for 45 s per loss there in the last 3 minutes, up to 3 minutes.
  No new site, no help with a started one, and no walk to a firebase inside either.
  Before this, a mine or turret killed by a raider was the next job of the nearest idle
  engineer, which rebuilt it under the same guns, round after round.

`tests/zz_ai_stall_probe.rs` plays AI-only matches and prints, per side and minute, the
army units parked near home (3+ minutes within 60 m) and how tightly the mines are
packed (`STALL=map:players:minutes[:seed]`, `STALL_WHY=1` lists what the parked units
are doing). Park-ups show after 30 minutes, so run 40. `STALL_ALL=1` counts engineers
and support units too; `STALL_HUMAN=1 STALL_FORT=N` makes slot 0 a passive player behind
N turrets, topped up every minute, so the waves keep dying against it.

`tests/zz_ai_stream_probe.rs` measures streaming: per side and domain, the size of
each order sending units to the enemy's half (1-2, 3-5, 6+) and the share of units
there with fewer than three friends within 250 m (`STREAM=map:players:minutes[:seed]`).
Before the grouping above, almost every air and sea order was one or two units.

`tests/zz_ai_duel_probe.rs` plays AI against AI on a real map and prints each side's
economy every three minutes (`DUEL=map:diff:diff:minutes[:seed[:swap]]`, `DUEL_JOBS=1`
lists every builder's order, `DUEL_WHY=1` the upgrade gates).

## Extending the roster

There are no unit blueprint names in production or tactical selection. Add units to
the normal builder/factory build menus and populate their existing blueprint metadata:

- `motion.layer`, categories, and `water_build` determine movement/production domains.
- Weapon `target_mask`, damage, reload, salvo, and range describe combat capability.
- Cost and tech describe investment; `Artillery`, `Scout`, `Engineer`, and `Defense`
  categories supply specialized roles.
- Shields, radar, anti-missile capability, and drone carriers identify support units.

Factory selection checks for a legal nearby lot before choosing a domain. Ships have
separate objectives; shoreline attack positions must be navigable and within weapon
range of the target. Movement destinations are projected onto each hull's valid terrain.
A synthetic ship test exercises these paths before the real navy roster arrives.

This is a foundation for naval combat, not transport or amphibious-invasion planning.
Destination validation does not prove connectivity across separate seas/islands;
global route-aware theater planning and naval roster balance still need real maps
and ships. Unit special abilities beyond the metadata above need their own orders.

## Verification and diagnostics

```bash
cargo test -p mc-sim --lib ai::tests -- --nocapture
LAYOUT=twin_shoals:2:30:5 LAYOUT_OUT=/tmp cargo test --release -p mc-sim --test zz_ai_layout_probe -- --ignored --nocapture
cargo test -p mc-sim --test battle
cargo test -p mc-game ui::skirmish::tests
cargo test -p mc-net
cargo run --release -p mc-game -- --map dev16 --observe --ai-difficulty hard --bench 18000 --threads 4
```

Benchmarks print each AI's doctrine, stance, contacts, recovery count, main waves, raids, production,
economy, roster, losses, kills, and match winner. Stance numbers are 0 expansion,
1 defense, 2 raiding, 3 firebase construction, 4 attack.

The AI state and setup layout changed. Replay and network protocol versions are now 9;
older replay/network versions are rejected instead of silently diverging.


### Validation notes

The implementation has focused checks for fog-respecting memory, counters and their
expiry, configuration, retreating busy units, ship metadata and water destinations,
support quotas, recovering construction, tech-builder access, snapshots, sustained
AI combat, and finishing a game with a decisive army advantage. Battle tests also
compare simulation hashes across worker counts. The skirmish panel was inspected in
a 1920 x 1080 Windows GPU render.

An 18,000-tick Dev Basin 16 observation run produced 79 offensive raid dispatches,
three main waves, and 212 total kills, with 286 units still alive. It had no winner
at 30 simulated minutes. Mean simulation time was 1.65 ms/tick; the AI portion was
0.012 ms/tick. Pathfinding produced isolated spikes up to 491 ms. These are development
measurements, not a guarantee for other maps, seeds, armies, or hardware. This run
preceded the final scout quota correction.

Difficulty order is not yet settled. In 20 duels over 30 minutes on dev16, meridian_basin
and twin_shoals (2026-09-23), Normal led Hard 7 to 5 and Easy led every game on
meridian_basin; single games swing a lot, so compare several seeds and both starts.

Evenly matched economic duels can still settle into prolonged fights. Naval
connectivity, transport strategy, late-game stalemate breaking, and difficulty balance
need further playtesting with the real navy roster.

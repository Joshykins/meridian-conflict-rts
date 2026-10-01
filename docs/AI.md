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
  per mass), within the skill's payback (800 / 1100 / 1500 s), doubled while
  materials go spare (store over 40%). Up to one plus one per 30 mass/s run at once
  (`mine_upgrade_budget`), each started only with the energy to spare for it. Judged
  by the whole stall (mass included) and held to 660 s and one at a time, the AI
  upgraded about six mines in half an hour and its income went flat by 20 minutes.
- While materials go spare, up to four more upgrades run past the budget, started
  whatever the energy, and the side's mines are put last (`direct_focus`): the extra
  upgrades take only the materials and energy the factories leave.
- Power is built by the side's best builders, at the biggest plant they make (a
  Reactor III once income reaches 22 a second). Once any builder of a higher tier
  stands, lesser ones start no plants of their own: while power is wanted they
  assist the nearest plant going up, and they build a small one only when the
  energy has run out with no plant rising. The best builders pick their jobs first
  each think. T1 Masons used to dot 50 to 100 small reactors about a base.
- Engineers go up a tier where they stand once the side's tech allows (a third of
  them at most at once, the upgrade queued behind any job under way); spare
  builders assist an engineer's upgrade before a factory. A factory makes the best
  engineer it can at once while the side has fewer than one plus one per two
  factories of them; an upgrade's successor under construction does not count.
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

## Threat: energy, waves, tech and strategic projects

Measured on 2026-09-26 with `tests/zz_ai_threat_probe.rs`, Hard took 11 to 16 minutes to
kill a commander that never moved, spent much of each game in an energy stall,
held one or two factories on a full store, stayed at tech 1 for thirty minutes in
AI duels, and never built anything past tech 3. What changed:

- Energy is planned ahead (`ai/energy.rs`). `energy_need` is what the side would
  draw with every factory and builder at work plus the upkeep of everything standing
  or going up, the draw of upgrades running, and room for the next mine upgrade and
  tier step. In a stall power comes first, before the rest of the opening's mines
  and turrets; otherwise power is built until income reaches that need, before far
  mines and turrets (only the first watchtower goes ahead of it).
  While a resource is short the side's economy focus (`direct_focus`) is set to it,
  so the power or mines it builds are paid ahead of everything else.
- An upgrade starts only with the energy to spare for its own draw (`can_fund`):
  one started at 90% efficiency stalled every
  factory for minutes. Energy only: a side spending all it makes is short of mass
  nearly all the time. A candidate the side cannot fund no longer holds back a
  cheaper one behind it.
- Turrets on quiet mines wait until income reaches 6 a second or the opening's
  three mines stand; a mine under attack still gets its guard at once. Radar goes
  up once the first mines and two plants stand, not only after the first turret.
- Tech is a step taken on purpose: one factory goes up a tier once income reaches
  the skill's `tech_income` (tech 2), then three times it (tech 3), whether or not
  materials pile up (`tech_step`). A busy factory is taken too, the upgrade queued
  behind its current unit: factories kept busy were never idle when the AI looked.
- Factories are added while the ones standing could not spend 70% of the income
  if all were busy (`factory_mass_draw`), up to the skill's cap. A factory goes to
  the firebase only when the firebase is within 3 km of home: on a big map every
  factory past the second went eight kilometres out and was never finished.
- A land unit that is where it was sent counts as idle, though the crowd there keeps
  its order from ending (`ai/arrival.rs`): the blob's edge stands 10 m per square
  root of its size short of the point. The staging radius grows the same way. On
  Serac Divide 190 of 256 units sat at staging under a move order, the staging point
  looked empty, and no full wave left for twenty minutes. A wave takes units still
  walking to the staging point within three staging radii along with it.
- Strategic projects (`ai/projects.rs`): a builder with experimentals or strategic
  weapons in its menu starts one once the side could pay for it with 60% of five
  minutes' income (ten for tech 5), one at a time, two while materials pile up.
  Enemy silos seen are answered with an interceptor array and enemy spaceships with
  an anti-air experimental; otherwise the doctrine picks, the kind it holds fewest
  of first (aggressive: mobile experimentals, then nukes, then map guns; economic:
  nukes first; defensive: map guns first). Spare builders at home help the project
  before any other site. The kind comes from the data (a `strategic` launcher, an
  anti-air or artillery structure, or a mobile unit), never from a unit's name.
- Nukes: a ready warhead goes at the remembered enemy spot worth at least twice a
  warhead's mass under its blast, the enemy commander (seen in the last 10 s)
  counted as 60,000. Not where the side's own units are worth a quarter of that,
  and not into an interceptor's cover unless the salvo outnumbers the rounds it holds.

`tests/zz_ai_threat_probe.rs` plays an AI against a player who never gives an order,
printing per minute the AI's army, how much of it stands within 1.2 km of the enemy
start, income, store, spending, energy and nukes ordered, and the minute it wins
(`THREAT=map:difficulty:minutes[:seed[:swap]]`). `THREAT_AI=1` makes it an AI duel
with the same readout for both sides, `THREAT_WHERE=1` shows where the army is and
where its orders point, `THREAT_OPENING=N` lists every structure started in the
first N minutes, and `THREAT_ROSTER=1` lists each side's units at the end.

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

## Island maps: sea, sea mines and spaceships

Measured on 2026-09-30 with `tests/zz_ai_domain_probe.rs` on The Axis (8 AIs, two
seeds, 40 minutes): every side had one land factory and no shipyard for the whole
game, one or two sea mines, 55 to 196 land units of which up to 77 stood parked at
home, and 29 to 60 unarmed Vigils. No side was beaten. What changed:

- The land route (`ai/theatre.rs`): on its first think a side floods the land it can
  walk from its start, on a 64 m grid from the terrain alone, and keeps which enemy
  starts that reaches. With no land route to an enemy still in the game:
  - land-only combat units are made only up to a home guard of eight; past that a
    land factory makes hovers, amphibious units and engineers. The guard answers
    raids on its own ground and otherwise waits at the staging point; it is never
    part of a wave, and the army's size for stance and waves leaves it out.
  - land-only scouts and land-only experimentals are not built.
  - the first shipyard comes right after the opening mines, ahead of more power;
    land factories count a third and shipyards double when the next factory's
    domain is chosen. The first factory is a land factory on every map: it makes
    the engineers.
  - warships of the upper air come first among the strategic projects.
- A shipyard goes on the water nearest the start where one can stand, found in
  rings out to 2.4 km (`shipyard_anchor`), not by a site search around the base's
  yard, which reached the coast only on small islands.
- Sea mines: a mine at sea shares only with mines at sea, and reaches 1.5 km, so it
  is kept a sea reach from those and is not kept off by the island's own land mines
  (a land reach from every mine left a small island no sea to mine). Off-ore mines
  no longer wait for a turret. Of the nearest twelve open spots, the one that yields
  most for the walk there is taken.
- A ship joins the defence of home only against a raider it can shoot from the
  water. Sent at a tank inland, it got no order, sat idle off the coast for the
  whole raid and was kept from the fleet.
- Ships of every hull size gather into one fleet: a fleet sails once four ships,
  of any size, are at home. Split by size, boats, frigates and submarines each
  gathered two or three at home and never sailed.
- Spaceships with guns (`direct_capital` in `ai/groups.rs`) gather 400 m out from
  the start toward the enemy and strike once those gathered carry 2000 mass (a
  heavy frigate alone, two corvettes), at the enemy's factories, mines or start;
  a raid on the base calls them all home. They were counted as bombers: a wing of
  four was waited for, and they went after mines.
- An unarmed ship is not a strategic project. A side builds one radar ship (the
  Vigil) once its income reaches 25 a second, and it escorts the army.

`tests/zz_ai_domain_probe.rs` plays an all-AI team match, west against east, and
prints per side every few minutes its mines on land and at sea, its factories by
domain, and its army by domain (land, hover, naval, air, space): at home, out past
1.5 km and in the enemy's half, with how many have stood in one place for three
minutes (`DOMAIN=map:players:minutes[:seed[:difficulty]]`, `DOMAIN_EVERY=N`,
`DOMAIN_WHY=1` lists what the parked units are doing, `DOMAIN_ROSTER=1` each side's
units at the end).

## Strategies: plans, warp and landings

Added 2026-10-01 after the user said the AI never used warp, never landed an army
from a lift ship, and played the same game every time. Each side now holds a few
plans (`ai/strategy.rs`) and its builders, factories and army serve them. Hard
holds three at once, Normal two, Easy one (`Skill::gambits`).

| Plan | Picked when | What changes |
|---|---|---|
| `landing` | a builder can make a lift ship with a warp drive; far more on an island map, more against a fortified front | a lift ship is built (`lift_job`); land units board it at staging, it jumps beside the target and lets them out, then jumps home (`ai/landing.rs`) |
| `warp_raid` | an armed spaceship is in the menu, income 12+/s; more when little anti-air has been seen | warships go one at a time at the softest mine, plant or engineer on the enemy's outskirts, by warp; warships are a project kind first |
| `air_fleet` | an air factory stands, under 8 enemy anti-air seen; far more when their base has been seen with little | bombers wait behind the base for twice the usual wing (to 16), then go at the economy least covered by anti-air; air factories and bombers preferred |
| `hunt` | a land route, a factory, and engineers or mines seen on the enemy's outskirts (more for each) | every minute the fastest six at staging go after an engineer seen lately, or an outlying mine with the fewest guns (`ai/hunt.rs`) |
| `siege` | a map gun in the menu; more for a defensive side, a fortified enemy, 40+ income | map guns first among projects, two at a time, and a shield more per map gun (shields value projects) |
| `nuke_race` | a silo in the menu, the enemy base seen and no interceptor seen in it | the silo first among projects, two at a time; dropped once an interceptor is seen |
| `submarines` | a yard of ours makes them, enemy ships or a yard seen, fewer than 3 sonar seen | submarines and shipyards preferred |
| `scouting` | always open; more while no enemy factory has been seen | four scouts, not two, and the Vigil once income reaches 15/s, not 25 |

A plan must score at least 40 on what the side has seen to be taken up at all;
a slot is left empty rather than filled with a weak plan. The lift ship waits for
the second factory. The first plans are picked four minutes in, once the scouts have looked; before,
every side picked the same three from an empty map and kept them. Plans are
reviewed every two minutes. A held plan gets a bonus for its first three reviews
and loses score for each review after, so a side commits and a long game sees it
move on. One draw from the match's random stream makes sides with the same doctrine
pick differently. A plan whose score falls to zero (its unit no longer buildable, or
the enemy answered it) is dropped at the next review.

Warp, whatever the plans (`ai/warp_ops.rs`):

- A strike of armed spaceships jumps to `STANDOFF` (260 m) short of its target on
  the side it came from, then attack-moves in; ships within 1.4 km fly. Every jump
  comes out 150 m clear of any remembered Undertow's field (`safe_mark`).
- A spaceship below half health out at the front jumps home.
- The Vigil sweeps: each time its drive is ready it jumps 900 m short of the next
  place on a round of enemy starts, remembered enemy mines and far ore, drifts
  700 m across it, and jumps home when hurt or with anti-air within 1.2 km. It used
  to be counted as a radar tower (`land_radar` took any Intel unit with radar) and
  never left the base.
- A jump waits until the side has the energy for its charge.

Seen enemy silos are answered with an interceptor at once: the answer goes up
beside any other project, may cost twice as long's income, and an answer the side
cannot build no longer stops every other project.

On an island map the home guard grows by half of the lift ships' room while
`landing` is held, so a landing has cargo, and four stay home. A land group put
ashore where home cannot be walked to keeps fighting instead of falling back.

`tests/zz_ai_domain_probe.rs` prints each side's plans, landings started, lift ships
and warp jumps every report.

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

The AI knows whether its land army can walk to the enemy (above) and lands it by
lift ship while it holds the `landing` plan; naval roster balance still needs real
games. Unit special abilities beyond the metadata above need their own orders.

## Verification and diagnostics

```bash
cargo test -p mc-sim --lib ai::tests -- --nocapture
LAYOUT=twin_shoals:2:30:5 LAYOUT_OUT=/tmp cargo test --release -p mc-sim --test sim -- zz_ai_layout_probe:: --ignored --nocapture
cargo test -p mc-sim --test sim -- battle::
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
connectivity, late-game stalemate breaking, and difficulty balance need further
playtesting with the real navy roster.

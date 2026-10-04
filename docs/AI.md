# Skirmish AI: settings and the base

Every AI seat is played by the Commander, the planning AI: game plans held at
stakes, operations of grouped units, beliefs about the enemy, an economy run as
part of the plan, and an observer overlay of its reasoning. How it decides is in
`docs/AI_COMMANDER.md`. This page covers its settings and the code that carries
out what it decides about the base (`crates/mc-sim/src/ai/`): builders, factories,
upgrades, salvage, layout and the island-map rules. (The rule-driven "classic" AI
that came before was removed on 2026-10-01, once the Commander beat it; it is in
the git history.)

## Settings

In **Skirmish**, click an AI callsign and open **AI Settings** to edit that opponent.
Settings remain attached to the slot when its team or landing zone changes, and
survive map changes for shared slots. Observer slots have the same controls.

- **Easy / Normal / Hard**: decisions every 30 / 20 / 15 ticks, with 30 / 90 / 150
  seconds of scouting memory, and 24 / 50 / 100 orders a minute for its operations.
  Hard plays as well as the AI can; the levels below are handicapped in how they
  spend (`AiConfig::skill`): fewer builders given work per decision, shorter trips for
  mines, weaker bare-ground mines accepted, shorter upgrade paybacks, less power,
  fewer factories, later factory upgrades. All levels use the same resources and
  construction rules. Thinking every 8 ticks made the AI play worse, so Hard does not.
- **Doctrine** (Adaptive, Aggressive, Economic, Defensive): its tastes over the game
  plans (`docs/AI_COMMANDER.md`, "Personalities").
- **Forces** (balanced, land, air or naval emphasis): weights on the share of new
  units each force gets (`AiConfig::domain_weights`, 0..=200 each, 100 even). A zero
  weight builds no factory and no combat unit of that domain.

From WSL, build and play on Windows:

```bash
./play.sh --map crosswater --ai-difficulty hard --ai-doctrine adaptive
./play.sh --map crosswater --observe --ai-doctrine aggressive --ai-domains 100,140,80
```

CLI tuning applies to every AI in the launched match; use the skirmish set-up for
different opponents. Inputs are normalized once on world creation, serialized in
match setup and snapshots, and included in the simulation hash.

## Decisions and information

The AI sends ordinary commands with no income multiplier. It knows enemy landing
zones, as a player does. Unit composition comes only from detected, identified
contacts. Unknown radar blips do not disclose blueprints; unseen movement never
updates a remembered position. Sightings expire and revisiting an empty position
clears them. Memory is capped at 256 contacts per opponent.

There are no unit blueprint names in its decisions: a unit is what its blueprint
says it can do (`commander/profile.rs`).

## The economy and the base

- The first three mines come right after the first factory, on bare ground if no ore
  is near. Later bare-ground mines go only where they would get at least the skill's
  share of a whole circle of land, so the gaps between mines are left alone.
- Mine upgrades go to the mine whose next tier pays back soonest (energy counted at 6
  per mass), within the payback and as many at once as the economy sets
  (`commander/economy.rs`: more while the store fills). They wait while power is
  short, and each starts only with the energy to spare for its draw (`can_fund`,
  judged by the energy spent, not asked for). The economy focus (`direct_focus`)
  pays power first while energy is short and mines and their upgrades first while
  materials are.
- Power is built by the side's best builders, at the biggest plant they make (a
  Reactor III once income reaches 22 a second), for what the economy says it lacks
  beyond the plants going up. Once any builder of a higher tier stands, lesser ones
  start no plants of their own: while power is wanted they assist the nearest plant
  going up. T1 Masons used to dot 50 to 100 small reactors about a base.
- Engineers go up a tier where they stand once the side's tech allows (a third of
  them at most at once, the upgrade queued behind any job under way); spare
  builders assist an engineer's upgrade before a factory. A factory makes the best
  engineer it can at once while the side has fewer than one plus one per two
  factories of them; an upgrade's successor under construction does not count.
- Tech is a step taken on purpose: one factory goes up a tier once income reaches
  the skill's `tech_income` (tech 2), then three times it (tech 3), whether or not
  materials pile up (`tech_step`); tech 3 waits while the side's army is behind the
  one it believes in. A busy factory is taken too, the upgrade queued behind its
  current unit.
- Builders more than 1.5 km from home build no power, help only with sites near them,
  and leave the base's factories and yard buildings to builders at home.
- Adjacency (`ai/adjacent.rs`, the rules in `mc-sim/src/adjacency.rs`): a power plant
  or a fabricator takes the free lot flush against the side's buildings where it saves
  the most a second: what it saves the neighbours that use what it provides (a
  factory's build energy and materials at full speed, upkeep) and what they save it,
  materials counted at 6 energy. From the corners of a side in, so a side takes as
  many as fit. Only where it would save nothing does it go to a farm. A fabricator and
  a plant of its tech (which go down together) are never set against each other, and
  a fabricator is not upgraded into a tier that would bind it.
- Fabricators are built when the side has energy to spare (power not wanted, the store
  three-quarters full), its materials are not piling up, its income is at least three
  times the skill's `tech_income`, their upkeep stays under half the energy income,
  and the one it can build that pays back soonest (cost, energy at 6 per mass, and the
  best plant's price for its upkeep) does so within the economy's payback. One at a
  time; a builder takes it before helping the sites going up, since a materials stall
  is when it pays and it is paid first with the mines (`focus.rs`). Upgrades follow
  the same test. All of them are paused while the power is out and resumed once the
  store is half full and the income carries them.
- Base layout (`ai/layout.rs`): storage, and power with nothing to save, go in
  farms. Farm centres come from the ground alone (the widest open home ground
  240-420 m behind the base, or out to its flanks, never toward the enemy and clear
  of the factory yard), so a farm keeps its place all match. Each farm fills from its
  middle out on a grid one plant apart: 2x2 plants stand flush in a block, bigger
  plants and storage start on the next farm and keep their lanes. A shield goes only
  where it covers at least 1.5 times its own mass cost in factories, power and
  storage that no other shield covers, on the free lot near there that covers the
  most.
- Turrets: one per raid spot while defending, a few at the base's front (2 + half the
  factories), and one per mine; a mine or plant a ground raid took gets one at once
  (`docs/AI_COMMANDER.md`, "Urgent wants").
- A mine being planned claims its deposit like a built one: no other mine is planned
  within its reach.
- Buildings keep lanes (`ai/lots.rs`): 24 m between buildings (2x2s such as power may
  pack together), and a 48 m apron in front of every factory's exit kept clear with a
  lane around it. A provider may touch a building it saves or is saved by, and
  providers up to 4x4 may touch each other, so they can ring a factory; aprons still
  stay clear.
- Base buildings go only on ground the army can walk to from the start (`ai/staging.rs`
  floods the terrain around it, stopped by cliffs and water, not by buildings).
- Factories set no rally point: finished units roll out idle and the operations take
  them up. (A rally among the base's buildings jammed: units a few metres short of it
  in the crowd never finished the move.) The first scout comes as soon as a factory
  is free, a second in its turn.
- A land unit that is where it was sent counts as idle, though the crowd there keeps
  its order from ending (`ai/arrival.rs`): the blob's edge stands 10 m per square root
  of its size short of the point.
- Builders keep off ground under an enemy's guns and ground where the side just lost a
  building (`ai/danger.rs`). An armed enemy ground unit or turret seen in the last 10 s
  covers its weapon range plus 60 m; a building destroyed keeps builders 220 m clear
  for 45 s per loss there in the last 3 minutes, up to 3 minutes.
- Nukes (`ai/projects.rs`): a ready warhead goes at the remembered enemy spot worth at
  least twice a warhead's mass under its blast, the enemy commander (seen in the last
  10 s) counted as 60,000. Not where the side's own units are worth a quarter of that,
  and not into an interceptor's cover unless the salvo outnumbers the rounds it holds.

## Island maps: sea and spaceships

- The land route (`ai/theatre.rs`): on its first think a side floods the land it can
  walk from its start, on a 64 m grid from the terrain alone, and keeps which enemy
  starts that reaches. With no land route to an enemy still in the game, land-only
  combat units are made only up to a home guard of eight (more while it holds a
  landing, for cargo), land-only scouts are not built, and the first shipyard comes
  right after the opening mines, ahead of more power.
- A shipyard goes on the water nearest the start where one can stand, found in rings
  out to 2.4 km (`shipyard_anchor`).
- The navy only targets water its fleet can sail to (`ai/sea.rs` floods the sea on a
  128 m grid).
- Spaceships jump (`ai/warp_ops.rs`): a strike comes out 260 m short of its target on
  the side it came from, 150 m clear of any remembered Undertow's field, and
  attack-moves in.

## Extending the roster

Add units to the normal builder and factory build menus and fill in their blueprint
data; the AI reads what each can do from it (`docs/AI_COMMANDER.md`, "Profiles"):

- `motion.layer`, categories, and `water_build` determine movement and production
  domains.
- Weapon `target_mask`, damage, reload, salvo, and range describe combat capability.
- Cost and tech describe investment; `Artillery`, `Scout`, `Engineer`, and `Defense`
  categories supply specialized roles.
- Shields, radar, anti-missile capability, warp drives, holds and drone carriers
  identify support units.

## Probes

Ignored tests that play AI matches on a real map and print what they measure:

- `tests/zz_ai_tournament.rs` (`scripts/ai-tournament.sh`): doctrine against doctrine,
  wins and trades (`docs/AI_COMMANDER.md`, "Tournament").
- `tests/zz_ai_threat_probe.rs`: an AI against a player who never gives an order, per
  minute its army, how much of it stands near the enemy start, income, store,
  spending and nukes ordered, and the minute it wins
  (`THREAT=map:difficulty:minutes[:seed[:swap]]`; `THREAT_AI=1` makes it a duel,
  `THREAT_WHERE=1` shows where the army is, `THREAT_OPENING=N` lists every structure
  started in the first N minutes, `THREAT_ROSTER=1` each side's units at the end).
- `tests/zz_ai_stall_probe.rs`: army units parked near home (3+ minutes within 60 m)
  and how tightly the mines are packed (`STALL=map:players:minutes[:seed]`,
  `STALL_WHY=1`, `STALL_ALL=1`, `STALL_HUMAN=1 STALL_FORT=N`).
- `tests/zz_ai_stream_probe.rs`: whether it attacks in groups or a stream, per side
  and domain (`STREAM=map:players:minutes[:seed]`).
- `tests/zz_ai_duel_probe.rs`: AI against AI, each side's economy every three minutes
  (`DUEL=map:diff:diff:minutes[:seed[:swap]]`, `DUEL_JOBS=1`, `DUEL_WHY=1`).
- `tests/zz_ai_domain_probe.rs`: an all-AI team match, west against east, per side its
  mines on land and at sea, factories and army by domain
  (`DOMAIN=map:players:minutes[:seed[:difficulty]]`, `DOMAIN_EVERY=N`, `DOMAIN_WHY=1`,
  `DOMAIN_ROSTER=1`).
- `tests/zz_ai_layout_probe.rs`: how tidy each base is (`LAYOUT=map:players:minutes`,
  `LAYOUT_OUT=dir` writes a top-down picture of every base).

```bash
cargo test --profile gate -p mc-sim --lib -- ai::
cargo test --profile gate -p mc-sim --test sim -- battle::
cargo run --release -p mc-game -- --map crosswater --observe --ai-difficulty hard --bench 18000 --threads 4
```

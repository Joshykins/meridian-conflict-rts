# The Commander: a rebuilt skirmish AI

Status: being built (2026-10-01). This document is the contract between the layers;
code that disagrees with it is a bug in one or the other.

## Why

The skirmish AI (`docs/AI.md`) is one pass every second or two: count everything,
pick a build job per idle builder, send idle units at a target. It has no idea what
it is trying to do or whether it works, so every behaviour is a rule bolted onto the
loop, and it plays the same game every time. The user's asks, in their words:

- commit to game plans, give up on them, push one harder, or run several at once:
  "a small courier raid with some things in the backline is different than a full
  bastion is different than culverins is different than 'don't know if they have
  nukes but they haven't done much, so I should scout, and probably start slowly
  building anti nuke'";
- no unit micro: "pulling back hurt units ... that's very non-human in a game of this
  scale". Groups get orders, the way a player gives them;
- never name a unit or a race: adapt to whatever roster it is given, a T5 included,
  "which is a MASSIVE commitment";
- balance and manage the economy as part of the plan, not beside it;
- fun first: the user judges the result by playing it.

## Rules every layer keeps

- **Deterministic.** It runs inside the sim on every machine. Fixed point (`Fx`),
  integers, stable sorts, no hash maps, nothing from outside `State` and the
  blueprints. Its state lives in `AiState` and is hashed.
- **No names.** Nothing branches on a unit key, mesh or race. A unit is what its
  blueprint says it can do (`profile.rs`). A *mechanic* the data cannot describe
  (warp, lift ships, diving, strategic launchers, shields that drop in a stall) is
  taught once, in one adapter (`mechanics.rs`); a new unit using an existing
  mechanic needs no AI work.
- **Group orders only.** Every order the Commander gives goes to a group (an
  operation's units), and is one a player could give: move, attack-move, board,
  land, warp, launch, build, produce. No per-unit retreats or kiting; units fight
  with their own built-in behaviour.
- **Bounded.** Each think has a cost budget; large passes are sliced over thinks.

## Layers

```
            +--------------------------------------------------------+
            |  Commander (plans.rs)  portfolio of game plans          |
            |  probe / invest / all-in, escalate, abandon, hedge      |
            +-----------+--------------------+-----------------------+
                        | budgets, reserves  | operations wanted
            +-----------v--------+   +-------v------------------------+
            | Economy (economy.rs)|   | Operations (ops.rs)            |
            | forecast, reserve,  |   | groups with a goal, lifecycle, |
            | invest by payback   |   | evidence (mass traded)         |
            +-----------+--------+   +-------+------------------------+
                        |                    | unit demand
            +-----------v--------------------v------------------------+
            | Production (solver.rs): needs -> what to build, from     |
            | the race's menu by value per cost against the enemy      |
            +---------------------------------------------------------+
   reads:   Profiles (profile.rs)   World model (world_model.rs)
            Beliefs (beliefs.rs)    Attention (attention.rs)
```

### 1. Profiles (`profile.rs`)

Built once per match from the blueprints, never stored in `State` (it is a pure
function of the data). For every blueprint:

- `cost`: mass plus energy at `ENERGY_PER_MASS`, and build time;
- `reach[Target]` and `dps[Target]` for each target class: `Land`, `Structure`,
  `Air`, `Surface` (ships afloat), `Submerged` (torpedoes only), `Missile`
  (interceptors and anti-missile);
- `ehp` (health plus shield), `speed`, movement `Domain` (land, hover, amphibious,
  naval, submarine, air, space), whether it crosses water;
- `carry` (lift room), `room` (what it takes in a hold), `warp`, `dive`;
- `sight`, `radar`, `sonar`; `strategic` kind (nuke, interceptor); `map_gun` (a
  structure that out-ranges ordinary defences many times over);
- `build_power`, `income` (mass, energy), `upkeep`;
- `roles`: a bit set derived from the above (`Line`, `Raider`, `Artillery`,
  `AntiAir`, `AntiNaval`, `Hunter` (anti-sub), `Transport`, `Scout`, `Sensor`,
  `Siege`, `Strategic`, `Interceptor`, `Shield`, `Builder`, `Economy`, `Titan`).

`Titan` is not a tier: it is a unit whose cost is a large share of the side's
income (judged at decision time). **Coverage**: `tests/ai_coverage.rs` lists every
unit of every race with its roles and the plans that can use it, and fails when a
buildable combat unit has no role any plan uses.

### 2. Matchups (`matchup.rs`)

`fight(a, b)`: the expected result of equal mass of `a` against `b`, from the
profiles: damage each can put on the other per second (zero where it cannot reach
the target class), the range edge (the longer reach gets free volleys while the gap
closes at the closing side's speed), splash against crowds, and effective health.
Values are calibrated against **real simulated battles**:
`tests/zz_ai_matchup_probe.rs` stages headless fights between groups of equal mass
for every pair of combat units, prints where the estimate and the sim disagree, and
the constants are tuned until the estimate picks the winner of most pairs. The
calibration is a probe the developer runs; nothing in a match depends on it.

### 3. World model (`world_model.rs`)

A coarse grid (`CELL` metres) rebuilt every few thinks from the side's contacts:

- `threat[Target]`: the enemy damage per second that reaches a cell, per target
  class, from remembered armed contacts within their reach;
- `value`: enemy mass standing there (economy, factories, the commander);
- `stale`: ticks since the side last had eyes on the cell (`fog` visibility);
- `ours`: own strength present.

Once per match: the land route (`theatre.rs`), and landing zones (flat open land
near enemy value, `transport::lift_can_land`'s rule).

### 4. Beliefs (`beliefs.rs`)

What the side thinks about each enemy, each with a confidence 0..=100, raised by
evidence and decaying without it:

- `income`: estimated from mines seen and game time;
- `tech`: highest tier seen, and expected tier from time and income;
- `composition`: their army mass by domain, seen recently;
- `air`, `navy`, `subs`, `space`: they field it;
- `nukes`: they have or are building a strategic launcher. Seen: certain. Unseen:
  rises with game time, their income and tech, and how quiet they have been (little
  army seen for their income), falls as their base is scouted without one;
- `interceptors`: count seen, and rounds held;
- `turtling`: much static defence seen for their income;
- `army_at`: where their main army was last seen.

Scouting goes where the most important uncertainty is (`Scout` operations ask the
beliefs which question is worth most and the world model where to look).

### 5. Operations (`ops.rs`)

An operation is a group of units with one goal, owned by a plan:

| Kind | Goal |
|---|---|
| `Army` | the main land/hover force: gather, push to an objective, take it, hold or move on |
| `Defend` | answer a raid on the base or an outlying mine |
| `Raid` | a small fast group on soft targets |
| `Landing` | load a lift ship, jump (warp) beside the target, land, fight |
| `Strike` | aircraft at a target, as one wing |
| `AirGuard` | fighters over home and the army |
| `Fleet` | surface ships: control water, shell the coast |
| `Wolfpack` | submarines on ships and sea mines where sonar is thin |
| `Warships` | armed spaceships, by warp |
| `Siege` | artillery and map guns on a target from beyond its defences |
| `Strategic` | nukes: when, where, how many |
| `Scout` | eyes on what the beliefs need |

Lifecycle: `Forming` (asks for units: the solver builds them, idle ones are
assigned) -> `Gathering` (at its rally point) -> `Executing` (group orders toward
the goal) -> `Judging` (on arrival, success, or losses) -> `Withdrawing` (the whole
group, on a bad trade) -> `Done`. An operation keeps a ledger: mass it destroyed,
mass it lost, mass committed. Its owning plan reads the ledger as evidence.

### 6. Attention (`attention.rs`)

A side has a budget of orders per minute and a number of operations it can watch
closely (focus slots) by difficulty. Each think, operations rank by urgency (under
attack, just arrived, idle); only those that fit the budget get new orders; the rest
keep their last ones. Easy is slow and distracted, Hard attentive but not
superhuman. This is the main difficulty dial; resources are never handicapped.

### 7. Economy (`economy.rs`)

- **Forecast**: income and stores a few minutes ahead from what stands, is going up
  and is committed.
- **Budget**: each think income is split between `Economy` (mines, upgrades, power,
  factories, storage, reclaim), `Army` (the solver's production), `Defence`, `Tech`
  and `Projects` (anything that is a large share of income: map guns, silos,
  titans, big ships), by the plans held.
- **Reserves**: a committed project books its cost ahead; the allocator saves
  toward it instead of starting it and stalling halfway.
- **Investment by payback**: an economic option's worth is its return over a
  horizon against its cost, discounted by risk (an exposed mine is worth less while
  the enemy's raiders are about). Energy is a cost like mass (`ENERGY_PER_MASS`).

The existing builder code (where mines, plants, factories and shields go, the base
layout, upgrades by payback, power ahead of need) stays as the executor: the
Commander decides how much goes to each, and the builders place it.

### 8. Production (`solver.rs`)

Plans and operations raise **needs**, not unit names: a vector of what is wanted
(`Line`, `AntiAir`, `Hunter`, `Transport`, `Siege` ...) with weights. For each idle
factory the solver scores everything in its menu: how much of the open needs it
covers, how it fares against the believed enemy composition (`matchup.rs`), per cost,
times the personality's taste, within the `Army` budget. Projects (site-built or a
large share of income) come through the `Projects` budget and a reserve.

### 9. Plans (`plans.rs`)

A plan is a way to win or not lose, held at a **stake**:

- `Probe`: cheap, tells the side something (a Courier with a few raiders, one bomber
  wing, a scout run, a single warship raid);
- `Invest`: a real share of income (a Bastion assault, a bomber fleet, a navy, a map
  gun under shields, a nuke);
- `AllIn`: the plan is the game (massed landings, several map guns, a titan, a nuke
  race with salvos sized past their interceptors).

Plans: `Pressure` (land army waves), `Raid`, `Landing`, `AirPower`, `SeaControl`,
`SubWar`, `Warships`, `Siege`, `Strategic`, `Titan`, `Fortify`, `Boom`
(economy first), `Tech`, `Intel`. Each plan has, per stake, the needs it raises,
the operations it runs and its budget share; and rules to **escalate** (its
operations traded well, the enemy has no answer seen) or **abandon** (traded badly
twice, the enemy answered it, it cannot be built). Sunk cost does not count: a plan is
judged on what more spending would buy.

Hedges are plans too, held at `Probe` against a belief: `Fortify` with an
interceptor slowly when `nukes` confidence is middling, `AirPower`'s anti-air when
`air` is.

Nukes, as the example the user gave: the `Strategic` plan considers its options:
**saturate** (salvo past the rounds their interceptors hold), **go around** (value
outside interceptor cover), **strip** (a landing or map gun aimed at the
interceptor first), and holds or fires accordingly.

### 10. Personalities

The four doctrines become portfolio tastes (`Taste`): risk appetite (how soon a
plan escalates and how far), preferred plans, and how many probes it runs.

| Doctrine | Character |
|---|---|
| Adaptive | balanced; answers what it sees |
| Aggressive | the Raider: many probes, early pressure, escalates on success fast |
| Economic | the Gambler: booms, then a big commitment (titan, nukes, massed landing) |
| Defensive | the Fortress: fortifies, sieges, hedges early against everything |

## Interfaces

```rust
// profile.rs
pub(super) struct Profile { cost: Fx, dps: [Fx; TARGETS], reach: [Fx; TARGETS],
    ehp: Fx, speed: Fx, domain: Domain, roles: u32, carry: u16, room: u16, ... }
pub(super) struct Profiles { by_id: Vec<Profile> }        // index = BlueprintId
// matchup.rs
pub(super) fn fight(a: &Profile, b: &Profile) -> Fx;      // > 1: a wins, per mass
// world_model.rs
pub(super) struct WorldModel { cells: .., threat: Vec<[Fx; TARGETS]>, value: Vec<Fx>,
    stale: Vec<u32>, ours: Vec<Fx> }
// beliefs.rs
pub(super) struct Beliefs { enemy: Vec<EnemyBelief> }     // in AiState, hashed
// ops.rs
pub(super) struct Operation { id: u32, kind: OpKind, plan: PlanKind, phase: Phase,
    units: Vec<UnitId>, target: FxVec2, rally: FxVec2, ledger: Ledger, since: u32 }
// economy.rs
pub(super) struct Budget { share: [Fx; BUDGETS], reserve: Fx, reserved_for: Option<..> }
// solver.rs
pub(super) struct Needs { want: [Fx; ROLES] }
// plans.rs
pub(super) struct Plan { kind: PlanKind, stake: Stake, since: u32, evidence: Ledger }
```

## Selecting the brain

`AiConfig::brain` (`Classic` or `Commander`), in match setup, snapshots and the hash;
`--ai-brain classic|commander` on the command line and a choice in the skirmish
set-up. The tournament plays one against the other in the same match. Once the
Commander beats the Classic AI across the tournament, the Classic decision code is
deleted and the switch goes with it (no parked code).

## Tournament (`scripts/ai-tournament.sh`, `tests/zz_ai_tournament.rs`)

Many headless matches in parallel: maps x seeds x seats swapped x brains x doctrines,
2-player and team games, land, island and mixed maps. Per match: winner and minute,
mass traded per domain, plans tried, stakes reached, escalations and abandons,
operations by kind and their trades, landings, warp jumps, nukes fired, time to first
fight. Aggregated into Elo per (brain, doctrine) and a fun sheet: strategy variety,
first contact, engagements per ten minutes, comebacks.

## Overlay

An observer toggle in game draws each AI's mind: plans and stakes, beliefs with
confidence, each operation's group, target and phase, and why. It is how the user
reads whether the reasoning is what they would want.

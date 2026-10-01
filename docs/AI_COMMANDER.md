# The Commander: a planning skirmish AI

Status (2026-10-01): built and playable beside the classic AI (`docs/AI.md`). Pick it
per seat with **Mind** in the skirmish set-up, or `--ai-brain commander`. The classic
AI is still the default and still the stronger of the two (see "Where it stands").
This document is the contract between the layers; code that disagrees with it is a
bug in one or the other.

## Why

The classic AI is one pass every second or two: count everything, pick a build job
per idle builder, send idle units at a target. It has no idea what it is trying to
do or whether it works, so every behaviour is a rule bolted onto the loop, and it
plays much the same game every time. What was asked for:

- commit to game plans, give up on them, push one harder, or run several at once:
  "a small courier raid with some things in the backline is different than a full
  bastion is different than culverins is different than 'don't know if they have
  nukes but they haven't done much, so I should scout, and probably start slowly
  building anti nuke'";
- no unit micro: "pulling back hurt units ... that's very non-human in a game of
  this scale". Groups get orders, the way a player gives them;
- never name a unit or a race: adapt to whatever roster it is given, a T5 included,
  "which is a MASSIVE commitment";
- balance and manage the economy as part of the plan, not beside it;
- fun first: judged by playing against it and watching it.

## Rules every layer keeps

- **Deterministic.** It runs inside the sim on every machine: `Fx`, integers, stable
  sorts, no hash maps, nothing from outside `State` and the blueprints. Its state is
  `AiState::commander` (`state.rs`), hashed and saved with snapshots and replays.
  `battle::a_commander_ai_plays_deterministically_and_restores_from_a_snapshot`
  checks a Commander match at 0 and 6 workers and across a mid-match restore.
- **No names.** Nothing branches on a unit key, mesh or race. A unit is what its
  blueprint says it can do (`profile.rs`), so a new unit, a T5 or the Regency's
  roster needs no AI work. Mechanics (warp, lift ships, diving, launchers) are read
  from blueprint fields.
- **Group orders only.** Every order goes to an operation's units together and is
  one a player could give: move, attack-move, board, unload, warp, launch, build,
  produce. No per-unit retreats or kiting; units fight with their own behaviour.
  The one exception kept from the classic AI is the commander unit itself, which
  stays home (a player guards their king).
- **Bounded.** Orders a minute are capped by difficulty (`orders_a_minute`: Easy
  24, Normal 50, Hard 100): operations rank by urgency and the rest keep their last
  orders. Resources are never handicapped.

## How a think runs (`commander.rs`)

Every AI think (about once a second) a Commander side runs, in order:

1. **Profiles** of every blueprint (pure function of the data), the **reach**
   flood (which ground its army can walk to), the **world model** and **beliefs**.
2. **Upkeep**: operations' ledgers credited with kills and losses; dead units
   dropped; what hurt the side fades (`hurt`, a minute and a half's losses count).
3. **Front line and rally**: the furthest own mine toward the enemy, stepped back
   until it is reachable and under no land threat; the rally holds still unless
   the front moves 500 m, comes under fire, or becomes unreachable. Shelling while
   a wave gathers pushes it back (`rally_back`), and it creeps forward when quiet.
4. **Plans** reviewed once a minute (`plans.rs`), then the standing operations
   each held plan wants (`keep_ops`), then idle units assigned to operations.
5. **Incursions and defence**: enemy groups on its ground get a `Defend`
   operation sized to beat them.
6. **Operations run** (`ops_*.rs`), most urgent first, within the order budget;
   then strategic launches.

Builders and factories stay the classic executors (`builders.rs`,
`production.rs`): the Commander tells them what to make through `Directives`
(`solver.rs`), its **wants** (structures and projects for builders,
`commander_job`) and **production** (`solve_production`).

## Layers

### 1. Profiles (`profile.rs`)

For every blueprint, from its data alone: mass, `dps` and `reach` for each target
class (`Land`, `Structure`, `Air`, `Surface`, `Submerged`, `Space`), with each
gun's share of hits by its flight time and spread against that class; effective
health, speed, radius, movement domain (land, hover, amphibious, naval, sub, air,
space); lift room; and **roles** derived from the above: `LINE`, `RAIDER`,
`ARTILLERY`, `ANTI_AIR`, `ANTI_NAVAL`, `HUNTER`, `TRANSPORT`, `SCOUT`, `STRIKE`,
`DEFENSE`, `MAP_GUN`, `STRATEGIC`, `PROJECT` (costs many minutes of income),
`BUILDER`. `profile_tests.rs` fails when an armed unit of any race has no role.

### 2. Matchups (`matchup.rs`)

`fight(a, b)`: who wins equal mass of `a` against `b`, Lanchester-style from the
profiles, with the range edge (free volleys while the gap closes) and splash.
`edge` compares a group with a believed enemy. Calibrated against staged headless
fights: `zz_matchup_probe` (in `matchup_tests.rs`) fights every pair of combat units
at equal mass and prints where the estimate and the sim disagree; the estimate picks
the winner of about 90% of pairs. Run it with
`MATCHUP=0/4 cargo test --profile gate -p mc-sim --lib -- zz_matchup_probe --ignored --nocapture`
(`MATCHUP_ONLY=key` for one unit's pairs).

### 3. World model (`world_model.rs`)

A 512 m grid rebuilt each think from the side's remembered contacts: enemy
`threat` reaching each cell per target class (map guns counted only to four cells,
or they hide everything), enemy `value` standing there (the commander weighs most),
and `ours`. `commander.seen` keeps the tick each cell was last in view (without fog,
every cell always is); scouts and plans read how stale each cell is.

### 4. Beliefs (`beliefs.rs`)

What the side thinks of the enemy, each a guess from what it has seen, where it has
looked and seen nothing, and what time and income make likely:

- `income` and `tech`; `army` by domain, a fifth over what was seen and never less
  than forty seconds of their income;
- `fortified` (turrets, map guns, artillery), `anti_air`;
- confidences 0..=100 that they field `air`, `navy`, `subs`, `space`, and `nukes`
  (seen: certain; unseen: rising with time, income and tech and with how quiet
  they have been, falling as their base is scouted without one);
- `interceptors` seen and where, `sonar` cover, `army_at` (last sighting),
  `base_unseen`, `quiet`.

`Sticky` facts never fade: the highest tier seen and when each kind of force was
last seen.

### 5. Plans (`plans.rs`)

A plan is a way to win or not to lose, held at a **stake**: `Probe` (cheap, tells
the side something), `Invest` (a real share of income), `AllIn` (the plan is the
game). Fourteen plans:

| Plan | Runs | Appeal rises with |
|---|---|---|
| `Pressure` | the land army in waves | a land route; being ahead on land; falls against a fortified enemy |
| `Raid` | fast groups at engineers, outlying mines, power | soft targets seen |
| `Landing` | lift ships carry a force over and put it down by a weak spot | islands; ground held by guns |
| `AirPower` | bombers and gunships, fighters over them | weak enemy anti-air; entrenched ground |
| `SeaControl` | a surface fleet | water between the sides; their ships; losses to ships |
| `SubWar` | submarines | their ships; thin sonar |
| `Warships` | armed spaceships by warp | income; weak anti-air; islands |
| `Siege` | artillery and map guns | a fortified enemy; a map gun it can afford |
| `Strategic` | nukes | income; no interceptors seen at a base looked at lately |
| `Titan` | one unit worth many minutes of income | high income; holding its own on land |
| `Fortify` | defences, shields, interceptors | nukes believed likely; raids on the base; their air |
| `Boom` | economy first | early game; a safe lead on land |
| `Intel` | scouts and sensor ships | an unseen base; nukes a maybe |
| `AirDefense` | anti-air at home and with the army | their air seen; losses to aircraft |

Every review (a minute) each plan's appeal is its situation, plus the doctrine's
taste, plus `HOLD` if held, plus its record. A stake is taken past its mark
(`PROBE_AT` 55, `INVEST_AT` 95, `ALL_IN_AT` 150 less the doctrine's risk) by a
`MARGIN` either way, and moves one step a review. Then:

- **escalate**: a plan whose operations traded at 3:2 or better gains appeal
  (`Working`);
- **abandon**: worse than 3:5 loses appeal, more each time it fails again
  (`Failing`); ten minutes at a real stake with nothing traded (`Stale`); the menu
  can no longer build it or the enemy answered it (`Answered`);
- **commit**: a stake taken in the last three minutes is not dropped on a dip in
  appeal, only on failure or an answer;
- **budget**: one plan all in at most, and the stakes' points (probe 1, invest 3,
  all-in 6) within the side's budget (Easy 4, Normal 7, Hard 9): the least
  appealing give way. Several plans at once is the normal state.

Hedges are just plans held low: `Fortify` at a probe builds an interceptor slowly
while `nukes` confidence is middling; `AirDefense` answers aircraft seen or felt.

Each change is a `Note` (tick, plan, stake, why), shown in the overlay.

### 6. Operations (`ops.rs`, `ops_ground.rs`, `ops_air_sea.rs`, `ops_space.rs`)

An operation is a group with one goal, owned by a plan:

| Kind | Goal |
|---|---|
| `Army` | the main land force: gather at the rally, go when strong enough, take objectives |
| `Guard` | a small fast home guard that answers raids first |
| `Defend` | answer an incursion, sized to beat it |
| `Raid` | fast units on soft targets away from their guns |
| `Landing` | load lift ships, jump (warp) beside a weak spot, unload, fight |
| `Strike` | aircraft at a target as one wing; interceptors stripped first when nukes are planned |
| `AirGuard` | fighters over home |
| `Fleet` | surface ships: their ships, then the coast |
| `Wolfpack` | submarines on ships where sonar is thin |
| `Warships` | armed spaceships: jump in, fight, go home |
| `Siege` | artillery and map guns on a target from beyond its defences |
| `Scout` | eyes where the beliefs are least sure, around anti-air |

Lifecycle: `Gathering` (at its rally; idle units of the right kind join) ->
`Executing` (group orders toward the goal) -> `Withdrawing` (the whole group, by
attack-move so it fights its way back) -> `Done`. Each keeps a **ledger** of mass
committed, killed and lost; finished operations hand it to their plan, and
`trades` keeps a running total per kind.

Ground rules learnt in the tournaments (each is a comment at its code):

- a wave goes only at a third more than the land army it believes in, with the
  anti-air the enemy's air calls for, and never blind: their army seen, or their
  base seen lately with none in it;
- never into a turret belt it cannot take; a probe goes only at soft targets;
- shelled while gathering: helpless against aircraft, it goes home; against
  ground guns it charges them or steps the rally back;
- a group that has lost half and is losing the trade, or is helpless, withdraws as
  a group; near the rally it turns and fights instead;
- a raid on the base takes the guard first, then gathering groups, then, if that is
  not enough, groups out in the field: the commander dying at home loses the game;
- a raid far from home is answered only by enough to win, or let go.

### 7. Production and economy (`solver.rs`, `builders.rs`, `energy.rs`)

**Force shares**: each force (land, air, surface, submarine) gets a share of new
units from the stakes of the plans that use it, plus what has been hurting the side
(losses to aircraft raise air defence). **Role shares** within a force: anti-air by
enemy air and by losses from above, artillery by `Siege` and by losses to guns,
raiders by `Raid`, the line for the rest. Each idle factory builds for the force
and role furthest under its share, choosing among its menu by `fight` against the
believed enemy composition, per cost. Nothing names a unit.

**Wants** (`choose_wants`): structures and projects the plans call for, sized in
minutes of income by stake (probe 2, invest 4, all-in 8): interceptors by the nukes
belief, lift ships, silos, map guns, titans, warships, shields, coastal guns where
ships hurt it, anti-air where it was bombed and by the `AirDefense` stake, a sensor
ship once income allows. The commander unit takes only wants at home.

**Economy**: the classic builders place mines, power and factories; the Commander
steers them:

- extra engineers by the `Boom` stake, but none while a materials stall leaves the
  ones it has waiting (building below 70%);
- mines claimed as far out as a classic side pushing out, while `Boom` or a real
  `Pressure` stake holds;
- power for what its factories and builders draw **at the speed materials pay
  for**: counting them at full draw in a stall built a third more plants than the
  classic AI and no mines for three minutes;
- tech 3 waits while its army is behind the one it believes in (never less than two
  minutes of their income): a tech 3 factory begun at half the enemy's army left
  nothing to hold the base with.

### 8. Personalities

The four doctrines are tastes over the plans (`taste`): appeal added per plan and
risk (how readily it goes all in).

| Doctrine | Character |
|---|---|
| Adaptive | no bias; answers what it sees |
| Aggressive | raids, waves, landings, air and warships; little boom or fortifying; quick to go all in |
| Economic | boom, nukes, titans, landings; most willing to go all in |
| Defensive | fortify, siege, anti-air, nukes; no raids or landings; slow to go all in |

## Overlay: watching it think

In an observer match (skirmish with no human seat, or `--observe`), pick a
Commander side's eyes with its vision chip: the **AI Mind** card under the minimap
shows

- its plans, held ones first, each with a stake bar (orange: all in) and its appeal
  at the last review, then the next few it is weighing;
- its operations: kind, phase (orange while it is out executing), units, mass
  against what it wants, and enemy mass killed / own mass lost (red when losing);
- `SEEN`: minutes since it last saw each kind of force (air, navy, subs, space,
  nukes; a dash for never) and `HURT BY`: what has been killing it lately;
- its last decisions: minute, plan, new stake and why ("looks good", "working",
  "losing trades", "answered", "hedge", "stale", "less appealing").

The data comes from `World::ai_mind` (`mind.rs`), sent to the HUD with each side's
status. Headless: `MERIDIAN_VISION=N scripts/shot.sh run --map dev16 --players 2
--observe --ai-brain commander --ticks 9000` (from WSL, pass the variable through
with `WSLENV=MERIDIAN_VISION`).

## Selecting the brain

`AiConfig::brain` (`Classic` default, or `Commander`) is in match setup, snapshots,
replays and the hash. In game: **Mind** on each AI seat of the skirmish set-up. On
the command line: `--ai-brain classic|commander` for every AI seat.

## Tournament (`scripts/ai-tournament.sh`, `tests/zz_ai_tournament.rs`)

Many headless matches in parallel, then wins, Elo and the plans each side held:

```bash
TOURNEY_MAPS="serac_divide:2 twin_shoals:2" TOURNEY_SEEDS=7 TOURNEY_MINUTES=25 scripts/ai-tournament.sh /tmp/t1
TOURNEY=serac_divide:2:25:7:commander/adaptive:classic/aggressive:hard cargo test --profile gate -p mc-sim --test sim -- zz_ai_tournament:: --ignored --nocapture
```

The switches (`TOURNEY_EVERY`, `TOURNEY_ROSTER`, `TOURNEY_DEATHS`, `TOURNEY_OPS`,
`TOURNEY_ARMY`, `TOURNEY_SNAP`) are listed in `docs/SWITCHES.md`; the script's
own in its header. Notes on reading the results:

- **The match seed does not vary games**: the same map, seats and doctrines play the
  same match. Vary maps, seats and the opponent's doctrine instead.
- **Seats are not fair** on every map: serac_divide and twin_shoals favour one start
  by a wide margin. Always play both ways round.
- A match that ends without a winner is scored by worth (standing mass): half again
  as much is a lead. The mean log of the worth ratio (`log((c+1k)/(o+1k))`) shows
  smaller shifts than wins do; an eliminated side counts as worth 0.
- `TOURNEY_ARMY=key*n,...` gives both sides the same army at the start: a mirror that
  isolates how each brain handles a fight from how it builds.

## Where it stands

Tournament runs during the build, Hard, the Commander (adaptive) against the
classic AI on four 2-player maps (dev16, meridian_basin, serac_divide,
twin_shoals), each of the classic AI's four doctrines, both seats, 25 minutes: 32
games a run.

| Run | Change | Score | Mean log worth ratio |
|---|---|---|---|
| t41 | army fights by its rally, aims at what is in front | 0.34 | -1.11 |
| t43 | engineers and power paid for, mine reach from plans | 0.34 | -1.06 |
| t44 | the commander unit stays home | 0.36 | -0.95 |
| t45 | base raids recall armies; tech 3 waits while behind | 0.43 | -0.33 |
| t47 | plans committed for three minutes | 0.43 | -0.32 |

Score: a win 1, a lead 0.75, a draw 0.5. The classic AI still has the edge, mostly
on serac_divide; dev16 and meridian_basin are now draws. Per-operation trades over
t47 (killed : lost): warships 5.7, defend 1.1, army 0.65, raids small. The
Commander loses its army's trades where it gathers, to hover raids, corvettes and
artillery, and still builds a thinner army than the classic AI at the same income.

Team maps (4v4 and 3v3 on The Axis, Halden's Grip, Serac Sound, Vermilion Gorge):
RESULTS_TEAM

What it does that the classic AI does not:

- runs several plans at once and changes them for reasons it states;
- lands armies by lift ship on island maps, raids by warp with spaceships, sends
  submarines where sonar is thin, scouts where it is least sure;
- answers what hurts it: anti-air where it was bombed, coastal guns where ships
  shell it, artillery against turret belts;
- hedges: an interceptor slowly while their nukes are a maybe.

Open (the next things to build):

- **army trades**: gather nearer cover (turrets, shields) without giving up the
  front; meet raids on the gathering wave with the wave;
- **army share**: the classic AI fields about twice the army at the same income by
  minute 14; the economy still over-builds engineers and factories;
- **Easy and Normal** have not been tuned; the order budget is the dial;
- **the Regency**: plays through the same profiles, not yet tournament-tested on its
  own roster;
- once the Commander beats the classic AI across the tournament, the classic
  decision code and the `brain` switch are deleted (no parked code).

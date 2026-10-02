# The Commander: a planning skirmish AI

Status (2026-10-01): built and playable beside the classic AI (`docs/AI.md`). Pick it
per seat with **Mind** in the skirmish set-up, or `--ai-brain commander`. The classic
AI is still the default; since the long-game round on 2026-10-01 the Commander is
the stronger of the two on Hard (see "Where it stands").
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
  The one exception is the commander unit itself (a player guards their king):
  it stays home, and under fire it cannot answer, or worn under three fifths of
  its health, it walks out past the shooters' reach for half a minute (`king.rs`).
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

A plan's record is its operations' kills and losses, credited as they happen
(standing operations such as the air guard never finish, so a plan that heard
only from finished ones never learnt it was losing).

Big projects (titans, silos, map guns, warships) grow in appeal with income
(`rich`, up to 50 from 80 a second on): at 300 a second a 16 000-mass warship
is under a minute of income. A probe of such a plan builds one when it costs
under two minutes of income.

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
- a raid far from home is answered only by enough to win, or let go;
- one army, not a trickle: while a wave is out fighting, what gathers behind it
  joins it once it is worth a thousand mass (waves of two and three thousand met a
  five-thousand army one after another);
- a wave in the field moves on once its middle stands at the target with nothing
  to fight, or after five minutes with nothing in front of it: a wave of 340 never
  read as "mostly idle" and stood by home for ten minutes;
- artillery (`Siege`) sets out only behind a wave already out near its spot, or
  where little of the enemy stands: alone it walked up to the guns and traded 1:6.

### 7. Production and economy (`solver.rs`, `economy.rs`, `builders.rs`, `energy.rs`)

**Force shares**: each force (land, air, surface, submarine) gets a share of new
units from the stakes of the plans that use it, plus what has been hurting the side
(losses to aircraft raise air defence). **Role shares** within a force: anti-air by
enemy air and by losses from above, artillery by `Siege` and by losses to guns,
raiders by `Raid`, the line for the rest. Each idle factory builds for the force
and role furthest under its share, choosing among its menu by `fight` against the
believed enemy composition, per cost. Nothing names a unit.

**Anti-air against what is in the air.** Anti-air production and the `AirDefense`
plan ease off once the side's anti-air outweighs the aircraft it has seen by half.
Aircraft whose guns outreach every mobile anti-air it can make and every anti-air
turret it has standing (`outranging_air`: corvettes' 1500 m rails against 480 m
anti-air tanks) do not count as covered: they raise the air force's share and its
fighters, and call for the longest-reaching anti-air turret it can afford.

**Wants** (`choose_wants`): structures and projects the plans call for, sized in
minutes of income by stake (probe 2, invest 4, all-in 8): interceptors by the nukes
belief (seen: one for each of their silos and one over, up to eight, each sited
over the richest ground none covers), lift ships, silos, map guns, titans,
warships, shields, coastal guns where ships hurt it, anti-air where it was bombed,
by the `AirDefense` stake and against outranging aircraft, a sensor ship once
income allows, and a turret by a mine or plant a ground raid took. The commander
unit takes only wants at home.

**Urgent wants** answer a threat the side is under now: interceptors once warheads
are seen, the long-reaching anti-air turret against warships out of its anti-air's
reach (counted by what they killed lately too, since warships that warp in and
out are seldom on the map at a review), and the raid turret. Builders take them
before power and everything else, and when no idle builder can make one, a busy
engineer that can is pulled onto it (`urgent_builder`).

A force the plans want with no factory to make it (a shipyard for a sea plan, an
airfield for the air force) gets one even while the side is stalling, when no
other factory would be started.

**Economy** (`economy.rs`): a controller run at the top of every think, before the
builders, factories and upgrades it steers. It balances three things all game:

- **Never float.** A full materials store throws away what the mines dig. A store
  filling up (its fill smoothed over a few thinks, past 40%) calls for sinks: mine
  upgrades first (they pay back; more at once, and a longer payback allowed), a
  factory ahead of more far mines, then more builders.
- **Never stall energy.** Mines dig less while their upkeep goes unpaid. Power is
  kept ahead of spending the whole income at the difficulty's energy ratio (8 a
  unit of mass on Hard; costs run 4 to 7) plus the upkeep of everything standing
  or going up, a tenth over. Wanted: plants come after the mine claims at home;
  urgent (the store under 15%, or mines short of upkeep): plants come first,
  every builder helps, expanders included, and new mine upgrades wait. Only as
  many plants are started as the shortfall less what is already going up.
- **Build power matched to income.** Factories get the army's share of the income
  (half, more with a push on, less while booming), builders the rest. No new
  engineers or factories while building runs under 80% of full speed. One more
  engineer while every builder is busy and power, ore or a factory is waiting,
  up to twice what the builders' share could keep at full speed.

And it grows:

- **Expanders**: one to three engineers set apart (more on a boom, more with
  plenty of ore) claim free ore on the side's half one mine after another,
  nearest first. A deposit no lot could be found by is skipped for three minutes.
- **The opening** follows the user's: the commander puts up a factory, its first
  mine at home, then power; the factory's first engineer goes straight out as an
  expander, then a reclaimer (one early, more as income grows) before scouts.
- **The commander roams** for its first twelve minutes: mines and reclaim out to
  two fifths of the way to the enemy (2.5 km at most), home as soon as 200 mass of
  armed enemy is within 1.6 km of it, or its health falls under 70%.
- **Tech 3** waits while its army is behind the one it believes in.
- **Mine upgrades are never put last** (`direct_focus`): begun as the sink for a
  filling store, they once went last behind factories asking ten times the
  income and none finished in thirteen minutes. Whether an upgrade can be powered
  is judged by the energy spent, not asked for (`can_fund`), for every AI.

The overlay's ECONOMY lines show all of it: floating, stalling or balanced, store
and build speed, the power call, engineers and factories against what it wants,
expanders and free ore, mine upgrades, and how far the commander may roam.

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
| t51 | the economy controller (`economy.rs`) | 0.48 | -0.24 |

**Long games** (2026-10-01, after a user's 88-minute match the Commander lost:
it out-earned the classic AI 2:1 and built no project, held anti-air all in for
forty minutes and kept three interceptors against five silos). The same pairings
played to the end: 40 games of 60 minutes on five maps (adding Frostline), then
64 games of 45 minutes on eight (adding Meridian Crown, Vermilion Gorge and
Halden's Grip). A side eliminated counts 0, so the mean log worth ratio swings
far more than over 25-minute games.

| Run | Change | Games | Score | Mean log worth ratio |
|---|---|---|---|---|
| base | as t51, played long | 40 | 0.43 | -1.24 |
| r1 | upgrades never last, `can_fund` by spend; live plan records; anti-air by the air seen; interceptors by silos; projects by income | 40 | 0.50 | +0.27 |
| r2 | the commander's own retreat; aircraft that outreach its anti-air | 40 | 0.56 | +0.85 |
| r4 | one army, not a trickle; a missing shipyard or airfield; anti-air only against what it reaches | 64 | 0.60 | +1.03 |
| r6 | waves move on; urgent wants pull a builder; escorted artillery; raid turrets | 64 | 0.66 | +1.82 |
| r7 | bombers go for the artillery shelling it | 64 | 0.66 | +1.70 |

By map at r7: Frostline 0.97, Halden's Grip 0.84, Vermilion Gorge 0.81, Meridian
Basin 0.75, Meridian Crown 0.56, dev16 0.53, Serac Divide 0.44, Twin Shoals 0.38.
Operation trades at r7 (killed : lost): strike 5.2, air guard 3.4, warships 2.5,
defend 2.5, army 0.62, siege 0.23. One run to the next swings a map by up to 0.4
with no change near it: outcomes turn on eliminations, so judge a change by the
whole run and its trades.

The economy against the classic AI's (adaptive both, 8 games of 20 minutes on the
same four maps, means; "dry" is an energy store under 10% with building slowed):

| | Commander | Classic |
|---|---|---|
| Income at 10 / 15 / 20 min | 43 / 74 / 142 a s | 37 / 73 / 96 a s |
| Build speed (share of full speed paid) | 66-75% | 42-56% |
| Player-minutes floating (store 90% full) | 4 of 160 | 2 of 160 |
| Player-minutes dry of energy | 16 of 160 | 3 of 160 |
| Army mass at 20 min | 20.5k | 14.3k |

Score: a win 1, a lead 0.75, a draw 0.5. The classic AI still has the edge, mostly
on serac_divide; dev16 and meridian_basin are now draws. Per-operation trades over
t47 (killed : lost): warships 5.7, defend 1.1, army 0.65, raids small. The
Commander loses its army's trades where it gathers, to hover raids, corvettes and
artillery, and still builds a thinner army than the classic AI at the same income.

Team maps (t48: 4v4 on The Axis, Halden's Grip and Serac Sound, 3v3 on Vermilion
Gorge, against the classic AI's adaptive and aggressive doctrines, both sides, 30
minutes): 12 of 16 drawn, score 0.36; the four losses are all on Serac Sound, where
the Commander's whole team was eliminated. There its operations trade well: army
1.3, fleet 1.7, defence 1.9, and it warps 20 to 70 times a match.

Commander against Commander (t50: dev16, serac_divide and The Axis, aggressive
against economic and defensive against adaptive, both ways, 30 minutes): the
doctrines play different games. Aggressive held its waves all in and raided
(score 0.62); Defensive fortified and sieged (0.50); Adaptive mixed waves, raids,
landings and fleets (0.50); Economic went all in on nukes, titans and landings
(0.38): it has a nuke silo up by minute 30 on serac_divide.

What it does that the classic AI does not:

- runs several plans at once and changes them for reasons it states;
- lands armies by lift ship on island maps, raids by warp with spaceships, sends
  submarines where sonar is thin, scouts where it is least sure;
- answers what hurts it: anti-air where it was bombed, coastal guns where ships
  shell it, artillery against turret belts;
- hedges: an interceptor slowly while their nukes are a maybe.

Open (the next things to build):

- **army trades** (0.6 over a whole tournament): it loses its land fights to
  artillery parks that outreach it (T2 missile trucks at 1400 m, T3 artillery at
  1800 m), to cheap bombers and gunships at its rally, and to corvette raids;
  gather nearer cover (turrets, shields) without giving up the front;
- **siege** trades 0.2: falling back at a third (not twice) of what it faces made
  no difference (r8), so the loss is elsewhere, likely in where it stands;
- **Twin Shoals and Serac Divide**: small sea maps where corvette and boat raids
  decide early games;
- **energy**: still short more often than the classic AI (which keeps its store
  full by overbuilding), early while expanders outrun the plants and late when
  tech 2 and 3 mines raise upkeep sixfold;
- **Easy and Normal** have not been tuned; the order budget is the dial;
- **the Regency**: plays through the same profiles, not yet tournament-tested on its
  own roster;
- once the Commander beats the classic AI across the tournament, the classic
  decision code and the `brain` switch are deleted (no parked code).

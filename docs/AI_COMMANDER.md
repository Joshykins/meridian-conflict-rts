# The Commander: a planning skirmish AI

Status (2026-10-01): the skirmish AI. It plays every AI seat; its settings and the
base code it steers are in `docs/AI.md`. It was built beside the rule-driven
"classic" AI and replaced it once it beat it on Hard in long games (see "Where it
stands"); the classic AI is in the git history.
This document is the contract between the layers; code that disagrees with it is a
bug in one or the other.

## Why

The classic AI it replaced was one pass every second or two: count everything,
pick a build job per idle builder, send idle units at a target. It had no idea what it was trying
to do or whether it worked, so every behaviour was a rule bolted onto the loop, and
it played much the same game every time. What was asked for:

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

Builders and factories are executors (`builders.rs`, `production.rs`,
`upgrades.rs`; `docs/AI.md`): the Commander tells them what to make through `Directives`
(`solver.rs`), its **wants** (structures and projects for builders,
`commander_job`) and **production** (`solve_production`).

## Layers

### 1. Profiles (`profile.rs`)

For every blueprint, from its data alone: materials, `dps` and `reach` for each target
class (`Land`, `Structure`, `Air`, `Surface`, `Submerged`, `Space`), with each
gun's share of hits by its flight time and spread against that class; effective
health, speed, radius, movement domain (land, hover, amphibious, naval, sub, air,
space); lift room; and **roles** derived from the above: `LINE`, `RAIDER`,
`ARTILLERY`, `ANTI_AIR`, `ANTI_NAVAL`, `HUNTER`, `TRANSPORT`, `SCOUT`, `STRIKE`,
`DEFENSE`, `MAP_GUN`, `STRATEGIC`, `PROJECT` (costs many minutes of income),
`ELEVATOR` (a space elevator, from its `elevator:` data), `BUILDER`. `profile_tests.rs` fails when an armed unit of any race has no role.

### 2. Matchups (`matchup.rs`)

`fight(a, b)`: who wins equal materials of `a` against `b`, Lanchester-style from the
profiles, with the range edge (free volleys while the gap closes) and splash.
`edge` compares a group with a believed enemy. Calibrated against staged headless
fights: `zz_matchup_probe` (in `matchup_tests.rs`) fights every pair of combat units
at equal materials and prints where the estimate and the sim disagree; the estimate picks
the winner of about 90% of pairs. Run it with
`MATCHUP=0/4 cargo test --profile gate -p mc-sim --lib -- zz_matchup_probe --ignored --nocapture`
(`MATCHUP_ONLY=key` for one unit's pairs).

### 3. World model (`world_model.rs`)

A 512 m grid rebuilt each think from the side's remembered contacts: enemy
`threat` reaching each cell per target class (map guns counted only to four cells,
or they hide everything), enemy `value` standing there (the commander weighs most),
and `ours`. `commander.seen` keeps the tick each cell was last in view (without fog,
every cell always is); scouts and plans read how stale each cell is.

Past 800 owned units a side's think period grows (one period slower per 800 units,
four times at most; `think_scale` in its state, so a restored match thinks on the same
ticks) and the fog pass is spread over that many thinks. `Profiles` are built once and
shared (`scratch.rs`, never saved). The work a think spent (cells and contacts walked)
is on the overlay card and in the `ai.world_model_work` perf counter.

### 3b. Zones (`zones.rs`)

The whole map, land and water, split once into zones that know their neighbours.
There is one zone map for the match, shared by every side and the observer
(`ai::Shared`), so a zone id is the same place to all of them. It is built from the
pathing grid's terrain classes, which never take the terrain's later edits, so it is
the same on every machine and is rebuilt after a restore rather than saved.

- **Seeds**: every start, then every ore field (seeds within 400 m of a zone on the
  same ground join it), then a lattice every 1.5 km over ground no seed claims
  (open sea, bare land), skipped within 1 km of another seed. Zones grow over their
  own ground a ring at a time; a cliff between two nodes stops them. Ground no seed
  reached (an island with no ore, a lake) gets a zone of its own when it holds 4
  nodes or more. At most 1024 zones.
- **Links**: `adj` joins zones on the same ground a unit can cross between; `coast`
  joins land to the water it touches.
- Each think a `ZoneView` reads what changes from the world model and contacts: land
  fire, enemy materials, and the materials of our own structures in each zone.
- **Value**: 250 materials per ore field plus the enemy materials remembered in the zone;
  incursions are answered most valuable first. Target choice (`land_objective`) keeps
  the old "materials within 600 m" value and subtracts the worst fire on the zones between
  (`fire_along`): ranking by a whole zone's materials or ore sent waves at the strongest
  ground (decided matches fell from 11 of 16 to 6-9 in the 2x-resource ARC mirror).
  Scouts revisit ground with ore sooner than bare ground.
- **Shortfalls**: a defence raised against aircraft, shelling or raiders posts what it
  found missing (`commander.role_need`: anti-air, artillery, raiders, per mille) as a
  floor on that role's share of new units, fading over a few minutes (`solver.rs`).
- **Hedging nukes**: with the nukes only a maybe (40-89), an interceptor is raised
  only when one zone holds 11 000 materials of our structures.
- **Opening**: starts close together raise Pressure (a rush), free ore on its half
  raises Boom for the first 12 minutes.
- A structure the plans' wants found no lot for is skipped at that place for three
  minutes (`eco.blocked_wants`), as a deposit is.

### 3c. Knowledge (`knowledge.rs`)

The side's long memory of the map, by zone, saved with the match. Contacts are its
short memory of single units (the freshest 256, forgotten in seconds to minutes);
knowledge keeps, for every zone, what each enemy player was seen to hold there:

- structure materials and count by kind (`Built`: economy, factories, airfields,
  shipyards, guns, anti-air, artillery, launchers, interceptors, shields, sensors,
  other), and when they were last refreshed;
- armed mobile materials by domain, builders, and when they were last refreshed;
- when their commander was last seen there;
- per zone: when it was last mostly in view, how much of it is in view now, and the
  enemy radar blips there it cannot name.

Each think (`learn_zones`, from everything the side detects, before the contacts are
cut to 256) a zone is refreshed by the share of it in view, sampled every other node:
wholly in view, what is seen now replaces what was known; half in view, half of the
old is kept; out of view, the old stands. Something on radar is never less than
what was known. When read, structures are sure for ten minutes and fade to 40% at
thirty; an army is sure for twenty seconds and is forgotten at three minutes.

### 3d. Fronts (`fronts.rs`)

Where the side's ground meets an enemy's, found each think, one set per enemy
player, on land and at sea:

- **Claims**: our structure materials in each zone, and half our armed mobile materials; the
  enemy's from knowledge, structures by how sure they still are and the army
  believed there now. An enemy start never seen is still claimed (1000). At sea, a
  water zone also takes half the claim of the land on its coast.
- **Reach**: each claim of 250 or more spreads over the zone graph by walk (or sail)
  distance between zone middles, starting ahead by a tenth of its materials in metres
  (1.5 km at most): a strong base pushes the border away from it.
- **Border**: a zone both sides reach within 250 m of each other, or one touching the
  other side's ground. A connected run of border zones is cut into sectors of four
  hops from one of its ends, so a border across the map is several fronts; a cliff
  or a strait breaks a run of itself.
- Each front reads: its zones, their zones beside it (`entries`), what is at stake
  behind it within two hops on each side (structures and ore), armed materials near it on
  each side (one hop), and how many zones lie from it to their most valuable ground.
  A front of two zones or fewer with less than 600 at stake on both sides is dropped;
  at most 48 are kept, those with most at stake.
- **Identity**: a front keeps its id from think to think by the zones it shares with
  a front of the last think against the same enemy on the same ground: the biggest
  pick first, so a front that splits keeps its id on its larger part and two that
  merge keep the older one (`CommanderState::fronts`).

On the overlay, fronts are outlined orange with their id and the armed materials near
each side; the AI Mind card lists the four with most at stake.

### 4. Beliefs (`beliefs.rs`)

What the side thinks of the enemy, each a guess from what it has seen, where it has
looked and seen nothing, and what time and income make likely:

- `income`: their mines known (from knowledge) at the yield the side's own mines
  give for their materials (so a higher tier counts for more), and never less than the
  time says; `tech`;
- `army` by domain: the most of what the contacts saw lately and what knowledge
  believes is standing now, a fifth over that and never less than forty seconds
  of their income;
- `fortified` (turrets, map guns, artillery: contacts, or the guns knowledge keeps,
  whichever says more), `anti_air`;
- confidences 0..=100 that they field `air`, `navy`, `subs`, `space`, and `nukes`
  (seen: certain; unseen: rising with time, income and tech and with how quiet
  they have been, falling as their base is scouted without one);
- `interceptors` seen and where, `sonar` cover, `army_at` (last sighting),
  `base_unseen` (their start's zone looked at counts), `silos` (contacts or
  knowledge);
- `unaccounted`: what their income is believed to have bought over the match
  (`commander.earned`, a running sum) less everything known of theirs standing and
  everything the side has killed (`commander.killed`): what they may be building
  out of sight. `quiet` is ten minutes in with four minutes of their income
  unaccounted for.

The AI Mind card's THEM line shows income, army, guns and unaccounted materials.

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
| `Siege` | artillery and map guns (a gravitic engine counts as one) | a fortified enemy; a map gun it can afford |
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
  all-in 6) within the side's budget: 9 at 50 materials a second and under, a point
  more for every 20 above that, 40 at most (`budget_for`). The least appealing give
  way. Several plans at once is the normal state, and a richer side runs more: held
  to a fixed number, one earning 300 a second ran the same three plans as one
  earning 40. (Difficulty no longer sets it.)
- **more at once when rich**: the raids, landings and sieges a stake opens, and the
  silos, map guns, titans and warships it keeps, are multiplied by `scale`: one at 60
  a second and under, a half more for each 60 above, five at most. The groups stay
  the same size: groups waiting for five times the materials set out too late and
  traded worse at three times the income.

Hedges are just plans held low: `Fortify` at a probe builds an interceptor slowly
while `nukes` confidence is middling; `AirDefense` answers aircraft seen or felt.

A plan's record is its operations' kills and losses, credited as they happen
(standing operations such as the air guard never finish, so a plan that heard
only from finished ones never learnt it was losing).

Big projects (titans, silos, map guns, warships) grow in appeal with income
(`rich`, up to 50 from 80 a second on): at 300 a second a 16 000-materials warship
is under a minute of income. A probe of such a plan builds one when it costs
under two minutes of income.

Each change is a `Note` (tick, plan, stake, why), shown in the overlay.

### 6. Operations (`ops.rs`, `ops_ground.rs`, `ops_titan.rs`, `ops_air_sea.rs`, `ops_space.rs`)

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
| `Titan` | the side's titans (lot-built fighting units on land or hover): out alone at once, on an attack-move |

Lifecycle: `Gathering` (at its rally; idle units of the right kind join) ->
`Executing` (group orders toward the goal) -> `Withdrawing` (the whole group, by
attack-move so it fights its way back) -> `Done`. Each keeps a **ledger** of materials
committed, killed and lost; finished operations hand it to their plan, and
`trades` keeps a running total per kind.

Ground rules learnt in the tournaments (each is a comment at its code):

- a wave goes once it matches the land army it believes in (a third more held
  waves at their rally point most of the match), with the anti-air the enemy's air calls for, and never blind: their army seen, or their
  base seen lately with none in it; three times the army it believes in, gathered
  three minutes with nothing seen, goes all the same, at their start;
- scouts belong to the scouting operation only: the army and raids, filled first,
  used to take every one of them, and a side with 25 scouts saw nothing for
  twenty minutes;
- scouts on foot go to the walkable ground nearest the place to look at, and skip
  places with none: sent at cell centres on mountains, a side's scouts never left
  home in a 19-minute match;
- never into a turret belt it cannot take; a probe goes only at soft targets;
- targets come from the long memory as well as the contacts: a zone where enemy
  structures are known (mines and power, for a raid) is a target long after its
  contacts are forgotten, scored the same way (`remembered_objectives`);
- the waves gather behind the land front with most at stake (what it holds there
  and half what it could take), in our zone at its back (`front_rally`), when the
  army can walk there and no fire reaches it; with no fronts yet, on the line
  between the starts as before. It stays behind that front until another has half
  as much again at stake: picked afresh each think, the rally point jumped
  kilometres between near-level fronts and no wave ever stood gathered;
- shelled while gathering: helpless against aircraft, it goes home; against
  ground guns it charges them or steps the rally back;
- a group that has lost half and is losing the trade, or is helpless, withdraws as
  a group; near the rally it turns and fights instead;
- a raid on the base takes the guard first, then gathering groups, then, if that is
  not enough, groups out in the field: the commander dying at home loses the game;
- a raid far from home is answered only by enough to win (a third more than the
  raiders, and a little), or let go;
- a wave that cannot fill goes with three quarters of its mark after two minutes
  gathering and with half after four: a Hard side on 7 a second waited a whole
  match for a 380-materials wave and never attacked. The mark is never more than
  half as much again as the enemy army it believes in: sized by income alone, it
  chased an income that grew faster than the army, and 141 units sat gathering
  for 21 minutes;
- a raid is filled to its mark and no further, as the guard is: filled first and
  without end, raids left the army 1 to 3 units for its first 12 minutes;
- a wave still gathering takes on an enemy group near its rally only once it holds
  half its mark: one tank charging alone read as a trickle;
- a wave that took its target goes on to the next structure it beats, by the
  same checks as at the rally point, or holds the ground it took, and what gathers
  behind it joins it there; it no longer walks home after every win;
- a titan is the titans' own, never the guard's or the army's: in the guard a
  Behemoth stood among the mines for a whole match. It sets out the moment it is
  built, takes a nearer objective found on the way, and falls back only below 30%
  health and outmatched two to one; its own guns outrange what it meets (a Behemoth
  razed two shields, four tech 3 turrets and a factory from 2.4 km in ten seconds);
- a gravitic engine (`direct_gravitic`) puts its well on the enemy spot worth most
  inside one (their commander above all, as a warhead does; aircraft at the share of
  the crush they take), looks again once the well has held twenty seconds, and
  turns when what it holds is worth less than half the best spot; two engines never
  share a spot. Unaimed, the AI's Fallen never once opened a well;
- a side with a finished space elevator (`Ctx::cheap_army`) gets its units free, so
  its army goes as a probe does (at three fifths of the enemy army, blind), is
  never held at home, and does not break off a fight for the trade: with 50 000
  materials of free tanks at its rally, a side waited five minutes, then fell back
  from an even fight with 136 units standing;
- an army held at home (Pressure off) goes once it holds a minute of income, at
  least 3000 materials, and that mark never shrinks with waiting: held at nothing, two
  aggressive sides on an island map kept 436 000 and 122 000 materials at home;
- on a side with no land route its army is its hovercraft, and a target counts
  as reached where a hovercraft can float (`army_reaches`): asked only whether
  home's ground reached them, a hover army of 663 never found a target;
- the tier path never takes the side's only factory: upgrading it stops the army;
- one army, not a trickle: while a wave is out fighting, what gathers behind it
  joins it once it is worth a thousand materials (waves of two and three thousand met a
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

**Space elevator.** Not a plan but an investment: one is wanted once the side's
materials and energy income would each pay for it in `ELEVATOR_MINUTES` (12), a second
at twice that income, and so on, and its site starts on the same horizon (other
wants start on four minutes). While it rises it takes the builders the income can
keep busy on it, called home from anywhere (`elevator_short_of_help`; expanders
keep claiming mines). Standing, its queue is filled each fall with a whole payload
of combat units (`direct_factories`); it is not counted as one of the base's
factories.

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
  unit of materials on Hard; costs run 4 to 7) plus the upkeep of everything standing
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
  two fifths of the way to the enemy (2.5 km at most), home as soon as 200 materials of
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
- its operations: kind, phase (orange while it is out executing), units, materials
  against what it wants, and enemy materials killed / own materials lost (red when losing);
- `SEEN`: minutes since it last saw each kind of force (air, navy, subs, space,
  nukes; a dash for never) and `HURT BY`: what has been killing it lately;
- its last decisions: minute, plan, new stake and why ("looks good", "working",
  "losing trades", "answered", "hedge", "stale", "less appealing").

The card's **Map** switch draws the side's zones on the ground (`mind_map.rs`):

- **Mind**: what it believes. Its own zones are tinted its colour, each enemy zone its
  holder's colour, fading as what it knows of the zone ages; ground it has never had
  eyes on is dark. A dashed ring at a zone's middle is an army it believes in, fading
  with how sure it is. Each held zone is tagged: main base, forward base, firebase,
  expansion, army, builders or outpost.
- **Truth**: what every player really holds there, counted the same way.
- The zone under the pointer gets a card: everything known of it (structures by kind,
  how long ago and how sure, the army seen and believed now, builders, their
  commander), and in Mind, what is really there beside it.

The zones and their truth are published only while the switch is on
(`Watch::zones`, `World::ai_zone_map`, `World::ai_zone_truth`).

The data comes from `World::ai_mind` (`mind.rs`), sent to the HUD with each side's
status. Headless: `MERIDIAN_VISION=N scripts/shot.sh run --map crosswater --players 2
--observe --ticks 9000` (from WSL, pass the variable through
with `WSLENV=MERIDIAN_VISION`).

## Tournament (`scripts/ai-tournament.sh`, `tests/zz_ai_tournament.rs`)

Many headless matches in parallel, doctrine against doctrine, then wins, Elo and the
plans each side held:

```bash
TOURNEY_MAPS="serac_divide:2 twin_shoals:2" TOURNEY_SEEDS=7 TOURNEY_MINUTES=25 scripts/ai-tournament.sh /tmp/t1
TOURNEY=serac_divide:2:25:7:adaptive:aggressive:hard cargo test --profile gate -p mc-sim --test sim -- zz_ai_tournament:: --ignored --nocapture
```

The switches (`TOURNEY_EVERY`, `TOURNEY_ROSTER`, `TOURNEY_DEATHS`, `TOURNEY_OPS`,
`TOURNEY_ARMY`, `TOURNEY_SNAP`) are listed in `docs/SWITCHES.md`; the script's
own in its header. Notes on reading the results:

- **The match seed does not vary games**: the same map, seats and doctrines play the
  same match. Vary maps, seats and the opponent's doctrine instead.
- **Seats are not fair** on every map: serac_divide and twin_shoals favour one start
  by a wide margin. Always play both ways round.
- A match that ends without a winner is scored by worth (standing materials): half again
  as much is a lead. The mean log of the worth ratio (`log((c+1k)/(o+1k))`) shows
  smaller shifts than wins do; an eliminated side counts as worth 0.
- `TOURNEY_ARMY=key*n,...` gives both sides the same army at the start: a mirror that
  isolates how each side handles a fight from how it builds.

## Where it stands

Tournament runs while the classic AI still stood, Hard, the Commander (adaptive)
against the classic AI on four 2-player maps (the old dev16, meridian_basin, serac_divide,
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
Basin 0.75, Meridian Crown 0.56, dev16 (retired) 0.53, Serac Divide 0.44, Twin Shoals 0.38.
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
| Army materials at 20 min | 20.5k | 14.3k |

Score: a win 1, a lead 0.75, a draw 0.5. The classic AI still has the edge, mostly
on serac_divide; dev16 (retired) and meridian_basin are now draws. Per-operation trades over
t47 (killed : lost): warships 5.7, defend 1.1, army 0.65, raids small. The
Commander loses its army's trades where it gathers, to hover raids, corvettes and
artillery, and still builds a thinner army than the classic AI at the same income.

Team maps (t48: 4v4 on The Axis, Halden's Grip and Serac Sound, 3v3 on Vermilion
Gorge, against the classic AI's adaptive and aggressive doctrines, both sides, 30
minutes): 12 of 16 drawn, score 0.36; the four losses are all on Serac Sound, where
the Commander's whole team was eliminated. There its operations trade well: army
1.3, fleet 1.7, defence 1.9, and it warps 20 to 70 times a match.

Commander against Commander (t50: dev16 (retired), serac_divide and The Axis, aggressive
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
- the tournament now plays doctrine against doctrine; a fixed earlier build as the
  yardstick (run from its own worktree) is how to tell whether a change helps.

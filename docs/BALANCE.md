# Economy and tier balance

The rules the Aster numbers follow. Change the rule, then the numbers, not one unit at a time.
How to play them, and the economy race that measures it, is in `ECONOMY.md`.

## Tiers

Tier 1 is the anchor: its combat stats set the scale. Each tier costs about 4x the one below
(mass) and is worth about 1.3x as much per unit of mass, measured as
`sqrt(health * dps) / mass` against the tier 1 unit of the same line. So a tier 2 unit is about
5x a tier 1 unit, and a tier 3 about 25x. Health and damage are raised together (never lowered)
to reach the target; range specialists (SAM, strategic bomber) are capped at 3x.

Lines: tank (Warden -> Bulwark and Skimmer -> Paladin), artillery, mobile AA,
fighters, bombers, gunships, point defence, static AA. Aircraft and anti-air follow the
rules in "Air and anti-air" below instead: they are measured by what one pass or one shot
kills, which `sqrt(health * dps)` cannot see.

Off the lines, measured the same way:

- Arbalest (T3 lightning sniper, 560 mass): about 1.7 per unit of mass against the
  Paladin's 2.7. It is a range specialist (520 m against the Paladin's 280) and cannot
  defend itself up close, so it sits under the rule on purpose.
- Fulgur (T4 super-heavy tank, 3400 mass, about 4x a Paladin): 60000 health plus an
  18000 hull field, about 1067 direct dps from the AEB-2 and two bolt rifles, so about 2.6 per
  unit of mass, near the Paladin's. What puts it over is the AEB-2's channel: 2000 damage to
  everything within 7 m of it, which a column or a clump pays for many times. Raised on a
  lot by Mason IIIs (build power 150: one takes about 135 s); it has no factory.

- Strategic weapons (`docs/NUKES.md`): the Sunfall silo (T4, 9000 mass) assembles warheads
  of 6000 mass / 120000 energy in about 300 s at its own power (60), holds 2, and each does
  80000 inside 200 m falling to 2500 at 520 m: it erases a base's core, domes included. The
  Parhelion array (T3, 3200 mass) answers it at a quarter of the price a round (1500 mass,
  about 100 s), four held, covering marks within 2.4 km. A side that sees silos should
  build arrays; one array stops one warhead per interceptor it holds.
- The Culverin (T4 map gun, 10000 mass) shells bases from 1.5 to 22 km: 5200 damage in
  45 m every 9 s, about 580 dps on paper. It shells the nearest structure, land unit or
  ship its side sees or has on radar, and a 1 degree spread lands its shells evenly over a
  disc of about 210 m radius at 12 km (140 m off on average, `tests/culverin.rs`), so by
  area only about one shell in ten lands on the building it was aimed at, the rest on
  what stands round it. A dome is the answer: a T2 dome (9000, 90/s) goes down in about 20 s of hits, a T3
  dome (36000, 360/s) holds one gun off for nearly 3 minutes and two for under one.

## Air and anti-air

Set 2026-09-30 against Forged Alliance (FAF), whose land units our T1/T2 already match
almost exactly (a T1 tank is 56 mass / 300 health in both). Aircraft raid and live through
a pass; anti-air punishes them over seconds, not in one shot.

- **No AA shell or bullet one-shots a same-tier aircraft.** Flak keeps its wide splash
  (it still takes a whole bunched flight), but a shell does less than a T1 aircraft's health:
  Squall 200 a shell, Barrage 2 x 220; T1 aircraft have 160-300. A T1 AA gun takes about
  5 s (Gnat) or 2 s (Sparrow) to down a T1 bomber (FAF: 8 s / 3 s).
- **AA outreaches the ground guns of its tier**, so it can stand behind the line: Gnat 360 m
  and Sparrow 480 m against a T1 tank's 300 m. T3 AA still buys reach, not efficiency.
- **Fighters are about 1.4x faster than the bombers of their tier** and tougher than them,
  so they catch raids (Shrike 120 against the Petrel's 84, 250 health against 250).
  A higher-tier fighter's shot kills a lower-tier fighter in one or two hits, which is
  where its tier gap shows.
- **Bombers raid.** The Petrel's one pass (two 130 bombs together, 20 m blast) takes a
  knot of engineers or light AA but leaves a T1 tank standing; mines and reactors take
  several bombers or passes. The Eclipse is a glass hammer: 5000 a bomb (a T2 mine in one,
  a bare commander in three), 6000 health, so two Skyguard volleys or about 9 s of one
  Raptor bring it down.
- **Gunships hit like the ground of their tier, not harder.** Damage per mass against the
  same-tier tank: FAF's gunships 0.6-0.8x. The Kestrel is about 0.8x (it killed T2 tanks
  at 3x before); the Thunderhead is 0.6x our Paladin line and stays as it is.

Measure in the sim with `zz_aa_probe` (what each AA lands on each aircraft) and
`zz_petrel_probe` (Petrel passes on engineers, reactors, mines and tanks).

## Energy per mass

| kind | T1 | T2 | T3 |
|---|---|---|---|
| land units, engineers | 5 | 6 | 8 |
| aircraft | 15 | 18 | 20 |
| warships (docs/NAVY.md: one tier ahead of the land) | 6 | 7 | 9 |
| defences | 8 | 9 | 10 |
| economy structures (mines, reactors) | 6 | 6 | 6 |
| commander refits | 12 (engineering suites 10) | | |

A fixed ratio per kind means a reactor count that fits one activity fits the others too.

## Economy

- Mines: reach 1000 m, so 3-5 fit round a base without sharing much ground. Each pays a `base`
  from the moment it is finished (1.1 / 2.75 / 5.5 per second), and its land spreads out at
  10 m/s (full in about 100 s); shafts sink at 4 m/s and drifts run at 12 m/s. Each mine stores
  mass (250 / 750 / 2000).
- A new mine is the good investment and each tier above it a poorer one (the user's call,
  2026-09-26): a tier 2 mine yields 2.5x a tier 1, a tier 3 5x, the Deep Core 7.5x (T1 ground
  0.010 a hectare, ore 0.9). On good ground a tier 1 mine makes about 6.4 mass/s and pays back
  its 160 in about 25 s. An upgrade pays only the difference between the tiers: to tier 2
  1540 (1700 - 160), paying back in about 165 s; to tier 3 5500, about 345 s; to the Deep
  Core 8800, about 550 s.
- A mine's `base` is shared with the mines next to it the way its land is: it gets the part of
  the base that matches the part of its circle it holds, land or sea. Before this, every shaft
  paid its full base however close the mines stood, so a packed block of mines made several
  times what four spread out did (test `packing_mines_together_...`).
- Mines are easy to hurt: 600 / 2000 / 5000 / 10000 health (T1-T4; 1500 / 5000 / 12000 / 24000
  before 2026-09-30, when the user found tier 1 mines "bulky and hard to raid"). Three Wardens
  kill a tier 1 mine in about 10 s, and three Petrels in one pass. A raid on the mines is meant
  to pay: the mine that goes up again digs its land out from nothing, about 100 s.
- Measured with the duel probe on Serac Divide, Hard against Hard: 25-29 mass/s at 15 minutes
  (was 40-49 with 3x per tier), 48-65 at 30.
- The Deep Core (T4, `aster_core_mine_t4`) is built by Mason IIIs and Engineering Suite III
  commanders, or upgraded from a tier 3 mine, and meant to be a poor one. Like
  every tier it multiplies the mine's ground, ore and shaft yield, never a flat bonus (the user's
  call): 7.5x a tier 1, only 1.5x over tier 3 (base 8.25), for 16000 mass / 96000 energy / 14000
  time built outright (8800 / 52800 as an upgrade from tier 3). It opens with tech 3 (the only tech 4 build is the
  Fulgur, raised by Mason IIIs), has 10000 health and stores 4000.
- Mines stand on land or out at sea (`water_build`). A mine at sea mines only the sea, and a
  mine on land only the land: each territory stops at the shore, and land and sea mines never
  share ground with each other, only with their own kind (the user's call, 2026-09-29). At sea
  the reach is wider, 1500 m (`sea_reach`, so a whole circle of sea is 2.25x a land circle, at
  the same `ground` a hectare, with no ore), but the worked water spreads at only 4 m/s
  (`SEA_SPREAD_SPEED`: full in about 375 s against 100 on land).
- Reactors: 15 / 350 / 2000 energy/s for 75 / 700 / 2800 mass: 5 / 2 / 1.4 mass per energy a
  second, so each tier is far cheaper per unit of energy than the one below. A field of tier 1
  reactors is the stopgap of the opening, not the way to power a side (the user, 2026-09-30:
  "t1 pgens are too good of a deal"; they were 20 / 250 / 1500, only a third dearer than tier 2).
- Every tier upgrade, structure or engineer, pays only what the new tier costs over the old
  one (`Blueprints::upgrade_cost`: Mason to Mason II 148 mass / 940 energy, core mine tier 1
  to 2 1540 / 9240). A refit kit is paid in full.
- Commander's Material Formation Engine: +6 mass, +250 energy a second (about a good tech 1
  mine and a tech 2 reactor) for 1600 mass; it was +12 / +2000, worth a hundred tech 1 reactors.
- **Tier paths are the commitment** (the user, 2026-10-02: "tiering more expensive, more of a
  commitment", units cheaper against it). Only the ways to a new tier cost more; unit prices
  stay. Land and air factories 240 / 3,000 / 9,000 mass (Forge to II 2,760, II to III 6,000;
  naval 260 / 3,080 / 9,100; Regency alike), build times 3,700 / 10,000. The commander's
  Engineering Suite II costs 2,400 and Suite III 6,000 (kits pay in full; 1,800 / 4,500 time).
- **Engineers climb in the field**: an engineer puts its own next tier on at that tier's build
  power (`Blueprints::upgrade_power`), so Mason to Mason II takes 30 s and II to III about
  21 s (180 s and 107 s at its own power before). It still waits for the side's tech.
- **Material fabricators** (`fabricator: (mass)`) turn a lot of energy into a little material,
  as far as their upkeep is paid (nothing in a full stall), and go up like reactors. One of
  them and the same-tier power it needs pays back about ten times slower than a same-tier
  mine (the user's rule). Each tier is a building of its own on its power plant's lot, not
  an upgrade: T1 (2x2) +0.5/s for 15 energy/s, 50 mass, ~250 s; T2 (4x4) +2/s for 1,200,
  900 mass, ~27 min; T3 (8x8) +5/s for 9,000, 4,650 mass, ~58 min. They share the Mines
  priority switch. The Regency Condensers are the same.
- Stalls (`economy.rs`): short of materials or energy, everything slows by the same share:
  factories, builders, upkeep and the mines alike. The one exception is the side's focus
  (`focus.rs`), the Mines and Power priorities in one row under the economy panel: each of
  new mines and new power (and their upgrades) is paid Last, Even or First. First is paid in
  full before the rest; Last only out of what the rest leaves over, so it is built from
  excess. Mines First also puts reclaimers (scavenger towers, salvage units, drones and
  their carriers) first; Mines Last does not hold them back. A kind put first or last shows its own build speed on the row; the stall chip gives
  the rest's. A stalling resource's First pulses as the fix, and a note says what to build.
  The AI puts first whatever it is running out of and never puts anything last.
- Standing energy draw (`energy_upkeep`) is only for powered systems: shields, radar, sonar
  and the mines (checked on load). Guns, missile launchers, missile defence, nuke silos and
  reclaim cost energy to build, never to keep (2026-09-29: the Zenith, Narwhal, Sunfall,
  Culverin and Corona lost theirs).
- Mines run on energy: upkeep 10 / 60 / 300 / 600 per second (T1-T4), about two thirds of a T1
  reactor at T1 and a sixth of a reactor of their own tier above that. A mine digs at the share of the
  side's energy demand that is covered (behind the focus, if one is on): at worst a quarter of its output
  (`UNPOWERED` in `mines.rs`), so a side out of energy loses most of its mass too. That is why
  an energy stall is the one to prevent. A mass stall does not slow the mines, or it would feed
  itself. The stall chip names the resource short and shows the materials lost a second.

The throwaway mine probe (a test that places mines on the real maps and prints their output
over time) is the check for any change to the mine numbers.

### Build power (2026-09-30)

The user found that playing the economy well ran them out of build power, that late in a game
they could not get enough of it to spend their income, and that they were almost never short
of materials even while building big weapons and units. The ledger probe
(`zz_eco_ledger_probe`, AI against AI) showed why: an economy grew almost free of build power
(a mine upgrade was 0.3-0.8 build-power seconds per material paid, against 4-7 for a unit), so
income outran what the factories could turn into anything. At 30 minutes the AIs stood 50-115k
materials of economy against 5-12k of army. Two rules since:

- **Build time follows mass.** Every build takes about as long per material as others of its
  kind, economy included: a mine upgrade takes about 1.6 build-power seconds per material it
  pays (core mine times 160 / 2500 / 8800 / 14000), reactors about 1.7 (Reactor III 4800).
  Growing income now competes for build power with spending it.
- **Build power gets cheaper by tier.** Factories 20 / 120 / 360, so each tier's factory rolls
  out its line tank in 11-14 s (Warden 14, Bulwark 11, Paladin 13). Engineers 5 / 30 / 150
  (0.10 / 0.15 / 0.19 build power per material); the commander's Engineering Suite II 30,
  Suite III 150, Auxiliary Engineering Suite 60. A late-game side buys the build power to spend
  its income with a few tier 3 engineers, not dozens of Masons.

Measured on 8 AI duels (40 min, Hard): mass mined over a game fell by a third to a half (dev16
317k to 202k, Meridian Crown 309k to 145k), and income at 24 minutes to 46-108/s (was 108-226). Army standing at 30
minutes rose against economy standing (about 1:2 to 1:3, was 1:4 to 1:9).

## Reclaim

Materials are never destroyed, only moved: wrecks never decay and weapons cannot destroy them.
Ore does not run out (the user's call), so reclaim grows with how much is fought over rather than
replacing the mines.

- Wrecks keep more the higher the tier: 81% / 85% / 90% of the unit's mass (`default_wreck` in
  `raw.rs`; `wreck_fraction` in a unit file overrides it: the commander keeps 20%).
- A wreck gives up only what its reclaimer's side has room to store; the rest waits in the wreck.
- Reclaim power is build power (1 mass a second per point), so it rises with the engineer tiers.
- Reclaim is cheap and quick to pay back at every tier: a unit whose job is reclaim pays for
  itself in well under a minute of beam time on a wreck field, so it is a real income next to the
  mines. Its reach is far past an engineer's walk. A reclaimer's `power` is the whole unit's,
  split across the heads working that tick.
- Reclaiming takes no energy, not even a tower's: no reclaimer draws upkeep for it, and an
  energy stall never stops or slows it (checked on load; the Argus's upkeep is for its radar,
  sonar and field). Only a full mass store holds reclaim back, and never on a side
  that builds for free (the test range).

  | Unit | Tier | Mass | Power | Reach | Payback (beam) |
  | --- | --- | --- | --- | --- | --- |
  | Scavenger (tower, upgrades in place) | 1 | 120 | 6 | 640 | 20 s |
  | Scavenger II | 2 | 420 | 40 | 1,100 | 11 s |
  | Scavenger III | 3 | 1,700 | 200 | 1,700 | 9 s |
  | Gleaner (hover: land and water, works while moving) | 1 | 60 | 5 | 550 | 12 s |
  | Magpie (air, works while flying) | 1 | 55 | 4 | 450 | 14 s |
  | Thresher (land, 3 heads, anti-missile) | 2 | 320 | 30 | 850 | 11 s |
  | Trawler (boat, works while sailing) | 1 | 80 | 6 | 600 | 13 s |
  | Osprey (4 drones, power 5 each) | 2 | 200 | 20 | 800 | 10 s |
  | Argus salvage ray (radar/sonar plane, a Scavenger III's reach) | 3 | 1,200 | 40 | 1,700 | 30 s |
  | Commander drone port (2 drones) | 2 | 450 | 10 | 1,400 | 45 s |
- Materials Vault tiers hold 1,500 / 6,000 / 24,000 for 150 / 400 / 1,000 mass: storage gets cheaper per
  unit the higher the tier, so it never taxes a big economy. The Capacitor Bank costs 120 mass.
- Economy structures (mines, vaults, Scavengers) upgrade only as far as the side's tech.
- The HUD shows reclaim in the materials income and its share ("40% reclaim").

## Warships

Warships (the Valiant, Resolute and Dominion) fly in the air layer with everything else: there is
no separate space layer, and any anti-air weapon can hit them.

- Every spaceship sits a tier above the land and air units of its cost band, and pays 1.75x what
  it did as a tier lower (2026-10-02); only the Dominion kept its tier and price:

  | Ship | Tier | Mass | Energy | Build time |
  | --- | --- | --- | --- | --- |
  | Courier (light transport) | 2 | 280 | 4,200 | 1,400 |
  | Coffer (Regency light transport) | 2 | 280 | 4,200 | 1,400 |
  | Vigil (sensor ship) | 2 | 490 | 7,350 | 2,450 |
  | Valiant (rail corvette) | 3 | 1,925 | 31,500 | 11,550 |
  | Bastion (assault transport) | 3 | 4,200 | 63,000 | 25,200 |
  | Resolute (heavy frigate) | 4 | 8,750 | 105,000 | 35,000 |
  | Dominion (dreadnought) | 4 | 16,000 | 220,000 | 70,000 |

  Tech 2 ships are raised by the Mason II and III and the commander's Engineering Suite II and
  III (the Regency's Artificer II and III and the Exarch's suites); tech 3 and 4 ships by the
  Mason III and Engineering Suite III only.

- Warships are expensive, and they are countered by:
  - anti-space guns;
  - other warships;
  - fighters, against most hulls, when the ship has no anti-air cover.
- A ground anti-space gun outranges the warship it answers and beats it for less mass:
  - one Zenith (T4, 11,200 mass, 3,200 m) reliably kills one Dominion (16,000 mass, 2,500 m) at
    about 70% of its mass;
  - the Narwhal (T3, 3,800 mass, 3,000 m) does the same to a Resolute (T4, 8,750 mass) at
    under half its mass, but on its own it loses to a Dominion.
- `tests/dreadnought.rs` and `tests/narwhal.rs` fight each of these duels three ways: parked
  close, parked at the ship's own reach, and with the ship ordered in from out of range.

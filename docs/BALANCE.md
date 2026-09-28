# Economy and tier balance

The rules the Aster numbers follow. Change the rule, then the numbers, not one unit at a time.

## Tiers

Tier 1 is the anchor: its combat stats set the scale. Each tier costs about 4x the one below
(mass) and is worth about 1.3x as much per unit of mass, measured as
`sqrt(health * dps) / mass` against the tier 1 unit of the same line. So a tier 2 unit is about
5x a tier 1 unit, and a tier 3 about 25x. Health and damage are raised together (never lowered)
to reach the target; range specialists (SAM, strategic bomber) are capped at 3x.

Lines: tank (Warden -> Bulwark and Skimmer -> Paladin), artillery, mobile AA,
fighters, bombers, gunships, point defence, static AA.

Off the lines, measured the same way:

- Arbalest (T3 lightning sniper, 560 mass): about 1.7 per unit of mass against the
  Paladin's 2.7. It is a range specialist (520 m against the Paladin's 280) and cannot
  defend itself up close, so it sits under the rule on purpose.
- Fulgur (T4 super-heavy tank, 3400 mass, about 4x a Paladin): 60000 health plus an
  18000 hull field, about 1067 direct dps from the AEB-2 and two bolt rifles, so about 2.6 per
  unit of mass, near the Paladin's. What puts it over is the AEB-2's channel: 2000 damage to
  everything within 7 m of it, which a column or a clump pays for many times. Raised on a
  lot by Mason IIIs (build power 60: one takes about 330 s); it has no factory.

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
  its 160 in about 25 s; the upgrade to tier 2 (1700) pays back in about 180 s, to tier 3 (7200)
  in about 450 s, to the Deep Core in about 1000 s.
- A mine's `base` is shared with the mines next to it the way its land is: it gets the part of
  the base that matches the part of its circle it holds, land or sea. Before this, every shaft
  paid its full base however close the mines stood, so a packed block of mines made several
  times what four spread out did (test `packing_mines_together_...`).
- Mines are easy to hurt: 1500 / 5000 / 12000 health (T1-T3), so six Wardens kill a tier 1 mine
  in about 12 s and three Paladins a tier 3 in about 15. A raid on the mines is meant to pay.
- Measured with the duel probe on Serac Divide, Hard against Hard: 25-29 mass/s at 15 minutes
  (was 40-49 with 3x per tier), 48-65 at 30.
- The Deep Core (T4, `aster_core_mine_t4`) is an upgrade only, and meant to be a poor one. Like
  every tier it multiplies the mine's ground, ore and shaft yield, never a flat bonus (the user's
  call): 7.5x a tier 1, only 1.5x over tier 3 (base 8.25), for 16000 mass / 96000 energy / 2400
  time, so it pays back in about 1000 s. It opens with tech 3 (the only tech 4 build is the
  Fulgur, raised by Mason IIIs), has 24000 health and stores 4000.
- Mines stand on land or out at sea (`water_build`). Out at sea there is little land in reach,
  so an offshore mine lives on its shaft and on ore fields under the water.
- Reactors: 20 / 250 / 1500 energy/s for 75 / 700 / 2800 mass; each tier is cheaper per unit
  of energy than the one below.
- Factories: build power 20 / 80 / 200, so about seven factories spend the income at every
  tier. Engineers 5 / 20 / 60.
- Commander's Material Formation Engine: +6 mass, +250 energy a second (about a good tech 1
  mine and a tech 2 reactor) for 1600 mass; it was +12 / +2000, worth a hundred tech 1 reactors.
- Stalls (`economy.rs`): short of materials or energy, everything slows by the same share:
  factories, builders, upkeep and the mines alike. The one exception is the side's focus
  (`focus.rs`), the Mines and Power priorities in one row under the economy panel: each of
  new mines and new power (and their upgrades) is paid Last, Even or First. First is paid in
  full before the rest; Last only out of what the rest leaves over, so it is built from
  excess. A kind put first or last shows its own build speed on the row; the stall chip gives
  the rest's. A stalling resource's First pulses as the fix, and a note says what to build.
  The AI puts first whatever it is running out of and never puts anything last.
- Mines run on energy: upkeep 10 / 60 / 300 / 600 per second (T1-T4), about half a T1 reactor
  at T1 and a fifth of a reactor of their own tier above that. A mine digs at the share of the
  side's energy demand that is covered (behind the focus, if one is on): at worst a quarter of its output
  (`UNPOWERED` in `mines.rs`), so a side out of energy loses most of its mass too. That is why
  an energy stall is the one to prevent. A mass stall does not slow the mines, or it would feed
  itself. The stall chip names the resource short and shows the materials lost a second.

The throwaway mine probe (a test that places mines on the real maps and prints their output
over time) is the check for any change to the mine numbers.

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
  sonar, lasers and field). Only a full mass store holds reclaim back, and never on a side
  that builds for free (the test range).

  | Unit | Tier | Mass | Power | Reach | Payback (beam) |
  | --- | --- | --- | --- | --- | --- |
  | Scavenger (tower, upgrades in place) | 1 | 120 | 6 | 640 | 20 s |
  | Scavenger II | 2 | 420 | 40 | 1,100 | 11 s |
  | Scavenger III | 3 | 1,700 | 200 | 1,700 | 9 s |
  | Gleaner (land, works while moving) | 1 | 60 | 5 | 550 | 12 s |
  | Thresher (land, 3 heads, anti-missile) | 2 | 320 | 30 | 850 | 11 s |
  | Trawler (boat, works while sailing) | 1 | 80 | 6 | 600 | 13 s |
  | Osprey (4 free drones, power 5 each) | 1 | 140 | 20 | 600 | 7 s |
  | Argus salvage ray (radar/sonar plane) | 2 | 500 | 12 | 1,200 | - |
  | Commander drone port (2 drones) | 2 | 450 | 10 | 1,400 | 45 s |
- Materials Vault tiers hold 1,500 / 6,000 / 24,000 for 150 / 400 / 1,000 mass: storage gets cheaper per
  unit the higher the tier, so it never taxes a big economy. The Capacitor Bank costs 120 mass.
- Economy structures (mines, vaults, Scavengers) upgrade only as far as the side's tech.
- The HUD shows reclaim in the materials income and its share ("40% reclaim").

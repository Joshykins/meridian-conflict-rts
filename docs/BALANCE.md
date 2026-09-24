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
  18000 hull field, about 1067 direct dps from the AEB-2 and two compact bores, so about 2.6 per
  unit of mass, near the Paladin's. What puts it over is the AEB-2's channel: 2000 damage to
  everything within 7 m of it, which a column or a clump pays for many times. Raised on a
  lot by Mason IIIs (build power 60: one takes about 330 s); it has no factory.

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
  from the moment it is finished (1.5 / 4.5 / 13.5 per second), and its land spreads out at
  10 m/s (full in about 100 s); shafts sink at 4 m/s and drifts run at 12 m/s. Yield per
  hectare is 3x per tier (T1 ground 0.013, ore 1.2). Each mine stores mass (250 / 750 / 2000).
- A mine's `base` is shared with the mines next to it the way its land is: it gets the part of
  the base that matches the part of its circle it holds, land or sea. Before this, every shaft
  paid its full base however close the mines stood, so 49 T1 mines packed 100 m apart made
  81 mass/s (four mines spread out make 22). Now the same block makes 10.5, and each extra
  mine in it takes over 700 s to pay back (test `packing_mines_together_...`).
- Measured on dev16 with four mines about 2 km apart, once dug out: about 30 mass/s at T1,
  90 at T2, 265 at T3.
- The Deep Core (T4, `aster_core_mine_t4`) is an upgrade only, and meant to be a poor one: the
  same ground and ore yield as T3, and a flat +40 mass/s from the shaft (base 53.5), for
  16000 mass / 96000 energy / 2400 time. It pays back in about 400 s, where T2 -> T3 pays back
  in about 110 s. It opens with tech 3 (the only tech 4 build is the Fulgur, raised by Mason IIIs) and stores 4000.
- Mines stand on land or out at sea (`water_build`). Out at sea there is little land in reach,
  so an offshore mine lives on its shaft and on ore fields under the water.
- Reactors: 20 / 250 / 1500 energy/s for 75 / 700 / 2800 mass; each tier is cheaper per unit
  of energy than the one below.
- Factories: build power 20 / 80 / 200, so about seven factories spend the income at every
  tier. Engineers 5 / 20 / 60.
- Commander's Material Formation Engine: +6 mass, +250 energy a second (about a good tech 1
  mine and a tech 2 reactor) for 1600 mass; it was +12 / +2000, worth a hundred tech 1 reactors.
- Stalls (`economy.rs`): upkeep and the building or upgrading of power and mines are paid
  first; everything else shares what is left.

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
  Scavenger towers: 40 power over 640 m (tech 2), 200 over 960 m (tech 3, "Scavenger II").
- Materials Vault tiers hold 1,500 / 6,000 / 24,000.
- Economy structures (mines, vaults, Scavengers) upgrade only as far as the side's tech.
- The HUD shows reclaim in the materials income and its share ("40% reclaim").

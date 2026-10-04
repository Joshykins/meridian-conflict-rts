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
- Strider (Regency T4 assault tripod, 3400 mass, the Fulgur's price): 72000 health and no
  shield, about 1040 direct dps from two Pinch-fusion Cannons (2400 a shot, taking turns
  half a reload apart) out to 1000 m, and about 230 more from eight Gravitic Seekers thrown
  up and spread over the ground targets within 900 m. Shorter reach than the Fulgur's
  AEB-2 and nothing against aircraft. Raised on a lot by Artificer IIIs or the Exarch's
  Engineering Suite III.
- Breacher (T4 assault walker, 3800 mass): 72000 health plus a 16000 hull field, about 2000
  dps from its two gatling-breach cannons at full spin (300 a shot, 0.3 s) and 430 from the
  thermobaric launchers (32 rockets of 240 every 18 s), so about 3.9 per unit of mass, a little
  over the 1.3x a tier 3 rule: it has to close to 600 m (the Fulgur stands off at 1500).
  Each rocket also leaves 16 s of burning ground (40 a second to each enemy in it), which a
  clump standing in the field pays for many times. Raised on a lot by Mason IIIs like the
  Fulgur; it carries nothing for the air.

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
- The Regency's Springald (same cost and reach) is bent as the Kiln is from the Trebuchet:
  5600 in 40 m every 10 s (560 dps on paper), a 0.8 degree spread and a 4 s charge. Its
  shot is drawn as three strands wound round each other but is one shot and strikes once
  (`Weapon::braid`), so a dome takes it as it takes a Culverin shell.

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
| aircraft (the energy-hungry branch) | 15 | 35 | 38 |
| warships (docs/NAVY.md: one tier ahead of the land) | 6 | 7 | 9 |
| defences | 8 | 9 | 10 |
| economy structures (mines, reactors) | 6 | 6 | 6 |
| commander refits | 12 (engineering suites 10) | | |

A fixed ratio per kind means a reactor count that fits one activity fits the others too.

## Economy

- Mines (2026-10-03 rebalance, then half again the same day after a played Serac Divide duel
  sat on 3-7 a second for its first seven minutes; the first numbers, 2026-10-02, left a tier 1
  side on 5 a second for twenty minutes against unit prices set at Forged Alliance's): reach
  1000 m. Ore is what a mine is for: a hectare of ore pays `per_hectare` 0.9 a second at tier 1,
  a hectare of bare land `ground` 0.00054 (a whole circle of it about 0.15), and the shaft
  `base` 0.3 from the moment the mine is finished. Each tier multiplies all three: 3x, 7x, 12x a tier 1, so
  each step up gains less than the one before. `zz_mine_yield_probe` puts a median 3-4 ha of
  ore in a lone mine's reach on the duel maps (Serac Divide 2.9, about 13 on the 80 km maps),
  so a tier 1 mine on a duel field makes about 3 a second (half again FAF's tier 1 mine), one
  on bare land about 0.45. Land spreads out at 10 m/s (full in about 100 s); shafts sink at
  8 m/s and drifts run at 24 m/s, so the ore pays within a minute or so. Each mine stores 100 / 200 /
  400 / 800.
- Prices (mass / energy): 45 / 270, 750 / 4,500, 4,000 / 24,000, Deep Core 11,000 / 66,000.
  A new mine on a field pays back in under a minute; an upgrade pays only the difference
  between the tiers, about 2, 5 and 7 minutes on a duel field. The Regency Excavators dig
  as much for a little more mass, less energy and no upkeep (55 / 770 / 4,080 / 11,200).
- **A mine climbs one tier past its side's tech** (`Blueprints::upgrade_needs`): tier 2 at
  tech 1, tier 3 at tech 2, the Deep Core at tech 3. Growing the economy, building the army
  and buying the next tier are three choices, not one: a side can raise its mines before it
  pays a tier path, and an upgraded mine (2000 health) is a target worth raiding.
- **A mine keeps out of every other mine's reach** (the user's call, 2026-10-03), anyone's,
  finished or begun: mines stand at least 1000 m apart, so each keeps about 80% of its circle
  at worst and nobody packs a field. A mine's `base` is still shared with its neighbours the
  way its land is: it gets the part of the base that matches the part of its circle it holds,
  land or sea.
- Mines are easy to hurt: 600 / 2000 / 5000 / 10000 health (T1-T4). Three Wardens kill a tier 1
  mine in about 10 s, and three Petrels in one pass. A raid on the mines is meant to pay: the
  mine that goes up again digs its land out from nothing.
- The Deep Core (T4, `aster_core_mine_t4`) is built by Mason IIIs and Engineering Suite III
  commanders, or upgraded from a tier 3 mine. It opens with tech 3, the poorest buy of the line.
- Mines stand on land only (the user's call, 2026-10-02): each territory stops at the shore,
  and the sea in its circle is worth nothing, though it counts in its shaft's share.
- Mines run on energy: upkeep 2 / 8 / 30 / 60 a second (T1-T4). A mine digs at the share of
  the side's energy demand that is covered (behind the focus, if one is on): at worst a
  quarter of its output (`UNPOWERED` in `mines.rs`), so a side out of energy loses most of
  its mass too. A mass stall does not slow the mines, or it would feed itself.
- Reactors: 25 / 350 / 2000 energy/s for 125 / 700 / 2800 mass: 5 / 2 / 1.4 mass per energy a
  second, so each tier is far cheaper per unit of energy than the one below. A field of tier 1
  reactors is the stopgap of the opening, not the way to power a side (the user, 2026-09-30:
  "t1 pgens are too good of a deal"; they were 20 / 250 / 1500, only a third dearer than tier 2).
  On 2026-10-04 tier 1 went from 15/s to 25/s with its whole price (mass, energy, time)
  scaled by the same 5/3, so the mass per energy held.
- Every tier upgrade, structure or engineer, pays only what the new tier costs over the old
  one (`Blueprints::upgrade_cost`: Mason to Mason II 148 mass / 940 energy, core mine tier 1
  to 2 705 / 4,230). A refit kit is paid in full.
- Commander's Material Formation Engine: +3 mass, +100 energy a second for 1,200 mass, about
  a tier 1 mine and a half on a field, carried on the unit that ends the game if it dies.
- **Tier paths are the commitment** (the user, 2026-10-02: "tiering more expensive, more of a
  commitment", units cheaper against it). Only the ways to a new tier cost more; unit prices
  stay. Land and air factories 240 / 3,000 / 9,000 mass (Forge to II 2,760, II to III 6,000;
  naval 260 / 3,080 / 9,100; Regency alike), build times 3,700 / 10,000. The commander's
  Engineering Suite II costs 2,400 and Suite III 6,000 (kits pay in full; 1,800 / 4,500 time).
- **Engineers climb in the field**: an engineer puts its own next tier on at that tier's build
  power (`Blueprints::upgrade_power`), so Mason to Mason II takes 30 s and II to III about
  21 s (180 s and 107 s at its own power before). It still waits for the side's tech.
- **Material fabricators** (`fabricator: (mass)`) turn energy into material, as far as their
  upkeep is paid (nothing in a full stall, nothing while paused), and go up like reactors.
  One 2x2 building from tech 2 (the user, 2026-10-03), upgraded in place to tech 3. Each eats
  a whole power plant of its tier, and tech 3 is the better buy:

  | Tier | Makes | Draws | Mass | Payback, own-tier power |
  | --- | --- | --- | --- | --- |
  | T2 (2x2) | 3/s | 350 E/s (one T2 reactor) | 1,650 | ~13 min |
  | T3 (2x2, upgrade 7,350) | 20/s | 2,000 E/s (one T3 reactor) | 9,000 | ~10 min |

  All pay back slower than the mine upgrades, so they come after the mines; they need no
  ground, so a side whose mines are done grows on them. They share the Mines priority
  switch. The Regency Condensers are the same.
- **Adjacency** (`adjacency: Energy | Mass`, mc-sim `adjacency.rs`): a provider saves each
  finished building of its owner's whose lot shares an edge with its own (a corner is not
  enough). Power plants save energy: upkeep (fabricators, mines, shields, radar) and what a
  factory builds. Fabricators save a factory's materials. A provider's saving is what it
  would save a building it rings all the way round, times the share of that building's
  perimeter the two share, so every side covered adds more until the building is ringed;
  there is no other cap. The full-ring saving scales with the fourth root of what the
  provider makes (`mc_data::Adjacency::ring`): energy 60% at 2000/s, so T1 (25/s) 20%,
  T2 (350/s) 39%, T3 60%; materials 40% at 5/s, so T2 (1.5/s) 30%, T3 40%. Mixed rings add
  up side by side. Standing close is a risk, and the blasts make it so, not a rule:
  a fabricator's blast (T2 3500 to 50 m, T3 12500 to 80 m) destroys a power plant of its
  tech against any side of it (Reactor II 2200 / Generator II 3200, Reactor III 9720 /
  Generator III 12000) but not a factory of its tech, and a reactor's blast takes the
  fabricators against it (a Regency Power Generator's blast is a Reactor's of its tech,
  drawn as its supernova). The interface shows each link: a conduit on the ground, the unit panel's
  Adjacency band, tags on the selection's links, and the placing site's would-be links.
- Stalls (`economy.rs`): short of materials or energy, everything slows by the same share:
  factories, builders, upkeep and the mines alike. The one exception is the side's focus
  (`focus.rs`), the Mines and Power priorities in one row under the economy panel: each of
  new mines and new power (and their upgrades) is paid Last, Even or First. First is paid in
  full before the rest; Last only out of what the rest leaves over, so it is built from
  excess. Mines First also puts reclaimers (scavenger towers, Reclaimers, the commander's drones and
  their port) first; Mines Last does not hold them back. A kind put first or last shows its own build speed on the row; the stall chip gives
  the rest's. A stalling resource's First pulses as the fix, and a note says what to build.
  The AI puts first whatever it is running out of and never puts anything last.
- Standing energy draw (`energy_upkeep`) is only for powered systems: shields, radar, sonar
  and the mines (checked on load). Guns, missile launchers, missile defence, nuke silos and
  reclaim cost energy to build, never to keep (2026-09-29: the Zenith, Narwhal, Sunfall,
  Culverin and Corona lost theirs).
- **Build time follows mass.** Every build takes about as long per material as others of its
  kind, economy included: a mine upgrade takes about 1.6 build-power seconds per material it
  pays (core mine times 72 / 1100 / 6000 / 15000), reactors about 1.7 (Reactor III 4800).
  Growing income now competes for build power with spending it.
- **Build power gets cheaper by tier.** Factories 20 / 120 / 360, so each tier's factory rolls
  out its line tank in 11-14 s (Warden 14, Bulwark 11, Paladin 13). Engineers 5 / 30 / 150
  (0.10 / 0.15 / 0.19 build power per material); the commander's Engineering Suite II 30,
  Suite III 150, Auxiliary Engineering Suite 60. A late-game side buys the build power to spend
  its income with a few tier 3 engineers, not dozens of Masons.
- **Every factory of a tier spends at the same rate**, whatever it builds: about 4 / 20 / 60
  materials a second (tier 1 / 2 / 3, at its own power). Land and air come out there; ships
  took half the time per material and are now 5 build-power seconds per material at tier 1
  and 6 at tiers 2 and 3 (Frigate 1,150, Destroyer 5,520, Battleship 25,200). Tier 1
  aircraft stay a little slower (about 3 a second) on purpose.

Measured 2026-10-03 with `zz_eco_ledger_probe` (36 min, Hard against Hard, Serac Divide,
the old dev16 and Twin Shoals), before -> after: income at 6 minutes 5 -> 13-17 a second and at 12
minutes 5 -> 19-32; tech 2 at 21-30 -> about 12 minutes; mass mined over the game 14-20k ->
51-78k; tech 3 and the first experimentals by minute 36. The AI's own tech timing is
untuned; its tier 3 comes late.

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
- A Reclaimer on guard (Ctrl+G on a friendly unit) follows it and takes the wrecks anywhere in
  the guard ring, not only those in reach, then picks up the guard again (`area_work.rs`).
- Reclaiming takes no energy, not even a tower's: no reclaimer draws upkeep for it, and an
  energy stall never stops or slows it (checked on load). Only a full mass store holds reclaim back, and never on a side
  that builds for free (the test range).

  | Unit | Tier | Mass | Power | Reach | Payback (beam) |
  | --- | --- | --- | --- | --- | --- |
  | Scavenger (tower, upgrades in place) | 1 | 120 | 6 | 640 | 20 s |
  | Scavenger II | 2 | 420 | 40 | 1,100 | 11 s |
  | Scavenger III | 3 | 1,700 | 200 | 1,700 | 9 s |
  | Crucible, the Regency tower (upgrades in place; 1 head, then 3 from tech 2) | 1 | 130 | 7 | 560 | 19 s |
  | Crucible II | 2 | 450 | 45 | 950 | 10 s |
  | Crucible III | 3 | 1,800 | 220 | 1,450 | 8 s |
  | Gleaner, the ARC Reclaimer (hover: land and shallows, works while moving; upgrades in place) | 1 | 60 | 5 | 550 | 12 s |
  | Gleaner II | 2 | 240 | 20 | 800 | 12 s |
  | Gleaner III | 3 | 840 | 70 | 1,100 | 12 s |
  | Breaker, the Regency Reclaimer (I / II / III) | 1-3 | 60 / 240 / 840 | 6 / 24 / 84 | 450 / 650 / 900 | 10 s |
  | Commander drone port (2 drones) | 2 | 450 | 10 | 1,400 | 45 s |
- Materials Vault tiers hold 1,500 / 6,000 / 24,000 for 150 / 400 / 1,000 mass: storage gets cheaper per
  unit the higher the tier, so it never taxes a big economy. The Capacitor Bank costs 120 mass.
- Economy structures (mines, vaults, Scavengers, Crucibles) upgrade only as far as the side's tech.
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

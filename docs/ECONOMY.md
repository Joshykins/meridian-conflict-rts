# How to play the economy

The rules and numbers are in `BALANCE.md`. This is how to *play* them: a mental model,
and the plan that measured best when an economy-only player raced to a Deep Core.

## The model: one pipe, three valves, one gauge

Materials come **in** (mines, reclaim, the commander's MFE), are **spent** through build
power, and every material spent drags **energy** along with it. The economy runs at the
speed of whichever of the three is shortest.

| Valve | What it is | Rule of thumb |
|---|---|---|
| Income | materials a second | a T1 mine is ~1/s on an ore field for 45 (bare land ~0.15) |
| Build power | how fast you can spend | a new mine or reactor takes about **1-1.7 build-power seconds per material**, a mine upgrade about 1.6, units 4-7: growing the economy and spending it both take build power (since 2026-09-30). Buy it by tier: engineers 5 / 30 / 150, factories 20 / 120 / 360 |
| Energy | the tax on spending | **6 energy per material** on economy (10 on commander refits), plus the mines' upkeep; when it runs dry everything slows, the mines included |

**The materials store is the gauge.** Read it every few seconds:

- **Filling up:** you earn faster than you can spend. Add build power: engineers, or
  put idle builders on assist. A full store is income thrown away.
- **Empty, and every builder busy:** healthy. You are spending all you make.
- **Empty, and builders idle:** too little income. Build mines.
- **Energy dry:** power first (the Power: First switch), then more reactors. A dry
  store costs you most of your mines' output too.

**What to invest in: shortest payback first, and only while it pays back before you need
the money.** Paybacks, energy counted at 6 to 1:

| Investment | Pays back in |
|---|---|
| T1 mine (on a field) | ~25 s |
| Engineer (build power turning a full store into mines) | as soon as it has work |
| Commander MFE (+3 materials, +100 energy) | ~6 min |
| Mine T1→T2 (open at tech 1) | ~3 min |
| Mine T2→T3 (open at tech 2) | ~7 min |
| Mine T3→T4 (the Deep Core, tech 3) | ~11 min |
| Material fabricator and the reactor it eats | T2 ~13 min, T3 ~10 (less with reactors against it) |

So: mines first, and enough build power to raise them. The tier upgrades come only when
the time you have left is longer than their payback. In a race that ends at minute 12, a
side mine's T1→T2 upgrade (about 3 minutes plus the time to build it) barely pays. Over a
longer game it does. Mines climb one tier past your tech, so the mines can grow before you
buy a tier path.

## The race

**Stale since the 2026-10-02/03 mine rebalances** (ore worth far more, about 2 / 6 / 14 / 23
a second by tier on a duel field, mines one tier past the tech, fabricators by tier, tier
paths dearer): rerun the search before leaning on any
of the plan below. The plan and numbers below were searched before the 2026-09-30 rebalance (build time follows
mass, build power by tier, reactors 15 / 350 / 2000): rerun the search before leaning on them.
Searched again on the new numbers (4 maps x 2 starts): the best plan is the same but with
**3 mines instead of 6**, at 12.4 minutes. With mine upgrades now costing build power, builders
spent climbing the one Deep Core line beat builders spent spreading mines. Upgrading Masons
(+0.2 min) and Forge-carried tech (+4.5) still lose; no other single dial helps by more than
0.15 minutes.

`crates/mc-sim/tests/zz_eco_race_probe.rs` is one scripted Aster player alone on a map.
It has no enemy and no goal but finishing a Deep Core. Its choices are ~25 dials:
- how many mines, how far out, when to build the Forge and how many Masons
- the energy margin, when to take each tech step
- whether engineers or commander suites carry the tech
- the MFE, drone port and Scavengers, reclaim
- the focus switches, side upgrades
- whether to climb one mine or build the Deep Core outright

`scripts/eco-search.py` tries every other value of every dial, keeps what helps, and
repeats. The search played 4 maps × 2 starts per plan; the result below was then checked
on 3 maps it never saw.

**Starting plan: 17.1 min. Best plan: 12.4 min** (11.1-12.4 on the maps searched, 12.2-14.0
on the unseen dev16 (retired), the_axis and serac_sound).

### The best plan

1. **Forge first**, before the first mine. The commander alone (build power 10)
   cannot spend even two mines' income: the store sits full from minute 3.
2. **Eight Masons** as soon as the Forge stands. They put down six T1 mines out to
   4.5 km and T1 reactors wherever they stand.
3. **Energy at 9 per material of income**, over the mines' upkeep. Mostly T1 reactors
   (when searched they were only a third dearer per energy than T2; since 2026-09-30 they
   are 2.5 times as dear, 5 against 2 materials per energy/s).
4. **MFE on the commander** at 10 materials/s (~minute 4).
5. **Engineering Suite II at 20/s, Suite III at 25/s** (~minutes 7 and 8.5). The
   commander carries the tech: cheaper than Forge II and III, and it gets build power 160 (then 70).
6. **One mine climbs the whole line**, each tier the moment it opens (T2 at tech 2, T3
   and T4 at tech 3). Every builder with nothing else to do assists it. No other mine
   is upgraded.
7. **Mines: First** the whole game, Power: First only while energy is dry.
8. **Masons are never upgraded.** Two Scavengers go over the richest wreck fields.

### What each rule is worth

Each change below is to the best plan alone, averaged over all 14 games (7 maps × 2 starts):

| Change | Deep Core later by |
|---|---|
| Forge after 3 mines instead of first | +4.0 min |
| Build the Deep Core outright on a new spot instead of climbing a mine | +2.5 min |
| 4 Masons instead of 8 | +1.1 min |
| Masons upgrade themselves as tech opens | +0.7 min |
| No MFE | +0.7 min |
| Side mines upgrade when payback < 400 s | +0.3 min |
| Never touch the focus switches | +0.3 min |
| Forge II/III carry the tech instead of the commander | +0.2 min |
| Energy at 6 per material instead of 9 | +0.1 min (and 4x the stall time) |
| No reclaim at all (no Scavengers, builders never reclaim) | −0.1 min (noise) |

**Why the outright Deep Core loses:** the price is the same either way (160 + 1540 + 5500 +
8800 = 16000 materials). But climbing pays it in steps while income grows. The mine
earns 2.5x then 5x on the way up, and it keeps the best spot. The outright build needs
16000 at once, earns nothing until it is finished, and goes on a leftover spot.

**Why Mason upgrades lose:** an engineer builds its own next tier at power 10: 90 s to
Mason II, 320 s to Mason III. It does nothing else meanwhile. A new Mason from the Forge
takes 13 s.

**Why reclaim barely matters here:** the wreckage within reach of a start is 1-3k
materials (15-20% of the 16000 Deep Core line, less still with the reactors and tech on top). A Mason reclaims at most 5 a second, walking between
wrecks. The same Mason raising a mine makes ~6 a second for good. Reclaim pays when
wrecks lie thick and close, as after a battle, or from a Scavenger that costs no
builder's time.

## Rerun it

```
cargo test --profile gate -p mc-sim --test sim --no-run
ECO=serac_divide,crosswater ECO_PLANS='best; lean:engineers=4' ECO_LOG=1 \
  cargo test --profile gate -p mc-sim --test sim -- zz_eco_race_probe:: --ignored --nocapture
scripts/eco-search.py - 3
```

After a change to economy numbers, rerun the search: the best plan may move, and a
dial that stops mattering (or starts) is itself a balance finding.

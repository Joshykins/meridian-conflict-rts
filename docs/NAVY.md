# The navy

The rules the naval roster follows. Change the rule first, then the numbers. The look rules
live in `docs/STYLE.md` "The navy"; the tier arithmetic in `docs/BALANCE.md`.

## The navy runs one tier ahead of the land

A warship is priced and powered like a land unit of the tier above it: a tier 1 warship
like tier 2 land, tier 2 like tier 3 land, and tier 3 ships beyond anything on land. Ranges
are about twice the land unit of the same tier; health and firepower four to five times.
What ships pay for it is that they cannot hide, cannot leave the water, and shore batteries
(the Trebuchet, the Skyguard) and aircraft reach them. Energy per mass for warships is
6 / 7 / 9 by tier.

## Every unit hurts something other than ships

Torpedoes hit anything with a hull in the water: ships, dived hulls, and every structure
built on water (wharfs, buoys, water AA), because water-built structures
carry the Naval category. Cruise missiles and guns hit land and structures. A submarine
surfaces to use its deck gun. Nothing in the roster is useless on a map with a coast.

The other way round: every gun, howitzer, missile and bomb that hits land also hits ships
on the surface (their `targets` list `Naval`), so a coast is dangerous to a fleet and
shore batteries matter. A dived hull is still only found by sonar, and no gun aims at it; only torpedoes
target it, but a blast on the water over it (a shell, a bomb) reaches down onto it.

## Capital ships have holes on purpose

Three escorts, none complete:

- The **Marlin** (destroyer) carries interceptor tubes: a torpedo coming in is met by a
  short torpedo of its own, and both burst under the water. It stops nothing else. The
  **Moray** carries a pair astern too, for its own hull.
- The **Manta** (air-defence cruiser) burns missiles inside a 600 m radius with two lasers,
  so it screens the ships around it, the Argus's interceptor at sea. It stops no shell and
  no torpedo.
- The **Nautilus** (shield boat) puts a bubble over the ships around it: shells and both
  kinds of missile break on it. Torpedoes run under the skirt.

The **Leviathan** (battleship) has weak AA, no sonar and no torpedo defence. The **Atoll**
(carrier) is the fleet's long arm against air: a 2,800 m SAM battery and a 4,000 m radar,
interceptor tubes, and no gun at all, so aircraft that get in close are the escorts' to meet. The **Kraken** (strategic
submarine) gives itself away every time it launches.

## Torpedo launchers on station

Three structures put attack torpedoes into any hull in reach, dived submarines included.
A launcher gives up moving, so it wins on cost: at each tier it carries about twice the
torpedo DPS and twice the health per unit of mass of the submarine it meets, and it
reaches a little past that submarine's tubes (`torpedo_launchers.rs` checks all three).

- The **Breakwater** (tier 1, 600 m, past the Barracuda's 550) is a moored float with one
  twin-tube launcher turning under it. Refitted where it floats to the **Breakwater II**
  (tier 2, 800 m, past the Moray's 700): a second launcher and heavier torpedoes.
- The **Fathom** (tier 3, 1,000 m, past the Kraken's 900) is built on the seabed, where
  there is at least 20 m of water over the whole lot (`seabed`,
  `placement::SEABED_DEPTH`): wholly under the surface, so only sonar finds it and only
  torpedoes reach it, and ships sail over it. Its four batteries fire heavy torpedoes at
  hulls from fixed tubes (a torpedo turns onto its mark by itself), and interceptors
  (`intercepts`) at torpedoes running at anything near it. Its spire runs up to the
  surface however deep it stands (the model is stretched between
  `gpu_consts::spire::BASE` and `TOP`, entity.wgsl).

Launchers need no power: on a dark grid they still fire (shields, radar and the missile
lasers go down).

## Two missile doctrines

- **Sea skimmers** (the Kraken): cruise missiles launched from under the water. A boil on
  the surface, then each boosts up out of its hatch (`vertical_launch`), arcs over under
  100 m with its wings unfolding, glides down and runs in under 30 m over ground and water,
  so only close-in AA gets a shot, late. One whose target dies takes the nearest enemy it
  can strike near where that target was. Four heavy warheads a salvo, each an AEB: the motor
  burns blue and it goes off as the electric bore's blast (`ion_blast`), lightning striking
  back up its last run (`discharge`). The launch boil paints the submarine on enemy sonar
  and radar for 8 s.
- **Dumbfire rockets** (the Swordfish): thirty-two unguided rockets a salvo out of fixed,
  forward-canted cells, so the hull turns its bow onto the mark. They fly a high lob
  (`rake`) and fall over and round the mark. Each is light and easy for point defence to
  burn; the salvo is too many to burn them all.

- **High arcs** (the Megalodon's AEB strike missiles): launched dived, each climbs over a top
  of up to 2,500 m (`apogee`) and falls on a base up to 6 km off, going off as the electric
  bore's blast. A volley of four spreads over four marks (`split`) unless the boat is ordered
  onto one (an Attack), when all four fall on it. In the air for a long time and on radar
  the whole way: long-range SAMs and Mantas get many shots.

Every anti-missile laser comes out of a head on the model (`anti_missile_mounts`, the
shared `pd_laser` in models/aster/naval/mod.rs) marked with steady laser red
(`GLOW_LASER`), so the missile defence reads apart from the rest of the ship.

A defender buys the right umbrella: flak and point defence for skimmers, lasers for a rocket
salvo, long-range SAMs for arcs. One umbrella does not cover them all.

## Long guns need eyes

The Leviathan's batteries shoot to 2,520 m and it sees 1,440. It needs a Manta, a Swift, a buoy or a forward
unit spotting for it. Killing the spotters blinds it.

## The wreck economy

Ships sink and lie on the seabed. Any reclaimer takes a wreck within its reach however deep it
lies, engineers included; the **Trawler** (salvage boat) is the one that can sail out to it.
Tier 3 wrecks keep 90%: a sunk Leviathan is nearly 4,000 mass on the seabed. Controlling the
sea after a battle pays for the battle.

## The roster

| Unit | Tier | Role | Notes |
|---|---|---|---|
| Skiff | 1 | Attack boat | Rotary gun. |
| Pike | 1 | Frigate | Deck gun, AA mount, radar. |
| Barracuda | 1 | Attack submarine | Torpedoes, sonar. |
| Trawler | 1 | Salvage boat | A mobile reclaim head, 600 m reach; works what it passes while it sails. |
| Marlin | 2 | Destroyer | A bolt rifle (the Paladin's gun a size down) that lobs a little over terrain, 1500 m (a small battleship); torpedo tubes, sonar, interceptor tubes, light AA. |
| Manta | 2 | Air-defence cruiser | A 16-cell vertical missile array fired as one ripple to 1,850 m (past any flak gun, short of the Skyguard), spread over the fliers in range; radar, two missile-interception lasers screening 600 m around it, one light gun. |
| Swordfish | 2 | Rocket ship | Thirty-two unguided rockets per salvo, lobbed from fixed cells. No other weapon. |
| Moray | 2 | Hunter-killer submarine | Six guided torpedo tubes, sonar, stern interceptor tubes; a deck gun that only works surfaced. |
| Nautilus | 2 | Shield boat | A bubble over the fleet. Unarmed. |
| Leviathan | 3 | Battleship | Three triple turrets of heavy guns, secondary guns, weak AA. The hero. |
| Atoll | 3 | Air-defence carrier | Twelve hatched long-range SAM cells (2,800 m), fleet radar (4,000 m), interceptor tubes. No gun. |
| Kraken | 3 | Strategic submarine | Eight tubes; four sea-skimming cruise missiles per salvo, launched dived. |
| Breakwater | 1 | Torpedo launcher | A moored float firing attack torpedoes at hulls within 600 m (past a Barracuda); refitted to tier 2 (two launchers, 800 m). |
| Fathom | 3 | Seabed torpedo launcher | Built on the seabed in deep water; heavy torpedoes at hulls and interceptors at torpedoes from four batteries (1,000 m); only sonar finds it, only torpedoes reach it. |
| Megalodon | 4 | Experimental submarine | Built by engineers on the water. AEB strike missiles to 6 km (volleys of four over four marks), its own nuclear warheads (assembled aboard, four at most, launched dived), long-range torpedoes, interceptors, two deck rail turrets that work only surfaced, a hull shield that holds dived too. |
| Narwhal | 3 | Anti-ship trimaran | A Zenith rail down the keel: the hull turns to aim, the barrel elevates. Shoots only spaceships; nothing else aboard. |

## The Regency's navy

`data/factions/regency/units/naval.ron`, models `models::regency::naval`. Where ARC's
fleet is a set of surface specialists, the Regency's goes under: nearly every hull
dives, so a Regency fleet picks when the fight starts. What it pays: a gun that needs the
surface (`surfaced`) does nothing dived, and only torpedoes work from under the water.
Their guns fire plasma; their torpedoes and seekers are Gravitic (`plasma_grade:
Gravitic`), a plasma charge steered in gravity containment.

- The **Dirk** (attack boat) lies in ambush (`dive.ambush`): ordered down, it still
  marks targets for its surfaced-only gun as if it were up, comes up while that gun has
  a mark, and goes down again when it has none. It carries nothing that works under the
  water and no sonar, so it finds its marks by eye.
- The **Falchion** (frigate) is the one tech 1 hull that stays up: radar and flak for a
  squadron that is otherwise dived.
- The **Claymore** (heavy destroyer) dives with a hull shield that holds under the water
  too. Dived it fights with gravitic torpedoes and meets torpedoes with interceptors; its
  pinched-plasmeric cannon and its AA repeater need it surfaced, and it comes up only on order
  (it does not lie in ambush).
- The **Pavise** (cruiser) holds the sky (a seeker battery, two counter-seekers that burn
  down missiles over the ships round it) and has one long arm the Manta lacks: a heavy
  gravitic seeker, one at a time, at structures and nothing else.

| Unit | Tier | Role | Notes |
|---|---|---|---|
| Dirk | 1 | Attack boat | Dives; surfaces to fire its twin plasmeric repeater; no sonar. |
| Stiletto | 1 | Attack submarine | Gravitic torpedoes, sonar. |
| Falchion | 1 | Frigate | Heavy plasmeric repeater, plasmeric AA repeater, radar. Stays up. |
| Claymore | 2 | Heavy destroyer | Dives; hull shield; gravitic torpedoes and interceptors dived; pinched-plasmeric cannon and light AA repeater surfaced; radar, sonar. |
| Pavise | 2 | Cruiser | Gravitic seeker battery (AA, 1,800 m), two counter-seekers (600 m), a light repeater, radar; a heavy gravitic seeker at structures to 2,600 m. |
| Flamberge | 3 | Battleship | Dives (needs about 30 m of water to go under); three twin Pinch-fusion Cannon houses, four twin heavy repeaters on the beams, light flak, counter-seekers, all surfaced only; bow gravitic torpedoes dived or up. No sonar, no torpedo defence. |
| Mantlet | 3 | Air-defence carrier | Twelve heavy gravitic seeker cells (2,700 m), flak on both beams, counter-seekers, interceptor tubes, radar 3,800 m. No gun. |
| Rapier | 3 | Assault submarine | The fastest hull under the sea; six-tube gravitic torpedo launcher and interceptors dived; surfaced, two twin repeaters, flak and a gravitic seeker pod. |

The Slipway builds the tech 1 hulls and is refitted in place to the Slipway II and the
Slipway III for the tech 2 and tech 3 ones. Like ARC's Wharf it is a one-sided quay: the
berth is open water, so a capital ship longer than the yard still fits. Every Regency
hull that dives and carries guns brings them up only on order (the Dirk excepted, which
lies in ambush).

## The Leviathan

The unit the naval design language is nailed on:

- It moves the sea: a bow wave that stands up with speed, a wake three hulls long, and the
  hull heeling into turns.
- A salvo is an event: the ship heels away from the broadside, the muzzle blast stamps a
  pressure ring on the water and throws a spray sheet, and the recoil shoves the hull
  sideways.
- It looks like a warship, not a starship: no emitter strips on the hull; the only blue on
  it is the Arc Cannons' plasma cells and radiator lines. Its only
  lights are a ship's own (red/green sidelights, a white masthead and stern light, warm deck
  floods, lit scuttles; `GLOW_NAV_RED`, `GLOW_NAV_GREEN`, `GLOW_LAMP`, `WINDOWS`) and the
  missile-defence lasers' red.
- It fights broadside on. Engaged and stopped, it lays its hull so the mark sits 75 degrees
  off the bow (`motion.broadside`), on the nearer beam, a little forward of square, so all
  three batteries bear. Under way it does not wheel; it fires what bears.
- Each battery fires its three barrels on one tick (`salvo_delay: 0`), and charges and
  fires the moment it bears and is loaded: no battery waits for another, or for the hull
  to finish laying its beam. The forward batteries open while the hull is still coming
  round, and the aft one joins as it bears.
- Its secondaries are four twin quick-firing turrets on the sponsons, two a side, resting
  trained outboard (`facing`) and covering their own side; conventional orange shells.
- Four point-defence lasers (`anti_missile`, `anti_missile_mounts`) burn down missiles; each
  shot comes from the emitter nearest the missile. They do nothing against torpedoes.
- Its main guns are nine Arc Cannons, three to a house: the Trebuchet's Arc Howitzer made a
  naval gun, on a flatter arc (`loft: 2`). They are direct-fire guns, not artillery
  (`flat_fire`, as are the secondaries): their range rings are red. All three houses charge
  together for 3.4 s, arcs running over the tubes and jumping between them, then fire a
  broadside of blue plasma rounds that burst in lightning where they land (`discharge`), a
  long streak behind each (`streak`), and on land melt a pool of ground that glows and
  crusts over (`melt`); the look is the Arc Howitzer's (`howitzer`, STYLE.md).
  The ship has its own charge and broadside sounds (`leviathan_charge`,
  `leviathan_broadside`, data/sounds/arc_howitzer.ron): heard as one charge per hull and
  one report per house.
- The aft battery traverses inside its arc: from over one bow to the other it swings round
  by the stern, never across the bow it cannot bear through.
- It outranges its own eyes.
- Its gaps are the point.

## Inspect

```sh
cargo test -p mc-sim --test sim -- naval::
cargo test -p mc-sim --test sim -- naval_roster::
cargo test -p mc-sim --test sim -- torpedo_launchers::
cargo test -p mc-sim --test sim -- megalodon::
cargo test -p mc-sim --test sim -- regency_navy::
cargo test -p mc-sim --test sim -- broadside::
cargo test -p mc-path --test big_hulls
cargo test -p mc-models --lib tests::
./play.sh --scene naval --map twin_shoals
```

`--scene naval` puts the whole roster to sea either side of the open water it prints
(`naval scene: fleets either side of X,Y`; Twin Shoals: 7109,8663), both sides funded so
shields, sonar and lasers run; `naval-still` holds fire. Player 0's fleet lies to the west:
the Leviathan 420 m out on the line, the Atoll at (-540, -120), the Kraken at (-380, +90),
the Swordfish at (-340, -80), the Trawler at (-200, +170). The Leviathan's first salvo lands
at tick 36 (`--ticks 36 --follow 8 --alpha 0.5 --camera 6689,8663,170,140`).

Software previews of every hull: `MODEL_DUMP_DIR=DIR cargo test -p mc-models --lib -- --ignored dump_models`.

## Not done

- The Atoll's flight deck is for show: it does not dock, mend, launch or build aircraft yet.
  How that should work is undecided, as it needs aircraft mechanics the game lacks.
- The AI builds the new hulls as naval units but knows nothing about their roles.
- Size-5 hulls (Leviathan, Atoll) have not been checked out of a Wharf or through the
  shipped maps' channels.

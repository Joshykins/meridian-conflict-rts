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
built on water (wharfs, buoys, offshore mines, water AA), because water-built structures
carry the Naval category. Cruise missiles and guns hit land and structures. A submarine
surfaces to use its deck gun. Nothing in the roster is useless on a map with a coast.

## Capital ships have holes on purpose

Three escorts, none complete:

- The **Marlin** (destroyer) carries interceptor tubes: a torpedo coming in is met by a
  short torpedo of its own, and both burst under the water. It stops nothing else.
- The **Manta** (air-defence cruiser) burns missiles inside its radius, the Argus's
  interceptor at sea. It stops no shell and no torpedo.
- The **Nautilus** (shield boat) puts a bubble over the ships around it: shells and both
  kinds of missile break on it. Torpedoes run under the skirt.

The **Leviathan** (battleship) has weak AA, no sonar and no torpedo defence. The **Atoll**
(carrier) has the strongest AA afloat, interceptor tubes, and no anti-surface gun. The **Kraken** (strategic
submarine) gives itself away every time it launches.

## Two missile doctrines

- **Sea skimmers** (the Swordfish): guided, they track their target and fly under 30 m over
  ground and water, so only close-in AA gets a shot, late. Small warheads, many of them.
- **High arcs** (the Kraken): launched from under the water, they climb on a steep arc to
  about 1,500 m and fall on the target. In the air for tens of seconds and on radar the
  whole way, so long-range SAMs and Mantas get many shots, but each one that lands is a
  strategic hit. The launch boil paints the submarine on enemy sonar and radar for 8 s.

A defender buys the right umbrella: flak and point defence for skimmers, long-range SAMs for
arcs. One umbrella does not cover both.

## Long guns need eyes

The Leviathan shoots to 1,400 m and sees 400. It needs a Manta, a Swift, a buoy or a forward
unit spotting for it. Killing the spotters blinds it.

## The wreck economy

Ships sink and lie on the seabed. Reclaim reaches 10 m under the surface, so shallow wrecks
are anyone's, but a deep-water fight leaves mass only the **Trawler** (salvage boat) can lift.
Tier 3 wrecks keep 90%: a sunk Leviathan is nearly 4,000 mass on the seabed. Controlling the
sea after a battle pays for the battle.

## The roster

| Unit | Tier | Role | Notes |
|---|---|---|---|
| Skiff | 1 | Attack boat | Rotary gun. |
| Pike | 1 | Frigate | Deck gun, AA mount, radar. |
| Barracuda | 1 | Attack submarine | Torpedoes, sonar. |
| Trawler | 1 | Salvage boat | A mobile salvage ray on a folding mast; deploys to work; reaches seabed wrecks. |
| Marlin | 2 | Destroyer | Twin rail guns, torpedo tubes, sonar, interceptor tubes, light AA. |
| Manta | 2 | Air-defence cruiser | Vertical-launch SAMs, flak, radar, missile interception, one light gun. |
| Swordfish | 2 | Cruise-missile ship | Eight sea skimmers per salvo. No other weapon. |
| Moray | 2 | Hunter-killer submarine | Six guided torpedo tubes, sonar; a deck gun that only works surfaced. |
| Nautilus | 2 | Shield boat | A bubble over the fleet. Unarmed. |
| Leviathan | 3 | Battleship | Three triple plasma turrets, secondary guns, weak AA. The hero. |
| Atoll | 3 | Carrier | Docks, repairs and launches aircraft; builds T1/T2 aircraft; strong AA. |
| Kraken | 3 | Strategic submarine | Eight tubes; four high-arc missiles per salvo, launched dived. |

## The Leviathan

The unit the naval design language is nailed on:

- It moves the sea: a bow wave that stands up with speed, a wake three hulls long, and the
  hull heeling into turns.
- A salvo is an event: charge glow runs down the barrels, the ship heels away from the
  broadside, the muzzle blast stamps a pressure ring on the water and throws a spray sheet,
  and the recoil shoves the hull sideways. Shells fall as tall columns lit by the plasma.
- It outranges its own eyes.
- Its gaps are the point.

## Inspect

```sh
cargo test -p mc-sim --test naval
cargo test -p mc-sim --test naval_roster
cargo test -p mc-path --test big_hulls
cargo test -p mc-render --lib models::tests
./play.sh --scene naval --map twin_shoals
```

`--scene naval` puts the whole roster to sea either side of the open water it prints
(`naval scene: fleets either side of X,Y`; Twin Shoals: 7109,8663), both sides funded so
shields, sonar and lasers run; `naval-still` holds fire. Player 0's fleet lies to the west:
the Leviathan 420 m out on the line, the Atoll at (-540, -120), the Kraken at (-380, +90),
the Swordfish at (-340, -80), the Trawler at (-200, +170). The Leviathan's first salvo lands
at tick 36 (`--ticks 36 --follow 8 --alpha 0.5 --camera 6689,8663,170,140`).

Software previews of every hull: `MODEL_DUMP_DIR=DIR cargo test -p mc-render --lib -- --ignored dump_models`.

## Not done

- The Atoll does not dock, mend or launch aircraft yet, and builds none: an `airbase:` block
  on a moving hull needs the docking code (airbase.rs) taught about a base that moves.
- The AI builds the new hulls as naval units but knows nothing about their roles.
- Size-5 hulls (Leviathan, Atoll) have not been checked out of a Wharf or through the
  shipped maps' channels.

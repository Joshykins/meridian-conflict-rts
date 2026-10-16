# Regency space fleet

The fleet uses swept, split hulls built from overlapping charcoal armour, red seam
lights and violet plasma hardware. Its five models are authored at their blueprint
dimensions; weapon mounts and cargo apertures use the same metre coordinates.

| Unit | Tier | Built by | Role |
| --- | --- | --- | --- |
| Coffer | 2 | Artificer II/III; Exarch Engineering Suite II/III | Unarmed light transport; sixteen slots, enough for one commander |
| Ark | 3 | Artificer III; Exarch Engineering Suite III | Heavy transport; 192 slots, 44 m bay width and 36 m clearance |
| Vassal | 3 | Artificer III; Exarch Engineering Suite III | Escort frigate, the Valiant's peer: a Pinch-plasma Lance under the keel and three Twin Escort Plasma Repeaters |
| Suzerain | 3 | Artificer III; Exarch Engineering Suite III | Monitor with two anti-ship guns and six area suppression batteries |
| Scourge | 4 | Artificer III; Exarch Engineering Suite III | Capital destroyer, the Dominion's peer: an Edict Lance from the energy core through its middle, six Pinch-plasma Batteries, four Twin AA Repeaters, four Rondels |

All five are site-built aircraft tagged Space (the Scourge also Experimental). Select an appropriate engineer,
place the ship on its own lot and let construction finish. The Space subject/spawn
filter in the testing ground also exposes all five.

Coffer preserves the existing `regency_t2_transport` unit ID and `regency_coffer`
mesh key while replacing the old model completely. Its 26 m wide, 28 m high bay
clears the Exarch; commanders use all sixteen slots. Ark clears the complete Regency
land roster through tech 3.

The Ark is the Bastion's counterpart at the Bastion's size (300 m): a broad arrowhead
whose wings are laid in feathered plates, their tails the saw-toothed trailing edge,
with two armour ridges and a machinery trench down the spine, a bridge on the foredeck
and canted fins over the stern drives (`crates/mc-models/src/regency/space/ark/`). It
never sets down: it hangs on eight gravity lifts with its hold floor 38 m up, and its
ramp drops one in two from the belly, so it swings shut flush with the hull. Transport cargo capacity
is the engine's existing abstract hold capacity; units board and stow in sequence.

Select cargo and right-click a transport to board. The ship lands, opens its ramp
and takes the unit through the stern lane. Move or unload it at another location
to check departure, approach and disembarkation. The hold and ramp are above the
ground by one metre, matching the mesh and animation rig.

Vassal is the fleet's escort and the Valiant's peer, a small faceted hull (about 110 m)
dressed as the Suzerain, its bow cut short at a split prow whose two blades run out
over the lance (`crates/mc-models/src/regency/space/vassal/`). The Pinch-plasma Lance
hangs under the forward keel on a pylon and turns right round: it lays onto armies,
buildings and ships, gathering its plasma for a second and pinching it out as one red
jet (the Pinched grade's look). Three Twin Escort Plasma Repeaters keep the sky, one
on the foredeck, one on a barbette amidships firing over it and one hung under the
stern; they reach 200 m past the Valiant's rotary. A small hull shield stands in for the
Valiant's heavier hull. Suzerain's two heavy guns
engage ships, aircraft and naval hulls; its six splash batteries also bombard land
units and structures. Each gun uses its own weapon slot and pivot, so barrels
pitch and traverse with the weapon they represent.

Suzerain is a narrow faceted gun hull about 210 m long, dressed as the Ark is:
swept armour shards in courses, twin ridges down the spine aft of a horned bridge,
canted fins and teeth over the stern, a keel blade under the bow
(`crates/mc-models/src/regency/space/suzerain/`). Its weapons are built into the
hull: the chine swells into a blister at each battery, the ball seated in a pocket
pressed into it under a hooded plate, so each battery bears across its own side
(`facing` ±90, `arc` 220). The heavy guns turn in armoured wells let into the
deck, the after one on a raised step so it fires over the forward one.

Scourge is the tech 4 capital ship, about 700 m long, the Scourge's hull dressed as the
Regent was (the Regent, the tech 5 super destroyer, merged into it). One faceted
arrowhead of hard chines, two-toned (charcoal, and a paler steel stripe down each
shoulder: `PLATING` under `pattern::EMBER`), under the Regent's carapace of feathered
plates lifted over one another; red lines run down its chines and along its arms
(`pattern::EMBER_SLOT`). Out of its chines amidships stand the Regent's two arms, each
curving out to its corner and bending aft into a claw that reaches the stern
(`destroyer/wings.rs`); the Regent's terraced tower with its horns stands aft. It flies
at the Dominion's 700 m, closes to weapon range and holds station.

Its main gun is the Edict Lance (`edict_beam`): the Regent's Edict made a held beam, laid
from the emitter under the energy core, a drum through the middle of the hull: a ribbed
lens dome hung in a toothed crown, a vaned race and two crossed gimbal hoops
(`destroyer/core.rs`). Nothing aims it, so dark it lays onto a mark at once
(`lays_at_once`), and it charges only once its shot is laid. The core charges for 3 s
(`spin_up`): a sphere of light closes in on the lens, crossing the hull as a ring drawn
in round the emitter (regency.wgsl `reg_edict`), the emitter's rings spin up with the
charge in opposite ways (orbit pieces driven by `edict_lance_fx`), the Edict's star
kindles under the lens, lightning crawling in to it off the hull, and
`regency_edict_lance_charge` keeps time with it, the rings unlocking with a clang and
beating quicker as they spin. The beam lights as the charge fills: a red flash, a lens
flare, a pressure ring and a hard ring of
pinched plasma round the lens (`renderer/edict_lance_fx.rs`), heard as
`regency_edict_lance_release`. The beam is the Edict's column, 108 m across: a red-hot
core in a roiling scarlet and crimson sheath (`fade_beam::HELD_EDICT`), its roar the loop
`regency_edict_lance_hold`. It is fired down its line (`sweep`): locked on, it stays on
the mark; when the mark dies the beam stays lit while it slowly slews (12°/s) onto the
next, and the stretch between lands on the ground and melts it to glass; unlike an
excavation lance it digs no crater. What it melts is as wide as what it burns: its
`splash` (20 m), the trees it sets alight, not its column's width. With nothing left to shoot it goes out
(`regency_edict_lance_vent`), leaving a red filament along its line that cools and
breaks up, the star collapsing and the seams' red dying out of the hull. It reaches
2.2 km.

Six Pinch-plasma Batteries turn in ball mounts in pockets pressed into the chines, three
a side ahead of the arms, each bearing from past the bow round its own side to past the
stern (2.6 km). Four Twin AA Repeaters stand on pedestals, one on each arm's corner and
one each side of the after deck, and four Rondel crush lenses on the deck edges meet
missiles. The guns after the lance are weapon groups (`mounts`), each mount a house of
its own (`destroyer/guns.rs`). Its stern drives burn as plasma (`models::plasma_drives`):
violet plumes with rings carried down them, arcs at the mouths.

Inspect the fleet with:

```sh
scripts/shot.sh variants regency_t2_transport=base regency_t3_assault_transport=base regency_t2_space_frigate=base regency_t3_space_cruiser=base regency_t4_space_destroyer=base --views front34,rear34,top
scripts/shot.sh unit regency_t4_space_destroyer --scenario targets --ticks 60 --views 60:-5,120:20 --look 0,0,-300 --zoom 0.7   (the lance on its mark)
scripts/shot.sh unit regency_t4_space_destroyer --scenario targets --ticks 20 --views 60:-5 --look 0,0,0 --zoom 2   (the core charging)
```

Focused acceptance tests:

```sh
cargo test --profile gate -p mc-models regency::space
cargo test --profile gate -p mc-sim --test sim regency_space::
```

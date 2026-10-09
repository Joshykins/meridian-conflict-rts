# Regency space fleet

The fleet uses swept, split hulls built from overlapping charcoal armour, red seam
lights and violet plasma hardware. Its five models are authored at their blueprint
dimensions; weapon mounts and cargo apertures use the same metre coordinates.

| Unit | Tier | Built by | Role |
| --- | --- | --- | --- |
| Coffer | 2 | Artificer II/III; Exarch Engineering Suite II/III | Unarmed light transport; eight slots, enough for one commander |
| Ark | 3 | Artificer III; Exarch Engineering Suite III | Heavy transport; 96 slots, 44 m bay width and 36 m clearance |
| Vassal | 2 | Artificer II/III; Exarch Engineering Suite II/III | Economical frigate with two independently aiming plasma cannons |
| Suzerain | 3 | Artificer III; Exarch Engineering Suite III | Cruiser with two anti-ship guns and six area suppression batteries |
| Scourge | 4 | Artificer III; Exarch Engineering Suite III | Capital destroyer, the Dominion's peer: a Heavy Pinch-fusion Lance from the energy core through its middle, six Pinch-plasma Batteries, sixteen hatched seeker cells |

All five are site-built aircraft tagged Space (the Scourge also Experimental). Select an appropriate engineer,
place the ship on its own lot and let construction finish. The Space subject/spawn
filter in the testing ground also exposes all five.

Coffer preserves the existing `regency_t2_transport` unit ID and `regency_coffer`
mesh key while replacing the old model completely. Its 26 m wide, 28 m high bay
clears the Exarch; commanders use all eight slots. Ark clears the complete Regency
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

Vassal can engage ships, aircraft and surface targets. Suzerain's two heavy guns
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

Scourge is the tech 4 capital ship, about 750 m long: one faceted arrowhead of hard
chines with two great blades swept back past the stern, plated in two tones (charcoal,
and a paler steel stripe down each shoulder: `PLATING` under `pattern::EMBER`). It flies
at the Dominion's 700 m, closes to weapon range and holds station. The lance is laid from the lower lens of
the energy core, a drum through the middle of the hull lit at both ends; nothing turns. The core charges
for 2.5 s (`spin_up`: the core's charge animation and `regency_lance_charge` keep time
with it) before the beam lights. The lance is a held beam fired down its line (`sweep`):
locked on, it stays on the mark; when the mark dies the beam stays lit while it slowly
slews (12°/s) onto the next, and the stretch between lands on the ground and melts it
to glass (the renderer's ground melt). It is drawn as the power generator's star
(`star_beam`: a white-hot core in a broad prism sheath) and, unlike an excavation lance,
digs no crater. With nothing left to shoot the core winds down and vents
(`regency_lance_vent`). Six Pinch-plasma Batteries stand in gun houses down the
shoulders, three a side, each bearing from past the bow round its own side to past the
stern. Two blocks of eight hatched cells on the stern deck fire as one launcher, a
Belfry's gravitic seekers twice over. Its stern drives burn as
plasma (`models::plasma_drives`): violet plumes with rings carried down them, arcs at
the mouths.

Inspect the fleet with:

```sh
scripts/shot.sh variants regency_t2_transport=base regency_t3_assault_transport=base regency_t2_space_frigate=base regency_t3_space_cruiser=base regency_t4_space_destroyer=base --views front34,rear34,top
scripts/shot.sh unit regency_t4_space_destroyer --scenario targets --ticks 160 --views 200:50 --look 400,0,-500 --zoom 1   (the lance on its mark)
scripts/shot.sh unit regency_t4_space_destroyer --scenario targets --ticks 20 --views 60:-5 --look 0,0,0 --zoom 2   (the core charging)
```

Focused acceptance tests:

```sh
cargo test --profile gate -p mc-models regency::space
cargo test --profile gate -p mc-sim --test sim regency_space::
```

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
| Scourge | 3 | Artificer III; Exarch Engineering Suite III | Destroyer: a Heavy Pinch-fusion Lance from the energy core under the keel, two Gravitic Seeker Batteries |

All five are site-built aircraft tagged Space. Select an appropriate engineer,
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
units and structures. Each casemate uses its own weapon slot and pivot, so barrels
pitch and traverse with the weapon they represent.

Scourge closes to weapon range and holds station. The lance is laid from the lens of
the energy core in the pod under the middle of its keel; nothing turns. The core charges
for 2.5 s (`spin_up`: the core's charge animation and `regency_lance_charge` keep time
with it) before the beam lights. The lance is a held beam fired down its line (`sweep`):
locked on, it stays on the mark; when the mark dies the beam stays lit while it slowly
slews (12°/s) onto the next, and the stretch between lands on the ground and glasses
it (the renderer's molten-ground effect). It does not cut a navigable trench in the
simulation heightfield. With nothing left to shoot the core winds down and vents
(`regency_lance_vent`). Two blocks of eight hatched cells on the hump let out gravitic
seekers onto aircraft, as the Resolute's rocket cells do. Its stern drives burn as
plasma (`models::plasma_drives`): violet plumes with rings carried down them, arcs at
the mouths.

Inspect the fleet with:

```sh
scripts/shot.sh variants regency_t2_transport=base regency_t3_assault_transport=base regency_t2_space_frigate=base regency_t3_space_cruiser=base regency_t3_space_destroyer=base --views front34,rear34,top
scripts/shot.sh unit regency_t3_space_destroyer --scenario targets --ticks 115 --views left --look 180,0,-240 --zoom 0.5
scripts/shot.sh unit regency_t3_space_destroyer --scenario targets --ticks 20 --views 60:-5 --look 0,0,0 --zoom 2   (the core charging)
```

Focused acceptance tests:

```sh
cargo test --profile gate -p mc-models regency::space
cargo test --profile gate -p mc-sim --test sim regency_space::
```

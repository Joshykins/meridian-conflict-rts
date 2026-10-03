# Look and sound

What units, weapons and effects should look and sound like, recorded from
reviews of hero units so the rest of the roster can follow. The first hero
unit is the Warden (`aster_t1_tank`); the rules below come from its review.

## Tech tiers read at a glance

- **Tech 1 is plain.** Welded boxes, tube guns, steel and rubber. Nothing on a
  tech 1 line unit is lit: no glow strips, no emissive vents, no lit antenna
  tips, a dark bore instead of a glowing one. It should look like something a
  field workshop keeps running, and it carries the clutter of that: stowage,
  tools, spare track links, fuel drums, a pintle gun.
- **Emitters are earned.** Blue highlights, faceted shells and rail weapons
  belong to tech 2 and up, and there should be visibly more of them at each
  tier. A unit that looks more advanced than its tier is wrong even if it
  looks good.
- **Dirt follows the tier.** Tech 1 is the dirtiest (caked mud and spatter
  over the running gear and lower hull, thin runs down the steep faces, grime
  packed in the plate seams); higher tiers stay closer to parade white. The
  unit shader scales this by tech level. Dirt is fine detail over a smooth
  fade, never patches a hand to a metre across: on a hull a few metres long
  those read as camouflage (`surf_dirt`). The dust stops at
  the deck: a low hull under a tall mount (the Gnat's radar) says where its
  deck is, or the whole hull reads as camouflage blotches.
- A unit's role line says what it is in plain words: "Light Tank", not a
  class name that needs a manual.

## Surfaces

Detail belongs in the texture, not the mesh: a modelled vent or rivet costs
triangles on every copy of the unit at every zoom, a drawn one costs nothing
far away. Meshes give the forms; `shaders/surface.wgsl` draws what is on them.

- **No tiled pictures.** A repeating plate texture reads as a generic grid on
  anything big. Every face carries its own frame (`MeshVertex::face`), and
  detail is fitted to it: the outline follows the face's real edge, plates
  divide it into whole courses whose joints never line up, rivets are spaced
  to land on the corners. Change the mesh and the texture follows.
- **White plating** is courses of plates with seams, and some plates carry a
  rivet ring, an access hatch or a bank of louvres. No two plates have quite
  the same paint or polish.
- **Scratched steel under ARC's paint** (2026-10-02, `arc_metal` in
  `shaders/metal.wgsl`): a CC0 scan of brushed steel (`data/textures/metal`)
  gives the plating its grain and uneven sheen, and long scratches cut through
  the paint to bright metal, fuller on the dark trim. The Regency's steel is a
  different scan (worn iron, scattered scratches): the two factions never share
  a pattern.
- **Black has little orange lines**: level, short, one to a plate, let into a
  slot. Black cannot go darker at a seam, so its plates differ in sheen and
  their edges are scuffed lighter, which is what draws its forms. The lines
  are lit, brighter by tier, except on tech 1 line units, where nothing is
  lit: there they are orange paint.
- **Glow is drawn, and it moves.** Emission comes from the surface in HDR
  units and takes time and the unit's state: a furnace breathes and runs
  hotter while the factory builds, a print bed's grid wakes under a scan,
  apron lamps chase outward, power lines pulse toward the work. A lamp under
  dust is dimmer, not out.
- **Team colour can be paint**: a pattern can lay the owner's colour on a
  plate (a roof band, a door's head) without a separate mesh panel.
- **Specific faces ask for a pattern** (`models::pattern`, set with
  `b.paint(M).pattern(P)`, lasting until the next `paint`): shutter door,
  lift deck, marked apron, furnace louvres, power run, team band. Everything
  else gets the fitted generic treatment for its material. Gunmetal tubes
  stay plain tubes; a tube's plating wraps right round it instead of
  outlining each facet.
- **Damage shows on the surface.** As health drops, blast marks come up at
  places fixed for that unit, each growing in on its turn: ragged soot that
  climbs, paint burnt back to steel at the core, embers alight below half
  health. Plate edges chip further, and the lights in the black sputter and
  go out one by one. The wreck (burnt out all over) is a separate, later look.
- **A burn is one blotch, and never stipple.** First review: the marks "clip
  poorly, look like z-height clipping". Two causes, both rules now. Noise
  finer than a few pixels, thresholded, is a dot lattice that reads as
  z-fighting: every octave fades to its mean as it goes under the pixel
  (`surf_resolved`), and the noise is 3D so it never streaks along a face.
  And a mark is a column, not a ball: it scorches turret, deck and skirts
  under it alike, so from the game's camera it is one shape, not slices cut
  off wherever a part stands higher. Embers are hairline cracks of constant
  width, sized like the plating, never blobs or contour rings.
- **A burn smokes, and a deep one burns.** Smoke and flame rise from the
  marks themselves, never from "somewhere on the unit": a wisp of dark smoke
  from a fresh scorch, a column once it is alight, and flames (with the odd
  spark) past half health, where the embers show. How much follows the
  damage; a unit near death is alight at every mark. Flames are the turbulent
  forest-fire kind, spread over the mark's burnt middle and sized to it, so a
  tank burns from a spot and a factory over an area; the white-hot additive
  fire puff stacked on one point reads as an explosion, not a fire. Smoke is
  left where it was made, so a moving unit trails it on the map's wind;
  flame rides with the hull and only streaks back as it dies (first try left
  a row of candles hanging behind a fast tank). Damage smoke is not dust: the
  player's dust setting does not switch it off.
- The Forge (`factory_land`) is the reference for all of this.

## Weapons

- **Conventional weapons are fire-coloured.** A gun's shell is white-hot with
  an orange trace, its muzzle flash and impact are orange, it throws smoke and
  sparks. `color: Orange` in the weapon data.
- **Blue is for rail and electric weapons**, and those are rarely tech 1.
- **ARC fires no plasma.** Its tech suite is guns, rails and electric bores:
  conventional guns, cannons, missiles and flak through every tier; rail guns
  and rail cannons from about tech 2; the Argon Electric Bore and the heaviest
  rails at the top. Charged shells (a conventional round whose charge
  strikes down the last of its flight as lightning where it lands, `discharge`)
  are the Arc Howitzer's on the Trebuchet, and the Arc Cannons', three to a house
  on the Leviathan.
  It is not a strict ladder: a high-tier unit may carry a
  plain gun, and a light rail can turn up low. Arc projectors, plasma lances and
  pulse guns are gone; `plasma` stays in the data for other factions.
- **Rail guns are speed above all.** Every ARC rail is `hitscan`: the slug is
  there the moment it fires. Its whole path flashes white-hot and cools to
  orange, and a thin vapour trail hangs along it and drifts off on the wind
  (renderer `rail_beam`, sprites beam colour 5). It sounds like no powder gun:
  capacitor thunk, arc snap, supersonic crack, a tearing zip as the slug goes
  (`aster_rail_cannon` and its scaled family). Nothing is lobbed from a rail:
  heavy artillery is a conventional gun (the Culverin) or a charged howitzer
  (the Trebuchet's Arc Howitzer, "The electric bore").
- **Flak is a slow shell you can watch go up.** A powder gun (`flak: true`): a
  small hot round with a thin smoke wake, burst on a proximity or timed fuse in
  the aircraft's path. The burst is flak's signature, and nothing else in the game
  looks like it: a white blink, the charge burning inside a hard-edged black puff
  that hangs on the wind for seconds, hot
  shrapnel streaks flung out to the edge of the splash so its reach is seen, and
  burning scraps falling away (renderer/flak_fx.rs). No blue, no energy rings.
- **Rail guns are hardware, not emitters, and never look like a gun barrel.**
  A gun is a dark round tube (the Redoubt's jacketed battery, `parts::jacketed_gun`).
  A rail (`parts::rail_gun`) has no tube: two bright bare-metal rails side by side
  out of a boxy power block (heat-sink fins, bus bars over its back), the bore an
  open slot between them, a ladder of close-set dark clamp yokes along them, and
  the rails running on past the last yoke as two prongs. Light rails in dark
  clamps is the read. Long and slim: several times longer than wide. Nothing on
  them glows. The AEB is the one weapon that keeps the blue-white electric look.
- **Fewer lights.** Tech 2 and 3 no longer earn glow strips by default; the
  Paladin carries none at all (dark sensor slits, plain glass, bare metal sinks).
  Lit parts are for what really emits: AEBs, engines, work lights.
- **ARC shields are blue-white** (`shield_color` in `faction.ron`); the shader
  draws hits, the projector shaft and contact from a denser blue of it and
  flares from a paler one. The Replication Engine's veil is the Precursors' cold blue.
- **Weapon look is data.** Muzzle flash (`flash`), impact flash (`impact`, or `flash` if left out), shockwave,
  tracer size, trail, how long the wake hangs (`wake`), plasma around a traveling slug (`plasma`), energy bolts (`bolts`), a charged shell's lightning (`discharge`) and a bolt rifle's firing sequence (`arc_charge`) are set on the weapon. Shockwaves
  take the weapon's colour: blue for energy, dust for guns. The renderer scales a recipe from damage and colour;
  it does not special-case a unit.
- **Every shot is seen from muzzle to target.** A shell is drawn over its whole
  flight, its last stretch included, and its impact waits for it to arrive. At
  point-blank range the shot is a short streak, never nothing. A shell's trace
  is short (a fifth of a tick's travel): a shell, not a beam.
- An impact shows on the skin of what it hits, not inside it: sparks off
  armour, a burst of earth off the ground, smoke after either.

## The Regency look

The Regency roster is being redone from this section (2026-09-26). The grown look
(hide, tendrils, molten pools, glassed lots) is retired. References: Shockwave
and Scorponok from the Transformers films, the Driller concept art,
Megatron's Cybertronian jet, and Megatron from Revenge of the Fallen for the
commander: tall and long-limbed, every mass a faceted solid (no sticks with
blades glued on), a keeled chest, spiked pauldrons swept back, armoured boots
without toes.

- **Confident, not improvised.** ARC is a field workshop keeping stolen tech
  alive; the Regency understand what they hold. Their machines are dense and
  finished, and a higher tier is more elaborate, not more lit.
- **Symmetrical.** Most units are. The commander is the exception, as ARC's is.
- **Layered armour over exposed machinery.** Plates overlap and sweep back;
  shafts, cables and rings show in the gaps between them. Every joint is a
  visible gap, never a smooth sleeve.
- **Spikes are plate edges.** A spike is the swept-back trailing edge of an
  armour plate, never a thorn stuck on.
- **Buildings are ultra mechanical:** machinery seen working, such as rotating
  rings and shells that open and close. No gears, rams or ribbed shafts on
  buildings (2026-09-30: the user found them ugly); teeth go only on a ring that
  turns, like the toothed lift ring over a factory, which the user likes. A
  brace is a plated strut.
- **Nothing ceremonial.** No arches, obelisks, daises or crests: the look is
  machine all through.
- **The outline reads from above,** swept and pointed where ARC's is squared.
- Optics are red, as many as the unit needs.

### Finish

- **Warm graphite plating** (2026-10-02): steel a little warm of neutral, so the
  blue sky it reflects never turns it blue.
- **Darker graphite machinery** (no bronze or gold since 2026-10-02) on the exposed
  workings under the plates: shafts, joints, cables, rings, polished lighter
  where they are worked. From above a unit reads as graphite armour over darker
  workings.
- **Red highlights:** lit slots, optics and weapon heat, built into the model.
  Nothing on the plate's texture is lit.
- **Real worn steel, true to the form** (`shaders/regency.wgsl`, references: the
  2026-10-02 factory mock-ups). The plate is metal (metallic, reflecting the sky),
  finished with scanned steel (CC0, `data/textures/metal`): its grain, wear,
  scratches and uneven sheen, never its colour. The scan never repeats on a grid
  and is laid coarser on big models so it still reads at their viewing distance.
  Over it, everything follows the face it is on: each facet a shade apart, a worn
  bright edge where a face ends, the face laid in big plates by seams parallel to
  its longest edge (staggered like brickwork, grime in them), and on about half
  the broad faces one panel line round the face's own outline. A face no
  rectangle fits carries the distances to its own edges (`MeshVertex::face`, edge
  form), so all of this follows its true shape. No procedural scratches or
  noise patterns, no vents, bolts or engraving, nothing lit on the plate. The
  machinery takes the same scan's scratches, a turned collar near each column end.

### Names

Regency units and structures take short, grounded names of their own, never ARC's:
a rank for the commander, a trade for the builder, and for the rest the machine
or the job (the weapons keep their engineering names, "The Regency suite"). Tiers of a
structure that upgrades in place are numbered; the deepest mine is named for it.

| Job | Name |
|---|---|
| Commander, engineer, scout | Exarch, Artificer, Outrider |
| Wake tank, assault tripod (T3) | Wake, Strider |
| Battle scorpion (T4) | Harrow |
| Raider, light tank, mobile anti-air (T1) | Marauder, Sledge, Brazier |
| Salvage craft, light artillery (T1) | Breaker, Mattock |
| Battle tank / mobile anti-air (T2) | Glaive / Vane |
| Anti-spaceship gun, heavy artillery (T3) | Spire, Kiln |
| Land / air / naval factory | Anvil (II, III) / Skyforge (II, III) / Slipway (II, III) |
| Attack boat, submarine, frigate | Dirk, Stiletto, Falchion |
| Heavy destroyer, cruiser | Claymore, Pavise |
| Bombardment walker (T2) | Sower |
| Drone carrier and its drone (T2), beam assault craft (T3) | Quiver and Wick, Reaper |
| Battleship, carrier, assault submarine | Flamberge, Mantlet, Rapier |
| Air scout, fighter, bomber (T1) | Flechette, Quarrel, Petard |
| Light transport (T2), salvage drone (T1) | Coffer, Sickle |
| Interceptor, strike drone, torpedo bomber, reclaim carrier (T2) | Pilum, Voulge, Trident, Winnow |
| Mine | Excavator (II, III), Deep Excavator |
| Power | Power Generator (II, III) |
| Storage | Reliquary |
| Point defence T1 / T2 / T3 | Picket / Halberd / Sunspear |
| Anti-air, radar, wall | Canopy, Orrery (II, III), Palisade |
| Shield generator (T2, T3) | Ward (II) |
| Nuke silo, nuke defence (T4) | Mangonel, Barbican |
| Air superiority fighter, strategic bomber (T3) | Partisan, Maul |
| Spy plane, heavy scavenger (T3) | Augur, Scythe |

### Construction

- **Construction is violet,** the Regency's counterpart to ARC's amber: it marks
  what can build.
- **Nanites build.** A structure forms out of violet light that cools to red
  and then to plate, with thin rings and slow beams of particles round it
  (user direction, 2026-09-26; it replaced a black swarm).
  - The builder shoots **strands of particles** from its violet emitter, not one
    beam and not an arch: a few hairline threads that writhe like liquid,
    beaded with motes drifting slowly along them, violet as they leave and red
    as they arrive. They bow a little apart and turn slowly round the line
    between the ends. Their heads creep out when the work starts, and their
    tails drain into the site when it stops (`beams.wgsl` `BEAM_NANITE`). A
    Regency factory shoots them from each of its fabricator heads.
  - The hull **forms from the ground up.** What has just formed glows violet,
    then slowly turns red, then becomes the finished plate. The band is a few
    metres deep on any hull. A thin hot line runs along the front with a haze
    of violet motes just above it. Nothing shows above that (`entity.wgsl`
    `nanite_site`).
  - Round the site while it is fed (`BEAM_NANITE_SITE`, one per site), **violet
    rings** of many sizes come and go up its height, most of them near the
    front. Each turns red as it fades. They stay mostly inside the hull and
    never reach far past it. **Sheaves of red particle filaments** are thrown
    slowly up out of the lot, fanning apart and leaning a little outward,
    higher than the hull will stand.
  - The site's work light is still a tenth of ARC's amber. A work lamp's worth
    of violet floods a hull purple.
  - It sounds **deep and heavy**, a great machine felt more than heard. The
    loop is a throbbing sub note with its harmonics and a rush of air over it.
    Coming on, a deep thud and the sub swells up onto the loop's note while the
    rush climbs. Stopping, everything sinks away below the loop's note and a
    last deep thud settles it (`regency_nanite`, `_start`, `_end`).
- **The foundation is no flat square and no brick paving.** A Regency lot is black
  and graphite and round: a black machined hub under the building, a graphite thread
  running round it with beads strung on it (three a quarter, each smaller than
  the one before), and graphite lines out to smaller discs spread towards the
  lot's corners (one or two of those never laid), with bare ground between.
  Every disc is ringed in graphite inside, circle within circle. While the
  building goes up, faint violet runs out along the edges and ring cuts
  (`ground.wgsl` `lot_plate`, `nanite_lot`). Not overbuilt, not formal.

## The Regency suite

ARC's tech is human and hard-won; the Regency's is Precursor-derived and
understood (docs/LORE.md). The Regency fires plasma; ARC never does. Their one
sci-fi exception is **gravity manipulation**, and their power, weapons and
guided weapons all come from it. As with ARC, it is not a strict ladder.

- **Power: star cores.** Gravity pinches plasma into a small caged star
  (pinch fusion under star conditions). A higher tier holds a bigger star. The
  star burns in pinch fusion's prism (below), and it is light, never a solid
  ball: a boiling, crackling face with a ragged corona (plasma_puffs.wgsl
  `star_core`). Its gravity rings tumble round it each on its own axis. Each
  tier is held differently (cradle, yoke, crown), so the three read apart. A
  breached core collapses inward before it flashes out, never a plain
  explosion.
- **Weapons are plasma, and it is heavy.** Hyper-advanced plasma weaponry:
  never ARC's look (no shock ring, no dust and clods, no powder smoke or
  muzzle flash) and never comical (user direction, 2026-09-30, replacing
  "plasma that behaves like a slug of metal"). Each grade has its own charge,
  shot and strike (`renderer/regency_guns_fx.rs`, `plasma_puffs.wgsl`):
  - **Plasmeric bolt** (a plasma repeater, as Halo's): a steady stream of fat
    red bolts round a white-hot heart, their skin boiling and licking back off
    them, lighting the ground red as they pass.
    Each is spat out hard: a white-hot snap at the mouth, a red bloom thrown
    forward, droplets and sparks flung after it. Where one lands it dumps its
    heat at once: a white flash in a ragged red bloom, a knot of plasma left
    frying, droplets spattered out, sparks, a seared glowing spot. Heard as a
    snap, a heavy spit with a kick under it and a fizz riding off; it lands as
    a wet splat, a thud and plasma frying. **Plasmeric AA repeater** ripples the same bolts out of each tube in
    turn, and each bursts as a wide red bloom flinging sparkles and streaks of plasma.
  - **Pinched-plasmeric:** plasma gathered and squeezed in front of the bore
    (a ball, motes drawn in, red lightning snapping into it), then pinched
    out as one shot: a jet a quarter of a second long, a hard flash as it
    opens, the ball draining into it, a red wake hanging behind it; its head
    bursts in a billowing
    red bloom over a white heart, throws a spout of plasma up and molten
    spatter and globs out low (in place of a shock ring) and sears the ground,
    and the rest of the jet pours in after it.
  - **Pinch-fusion:** the same charge far harder: the ball is a star, as a
    power generator's (white-hot, the prism drifting over it), lightning
    crackling round it and pulled into it. The gun works with it (the
    Sunspear: its rails part, its lens heads slide out, its coils light from
    the breech in the prism going white, its gimbal cage spins up round its
    star core; after the shot its radiators vent and it cools through lavender
    to violet). Every light on a Pinch-fusion gun is the star's
    (`GLOW_PRISM`), not the red of the lower grades. It launches with a
    blinding white flash and a cone of the star's streamers, pinching out a
    jet of fusion, long and fast (a white-hot core in a sheath of the prism,
    the fastest Regency shot), arcing a little onto its mark (laid flat,
    `flat_fire`) with a long hot trail taking the prism and cooling to violet
    behind it. Where it lands it goes off as a small supernova, drawn with
    the dying power generator's own pieces: a blinding flash, a hollow shell
    tearing outward, white, then the prism, cooling to lavender and violet,
    streamers round its waist, up from it and every way, lightning into the
    ground; it melts the ground into a wide glowing pool and leaves a small
    star burning over it for seconds, slowly letting white lightning go while
    lavender sparkles drift off it. No reds or oranges anywhere in it.
  - **No spirals, no rings:** the plasma boils, churns and billows in cells
    and lumps; nothing is wound round a middle in arms (user, 2026-09-30).
  - **No mist:** plasma is hard-edged and goes out fast. A trail is a hot
    filament that cools and breaks up, not a chain of puffs; a blast is a
    sharp heart and torn filaments, not a soft bloom, and what it throws is
    eaten through as it cools rather than spreading into a haze (user,
    2026-10-01: the puffy trails and blasts looked bad).
  - **Sound:** plasmatic and heavy: compression, sizzle and roar, never a pew,
    a zap or a cannon crack.
- **Weapon names are grounded engineering:** a grade that says how the shot
  is made, then the gun that fires it, the way ARC's Argon Electric Bore
  does. Every Regency gun sets `plasma_grade` (`Plasmeric`, `Pinched`,
  `PinchFusion`), which names its kind on the HUD; no ARC gun does (a test
  holds both). The three grades mirror ARC's direct-fire ladder, and like it they are not a strict tech ladder:

  | Rung | ARC | Regency | What the shot looks like |
  |---|---|---|---|
  | 1 | Cannon | **Plasmeric** | A plasma bolt: a fat red teardrop with a pink-white heart |
  | 2 | Railgun | **Pinched-plasmeric** | Plasma gathered and condensed by gravity in front of the bore, fired out as a tight, dense stream |
  | 3 | AEB | **Pinch-fusion** | The condensed plasma is pushed until it fuses (white, every colour round its rim) and launched: bright white bursts strobing along the stream |

- **Pinch fusion's light is the prism.** Where plasma fuses it burns
  white-hot, and only toward its edge breaks into a turning pastel prism:
  rose, magenta, lavender, peach-gold (the Bifrost; a pink-white beam in
  space). No blues or greens. Mostly white, a little colour. The star cores
  have it, and the Pinch-fusion guns are drawn with them: their charge is a
  star, their strike a supernova, their lights the star's. The lower grades
  stay red. One colour source:
  `gpu_consts::prism` and common.wgsl `prism`.

- **The set**
  - **Direct fire**
    - **Plasmeric Repeater:** the everyday rapid shot, visibly slower than a
      rail, on everything from scouts and line units to the commander, point
      defence and vehicle mounts. Size shows in the mount (Light, Twin,
      Heavy), not in a new name.
    - **Pinched-plasmeric Rifle / Pinched-plasmeric Cannon:** snipers / tanks
      from tech 2 and heavy defences (the tech 1 Sledge carries a Plasmeric
      Repeater, user call 2026-10-02). The Regency's answer to an ARC rail; it stops in what
      it hits.
    - **Pinch-fusion Rifle / Pinch-fusion Cannon:** the same roles on the top
      rung, the Regency's counterpart to the AEB.
    - **Pinch-fusion Bore (PFB):** the heaviest pinch-fusion gun, for the
      biggest units (as the AEB-2 is to the AEB).
  - **Beams:** continuous fire, swept across whatever is in front of them,
    glassing the ground they cross. Only on big things (battleships, titans,
    top defences).
    - **Plasmeric Beam:** a dense red beam.
    - **Pinched-plasmeric Beam:** the middle grade, a tight dense stream
      held on the target (the T4 battle scorpion).
    - **Pinch-fusion Beam:** a grade up, with fusion bursts strobing along
      it. The **Orbital Pinch-fusion Beam** is the campaign's glassing beam.
  - **Indirect fire:** **Plasmeric Mortar**, **Plasmeric Howitzer**.
  - **AA:** **Plasmeric AA Repeater**, bolts out of each tube in turn that
    burst into a spray of plasma.
  - **Air-dropped:** **Plasmeric Bomb**.
  - **Thrown:** the **Gravitic Bomb**, a plasma charge in gravity containment,
    charged in the hands and launched to land around its target (the T4
    battle scorpion's claws).
  - **Guided: the Gravitic Seeker.** The Regency's missiles: a gravity
    containment holds a plasma charge and steers it onto its target. No
    exhaust plume. Drawn as its charge, not a rocket (renderer/gravitic_fx.rs,
    sprites.wgsl `gravitic_seeker`; any missile with a `plasma_grade`): no
    body or motor flame. It leaves its cell with a hard red flash and
    filaments snapping in on it; in flight it is a hard-edged lavender-white
    heart in a violet body, drawn out a little behind, held in a faint gravity
    lens that shimmers at its edge, and it leaves a black smoke tube that
    glows violet just behind the charge, spreads and goes grey as it hangs.
    Violet and black smoke are the sign of a Regency missile a defence can
    take, set apart from ARC's white smoke; only the missiles are violet, so
    the counter-seekers that hunt them stay red. Where it strikes the lens snaps in and lets
    go: a hard red burst over a white heart, filaments torn out, globs and
    sparks thrown out low, the ground glassed under it; sized by its damage
    and `impact`, so a heavy seeker's is many times a battery seeker's, and
    it throws red lightning into the ground. The family covers every role
    ARC's missiles do:
    - **Gravitic Seeker Pod:** salvos.
    - **Gravitic Seeker Battery:** AA.
    - **Heavy Gravitic Seeker:** tactical and cruise strikes. Its charge
      gathers over the cell first, a red ball swelling as motes are drawn in.
      The Sower's are lobbed high and part over the mark into six sub-seekers
      that rain round it (`Weapon::cluster`): a hard white heart in a red flash
      and sparks where the cage parts, each piece a smaller seeker that strikes
      at the size of its share.
    - **Gravitic Counter-seeker:** missile defence (a faction's
      `anti_missile_look: CounterSeeker`; ARC's is the laser). A small red
      charge thrown off the mount runs the missile down along a cooling
      filament and bursts on it, small, hard and short, the tick the
      defence kills it; a burn let go without a kill fizzles where it got to.
    - **Gravitic Interceptor:** anti-nuke.
    - **Gravitic Torpedo:** the same containment steering under water (also
      air-dropped); it boils the sea around a hull, and its hit is a steam
      blast.
  - **Strategic:** the **Pinch-fusion Warhead**. Every race has a nuke of
    about the same yield with its own effect. This one is red: a small star
    forms, holds a beat pulling debris in, then flashes, and leaves a glassed
    crater that burns red for a while; no mushroom.
- **Nanites** build and take apart (the Regency reclaim). They are never a
  weapon.
- **Shields:** bubble shields under the same rules as ARC's (they draw energy
  and drop in any stall). The Ward (tech 2, upgrading to Ward II) and the
  Exarch's Personal Shield. A Regency field is a **prism veil**
  (faction.ron `shield_look: Prism`): white-hot glass with pinch fusion's
  prism drifting over it, the colour turning with the angle it is seen at,
  folds of brighter colour hanging in it like a curtain, on a lattice of
  red-tinged triangles where ARC's glass has hexes. The shaft that climbs to the
  crown is a white-hot jet in a prism sheath, born in the Ward's small caged
  star; a hit flashes white and rings out red along the lattice. A Ward's
  breached cage frees its star, which goes nova like a power generator's.
- **Radar is radar,** the same as ARC's.

## The fusion plants

ARC's power plants (`models/aster/reactor.rs`) fill their lot and read four-fold round
the middle. Only tech 3 holds an orb. Tech 1, the cell, is a solid armoured column on a
round cradle with two segmented rings turning the opposite ways round it, stepping down
outward so the column shows; arcs jump from the column to the inner ring and across to the
outer one. Tech 2 is a faceted, squared-off dome over the whole lot, windows in its sloped
facets onto blue fusion streaming inside (`pattern::FUSION`), buttresses on the diagonals,
a ring turning round its crown. Tech 3 is a white-hot star with a blue limb held up on four
panels that arc to it, over the heaviest foundation (heat sinks on splayed feet, their fins
lifting in a wave; capacitor bastions), with two blade sets round the body's head on rings
of their own: four short blades inside, two tall ones outside, turning the opposite ways.
Anything standing on a plant stands on a still deck: never on a turning disc. Not wanted
(the user turned each down, 2026-09-23 to 2026-10-01): heat sinks on tech 1 and 2, pumps
and pistons, a turntable under the core, a big or blotchy orb, orbs on sticks, towers,
arches, cooling towers, a cup the core sits in, plain stacked cylinders. A plant going up
breaks its charge loose and bursts electric blue on the tick it dies, never after its wreck
shows: a ball of ionised air, lightning out of it and along the ground, a blue shock ring.
Never a fireball or a mushroom cloud (the user,
2026-10-01); those are the commander's and the warheads' (`renderer/reactor_blast.rs`).

## The electric bore

The Argon Electric Bore (AEB) is Aster's lightning gun: the Arbalest (tech 3
sniper) carries one, the Fulgur (tech 4) an AEB-2. The Arbalest is drawn in the Fulgur's
language: a slim faceted barrel with a dark core in swept strakes and a stepped
crown, capacitor drums on the turret's back, blue only in thin seams, no lit rings.
It fires its four ground stakes in to plant, as the Trebuchet does. The Raptor (tech 3 air
superiority fighter) and the Paladin (tech 3 assault walker) carry the
projectile form, the bolt rifle, and the Trebuchet (tech 3 mobile artillery)
its siege form, the Arc Howitzer: a long charge that swells blue in the mouth,
then one great bolt lobbed high that bursts in lightning where it lands. The
bolt rifle's form: no channel, a fast bolt of blue plasma (a
round white-hot head in a soft blue sheath, a tail narrowing and dying away
behind it, `plasma`) that bursts in a flash and forks of lightning thrown out
every way round the hit (`discharge`, drawn as an air burst when the hit is
off the ground). The bolt is the messenger, never a beam: it leaves only a
faint thread, and the burst's forks re-strike and its glow lingers for over a
second.

The bolt rifle itself is its own hardware, neither the bore's ringed barrel nor
the rail gun's open rails: a closed, faceted body round a hidden bore, its
blue only thin seams. The Paladin's is a sculpted housing with a plasma cell on
its outboard flank feeding a slim core barrel, swept radiator blades along the
core and a stepped, ported muzzle collar. The Raptor's is the fighter's cut:
a dark core in a light sleeve cut on a slant, a dark cowl over its back.
The Marlin carries the Paladin's gun a size down in its forward gun house, on a
slight lob so it reaches over a headland.
The Paladin's shoulder carries the gun back into itself: its armour hoods forward
over the gun's housing on a slant, the gun's plasma cell runs on as a long canister
down its outboard flank and the gun's seams run back along it. The whole shoulder
pitches on its axle to aim, and each barrel kicks back into its housing on its own shot.

The Paladin's rifles fire in a sequence (`arc_charge`, the gun's length; the
charge time is the weapon's `charge_time`; renderer/bolt_rifle_fx.rs): over the
charge small arcs crawl along the radiator blades and jump between their edges
and the core, starting by the housing and running forward, denser as it builds,
and in its last part a crackling knot curls round the muzzle collar; on the shot
a short blue-white flash, a few forked strokes snapping a few metres out of the
muzzle, sparks, haze venting from the housing and the blade seams flaring and
cooling. Only arcs and seams glow, never a lit knot at the muzzle. The Raptor's
rifles fire plainly.

The Trebuchet's howitzer is the same family made a siege piece, wide and laid
up at rest in a well between the trunnion shoulders of a low sloped armoured
house: a faceted breech housing with a plasma cell on each flank, a long fat
core under six radiator blades, and a plain stepped collar round a wide bore
(no flared muzzle brake). Before it fires, the Trebuchet stakes itself down:
one corner after another (front left, rear right, front right, rear left) a
launcher tube swings down from along the fender and fires a heavy spike into
the ground, still gathering speed as it strikes; the tube kicks back, the
ground takes a shock and throws up earth and dust, and a hard thump is heard
(`aster_stake_drive`). Packing, it all runs the other way.

The howitzer fires in a sequence of its own (`howitzer`: the tube's length and
housing radius; renderer/arc_howitzer_fx.rs), bigger than the rifle's and built
up the whole way to the shot: the plasma cells' seams light and arcs crawl over
them, arcs run up the blade roots and jump from blade edge to blade edge, on a
house of several tubes (the Leviathan's Arc Cannons) arcs jump between the tubes,
and in the last part the collar crackles, arcs spit off the muzzle and a knot of
plasma swells in the bore with sparks drawn in to it. The shot is a white-hot
core in a blue bloom, a plasma jet down the bore, a shock front, vapour thrown
out flat and ahead, forks of lightning and blue sparks; after it the blade seams
flare and cool, arcs die off along the tube and the cells vent haze. Its sounds
are data/sounds/arc_howitzer.ron: a charge that climbs to the shot and overloads,
and a report that holds and rolls back as thunder.

- **A shot is one bolt, at once.** The AEB and AEB-2 are `hitscan`: no round is
  seen leaving the gun. After the charge the whole channel lights from muzzle to
  strike: a straight, sustained white-cyan plasma column, surrounded by five
  branching lightning return strokes over 1.28 seconds. The column has a blue
  sheath and travelling density ripples; the arcs wander around it and light the
  ground with each pulse. The blast at the end is the weapon's own splash. Both
  launch and impact send out blue pressure fronts; the Arbalest's impact wave is
  especially pronounced for its size.
- **The AEB-2's strike goes off** (`bore.blast`, `bore.blast_time`; renderer
  `bore_blast`): a blinding flash, two pressure fronts that bend the trees, a
  white-hot heart swelling into a churning blue ball of ionised air that lifts off
  the ground, lightning re-striking out over the ground round it for seconds, what
  it hit burning inside it and climbing off as soot, and a glassed crater. It burns
  about six seconds. The Arbalest's strike has no ball: only its splash.
- **The AEB-2 does not aim well** (`spread`), and its charge sears everything
  within `bore.width` of the channel on the way (`bore.damage` each), scorches
  the ground under it. Low channels melt the ground; a shot high over a valley
  leaves the floor alone. Every bore, including the Arbalest and compact AEBs,
  burns trees along the entire projected channel (4 m half-width minimum).
- **The ground it runs over goes molten, then cools** (`bore.cool` seconds): the
  ground holds heat per square metre (renderer/ground_melt.rs), so burns that
  overlap run together into one surface and ground under steady fire heats past
  what one hit gives it. Past the melt point it glows white-yellow, orange, then
  dull red, darker skins drifting on it and joining up as it cools, the last
  glow in a wandering web of veins (no plates, no polygon cracks). Where it
  melted it is left black-green glass for the match, charcoal scorch round it. The Arbalest leaves only a small
  pool where it strikes. It melts as fast as the channel is low: a ground gun's
  track melts at once, while under a shot struck down from high up the ground
  takes seconds to heat, glowing up from the strike outward and slowest under
  the channel's high end.
- Sound (`data/sounds/bore.ron`): capacitors filling (two contactors, a climbing
  stack), a crack at the muzzle as the charge goes, then the strike: a snap, a
  buzzing stack falling fast, a deep thump and thunder rolling back twice. The
  strike is heard from the point of the bolt nearest the camera, so a shot seen
  leaving the gun is loud however far off it lands. The AEB-2's charge and fire are
  the Arbalest's larger; its strike is its own, deeper and longer: a detonation
  felt more than heard, crackle re-striking as the ball burns, a low roar and a
  long roll.

## Experimentals

Tech 4 machines are too big for any factory: Mason IIIs raise them on a lot of
their own, like a structure (`footprint` on a mobile unit), and the finished
machine drives off it. The Fulgur is the first: a super-heavy assault tank,
four tracks, a hull field, the AEB-2 on the main turret and gun houses of its
own on the hull (`hull_mounts`: a Paladin-pattern bolt rifle on each sponson and a
rotary AA gun, the Sparrow's gatling, on the engine deck that rests facing aft,
`facing: 180`, raised to the sky like every land AA gun at rest). The
turret is broad and centred, with armoured cheeks rising either side of the gun.
The AEB-2 sits well back between them: a faceted housing with twin capacitor pods
on its back, a dark core carried in three swept strakes, a stepped ported crown.
As the ARC's heaviest gun it is dressed in plates and thin seams, not the bore's
lit induction rings (the user disliked those on it, 2026-09-28). The main
pressure-wave radius is 3.33 times its former size. The Arbalest keeps the ringed emitter, its gun
and breech centered on the turret, and reaches 520 m, on a pitching trunnion
drum and receiver sleeve that overlaps the barrel throughout recoil.

## Ground contact

Tracked vehicles mark the ground and raise dust while they move. Marks fade
over about a minute. Both are drawn only near the camera.

## Foundations

- **The lot is paved, kerb to kerb.** A building stands in its lot with a
  walkable apron round it (economy structures at least 5 m back from the lot
  edge); the whole lot is laid in 4 m precast slabs on a world-anchored grid,
  so lots side by side pave as one yard, not tiles. The user asked for this
  (2026-09-23) after the old poured-plan slab, textured with the armour plate
  map (bolts, access hatches), "looked terrible".
- Weathering is anchored to the world (broad grime, oil blots) so it never
  outlines a single lot; the only per-lot marks are a worn team L in each
  corner and a contact shadow where the building meets the slab.
- **Never under water.** The pad stops at the waterline, a little wet near it;
  an offshore rig stands on its piles.

## Ore fields and materials

- **One red-orange for everything materials** (`hud::MASS`, 0xFF6B3D; linear
  `(1.0, 0.147, 0.047)` in ground.wgsl): numbers, fields, mine points, the
  minimap. The sim still calls it `mass`; the old mint green is
  now `hud::HEALTHY`, for unit health only.
- **In play an ore field is a faint outline**, nothing else. A field a mine
  the viewer has seen is working gets a brighter rim and a light fill, the
  same hue, so worked ground reads on the strategic view and the minimap.
- **Mine points** (placing or selecting a core mine, a builder with one
  queued, or holding Ctrl): every ore field holds one point, and the fields
  light up. Each point in view is a ring on the ground just outside a mine's
  lot: a free one dashed in the materials red-orange, turning slowly; a taken
  one solid in its mine's side colour; one the viewer's builders plan, grey
  dashes turning the other way. Far out a ring becomes a small diamond, and the
  minimap shows the same as dots. Placing a mine snaps it onto the nearest
  point, with what it would make there and its payback over it.
- **The core mine is major infrastructure**: a 3x3 lot (the model is
  authored at 7x7 and shrunk evenly), and it digs. The user
  found the first one "too mechanical" and wanted to see the hole: the middle
  of the lot is a pit cut into the rock in faces and benches, ore seams glowing
  in its walls, and a bore down from its floor further than the eye can follow.
  A headframe straddles it and drives pipe down the bore in a steady beat: the
  driver is hauled up, the next section rises out of its magazine and swings in
  over the string, the driver drops and the whole string goes down a section.
  All that should be seen is pipe going further and further down into a hole.
  Every piece must do a job you can read off it: the user disliked vents,
  gears and menacing bits that seemed to do nothing (talons, claws, blade
  halos, spikes). The tiers start plain and grow into better machinery, each
  adding to the last: tier 1 an open four-legged headframe with the winch in
  its head, hoist cables, the magazine, a few spare sections and one ore
  line; tier 2 ore lines on all four sides, a tower stage with a sheave house
  cabled to a winch house on the deck, more spare pipe; tier 3 the tower clad
  in plate and glowing induction coils round the rails, fed by capacitor
  banks; tier 4 (the deep core) a heavier driver on a longer stroke, fatter
  pipe, twice the coils and capacitors, and a small shockwave with each blow. Every tier strikes with the
  same sound (`mine_blow`). Built out at sea it is the same unit as an
  offshore rig: raised on stilts, no pit, the pipe going down through a moon
  pool into the water.
## Light

Bloom is a soft halo around things that emit light (emitters, flashes, hot
sparks), never a glow on sunlit plating and never a visible ring or blob. If a
small emitter reads as a ball of light from a distance, the emitter is too
bright or should not be there.

## Strategic icons

An icon is a picture of the thing: a tank is a tank in profile, not an abstract
shape the player has to learn. The HUD draws the same pictures as the
battlefield (`hud/icons.rs`, `shaders/icons.wgsl`). Shapes that are still
abstract (diamond for artillery, triangle for bots) are candidates for the
same treatment.

## The commander

- **Black and white armour, not a statue.** White plates (`PLATING`) sit over
  a black underlayer (`ACCENT`): the black is the gap between the plates, not
  a graphite hull. Grey (`METAL`) is joints and pistons only. From the RTS
  camera the top surfaces decide the read, so the helmet and pauldrons carry
  white plates with black between them, and the team colour.
- **Lean, long in the limb.** A thick torso on short arms reads as a toy. The
  arms hang to the waist and the rifle and the projector are most of a forearm
  longer than the hull is deep.
- **It strides; it does not shuffle.** A walker's legs are posed from the ground
  it has covered, so a planted foot stays planted. That alone is not enough:
  many small quick steps with the feet barely off the ground look like treads.
  The commander takes long, high steps (16 m to a cycle, feet lifted nearly two
  metres, planted for under half the cycle so there is a moment in the air),
  the body sinking onto each step. Each footfall raises dust, and **is heard
  when it lands**: footsteps are one-shot sounds (`step` in a unit's `sounds`)
  timed by the same stride as the model, never thuds baked into a loop. The
  Paladin and Lancer use the same rig at a walk.
- **It aims with its whole body, and smoothly.** The torso turns to the target
  and the forearm pitches up or down onto it (the rifle at what it shoots, the
  projector at the middle of what it builds); the shot and the beam leave from
  where the arm really points. Anything that turns is interpolated between
  ticks like the hull is: a turret stepped ten times a second reads as jitter.
- **One torso, one job.** The gun arm and the build arm share a torso. It turns
  to face what it shoots or what it builds, it builds only once it points
  there, and while it works its weapons hold their fire. Work it was ordered to
  do wins over targets of opportunity; stop it and the gun comes back up.
- **Refits show, each where it belongs, and never crowd the back.** Engineering
  suites are built into the left arm (and Suite III down the leg), not onto a
  pack. The bare commander has nothing on its back: the back is a slot, taking
  the Material Formation Engine (a wide, low block with orange-banded drums) or
  the Personal Shield (a narrow core under two tall lit fins). The right arm
  always keeps its machine gun; the cannon bolts on over it and is rebuilt as
  the rail cannon. The right shoulder takes a flak cannon or a howitzer, the
  left a second projector that folds out over the shoulder to build and hangs
  behind the upper arm when it is not. Two alternatives in one slot must differ
  in silhouette, not only in colour.

## Construction

- **Construction is amber** for ARC (the Regency's is violet, see "The Regency
  look"). Yellow-orange (`GLOW_AMBER`, the `AMBER` of the
  shaders) marks everything that builds: the lens and collar of a build arm,
  feed tanks, conduits, and the build beam itself. Blue stays energy weapons,
  orange stays guns. Build emitters run hotter while the unit is building.
- A build beam is a steady hot thread from the emitter to where it meets the
  work — the near side, the point the builder is printing from — with pulses
  running down it and sparks where it lands. It does not wander: it is a tool
  held on its job.
- What is being built fills in all over: plates of the finished mesh appear
  throughout the volume as progress rises, a scanning amber hologram of what
  is still coming. Work light runs out in waves from the weld — white-hot at
  the front, warm behind it. Stopping the beam leaves the hull where it is;
  a new weld never rewrites what is already up. The last stretch is the whole
  thing cooling off into the finished material.
- The beam sounds like a projector depositing: a held fifth with a mid weld
  (`build_beam`, a loop). It is heard coming on and shutting off: a contactor
  and the projector catching (`build_start`), a contactor and the machine
  running down with a light latch (`build_end`), both made of the loop's own
  notes. The same shape as reclaim, a different voice — higher, cleaner, no
  grind. All three sounds are in `data/sounds/build.ron`.
- **A refit pins the unit.** While it is being upgraded a mobile unit cannot
  move or build (orders given meanwhile wait behind the refit), but it still
  shoots. Only Stop, or cancelling it in the queue, ends a refit.
- **An upgrade is seen being built.** What the next tier adds is part of the
  model (`MeshBuilder::upgrade`): nothing until the refit starts, then an amber
  hologram, then each piece goes up in turn, hot at first and cooling into the
  finished part, while work light passes over the unit and the welding sparks
  concentrate where the change is (a commander's build arm). The finished tier
  is the same pieces, plain. A mass extractor is refitted the same way: the
  next kit is built onto the wellhead, and the cracks it mines stay in view.
  Other structures (factory and the rest) rebuild like any other site, and an
  engineer helping them prints with a build beam.
- Higher engineering tiers read as more build kit, not as decoration: more
  nozzles on the arm, more tankage on the pack, masts, conduits.

## Repair

- **Repair is mint-green**, the mass colour, and only repair is that: a thin mint
  core, teal around it, deep green at the edge. The beam is a tool, thinner
  than reclaim's cone, tight at the projector and a little wider where it
  meets the hull.
- The hull is seen being patched. Hard-edged plates leave the emitter white-hot
  and settle onto the armour, cooling through teal to green as they seat. The
  opposite of reclaim: nothing is torn off, nothing flies back. No mesh is
  involved: it is all in `beams.wgsl`, from one record per beam (`kind` 2).
- **Nothing pops.** A beam that comes on is empty and fills from the emitter;
  one that shuts off stops sending plates and lets those in flight land.
  Only the light itself comes and goes quickly.
- Construction stays amber. Repair never borrows that, and never the
  white-orange-red of reclaim.
- The beam sounds like a stitch: a mid pulse with a seating tick
  (`repair_beam`, a loop). It is heard coming on and shutting off: a contactor
  and the stitch catching (`repair_start`), a contactor and the machine
  running down with a light latch (`repair_end`), both made of the loop's own
  notes. The same shape as build and reclaim, a different voice — tighter,
  mid, no grind and no projector fifth. All three sounds are in
  `data/sounds/repair.ron`.

## Reclaim

- **Reclaim is white, orange and red**, and only reclaim is all three at once:
  a thin white-hot core, orange around it, red at the edge. The beam is a
  cone: wide where it grips the target, narrow where it enters the emitter.
- The target is seen coming apart. Hard-edged shards tear loose from all over
  it, dull red, and tumble up the beam faster and faster, heating through
  orange to white as they reach the emitter. No mesh is involved: it is all in
  `beams.wgsl`, from one record per beam.
- **Nothing pops.** A beam that comes on is empty and fills from the target;
  one that shuts off stops tearing bits loose and lets those in flight arrive.
  Only the light itself comes and goes quickly.
- The reclaim pointer wears the same colours: its three arrows heat from red
  through orange to white as they chase each other round.
- Orange and red cover what is behind them (premultiplied alpha). Added to
  grass as light they turn yellow.
- What is reclaimed to nothing goes quietly: a soft flare and a few embers.
  No blast, no smoke, no wreck, no scorch mark. (A commander's reactor goes up
  regardless.)
- The beam sounds like a machine pulling: a low throb with a grinding edge
  (`reclaim_beam`, a loop). It is heard coming on and shutting off: a contactor
  and the throb catching (`reclaim_start`), a contactor and the machine running
  down with a dull thunk (`reclaim_end`), both made of the loop's own notes.
- **No swooshes.** Rising noise sweeps for "bits being drawn in", and a rising
  whoosh when the last of something goes, were tried and sounded fake. What is
  reclaimed to nothing has no sound of its own: the beam shutting off is the
  end of it. All three sounds are in `data/sounds/reclaim.ron`.

## Death

- A unit going up is an event, not a big impact: a white-hot detonation,
  secondary blasts walking across the hull, burning fragments thrown wide, a
  dust ring along the ground, then fire that turns into black smoke over the
  wreck for a few seconds.
- The wreck is the unit broken, not the unit painted black: the turret blown
  off and lying clear to one side, the hull crumpled and caved in where it was
  hit, settled crooked. Every wreck breaks differently. Burn shading comes from
  the model position, so faces in the same plane never flicker against each other.
- Wrecks are dark: burnt steel and soot, well below the ground's brightness, so
  a field of them reads as scrap and not as parked units.
- **No wreck is made by hand for a unit.** How a wreck lies comes from its model
  (size, height, length), its domain (structure, land, sea, air, spacecraft) and
  how it came down (in place, out of the sky, sunk). A structure slumps onto its
  lot; a vehicle settles into the dirt; a ship lies in the silt with the list and
  some of the trim it sank with; an aircraft is driven in nose first; a
  spacecraft, a big ship or a big aircraft breaks into sections along its
  length (more of them when it came out of the sky): it breaks its back, the
  pieces in line with a gap between them, each dug in at its own angle, torn
  ends crushed and dark inside. Wrecks go into the ground shallowly: enough to
  sit in it, never so far the hull is lost. The plating is
  crumpled, dented and twisted along the hull.
- Blasts wear wrecks away (`mc-sim/src/wreck_damage.rs`): what is left is
  flatter, lower in the ground and ragged from the top, and a wreck with nothing
  left is gone. Reclaim wears it the same way.
- **A warhead is the biggest thing in the match, and a commander going up is a small one**
  (`docs/NUKES.md`): the same slow nuclear blast, smaller. What follows about the
  commander's death is how it read before; the flash, the stalk and the fire hold.
- **A commander's death was the biggest thing in the match.** A flash that whites
  the whole view out and only slowly lets it back (no edge to it: what is seen
  is where it stops saturating), a shock front and driven dust out to the edge
  of the blast, a fireball rolling up into a cloud on a stalk, burning
  fragments thrown far, fire in the crater for a long while. Fire on that scale
  is dimmer and more ragged (`PUFF_FIREBALL`), or it becomes white discs. Its
  sound (`commander_death`) is nearly all bass, ten seconds long, and heard
  wherever the camera is: containment cracking, a breath, the detonation, two
  more heaves of the ground, thunder rolling away.

## Aircraft

- **Aircraft are drones.** No canopies, no cockpits: a sensor window or slit
  where a pilot would sit.
- **Each airframe has a planform of its own that reads from above at play
  zoom.** The camera looks down, so the outline seen from above is what tells
  units apart; colour and detail come second. Never build a family of aircraft
  from one shared fuselage-plus-delta helper with the numbers changed: that made
  the Shrike, Peregrine and Raptor the same white dart at three sizes.
  The fighter line now reads as: Shrike (tech 1) a straight, unswept wing square
  across a dark body, a V-tail; Peregrine (tech 2) a long needle behind a black
  radome, a small delta far aft, a missile on each wingtip reaching ahead of the
  wing; Raptor (tech 3) forward-swept wings, big canards, and its two bolt
  rifles (slant-cut sleeves) out ahead of the nose like mandibles. The Argus (tech 3 radar and
  missile-defence picket) has the only joined wing: a low wing swept back and a
  high one swept forward off the fin, meeting at a pod on each tip, a diamond
  from above under a turning lens rotodome. Its anti-missile lasers are the red
  heads on the tip pods, where the beams leave.
- **White armour over a dark frame**, as on ground units: mostly-white aircraft
  look flat. Hard chines and flat faces (hull stations lofted with
  `air::band`), not round tubes.
- The weapon is visible and sits at its muzzle: a missile on the rail it
  launches from, a lance whose tip is the muzzle in data.
- **A VTOL's engines are pods that move**, on `part::VTOL_FRONT`/`VTOL_REAR`
  about pivots the model declares (`MeshBuilder::set_vtol`), turning on a
  trunnion you can see: stood up to hover, laid forward to go, tipped back to
  brake, a fan or turbine turning in the intake. Since the camera
  looks down on a hovering aircraft, a jet pointing at the ground is hidden by
  its own pod: the effect that reads is the bloom that reaches past the pod's
  rim, and the wash on the ground under it. A VTOL's jets burn a blue drive plume (the Kestrel's), its lift fans throw a blue field.

## The navy

- **Warships are bigger than land units, slower, tougher and longer-ranged.**
  A hull's origin is its waterline: `height` stands above the water and the
  keel is drawn below it. Tech 1 hulls are plain light-metal plating with dark
  frames, faceted (stealth-sloped) superstructures for the sci-fi tinge, a
  team stripe and hull number painted on (`HULL` pattern), nothing lit.
- **A gun has a firing arc, and the ship turns to bring it round.** The
  frigate's deck gun cannot bear straight aft (`arc: 270`); a target astern
  makes the hull turn. The AA mount turns on its own and out-ranges the deck
  gun, so an escort covers the ships around it. No helipads on a tech 1 hull.
- **Submarines dive by default, and a dived hull is another world.** Only
  sonar finds it (vision and radar do not), only torpedoes aim at it (a
  blast on the water over it still reaches it), and it passes under surface ships without touching them.
  V dives or surfaces the selection. Sonar is dark green on the rings;
  torpedo reach is green.
- **The Paladin walks the seabed.** It is amphibious and keeps to the bottom.
  Wading, it is seen and shot at like anything ashore and its projectors fire;
  once the sea closes over it, it is under water with the dived hulls: only
  sonar finds it and only torpedoes aim at it (a blast on the water over it
  still reaches it). A gun whose muzzle is under the
  water is silent; the tubes on its shins fire only from under it.
- **Sonar is a buoy**, not a hut: a float at the waterline, a mast, and the
  hydrophone array hanging under the water. Three tiers, each refitted onto
  the last, each changing the outline above the water, not only what hangs
  under it: tech 1 a bare float and pole mast; tech 2 three outrigger
  sponsons (a three-pointed plan) and a lattice tripod; tech 3 a big white
  radome and a lit array turning at the masthead (still when unpowered).
- **A torpedo with nothing to hit goes off where it is.** One fired at a
  point runs there and bursts.
- **The torpedo bomber drops low and lets its torpedoes fall in.** The Gannet
  (tech 2 air) hears dived hulls with its own sonar, glides down to the wave
  tops on its run in and only drops from low over the water; each torpedo
  falls, splashes in, then runs and homes like a submarine's, keeping its mark
  on its own seeker once the aircraft has flown on. Its outline from above is
  a long cross: a straight gull wing with fan pods at the knuckles and a
  torpedo under each inner wing reaching well ahead of the leading edge.
- **Ships go down slowly.** A small blast, then the hull lists and settles,
  sinks with its fires still burning above the water and bubbles below, and
  lies on the seabed as a wreck.
- **Anything that goes off on the water moves the water**: rings that run out
  and fade, foam, white water thrown up that falls back to the surface, and
  a flash that lights the water around it. A torpedo hit is a tall white
  column and an orange flash seen through the sea.
- **A wake is where the ship has been.** It grows out behind the stern as the
  ship gets going and stays where it was laid when the ship stops; it never
  pops in at full length.
- Water swallows the top end of a sound: a blast under the surface is nearly
  all bass (`torpedo_hit`), a shell into the sea is a slap and a thump and the
  spout falling back (`shell_in_water`). Sounds are in `data/sounds/naval.ron`.
- **A warship's guns turn on houses of their own.** Every mounted weapon
  (`mount: true`) is its own gun house (`MeshBuilder::with_house`), yawing on
  its pivot with its weapon; only what recoils inside it pitches. A house astern
  (`rear: true`) is authored facing forward and rests turned round. Fixed
  launchers (torpedo tubes, missile cells) are hull geometry with doors.
- **Tech 2 hulls earn a few emitters, tech 3 earns more.** Tech 2: lit sensor
  panels, blue glow on rail guns, orange seams on missile cells. Tech 3: flux
  conduits from the citadel to each barbette, charge rings at the muzzles, a
  charge glow that runs down the barrels before a salvo. Tech 1 stays unlit.
- **Submarines are not tubes.** Angular, chined pressure hulls, several tube
  doors and hatches, several engines, planes with pods on them; the Moray an
  arrowhead, the Kraken a flat diamond with a missile deck.
- **The battleship moves the sea.** A salvo stamps a pressure ring on the water
  under the guns, throws a spray sheet off the hull along the barrels, heels the
  ship away from the broadside, and its heavy shells fall as tall lit columns. A big hull throws a
  standing bow wave at speed.
- **Torpedo defence is a torpedo.** Interceptor tubes (`intercepts: true`) fire
  a short torpedo at one coming in; both burst under the water
  (`TorpedoIntercepted`). No decoys, no radius.
- **You can tell a torpedo by its line.** Every torpedo leaves a line on the
  water that reads from strategic height, and its look (`TorpedoLook`: the
  weapon's `torpedo_look`, else an interceptor's `Sprint`, else its side's
  faction's) sets its body and its line: ARC air-driven torpedoes a white seam
  of bubbles over a pale band; heavy ones a long fat body and twin screw seams
  that lie long; long-range pump-jets a dotted line of gulps; interceptors a
  short body with a hot pale motor and a thin fizzing line that snakes and is
  soon gone; the Regency's plasma drives a red glow under a glassy steam line.
- **Shores have surf.** Breakers roll in square to every shore, bunching up as
  the water shoals, break into a white lip, run in as white water and wash up
  the sand in ragged lobes, leaving it wet and dark behind them (shore.wgsl).
  They come in sets, and a crash is heard where and when one is seen to break
  (`mc_render::shore`, the ambience). A canyon lake has little surf.
- **The sea from under it.** The free camera may go under the water: the view
  is tinted and dimmed by the water, the surface overhead shows the sky through
  a round window and mirrors the water beyond it, and light shafts slant down.
  The play camera never goes under.
- **A dived launch is a boil.** A missile leaving a dived hull (`DivedLaunch`)
  bubbles up, breaches in a spray column, then climbs on a cold lob before the
  motor lights; the boat is on radar for the next eight seconds. A high-arc
  missile boosts with a bright column going up, goes dark over the top, and
  glows again coming down. A sea skimmer runs low with a spray line under it.
  The rules behind the roster are in `docs/NAVY.md`.

## Sound

- **No static.** Noise is only the first few milliseconds of a bang (a short,
  band-limited crack, nothing much above 2.5 kHz) and quiet, low tails. The
  body of every battle sound is tonal: falling sines from the low mids into
  the bass. Bright or long noise, and saturation over a noise tail, read as
  crackle and static.
- A gunshot is a crack and a mid-range bark over a thump, then the shell heard
  going away: a falling whine off to one side.
- A tank's main gun (`tank_gun`) is not that: one hard slam of crack and
  chest-deep boom together, driven hard, the report rolling off the ground
  and slapping back, the recoil and the breech clacking open behind it. No
  shell whine; the light cannon's bark and whine read as a toy on a tank.
- An impact is an explosion first: bass and low-mid boom. Never a ringing
  plate. Armour only adds a harder crack and a dull thud.
- Energy weapons hit like guns (snap, body, real bass) and are told apart by
  the bolt: a buzzing stack of harmonics falling fast. Never a thin chirp.
  Heavy ones (the Redoubt's battery, tech 2 and 3 guns) have their own deeper
  sound, not the light one pitched down.
- A death has its own sound, in stages: detonation, a second blast a beat
  later, ammunition cooking off, torn metal landing, fire coming up.
- **Sounds are a library, not a property of each weapon.** Battle sounds are
  named recipes in `data/sounds/*.ron` (general: guns, impacts, deaths, running
  gear) and `data/factions/<faction>/sounds.ron` (the faction's own: Aster's
  rail guns, the flak burst, its hover drive). Unit files name them in `sounds: (...)`
  blocks: `fire`, `charge` + `charge_time`, `impact`, `ground` on a weapon,
  `death` and `moving` on a unit. What a unit file leaves out comes from the
  library's `defaults`. Reach for an existing sound first; a new one has to
  earn its place. A bigger version of a sound is `like` it at a larger `size`,
  not a new recipe.
- Units answer being selected, and the answer is a gesture, never a beep: a
  held sine is the dullest sound there is. Pitch moves, the voice (`Fm`) starts
  bright and mellows like a struck or driven thing, and most answers happen in
  two stages. A walker's servos chirp twice, each running up to its note;
  armour blips its engine, a growl that revs up and settles over a hatch thud;
  a builder runs a tool up to speed and chirps ready; the commander is the only
  chord, three bell notes struck in rising order. Structures are plant, not
  vehicles: a factory clanks twice under a climbing hum, an extractor strokes
  twice, a generator's contactor sparks and the bus buzzes up, storage is a
  hollow knock that sinks, an emplacement traverses, clunks and locks twice, a
  radar chirps a sweep and hears its returns, a wall is a dead knock. The kind of unit is
  told by the gesture, not by level or pitch, and every tier of a unit answers
  alike. They stay short and quiet, on the interface's D and A, at interface
  volume. Recipes are in `data/sounds/responses.ron`, by `icon` kind under
  `defaults.select`; a unit file can name its own with `sounds: (select: ...)`.
- Bigger weapons are lower and longer, not just louder. The interface set stays
  tonal and light; the battle set is physical.
- Battle sounds sit in the world: loudest at the middle of the view, panned by
  where they are on screen, quieter from orbit. Only the loudest few of each
  kind play per tick, so a barrage stays a barrage.
- `--dump-sounds DIR` writes the whole bank as WAV files for listening outside
  the game.

## Replicators (Survival)

- **Not Aster: Precursor work.** The Replication Engine and its nodes follow the
  Precursor concept sheet (Forerunner-like): pale alloy plates that read as
  dressed stone (`PRECURSOR`), deep dark recesses and joints between them
  (`PRECURSOR_DARK`), pieces that float apart with clean gaps, every edge
  chamfered, nothing riveted, no pipes. The plate's texture is
  `pattern::PRECURSOR`: a few big panels per face cut by deep grooves that are
  raked or pointed (never a square grid), a frame line inset along the long
  edges, rails that break off at the cuts, the odd elongated-hexagon inlay, a
  faint stone mottle, and on some panels a slot of light with pointed ends.
- **One colour, the cold replication light** (`GLOW_PRECURSOR`, `SURF_PRECURSOR`):
  a blue colder and whiter than Aster's emitters, with less green in it so it
  never reads as their cyan. It is where matter is made or carried: the core's
  faces, the arms' feeds, the lenses, the crystals, the arc plates' edges. It is
  alive: one slow breath across the machine and bands of light rising up it
  (`precursor_pulse`, shared by the glow material and the plate's slots), and
  it gutters on a badly hurt machine.
- **The engine is a monument, grounded**: a platform of sector slabs (a print bed
  under each bay, a step over each gap), a dark core column with light up every
  face, two rings of floating C-shaped arc plates, eight jointed arms (shoulder,
  guarded upper arm, elbow, forearm, a projector head with its lens between two
  claws), a floating crown of blades round the ray's crystal with a halo turning
  over it and a spire above, the Lance on a collar of blocks under the crown.
- **The node is the engine's engineer** (the concept's Engineer, Tech III): a
  spine with its light, a crown of C-shaped arcs, four claw arms hanging, a
  turning hologram disc and a pointed base, all hovering clear of the ground.
- **The veil says "you cannot break this".** It shares nothing with the cyan
  honeycomb: a dark, heavy membrane that dims what is inside, a geodesic
  lattice of ice-white struts in latitude bands that turn slowly against
  each other, bright seams where the bands meet, a hard rim. A hit is a white
  caustic flare that is shed sideways and slides off round the dome, lighting
  the struts it passes. No ripple, no peel, no break, and it fuses with nothing.
- **The ray is the biggest light on the map**: a blinding white core in a
  cold blue sheath, filaments crackling round it, pulses running out from the
  engine, a star flare at either end, light splashed over the ground at the
  site and motes of matter drawn up into it. It stays a few pixels wide from
  any distance.
- **Printing is the replication blue, not amber.** A print beam is two fans sweeping the
  unit's volume with packets of matter landing all over it, and the unit fills
  in with the construction look recoloured to the replication blue.

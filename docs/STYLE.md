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
- **Dirt follows the tier.** Tech 1 is the dirtiest (dust over the running
  gear and lower hull, grime in the wear map); higher tiers stay closer to
  parade white. The unit shader scales this by tech level.
- A unit's role line says what it is in plain words: "Light Tank", not a
  class name that needs a manual.

## Weapons

- **Conventional weapons are fire-coloured.** A gun's shell is white-hot with
  an orange trace, its muzzle flash and impact are orange, it throws smoke and
  sparks. `color: Orange` in the weapon data.
- **Blue is for energy weapons**, and those are not tech 1.
- **Every shot is seen from muzzle to target.** A shell is drawn over its whole
  flight, its last stretch included, and its impact waits for it to arrive. At
  point-blank range the shot is a short streak, never nothing. A shell's trace
  is short (a fifth of a tick's travel): a shell, not a beam.
- An impact shows on the skin of what it hits, not inside it: sparks off
  armour, a burst of earth off the ground, smoke after either.

## Ground contact

Tracked vehicles mark the ground and raise dust while they move. Marks fade
over about a minute. Both are drawn only near the camera.

## Foundations

- **A poured lot, not a tile.** The slab sits in the build-grid cell but is not
  a square: a rectangle biased toward the face, uneven chamfers, a bay or a
  notch. Neighbouring lots do not meet as a grid.

## Mass deposits

- **Cracks, not a glow.** A deposit is stone split by the ore: dark in the
  cut, a dull mineral seam along the lips, a collapsed pit at the centre.
  Never a neon amber splat.
- **The extractor mines them.** The wellhead is a ring around an open bore;
  intake bells sweep the fissures just above the ground. The pad does not
  cover the cracks, and nothing on it is a drill.
- **They stay readable.** From orbit a mass-green ring and cross sit over
  every site (the same mark as the extractor icon); close in the cracks do
  the talking. The minimap carries the same green cross.

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

- **Not a white statue.** A dark graphite frame carrying white armour: the
  plating colour is for what is armour (breastplates, greaves, cuisses, a guard
  along the pauldrons), everything under it is `PLATING_DARK` and frame. From
  the RTS camera the top surfaces decide the read, so the head and pauldrons
  are dark and carry the team colour.
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

## Construction

- **Construction is amber.** Yellow-orange (`GLOW_AMBER`, the `AMBER` of the
  shaders) marks everything that builds: the lens and collar of a build arm,
  feed tanks, conduits, and the build beam itself. Blue stays energy weapons,
  orange stays guns. Build emitters run hotter while the unit is building.
- A build beam is a steady hot thread from the emitter to where it meets the
  work — the near side, the point the builder is printing from — with pulses
  running down it and sparks where it lands. It does not wander: it is a tool
  held on its job.
- What is being built expands from that weld: a sphere of the finished mesh
  growing out through the volume, a scanning amber hologram of what is still
  coming. The print is white-hot at the front and warm behind it. The last
  stretch is the whole thing cooling off into the finished material.
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
  Other structures (factory and the rest) rebuild from the weld like any other
  site, and an engineer helping them prints with a build beam.
- Higher engineering tiers read as more build kit, not as decoration: more
  nozzles on the arm, more tankage on the pack, masts, conduits.

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
- **A commander's death is the biggest thing in the match.** A flash that whites
  the whole view out and only slowly lets it back (no edge to it: what is seen
  is where it stops saturating), a shock front and driven dust out to the edge
  of the blast, a fireball rolling up into a cloud on a stalk, burning
  fragments thrown far, fire in the crater for a long while. Fire on that scale
  is dimmer and more ragged (`PUFF_FIREBALL`), or it becomes white discs. Its
  sound (`commander_death`) is nearly all bass, ten seconds long, and heard
  wherever the camera is: containment cracking, a breath, the detonation, two
  more heaves of the ground, thunder rolling away.

## Sound

- **No static.** Noise is only the first few milliseconds of a bang (a short,
  band-limited crack, nothing much above 2.5 kHz) and quiet, low tails. The
  body of every battle sound is tonal: falling sines from the low mids into
  the bass. Bright or long noise, and saturation over a noise tail, read as
  crackle and static.
- A gunshot is a crack and a mid-range bark over a thump, then the shell heard
  going away: a falling whine off to one side.
- An impact is an explosion first: bass and low-mid boom. Never a ringing
  plate. Armour only adds a harder crack and a dull thud.
- Energy weapons hit like guns (snap, body, real bass) and are told apart by
  the bolt: a buzzing stack of harmonics falling fast. Never a thin chirp.
  Heavy ones (the Bastion's battery, tech 2 and 3 guns) have their own deeper
  sound, not the light one pitched down.
- A death has its own sound, in stages: detonation, a second blast a beat
  later, ammunition cooking off, torn metal landing, fire coming up.
- **Sounds are a library, not a property of each weapon.** Battle sounds are
  named recipes in `data/sounds/*.ron` (general: guns, impacts, deaths, running
  gear) and `data/factions/<faction>/sounds.ron` (the faction's own: Aster's
  arc weaponry, its hover drive). Unit files name them in `sounds: (...)`
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
  radar pings and hears its return, a wall is a dead knock. The kind of unit is
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

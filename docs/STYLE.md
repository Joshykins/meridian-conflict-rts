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
  parade white. The unit shader scales this by tech level. The dust stops at
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
  width (`surf_crack`), sized like the plating, never blobs or contour rings.
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
- **Blue is for energy weapons**, and those are not tech 1.
- **Weapon look is data.** Muzzle flash (`flash`), impact flash (`impact`, or `flash` if left out), shockwave,
  tracer size, trail, how long the wake hangs (`wake`), plasma around a traveling slug (`plasma`), and energy bolts (`bolts`) are set on the weapon. Shockwaves
  take the weapon's colour: blue for energy, dust for guns. The renderer scales a recipe from damage and colour;
  it does not special-case a unit.
- **Every shot is seen from muzzle to target.** A shell is drawn over its whole
  flight, its last stretch included, and its impact waits for it to arrive. At
  point-blank range the shot is a short streak, never nothing. A shell's trace
  is short (a fifth of a tick's travel): a shell, not a beam.
- An impact shows on the skin of what it hits, not inside it: sparks off
  armour, a burst of earth off the ground, smoke after either.

## The electric bore

The Argon Electric Bore (AEB) is Aster's lightning gun: the Arbalest (tech 3
sniper) carries one, the Fulgur (tech 4) an AEB-2.

- **A shot is two things.** First an argon tracer round, an ordinary blue slug
  seen from muzzle to target. When it lands, the charge is dumped down the
  ionised channel it left: a straight, sustained white-cyan plasma column from
  muzzle to strike, surrounded by five branching lightning return strokes over
  1.28 seconds. The column has a blue sheath and travelling density ripples;
  the arcs wander around it and light the ground with each pulse. The small launch flash and
  thin tracer only establish the channel; the discharge is the main event. The
  blast at the end is the weapon's own splash. Both launch and impact send out
  blue pressure fronts; the Arbalest's impact wave is especially pronounced for
  its size, while the launch wave stays separate from the small tracer flash.
- **The AEB-2 does not aim well** (`spread`), and its charge sears everything
  within `bore.width` of the channel on the way (`bore.damage` each), scorches
  the ground under it. Low channels melt the ground; a shot high over a valley
  leaves the floor alone. Every bore, including the Arbalest and compact AEBs,
  burns trees along the entire projected channel (4 m half-width minimum).
- **The ground it runs over goes molten, then cools** (`bore.cool` seconds): a
  pool that glows white-yellow when fresh, orange, then dull red, while a dark
  glassy crust closes over it from the rim in, last glowing only in its
  cracks. The charcoal scorch under it stays. The Arbalest leaves only a small
  pool where it strikes.
- Sound (`data/sounds/bore.ron`): capacitors filling (two contactors, a climbing
  stack), a light crack for the tracer, then the strike: a snap, a buzzing stack
  falling fast, a deep thump and thunder rolling back twice. The AEB-2's are
  the same sounds larger. The Fulgur's compact mounts use quieter, shorter
  variants so the main discharge dominates the pair.

## Experimentals

Tech 4 machines are too big for any factory: Mason IIIs raise them on a lot of
their own, like a structure (`footprint` on a mobile unit), and the finished
machine drives off it. The Fulgur is the first: a super-heavy assault tank,
four tracks, a hull field, the AEB-2 on the main turret and gun houses of its
own on the hull (`hull_mounts`: two compact AEB turrets and a rear shatter AA
projector). The long chamfered hull carries a centered turret with the main
barrel set into a narrow upper-left breech fairing. The broad turret roof stays
low, with only the sloped fairing rising around the breech; the barrel elevates
about its trunnion to track terrain. Open induction collars and tapered ceramic
blades replace the boxy barrel jacket. The main pressure-wave radius is 3.33
times its former size. The Arbalest shares this emitter family, with its gun
and breech centered on the turret, and reaches 520 m. Both use a pitching
trunnion drum and receiver sleeve that overlaps the barrel throughout recoil.

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
  `(1.0, 0.147, 0.047)` in ground.wgsl): numbers, fields, veins, territories,
  shafts, the minimap. The sim still calls it `mass`; the old mint green is
  now `hud::HEALTHY`, for unit health only.
- **In play an ore field is a faint outline**, nothing else. A field a mine
  the viewer has seen is working gets a brighter rim and a light fill, the
  same hue, so worked ground reads on the strategic view and the minimap.
- **The survey** (placing or selecting a core mine, or holding Ctrl): the
  fields light up with their veins, branching orange lines at depth under
  each field; every mine in sight shows its territory, its circle cut
  straight where it meets a neighbour, and a shaft sunk from the mine to each
  field that falls to it. Placing shows what the mine would make there, its
  efficiency and its payback.
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
  the rail cannon. The right shoulder takes a shatter cannon or a howitzer, the
  left a second projector that folds out over the shoulder to build and hangs
  behind the upper arm when it is not. Two alternatives in one slot must differ
  in silhouette, not only in colour.

## Construction

- **Construction is amber.** Yellow-orange (`GLOW_AMBER`, the `AMBER` of the
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
- **A commander's death is the biggest thing in the match.** A flash that whites
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
  wing; Raptor (tech 3) forward-swept wings, big canards, and its two Plasma
  Lances out ahead of the nose like mandibles.
- **White armour over a dark frame**, as on ground units: mostly-white aircraft
  look flat. Hard chines and flat faces (hull stations lofted with
  `air::band`), not round tubes.
- The weapon is visible and sits at its muzzle: a missile on the rail it
  launches from, a lance whose tip is the muzzle in data.
- **A VTOL's engines are pods that move**, on `part::VTOL_FRONT`/`VTOL_REAR`
  about pivots the shader knows (`models::vtol_nacelles`): stood up to hover,
  laid down to cruise, a fan or turbine turning in the intake. Since the camera
  looks down on a hovering aircraft, a jet pointing at the ground is hidden by
  its own pod: the effect that reads is the bloom that reaches past the pod's
  rim, and the wash on the ground under it. Jets are orange, lift fields blue.

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
  sonar finds it (vision and radar do not), only torpedoes reach it (no
  splash either), and it passes under surface ships without touching them.
  V dives or surfaces the selection. Sonar is dark green on the rings;
  torpedo reach is green.
- **The Paladin walks the seabed.** It is amphibious and keeps to the bottom.
  Wading, it is seen and shot at like anything ashore and its projectors fire;
  once the sea closes over it, it is under water with the dived hulls: only
  sonar finds it and only torpedoes reach it. A gun whose muzzle is under the
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
- **Tech 2 hulls earn a few emitters, tech 3 earns plasma.** Tech 2: lit sensor
  panels, blue glow on rail guns, orange seams on missile cells. Tech 3: flux
  conduits from the citadel to each barbette, plasma rings at the muzzles, a
  charge glow that runs down the barrels before a salvo. Tech 1 stays unlit.
- **Submarines are not tubes.** Angular, chined pressure hulls, several tube
  doors and hatches, several engines, planes with pods on them; the Moray an
  arrowhead, the Kraken a flat diamond with a missile deck.
- **The battleship moves the sea.** A salvo stamps a pressure ring on the water
  under the guns, throws a spray sheet off the hull along the barrels, heels the
  ship away from the broadside, and its shells are plasma streaks that fall as
  tall lit columns. A big hull throws a standing bow wave at speed.
- **Torpedo defence is a torpedo.** Interceptor tubes (`intercepts: true`) fire
  a short torpedo at one coming in; both burst under the water
  (`TorpedoIntercepted`). No decoys, no radius.
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

- **Not Aster.** The Replication Engine and its nodes are black obsidian plate
  (`ACCENT` with `pattern::VEINED`: Aster's dark plating, its little lights
  violet instead of orange) over gunmetal frames, on a graphite plinth. Their
  one colour is the white-hot violet of replication (`GLOW_VIOLET`), and it is
  only where matter is made or carried: the furnace mouths, the feeds down the
  gantries, the projector lenses, the fin edges running up to the crown, the
  ray's crystal. Every piece does a job: hull furnace, eight print bays, fins,
  crown emitter, the Lance on its collar.
- **The veil says "you cannot break this".** It shares nothing with the cyan
  honeycomb: a dark, heavy membrane that dims what is inside, a geodesic
  lattice of violet-white struts in latitude bands that turn slowly against
  each other, bright seams where the bands meet, a hard rim. A hit is a white
  caustic flare that is shed sideways and slides off round the dome, lighting
  the struts it passes. No ripple, no peel, no break, and it fuses with nothing.
- **The ray is the biggest light on the map**: a blinding white core in a
  violet sheath, filaments crackling round it, pulses running out from the
  engine, a star flare at either end, light splashed over the ground at the
  site and motes of matter drawn up into it. It stays a few pixels wide from
  any distance.
- **Printing is violet, not amber.** A print beam is two fans sweeping the
  unit's volume with packets of matter landing all over it, and the unit fills
  in with the construction look recoloured to replication violet.

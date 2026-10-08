# The standard

How every unit, structure and effect in the game is built, so the factions each
speak one design language and any of them can be reskinned by data. The
**reference set** shows this standard in practice: a few examples finished to
the final bar, viewable in game. ARC and the Regency are measured against it
and brought up to it.

[STYLE.md](STYLE.md) says what each faction looks and sounds like. This file
says how any faction's look is put together. Where the two disagree, STYLE.md
wins on taste and this file wins on structure.

Status (2026-10-08):
- Built: skins carry every model's surface slots, its finish (texture detail,
  seams, wear, grime) and the Regency film (`data/skins`, below).
- Built: models carry authored surface marks: zones, fittings and painted marks
  (below). The ARC Warden (`tank_light`) is the first unit marked.
- Everything marked *planned* does not exist in the tree yet.

## Why: the 2026-10-08 audit

Both factions share one layer of a language and are missing the others.

- **ARC shares finished parts but not finish.**
  - What holds: grey plate, blue panels and the team panel. `parts::team_panel`
    has 191 calls in about 35 files.
  - Under that, 39 of 86 model files use no shared part besides `team_panel`
    and build from raw boxes.
  - There are no faction-wide shape constants.
  - Two tracked-hull systems exist (`tracked_chassis`, and `track` +
    `hull_plan`).
  - Rail guns are built five ways.
  - Older files (`vehicles.rs`, `structures.rs`, `mechs.rs`) model glow and
    vents as mesh; newer ones (titan, resolute, citadel) draw them as surface
    patterns.
  - The T1 bot and T1 artillery are lit, against the tier rule.
- **The Regency shares finish but not parts.**
  - What holds: the `kit.rs` paint helpers and the `machine.rs` courses (e.g.
    `dark_plate`: 779 calls in 108 files).
  - Above that layer, the Plasmeric Repeater exists five times, `CHAMFERED` is
    pasted into five files, a local `THICK` is defined 15 times, and about 20
    local gun and turret functions exist.
  - Nanite light is drawn two ways (`NANITE_SLOT` vs `GLOW_VIOLET` mesh).
  - Some helpers go against STYLE.md (`kit::blade`, `kit::ram`).
  - The T4 Carillon has less detail than the T1 extractor.
- **Effects** are mostly chosen by data, but each look enum
  (`Construction`, `ShieldLook`, `TorpedoLook`, `AntiMissileLook`, `WarpStyle`,
  …) is a hardcoded branch.
  - Colours are shader literals (the nanite violet is written four times, and
    the nanite red already disagrees between `beams.wgsl` and `entity.wgsl`).
  - A few effects are still picked by mesh or unit name (`renderer.rs`
    assault_air/rotor_gunship, `print_heads.rs` regency_brood).

The fix is not "more detail" on its own. It is a shared grammar between
"paint" and "whole model", in three layers.

## The three layers

Every model is built from these, in this order. A layer never reaches past the
one under it: a model never sets a colour, a part never hard-codes a size the
proportions own.

### 1. Proportions (per faction, *planned*)

One module per faction (`aster/proportions.rs`, `regency/proportions.rs`) holds
the numbers that make a faction's shapes read as one family:

- bevel and chamfer steps (a short ladder, e.g. small / medium / large, scaled by
  unit size)
- plate thickness and course depth
- the barrel sizes: bore, jacket and muzzle steps per weapon class
- track, wheel, deck and skirt sizes per chassis class
- panel insets, seam widths, how much detail is allowed per tier

A model takes its sizes from these, or from a part that does. A literal size in
a model file is for that model's own primary forms only.

### 2. The parts kit (per faction)

Each family of part is built **once**, by one function, and every model uses
it:

- **Bodies:** tracked chassis, hover skirt, walker leg, hull (land, naval, air),
  turret ring and shell.
- **Weapons:** one per weapon family (cannon, rail, repeater, missile cell,
  howitzer, beam). Size and variant are parameters; there is no copy.
- **Mid-scale detail:** pistons, hatches, cable runs, bolted boxes, sensor
  clusters, stowage. They go where forms meet (joints, mounts, panel breaks,
  under overhangs), never scattered over big flat faces.

ARC's `parts.rs` and the Regency's `kit.rs`/`machine.rs`/`chassis.rs` are where
these live today. The work is to fill the gaps and remove the copies, not to
start over.

### 3. Finish (skins and marks)

All fine detail, colour and light are drawn by the surface: rivets, seams,
louvres, grilles, scratches, wear, dirt, lit slots. The mesh never carries a lit
piece or a small fitting (CLAUDE.md §9), and no Rust or WGSL code holds a colour.

Two things feed the surface:
- **The skin** (below): what each slot looks like and how plates are finished.
- **The model's marks** ("Texture detail and marks", below): what the surface
  carries where, placed by the model's author.

A model paints **slots** (`mc_models::material`, `gpu_consts::skin`), which say
what a surface is, never what colour it is:

| Slot | Material | Meaning |
|---|---|---|
| `plate` | `PLATING` | main armour |
| `plate_dark` | `PLATING_DARK` | armour painted dark |
| `trim` | `ACCENT` | frames, joints, recesses |
| `metal` | `METAL` | working metal: barrels, pistons |
| `rubber` | `TREAD` | tracks, tyres, feet |
| `glass` | `GLASS` | sensors, canopies |
| `emitter` | `GLOW` | the faction's own lights |
| `gun_glow` | `GLOW_ORANGE` | conventional weapons' light |
| `build_glow` | `GLOW_AMBER` | construction's light |
| `beacon` | `GLOW_RED` | obstruction lamps |
| `nanite` | `GLOW_VIOLET` | nanite light |
| `laser` | `GLOW_LASER` | missile defence, Regency lamps |
| `visor` | `VISOR` | helmet visor glass |

Team colour (`TEAM`) and shield colour are not skin slots: a skin may not change
whose a unit is.

## Skins

A skin is one data file, `data/skins/<key>.ron` (`mc_data::skins`), that says
what every slot a model paints looks like: its colour, metal, roughness and
light. A colour is given outright (`Rgb`), as a light (`Light(hue, k)`), from the
skin's own `highlight` (`Highlight(k)`) or as the plate tinted (`Plate(tint)`), so
a slot that follows another follows it when a skin changes it. The Regency's skin
also carries its `film`, the colours regency.wgsl finishes Regency plate in.

- A faction names its skin in `faction.ron` (`skin: "arc"`). A variant (desert
  ARC, a Regency house colour, a reward) is another skin file: `arc_verdant` is
  ARC in white and green.
- On the GPU, skins are rows in one buffer (scene set 34, renderer/skins.rs), and
  each `ModelInfo` carries a skin index. `scripts/shot.sh unit KEY --skin KEY`
  dresses every model in another skin with no rebuild.
- Skins are presentation only and never reach `State` (determinism unaffected).
- A skin's `finish` says how plated faces are finished:
  - `kind`: `Arc` (paint over scanned steel, scratched at the edges) or
    `Regency` (steel under the film)
  - `plate`: plate size, times the standard
  - `wear`: paint lost at the edges, times the standard
  - `lights`: the lights let into dark plating, `Off`, `Dashes(colour)` or
    `Dots(colour)`
  - `detail`: texture detail, 0 (none) to 1 (dense panelling and zones)
  - `seams`: how deep and dark seams and fittings are cut, times the standard
  - `grime`: field dirt, times its tier's
- A plain plated face takes its skin's finish whatever model it is on, so an ARC
  tank in the Regency skin wears the film. A face with a specific pattern (an
  airframe, a warship hull, a deck) keeps it.
- ARC's finish (`arc.ron`): detail 1, seams 2.8, plate 0.65, wear 2, grime 1.4.
  Grime above about 2 reads as camouflage blotches (STYLE.md).
- Not yet in a skin (*planned*): the lights drawn by specific patterns (windows,
  strips, decks, lamps), the foundations' graphite, and per-unit slot overrides.

**Distribution.** Skins may be a paid extra, a developer or tournament reward,
or made by modders. So from the start:

- Skins have stable ids, and a match, replay or save records the id of each
  player's skin.
- A missing or unowned skin falls back to the faction default, never an error.
- A skin from outside the build is untrusted input (CLAUDE.md §7): read with a
  size limit, validated, and its textures limited in size and format. It is
  data and pictures only, never code or shaders.
- Ownership (who may use which skin) is a separate system and is not built
  until it is needed.

**What a skin cannot change.** A skin must not hide gameplay:

- the shape (no mesh replacement)
- team colour
- tier cues
- the size of a shot or blast
- a minimum brightness and opacity for shots, beams and warnings

The loader clamps these values. A skin that would make a weapon invisible is
refused.

## Texture detail and marks

A real machine's surface is not random, and neither is ours. Detail comes in two
layers, both laid out on purpose; only weathering (wear, scratches, dirt) varies
by chance.

**Panelling, by zone.** Plates are cut into a regular grid of panels, as dense as
the skin's `detail`. The zone a point lies in sets the grid
(greebles.wgsl `surf_greeble`):

| Zone (`surface_mark`) | Panelling | Where it goes |
|---|---|---|
| none | plain panels, a medium grid | anywhere not marked |
| `ARMOUR` | few, large panels; quiet | glacis, turret front, anything that takes hits |
| `MACHINERY` | small panels, a bolt in each corner | engine bays, bustles, hull tails, mechanisms |
| `ENGINE` | grating over every panel | engine decks, intakes |
| `WALKWAY` | tread plate | fender tops, decks crews walk on |

**Fittings and paint, placed.** A model's author lays marks with
`MeshBuilder::mark(min, max, kind, arg)`: a box in the model's own space, drawn
on whichever face it crosses and filling the box (greebles.wgsl `fitting_h`,
`mark_paint`):

| Kind | What | `arg` |
|---|---|---|
| `GRILLE` | square holes in a frame | hole pitch, 0 standard |
| `LOUVRES` | slats raked down | slat pitch |
| `VENT` | round holes, staggered | hole pitch |
| `RIBS` | raised stiffeners along the long side | rib pitch |
| `ACCESS` | a bolted cover with a handle | |
| `HATCH` | a round lid, its hinge | |
| `BOLTS` | a row of bolt heads along the long side | bolt pitch |
| `PANEL` | bolted applique plate | |
| `SEAM` | a weld bead along the long side | |
| `HAZARD` | yellow and black stripes (paint) | |
| `STENCIL` | a number (paint) | the number |
| `WARNING` | a warning triangle (paint) | |
| `LABEL` | lines of stencilled lettering (paint) | which words |
| `TEAM` | the owner's colour, edged by a pin line (paint, under dirt) | |
| `LAMP` | a slot lit in the skin's lamp colour (Regency) | |
| `LAP` | a plate lapped over the next: shadow under, bright edge (Regency) | |
| `GROOVE` | a machined channel with polished lips (Regency) | channel width |
| `OUTLINE` | an inset panel line, no bolts (Regency) | |

Fittings carry relief and darken their hollows; paint goes on after dirt, as
fresh paint. Later marks lie over earlier ones. A model carries at most 48
(`surface_mark::MAX`, a test holds every model to it).

**Authoring rules.** The Warden (`vehicles.rs` `warden_marks`) is the worked
example.
- Mark the zones first: armour where it would take hits, machinery and engine
  where things work, walkways where crews would step. Zones cover the whole
  model in a few boxes.
- Then the fittings, where a real machine would have them: a hatch where a crew
  gets in, louvres and grilles over engines, access covers on the sides of
  engine bays, bolt rows along deck edges, applique on the faces that take hits,
  weld seams down the middle of big plates.
- Team colour is a `TEAM` mark on a level face that reads from above (a roof, a
  deck, a wing), never a plate of mesh. ARC's `parts::team_panel` lays one.
- Paint marks belong where they mean something: hazard stripes on tow points,
  hinges and moving parts; a warning sign by an exhaust, a hot or a dangerous
  part; the hull number on a flank both sides can read; lettering under it or
  on access covers. Never as decoration on a plain face.
- Keep the big faces calm. A tank-sized unit carries about 15–25 fittings and a
  few paint marks; a structure more; a scout fewer.
- A mark's box must cross the face it is meant for. A box that only meets a
  track or a barrel (rubber, metal) draws nothing. Look at each mark in a close
  shot (`scripts/shot.sh unit KEY --look X,Y,Z --zoom 5`).
- Mirror what is mirrored (`b.mirror_y`): a mark recorded inside it lands on both
  sides.

## Effect palettes (*planned*)

Every effect is a **template with parameters**, not an enum arm with colours
inside it:

- muzzle flash, shot and trail
- impact and crater scorch
- hit sparks and damage smoke
- death, wreck and wreck glow
- build, reclaim and warp-in
- shield, torpedo wake and anti-missile
- nuke

An effect palette (`data/effects/<id>.ron`) picks a template for each and gives
its colour ramps, intensity, smoke tint, particle counts and timing. For
example, "nanite build" becomes the swarm-build template with the Regency's
colours, so a recoloured Regency, or a faction that builds the same way, needs
only data.

The per-unit `effects` block ([EFFECTS.md](EFFECTS.md): dust and shockwave
colour) is the seed of this. The look enums in `mc-data` (`Construction`,
`ShieldLook`, `TorpedoLook`, `AntiMissileLook`, `LineLook`, `WarpStyle`,
`StrategicLook`) become template choices, and their colours move out of the
shaders into palettes.

Sounds already work this way (`sounds.ron`). A sound set covers every role:
each weapon family, impact, death by size, build, reclaim, warp, select and
order.

## What a finished unit has

The checklist every unit meets before it counts as done, and what the reference
set shows for each.

**Form**
- [ ] Silhouette reads at strategic zoom: about three primary masses, its role
      clear from the shape.
- [ ] Sizes come from the faction's proportions; parts come from the kit.
- [ ] Mid-scale detail sits where forms meet, and flat faces stay quiet.
- [ ] The tier reads at a glance (STYLE.md: T1 plain and unlit, emitters earned).
- [ ] Detail grows with tier: a higher tier is never barer than a lower one.

**Finish**
- [ ] Painted only through slots; no `GLOW_*` mesh pieces, no colour in code.
- [ ] Its zones marked, and its fittings and paint marks placed where a real
      machine would carry them ("Texture detail and marks").
- [ ] No fitting modelled that a mark can draw (bolts, vents, grilles,
      hatch outlines), no lit piece of mesh, and team colour painted (`TEAM`).
- [ ] Looks right in its own skin and in at least one other.

**Motion**
- [ ] Idle, move and turn; turret tracking; recoil or a fire cycle.

**Weapon**
- [ ] Muzzle, shot, trail, impact, crater: all from the effect palette.
- [ ] Sound: attack, body, tail, and a distant version.

**Life**
- [ ] Hit reactions and damage smoke by health.
- [ ] Death sized to the unit (small, medium, large, air crash, ship sinking)
      and a wreck.
- [ ] Being built (the faction's build effect), reclaim, warp-in.

## The reference set (*planned*)

A small set of examples, each finished to the whole checklist. It is the
standard made visible, and the first thing built on the skin layer.

| Example | What it shows |
|---|---|
| T1 line tank | silhouette, T1 clutter, dirt, unlit plain finish |
| T3 hero walker | top-tier detail density, earned glow, animation |
| Aircraft | small-scale readability, trails, crash death |
| Ship | waterline, wake, sinking |
| Factory | build effect, lights that follow its state |
| Mine or generator | economy readability |
| Defence turret | weapon mount, tracking |
| Engineer | build beam, reclaim, assist |

The weapons across the set cover each family once: cannon, beam, missile,
artillery, AA.

The set lives as real data (loaded, built and tested, so it cannot go stale)
and can be spawned only in the range, never in a match.

### Viewing it (*planned*)

- **Gallery map** on range mode: one station per example, each looping its
  show (fire at a target, take hits, die, get reclaimed, rebuild, warp in).
  Includes a skin picker, and skin files reload on save.
- **Layer toggles:** bare forms → + parts → + finish → + dirt and wear →
  + glow, with an optional wireframe and triangle count.
- **Comparison sheets:** `scripts/shot.sh` renders an example next to the ARC
  and Regency units of the same role, and one unit across a row of skins.

## Checks (*planned*)

Each rule here that can be measured becomes a gate, so the language cannot
drift again:

- No colour literals in effect, surface or FX code (WGSL and Rust), only reads
  from skins and palettes. The same idea as `gpu_consts.rs` for ids.
- No `GLOW_*` paint in model code once a faction's glow has moved to surface
  patterns.
- No branching on a unit, mesh or sound name outside `data/`, tests and debug
  scenes (CLAUDE.md §5; already a rule, not yet checked).
- Each weapon family is built by one kit function (a test lists the families
  and their builders).
- A skin swap test: every model renders with every installed skin without a
  missing slot.

## Order of work

1. This spec. (Done.)
2. The skin layer and the first example, the T1 tank, together. (Done for ARC:
   skins, finish, marks, the Warden marked.) Next its Regency counterpart.
3. The gallery map, layer toggles and comparison sheets.
4. The rest of the reference set, each with its effects as templates.
5. Moving ARC and the Regency over, one family at a time. Colours go into skin
   files, each enum look becomes a template, and each copy goes into the kit.
   Shots before and after must match.

## Fix list from the audit

Starting points for step 5. File references are from 2026-10-08.

**ARC**
- One `rail_gun` for citadel/mod.rs:797, zenith.rs:765,
  air/corvette.rs:494, titan.rs:1886 and naval/trimaran.rs:445.
- One tracked-hull system (`tracked_chassis` vs `track` + `hull_plan` in
  assault_tank, bore_tank, trebuchet).
- Remove the lit parts from the T1 bot (mechs.rs:1804-1859) and the T1
  artillery (vehicles.rs:732, 786).
- Gate the lit antenna tips on T1 factories by tier (factories.rs:750, 947,
  1424).
- Move mesh vents and glow strips in vehicles.rs, structures.rs and mechs.rs
  to surface patterns.
- Raise the bare same-tier outliers: the Swift scout (air.rs:239) against the
  Shrike.
- Pick effects by data instead of mesh name (renderer.rs:1280-1291,
  aircraft_trails.rs:47, heavy_rail_fx.rs:361, 643).
- Make rails match STYLE.md: 13 weapons use `rail: true` with a projectile
  speed, while only 2 set `hitscan`.

**Regency**
- One Plasmeric Repeater (guns.rs:22, with local copies in
  commander/arms.rs:94, naval/frigate.rs:308, naval/cruiser.rs:392,
  naval/assault_submarine.rs:343).
- Move `CHAMFERED` and the 15 local `THICK`s into proportions.
- One prism charge brush (turrets/sunspear.rs:110, strider/cannon.rs:25,
  fusion_guns/howitzer.rs:98).
- Nanite light only via `NANITE_SLOT`; remove the `GLOW_VIOLET` mesh pieces
  (commander arms and shoulder, brood, tidebrood, taproot, space hulls).
- Settle `kit::blade` and `kit::ram` against STYLE.md (no thorns, no rams).
  Remove the bronze wording (89 files) and the retired `pattern::MOLTEN` use
  (taproot.rs:236).
- Bring colossus/detail.rs's private `armor()` material into the kit.
- Raise the T4 Carillon's detail above the T1 extractor's.
- Move the nanite violet and red out of beams.wgsl, entity.wgsl, ground.wgsl
  and regency.wgsl into the palette (they already disagree).
- Pick by data instead of name in print_heads.rs:154.

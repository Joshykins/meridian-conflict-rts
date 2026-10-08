# The standard

How every unit, structure and effect in the game is built, so the factions each
speak one design language and any of them can be reskinned by data. The
**reference set** shows this standard in practice: a few examples finished to
the final bar, viewable in game. ARC and the Regency are measured against it
and brought up to it.

[STYLE.md](STYLE.md) says what each faction looks and sounds like. This file
says how any faction's look is put together. Where the two disagree, STYLE.md
wins on taste and this file wins on structure.

Status (2026-10-08): skins carry every model's surface slots and the Regency
film (`data/skins`, below). The rest is a plan: nothing marked *planned* exists in
the tree yet.

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

### 3. Finish (skins, *planned*)

All fine detail, colour and light are drawn by the surface from a **skin**:
rivets, seams, louvres, scratches, wear, dirt, lit slots. The mesh never
carries a lit piece (CLAUDE.md §9), and no Rust or WGSL code holds a colour.

A model paints **slots**, which say what a surface is, never what colour it is:

| Slot | Meaning |
|---|---|
| `plate` | main armour |
| `plate_dark` | secondary armour, recesses |
| `trim` | edges, frames, rails |
| `metal` | bare working metal: barrels, pistons, joints |
| `rubber` | tracks, tyres, seals |
| `bore` | barrel mouths, intakes, holes |
| `emitter` | a lit slot or line (its pattern decides the shape) |
| `team` | the owner's colour |
| `glass` | canopies, lenses, windows |

(The final list comes from the materials in `mc-models/src/lib.rs` and
`gpu_consts.rs`. A slot replaces a material id, not the other way round.)

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
- A skin's `finish` says how plated faces are finished: `kind` (`Arc`, paint
  over scanned steel scratched at the edges, or `Regency`, steel under the film),
  `plate` (plate size), `wear` (paint lost at the edges) and `lights`, the lights
  let into dark plating: `Off`, `Dashes(colour)` or `Dots(colour)`. A plain
  plated face takes its skin's finish whatever model it is on, so an ARC tank in
  the Regency skin wears the film. A face with a specific pattern (an airframe,
  a warship hull, a deck) keeps it.
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
- [ ] Face-fitted surface detail (plate courses, seams, rivets), and dirt by tier.
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

1. This spec.
2. The skin layer and the first example, the T1 tank, together. It is built on
   slots from day one, then shown in ARC's and Regency's skins. A tank that
   reads right in all three proves the layer.
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

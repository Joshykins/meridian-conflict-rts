# Strategic missiles

The nuclear silo (Sunfall, tech 4) and the interceptor array (Parhelion, tech 4, built
beside it; key still `aster_t3_nuke_defense`), and a
commander's reactor going up. Asked for on 2026-09-24: massive, slow blasts that sear
through the clouds, melt the ground and linger; a flash that blinds; a shockwave rolling
across the map; trees set alight and flattened; lightning in the mushroom cloud; distinct
icons and long high trails; a launch button that feels like one; ammunition assembled
under the player's control; and the commander's death a nuke too.

## Rules

- **Launchers assemble their own rounds** (`strategic` on the blueprint,
  `crates/mc-data/src/strategic.rs`), paid out of income like a build: a warhead is 6000
  mass / 120000 energy over about 5 minutes at the silo's own power (60); an interceptor
  1500 / 30000 over about 100 s (power 45). A silo holds 2, an array 4.
- **Auto-build is on by default**: a launcher starts assembling the moment it is finished
  and stops when full. Off, it assembles only rounds queued by hand
  (`Command::SetAutoBuild`, `Command::QueueRounds`); turning it off mid-round keeps that
  round as one queued. **Z pauses** assembly like any factory. **Engineers assist** it
  like a factory (right-click or Assist): their build power is added to the round.
- **A silo launches only when told** (`Command::LaunchNuke`, the launch button or N with a
  silo picked): anywhere on the map or the minimap. **One order is one warhead**: among the
  silos ordered, the one with the most warheads not yet given a mark takes it (ties: the
  nearest to the mark, then the lowest id); none free, nothing happens. A silo keeps its
  marks in a queue (`Launcher::targets`, numbered by `Strategic::orders`) and fires them in
  turn: the blast doors open (4 s) for the first, and stay open, the next going 2.5 s
  (`SILO_NEXT_TICKS`) after the last. The doors close 6 s after the last is away.
- **The flight** is one path (`nukes::WarheadPath`), the same one the interface draws: the
  warhead climbs dead straight up out of the tube for 380 m (about 5 s of its 7 s boost,
  which starts at 1/25 of its cruise and reaches it as the boost ends), then follows a
  cubic curve whose first control point stands straight over that climb and whose last
  stands over the mark, pulled back 15% of the span toward the silo: it pitches over
  gradually (no kink anywhere; heading and position are continuous every tick), tops out
  45% of the span above its ends (at least 900 m, at most the silo's 4.2 km apogee) and
  comes down steep (about 14 degrees off plumb on a 4 km shot, 28 on a 20 km one), speeding
  up to 1.6 times its cruise (320 m/s) over its last 2.5 km, bursting 55 m above the mark.
  From ignition: 1 km takes 11 s (tops out 1.1 km up), 4.2 km 24 s (2.1 km up),
  9 km 46 s (4.3 km), 20 km 76 s. It is walked by
  distance over 96 samples in wide fixed point; the time left shown is exact
  (`WarheadPath::ticks_left`).
- **An array fires by itself** at an enemy warhead bound for somewhere within its coverage
  (2.4 km), once that warhead is coming down and an interceptor fired now would meet it
  inside the coverage, or it is inside already (never at one still climbing, or still
  out over somebody else's country): one interceptor a warhead, fast (900 m/s), bursting
  beside it. Each tick it works out the first point on the warhead's path it can reach
  in time (`rendezvous`), turns toward it at no more than 30 degrees a tick, and paces
  itself to arrive with the warhead, not before it, so it never flies on past and has to
  come round. Several arrays covering one mark each take a different warhead. A mark just past
  the edge of an array's cover, reached from beyond it, is outside it: the cover is a
  radius, not a promise.
- **A detonation runs out.** A warhead is about 130 kt (a megaton, twice the reach since
  blast reach goes with the cube root of yield, was tried 2026-09-25 and halved back).
  Everything inside 200 m takes 80000; it falls off to 2500 at 520 m. The damage front runs
  out over 1 s (fast at first), so the middle dies at once and the edge a moment later. It hurts **everything**, its owner's too. **Domes do not stop it**:
  a dome takes what it has left and the rest goes through to what is under it, so no dome
  survives the middle of a warhead (a charged dome can still hold back a commander's
  blast). Trees die out to 1.3 times the damage radius.
- **A commander's reactor is a small nuclear blast** (`nukes::COMMANDER_BLAST`: 7000 in the
  inner 90 m, falling to 400 at 300 m, over 0.8 s), drawn and heard the same way, smaller.
- Missiles in flight are not projectiles: nothing else shoots at them, fog does not hide
  them, and every side sees every launch.

## On screen

- **The blast** (`renderer/nuke_fx.rs`, `shaders/nuke.wgsl`, `renderer/nuke_volume.rs`): a
  volume marched at half the output's size in one full-screen pass (a fixed dither, smoothed
  by the composite's tent, which along silhouettes weights the texels by how near their
  depth is to the pixel's own), so a blast filling the view costs about a millisecond. Up to
  32 blasts are drawn at once (nearest first; 48 are remembered); where many cross one ray they share the
  steps, and stretches of the ray through overlapping blasts are marched together (all
  sampled each step), so neighbouring caps blend without a seam. The view ray runs from the eye through depth 0.01, as the clouds' does: through
  depth 1e-7 it lost the cloud at some angles and zooms (rounding in `w`). No `pow()` of a
  negative base anywhere (NaN on NVIDIA, the black holes of 2026-09-25).
- **Size**: everything is drawn for the damage radius over 520 m (`scale`, 1 for a
  warhead); widths go with `scale`, heights with `rise` = scale^0.6 above 1 (a real cloud's
  top rises far slower with yield than it spreads), fire times with `slow` = sqrt(scale). Nothing
  whites out the screen: the flash is the fireball itself, far too bright to look at, and the
  light it throws over the country. The shock runs out fast along the ground (the sim's
  damage front, 1 s to the damage radius, then on at 330 m/s) with a thin curtain of dust on
  it, bending trees as it passes; a Wilson cloud, a faint shell of condensation, blooms round
  the fireball a moment behind it and breaks up within four seconds. The fireball hangs on
  the ground boiling (the billows' warp field moves through time, fast at first, and the ball
  turns over from about a second on), white, then yellow, then orange with deep red gaps;
  soot gathers on its crowns while fire shows between them, and it goes to sooty brown
  billows by about 12 s; it climbs slowly on a stem of dust and rolls over into a cap (about 1.7 km up after a minute), glowing
  through its cracks and underneath while its smoke goes to a pale grey, leaving a ring of
  condensation round the stem. A low ring of dust (the base surge) boils out along the
  ground. The stem goes first, the cap thins away within about a minute and a quarter. The
  clouds are thrown back once by the shock and once where the cap climbs through them.
  Shapes over time are mirrored between nuke.wgsl and `nuke_fx::Blast`.
- **The crater** (`renderer/craters.rs`, `terrain.wgsl craters_at`): a glassed pool that
  cools from white-yellow through orange plates and glowing cracks to black-green glass over
  about six minutes (a commander's over about four), a bowl and lip in the shading, and
  charcoal streaks feathering out past the damage radius. It stays for the match. The sim
  leaves no stain for it. `add_crater_styled(.., CraterStyle::Blast)` gives other blasts a
  crater without glass.
- **Opacity and life** (`fade_left`, `nuke_fx::BLAST_LIFE`): solid only for its first
  seconds; from about 3 s it thins to a fortieth of its density by about 25 s (a soft,
  see-through cloud) and is gone by 75 s. Asked for 2026-09-25: it stayed opaque too long
  and lasted too long.
- **The stem** (2026-09-25) is an updraft: wide where dust is drawn in along the ground,
  narrowing to a waist a quarter of the way up, then widening steadily until it opens into
  the cap, its billows (the cap's own) streaming and twisting upward. It thins at
  `fade_left`^0.65, slower than the cap, so the cap never hangs on nothing. The lingering
  cloud goes to a mid grey (albedo 0.19), not white, and its fire dies sooner (heat tail
  16 s, ember 18 s) so it does not stay orange.
- **Through the cloud deck**: the march reads the clouds' march target (distance to the
  cloud, set 1 binding 3) and resolved picture (how much gets through, binding 4; one set
  per cloud history); what lies past the deck is seen through it, eased across the deck's
  depth. `atmos.ground_color.w` = 1 while the clouds are marched (not `atmos.view.w`: that
  is the focus window's strength in `clearing()`, and setting it made every cloud
  see-through). **The fireball lights the clouds** in its own colour (clouds.wgsl
  `gather_fires`/`fire_glow`, mirroring the shape functions), white in the flash, then
  yellow, orange, a dim red. The cap's lightning lights the clouds at 0.3 of a storm
  stroke (`Sky::strike`'s `glow`), thunder unchanged.
- **Salvos** (2026-09-25; 20-60 warheads on one target will be common). **Every warhead
  keeps its own fireball, stem and cap.** Pouring one column into a neighbour (a later
  fireball leaning over and draining into a standing column, neighbouring columns
  coalescing) was built and dropped the same day: in a carpet some warheads never got a
  mushroom of their own, only a large one beside them, which the user found underwhelming.
  What is left (`nuke_fx.rs` "Salvos"): a burst on the very spot of a fireball still on
  the ground (within a quarter of its damage radius, under 1.5 s) goes into it, as the two
  could not be told apart: it flares and grows a little (at most 1.6 times). Every burst
  stirs the columns standing near it (within 1.6 of their heads, under 60 s old), more the
  nearer: their billows churn (`churn`, added to the boiling's phase), their caps heave (5%,
  never past 1.1 of their size) and its flash lights them from below (`fuel`). The blast's
  fourth vec4 in `Globals::nukes` is (fuel, unused, churn, thick), read by nuke.wgsl and
  clouds.wgsl `gather_fires`. Up to 64 blasts are drawn (96 remembered), 32 stretches a ray.
  A crater hit again is heated again and widens a little (at most 1.5 times a warhead's)
  instead of a copy crowding older craters out.
  **Cost** was mostly one bug: the surge's box reached the shock's front for its whole
  minute, a flat box kilometres wide, so every blast of a salvo overlapped every other
  along the ground and all were marched together. It now reaches the front only while the
  dust curtain stands (8 s). The joint march never takes more than 80 steps. Missiles: up
  to 64 drawn (nearest first); a salvo lays its trails in longer steps (up to 3 times) so
  every trail fits the reserved puff slots.
- **Clouds** are disturbed, never cleared: a blast swirls them round it with a little shove
  outward (clouds_sim.wgsl `KIND_BLAST`) and thins them only very slightly.
- **Lightning in the cloud** while it forms (3-45 s): jagged strokes through the cap and
  down to the ground, each lighting the volume from inside and, through the sky's flash
  system, the clouds and country round it, with thunder.
- **Trees**: under the fireball they are gone; further out a share of them (at most 320 a
  blast) are thrown flat, away from the middle, scorched, and the rest are gone too; a few
  at the edge (at most 24) burn where they stand. Every fallen or burning tree is drawn and
  shadowed on its own, which is what made blasts over forest crawl before.
- **Missiles**: lathed bodies (a white warhead with a dark re-entry vehicle and a team
  band; a dark interceptor with an orange band), a motor plume while they burn, and thick
  white trails that hang and drift for most of a minute (reserved puff slots). A warhead
  coming down glows at the nose. The plume (`vs_plume`/`fs_plume`) is a glow marched
  through a lathed hull from the nozzle at the body's tail, so it is round from every side
  and a bright disc seen up the tail. The trail (puffs.wgsl `strategic_trail`) is a chain
  of tube segments whose density tents overlap to add up to one, integrated along each
  pixel's ray, so it has no seams from any side or straight down it; it starts a step
  behind the nozzle.
- **The silo**: blast doors slide apart for a launch (`part::SILO_DOOR`, `deploy`), the
  warhead stands in the tube while one is held (`part::SILO_ROUND`, `_pad3[2]`), smoke pours
  out of the tube and flame trenches. The array's cells show the rounds it holds.

## Interface

- **The launcher panel**, right of the order card (`hud/silo.rs`): rounds as missile
  slots filling up, assembly state and time left, auto-build, the queue with auto-build
  off, and on a silo the launch button: hazard bands crawling along its edges and a red
  core breathing while armed, "Select target" while aiming, "Launching" while every
  warhead has a mark; dark and showing the next warhead's assembly otherwise. **Several
  silos picked read as one battery** (`Battery`): "Warheads 7 / 10 · 5 silos", one slot a
  warhead (those with a mark underlined), "2 targeted · 5 free", how many are assembling;
  auto-build acts on them all.
- **Aiming** (`nuke_marks.rs`): the core, damage and burn rings under the pointer, the
  flight the warhead would fly (`WarheadPath`, drawn in space with its ground track) from
  the silo that would fire it (`silo::next_silo`, the sim's rule), the flight time with
  the doors, how many warheads are left to give out beside the pointer, and every known
  enemy array's cover, with "Under interceptor cover" when the mark lies in one. A click
  gives one warhead and stands down; **shift-click keeps aiming**, each click giving the
  next, until none is free. Launches sent are counted against their silo until the frame
  shows them (`View::nuke_sent`, `silo::settle_sent`), so quick clicks never spend one
  twice. The minimap takes a click.
- **Launches waiting their turn** (ours only; `RenderFrame::planned_launches`): each mark
  numbered in the order given, its blast ring, the whole flight from its silo, and the time
  to the burst ("Doors opening" for the one under way); on the minimap, the track from the
  silo and the number.
- **In flight** (`RenderFrame::warhead_tracks`): every warhead's icon on the map with what
  is left of its flight ahead of it and that flight's ground track, the blast rings at the
  mark pulsing faster as it comes, and a countdown; interceptors as cyan darts; the same
  on the minimap (a warhead flies in the upright plane through silo and mark, so its
  track there is straight). A banner per warhead across the top ("Nuclear launch detected"
  for an enemy's, with a klaxon), click to look. Short notes for "Warhead intercepted" and
  "Warhead ready".
- **Icons**: `IconKind::Silo` (a missile in an open tube) and `IconKind::AntiNuke` (a
  missile rising out of a shield's bowl toward its mark).

## Sound (`data/sounds/nuke.ron`)

Everything big and low; nothing whines or rings. The detonation (30 s: a crack, a slam and a
sub-bass thump, the ground heaving three times, the shock cracking overhead now and then,
and thunder rolling for half a minute as `Roll` layers, low-passed noise whose level swells
and sags at random, with its body at 60-120 Hz so laptop speakers carry it), heard anywhere
at once (no delay for the sound's travel, asked for 2026-09-25); the launch in two stages
(the tube's charge, a whump and steam, then the motor lighting: a slam, a sub-bass shove and
a low fluttering roar torn by a solid motor's crackle); `warhead_flight`, looped while it
climbs (the roar and its crackle), and `warhead_fall`, looped as it comes down (a deep rush of
air, louder as it nears the ground, never higher); the interceptor's crack; the high kill; the silo doors; the klaxon; a low
settle when a warhead is ready. The commander's death keeps its own recipe (16 s, the same
rolling tail, smaller), trimmed of its climbing whine and its rising tail; the victory/defeat chord no longer plays over it.

Salvos (2026-09-25, `crates/mc-game/src/audio/salvo.rs`, used by `nuke_sounds` and
`warhead_loops` in `game.rs`): 20-60 warheads at once must sound massive, not like a wall
of noise, and must not eat the mixer's voices. On the client's wall clock, never sim state:
the alarm sounds once and not again while it is still sounding (its length, 7.5 s); launches
within 0.8 s of a roar's start fold into it (so at most one roar group per 0.8 s; a silo
queue firing every 2.5 s is heard each time), the roar at `0.7 * (1 + 0.3 ln n)` capped at 1,
with a layer at pitch 0.9 (0.14 s late) from 4 launches and one at 0.82 (0.36 s late) from
10, so a big salvo is deeper rather than louder; a detonation within 1 s of the last one
played folds into it (a tick's bursts are one, the loudest), at most 5 full detonations
start in any 30 s, and past that one at pitch 0.85 and 0.6 of the gain, no oftener than every
4 s; interceptor launches, kills and silo doors play at most once per 0.25 s each (the
loudest of the tick). The loops are at most three: one `warhead_flight` and two
`warhead_fall` (the warhead nearest to landing, and the rest), each merged as the root of
the summed squares of the warheads' levels, capped at 1, panned by the level-weighted mean.
Checks: `cargo test -p mc-game --bin meridian salvo`.

## Checks

- Sim: `cargo test -p mc-sim --test sim -- nukes::` (assembly, auto-build and queue, assist,
  flight and its shape, front, domes, interception and holding fire outside the cover,
  interceptors from every side of a mark and against a salvo flying straight in,
  launches in turn, a launch order finding the silo with the most free, commander).
- Blast frames on the GPU: `cargo test --release -p mc-render --lib nuke_shots -- --ignored
  --nocapture` with `NUKE_TIMES`, `NUKE_CAM` (dist,yaw,tilt), `NUKE_AT`, `NUKE_RADIUS`,
  `NUKE_MISSILE=1` for missiles in flight, `NUKE_OUT`. Salvos: `NUKE_SALVO=n,seconds,metres`
  (n more warheads after the first, spread over that time and up to that far off the mark),
  `NUKE_MISSILES=n` (n warheads coming down from all round first). Each written frame
  prints what the volume costs (the scene with and without it) and the blasts held.
- Salvo stirring and folding: `cargo test -p mc-render --lib salvo`. Sim: `cargo test --release -p mc-sim
  --test sim -- nuke_salvo:: --nocapture` (60 silos on a T5 with an array beside it; prints tick
  times; `NUKE_SALVO=n` for more).
- In a match shot: `MERIDIAN_NUKE=x,y[,ticks]` arms player 0's silo and launches;
  `MERIDIAN_AIM=1` with `--cursor` shows aiming (add `MERIDIAN_ARM=2` to give the silo its
  warheads; `MERIDIAN_AIM=ground` aims a titan's strike instead, `MERIDIAN_AIM=reclaim` gives
  the Reclaim order with what is under `--cursor` ringed, and `MERIDIAN_RECLAIM=1` holds up
  the Control reclaim survey); `MERIDIAN_NUKE=x,y,ticks,x2,y2` queues a second mark on the same silo. E.g. `--range --unit aster_t4_nuke_silo
  --select nuke_silo --ticks 100 --follow 200 --camera 10200,12300,3200,20` with
  `MERIDIAN_NUKE=10200,12300,190` on dev16.

## Not done

- The AI neither builds silos nor arrays (they carry only the `Strategic` category, which
  no AI job asks for).
- `nuke_shots` checks: `NUKE_NO_TREES=1` leaves the forest standing (for costing the volume
  alone); every written frame prints the least CPU and GPU pass times over nine renders.

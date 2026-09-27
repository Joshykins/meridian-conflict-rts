# Replays and marked issues

## What is recorded

Every skirmish, survival match, multiplayer match and test range session writes
`replays/<id>.mcreplay` beside the working directory. The id is the match's start time in
UTC (`20260926-223017`). The file is the match's start message followed by the command
log, one record per tick (see `crates/mc-net/src/replay.rs`), flushed every 5 s of play,
so a replay can be read while its match is still running and a crash leaves one that
plays up to the crash. Scripted scenes (`--scene`) are not recorded. The newest 20
unmarked replays are kept.

A single-player session records itself. A multiplayer match and the test range are
recorded by the sim thread instead (`crates/mc-game/src/recorder.rs`), from what it
carries out:

- In a multiplayer match every player keeps their own copy. A player who joins a match
  already running has no copy (they came in on a snapshot, with no ticks before it). One
  who drops and reconnects keeps the ticks up to the drop.
- On the test range the opening set-up (the pad subject and a staged scenario) is written
  into the ticks it was carried out in, so playback needs nothing special. The Sky tab's
  weather is kept as notes (record 6), and a replay shows each one from the tick it
  changed at, windowed or headless. Reload Data starts a new recording.

A replay holds commands, not state: playing one runs the simulation again. That makes
the files small, but it only reproduces the match if the simulation, the unit data and
the map are the ones that recorded it (see *Older builds* below).

## Marking an issue

F1 opens the profiler with a Report card above it: the match id (Copy puts it on the
clipboard), a note field and **Mark Issue** (Enter in the note also marks). A mark:

- appends a block to `replays/issues.log`: match id, mark number, tick and match time,
  note, map, camera, FPS, CPU and GPU time with every GPU scope, sim phases, counts, and
  a `repro:` command line;
- saves what the window showed, HUD included, as `replays/<id>-mark<N>.png`.

Marks made while watching a replay go on that replay's match.

## Watching

Main menu, **Replays**: every recording, newest first, with map, date, length,
commanders and marks. Watch from the start, or click a mark to start 5 s before it.
In the match, the timeline along the bottom shows the match clock and every mark (hover
for its note). Click or drag the track to go anywhere; Start, -30 s, Play/Pause, +30 s,
and Prev/Next Mark step about. Game speed works as in a match; the replay stops at its
end rather than closing, so it can be rewound.

Going back restores the nearest snapshot (the player keeps one every 30 s of play, fewer
past 1 GB) and replays forward to the moment at full speed without drawing, so a jump
takes about as long as simulating the ticks between the snapshot and the target.

From the command line:

```bash
meridian --replay replays/20260926-223017.mcreplay --at 8:43
```

opens it in a window at 8:43. `--at mark:2` goes to mark 2 of that match; with
`--screenshot FILE.png`, `--bench` or `--perf FILE.json` it runs headless instead, which
is what the `repro:` line in the issue log does. A mark made through the free camera
also carries its pitch and lens (`MERIDIAN_PITCH`, `MERIDIAN_FOV`) and the height it
held its focus at (the fifth `--camera` value), so the shot sees what the window did.

## Older builds

A replay only plays back faithfully on the build that recorded it. Four things decide
that, and the game checks each:

| What | Where it is kept | What happens on a mismatch |
|---|---|---|
| Replay format | `REPLAY_FORMAT_VERSION` in the file header | Refused; listed as "recorded by another build" |
| Unit data | `blueprint_hash` in the start message | Plays, but may diverge |
| Map | `map_id` (content id) in the start message | Refused when no map in `maps/` has it |
| Simulation code | the build name (record 5) | Plays, but may diverge; the Replays screen warns |

Divergence is caught: the recording keeps the state hash of every tick, and playback logs
`replay diverged at tick N` at the first tick that differs.

The build name is `MERIDIAN_BUILD` at compile time, or the package version with `-dev`.
Playing an old replay faithfully therefore means running the build it names. That is a
distribution job, not something a single binary can do:

1. The release pipeline sets `MERIDIAN_BUILD` to a unique name (version plus commit) and
   keeps every released build, with its `data/` and baked maps, available to download.
2. The launcher, given a replay whose build is not the installed one, fetches that build
   into a side-by-side folder and starts it with `--replay FILE`.
3. The newest build can still list, and explain, every old replay: it reads the header
   and the build record without playing it.

Until then, a replay from another build opens with a warning and may play out
differently from the match that was recorded.

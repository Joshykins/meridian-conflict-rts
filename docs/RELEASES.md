# Builds, channels and old replays

## A build's identity

`crates/mc-game/build.rs` stamps every build with:

| Stamp | What | Example |
|---|---|---|
| `MERIDIAN_CHANNEL` | Who it is for: `dev` (built by hand, the default), `playtest` or `release`. Set it in the environment of the build. | `playtest` |
| `MERIDIAN_BUILD` | Its name: version, channel (left out for a release), short commit | `0.1.0-playtest+183fa3e2e1` |
| `MERIDIAN_BUILD_NUMBER` | Commits up to it; "Build N" on the menu | `1099` |
| `MERIDIAN_COMMIT` | The full commit hash | |
| `MERIDIAN_SIM` | The simulation fingerprint: a hash of the sources of every crate listed in `crates/mc-game/sim_crates.txt`, with `Cargo.lock` and `rust-toolchain.toml` | `a0bf5e5461660d67` |

`meridian --version` prints them. The channel is `mc_core::Channel`; its numbers are
written into replays, so they are never renumbered.

Network players must run builds with the same name, so a playtest build and a release
build never share a match.

## Playtest-only units and maps

A unit or a map can be marked playtest-only, to try it on playtesters before
everyone has it. Dev and playtest builds have it; a release build does not have it
at all.

- **A unit:** `playtest: true` in its entry in `data/factions/<faction>/units/*.ron`.
  mc-game loads the data with `Blueprints::load_for(dir, build_info::channel())`; in
  a release build that leaves the unit out before the tables are compiled
  (`crates/mc-data/src/playtest.rs`), and with it its lore and every build list
  entry naming it (factories, engineers, the commander, refit modules), so it is in
  no build menu, factory roster, AI plan, unit browser or line-up. An upgrade into it
  goes too: the unit below stops at its own tier. A unit that cannot work without it
  is an error, not a broken unit: mark that one playtest-only as well. That covers
  a builder whose whole list is playtest-only (a release factory that builds
  nothing), a faction's commander, a drone and a gun's sabot casing. A test loads
  the checked-in data as a release build, so `scripts/check.sh` catches such a flag
  before any release does.
- **A map:** `playtest: true` in its settings file, `maps/<stem>.ron`.
  `setup::list_maps`, which every map list reads (skirmish, survival, multiplayer,
  the map browser, the range), leaves it out of a release build, and `--map` refuses
  it there.

Since the unit data differ, a release build's unit data hash differs from a
playtest build's whenever anything is flagged: replays and network games never
cross between them.

## Each channel's server

Playtest and release builds play on servers of their own, so the two never meet in
one list of games. The address is set when the build is made:

| Variable (compile time) | Read by |
|---|---|
| `MERIDIAN_SERVER_PLAYTEST` | a playtest build |
| `MERIDIAN_SERVER_RELEASE` | a release build |

`host:port`, or a host alone for port 7777. The release script sets both, for every
build: each build reads only its own channel's, so the two cannot be crossed. Left
unset, and in a dev build always, there is no default server and the multiplayer
screen opens on the local network. A player can still type any address; the
settings file remembers it per channel (`servers`), so a playtest and a release
build on one computer keep their own. How to run the second server:
`docs/SERVER.md`, *A second server for playtest builds*.

## Why the fingerprint

A replay holds commands, not state, so it plays back faithfully only on a build with
the same simulation, unit data and map. A build whose simulation fingerprint and unit
data hash match a replay's plays it, whatever else changed: an update that touches only
drawing, sound or the interface keeps every old replay playing in the newest build.
Anything else needs the build that recorded it (`docs/REPLAYS.md`, *Older builds*).

`sim_crates.txt` must list mc-sim and every workspace crate it depends on; a test in
`build_info.rs` fails when a listed crate uses an `mc-*` crate the file does not list.

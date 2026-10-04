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

## Why the fingerprint

A replay holds commands, not state, so it plays back faithfully only on a build with
the same simulation, unit data and map. A build whose simulation fingerprint and unit
data hash match a replay's plays it, whatever else changed: an update that touches only
drawing, sound or the interface keeps every old replay playing in the newest build.
Anything else needs the build that recorded it (`docs/REPLAYS.md`, *Older builds*).

`sim_crates.txt` must list mc-sim and every workspace crate it depends on; a test in
`build_info.rs` fails when a listed crate uses an `mc-*` crate the file does not list.

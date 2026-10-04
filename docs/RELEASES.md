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

## The build store

Every published build is kept, so any replay can be played by the build that
recorded it. The store is a plain tree of files on Cloudflare R2, read over HTTPS
(`crates/mc-builds`):

```text
blobs/<sha256>.zst                           one file's bytes, zstd-packed, shared by
                                             every build that has that file
builds/windows-x64/<build>.json (+ .sig)     a build's manifest: every file by hash
channels/<channel>/windows-x64.json (+ .sig) the channel's newest build
```

A build is a folder: `meridian.exe`, `data/`, `maps/` and
`launcher/MeridianConflict.exe`. Since blobs are named by their bytes, publishing a
build uploads only the files that changed (usually the executable and a few data
files), and installing one downloads only what no installed build already has.

Manifests are signed with the release key. A build trusts only manifests that verify
against the public key it was built with (`crates/mc-builds/signing.pub`), and checks
every file it downloads against its manifest's hash, so the store and the connection
need not be trusted. The private key is `~/.config/meridian-release/signing.key` on
the machine that publishes: **back it up**. Lost, no build already out there will
trust a new one's updates (players would install afresh); leaked, anyone holding the
bucket's write key too could publish. `mc-release keygen` made it, once.

Downloads go through the system's `curl` (in Windows 10 and later, and every Linux),
so the game carries no TLS stack.

## An install and the launcher

What a player unzips and runs:

```text
MeridianConflict.exe      the launcher (crates/mc-launcher)
versions/<build>/         each installed build, whole; a file shared with another
                          build is a hard link, so an old build costs only what changed
versions/current          the build the launcher starts
replays/                  the game runs here: recordings and marks
launcher.log              what the last start did
```

- **Starting:** the launcher starts the current build, in the install's folder, with
  `MERIDIAN_LAUNCHER` naming itself, and exits. It keeps the 8 builds used last.
- **Updates:** a game the launcher started looks for its channel's newest build as the
  front end opens and downloads it in the background (`crates/mc-game/src/builds.rs`).
  A card at the foot of the menu then says *Build N is ready* with **Restart**, which
  starts the launcher and quits. An update brings the launcher it was published with;
  the game puts it beside the running one as `MeridianConflict.exe.new`, and the
  launcher swaps it in on its next start.
- **Old replays:** Match History shows, for a recording this build does not play as
  recorded (`docs/REPLAYS.md`, *Older builds*), **Download Its Build** with its
  progress, then **Watch in Its Build**. That starts `MeridianConflict --replay FILE`
  and quits; the launcher plays the replay in that build with `--replay-only`
  (leaving the replay quits it, and it never writes the settings file, which is the
  newer build's), then starts the current build again on Match History
  (`--open history`). Started by hand with a replay whose build is not installed, the
  launcher downloads it first.

A dev build (no store) and a game started without the launcher have no install: Match
History names the build a recording needs and the command below instead.

## Branches

| Branch | Channel | Moves by |
|---|---|---|
| `dev` | dev, never published | every session landing (`scripts/worktree.sh land`) |
| `playtest` | playtest | `scripts/channel.sh promote playtest` (to `dev`), a playtest hotfix |
| `master` | release | `scripts/channel.sh promote release` (to `playtest`), a release hotfix |

Each branch is contained in the one below it (`master` in `playtest` in `dev`), so a
promotion is a fast-forward: release gets exactly what playtesters played, and
playtest exactly what was on `dev`.

A hotfix is a fix for a published build that cannot wait for `dev`'s next promotion:

```bash
scripts/channel.sh hotfix release crash-on-load
```

makes a worktree (`../mc-hotfix-crash-on-load`) on a branch from `master`. Fix and
commit there, then `scripts/channel.sh land` from inside it: it runs
`scripts/check.sh`, fast-forwards `master`, and merges `master` down into `playtest`
and `playtest` into `dev`, in scratch worktrees, so the fix is not lost at the next
promotion. A merge that stops on a conflict is left in `../mc-merge-<branch>`:
resolve and commit there, and `scripts/channel.sh merge-down release` finishes it.
`scripts/release.sh release` then publishes the fix, and `scripts/channel.sh finish`
removes the hotfix worktree. A playtest hotfix is the same with `playtest`.

`scripts/channel.sh status` shows where the three stand. Nothing here pushes to
GitHub: `git push origin dev playtest master` when you want them there.

## Publishing

```bash
scripts/release.sh playtest
```

builds the tip of the channel's branch (`playtest`, or `master` for release) for Windows with the channel, the store's URL and the
channel's server from `release/config.sh`, stages it (a release leaves out
playtest-only maps), packs and signs it into the local copy of the store
(`~/.local/share/meridian-release/store`), uploads what R2 lacks (blobs, then the
build's manifest, then the channel's pointer, last), and writes
`target/dist/MeridianConflict-Playtest-Build<N>.zip`: the launcher with this build
installed, to hand to a new player. Players who have an install get it as an update.
`--no-upload` stops before uploading: the zip still works, and a later run uploads.
The build's `.pdb` goes to the main checkout's `target/dist` (`MeridianConflict-<Channel>-Build<N>-<commit>.pdb`), where `scripts/symbolize.sh` finds it for a crash report from that build: keep (and back up) every published build's.

Setting up R2, once:

1. In Cloudflare, create an R2 bucket and give it public access (a custom domain, or
   the bucket's `r2.dev` URL). Put that URL in `STORE_URL` and the bucket's name in
   `R2_BUCKET` in `release/config.sh`, with its S3 endpoint
   (`https://<account id>.r2.cloudflarestorage.com`) in `R2_ENDPOINT`.
2. Create an R2 API token with write access to the bucket, and give it to the AWS CLI:
   `aws configure --profile meridian-r2` (region `auto`).
3. Fill in `SERVER_PLAYTEST` and `SERVER_RELEASE` (`docs/SERVER.md`).

A build bakes in the store URL: moving the store later strands the builds already
out there, which keep reading the old one.

## Dev builds and old replays

Dev builds are never published. A replay one recorded names its commit, and

```bash
scripts/replay-build.sh replays/<id>.mcreplay
```

builds that commit for Windows in a worktree of its own (`../mc-replay-<commit>`,
kept, with a build directory shared by every commit so later builds are incremental)
and opens the replay in it. `mc-release origin FILE` prints what a replay says of
the build that recorded it.

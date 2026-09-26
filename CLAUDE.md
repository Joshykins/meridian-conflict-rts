# Meridian Conflict: rules for working in this tree

These rules hold for every change, by a person or an agent. The why and the full
findings behind them are in `docs/AUDIT-2026-09-25.md`; the engine's own rules
(determinism, world conventions) are in `docs/ARCHITECTURE.md` and still apply.
A rule marked **[gate]** is, or is planned to be, checked by `scripts/check.sh`;
until the gate lands, follow it by hand.

## 1. Work lands in commits, and many sessions share this tree

Several Claude sessions usually work in this checkout at the same time, each on
its own task. They share one working tree, one index and one `dev` branch, which
keeps builds warm and the Windows GPU build pointed at one place. The rules
below exist so that no session ever loses, commits or rewrites another
session's work.

**Commit your own work, and only your own work.**
- Keep a list of every file you create, edit or delete, and commit when a piece
  of work is done and verified: `scripts/commit.sh -m "message" <your paths>`.
  It commits exactly those paths, retries while another session holds the git
  index lock, and never picks up anything else. Commit small and often. Work
  left uncommitted at the end of a task is a bug.
- Before committing a file, read its diff (`git diff -- <path>`). If a hunk is
  not yours, another session is editing that file too. Message it
  (`ListAgents`, then `SendMessage`) and agree who commits the file and when.
  Don't commit someone else's half-done edit without asking. Don't revert it.
- Never, in the shared tree:
  - `git add -A`, `git add .` or `git commit -a`
  - `git stash`
  - `git reset --hard`
  - `git checkout -- <path>` or `git restore` on a path you did not change
  - `git clean`
  - switching branches, rebasing or amending a commit that is not your own
  - `cargo fmt` across the workspace, or `cargo clippy --fix` across the
    workspace

  Each of these rewrites or captures other sessions' files. Format only your
  own files: `rustfmt --edition 2021 <paths>`.
- The pre-commit hook (`scripts/hooks`, turned on with `git config
  core.hooksPath scripts/hooks`) checks only what is being committed:
  - that it is rustfmt-formatted
  - that it adds no `#[allow]` or `dbg!`
  - that it adds no artefacts (`.pyc`, `.mcmap`, root screenshots, files over
    5 MB)

  It never builds, because the tree around your files may be mid-edit by
  someone else.

**Check what you committed, not the shared tree.**
- `scripts/check.sh` runs fmt, clippy with warnings as errors, and every test.
  In the shared tree it can fail because of another session's half-done work.
  That is not yours to fix: tell that session.
- `scripts/check.sh --head` runs the same check on the last commit alone, in a
  worktree of its own (`../meridian-conflict-verify`). Run it after
  committing. If HEAD does not build because your commit needs a file another
  session hasn't committed yet (or the other way round), sort it out with that
  session straight away.
- A commit that is known to break the build or the tests does not stay on
  `dev`: fix it forward within the hour, and say so in the message.

**Broad or long work goes in a worktree.**
- A change that touches many files at once goes on a worktree and branch of
  its own, with its own target dir:
  - reformatting
  - renames or moving modules
  - lint sweeps
  - a refactor that takes hours

  Create it with `git worktree add ../mc-<topic> -b <topic> dev`, then link
  the baked maps in with `ln -s $PWD/maps/*.mcmap ../mc-<topic>/maps/`.
- To land it:
  1. Rebase onto `dev` inside the worktree.
  2. Run `scripts/check.sh` there.
  3. Fast-forward `dev` from the shared tree: `git merge --ff-only <topic>`.

  Git refuses the merge, and changes nothing, if a file it would update has
  uncommitted edits in the shared tree. If that happens, ask the session that
  owns those edits to commit them, then rebase again.
- Say so to the busy sessions (`ListAgents`) before you land a broad change:
  their open files change under them.
- Formatting-only changes (rustfmt runs, renames) go in commits of their own.

## 2. No dead code, no parked code

- Git history is where old work is kept. Do not keep a model, shader branch,
  icon, sound, data file, CLI flag or enum arm "parked" for later. Delete it.
  Recover it from history if it comes back.
- **Removing a feature removes all of it.** Go through this list before you
  commit a removal:
  - sim code
  - blueprint/data files
  - render model
  - shader branches and constants
  - icon slot drawing
  - sounds
  - HUD
  - CLI flags and `--help`
  - env switches
  - tests and probes
  - README and docs
  - memory notes
- No `#[allow(dead_code)]` or `#[allow(unused_*)]`. If something is used only
  by tests, put it behind `#[cfg(test)]`. If it truly must stay, write
  `#[expect(dead_code, reason = "...")]`. That warns when the item comes back
  into use, so a stale suppression cannot outlive its reason **[gate]**.
- New items are private or `pub(crate)` by default. Make something `pub` only
  when another crate uses it; `unreachable_pub` then lets rustc find dead code
  **[gate]**.
- A `zz_*` probe is either `#[ignore]` and documented (what it measures, how
  to run it), or it asserts and is renamed as an ordinary test. Nothing in a
  plain `cargo test` may need a file that is not checked in, such as a baked
  `.mcmap`.
- Documentation is code:
  - A README or doc command that no longer runs is a bug.
  - A new `--flag` goes in `--help`.
  - A new env switch goes in the switch list (see 7).

## 3. The simulation stays deterministic

The ARCHITECTURE determinism rules apply to mc-sim, mc-path and anything they call.
In addition:

- Float math must never reach sim state. That includes `to_f32()` and
  `from_f32()` on the way in or out. Presentation floats (the mirror, HUD
  helpers, dead-zone rings) live in clearly marked presentation modules. They
  are never scattered through tick code **[gate: source-scan test + clippy
  `disallowed_types`/`float_arithmetic` in mc-sim, mc-path]**.
- Sorting is total. Use a stable sort, or `sort_unstable` on a key with no
  ties. `sort_unstable` followed by `dedup` on a key with ties is not
  deterministic across std versions.
- **Every cap has a visible result.** A full table, a patrol point list or a
  standing-order list returns a `SimError` that reaches the player. It never
  truncates with `.take(N)` or skips with `None => {}`. AI-side caps are
  allowed, and each one gets a comment saying so.
- A new unit domain or system (air, navy, nukes, survival...) gets a scenario in
  the thread-count determinism matrix. The same match must give the same hash
  at 0, 1, 3 and 8 workers, and after a snapshot/restore.

## 4. One source of truth for every CPU-GPU contract

- Rust owns the ids, bits and array lengths the shaders use: part ids,
  materials, rig bits, `ModelInfo` flag words, puff/glow kinds, pass kinds and
  buffer array sizes. The shaders get them as generated WGSL constants. Do
  not hand-copy a number into a shader, and do not write bare literals like
  `in.part == 16u` or `model.icon & 0x400000u` **[gate: generated prelude]**.
- Every `#[repr(C)]` struct uploaded to the GPU has a test comparing its size
  and **every field offset** with the WGSL struct, using naga's layouter. Every
  descriptor binding is checked against the pipeline layout table **[gate]**.
- A WGSL struct is defined once, in a shared prelude file. It is never copied
  into two shaders.
- An enum whose numbers cross a boundary (a shader, a file format or the wire)
  has explicit discriminants. Never renumber a variant, and never reuse a
  retired number. When you retire one, leave a `// retired: N` comment, not a
  dead variant.

## 5. Behaviour comes from data, not names

- Outside `data/`, tests and debug scenes, no code branches on a unit key,
  mesh name or sound name. That rules out `== "aster_..."`, `mesh ==
  "assault_air"` and `sound == "aster_shatter"`. Add a blueprint or model field
  instead, such as `flak`, `assault_dive` or a per-faction `air_support`, so a
  second faction (Naga) gets the behaviour by data.
- Per-model traits live in one record per model (one `ModelDef`), not in
  several `match mesh { ... }` tables that must all be updated together.

## 6. Size and shape

- Do not add fields to the god structs `Renderer`, `Game`, `Hud` or `World`.
  Add a sub-struct that owns its state, its buffers and its `destroy`/`Drop`,
  and give the parent one field.
- A new function over ~150 lines, or a file over ~1500 lines, gets split
  before commit. If you have to touch a file already over the line, move the
  part you are touching out.
- Test modules longer than ~300 lines go in a sibling `tests.rs` file.
- GPU resources are owned. A new buffer or image gets an owner that destroys
  it, not a new line in a hand-written list in `Renderer::drop`.

## 7. Safety, errors, configuration

- `unsafe` is allowed only in mc-render, mc-jobs, mc-music and mc-game's
  window/loader glue. Every other crate has `#![forbid(unsafe_code)]`. Every
  `unsafe` block has a `// SAFETY:` comment saying why it holds **[gate:
  `clippy::undocumented_unsafe_blocks`]**. A type that implements `Send`/`Sync`
  by hand does not have a safe constructor for arbitrary `T`.
- Bytes from outside the process are untrusted: the network, replays, snapshots,
  match options and map files. Decode them with a size limit
  (`bincode::options().with_limit(..)`), and check invariants after decoding
  (`State::validate`). Never `unwrap` in mc-net or decode paths.
- Library crates return typed errors. `Result<_, String>` is for the binary's
  top level only.
- Every env switch and CLI flag is listed in one place: the `--help` text and
  `docs/SWITCHES.md`. A switch that is not listed does not get added.

## 8. Toolchain

- The toolchain is pinned in `rust-toolchain.toml`, so Windows and WSL build
  with the same compiler. Change the pin in a commit of its own.
- New dependencies go in `[workspace.dependencies]` and are used with
  `workspace = true`. Crates never use `path = "../..."` dependencies.

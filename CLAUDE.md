# Meridian Conflict: rules for working in this tree

These rules hold for every change, by a person or an agent. The why and the full
findings behind them are in `docs/AUDIT-2026-09-25.md`; the engine's own rules
(determinism, world conventions) are in `docs/ARCHITECTURE.md` and still apply.
A rule marked **[gate]** is checked by `scripts/check.sh`, which runs rustfmt,
clippy with warnings as errors, and the tests. A rule without the mark is
followed by hand and in review.

## 1. Work lands in commits, and many sessions share `dev`

Several Claude sessions usually work at the same time, each on its own task,
and all of them land on one `dev` branch. The rules below exist so that no
session ever loses, commits or rewrites another session's work, and so that no
session waits on another's build.

**Work in a worktree of your own.**
- A task that edits code or takes shots starts with
  `scripts/worktree.sh start <topic>` and works in the directory it prints:
  your own branch, your own build dirs (Windows and WSL) and your own shot
  server. Another session's half-done edit never breaks your build, and your
  shots never queue behind their builds. A new worktree's first builds are
  cold (about 5 minutes for the Windows game build, more for the first
  `scripts/check.sh`), so keep one worktree for the whole task rather than
  making a new one per change; after that builds are incremental.
- Commit there as you go; plain git is fine inside your own worktree, since no
  one else's files are in it. Land small and often, every unit and every fix:
  `scripts/worktree.sh land` rebases onto `dev`, runs `scripts/check.sh` and
  fast-forwards `dev`. Keep working in the same worktree afterwards, and run
  `scripts/worktree.sh finish` when the task is done.
- The main checkout (the `meridian-conflict` directory, on `dev`) is where
  landings happen, so keep it clean: `git merge --ff-only` refuses to update a
  file someone is editing there. A small edit made directly in it (a doc, a
  data value) follows the shared-tree rules below.
- Before landing a broad change (a rename, a move, a lint sweep), tell the busy
  sessions (`ListAgents`): their worktrees will conflict on rebase. Formatting-
  only changes (rustfmt runs, renames) go in commits of their own.

**In the shared main checkout, commit your own work and only your own work.**
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
- The tests build in the `gate` profile (optimised, incremental, no LTO), and a
  new checkout's third-party crates come ready-built from
  `scripts/target-seed.sh`, so a check after an edit takes under a minute. Run
  tests by hand the same way, `cargo test --profile gate -p <crate>`, not
  `--release`, which rebuilds everything a second time without incremental.
- `scripts/check.sh --head` runs the same check on the last commit alone, in a
  worktree of its own (`../meridian-conflict-verify`). Run it after
  committing. If HEAD does not build because your commit needs a file another
  session hasn't committed yet (or the other way round), sort it out with that
  session straight away.
- A commit that is known to break the build or the tests does not stay on
  `dev`: fix it forward within the hour, and say so in the message.

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
- mc-sim's integration tests build as one binary, `sim` (`tests/all.rs`): a new
  `tests/<name>.rs` gets a `mod <name>;` line there (a test fails otherwise),
  and one file's tests run with `cargo test -p mc-sim --test sim -- <name>::`.
- Documentation is code:
  - A README or doc command that no longer runs is a bug.
  - A new `--flag` goes in `--help`.
  - A new env switch goes in the switch list (see 7).

## 3. The simulation stays deterministic

The ARCHITECTURE determinism rules apply to mc-sim, mc-path and anything they call.
In addition:

- **Floats never reach sim state.** That includes `to_f32()` and `from_f32()`,
  and platform maths like `atan2`/`sin`/`sqrt` on floats, which differ between
  the Windows and Linux builds. Use `Fx`, `Angle` and integers.
- **The determinism gate [gate].** Each of `crates/mc-sim/clippy.toml` and
  `crates/mc-path/clippy.toml` bans f32/f64, `HashMap`/`HashSet`, wall-clock
  time, thread identity and `sort_unstable_by(_key)`. Each crate's `lib.rs`
  switches the ban on, along with `float_arithmetic`, for the simulation but
  not its tests. `tests/determinism_gate.rs` fails if either is removed.
  - Presentation code (the render mirror, HUD and audio values nothing writes
    back into `State`) may use floats. Each such item says so with
    `#[expect(clippy::float_arithmetic, ..., reason = "presentation: <who reads
    it>")]`.
  - Only `mirror.rs` is exempt as a whole module (the fabricator geometry
    both sides share lives in `mc_core::print_heads`, outside the sim).
- Sorting is total. A plain `sort_unstable()` is fine, because equal elements
  are identical. A `_by`/`_by_key` sort on a key that can tie must be stable
  (`sort_by_key`). If the key is unique, expect the lint and say why.
- **Every cap has a visible result.**
  - A player's command that runs into a limit is refused with
    `SimEvent::CommandRefused` (`Refusal`), and the HUD says why. Examples are
    a patrol route that is too long, or a factory's standing orders being
    full.
  - A table that fills up returns `SimError::TableFull`.
  - Nothing truncates with `.take(N)` or skips with `None => {}` silently. A
    deliberate cap (AI budgets, cosmetic extras like sabot casings, debug
    tools) carries a comment that says so and why.
- **The determinism matrix [gate].** `mc-sim/tests/determinism.rs` plays one
  match with land, sea, subs, air, a titan and a nuke. The hash must be the
  same at 0, 1, 3 and 8 workers, and after a mid-match snapshot is restored.
  A new unit domain or system gets added to that match.
  `scripts/determinism-cross.sh` compares the final hash between the Windows
  and Linux builds. Run it after touching anything numeric in the sim.

## 4. One source of truth for every CPU-GPU contract

- Rust owns the ids, bits and array lengths the shaders use: part ids,
  materials, rig bits, `ModelInfo` flag words, puff/glow kinds, pass kinds and
  buffer array sizes. They live in `mc-models/src/gpu_consts.rs`, and
  `build.rs` generates them into every shader as `PREFIX_NAME` constants. Do
  not hand-copy a number into a shader, and do not write bare literals like
  `in.part == 16u` or `model.icon & 0x400000u`. Add the constant to
  `gpu_consts.rs` and use its generated name.
- **Every struct the CPU writes for the GPU is marked [gate].** Put
  `//!rust <path::to::RustType>` on the line above the WGSL struct.
  - `build.rs` then generates a test that the Rust type has the WGSL size, and
    that every WGSL member sits at the offset of the Rust field with the same
    name (`src/gpu_layout.rs`). So member names match between the two sides.
  - Only padding starts with `_`, and it never carries data. A packed word
    gets a real name.
  - A marked struct may be defined in one file only; the build fails if it is
    copied into a second shader. Shared structs live in a prelude
    (`common.wgsl`).
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

- **`unsafe` is denied workspace-wide [gate].** Only mc-render, mc-jobs,
  mc-music and mc-game opt out, with a crate-level `expect` naming why. Every
  `unsafe` block has a `// SAFETY:` comment saying why it holds here
  (`clippy::undocumented_unsafe_blocks`). A value sent across threads by hand
  goes through the `unsafe trait HandOff` in `loading.rs`, never a blanket
  `unsafe impl Send`.
- **Bytes from outside the process are untrusted [gate].** That covers the
  network, replays, snapshots, match options and map files.
  - Decode them with a size limit: `mc_sim::decode_untrusted`, or bincode
    `options().with_limit(..)`.
  - A restored `State` passes `State::validate` and `validate_ids` before it
    is used.
  - mc-net has no `unwrap`, `expect` or `panic!` outside tests.
  - `tests/untrusted_input.rs` feeds corrupted bytes in and must never panic.
- A panic anywhere in the game writes `crash-<time>.log` beside the settings
  file (`crash.rs`).
- Library crates return typed errors. `Result<_, String>` is for the binary's
  top level only.
- Every env switch and CLI flag is listed in one place: the `--help` text and
  `docs/SWITCHES.md`. A switch that is not listed does not get added.

## 8. Toolchain

- The toolchain is pinned in `rust-toolchain.toml`, so Windows and WSL build
  with the same compiler. Change the pin in a commit of its own.
- New dependencies go in `[workspace.dependencies]` and are used with
  `workspace = true`. Crates never use `path = "../..."` dependencies.

## 9. Building and showing models

- Look at a unit with `scripts/shot.sh unit KEY`: a six-angle sheet from a warm
  shot server, ~1-2 s. `--views front34,left`, `--look X,Y,Z --zoom N` for a
  close-up, `--frames N --turn DEG` for a turntable; `scripts/shot.sh --help`.
  Do not build into a `meridian-target-<topic>` dir of your own: `shot.sh`
  shares one incremental build and skips the game build when it can.
- The loop is fast when you stay on its fast paths: an edit under
  `crates/mc-models/src` reaches a picture in ~15 s, a `.wgsl` edit in ~6 s
  and a `data/` edit in ~5 s, none of them rebuilding the game. Only the unit
  being shot gets fresh meshes; look at the unit you changed.
- **New or reworked models are shown to the user as variants.** Write two or
  three real alternatives for each open design question as extra catalogue
  keys, `<mesh>~<name>` (`ModelDef::new("tank_light~slim", ...)`), and render
  them together: `scripts/shot.sh variants KEY=base,MESH~a,MESH~b ...` with
  every unit of the batch in one call. It writes one labelled sheet per unit
  (a row per variant) to `artifacts/shots/`; ten units with three variants
  each take about a minute. Send the sheets together, in one message, and ask
  for picks by letter (e.g. "tank B, scout A"), not one unit per round.
- Once a variant is chosen it becomes the mesh and the other `~` keys are
  deleted before the unit is committed (section 2: no parked code).

## 10. Every finished request leaves a check task for the user

- When a request is done (landed, or answered if it changed nothing in game),
  add a task to josh-os with `mcp__josh-os__create_task`: focus **Meridian
  RTS** (`trackId` `9307e1c1-9014-4c0b-a1b1-86deeb20e7f7`), `pile` `queue`,
  `process` `claude-code meridian`.
- The title says what to look at in game ("Check: Fulgur heat sinks aft of
  the turret"). The description says what changed and the commit. The steps
  say how to see it: the map or scene, the units to spawn, the keys to press,
  the camera angle, what right and wrong look like.
- One task per request, not per commit. A request that changed nothing you
  can see in game (a doc, a refactor, a tool) gets no task.

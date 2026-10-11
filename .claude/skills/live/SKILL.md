---
name: live
description: Work the test range the user is playing right now through the live desk (scripts/live.sh). Lay structures and prefabs (finished, as sites, or built by engineers in order), check fits, read back what they built or selected, save prefabs to their library, move their camera and screenshot their screen. Use when the user says "live", "in the range", "I'm in game", or asks to lay out, show, try or build a base, block or prefab, including AI base layouts.
---

# Live desk: working the user's running test range

The user is in the game, in the test range, watching. Everything here happens on
their screen as you do it. `scripts/live.sh --help` lists every request and field.
README "The live desk" has the overview.

## Start

1. `scripts/live.sh state`. If it says "no game is listening", ask the user to open
   the test range in a build from `dev` at cd212afa or later, then stop.
2. Read `state`: `map`, `pad`, `camera.at`, `cursor` (the ground under their
   pointer), `structures` (id, key, owner, x, y, turn, build, selected) and their
   `prefabs`.
3. Look up unit keys in `data/factions/<faction>/units/*.ron`; never guess a key.
   The factions are `aster` (ARC) and `regency`.

## Laying

- Coordinates are metres; x is east and y is north. A layout's middle snaps to the
  12 m build grid; members are offsets from it. `turn` is quarter turns
  anticlockwise from the structure's build heading.
- Where it goes (`at`):
  - `"cursor"` when the user points at a place ("here", "where my mouse is").
  - `"camera"` for where they are looking.
  - `[x, y]` to put it beside something else.
- Run `scripts/live.sh check` before laying anything new. Every site should say
  `"fits": "yes"`; fix the ones that do not before laying.
- How it comes to stand (`how`):
  - `"sites"` shows a plan without building it.
  - `"done"` shows the finished base.
  - `"build"` shows how it gets built. It spawns engineers (`builders`, default 1)
    40 m south of the layout, and they build it in the order of the list.
    `"free_build": true` stops it waiting on the economy.
- Put alternatives side by side, with gaps of 100 m or more, and name them A, B and
  C. Ask the user to pick by letter (CLAUDE.md section 9).

## Seeing what you did

- Take `scripts/live.sh shot -o NAME` after each change. It saves to
  `artifacts/live/NAME.png`; read it yourself, and send it with SendUserFile when
  the user is not at the screen.
- Use `scripts/live.sh camera X Y ZOOM` to frame what you laid first. A zoom of
  300 to 900 shows a base.

## Watching the AI grow a base

- `scripts/live.sh grow --at cursor --front pad --speed 800` hands red to the Commander
  AI (Hard) with its commander at `--at`; blue's start moves to `--front`, which the
  AI's base faces. `--free` stops it waiting on the economy. The player keeps blue.
- `scripts/live.sh ungrow` hands red back. Use `shot` to see how far it got.

## Taking the user's work back

- "This one", or what they selected: `scripts/live.sh capture` returns it as a
  prefab with members in build order. `capture ID...` captures given ids.
- When they like a layout, `scripts/live.sh keep FILE` saves it to their prefab
  library by name, so they can place it themselves with P. Ask before replacing a
  prefab they named.

## Do not

- Do not `clear` or `remove-area` without asking. The range may hold what they are
  testing. Use `remove ID...` for what you laid yourself, and keep track of the ids
  from `state`.
- Do not put AI prefab data in `data/` until the AI reads it (CLAUDE.md section 2:
  no parked data). Keep drafts in the scratchpad, or in the library with `keep`.
- Any refusal comes back as `{"ok": false, "error": ...}`. Read it and fix the
  request rather than retrying it unchanged.

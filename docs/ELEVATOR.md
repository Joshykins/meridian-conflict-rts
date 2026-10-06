# The space elevator

The Skyhook (`aster_t5_space_elevator`, ARC tier 5) is a factory and a mine at once: a
cable out of orbit, three clamp towers to hold it, and a cargo platform that rides down it
every half minute with whatever its side queued on it, free.

## How it runs

- Raised on a 26 x 26 lot by Mason IIIs or the commander's Engineering Suite III.
- Finished, it lowers its cable (`tether`, 24 s): the cable falls out of the sky into the
  well, and the three jaws swing in and close on it.
- From then on it runs in cycles of `period` (30 s): the platform comes down the cable
  (`descent`, 9 s, braking hard at the end), docks (`dock`, 9 s), goes back up (`ascent`,
  9 s), and is away for the rest. Sirens sound as each platform starts down.
- The tick the platform docks it lets off the front of the elevator's queue, free, as
  long as the next unit fits in what is left of the `payload` (20000 mass). The queue keeps
  its order: the first unit that does not fit, and everything behind it, waits for the
  next platform. Whatever of the payload is left goes into the side's store as mass (past
  a full store it is lost, like any other income).
- Its queue is a factory's: the same build menu, Repeat, rally point and standing orders.
  Nothing is ever built on a pad and nothing is paid for. Units on the ground drive off the
  deck between the clamp towers (at 60, 180 and 300 degrees off its heading); aircraft
  lift straight off.
- What it can bring down is its `builder.builds` list: every unit ARC's land and air
  factories make, and the T4 land experimentals. Ships and anything dearer than one
  payload are refused by the data check (`Blueprints::load`).

## Where it lives

| Piece | File |
| --- | --- |
| Data | `data/factions/aster/units/experimental.ron` (`elevator:`), `crates/mc-data/src/elevator.rs` |
| Sim | `crates/mc-sim/src/elevator.rs` (clock, delivery, `pose`), tests in `crates/mc-sim/tests/elevator.rs` |
| Mirror | `UnitInstance::mount` (cable drop, clamp grip) and `deploy` (platform height) |
| Model | `crates/mc-models/src/aster/elevator.rs`, contract in `gpu_consts::elevator` |
| Shader | `entity.wgsl`, the `ELEVATOR_PART_*` branch |
| Effects | `crates/mc-render/src/renderer/elevator_fx.rs` |
| Sound | `data/sounds/elevator.ron`, `crates/mc-game/src/audio/elevator.rs` |
| HUD | `crates/mc-game/src/hud/elevator.rs` |

See it on the test range: `--range --unit aster_t5_space_elevator --scenario elevator`.

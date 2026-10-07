# The space elevator

The Skyhook (`aster_t5_space_elevator`, ARC tier 5) is a factory and a mine at once: a
cable out of orbit, three clamp towers to hold it, and three cargo crates that fall down
it every half minute with whatever its side queued in them, free. The crates only ever
come down: they land, open, let their units drive out and sink into the ground.

## How it runs

- Raised on a 26 x 26 lot by Mason IIIs or the commander's Engineering Suite III.
- Finished, it lowers its cable (`tether`, 24 s): the cable falls out of the sky into the
  well, and the three jaws swing in and close on it. The cable never ends where the eye
  can see it: above the clouds it runs straight on up to 240 km (`gpu_consts::elevator::TOP_Z`),
  past the highest camera, and thin lamps climb it.
- From then on it runs in cycles of `period` (30 s): three crates fall down the cable
  (`descent`, 7 s, braking hard at the end) and land one in each way out between the
  towers, stand landed (`dock`, 18 s: the doors and roofs open over the first 4 s, then
  the units drive out), and sink into the ground (`sink`, 4 s). Sirens sound as each fall
  starts.
- The tick the crates touch down they set down the front of the elevator's queue, free,
  seated inside them (`elevator::seat_units`), as long as the next unit fits in what is
  left of the `payload` (20000 mass) and has a seat: a crate is 72 m wide and 100 m long
  and units are packed in rows from its doors inward, the first crate first. The queue
  keeps its order: the first unit that does not fit, and everything behind it, waits for
  the next fall. Whatever of the payload is left goes into the side's store as mass (past a
  full store it is lost, like any other income).
- Its queue is a factory's: the same build menu, Repeat, rally point and standing orders.
  Nothing is ever built on a pad and nothing is paid for.
- A crate's units are one group. They wait inside until the doors are open, then drive out
  row by row (a row every second, nearest the doors first), and once every one of them is
  out they leave together on the elevator's standing orders (a formation, as for a
  batch), else to its rally point, else a way out along the crate's bearing. Aircraft lift
  out through the open roof. Units that have not got out when the crates start to sink
  are put out at once. **Batch overrides this**: with the elevator in a batch (Shift+L),
  each unit goes to the batch's muster as it comes out instead.
- What it can bring down is its `builder.builds` list: every unit ARC's land and air
  factories make, and the T4 land experimentals. Ships, anything dearer than one payload
  and anything too wide for a crate (`mc_core::elevator_bay::seats`) are refused by the
  data check (`Blueprints::load`).
- The interface: the elevator's panel stands between its order card and its build menu,
  but the menu keeps its room first: on a narrow screen the panel shrinks to a compact
  one, then goes (`hud::elevator::width_for`); the queue strip over the menu always says
  what the crates are doing.

## Where it lives

| Piece | File |
| --- | --- |
| Data | `data/factions/aster/units/experimental.ron` (`elevator:`), `crates/mc-data/src/elevator.rs` |
| Sim | `crates/mc-sim/src/elevator.rs` (clock, delivery, `pose`), tests in `crates/mc-sim/tests/elevator.rs` |
| Mirror | `UnitInstance::mount` (cable drop, clamp grip) and `deploy` (crates' height: 1 in orbit, 0 landed, -0.5 open, -1 sunk) |
| Bays | `crates/mc-core/src/elevator_bay.rs`: the crates' size, shared by the sim's seating and the model |
| Model | `crates/mc-models/src/aster/elevator/`, contract in `gpu_consts::elevator` |
| Shader | `entity.wgsl`, the `ELEVATOR_PART_*` branch |
| Effects | `crates/mc-render/src/renderer/elevator_fx.rs` |
| Sound | `data/sounds/elevator.ron`, `crates/mc-game/src/audio/elevator.rs` |
| HUD | `crates/mc-game/src/hud/elevator.rs` |

See it on the test range: `--range --unit aster_t5_space_elevator --scenario elevator`.

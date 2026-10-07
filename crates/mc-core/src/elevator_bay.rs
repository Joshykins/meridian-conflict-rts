//! The Skyhook's cargo crates, in metres: three bays stand on the elevator's well,
//! one in each way out between the clamp towers, and the sim seats units in them
//! (`mc_sim::elevator::seat_units`) where the model draws the crates
//! (`mc-models`' `gpu_consts::elevator`, kept equal by a test there).
//!
//! A bay is a box on the bearing of its way out (60, 180 and 300 degrees off the
//! elevator's heading): `FROM` metres out from the cable's axis to its inner wall,
//! `LENGTH` along the bearing to its doors, `WIDTH` across and `HEIGHT` tall.

/// Bays on one elevator.
pub const COUNT: usize = 3;
/// Cable axis to the bay's inner wall.
pub const FROM: i32 = 32;
/// Inner wall to the doors.
pub const LENGTH: i32 = 100;
/// Between the side walls.
pub const WIDTH: i32 = 72;
/// Floor to roof.
pub const HEIGHT: i32 = 72;
/// Clear space kept to the walls and between units.
pub const MARGIN: i32 = 4;

/// How far the crates' height parameter (`UnitInstance::deploy`) goes below zero while
/// their doors and roofs open: -`DOORS_OPEN` is landed with everything open, and the
/// rest of the way to -1 is the crates sinking into the ground (1 and over: high in
/// orbit or away, not drawn).
pub const DOORS_OPEN: f32 = 0.5;

/// Whether a unit of `radius` (metres, as `Fx`) fits a bay at all.
pub fn seats(radius: crate::Fx) -> bool {
    let across = radius * 2 + crate::Fx::from_int(2 * MARGIN);
    across <= crate::Fx::from_int(WIDTH.min(LENGTH))
}

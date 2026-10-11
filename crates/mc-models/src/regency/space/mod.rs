//! Regency spacecraft: swept split hulls, layered charcoal plates and violet plasma.
//! Authored at blueprint scale; cargo apertures and weapon mounts share the data's metres.
pub(crate) mod ark;
mod body;
mod destroyer;
mod dress;
mod hull;
mod style;
mod suzerain;
#[cfg(test)]
mod tests;
mod transports;
mod vassal;

use crate::library::ModelDef;

pub(crate) use destroyer::{DRIVE_SIZE as DESTROYER_DRIVE, NOZZLES as DESTROYER_NOZZLES};

pub(crate) const MODELS: &[ModelDef] = &[
    ModelDef::new("regency_coffer", 44.0, 36.0, transports::light).with_ramp(-28.0, 1.0),
    ModelDef::new("regency_space_frigate", 56.0, 18.0, vassal::build),
    ModelDef::new("regency_space_cruiser", 96.0, 30.0, suzerain::build),
    ModelDef::new(
        "regency_space_destroyer",
        320.0,
        130.0,
        destroyer::destroyer,
    )
    .with_drives(
        DESTROYER_NOZZLES[0],
        DESTROYER_NOZZLES[2][1],
        DESTROYER_DRIVE,
    ),
];

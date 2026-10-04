//! Regency spacecraft: swept split hulls, layered charcoal plates and violet plasma.
//! Authored at blueprint scale; cargo apertures and weapon mounts share the data's metres.
mod hull;
#[cfg(test)]
mod tests;
mod transports;
mod warships;

use crate::library::ModelDef;

pub(crate) const MODELS: &[ModelDef] = &[
    ModelDef::new("regency_coffer", 44.0, 36.0, transports::light).with_ramp(-28.0, 1.0),
    ModelDef::new("regency_ark", 78.0, 48.0, transports::assault).with_ramp(-46.0, 1.0),
    ModelDef::new("regency_space_frigate", 48.0, 16.0, warships::frigate),
    ModelDef::new("regency_space_cruiser", 96.0, 30.0, warships::cruiser),
    ModelDef::new("regency_space_destroyer", 122.0, 48.0, warships::destroyer),
];

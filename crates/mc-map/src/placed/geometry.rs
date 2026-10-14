//! Shared authored collision geometry for simulation and presentation.
use super::Doodad;
use crate::Prop;
use mc_core::{Fx, FxVec2};
pub struct PropPart {
    pub centre: FxVec2,
    pub half: FxVec2,
    pub top: Fx,
}
pub fn prop_parts(prop: &Prop, component: Option<&Doodad>) -> Vec<PropPart> {
    if let Some(f) = component.and_then(|d| d.footprint.as_ref()) {
        return vec![PropPart {
            centre: FxVec2::ZERO,
            half: f.half,
            top: f.height,
        }];
    }
    crate::city::structure(prop.kind).map_or_else(Vec::new, |kind| {
        kind.plan
            .iter()
            .zip(kind.tops)
            .map(|(&(x, y, hx, hy), &top)| PropPart {
                centre: FxVec2::from_ints(x, y),
                half: FxVec2::from_ints(hx, hy),
                top: Fx::from_int(top),
            })
            .collect()
    })
}

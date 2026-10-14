//! Authored prop components: deterministic collision and durability overrides.
use mc_core::{Fx, FxVec2};
use mc_map::{
    placed::{prop_parts as parts, Doodad},
    Prop,
};
use std::collections::BTreeMap;
pub(crate) type Components = BTreeMap<u32, Doodad>;

pub(crate) fn validate(props: &[Prop], components: &Components) -> Result<(), crate::SimError> {
    for (&index, d) in components {
        let p = props
            .get(index as usize)
            .ok_or_else(|| crate::SimError::Setup("doodad references a missing prop".into()))?;
        if index != d.prop_index || d.health > 1_000_000 || d.reclaim > 1_000_000 {
            return Err(crate::SimError::Setup("invalid doodad component".into()));
        }
        if let Some(f) = &d.footprint {
            if f.half.x <= Fx::ZERO
                || f.half.y <= Fx::ZERO
                || f.height <= Fx::ZERO
                || f.half.x > Fx::from_int(256)
                || f.half.y > Fx::from_int(256)
                || f.height > Fx::from_int(1024)
            {
                return Err(crate::SimError::Setup("invalid doodad footprint".into()));
            }
        }
        let needs_shape = d.health > 0 || d.reclaim > 0 || d.blocking;
        if needs_shape && parts(p, Some(d)).is_empty() {
            return Err(crate::SimError::Setup(format!(
                "doodad {} needs a footprint for its components",
                d.name
            )));
        }
    }
    Ok(())
}
/// Explicit footprints block cells touched by their rotated box. The map bounds
/// cap work even for a component whose prop is outside the playable edge.
pub(crate) fn cells(prop: &Prop, d: &Doodad, size: FxVec2) -> Option<Vec<crate::lots::CellRect>> {
    if !d.blocking {
        return Some(Vec::new());
    }
    let f = d.footprint.as_ref()?;
    let scale = Fx::milli(i64::from(prop.scale_milli));
    let half = f.half * scale;
    let cell = mc_map::CELL_SIZE_M;
    let radius = half.x + half.y + Fx::from_int(cell);
    let clamp = |value: Fx, size: Fx| {
        (value.floor_int().div_euclid(cell)).clamp(0, (size.floor_int() / cell - 1).max(0)) as u32
    };
    let (x0, x1) = (
        clamp(prop.pos.x - radius, size.x),
        clamp(prop.pos.x + radius, size.x),
    );
    let (y0, y1) = (
        clamp(prop.pos.y - radius, size.y),
        clamp(prop.pos.y + radius, size.y),
    );
    let mut out = Vec::new();
    let pad = Fx::from_int(cell) / 2;
    for y in y0..=y1 {
        let mut start = None;
        for x in x0..=x1 {
            let centre = FxVec2::from_ints(x as i32 * cell + cell / 2, y as i32 * cell + cell / 2);
            let local = (centre - prop.pos).rotate(-prop.heading);
            let inside = local.x.abs() <= half.x + pad && local.y.abs() <= half.y + pad;
            if inside {
                start.get_or_insert(x);
            }
            if !inside || x == x1 {
                if let Some(a) = start.take() {
                    let b = if inside { x } else { x - 1 };
                    out.push(((a, y), (b, y)));
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;

impl crate::World {
    /// Decorative trees may be cleared instantly. Authored props instead obey
    /// their health and blocking components through the structure damage path.
    pub(crate) fn is_scenery_tree(&self, prop: usize) -> bool {
        self.map.props[prop].kind.is_tree() && !self.map.doodads.contains_key(&(prop as u32))
    }
}

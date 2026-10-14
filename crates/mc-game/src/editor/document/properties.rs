use super::*;
pub(super) fn apply(
    placed: &mut Placed,
    id: u64,
    name: &str,
    group: Option<&str>,
    owner: Option<u8>,
    blueprint: Option<&str>,
    blueprints: &mc_data::Blueprints,
) -> Result<(), String> {
    if let Some(unit) = placed.units.iter_mut().find(|u| u.id == id) {
        if let Some(key) = blueprint {
            if blueprints.unit_by_key(key).is_none() {
                return Err(format!("Unknown unit: {key}"));
            }
            unit.blueprint = key.into();
        }
        unit.name = name.into();
        if let Some(group) = group {
            unit.group = group.into();
        }
        if let Some(owner) = owner {
            unit.owner = owner;
        }
        return Ok(());
    }
    if group.is_some() || owner.is_some() || blueprint.is_some() {
        return Err("Only units have blueprint, group and player properties".into());
    }
    let named = placed
        .regions
        .iter_mut()
        .map(|o| (o.id, &mut o.name))
        .chain(placed.points.iter_mut().map(|o| (o.id, &mut o.name)))
        .chain(placed.cameras.iter_mut().map(|o| (o.id, &mut o.name)))
        .chain(placed.doodads.iter_mut().map(|o| (o.id, &mut o.name)))
        .find(|(key, _)| *key == id);
    let Some((_, target)) = named else {
        return Err("Selected object no longer exists".into());
    };
    *target = name.into();
    Ok(())
}

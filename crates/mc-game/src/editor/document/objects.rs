use super::*;
pub(super) fn apply(
    command: &Command,
    placed: &mut Placed,
    blueprints: &mc_data::Blueprints,
) -> Result<(), String> {
    match command {
        Command::Unit {
            name,
            group,
            blueprint,
            owner,
            at,
            heading,
        } => {
            if blueprints.unit_by_key(blueprint).is_none() {
                return Err(format!("Unknown unit: {blueprint}"));
            }
            if !heading.is_finite() {
                return Err("Heading must be finite".into());
            }
            let at = point(*at)?;
            let heading = Angle::from_radians_f32(heading.to_radians());
            if let Some(u) = placed.units.iter_mut().find(|u| u.name == *name) {
                u.group = group.clone();
                u.blueprint = blueprint.clone();
                u.owner = *owner;
                u.at = at;
                u.heading = heading;
            } else {
                placed.units.push(PlacedUnit {
                    id: placed.next_id,
                    name: name.clone(),
                    group: group.clone(),
                    blueprint: blueprint.clone(),
                    owner: *owner,
                    at,
                    heading,
                });
                placed.next_id += 1;
            }
        }
        Command::Region { name, min, max } => {
            let (min, max) = (point(*min)?, point(*max)?);
            if let Some(region) = placed.regions.iter_mut().find(|r| r.name == *name) {
                region.min = min;
                region.max = max;
            } else {
                placed.regions.push(Region {
                    id: placed.next_id,
                    name: name.clone(),
                    min,
                    max,
                });
                placed.next_id += 1;
            }
        }
        Command::Point { name, at } => {
            let at = point(*at)?;
            if let Some(p) = placed.points.iter_mut().find(|p| p.name == *name) {
                p.at = at;
            } else {
                placed.points.push(mc_map::placed::Point {
                    id: placed.next_id,
                    name: name.clone(),
                    at,
                });
                placed.next_id += 1;
            }
        }
        Command::Camera {
            name,
            at,
            distance,
            yaw,
        } => {
            if !yaw.is_finite() {
                return Err("Camera yaw must be finite".into());
            }
            let at = point(*at)?;
            let yaw = Angle::from_radians_f32(yaw.to_radians());
            if let Some(c) = placed.cameras.iter_mut().find(|c| c.name == *name) {
                c.at = at;
                c.distance = *distance;
                c.yaw = yaw;
            } else {
                placed.cameras.push(mc_map::placed::Camera {
                    id: placed.next_id,
                    name: name.clone(),
                    at,
                    distance: *distance,
                    yaw,
                });
                placed.next_id += 1;
            }
        }
        Command::Transform { id, at, heading } => {
            let at = point(*at)?;
            if !heading.is_finite() {
                return Err("Heading must be finite".into());
            }
            if let Some(u) = placed.units.iter_mut().find(|u| u.id == *id) {
                u.at = at;
                u.heading = Angle::from_radians_f32(heading.to_radians());
            } else if let Some(p) = placed.points.iter_mut().find(|p| p.id == *id) {
                p.at = at;
            } else if let Some(c) = placed.cameras.iter_mut().find(|c| c.id == *id) {
                c.at = at;
                c.yaw = Angle::from_radians_f32(heading.to_radians());
            } else if let Some(r) = placed.regions.iter_mut().find(|r| r.id == *id) {
                let half = (r.max - r.min) * Fx::HALF;
                r.min = at - half;
                r.max = at + half;
            } else {
                return Err("Select a unit, point, camera or region to transform".into());
            }
        }
        Command::Properties {
            id,
            name,
            group,
            owner,
            blueprint,
        } => properties::apply(
            placed,
            *id,
            name,
            group.as_deref(),
            *owner,
            blueprint.as_deref(),
            blueprints,
        )?,
        Command::Delete { id } => {
            placed.units.retain(|u| u.id != *id);
            placed.regions.retain(|r| r.id != *id);
            placed.points.retain(|p| p.id != *id);
            placed.cameras.retain(|c| c.id != *id);
            placed.doodads.retain(|d| d.id != *id);
        }
        _ => return Err("Expected an object command".into()),
    }
    Ok(())
}

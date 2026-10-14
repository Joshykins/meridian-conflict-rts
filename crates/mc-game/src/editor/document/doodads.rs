//! Prop table transactions preserve authored identities through deletion and save sorting.
use super::*;
use mc_map::{
    placed::{Doodad, Footprint},
    Prop, PropKind,
};
#[derive(Clone)]
pub(super) struct State {
    pub props: Vec<Prop>,
    tints: Vec<[u8; 3]>,
}
impl State {
    pub(super) fn from_map(map: &EditMap) -> Self {
        Self {
            props: map.props.clone(),
            tints: map.tints.clone(),
        }
    }
    pub(super) fn matches(&self, map: &EditMap) -> bool {
        self.props == map.props && self.tints == map.tints
    }
    pub(super) fn restore(&self, map: &mut EditMap) {
        map.props.clone_from(&self.props);
        map.tints.clone_from(&self.tints);
    }
}
pub(super) fn apply(
    command: &Command,
    map: &EditMap,
    state: &mut Option<State>,
    placed: &mut Placed,
) -> Result<bool, String> {
    match command {
        Command::Doodad { .. } => {
            upsert(
                command,
                map,
                state.get_or_insert_with(|| State::from_map(map)),
                placed,
            )?;
        }
        Command::Transform { id, at, heading } => {
            let Some(d) = placed.doodads.iter().find(|d| d.id == *id) else {
                return Ok(false);
            };
            if !heading.is_finite() {
                return Err("Heading must be finite".into());
            }
            let at = point(*at)?;
            inside(at, map)?;
            let p =
                &mut state.get_or_insert_with(|| State::from_map(map)).props[d.prop_index as usize];
            p.pos = at;
            p.heading = Angle::from_radians_f32(heading.to_radians());
        }
        Command::Scale { id, factor } => {
            let Some(d) = placed.doodads.iter().find(|d| d.id == *id) else {
                return Ok(false);
            };
            let props = state.get_or_insert_with(|| State::from_map(map));
            let p = &mut props.props[d.prop_index as usize];
            let scale = f32::from(p.scale_milli) * factor;
            if !factor.is_finite() || *factor <= 0.0 || !(50.0..=16_000.0).contains(&scale) {
                return Err("Resulting doodad scale must be 0.05 to 16".into());
            }
            p.scale_milli = scale.round() as u16;
        }
        Command::Delete { id } => {
            let Some(index) = placed.doodads.iter().position(|d| d.id == *id) else {
                return Ok(false);
            };
            let prop = placed.doodads.remove(index).prop_index;
            let state = state.get_or_insert_with(|| State::from_map(map));
            state.props.remove(prop as usize);
            if !state.tints.is_empty() {
                state.tints.remove(prop as usize);
            }
            for d in &mut placed.doodads {
                if d.prop_index > prop {
                    d.prop_index -= 1;
                }
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}
fn inside(at: FxVec2, map: &EditMap) -> Result<(), String> {
    let size = map.info.size_metres();
    if at.x < Fx::ZERO || at.y < Fx::ZERO || at.x > size.x || at.y > size.y {
        return Err("Doodad lies outside the map".into());
    }
    Ok(())
}
fn upsert(
    command: &Command,
    map: &EditMap,
    state: &mut State,
    placed: &mut Placed,
) -> Result<(), String> {
    let Command::Doodad {
        name,
        kind,
        at,
        heading,
        scale,
        tint,
        blocking,
        health,
        reclaim,
        footprint,
    } = command
    else {
        return Err("Expected doodad".into());
    };
    let kind = *PropKind::ALL
        .iter()
        .find(|k| format!("{k:?}").eq_ignore_ascii_case(kind))
        .ok_or("Unknown prop kind; use the doodad catalogue")?;
    if !heading.is_finite() || !scale.is_finite() || !(0.05..=16.0).contains(scale) {
        return Err("Doodad scale must be 0.05 to 16 and heading finite".into());
    }
    let at = point(*at)?;
    inside(at, map)?;
    let footprint=footprint.map(|f|{
        if f.iter().any(|v|!v.is_finite() || *v<=0.0) || f[0]>256.0 || f[1]>256.0 || f[2]>1024.0{return Err("Footprint needs positive half-width, half-depth and height within 256/256/1024 m".to_owned());}
        Ok(Footprint{half:FxVec2::new(Fx::from_f32(f[0]),Fx::from_f32(f[1])),height:Fx::from_f32(f[2])})
    }).transpose()?;
    let prop = Prop {
        kind,
        pos: at,
        heading: Angle::from_radians_f32(heading.to_radians()),
        scale_milli: (scale * 1000.0).round() as u16,
        wear_milli: 0,
    };
    let existing = placed.doodads.iter().position(|d| d.name == *name);
    let prop_index = existing.map_or(state.props.len() as u32, |i| placed.doodads[i].prop_index);
    let component = Doodad {
        id: existing.map_or(placed.next_id, |i| placed.doodads[i].id),
        name: name.clone(),
        prop_index,
        tint: *tint,
        blocking: *blocking,
        health: *health,
        reclaim: *reclaim,
        footprint,
    };
    if (component.blocking || component.health > 0 || component.reclaim > 0)
        && mc_map::placed::prop_parts(&prop, Some(&component)).is_empty()
    {
        return Err("This prop needs a custom footprint for collision, health or reclaim".into());
    }
    if let Some(i) = existing {
        state.props[prop_index as usize] = prop;
        placed.doodads[i] = component;
    } else {
        state.props.push(prop);
        if !state.tints.is_empty() {
            state.tints.push(*tint);
        }
        placed.doodads.push(component);
        placed.next_id += 1;
    }
    if !state.tints.is_empty() {
        state.tints[prop_index as usize] = *tint;
    }
    Ok(())
}

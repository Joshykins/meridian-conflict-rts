use super::*;
impl Mission {
    pub(super) fn spawn(&mut self, group: &str, world: &mut World) -> Result<(), SimError> {
        if self.groups.contains_key(group) {
            return Err(SimError::Setup(format!(
                "mission group {group} was spawned twice"
            )));
        }
        let mut ids = vec![];
        for u in self
            .definition
            .placed
            .units
            .iter()
            .filter(|u| u.group == group)
        {
            let bp = world.blueprints.id_of(&u.blueprint).ok_or_else(|| {
                SimError::Setup(format!("missing mission blueprint {}", u.blueprint))
            })?;
            let row = world.spawn_unit(bp, u.owner, u.at, u.heading, true)?;
            let id = world.state.units.slots.handle(row);
            self.units.insert(u.name.clone(), id);
            ids.push(id);
        }
        self.groups.insert(group.to_owned(), ids);

        Ok(())
    }
    pub(super) fn move_group(
        &self,
        group: &str,
        point: &str,
        attack: bool,
        world: &mut World,
    ) -> Result<(), SimError> {
        let pos = self
            .definition
            .placed
            .points
            .iter()
            .find(|p| p.name == point)
            .ok_or_else(|| SimError::Setup(format!("missing mission point {point}")))?
            .at;
        let ids = self.groups.get(group).cloned().unwrap_or_default();
        let mut by_owner: BTreeMap<u8, Vec<UnitId>> = BTreeMap::new();
        for id in ids {
            if let Some(row) = world.state.units.row(id) {
                by_owner
                    .entry(world.state.units.owner[row])
                    .or_default()
                    .push(id);
            }
        }
        for (owner, units) in by_owner {
            let command = if attack {
                Command::AttackMove {
                    units,
                    target: pos,
                    queue: false,
                }
            } else {
                Command::Move {
                    units,
                    target: pos,
                    queue: false,
                }
            };
            world.apply_as(owner, &command)?;
        }

        Ok(())
    }
    /// A ship already travelling from outside the battle arrives through the
    /// normal transit/emergence simulation, including its effects and sounds.
    pub(super) fn arrive(&self, unit: &str, world: &mut World) -> Result<(), SimError> {
        let row = self.live_row(unit, world)?;
        if world.bp(row).warp.is_none() {
            return Err(SimError::Setup(
                "Authored arrival requires a warp drive".into(),
            ));
        }
        let units = &mut world.state.units;
        units.flags[row] |= crate::tables::flag::IN_FACTORY;
        units.warp[row] = crate::warp::WarpState {
            phase: crate::warp::WarpPhase::Transit,
            length: 25,
            from: units.pos[row],
            to: units.pos[row],
            ..Default::default()
        };
        Ok(())
    }
}

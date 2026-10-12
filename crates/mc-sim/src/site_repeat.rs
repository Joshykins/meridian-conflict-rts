//! Repeat on a unit raised on a lot of its own (`is_site_built_unit`, an experimental
//! too big for any factory): the half-built site carries `flag::REPEAT`
//! (`Command::SetRepeat`), and once it is finished and has driven off, the lot is laid
//! again with the same unit. The builders that had nothing else queued take up the new
//! site; with none, it waits there with no builders on it. The new site carries Repeat
//! on, so it goes round until the player turns it off, takes the site apart, or an
//! enemy kills it: a site that dies takes its Repeat with it.

use mc_core::{Angle, FxVec2, StateHasher, TICKS_PER_SECOND};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

use crate::mirror::Refusal;
use crate::orders::order;
use crate::tables::*;
use crate::{SimError, World};

/// How long a finished unit's lot may stay blocked (its own side's units are sent off
/// it, an enemy's are not) before the repeat is given up and the player told.
const MOST_WAIT: u16 = 30 * TICKS_PER_SECOND as u16;

/// A finished unit's lot waiting to be laid again (`State::site_repeats`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SiteRepeat {
    pub owner: u8,
    pub blueprint: BlueprintId,
    pub pos: FxVec2,
    pub heading: Angle,
    /// The finished unit's priority in a stall, kept on the next.
    pub priority: crate::focus::Priority,
    /// Builders that had only this unit queued: they take up the new site.
    pub builders: Vec<UnitId>,
    /// Ticks the lot has been blocked.
    pub waited: u16,
}

/// Every lot waiting to be laid again, in the order they were finished.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SiteRepeats {
    pub waiting: Vec<SiteRepeat>,
}

impl SiteRepeats {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.waiting.len() as u64);
        for r in &self.waiting {
            h.write_u64(
                r.owner as u64
                    | (r.blueprint.0 as u64) << 8
                    | (r.heading.0 as u64) << 24
                    | (r.priority as u64) << 40
                    | (r.waited as u64) << 48,
            );
            h.write_i64(r.pos.x.0);
            h.write_i64(r.pos.y.0);
            h.write_u64(r.builders.len() as u64);
            for b in &r.builders {
                h.write_u64(b.0 as u64);
            }
        }
    }
}

impl World {
    /// `ids`' own half-built units on lots of their own: the ones Repeat lays again.
    pub(crate) fn owned_repeat_sites(&self, player: u8, ids: &[UnitId]) -> Vec<usize> {
        let units = &self.state.units;
        ids.iter()
            .filter_map(|id| units.row(*id))
            .filter(|&row| {
                units.owner[row] == player
                    && units.flags[row] & (flag::UNDER_CONSTRUCTION | flag::IN_FACTORY)
                        == flag::UNDER_CONSTRUCTION
                    && self.bp(row).is_site_built_unit()
            })
            .collect()
    }

    /// `row`, a unit raised on its own lot, is finished with Repeat on: the switch
    /// passes from it to the lot, which waits to be laid again (`run_site_repeats`).
    pub(crate) fn queue_site_repeat(&mut self, row: usize, pos: FxVec2, heading: Angle) {
        self.state.units.flags[row] &= !flag::REPEAT;
        let id = self.state.units.id(row);
        let owner = self.state.units.owner[row];
        let blueprint = self.state.units.blueprint[row];
        let units = &self.state.units;
        let orders = &self.state.orders;
        // Its builders whose whole queue is this unit: building it, or helping raise it.
        let builders = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == owner && r != row)
            .filter(|&r| {
                let mut queue = orders.iter(units, r);
                let only = queue.next().is_some_and(|o| match o.kind {
                    OrderKind::Build => {
                        o.target == id || (o.pos == pos && o.blueprint == blueprint)
                    }
                    OrderKind::Assist => o.target == id,
                    _ => false,
                });
                only && queue.next().is_none()
            })
            .map(|r| units.id(r))
            .collect();
        self.state.site_repeats.waiting.push(SiteRepeat {
            owner,
            blueprint,
            pos,
            heading,
            priority: units.priority[row],
            builders,
            waited: 0,
        });
    }

    /// Lays each waiting lot again once the finished unit (and anything else of its
    /// side) is off it, and sets its builders to work on it.
    pub(crate) fn run_site_repeats(&mut self) -> Result<(), SimError> {
        if self.state.site_repeats.waiting.is_empty() {
            return Ok(());
        }
        let waiting = std::mem::take(&mut self.state.site_repeats.waiting);
        let mut kept = Vec::with_capacity(waiting.len());
        for mut r in waiting {
            if self.state.players[r.owner as usize].defeated {
                continue;
            }
            let bp = self.blueprints.unit(r.blueprint).clone();
            // Built over (or otherwise shut) since: no lot to lay again.
            if !self.can_place_turned(&bp, r.pos, r.heading) || r.waited >= MOST_WAIT {
                self.refuse(r.owner, Refusal::RepeatBlocked);
                continue;
            }
            let lot = bp.footprint_at(r.heading);
            if self.mobile_units_in_footprint(lot, r.pos, usize::MAX, r.blueprint) {
                self.send_off_site(lot, r.pos, r.owner, usize::MAX, r.blueprint)?;
                r.waited += 1;
                kept.push(r);
                continue;
            }
            let site = self.spawn_unit(r.blueprint, r.owner, r.pos, r.heading, false)?;
            self.state.units.flags[site] |= flag::REPEAT;
            self.state.units.priority[site] = r.priority;
            for b in &r.builders {
                // Only those still idle: a builder given other work since keeps it.
                let Some(row) = self.state.units.row(*b) else {
                    continue;
                };
                if self.state.units.order_head[row] != NO_ORDER {
                    continue;
                }
                // It walks back to the lot and joins the site there (`run_build`).
                let mut o = order(OrderKind::Build, r.pos, Handle::NONE);
                o.blueprint = r.blueprint;
                o.heading = r.heading;
                self.state.orders.push_back(&mut self.state.units, row, o)?;
            }
        }
        self.state.site_repeats.waiting = kept;
        Ok(())
    }
}

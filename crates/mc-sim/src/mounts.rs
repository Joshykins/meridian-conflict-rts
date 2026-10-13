//! Weapon groups' guns (`mc_data::mounts`): one weapon of a unit file that is many houses on
//! the hull, each with its own aim, mark, reload and salvo.
//!
//! State: `Units::mounts`, one [`MountState`] per gun slot of a unit that has a group
//! (`UnitBlueprint::gun_slot`), empty for every other unit; an ordinary weapon's slot in it
//! is never read. Hashed and validated only for the rows that have any.
//!
//! Targeting (`World::mount_picks`, called from `run_targeting` past its hold-fire and stun
//! gate) searches once per group round the hull, out to the group's reach plus its farthest
//! pivot, and gives each gun the nearest mark it may shoot from its own pivot.
//!
//! Firing (`World::step_group`, called from `run_weapons` past the same gate) steps each
//! gun through the one gun step every weapon takes (`combat/step.rs`): the gun's state is
//! laid into its weapon's slot for the step and taken back after, so a group's gun turns,
//! charges and fires as any house does. The slot is then left holding the group's summary
//! (the first gun's mark, the soonest reload), which is what line of fire, the AI, the
//! HUD and the orders read of the weapon.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{Blueprints, UnitBlueprint, Weapon};
use serde::{Deserialize, Serialize};

use crate::combat::{takes_ground, target_kinds};
use crate::tables::{Handle, UnitId, Units};
use crate::{SimError, State, World};

/// One gun of a group: what `Units` keeps per weapon slot for an ordinary weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountState {
    /// Ticks to its next shot.
    pub cooldown: u16,
    pub salvo_left: u8,
    /// The ground stands between it and its mark (`line_of_fire.rs`).
    pub blocked: bool,
    /// Its house's yaw off the nose, and its pitch, this tick and the last.
    pub yaw: Angle,
    pub prev_yaw: Angle,
    pub pitch: Angle,
    pub prev_pitch: Angle,
    pub target: UnitId,
    /// A held beam: where its stream lands, and whether it is streaming (`Units::beam_at`).
    pub beam_at: FxVec2,
    pub streaming: bool,
    /// A gun that spins up first: its spin, as `Units::spin` keeps it.
    pub spin: [u16; 4],
    /// `Bombard`: the point it lays on (`Units::ground_aim`).
    pub ground_aim: FxVec2,
}

impl MountState {
    /// A gun at rest, turned to face where `gun` rests.
    fn at_rest(gun: &Weapon) -> Self {
        Self {
            cooldown: 0,
            salvo_left: 0,
            blocked: false,
            yaw: gun.facing,
            prev_yaw: gun.facing,
            pitch: Angle::ZERO,
            prev_pitch: Angle::ZERO,
            target: Handle::NONE,
            beam_at: FxVec2::ZERO,
            streaming: false,
            spin: [0; 4],
            // Far from any map, as `Units::spawn` lays it: the first bombardment picks a point.
            ground_aim: FxVec2::from_ints(-1_000_000, -1_000_000),
        }
    }

    fn hash(&self, h: &mut mc_core::StateHasher) {
        h.write_u64(
            self.cooldown as u64
                | (self.salvo_left as u64) << 16
                | (self.blocked as u64) << 24
                | (self.streaming as u64) << 25
                | (self.yaw.0 as u64) << 32
                | (self.pitch.0 as u64) << 48,
        );
        h.write_u64(self.target.0 as u64);
        for v in [
            self.beam_at.x,
            self.beam_at.y,
            self.ground_aim.x,
            self.ground_aim.y,
        ] {
            h.write_i64(v.0);
        }
        h.write_u64(self.spin.iter().fold(0u64, |acc, &n| acc << 16 | n as u64));
    }
}

/// What a gun's step reads and writes of its weapon's slot, and of the row's own stream
/// and spin, which a group's guns each keep for themselves.
#[derive(Clone, Copy)]
struct Slot {
    cooldown: u16,
    salvo_left: u8,
    blocked: bool,
    yaw: Angle,
    pitch: Angle,
    target: UnitId,
    beam_at: FxVec2,
    streaming: bool,
    spin: [u16; 4],
    ground_aim: FxVec2,
}

impl Slot {
    fn take(u: &Units, row: usize, w: usize, rotary: usize) -> Self {
        Self {
            cooldown: u.weapon_cooldown[row][w],
            salvo_left: u.weapon_salvo_left[row][w],
            blocked: u.shot_blocked[row] & 1 << w != 0,
            yaw: u.weapon_yaw[row][w],
            pitch: u.arm_pitch[row][2 + w],
            target: u.weapon_target[row][w],
            beam_at: u.beam_at[row],
            streaming: u.streaming[row],
            spin: u.spin[row][rotary],
            ground_aim: u.ground_aim[row][w],
        }
    }

    fn put(self, u: &mut Units, row: usize, w: usize, rotary: usize) {
        u.weapon_cooldown[row][w] = self.cooldown;
        u.weapon_salvo_left[row][w] = self.salvo_left;
        u.shot_blocked[row] = u.shot_blocked[row] & !(1 << w) | (self.blocked as u16) << w;
        u.weapon_yaw[row][w] = self.yaw;
        u.arm_pitch[row][2 + w] = self.pitch;
        u.weapon_target[row][w] = self.target;
        u.beam_at[row] = self.beam_at;
        u.streaming[row] = self.streaming;
        u.spin[row][rotary] = self.spin;
        u.ground_aim[row][w] = self.ground_aim;
    }

    fn of(m: &MountState) -> Self {
        Self {
            cooldown: m.cooldown,
            salvo_left: m.salvo_left,
            blocked: m.blocked,
            yaw: m.yaw,
            pitch: m.pitch,
            target: m.target,
            beam_at: m.beam_at,
            streaming: m.streaming,
            spin: m.spin,
            ground_aim: m.ground_aim,
        }
    }

    fn store(self, m: &mut MountState) {
        m.cooldown = self.cooldown;
        m.salvo_left = self.salvo_left;
        m.blocked = self.blocked;
        m.yaw = self.yaw;
        m.pitch = self.pitch;
        m.target = self.target;
        m.beam_at = self.beam_at;
        m.streaming = self.streaming;
        m.spin = self.spin;
        m.ground_aim = self.ground_aim;
    }
}

/// A group's gun's choice this tick (`World::mount_picks`): its gun slot, its mark, and
/// whether the ground hides that mark from it.
pub(crate) type MountPick = (u16, UnitId, bool);

/// The guns `bp` keeps state for: one per gun slot when it has a group, else none.
fn mounts_of(bp: &UnitBlueprint) -> Vec<MountState> {
    if !bp.weapons.iter().any(Weapon::is_group) {
        return Vec::new();
    }
    bp.weapons
        .iter()
        .flat_map(|w| (0..w.guns()).map(move |m| MountState::at_rest(w.gun(m))))
        .collect()
}

impl Units {
    /// Hashes `row`'s guns: nothing for a unit without a group.
    pub(crate) fn hash_mounts(&self, row: usize, h: &mut mc_core::StateHasher) {
        let mounts = &self.mounts[row];
        if mounts.is_empty() {
            return;
        }
        h.write_u64(mounts.len() as u64);
        for m in mounts {
            m.hash(h);
        }
    }

    /// Every gun's last-tick aim taken from this tick's, as the tick begins.
    pub(crate) fn begin_mounts_tick(&mut self) {
        for mounts in self.mounts.iter_mut().filter(|m| !m.is_empty()) {
            for m in mounts {
                m.prev_yaw = m.yaw;
                m.prev_pitch = m.pitch;
            }
        }
    }
}

impl State {
    /// Every live unit keeps exactly the guns its blueprint has (`mounts_of`): a snapshot
    /// is from outside the process, and the weapons phase indexes them by gun slot.
    /// Call after `validate_ids`.
    pub fn validate_mounts(&self, blueprints: &Blueprints) -> Result<(), String> {
        let units = &self.units;
        for row in units.slots.iter() {
            let bp = blueprints.unit(units.blueprint[row]);
            let want = if bp.weapons.iter().any(Weapon::is_group) {
                bp.gun_count()
            } else {
                0
            };
            if units.mounts[row].len() != want {
                return Err(format!(
                    "units: row {row} keeps {} guns, its blueprint has {want}",
                    units.mounts[row].len()
                ));
            }
        }
        Ok(())
    }
}

impl World {
    /// Gives `row` the guns its blueprint has, at rest: on spawn, and when an upgrade or a
    /// refit changes its blueprint.
    pub(crate) fn fit_mounts(&mut self, row: usize) {
        let mounts = mounts_of(self.bp(row));
        let have = &self.state.units.mounts[row];
        if have.len() != mounts.len() {
            self.state.units.mounts[row] = mounts;
        }
    }

    /// One tick of every gun of group `w` of `row`, in mount order, each through the gun
    /// step every weapon takes (`step_gun`). The row's own stream and spin are kept, and
    /// the weapon's slot is left holding the group's summary.
    pub(crate) fn step_group(&mut self, row: usize, w: usize) -> Result<(), SimError> {
        let bp = self.blueprints.clone();
        let bp = bp.unit(self.state.units.blueprint[row]);
        let weapon = &bp.weapons[w];
        let (first, n, rotary) = (bp.gun_slot(w), weapon.mounts.len(), weapon.rotary as usize);
        let own = Slot::take(&self.state.units, row, w, rotary);
        let mut done = Ok(());
        for m in 0..n {
            Slot::of(&self.state.units.mounts[row][first + m]).put(
                &mut self.state.units,
                row,
                w,
                rotary,
            );
            done = self.step_gun(row, w, Some(m));
            let after = Slot::take(&self.state.units, row, w, rotary);
            after.store(&mut self.state.units.mounts[row][first + m]);
            if done.is_err() {
                break;
            }
        }
        let summary = self.group_summary(row, first, n, own);
        summary.put(&mut self.state.units, row, w, rotary);
        done
    }

    /// The weapon slot of a group: the row's own stream and spin (`own`), the first gun's
    /// aim, the first gun with a mark's mark, the soonest reload and the longest salvo.
    fn group_summary(&self, row: usize, first: usize, n: usize, own: Slot) -> Slot {
        let units = &self.state.units;
        let guns = &units.mounts[row][first..first + n];
        let lead = guns
            .iter()
            .find(|m| units.row(m.target).is_some())
            .unwrap_or(&guns[0]);
        Slot {
            cooldown: guns.iter().map(|m| m.cooldown).min().unwrap_or(0),
            salvo_left: guns.iter().map(|m| m.salvo_left).max().unwrap_or(0),
            blocked: lead.blocked,
            yaw: guns[0].yaw,
            pitch: guns[0].pitch,
            target: lead.target,
            ground_aim: guns[0].ground_aim,
            ..own
        }
    }

    /// The mark each gun of every group of `row` lays on this tick, called from
    /// `run_targeting` past its hold-fire and stun gate: `ordered` is the unit its orders
    /// name, `ground` whether they name a point on the ground (`strike`: a storm's).
    /// `due` is the row's turn to look along its lines of fire again.
    pub(crate) fn mount_picks(
        &self,
        row: usize,
        ordered: Option<usize>,
        (ground, strike): (bool, bool),
        due: bool,
    ) -> Vec<MountPick> {
        let bp = self.bp(row);
        let units = &self.state.units;
        let mut out = Vec::new();
        for (w, weapon) in bp.weapons.iter().enumerate() {
            if !weapon.is_group() {
                continue;
            }
            let first = bp.gun_slot(w);
            // Every unit keeps the guns its blueprint has (`fit_mounts`, `validate_mounts`).
            let guns = &units.mounts[row][first..first + weapon.mounts.len()];
            let mut near: Option<Vec<usize>> = None;
            for (m, state) in guns.iter().enumerate() {
                let gun = weapon.gun(m);
                let slot = first + m;
                let pick = if let Some(t) = ordered.filter(|&t| self.is_valid_target(row, t, gun)) {
                    Some(t)
                } else if ground && takes_ground(bp, gun, strike) {
                    None
                } else {
                    let near = near.get_or_insert_with(|| self.group_candidates(row, w));
                    self.choose_for_gun(row, gun, state, ordered, near)
                };
                let (pick, blocked) = self.gun_line(row, gun, state, pick, ordered, due);
                out.push((
                    slot as u16,
                    pick.map_or(Handle::NONE, |t| units.id(t)),
                    blocked,
                ));
            }
        }
        out
    }

    /// Each group's slot in `targets` and `blocked` from its guns' `picks`: its first gun
    /// with a mark's.
    pub(crate) fn group_marks(
        &self,
        row: usize,
        picks: &[MountPick],
        targets: &mut [UnitId; mc_data::MAX_WEAPONS],
        blocked: &mut u16,
    ) {
        let bp = self.bp(row);
        for (w, weapon) in bp.weapons.iter().enumerate() {
            if !weapon.is_group() {
                continue;
            }
            let first = bp.gun_slot(w);
            let guns = &picks[first..first + weapon.mounts.len()];
            let lead = guns
                .iter()
                .find(|p| p.1 != Handle::NONE)
                .unwrap_or(&guns[0]);
            targets[w] = lead.1;
            *blocked = *blocked & !(1 << w) | (lead.2 as u16) << w;
        }
    }

    /// Every gun of `row` let go of its mark: hold fire, or stunned. `None` when none has one.
    pub(crate) fn mounts_released(&self, row: usize) -> Option<Vec<MountPick>> {
        let mounts = &self.state.units.mounts[row];
        mounts.iter().any(|m| m.target != Handle::NONE).then(|| {
            (0..mounts.len())
                .map(|slot| (slot as u16, Handle::NONE, false))
                .collect()
        })
    }

    /// Takes the guns' marks `run_targeting` chose.
    pub(crate) fn apply_mount_picks(&mut self, row: usize, picks: Vec<MountPick>) {
        let mounts = &mut self.state.units.mounts[row];
        for (slot, target, blocked) in picks {
            if let Some(m) = mounts.get_mut(slot as usize) {
                m.target = target;
                m.blocked = blocked;
            }
        }
    }

    /// The enemies any gun of group `w` of `row` might reach: one search round the hull,
    /// out to the group's reach and its farthest pivot. Rows in the index's order, which
    /// depends only on the tables.
    fn group_candidates(&self, row: usize, w: usize) -> Vec<usize> {
        let bp = self.bp(row);
        let weapon = &bp.weapons[w];
        let units = &self.state.units;
        let reach = weapon.range_max + bp.mount_reach(w);
        let mut near = Vec::new();
        self.index.query_foes(
            units.pos[row],
            reach,
            target_kinds(weapon.target_mask),
            self.team_mask(units.owner[row]),
            |e| {
                if self.unit_entry_is_current(e) {
                    near.push(e.row as usize);
                }
                true
            },
        );
        near
    }

    /// What `gun` of `row` lays on, choosing for itself, as `run_targeting` chooses for an
    /// ordinary weapon: it keeps a mark it may still shoot, leaves it for one of the kinds
    /// it prefers, and a wall for anything else; else it takes the nearest from its own
    /// pivot (a held beam still streaming: from where its stream lands), preferred kinds
    /// first, walls last.
    fn choose_for_gun(
        &self,
        row: usize,
        gun: &Weapon,
        state: &MountState,
        ordered: Option<usize>,
        near: &[usize],
    ) -> Option<usize> {
        if !self.fires_at_will(row) {
            return None;
        }
        let units = &self.state.units;
        let current = units
            .row(state.target)
            .filter(|&t| self.is_valid_target(row, t, gun));
        let prefer = gun.prefer_mask;
        if let Some(t) = current {
            if !self.is_wall(t) && (prefer == 0 || self.hittable(t, prefer)) {
                return Some(t);
            }
        }
        let from = if gun.beam && state.streaming {
            state.beam_at
        } else {
            self.gun_origin(row, gun)
        };
        let origin = self.gun_origin(row, gun);
        // Nearest first: rows are unique, so equal keys never tie.
        let mut by_distance: Vec<(Fx, usize)> = near
            .iter()
            .filter(|&&t| {
                origin.distance(units.pos[t]) - self.bp(t).radius <= gun.range_max
                    && self.hittable(t, gun.target_mask)
            })
            .map(|&t| (from.distance(units.pos[t]), t))
            .collect();
        by_distance.sort_unstable();
        let (mut preferred, mut nearest, mut wall) = (None, None, None);
        for &(_, t) in &by_distance {
            if preferred.is_some() || (prefer == 0 && nearest.is_some()) {
                break;
            }
            if !self.is_valid_target(row, t, gun) {
                continue;
            }
            if self.is_wall(t) {
                wall = wall.or(Some(t));
            } else {
                if prefer != 0 && self.hittable(t, prefer) {
                    preferred = Some(t);
                }
                nearest = nearest.or(Some(t));
            }
        }
        match current {
            // On a wall no order names: anything else in reach first.
            Some(t) if self.is_wall(t) && ordered != Some(t) => preferred.or(nearest).or(Some(t)),
            // On a kind it does not prefer: one it does, if there is one.
            Some(t) => preferred.or(Some(t)),
            None => preferred.or(nearest).or(wall),
        }
    }

    /// `pick` for `gun` of `row` checked along its line of fire (`line_of_fire.rs`): on the
    /// row's turn, or at once on a new mark. A gun that cannot see its mark turns to one it
    /// can, unless an order names the one it is on; else it holds, blocked.
    fn gun_line(
        &self,
        row: usize,
        gun: &Weapon,
        state: &MountState,
        pick: Option<usize>,
        ordered: Option<usize>,
        due: bool,
    ) -> (Option<usize>, bool) {
        let Some(t) = pick.filter(|_| self.needs_line(row, gun)) else {
            return (pick, false);
        };
        let units = &self.state.units;
        if !due && units.id(t) == state.target {
            return (pick, state.blocked);
        }
        if self.clear_shot(row, gun, t) {
            return (pick, false);
        }
        if ordered != Some(t) && self.fires_at_will(row) {
            if let Some(seen) = self.visible_alternative(row, gun, t) {
                return (Some(seen), false);
            }
        }
        (pick, true)
    }
}

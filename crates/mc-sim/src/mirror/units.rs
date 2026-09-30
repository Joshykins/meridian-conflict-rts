//! The units' instances in the render mirror: one per unit the viewer may see, built
//! in parallel chunks of the unit table (each chunk's gun-house poses numbered on
//! from the chunks before it), in slot order.

use super::*;

/// Units per parallel chunk.
const CHUNK: usize = 256;

/// What every unit's instance reads this tick besides the unit itself.
pub(super) struct UnitPass<'a> {
    pub(super) viewer: Option<u8>,
    /// Lift ships' decks, which what walks up a ramp stands on.
    pub(super) decks: &'a [Deck],
    /// Construction wave origins by unit, in `welds`.
    pub(super) weld_range: &'a HashMap<u32, (u32, u32)>,
    pub(super) welds: &'a [ConstructionWeld],
}

impl World {
    /// Every unit's instance, in slot order, onto `units`, and the gun-house poses they
    /// name onto `houses` (both empty to start with).
    pub(super) fn unit_instances(
        &self,
        pass: &UnitPass,
        units: &mut Vec<UnitInstance>,
        houses: &mut Vec<HousePose>,
    ) {
        let rows: Vec<usize> = self.state.units.slots.iter().collect();
        let parts = self
            .pool
            .parallel_map_chunks(rows.len(), CHUNK, |_, range| {
                let mut out = Vec::with_capacity(range.len());
                let mut poses = Vec::new();
                for &row in &rows[range] {
                    out.extend(self.unit_instance(pass, row, &mut poses));
                }
                (out, poses)
            });
        for (out, poses) in parts {
            // A chunk numbers its houses from zero: on from those before it here.
            let base = houses.len() as u32;
            units.extend(out.into_iter().map(|mut u| {
                if u.status[1] >> UNIT_HOUSE_SHIFT != 0 {
                    u.status[1] += base << UNIT_HOUSE_SHIFT;
                }
                u
            }));
            houses.extend(poses);
        }
    }

    /// One unit's instance, `None` when the viewer does not see it drawn. Its gun
    /// houses' poses, if it has houses of its own, go onto `houses`.
    fn unit_instance(
        &self,
        pass: &UnitPass,
        row: usize,
        houses: &mut Vec<HousePose>,
    ) -> Option<UnitInstance> {
        let s = &self.state;
        let (viewer, decks) = (pass.viewer, pass.decks);
        let pitch = signed_pitch;
        // What walks up a lift ship's ramp stands on it, not on the ground under it.
        let on_deck = |row: usize, p: mc_core::FxVec2| -> f32 {
            if decks.is_empty() || self.is_air(row) {
                return 0.0;
            }
            let p = [p.x.to_f32(), p.y.to_f32()];
            decks.iter().map(|d| d.lift(p)).fold(0.0, f32::max)
        };
        let deck_up = |row: usize| -> Option<u32> {
            if decks.is_empty() || self.is_air(row) {
                return None;
            }
            let p = s.units.pos[row];
            let up = decks
                .iter()
                .find_map(|d| d.up([p.x.to_f32(), p.y.to_f32()]))?;
            let q = |v: f32| ((v * 32767.0).round().clamp(-32767.0, 32767.0) as i16 as u16) as u32;
            Some(q(up[0]) | q(up[1]) << 16)
        };
        let shown = self.warp_shown(viewer, row);
        let leaving = matches!(shown, warp::Shown::Leaving);
        match shown {
            warp::Shown::Hidden => return None,
            warp::Shown::Plain => {
                if let Some(v) = viewer {
                    if self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row) {
                        return None;
                    }
                }
            }
            warp::Shown::Leaving | warp::Shown::Listed => {}
        }
        let (warp_fx, warp_marks) = self.warp_fx(viewer, row);
        let bp = self.bp(row);
        // A refit is shown on the unit being refitted, not as a second unit inside it.
        if s.units.has_flag(row, crate::tables::flag::UPGRADE) {
            return None;
        }
        // In a lift ship's hold: its own side still lists it (`UNIT_STORED`) so the
        // hold can show it and it can be picked and ordered, but nothing draws it.
        let stored = s.units.hangar[row] != crate::Handle::NONE;
        if stored && viewer.is_some_and(|v| self.are_enemies(v, s.units.owner[row])) {
            return None;
        }
        // A pause is an order, not something the enemy can see: no mark on their side.
        let own_view = !viewer.is_some_and(|v| self.are_enemies(v, s.units.owner[row]));
        let paused_mark = s.units.paused[row] && own_view;
        let batch_mark = own_view && self.batching(row);
        let site = self.structure_upgrade(row);
        let refit = s
            .orders
            .front(&s.units, row)
            .filter(|o| o.kind == crate::tables::OrderKind::Upgrade && self.upgrades_in_place(row));
        let upgrade = site
            .or(refit.and_then(|_| s.units.row(s.units.build_target[row])))
            .map_or(0.0, |t| {
                (s.units.build_progress[t] / self.bp(t).build_time)
                    .to_f32()
                    .clamp(0.002, 1.0)
            });
        let step = s.units.gait_step[row];
        let mut flags = s.units.flags[row];
        // In warp it is out of the world, not in a factory: drawn as it leaves, then only listed.
        if !matches!(shown, warp::Shown::Plain) {
            flags &= !crate::tables::flag::IN_FACTORY;
        }
        let contact = if matches!(shown, warp::Shown::Plain) {
            self.contact_flags(viewer, row)
        } else {
            0
        };
        if contact & STATE_RADAR != 0 && flags & crate::tables::flag::IN_FACTORY != 0 {
            return None;
        }
        let (build, weld_id) = match site {
            Some(t) => {
                // The structure rebuilds in place: same fill-and-wave as a fresh site.
                flags |= crate::tables::flag::UNDER_CONSTRUCTION;
                (
                    (s.units.build_progress[t] / self.bp(t).build_time).to_f32(),
                    s.units.id(t).0,
                )
            }
            None => (
                (s.units.build_progress[row] / bp.build_time).to_f32(),
                s.units.id(row).0,
            ),
        };
        // The tube that kicks is the arm's main gun: the heaviest on the first weapon's elbow.
        let arm = bp.weapons.first().and_then(|w| w.pivot);
        let main = (0..bp.weapons.len())
            .filter(|&w| {
                w == 0 || (arm.is_some() && bp.weapons[w].pivot == arm && !bp.weapons[w].mount)
            })
            .max_by_key(|&w| (bp.weapons[w].damage, std::cmp::Reverse(w)))
            .unwrap_or(0);
        let (recoil, prev_recoil) = match bp.weapons.get(main) {
            // A held beam never kicks: it fires every tick, and a kick a tick shook the
            // gun at ten a second. Its "recoil" is how braced it is, this tick and last.
            Some(w) if w.beam => {
                beam_brace(w, s.units.spin[row], s.units.weapon_cooldown[row][main])
            }
            w => barrel_recoil_pair(
                s.units.weapon_cooldown[row][main],
                w.map(|w| w.reload_ticks).unwrap_or(0),
            ),
        };
        let twin = twin_arm_gun(&bp.weapons, main);
        let mounted = bp.weapons.iter().position(|w| w.mount);
        let (mount_kick, prev_mount_kick) = mounted.map_or((0.0, 0.0), |w| {
            barrel_recoil_pair(s.units.weapon_cooldown[row][w], bp.weapons[w].reload_ticks)
        });
        let house = self.house_pose(row, mounted.or(twin.map(|t| t.0)), houses);
        let spin = bp
            .weapons
            .iter()
            .find(|w| w.spin_ticks > 0)
            .map_or([0.0; 2], |_| {
                let [_, turn, step, _] = s.units.spin[row];
                let step = step as f32;
                let now = turn as f32 * (std::f32::consts::TAU / 65536.0);
                [now - step * (std::f32::consts::TAU / 65536.0), now]
            });
        let (weld, weld_first, weld_count) = weld_on_unit(pass.weld_range, pass.welds, weld_id)
            .or_else(|| weld_on_unit(pass.weld_range, pass.welds, s.units.id(row).0))
            .unwrap_or(([0.0; 3], 0, 0));
        Some(UnitInstance {
            prev_pos: {
                let mut p = s.units.prev_pos[row].extend(s.units.prev_z[row]).to_f32();
                p[2] += on_deck(row, s.units.prev_pos[row]);
                p
            },
            prev_heading: s.units.prev_heading[row].to_radians_f32(),
            pos: {
                // The tick it jumps it is drawn where it left, streaking out (`warp.rs`).
                let (at, z) = if leaving {
                    (s.units.prev_pos[row], s.units.prev_z[row])
                } else {
                    (s.units.pos[row], s.units.z[row])
                };
                let mut p = at.extend(z).to_f32();
                p[2] += on_deck(row, at);
                p
            },
            heading: s.units.heading[row].to_radians_f32(),
            blueprint: s.units.blueprint[row].0 as u32,
            owner_flags: s.units.owner[row] as u32
                | (flags as u32) << 8
                | if s.units.order_head[row] == crate::tables::NO_ORDER {
                    STATE_IDLE
                } else {
                    0
                }
                | if self.kit_unpowered(row) {
                    STATE_UNPOWERED
                } else {
                    0
                }
                | if self.kit_charging(row) {
                    STATE_CHARGING
                } else {
                    0
                }
                | contact,
            health: (s.units.health[row]
                / crate::veterancy_health(bp.health, s.units.veterancy[row]).max(Fx::ONE))
            .to_f32(),
            build,
            turret_yaw: s.units.weapon_yaw[row][0].to_radians_f32(),
            radius: bp.radius.to_f32(),
            unit_id: s.units.id(row).0,
            packed: {
                let level = s.units.veterancy[row];
                let need = crate::veterancy_need(level);
                let share = if level >= crate::VETERANCY_MAX {
                    0.0
                } else {
                    (s.units.veterancy_progress[row] / need).to_f32()
                };
                UnitInstance::pack_veterancy(s.units.kills[row], level, share)
                    | (s.units.fire_state[row] as u32) << UNIT_FIRE_STATE_SHIFT
                    | if s.units.burn_ticks[row] > 0 {
                        UNIT_BURNING
                    } else {
                        0
                    }
            },
            // A core mine's gait is its hammer's beat instead: blows struck, and the share of one
            // this tick added (`mines::hammer_gait`).
            gait: match s
                .mines
                .by_unit
                .get(&s.units.id(row))
                .filter(|_| bp.mine.is_some())
            {
                Some(m) if bp.mine.is_some_and(|m| m.hammer) => {
                    crate::mines::hammer_gait(m.age, bp.tech)
                }
                // A mine that strikes nothing has no beat.
                Some(_) => [0.0; 3],
                None => [
                    (s.units.gait[row] & 0xF_FFFF) as f32 / 256.0,
                    step[0] as f32 / 256.0,
                    step[1] as f32 / 256.0,
                ],
            },
            upgrade,
            arm_pitch: [
                pitch(s.units.prev_arm_pitch[row][0]),
                pitch(s.units.arm_pitch[row][0]),
                pitch(s.units.prev_arm_pitch[row][1]),
                pitch(s.units.arm_pitch[row][1]),
            ],
            prev_turret_yaw: s.units.prev_weapon_yaw[row][0].to_radians_f32(),
            weld,
            recoil,
            prev_recoil,
            weld_first,
            weld_count,
            // A siege gun's spade, or a builder's folding gear (`Builder::unfold_ticks`).
            deploy: {
                let need = self.deploy_span(row);
                if need == 0 {
                    0.0
                } else {
                    s.units.deploy[row] as f32 / need as f32
                }
            },
            prev_deploy: {
                let need = self.deploy_span(row);
                if need == 0 {
                    0.0
                } else {
                    s.units.prev_deploy[row] as f32 / need as f32
                }
            },
            _pad2: [
                s.units.prev_bank[row] as f32 * (std::f32::consts::TAU / 65536.0),
                s.units.bank[row] as f32 * (std::f32::consts::TAU / 65536.0),
            ],
            refit_modules: refit
                .and_then(|o| self.blueprints.refit_result(bp.id, o.blueprint).ok())
                .map_or(0, |to| self.blueprints.look(to)),
            status: [
                s.units.dive[row] as u32
                    | if s.units.dive_goal[row] {
                        UNIT_DIVE_GOAL
                    } else {
                        0
                    }
                    | if paused_mark { UNIT_PAUSED } else { 0 }
                    | if batch_mark { UNIT_BATCH } else { 0 }
                    | if stored { UNIT_STORED } else { 0 }
                    | self.lift_gear(row) << UNIT_GEAR_SHIFT
                    | if !stored && deck_up(row).is_some() {
                        UNIT_ON_DECK
                    } else {
                        0
                    }
                    | warp_marks,
                house.map_or(0, |i| (i as u32 + 1) << UNIT_HOUSE_SHIFT)
                    | twin.map_or(0, |(w, right)| {
                        (w as u32 + 1) << UNIT_TWIN_SHIFT | if right { UNIT_TWIN_RIGHT } else { 0 }
                    }),
                if stored {
                    s.units.hangar[row].0
                } else {
                    deck_up(row)
                        .or_else(|| self.loaded_cells(row))
                        .unwrap_or_else(|| self.launcher_pad(row))
                },
            ],
            mount: mounted.map_or([0.0; 4], |w| {
                let off = |yaw: &[mc_core::Angle; mc_data::MAX_WEAPONS]| pitch(yaw[w] - yaw[0]);
                [
                    off(&s.units.prev_weapon_yaw[row]),
                    off(&s.units.weapon_yaw[row]),
                    pitch(s.units.prev_arm_pitch[row][2 + w]),
                    pitch(s.units.arm_pitch[row][2 + w]),
                ]
            }),
            spin_recoil: [spin[0], spin[1], prev_mount_kick, mount_kick],
            fx: warp_fx,
            drive_swing: [0.0; 2],
            _pad3: [0.0; 2],
        })
    }

    /// Every weapon's pose for a unit whose guns sit on houses of their own, or whose arm
    /// has a twin that kicks on its own shots (`gunned`, the weapon that makes it so), or
    /// whose reclaim heads turn on pivots: pushed onto `houses`, and its index there.
    fn house_pose(
        &self,
        row: usize,
        gunned: Option<usize>,
        houses: &mut Vec<HousePose>,
    ) -> Option<usize> {
        let s = &self.state;
        let bp = self.bp(row);
        let pitch = signed_pitch;
        // Guns on houses of their own (`rig::HOUSE`), or a twin on the arm that kicks on
        // its own shots: every weapon's pose, in a side list.
        let house = gunned.map(|_| {
            let mut hp = HousePose::default();
            for (w, weapon) in bp.weapons.iter().enumerate().take(mc_data::MAX_HOUSES) {
                let slot = crate::combat::pitch_slot(weapon, w);
                hp.pose[w] = [
                    s.units.prev_weapon_yaw[row][w].to_radians_f32(),
                    s.units.weapon_yaw[row][w].to_radians_f32(),
                    pitch(s.units.prev_arm_pitch[row][slot]),
                    pitch(s.units.arm_pitch[row][slot]),
                ];
                // Mid-salvo, each shot kicks on its own: the countdown is the gap to the
                // next shot, not the reload.
                let run = if s.units.weapon_salvo_left[row][w] > 0 {
                    weapon.salvo_delay_ticks.max(1) as u16
                } else {
                    weapon.reload_ticks
                };
                let (now, prev) = barrel_recoil_pair(s.units.weapon_cooldown[row][w], run);
                hp.kick[2 * w] = prev;
                hp.kick[2 * w + 1] = now;
            }
            houses.push(hp);
            houses.len() - 1
        });
        // Reclaim heads on pivots are houses too: head `i` in slot `i`, and they never kick.
        let house = house.or_else(|| {
            let heads = bp.reclaimer.as_ref().map(|r| r.heads()).unwrap_or(&[]);
            heads.iter().any(|h| h.pivot.is_some()).then(|| {
                let mut hp = HousePose::default();
                for (i, pose) in hp.pose.iter_mut().enumerate().take(heads.len()) {
                    *pose = [
                        s.units.prev_weapon_yaw[row][i].to_radians_f32(),
                        s.units.weapon_yaw[row][i].to_radians_f32(),
                        pitch(s.units.prev_arm_pitch[row][2 + i]),
                        pitch(s.units.arm_pitch[row][2 + i]),
                    ];
                }
                houses.push(hp);
                houses.len() - 1
            })
        });
        house
    }
}

/// An angle as radians either side of level: a little below level is a little below
/// zero, not nearly a full turn.
fn signed_pitch(a: mc_core::Angle) -> f32 {
    mc_core::Angle::ZERO.delta_to(a) as f32 * (std::f32::consts::TAU / 65536.0)
}

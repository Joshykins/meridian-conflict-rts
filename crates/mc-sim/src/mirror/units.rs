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
        // The sim banks every jet to 45 degrees at its hardest turn (its roll is its
        // steering state). A fighter is drawn rolled to 75: a hard turn reads as one
        // flown wings near vertical, not as a skid.
        let fighter = bp.has(mc_data::cat::ANTI_AIR)
            && bp
                .motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Air && !m.hover);
        let roll_scale = std::f32::consts::TAU / 65536.0 * if fighter { 75.0 / 45.0 } else { 1.0 };
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
        // A pause or a priority is an order, not something the enemy can see: no mark on
        // their side.
        let own_view = !viewer.is_some_and(|v| self.are_enemies(v, s.units.owner[row]));
        let paused_mark = s.units.paused[row] && own_view;
        let priority_mark = if own_view {
            s.units.priority[row] as u32
        } else {
            0
        };
        let batch_mark = own_view && self.batching(row);
        // A half-built unit on its own lot that is raised there again once done.
        let repeat_mark = own_view
            && s.units.has_flag(row, crate::tables::flag::REPEAT)
            && s.units
                .has_flag(row, crate::tables::flag::UNDER_CONSTRUCTION)
            && bp.is_site_built_unit();
        let site = self.structure_upgrade(row);
        // The modules it is being refitted with show on it as the work goes on.
        let refit = s
            .orders
            .front(&s.units, row)
            .filter(|o| o.kind == crate::tables::OrderKind::Upgrade && self.upgrades_in_place(row));
        let upgrade = self.upgrade_work(row).map_or(0.0, |t| {
            (s.units.build_progress[t] / self.bp(t).build_time)
                .to_f32()
                .clamp(0.002, 1.0)
        });
        let step = s.units.gait_step[row];
        // A dashing unit's wings: its windup and landing (`dash_pose`).
        let dash = bp.dash.as_ref().map(|d| dash_pose(d, &s.units.dash[row]));
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
            Some(w) if w.beam => beam_brace(
                w,
                s.units.spin[row][w.rotary as usize],
                s.units.weapon_cooldown[row][main],
            ),
            Some(w) => salvo_recoil_pair(
                s.units.weapon_cooldown[row][main],
                w,
                s.units.weapon_salvo_left[row][main],
            ),
            None => (0.0, 0.0),
        };
        let twin = twin_arm_gun(&bp.weapons, main);
        let mounted = bp.weapons.iter().position(|w| w.mount);
        let (mount_kick, prev_mount_kick) = mounted.map_or((0.0, 0.0), |w| {
            let gun = &bp.weapons[w];
            let cooldown = s.units.weapon_cooldown[row][w];
            if gun.run_out_ticks > 0 {
                run_out_pair(
                    cooldown,
                    gun.reload_ticks,
                    gun.run_out_ticks,
                    gun.charge_ticks,
                )
            } else {
                barrel_recoil_pair(cooldown, gun.reload_ticks)
            }
        });
        let house = self.house_pose(row, mounted.or(twin.map(|t| t.0)), houses);
        // Each rotary gun's barrels, by its slot (`Weapon::rotary`): turned last tick and this.
        // A shoulder gun's flung gear takes the first slot when the unit has no rotary gun.
        let spin = |slot: usize| {
            if bp.weapons.iter().filter(|w| w.spin_ticks > 0).count() <= slot {
                return match mounted.filter(|_| slot == 0) {
                    Some(w) if bp.weapons[w].fling > 0 => flung_pair(
                        s.tick,
                        s.units.id(row).0,
                        s.units.weapon_cooldown[row][w],
                        (bp.weapons[w].reload_ticks, bp.weapons[w].charge_ticks),
                        bp.weapons[w].fling,
                    ),
                    _ => [0.0; 2],
                };
            }
            let [_, turn, step, _] = s.units.spin[row][slot];
            let step = step as f32 * (std::f32::consts::TAU / 65536.0);
            let now = turn as f32 * (std::f32::consts::TAU / 65536.0);
            [now - step, now]
        };
        let (spin, twin_spin) = (spin(0), spin(1));
        let (weld, weld_first, weld_count) = weld_on_unit(pass.weld_range, pass.welds, weld_id)
            .or_else(|| weld_on_unit(pass.weld_range, pass.welds, s.units.id(row).0))
            .unwrap_or(([0.0; 3], 0, 0));
        // A space elevator: its cable, clamps and platform, last tick and this
        // (`elevator::pose`), in `mount` and `deploy` (`gpu_consts::elevator`).
        let elevator = bp.elevator.map(|e| {
            let age = self.elevator_age(row);
            (
                crate::elevator::pose(&e, age.map(|a| a.saturating_sub(1))),
                crate::elevator::pose(&e, age),
            )
        });
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
            health: (s.units.health[row] / self.unit_max_health(row).max(Fx::ONE)).to_f32(),
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
                    (s.units.gait[row] % (mc_core::GAIT_WRAP * 256)) as f32 / 256.0,
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
            // A material fabricator: how hard it works this tick, in both (`fabricator_work`).
            deploy: if let Some((_, now)) = elevator {
                now[2]
            } else if let Some(dash) = dash {
                dash[1]
            } else if let Some(work) = self.fabricator_work(row) {
                work
            } else {
                let need = self.deploy_span(row);
                if need == 0 {
                    0.0
                } else {
                    s.units.deploy[row] as f32 / need as f32
                }
            },
            prev_deploy: if let Some((was, _)) = elevator {
                was[2]
            } else if let Some(dash) = dash {
                dash[0]
            } else if let Some(work) = self.fabricator_work(row) {
                work
            } else {
                let need = self.deploy_span(row);
                if need == 0 {
                    0.0
                } else {
                    s.units.prev_deploy[row] as f32 / need as f32
                }
            },
            _pad2: [
                s.units.prev_bank[row] as f32 * roll_scale,
                s.units.bank[row] as f32 * roll_scale,
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
                    | if repeat_mark { UNIT_REPEAT } else { 0 }
                    | priority_mark << UNIT_PRIORITY_SHIFT
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
                        .or_else(|| self.fired_cells(row))
                        .or_else(|| self.held_orbs(row))
                        .unwrap_or_else(|| self.launcher_pad(row))
                },
            ],
            mount: if let Some((was, now)) = elevator {
                [was[0], now[0], was[1], now[1]]
            } else {
                mounted.map_or([0.0; 4], |w| {
                    let off = |yaw: &[mc_core::Angle; mc_data::MAX_WEAPONS]| pitch(yaw[w] - yaw[0]);
                    [
                        off(&s.units.prev_weapon_yaw[row]),
                        off(&s.units.weapon_yaw[row]),
                        pitch(s.units.prev_arm_pitch[row][2 + w]),
                        pitch(s.units.arm_pitch[row][2 + w]),
                    ]
                })
            },
            spin_recoil: [spin[0], spin[1], prev_mount_kick, mount_kick],
            fx: warp_fx,
            drive_swing: [0.0; 2],
            twin_spin,
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
        // its own shots: every gun's pose, by gun slot (`UnitBlueprint::gun_slot`), eight
        // to a record, in as many records in a row as the unit has guns for.
        let house = gunned.map(|_| {
            let first = houses.len();
            let records = bp.gun_count().div_ceil(mc_data::MAX_HOUSES).max(1);
            houses.resize(first + records, HousePose::default());
            for (w, weapon) in bp.weapons.iter().enumerate() {
                let slot = bp.gun_slot(w);
                for m in 0..weapon.guns() {
                    let (g, (pose, kick)) = (slot + m, self.gun_pose(row, w, m));
                    let (hp, k) = (
                        &mut houses[first + g / mc_data::MAX_HOUSES],
                        g % mc_data::MAX_HOUSES,
                    );
                    hp.pose[k] = pose;
                    hp.kick[2 * k] = kick.1;
                    hp.kick[2 * k + 1] = kick.0;
                }
            }
            first
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

impl World {
    /// Gun `m` of weapon `w` of `row` (a group's gun, else the weapon's one): its yaw off
    /// the hull last tick and this, then its pitch last tick and this (radians); and its
    /// kick this tick and last. Mid-salvo each shot kicks on its own: the countdown is the
    /// gap to the next shot, not the reload. A held beam fires every tick, and never kicks.
    fn gun_pose(&self, row: usize, w: usize, m: usize) -> ([f32; 4], (f32, f32)) {
        let s = &self.state;
        let weapon = self.bp(row).weapons[w].gun(m);
        let (yaw, prev_yaw, pitch, prev_pitch, cooldown, salvo_left) =
            match s.units.mounts[row].get(self.bp(row).gun_slot(w) + m) {
                Some(g) if self.bp(row).weapons[w].is_group() => (
                    g.yaw,
                    g.prev_yaw,
                    g.pitch,
                    g.prev_pitch,
                    g.cooldown,
                    g.salvo_left,
                ),
                _ => {
                    let slot = crate::combat::pitch_slot(weapon, w);
                    (
                        s.units.weapon_yaw[row][w],
                        s.units.prev_weapon_yaw[row][w],
                        s.units.arm_pitch[row][slot],
                        s.units.prev_arm_pitch[row][slot],
                        s.units.weapon_cooldown[row][w],
                        s.units.weapon_salvo_left[row][w],
                    )
                }
            };
        let pose = [
            prev_yaw.to_radians_f32(),
            yaw.to_radians_f32(),
            signed_pitch(prev_pitch),
            signed_pitch(pitch),
        ];
        let run = if salvo_left > 0 {
            weapon.salvo_delay_ticks.max(1) as u16
        } else {
            weapon.reload_ticks
        };
        let kick = if weapon.beam {
            (0.0, 0.0)
        } else {
            barrel_recoil_pair(cooldown, run)
        };
        (pose, kick)
    }
}

/// An angle as radians either side of level: a little below level is a little below
/// zero, not nearly a full turn.
fn signed_pitch(a: mc_core::Angle) -> f32 {
    mc_core::Angle::ZERO.delta_to(a) as f32 * (std::f32::consts::TAU / 65536.0)
}

/// Ticks the landing's blow takes to spend itself in the wings and legs (`dash_pose`).
const LAND_TICKS: u16 = 10;

/// What a dashing unit's wings and legs are doing (`mirror::UnitInstance::deploy`, read by
/// flight.wgsl), last tick and this: its windup's share, 0 to 1; then from the landing,
/// -1 at the blow easing back to 0 over `LAND_TICKS`. Zero for a unit that never dashed.
fn dash_pose(dash: &mc_data::dash::Dash, s: &crate::dash::DashState) -> [f32; 2] {
    match s.phase {
        crate::dash::DashPhase::Windup => {
            let w = dash.windup_ticks.max(1) as f32;
            return [s.ticks.saturating_sub(1) as f32 / w, s.ticks as f32 / w];
        }
        // Gone: not drawn, held at the full charge it left with.
        crate::dash::DashPhase::Transit => return [1.0; 2],
        crate::dash::DashPhase::Ready => {}
    }
    // Ticks since it landed: its cooldown started at the landing.
    let since = dash.cooldown_ticks.saturating_sub(s.cooldown);
    if s.to == mc_core::FxVec2::ZERO || since > LAND_TICKS {
        return [0.0; 2];
    }
    let blow = |t: u16| -(1.0 - t as f32 / LAND_TICKS as f32).clamp(0.0, 1.0);
    // The landing tick follows the full windup.
    let was = if since == 0 { 1.0 } else { blow(since - 1) };
    [was, blow(since)]
}

/// Ticks a shoulder gun's flung gear takes to drift round once at rest.
const FLING_DRIFT_TICKS: u32 = 320;

/// Whole turns a shoulder gun's flung gear is wound through over its charge
/// (`Weapon::charge_ticks`), turning faster and faster into the shot.
const WIND_TURNS: f32 = 2.0;

/// How far a shoulder gun's flung gear (`Weapon::fling`, the Exarch mortar's crystal) has
/// turned, last tick and this, in radians: a slow drift at rest (one turn every
/// `FLING_DRIFT_TICKS`, each unit from its own start); wound through `WIND_TURNS` over the
/// countdown's last `charge` ticks, faster and faster into the shot; and each shot flinging
/// it through `turns` whole turns, fast at the shot and easing to rest as the reload runs
/// out. Every part ends on whole turns, so nothing jumps at the shot or the next one.
fn flung_pair(
    tick: u32,
    unit: u32,
    cooldown: u16,
    (reload, charge): (u16, u16),
    turns: u8,
) -> [f32; 2] {
    use std::f32::consts::TAU;
    let reload = reload.max(1);
    let turns = turns as f32;
    let flung = |c: u16| TAU * turns * (1.0 - (c.min(reload) as f32 / reload as f32).powi(3));
    // From nothing at the charge's first tick to whole turns at its last (a countdown of 1).
    let wound = |c: u16| {
        if charge < 2 || c == 0 || c > charge {
            return 0.0;
        }
        let f = (charge - c) as f32 / (charge - 1) as f32;
        TAU * WIND_TURNS * f * f
    };
    let drift = TAU / FLING_DRIFT_TICKS as f32;
    let now_drift = (tick.wrapping_add(unit.wrapping_mul(97)) % FLING_DRIFT_TICKS) as f32 * drift;
    // The tick it fires it starts from where the last shot left it: a whole number of turns.
    // At rest it was at rest last tick too, whole turns on from its wind (a charge that
    // never fired).
    let was = if cooldown == reload {
        0.0
    } else if cooldown == 0 {
        flung(1)
    } else {
        flung(cooldown + 1) + wound(cooldown + 1)
    };
    [
        now_drift - drift + was,
        now_drift + flung(cooldown) + wound(cooldown),
    ]
}

/// Ticks a heavy shoulder gun holds at full kick before it starts to run out.
const KICK_HOLD: u16 = 2;
/// How far back a charging heavy shoulder gun has drawn by its shot, a share of its kick.
const DRAW_BACK: f32 = 0.18;

/// A heavy shoulder gun's kick (`Weapon::run_out_ticks`) this tick and last, a share of the
/// model's: drawn back a little over the countdown's last `charge` ticks, slammed all the
/// way back at the shot, held there `KICK_HOLD` ticks, then run out over `run_out` ticks,
/// easing off at both ends as a recuperator does.
fn run_out_pair(cooldown: u16, reload: u16, run_out: u16, charge: u16) -> (f32, f32) {
    let kick = |c: u16| -> f32 {
        if c == 0 || reload == 0 {
            return 0.0;
        }
        let since = reload.saturating_sub(c);
        if c <= charge && since > KICK_HOLD + run_out {
            let f = (charge - c) as f32 / charge as f32;
            return DRAW_BACK * f * f;
        }
        if since < KICK_HOLD {
            return 1.0;
        }
        let x = (since - KICK_HOLD) as f32 / run_out.max(1) as f32;
        if x >= 1.0 {
            return 0.0;
        }
        (1.0 - x) * (1.0 - x) * (1.0 + 2.0 * x)
    };
    // At rest it was at rest last tick too (a charge that never fired lets go at once).
    if cooldown == 0 {
        return (0.0, 0.0);
    }
    // The shot slams it back from where its charge drew it.
    let prev = if cooldown == reload {
        kick(1)
    } else {
        kick(cooldown.saturating_add(1))
    };
    (kick(cooldown), prev)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    /// The crystal is flung from where the last shot left it, fastest at the shot, and comes
    /// to rest a whole number of turns on, so nothing jumps at the next shot.
    #[test]
    fn a_flung_gear_turns_whole_turns_fastest_at_the_shot() {
        let off = |a: f32| (a / TAU - (a / TAU).round()).abs();
        let at = |c: u16| super::flung_pair(1000, 7, c, (160, 0), 3);
        let [was, now] = at(160);
        assert!(off(now - was) < 0.01, "the shot: {was} -> {now}");
        let first = at(159);
        let last = at(1);
        assert!(first[1] - first[0] > 10.0 * (last[1] - last[0]));
        let [_, ready] = at(0);
        assert!(off(ready - at(160)[1]) < 0.01);
        assert!((ready - at(160)[1] - 3.0 * TAU).abs() < 0.01);
    }

    /// Over the charge the crystal winds up faster and faster, and reaches the shot on whole
    /// turns, so the shot's fling takes up from it without a jump.
    #[test]
    fn a_flung_gear_winds_up_into_the_shot() {
        let off = |a: f32| (a / TAU - (a / TAU).round()).abs();
        let at = |c: u16| super::flung_pair(1000, 7, c, (160, 32), 3);
        let (early, late) = (at(30), at(2));
        assert!(late[1] - late[0] > 5.0 * (early[1] - early[0]));
        let (last, fired) = (at(1)[1], at(160)[0]);
        assert!(off(last - fired) < 0.01, "{last} -> {fired}");
        let [was, now] = at(0);
        assert!((now - was).abs() < 0.1, "at rest: {was} -> {now}");
    }

    /// A heavy gun draws back over its charge, slams all the way back at the shot, holds, and
    /// runs out smoothly, home long before its next charge.
    #[test]
    fn a_heavy_gun_kicks_holds_and_runs_out() {
        let at = |c: u16| super::run_out_pair(c, 160, 28, 32);
        assert_eq!(at(0), (0.0, 0.0));
        let (drawn, _) = at(1);
        assert!(drawn > 0.1 && drawn < 0.3, "{drawn}");
        assert_eq!(at(160), (1.0, drawn));
        assert_eq!(at(159).0, 1.0);
        let mut last = 1.0;
        for c in (100..159).rev() {
            let (now, was) = at(c);
            assert!(now <= was && now <= last, "{c}");
            last = now;
        }
        assert_eq!(at(100).0, 0.0);
    }
}

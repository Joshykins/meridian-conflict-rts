//! The render mirror: what the simulation publishes for presentation.
//!
//! Built once at the end of a tick on the sim side. Each record carries the
//! previous and the current tick's transform, so the GPU interpolates on its
//! own and the render thread never touches an entity. Floats are fine here:
//! nothing in this module is read back by the simulation.

use crate::tables::UnitId;
use crate::World;
use bytemuck::{Pod, Zeroable};
use mc_core::{Fx, FxVec3};
use mc_data::{cat, BlueprintId, WeaponColor};

#[derive(Clone, Debug)]
pub enum SimEvent {
    UnitCompleted {
        unit: UnitId,
        owner: u8,
    },
    UnitDied {
        pos: FxVec3,
        blueprint: BlueprintId,
        owner: u8,
    },
    /// The last of a unit, or of a wreck, went up a reclaim beam. Instead of `UnitDied`: nothing blows up.
    Reclaimed {
        pos: FxVec3,
        blueprint: BlueprintId,
        wreck: bool,
    },
    /// A weapon with a target began charging for its next salvo (`Weapon::charge_ticks` before it is due).
    WeaponCharging {
        pos: FxVec3,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// `vel` is metres per tick. `blueprint` and `weapon` name the weapon that fired.
    ShotFired {
        pos: FxVec3,
        vel: FxVec3,
        color: WeaponColor,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// `after` is how far into the tick the shot arrived, zero to one, so the
    /// burst can wait for the shell that is drawn flying in. `on_unit`: it
    /// struck a unit rather than the ground.
    Impact {
        pos: FxVec3,
        splash: mc_core::Fx,
        color: WeaponColor,
        after: mc_core::Fx,
        on_unit: bool,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// The terrain edit table grew (or was replaced by a snapshot).
    TerrainEdited,
    BuildRejected {
        player: u8,
    },
    PlayerDefeated {
        player: u8,
    },
    MatchOver {
        winner_team: u8,
    },
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct UnitInstance {
    pub prev_pos: [f32; 3],
    pub prev_heading: f32,
    pub pos: [f32; 3],
    pub heading: f32,
    pub blueprint: u32,
    /// `owner | flags << 8`, plus the `KIND_*` bits.
    pub owner_flags: u32,
    pub health: f32,
    /// Zero to one; one when complete.
    pub build: f32,
    /// Turret yaw of the first weapon relative to the hull, radians.
    pub turret_yaw: f32,
    pub radius: f32,
    pub unit_id: u32,
    /// Props: scale in thousandths. Units: kills in the low 16 bits, rank in
    /// 16..24, progress toward the next rank (0..=255) in 24..32.
    pub _pad: u32,
    /// Ground covered in metres (wraps at 4096), then what this tick and the
    /// tick before added to it: the vertex shader times a walker's stride by it.
    pub gait: [f32; 3],
    /// Zero, or how far along the unit's refit is: above zero from its first tick, one when done.
    pub upgrade: f32,
    /// Pitch of the gun arm last tick and this, then of the build arm: radians, up positive.
    pub arm_pitch: [f32; 4],
    /// `turret_yaw` last tick, so the turret glides between ticks like the hull does.
    pub prev_turret_yaw: f32,
    /// Local-space point a construction beam is printing from. Zero when the unit is not a site.
    pub weld: [f32; 3],
}

impl UnitInstance {
    pub fn pack_veterancy(kills: u32, level: u8, progress: f32) -> u32 {
        let kills = kills.min(0xFFFF);
        let progress = (progress.clamp(0.0, 1.0) * 255.0).round() as u32;
        kills | (level as u32) << 16 | progress << 24
    }

    pub fn kill_count(&self) -> u32 {
        self._pad & 0xFFFF
    }

    pub fn veterancy_level(&self) -> u8 {
        ((self._pad >> 16) & 0xFF) as u8
    }

    /// Zero to one toward the next rank. Zero when already at the top.
    pub fn veterancy_progress(&self) -> f32 {
        ((self._pad >> 24) & 0xFF) as f32 / 255.0
    }
}

/// `owner_flags` bit: the record is a wreck. `health` holds the share of mass left.
pub const KIND_WRECK: u32 = 1 << 31;
/// `owner_flags` bit: a map prop (set by the renderer for its static records).
pub const KIND_PROP: u32 = 1 << 30;
/// `owner_flags` bit: a placement preview added by the UI, not a real unit.
pub const KIND_GHOST: u32 = 1 << 29;
/// `owner_flags` bit: the unit has no orders. Above the sim's sixteen flag bits.
pub const STATE_IDLE: u32 = 1 << 24;
/// Detected by radar only: draw the strategic icon, never the model.
pub const STATE_RADAR: u32 = 1 << 25;
/// Radar contact that vision has never identified: a grey blip, not the real icon.
pub const STATE_UNIDENTIFIED: u32 = 1 << 26;

/// Most units `write_orders` lists when asked for a whole side's queues.
pub const MAX_LISTED_UNITS: usize = 1024;
/// Most orders listed per unit in `UnitOrders`; a longer queue is cut short for display.
pub const MAX_LISTED_ORDERS: usize = 96;

/// One entry of a unit's order queue, for the interface.
#[derive(Clone, Copy, Debug)]
pub struct QueuedOrder {
    pub kind: crate::tables::OrderKind,
    /// Where the order takes the unit: its position, or its target's.
    pub pos: [f32; 2],
    /// The order's own position, exactly: what `Command::RelocateOrder` takes as `from`.
    pub at: mc_core::FxVec2,
    /// What `Build`, `Produce` and `Upgrade` make.
    pub blueprint: BlueprintId,
}

/// The order queue of one unit the interface asked about, front first.
#[derive(Clone, Debug, Default)]
pub struct UnitOrders {
    pub unit_id: u32,
    pub orders: Vec<QueuedOrder>,
    /// Zero to one: how far along the thing this unit is building is. Zero when it builds nothing.
    pub progress: f32,
}

/// A structure a builder has been ordered to build and has not begun.
#[derive(Clone, Copy, Debug)]
pub struct PlannedBuild {
    pub unit_id: u32,
    pub blueprint: BlueprintId,
    pub pos: [f32; 2],
    /// The site exactly: what `Command::RelocateOrder` takes as `from`.
    pub at: mc_core::FxVec2,
    pub heading: f32,
}

/// Set in `ProjectileInstance::color` for a missile.
pub const PROJECTILE_MISSILE: u32 = 1 << 8;
/// Set in `ProjectileInstance::color` for a construction beam: drawn whole from
/// `prev_pos` (the emitter) to `pos` (where the beam meets the work). `size` is the work's radius.
pub const PROJECTILE_BEAM: u32 = 1 << 10;
/// Low byte of `ProjectileInstance::color` for a construction beam (0 blue, 1 orange are weapons).
pub const COLOR_BUILD: u32 = 2;
/// Set in `ProjectileInstance::color` for a shot fired this tick: `prev_pos` is
/// the muzzle, and its trace must not reach back behind it.
pub const PROJECTILE_FRESH: u32 = 1 << 9;
/// Bits 16..24 of `ProjectileInstance::color`: the shot ends during this tick,
/// this many 255ths of the way through it, at `pos`. Zero for a shot still in flight.
pub const PROJECTILE_ENDS_SHIFT: u32 = 16;

/// A shot that hit something this tick. It has already left the projectile
/// table, but its last stretch still has to be drawn. Not state.
#[derive(Clone, Copy, Debug)]
pub struct SpentShot {
    pub from: FxVec3,
    pub to: FxVec3,
    /// Share of the tick the last stretch took, zero to one.
    pub after: mc_core::Fx,
    pub blueprint: BlueprintId,
    pub weapon: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct ProjectileInstance {
    pub prev_pos: [f32; 3],
    /// Tracer colour (0 blue, 1 orange), plus `PROJECTILE_MISSILE` for a missile.
    pub color: u32,
    pub pos: [f32; 3],
    pub size: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct StainInstance {
    pub pos: [f32; 2],
    pub radius: f32,
    /// `strength | seed << 8`.
    pub strength_seed: u32,
}

/// One tick's worth of presentation data.
#[derive(Clone, Default)]
pub struct RenderFrame {
    pub tick: u32,
    /// Units first, then wrecks (flagged `KIND_WRECK`).
    pub units: Vec<UnitInstance>,
    pub projectiles: Vec<ProjectileInstance>,
    /// Reclaim beams at work this tick.
    pub beams: Vec<crate::reclaim::BeamInstance>,
    /// The unit each of `beams` comes from, so the game can tell a beam starting from one carrying on.
    pub beam_sources: Vec<u32>,
    /// Mobile builders whose construction beam is on this tick, and where it meets the work.
    pub build_sources: Vec<(u32, [f32; 3])>,
    pub stains: Vec<StainInstance>,
    /// Poured structure lots, including those whose building is already gone.
    pub pads: Vec<StainInstance>,
    pub events: Vec<SimEvent>,
    /// Fog for the viewer, two bytes per 64 m cell: visible now, explored. Empty when fog is off.
    pub fog: Vec<u8>,
    pub fog_dims: (u32, u32),
    /// One bit per map prop, set when destroyed.
    pub props_dead: Vec<u32>,
    /// The whole terrain edit table, in order.
    pub terrain_edits: Vec<mc_map::FlattenRecord>,
}

impl World {
    /// Where a construction beam leaves this builder.
    fn builder_emitter(&self, row: usize) -> FxVec3 {
        let bp = self.bp(row);
        let s = &self.state;
        match bp.builder.as_ref().and_then(|b| b.arm) {
            Some(arm) => {
                let facing = s.units.heading[row] + s.units.weapon_yaw[row][0];
                let at = crate::world::pitched(arm.emitter, arm.pivot, s.units.arm_pitch[row][1]);
                (s.units.pos[row] + mc_core::FxVec2::new(at.x, at.y).rotate(facing))
                    .extend(s.units.z[row] + at.z)
            }
            None => s.units.pos[row].extend(s.units.z[row] + bp.height),
        }
    }

    /// The point on `target` a beam from `from` prints at: where it meets the
    /// work, in the world and in the work's model space.
    fn weld_on(&self, target: usize, from: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let s = &self.state;
        let bp = self.bp(target);
        let heading = s.units.heading[target].to_radians_f32();
        let (sin, cos) = heading.sin_cos();
        let origin = s.units.pos[target].extend(s.units.z[target]).to_f32();
        let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
        let center = [origin[0], origin[1], origin[2] + h * 0.5];
        let dir = [
            center[0] - from[0],
            center[1] - from[1],
            center[2] - from[2],
        ];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2])
            .sqrt()
            .max(0.001);
        let inset = r * 0.82;
        let hit = [
            center[0] - dir[0] / len * inset,
            center[1] - dir[1] / len * inset,
            (center[2] - dir[2] / len * inset).clamp(origin[2] + h * 0.15, origin[2] + h * 0.8),
        ];
        let off = [hit[0] - origin[0], hit[1] - origin[1], hit[2] - origin[2]];
        (
            hit,
            [
                off[0] * cos + off[1] * sin,
                -off[0] * sin + off[1] * cos,
                off[2],
            ],
        )
    }

    /// Work a mobile builder's beam can print on: an open site, or an upgrade
    /// assembling inside the unit it replaces.
    fn print_target(&self, row: usize) -> Option<usize> {
        use crate::tables::flag;
        self.state
            .units
            .row(self.state.units.build_target[row])
            .filter(|&t| {
                !self.state.units.has_flag(t, flag::IN_FACTORY)
                    || self.state.units.has_flag(t, flag::UPGRADE)
            })
    }

    /// The hidden successor a structure is assembling in place, when there is one.
    /// Extractors are refitted like a mobile unit: the next kit is built onto them.
    fn structure_upgrade(&self, row: usize) -> Option<usize> {
        use crate::tables::flag;
        if self.bp(row).is_mobile() || self.bp(row).has(cat::EXTRACTOR) {
            return None;
        }
        self.state
            .units
            .row(self.state.units.build_target[row])
            .filter(|&t| {
                self.state.units.has_flag(t, flag::UPGRADE)
                    && self.state.units.has_flag(t, flag::UNDER_CONSTRUCTION)
            })
    }

    /// Each open site's print origin: the weld of the closest mobile builder
    /// working on it. Sites nobody is on expand from the pad.
    fn construction_welds(&self) -> std::collections::HashMap<u32, ([f32; 3], [f32; 3])> {
        use crate::tables::flag;
        let s = &self.state;
        let mut best: std::collections::HashMap<u32, (f32, [f32; 3], [f32; 3])> =
            std::collections::HashMap::new();
        for row in s.units.slots.iter() {
            if s.units.flags[row] & flag::BUILDING == 0 || !self.bp(row).is_mobile() {
                continue;
            }
            let Some(t) = self.print_target(row) else {
                continue;
            };
            let from = self.builder_emitter(row).to_f32();
            let (world, local) = self.weld_on(t, from);
            let dx = world[0] - from[0];
            let dy = world[1] - from[1];
            let dz = world[2] - from[2];
            let dist = dx * dx + dy * dy + dz * dz;
            let id = s.units.id(t).0;
            if best.get(&id).is_none_or(|(d, _, _)| dist < *d) {
                best.insert(id, (dist, world, local));
            }
        }
        best.into_iter()
            .map(|(id, (_, world, local))| (id, (world, local)))
            .collect()
    }

    /// Fills `frame` with this tick's mirror, reusing its allocations. Units the
    /// viewer cannot detect are left out, so fog is enforced before the GPU.
    /// `viewer = None` shows everything (observers, replays, tools).
    pub fn write_render_frame(&self, viewer: Option<u8>, frame: &mut RenderFrame) {
        let s = &self.state;
        frame.tick = s.tick;
        frame.units.clear();
        let welds = self.construction_welds();
        // Signed: a little below level is a little below zero, not nearly a full turn.
        let pitch = |a: mc_core::Angle| {
            mc_core::Angle::ZERO.delta_to(a) as f32 * (std::f32::consts::TAU / 65536.0)
        };
        for row in s.units.slots.iter() {
            if let Some(v) = viewer {
                if self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row) {
                    continue;
                }
            }
            let bp = self.bp(row);
            // A refit is shown on the unit being refitted, not as a second unit inside it.
            if s.units.has_flag(row, crate::tables::flag::UPGRADE) {
                continue;
            }
            let site = self.structure_upgrade(row);
            let refit = s.orders.front(&s.units, row).filter(|o| {
                o.kind == crate::tables::OrderKind::Upgrade
                    && (bp.is_mobile() || bp.has(cat::EXTRACTOR))
            });
            let upgrade = site
                .or(refit.and_then(|_| s.units.row(s.units.build_target[row])))
                .map_or(0.0, |t| {
                    (s.units.build_progress[t] / self.bp(t).build_time)
                        .to_f32()
                        .clamp(0.002, 1.0)
                });
            let step = s.units.gait_step[row];
            let mut flags = s.units.flags[row];
            let contact = self.contact_flags(viewer, row);
            if contact & STATE_RADAR != 0 && flags & crate::tables::flag::IN_FACTORY != 0 {
                continue;
            }
            let (build, weld_id) = match site {
                Some(t) => {
                    // The structure rebuilds in place: same grow-from-weld as a fresh site.
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
            frame.units.push(UnitInstance {
                prev_pos: s.units.prev_pos[row].extend(s.units.prev_z[row]).to_f32(),
                prev_heading: s.units.prev_heading[row].to_radians_f32(),
                pos: s.units.pos[row].extend(s.units.z[row]).to_f32(),
                heading: s.units.heading[row].to_radians_f32(),
                blueprint: s.units.blueprint[row].0 as u32,
                owner_flags: s.units.owner[row] as u32
                    | (flags as u32) << 8
                    | if s.units.order_head[row] == crate::tables::NO_ORDER {
                        STATE_IDLE
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
                _pad: {
                    let level = s.units.veterancy[row];
                    let need = crate::veterancy_need(level);
                    let share = if level >= crate::VETERANCY_MAX {
                        0.0
                    } else {
                        (s.units.veterancy_progress[row] / need).to_f32()
                    };
                    UnitInstance::pack_veterancy(s.units.kills[row], level, share)
                },
                gait: [
                    (s.units.gait[row] & 0xF_FFFF) as f32 / 256.0,
                    step[0] as f32 / 256.0,
                    step[1] as f32 / 256.0,
                ],
                upgrade,
                arm_pitch: [
                    pitch(s.units.prev_arm_pitch[row][0]),
                    pitch(s.units.arm_pitch[row][0]),
                    pitch(s.units.prev_arm_pitch[row][1]),
                    pitch(s.units.arm_pitch[row][1]),
                ],
                prev_turret_yaw: s.units.prev_weapon_yaw[row][0].to_radians_f32(),
                weld: welds
                    .get(&weld_id)
                    .or_else(|| welds.get(&s.units.id(row).0))
                    .map(|(_, local)| *local)
                    .unwrap_or_else(|| {
                        if flags & crate::tables::flag::UNDER_CONSTRUCTION != 0 {
                            [0.0, 0.0, bp.height.to_f32() * 0.2]
                        } else {
                            [0.0; 3]
                        }
                    }),
            });
        }

        frame.projectiles.clear();
        let look = |blueprint: BlueprintId, weapon: u8| {
            let weapon = &self.blueprints.unit(blueprint).weapons[weapon as usize];
            (
                weapon.color as u32
                    | if weapon.missile {
                        PROJECTILE_MISSILE
                    } else {
                        0
                    },
                0.3 + weapon.damage.to_f32().sqrt() * 0.045,
            )
        };
        let muzzles: std::collections::HashSet<[i64; 3]> =
            self.muzzles.iter().map(|m| [m.x.0, m.y.0, m.z.0]).collect();
        let fresh = |from: FxVec3| {
            if muzzles.contains(&[from.x.0, from.y.0, from.z.0]) {
                PROJECTILE_FRESH
            } else {
                0
            }
        };
        for i in 0..s.projectiles.len() {
            let (color, size) = look(s.projectiles.blueprint[i], s.projectiles.weapon[i]);
            let from = s.projectiles.prev_pos[i];
            frame.projectiles.push(ProjectileInstance {
                prev_pos: from.to_f32(),
                color: color | fresh(from),
                pos: s.projectiles.pos[i].to_f32(),
                size,
            });
        }
        // Shots that landed this tick fly their last stretch, so a shell is seen
        // all the way in, and one fired at point-blank range is seen at all.
        for shot in &self.spent {
            let (color, size) = look(shot.blueprint, shot.weapon);
            let ends = ((shot.after.to_f32() * 255.0) as u32).clamp(1, 255);
            frame.projectiles.push(ProjectileInstance {
                prev_pos: shot.from.to_f32(),
                color: color | fresh(shot.from) | ends << PROJECTILE_ENDS_SHIFT,
                pos: shot.to.to_f32(),
                size,
            });
        }

        // Construction beams: from each mobile builder at work to the weld it prints from,
        // including engineers helping an upgrade assemble inside its parent.
        use crate::tables::flag;
        frame.build_sources.clear();
        for row in s.units.slots.iter() {
            let flags = s.units.flags[row];
            if flags & flag::BUILDING == 0 || !self.bp(row).is_mobile() {
                continue;
            }
            if viewer.is_some_and(|v| {
                self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row)
            }) {
                continue;
            }
            let Some(t) = self.print_target(row) else {
                continue;
            };
            let from = self.builder_emitter(row);
            let to = welds
                .get(&s.units.id(t).0)
                .map(|(world, _)| *world)
                .unwrap_or_else(|| self.weld_on(t, from.to_f32()).0);
            frame.build_sources.push((s.units.id(row).0, to));
            frame.projectiles.push(ProjectileInstance {
                prev_pos: from.to_f32(),
                color: COLOR_BUILD | PROJECTILE_BEAM,
                pos: to,
                size: self.bp(t).radius.to_f32(),
            });
        }

        self.write_reclaim_beams(viewer, &mut frame.beams, &mut frame.beam_sources);

        for row in s.wrecks.slots.iter() {
            if let (Some(v), true) = (viewer, s.fog_enabled) {
                if !self.fog.is_visible(s.wrecks.pos[row], self.team_mask(v)) {
                    continue;
                }
            }
            let pos = s.wrecks.pos[row].extend(s.wrecks.z[row]).to_f32();
            let heading = s.wrecks.heading[row].to_radians_f32();
            let bp = self.blueprints.unit(s.wrecks.blueprint[row]);
            frame.units.push(UnitInstance {
                prev_pos: pos,
                prev_heading: heading,
                pos,
                heading,
                blueprint: bp.id.0 as u32,
                owner_flags: KIND_WRECK,
                health: (s.wrecks.mass[row] / s.wrecks.mass_max[row]).to_f32(),
                build: 1.0,
                turret_yaw: 0.0,
                radius: bp.radius.to_f32(),
                unit_id: s.wrecks.slots.handle(row).0,
                _pad: 0,
                gait: [0.0; 3],
                upgrade: 0.0,
                arm_pitch: [0.0; 4],
                prev_turret_yaw: 0.0,
                weld: [0.0; 3],
            });
        }

        frame.stains.clear();
        for i in 0..s.stains.len() {
            frame.stains.push(StainInstance {
                pos: s.stains.pos[i].to_f32(),
                radius: s.stains.radius[i].to_f32(),
                strength_seed: s.stains.strength[i] as u32 | (s.stains.seed[i] as u32) << 8,
            });
        }

        frame.pads.clear();
        for i in 0..s.pads.len() {
            frame.pads.push(StainInstance {
                pos: s.pads.pos[i].to_f32(),
                radius: s.pads.radius[i].to_f32(),
                strength_seed: s.pads.packed[i],
            });
        }

        frame.events.clear();
        frame.events.extend(self.events.iter().cloned());

        frame.fog.clear();
        frame.fog_dims = self.fog.dims();
        if let (Some(v), true) = (viewer, s.fog_enabled) {
            let mask = self.team_mask(v);
            let (visible, explored) = (self.fog.visible_cells(), self.fog.explored_cells());
            frame.fog.reserve(visible.len() * 2);
            for i in 0..visible.len() {
                // Radar paints blips, not the ground: only vision lights a cell.
                frame.fog.push(if visible[i] & mask != 0 { 255 } else { 0 });
                frame
                    .fog
                    .push(if explored[i] & mask != 0 { 255 } else { 0 });
            }
        }

        frame.props_dead.clear();
        frame.props_dead.extend(
            s.props_dead
                .iter()
                .flat_map(|w| [*w as u32, (*w >> 32) as u32]),
        );
        frame.terrain_edits.clear();
        frame
            .terrain_edits
            .extend(s.terrain_edits.iter().map(|e| e.record()));
    }

    /// The order queues of `watch` (unit ids), for the interface. Ids that no
    /// longer resolve, and units `viewer` does not own, are left out.
    /// The order queues of the units in `watch` and, with `everyone` naming a player, of
    /// every mobile unit of theirs that has orders. `viewer` limits both to one owner.
    pub fn write_orders(
        &self,
        viewer: Option<u8>,
        watch: &[u32],
        everyone: Option<u8>,
        out: &mut Vec<UnitOrders>,
    ) {
        use crate::tables::OrderKind;
        let s = &self.state;
        out.clear();
        let watched = watch
            .iter()
            .filter_map(|&id| s.units.row(crate::Handle(id)));
        let rest = s.units.slots.iter().filter(|&row| {
            everyone == Some(s.units.owner[row])
                && s.units.is_active(row)
                && self.bp(row).is_mobile()
                && s.orders.front(&s.units, row).is_some()
                && !watch.contains(&s.units.id(row).0)
        });
        for row in watched.chain(rest.take(MAX_LISTED_UNITS)) {
            let id = s.units.id(row).0;
            if viewer.is_some_and(|v| v != s.units.owner[row]) {
                continue;
            }
            let orders = s
                .orders
                .iter(&s.units, row)
                .take(MAX_LISTED_ORDERS)
                .map(|o| {
                    let target = match o.kind {
                        OrderKind::Attack | OrderKind::Assist | OrderKind::ReclaimUnit => {
                            s.units.row(o.target).map(|r| s.units.pos[r])
                        }
                        OrderKind::Reclaim => {
                            s.wrecks.slots.resolve(o.target).map(|r| s.wrecks.pos[r])
                        }
                        OrderKind::Produce | OrderKind::Upgrade => Some(s.units.pos[row]),
                        _ => None,
                    };
                    QueuedOrder {
                        kind: o.kind,
                        pos: target.unwrap_or(o.pos).to_f32(),
                        at: o.pos,
                        blueprint: o.blueprint,
                    }
                })
                .collect();
            let progress = s.units.row(s.units.build_target[row]).map_or(0.0, |t| {
                (s.units.build_progress[t] / self.bp(t).build_time).to_f32()
            });
            out.push(UnitOrders {
                unit_id: id,
                orders,
                progress,
            });
        }
    }

    /// Every structure `player`'s builders have planned, begun ones left out: they are on the map.
    pub fn write_plans(&self, player: u8, out: &mut Vec<PlannedBuild>) {
        let units = &self.state.units;
        out.clear();
        out.extend(self.planned_sites(player).map(|(row, o)| PlannedBuild {
            unit_id: units.id(row).0,
            blueprint: o.blueprint,
            pos: o.pos.to_f32(),
            at: o.pos,
            heading: o.heading.to_radians_f32(),
        }));
    }

    /// Detection shared across the viewer's team.
    fn detects_for_team(&self, viewer: u8, row: usize) -> bool {
        !self.state.fog_enabled
            || self
                .fog
                .is_detected(self.state.units.pos[row], self.team_mask(viewer))
    }

    /// How the viewer should draw this enemy: radar-only, and whether vision
    /// has ever named it. Allies and observers get nothing extra.
    fn contact_flags(&self, viewer: Option<u8>, row: usize) -> u32 {
        let Some(v) = viewer else {
            return 0;
        };
        if !self.state.fog_enabled {
            return 0;
        }
        if !self.are_enemies(v, self.state.units.owner[row]) {
            return 0;
        }
        let mask = self.team_mask(v);
        let pos = self.state.units.pos[row];
        if self.fog.is_visible(pos, mask) {
            return 0;
        }
        let mut flags = STATE_RADAR;
        if !self
            .fog
            .is_identified(row, self.state.units.id(row).generation(), mask)
        {
            flags |= STATE_UNIDENTIFIED;
        }
        flags
    }
}

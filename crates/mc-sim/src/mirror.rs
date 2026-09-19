//! The render mirror: what the simulation publishes for presentation.
//!
//! Built once at the end of a tick on the sim side. Each record carries the
//! previous and the current tick's transform, so the GPU interpolates on its
//! own and the render thread never touches an entity. Floats are fine here:
//! nothing in this module is read back by the simulation.

use crate::tables::UnitId;
use crate::World;
use bytemuck::{Pod, Zeroable};
use mc_core::FxVec3;
use mc_data::{BlueprintId, WeaponColor};

#[derive(Clone, Debug)]
pub enum SimEvent {
    UnitCompleted { unit: UnitId, owner: u8 },
    UnitDied { pos: FxVec3, blueprint: BlueprintId, owner: u8 },
    ShotFired { pos: FxVec3, color: WeaponColor, owner: u8 },
    Impact { pos: FxVec3, splash: mc_core::Fx, color: WeaponColor },
    /// The terrain edit table grew (or was replaced by a snapshot).
    TerrainEdited,
    BuildRejected { player: u8 },
    PlayerDefeated { player: u8 },
    MatchOver { winner_team: u8 },
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
    pub _pad: u32,
}

/// `owner_flags` bit: the record is a wreck. `health` holds the share of mass left.
pub const KIND_WRECK: u32 = 1 << 31;
/// `owner_flags` bit: a map prop (set by the renderer for its static records).
pub const KIND_PROP: u32 = 1 << 30;
/// `owner_flags` bit: a placement preview added by the UI, not a real unit.
pub const KIND_GHOST: u32 = 1 << 29;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct ProjectileInstance {
    pub prev_pos: [f32; 3],
    /// 0 blue, 1 orange.
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
    pub stains: Vec<StainInstance>,
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
    /// Fills `frame` with this tick's mirror, reusing its allocations. Units the
    /// viewer cannot detect are left out, so fog is enforced before the GPU.
    /// `viewer = None` shows everything (observers, replays, tools).
    pub fn write_render_frame(&self, viewer: Option<u8>, frame: &mut RenderFrame) {
        let s = &self.state;
        frame.tick = s.tick;
        frame.units.clear();
        for row in s.units.slots.iter() {
            if let Some(v) = viewer {
                if self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row) {
                    continue;
                }
            }
            let bp = self.bp(row);
            frame.units.push(UnitInstance {
                prev_pos: s.units.prev_pos[row].extend(s.units.prev_z[row]).to_f32(),
                prev_heading: s.units.prev_heading[row].to_radians_f32(),
                pos: s.units.pos[row].extend(s.units.z[row]).to_f32(),
                heading: s.units.heading[row].to_radians_f32(),
                blueprint: s.units.blueprint[row].0 as u32,
                owner_flags: s.units.owner[row] as u32 | (s.units.flags[row] as u32) << 8,
                health: (s.units.health[row] / bp.health).to_f32(),
                build: (s.units.build_progress[row] / bp.build_time).to_f32(),
                turret_yaw: s.units.weapon_yaw[row][0].to_radians_f32(),
                radius: bp.radius.to_f32(),
                unit_id: s.units.id(row).0,
                _pad: 0,
            });
        }

        frame.projectiles.clear();
        for i in 0..s.projectiles.len() {
            let weapon = &self.blueprints.unit(s.projectiles.blueprint[i]).weapons[s.projectiles.weapon[i] as usize];
            frame.projectiles.push(ProjectileInstance {
                prev_pos: s.projectiles.prev_pos[i].to_f32(),
                color: weapon.color as u32,
                pos: s.projectiles.pos[i].to_f32(),
                size: 0.3 + weapon.damage.to_f32().sqrt() * 0.045,
            });
        }

        for row in s.wrecks.slots.iter() {
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

        frame.events.clear();
        frame.events.extend(self.events.iter().cloned());

        frame.fog.clear();
        frame.fog_dims = self.fog.dims();
        if let (Some(v), true) = (viewer, s.fog_enabled) {
            let mask = self.team_mask(v);
            let (visible, radar, explored) = (self.fog.visible_cells(), self.fog.radar_cells(), self.fog.explored_cells());
            frame.fog.reserve(visible.len() * 2);
            for i in 0..visible.len() {
                // Radar coverage shows as half-lit.
                let now = if visible[i] & mask != 0 { 255 } else if radar[i] & mask != 0 { 110 } else { 0 };
                frame.fog.push(now);
                frame.fog.push(if explored[i] & mask != 0 { 255 } else { 0 });
            }
        }

        frame.props_dead.clear();
        frame.props_dead.extend(s.props_dead.iter().flat_map(|w| [*w as u32, (*w >> 32) as u32]));
        frame.terrain_edits.clear();
        frame.terrain_edits.extend(s.terrain_edits.iter().map(|e| e.record()));
    }

    /// Detection shared across the viewer's team.
    fn detects_for_team(&self, viewer: u8, row: usize) -> bool {
        !self.state.fog_enabled || self.fog.is_detected(self.state.units.pos[row], self.team_mask(viewer))
    }
}

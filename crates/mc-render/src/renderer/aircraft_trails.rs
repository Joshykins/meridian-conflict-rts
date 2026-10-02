//! Aircraft engines: contrails and haze behind jets along the flown path (from the mesh's
//! nozzles in `models::aircraft_exhausts`, or else the exhausts its model records), and a VTOL's
//! pods (`models::Vtol`) burning where they point, tilted as the entity shader tilts them
//! (`models::vtol_tilt`). A pod jet is a blue drive plume out of each nozzle (`PUFF_THRUST`),
//! longer and hotter the harder the aircraft is being driven, with a glow at the mouth; a lift fan is
//! a wide blue field under its duct. Nothing is emitted for parked, hidden or unfinished
//! aircraft.

use super::capital_fx::{PUFF_THRUST, PUFF_THRUST_GLOW};
use super::{Renderer, KIND_WRECK, PUFF_CONTRAIL, PUFF_PLASMA, STATE_RADAR};
use crate::camera::Camera;
use crate::models::{self, Vtol};
use glam::Vec3;
use mc_sim::mirror::UnitInstance;

/// Per blueprint: its VTOL pods, if it has any (`Model::vtol`).
#[derive(Default)]
pub(super) struct VtolPods(pub(super) Vec<Option<Vtol>>);

/// Where a pod's nozzle is, and which way its gas leaves, in the hull's frame (+x
/// forward, +y left, +z up): the pod tilted `tilt` about its pivot.
fn pod_nozzle(vtol: &Vtol, at: [f32; 3], left: bool, tilt: f32) -> (Vec3, Vec3) {
    let pivot = Vec3::new(at[0], if left { at[1] } else { -at[1] }, at[2]);
    let (s, c) = tilt.sin_cos();
    let back = -vtol.nozzle[0];
    (
        pivot + Vec3::new(back * c, 0.0, back * s),
        Vec3::new(-c, 0.0, -s),
    )
}

impl Renderer {
    pub(super) fn aircraft_trails(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        let hidden = KIND_WRECK
            | STATE_RADAR
            | ((mc_sim::tables::flag::IN_FACTORY | mc_sim::tables::flag::UNDER_CONSTRUCTION)
                as u32)
                << 8;
        for u in units {
            if u.owner_flags & hidden != 0 || u.build < 1.0 {
                continue;
            }
            let bp = self
                .blueprints
                .unit(mc_data::BlueprintId(u.blueprint as u16));
            let vtol = self.vtol.0.get(u.blueprint as usize).copied().flatten();
            let fans = vtol.map_or(bp.visual.mesh == "reclaim_drone", |v| v.fans);
            let hover_flight = bp.motion.is_some_and(|m| m.hover);
            let assault = bp.visual.mesh == "assault_air";
            let capital = bp.is_capital_ship();
            // The ports are placed on the model as authored; a blueprint drawn bigger or
            // smaller moves them with the hull.
            let (authored_radius, authored_height) =
                models::authored_size(&bp.visual.mesh).unwrap_or((1.0, 1.0));
            let fit = Vec3::new(
                bp.radius.to_f32() / authored_radius,
                bp.radius.to_f32() / authored_radius,
                bp.height.to_f32() / authored_height,
            );
            // A jet's ports: the mesh's table where it has one, otherwise the exhausts its
            // model records (`MeshBuilder::add_exhaust`), already at the unit's size.
            let table = models::aircraft_exhausts(&bp.visual.mesh);
            let ports: Vec<Vec3> = if !table.is_empty() {
                table.iter().map(|p| Vec3::from(*p) * fit).collect()
            } else if bp
                .motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
            {
                self.heat_haze
                    .ports(u.blueprint)
                    .iter()
                    .map(|e| Vec3::from(e.at))
                    .collect()
            } else {
                Vec::new()
            };
            let transport_flight = bp.transport.is_some();
            if ports.is_empty() && vtol.is_none() {
                continue;
            }
            let from = Vec3::from(u.prev_pos);
            let to = Vec3::from(u.pos);
            let distance = from.distance(to);
            if capital {
                self.capital_drives(u, time, camera.focus.truncate());
                continue;
            }
            if transport_flight && to.z <= self.ground_height(to.truncate()) + 1.0 {
                continue;
            }
            // A hovering VTOL's engines run whether it moves or not; a jet that is
            // still is parked, and leaves nothing.
            let still = distance < 0.08;
            if (still && !hover_flight) || distance > 40.0 {
                continue;
            }
            if let Some(vtol) = vtol {
                self.vtol_jets(u, &vtol, fit, time);
                continue;
            }
            let pitch = (to.z - from.z)
                .atan2((to - from).truncate().length().max(2.0))
                .clamp(-0.2, 0.2);
            let samples = if still {
                1
            } else {
                (distance / 1.6).ceil().clamp(1.0, 16.0) as usize
            };
            let delta = (u.heading - u.prev_heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            for i in 0..samples {
                let t = (i as f32 + 0.5) / samples as f32;
                let yaw = u.prev_heading + delta * t;
                let bank = u._pad2[0] + (u._pad2[1] - u._pad2[0]) * t;
                let horizontal = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
                let pitch = if assault || transport_flight {
                    u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * t
                } else if hover_flight {
                    u.arm_pitch[2] + (u.arm_pitch[3] - u.arm_pitch[2]) * t
                } else {
                    pitch
                };
                let forward = horizontal * pitch.cos() + Vec3::Z * pitch.sin();
                let up = Vec3::Z * pitch.cos() - horizontal * pitch.sin();
                let left = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
                let rolled_left = left * bank.cos() + up * bank.sin();
                let rolled_up = up * bank.cos() - left * bank.sin();
                for &port in &ports {
                    let nozzle = -forward;
                    let at = from.lerp(to, t)
                        + forward * port.x
                        + rolled_left * port.y
                        + rolled_up * port.z;
                    let start = time + t * self.tick_seconds;
                    if hover_flight && fans {
                        // A lift fan's field: a small blue glow in the wash under the
                        // duct, hanging a moment where it was thrown.
                        let carried = (to - from) / self.tick_seconds.max(0.02);
                        let drift = nozzle * 3.0 + carried * 0.6;
                        self.push_puff(
                            PUFF_PLASMA,
                            at + nozzle * 0.35,
                            drift,
                            start,
                            0.3,
                            (0.55, 0.8),
                        );
                        continue;
                    }
                    if hover_flight {
                        if still {
                            continue;
                        }
                        let drift = nozzle * 0.5
                            + rolled_left * (self.scatter.signed() * 1.2)
                            + rolled_up * (self.scatter.signed() * 0.6);
                        let life = 1.7 + self.scatter.unit() * 0.4;
                        self.push_puff(PUFF_CONTRAIL, at, drift, start, life, (0.6, 3.6));
                        continue;
                    }
                    let drift = nozzle * 0.5
                        + rolled_left * (self.scatter.signed() * 1.5)
                        + rolled_up * (self.scatter.signed() * 0.8);
                    let life = 2.6 + self.scatter.unit() * 0.4;
                    self.push_puff(PUFF_CONTRAIL, at, drift, start, life, (1.0, 6.0));
                }
            }
        }
    }

    /// A VTOL's four pods this tick: each burns out of its nozzle, tilted with the pod.
    fn vtol_jets(&mut self, u: &UnitInstance, vtol: &Vtol, fit: Vec3, time: f32) {
        let from = Vec3::from(u.prev_pos);
        let to = Vec3::from(u.pos);
        let travel = to - from;
        let dt = self.tick_seconds.max(0.02);
        let carried = travel / dt;
        let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        // One slice per ~2 m flown, so the flames bend through a turn.
        let slices = (travel.length() / 2.0).ceil().clamp(1.0, 6.0) as usize;
        let mouth = vtol.nozzle[1] * fit.x;
        for i in 0..slices {
            let t = (i as f32 + 0.5) / slices as f32;
            let yaw = u.prev_heading + turn * t;
            let bank = u._pad2[0] + (u._pad2[1] - u._pad2[0]) * t;
            let pitch = u.arm_pitch[2] + (u.arm_pitch[3] - u.arm_pitch[2]) * t;
            let horizontal = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
            let forward = horizontal * pitch.cos() + Vec3::Z * pitch.sin();
            let up = Vec3::Z * pitch.cos() - horizontal * pitch.sin();
            let left = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
            let rolled_left = left * bank.cos() + up * bank.sin();
            let rolled_up = up * bank.cos() - left * bank.sin();
            let world = |v: Vec3| forward * v.x + rolled_left * v.y + rolled_up * v.z;
            let origin = from.lerp(to, t);
            let start = time + t * dt;
            // Driven hard (nose down, climbing, braking) the jets run hotter and longer.
            let throttle = (0.45
                + (pitch.abs() / 0.3).min(1.0) * 0.4
                + (travel.z / dt / 12.0).clamp(0.0, 0.3))
            .min(1.0);
            for (pivot, front) in vtol.pods() {
                for left_side in [true, false] {
                    let tilt = models::vtol_tilt(pitch, turn, left_side, front);
                    let (port, nozzle) = pod_nozzle(vtol, pivot, left_side, tilt);
                    let at = origin + world(port * fit);
                    let nozzle = world(nozzle);
                    if vtol.fans {
                        // A lift fan's field: a wide blue glow in the wash under the
                        // duct, hanging a moment where it was thrown.
                        self.push_puff(
                            PUFF_PLASMA,
                            at + nozzle * mouth * 0.8,
                            nozzle * 6.0 + carried * 0.6,
                            start,
                            0.3,
                            (mouth * 1.6, mouth * 2.3),
                        );
                        continue;
                    }
                    // The plume: a blue ribbon out of the mouth, the tip flickering.
                    let length = mouth * (5.0 + 7.0 * throttle) * (0.9 + 0.2 * self.scatter.unit());
                    self.push_drive(
                        PUFF_THRUST,
                        at - nozzle * 0.05,
                        nozzle * length,
                        start,
                        dt * 1.6 / slices as f32,
                        (mouth * 0.95, mouth * 0.95),
                        carried,
                        throttle,
                    );
                    if i == 0 {
                        // The mouth's glow, seen past the pod's rim from above.
                        self.push_drive(
                            PUFF_THRUST_GLOW,
                            at + nozzle * 0.25,
                            Vec3::ZERO,
                            start,
                            dt * 1.6,
                            (mouth * (2.2 + throttle), mouth * (2.4 + throttle)),
                            carried,
                            throttle,
                        );
                    }
                    if i == 0 && travel.truncate().length() > 1.0 {
                        // Under way, a thin haze behind.
                        let drift = nozzle * 0.5
                            + rolled_left * (self.scatter.signed() * 1.2)
                            + rolled_up * (self.scatter.signed() * 0.6);
                        let life = 1.7 + self.scatter.unit() * 0.4;
                        self.push_puff(
                            PUFF_CONTRAIL,
                            at + nozzle * length,
                            drift,
                            start,
                            life,
                            (0.6, 3.6),
                        );
                    }
                }
            }
        }
    }
}

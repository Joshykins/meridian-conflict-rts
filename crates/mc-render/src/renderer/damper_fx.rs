//! A warp dampener in the world (`RenderFrame::dampers`; its range ring for selection and
//! placement is the HUD's). A live one (finished and powered):
//!
//! - its field projector, at the top of the structure, glows crimson, swelling and
//!   fading slowly, and lights the structure round it;
//! - its field's edge stands on the ground as a faint crimson curtain a few tens of
//!   metres tall, in slow streaks drifting round the ring (`PUFF_VEIL`, laid in stretches
//!   near the eye every `VEIL_EVERY` seconds).
//!
//! An unpowered or unfinished one is dark. When it has a jump (a `WarpView` snagged in
//! transit or coming out, its mark in the field), the projector surges and throws a
//! crackling tether of crimson lightning out to the mark (`FADE_BEAM_TETHER`, through
//! `stun_fx`'s strokes), dragging the ship out; the warp's own rift is `warp_fx`'s.

use super::capital_fx::PUFF_LAMP;
use super::stun_fx::Flash;
use super::{Renderer, PUFF_SPARK};
use crate::camera::Camera;
use crate::gpu_consts::{fade_beam, puff};
use glam::{Vec2, Vec3};
use mc_sim::mirror::{DamperView, RenderFrame, WarpView};
use mc_sim::tables::WarpPhase;

/// The field's colour: crimson, a little blue in it (linear).
const FIELD: Vec3 = Vec3::new(1.0, 0.1, 0.3);
/// Seconds between layings of a stretch of the field's edge; each lives twice that.
const VEIL_EVERY: f32 = 2.0;
/// Metres of edge a curtain covers, and how tall it stands.
const VEIL_STEP: f32 = 110.0;
const VEIL_TALL: f32 = 42.0;
/// How bright the edge is (HDR, before its streaks and swell).
const VEIL_GLOW: f32 = 0.55;
/// Metres from the eye's focus past which the edge is not laid.
const VEIL_NEAR: f32 = 5000.0;
/// Where the projector sits: this share of the structure's height, over its middle.
const PROJECTOR: f32 = 0.95;
/// Height (m) of the dampener the projector's glow is sized for.
const REFERENCE_HEIGHT: f32 = 46.0;

/// The dampener nearest `to` whose live field reaches it.
fn holding(dampers: &[DamperView], to: [f32; 3]) -> Option<&DamperView> {
    let at = Vec2::new(to[0], to[1]);
    dampers
        .iter()
        .filter(|d| d.live && Vec2::new(d.pos[0], d.pos[1]).distance(at) <= d.radius)
        .min_by(|a, b| {
            let da = Vec2::new(a.pos[0], a.pos[1]).distance(at);
            let db = Vec2::new(b.pos[0], b.pos[1]).distance(at);
            da.total_cmp(&db)
        })
}

/// How hard a snagged jump pulls on the tether, 0 to 1: full through the transit, and as
/// the ship comes out, peaking as it is torn free and then let go.
fn tether_pull(w: &WarpView) -> f32 {
    match w.phase {
        WarpPhase::Transit => 1.0,
        WarpPhase::Emerge => {
            let f = w.ticks as f32 / w.length.max(1) as f32;
            if f < 0.8 {
                1.0 + 0.5 * f / 0.8
            } else {
                1.5 * (1.0 - (f - 0.8) / 0.2).max(0.0)
            }
        }
        _ => 0.0,
    }
}

impl Renderer {
    /// Once a tick (from `emp_tick`): the live dampeners' glow and field edge, and the
    /// tethers of the jumps they hold.
    pub(super) fn dampers_tick(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        let focus = camera.focus.truncate();
        let dt = self.tick_seconds.max(0.02);
        for d in &frame.dampers {
            if !d.live {
                continue;
            }
            let Some(top) = self.projector(frame, d) else {
                continue;
            };
            let s = (top.z - d.pos[2]) / (REFERENCE_HEIGHT * PROJECTOR);
            let pulse = 0.55 + 0.45 * (time * 1.4 + (d.unit_id % 17) as f32).sin();
            if top.truncate().distance(focus) < VEIL_NEAR + d.radius {
                self.push_drive(
                    PUFF_LAMP,
                    top,
                    FIELD * 5.0 * pulse,
                    time,
                    dt * 1.6,
                    (6.0 * s, 6.0 * s),
                    Vec3::ZERO,
                    0.0,
                );
                self.emp_fx.flashes.push(Flash {
                    pos: top,
                    start: time,
                    life: dt * 1.5,
                    color: FIELD * 900.0 * s * s * pulse,
                    range: 45.0 * s,
                    flicker: 0.0,
                });
            }
            self.lay_veil(d, time, focus);
        }
        for w in &frame.warps {
            if !w.dampened {
                continue;
            }
            let pull = tether_pull(w);
            let Some(d) = holding(&frame.dampers, w.to).copied() else {
                continue;
            };
            if pull <= 0.0 {
                continue;
            }
            if let Some(top) = self.projector(frame, &d) {
                self.tether(top, Vec3::from(w.to), pull, time);
            }
        }
    }

    /// Where a dampener's field projector is: over its middle, near the top.
    fn projector(&self, frame: &RenderFrame, d: &DamperView) -> Option<Vec3> {
        let u = frame.units.iter().find(|u| u.unit_id == d.unit_id)?;
        let bp = self
            .blueprints
            .unit(mc_data::BlueprintId(u.blueprint as u16));
        Some(Vec3::from(d.pos) + Vec3::Z * bp.height.to_f32() * PROJECTOR)
    }

    /// The field's edge near the eye, a stretch at a time, every `VEIL_EVERY` seconds.
    fn lay_veil(&mut self, d: &DamperView, time: f32, focus: Vec2) {
        let due = self
            .emp_fx
            .veils
            .iter()
            .find(|v| v.0 == d.unit_id)
            .is_none_or(|v| time - v.1 >= VEIL_EVERY || v.1 > time);
        if !due {
            return;
        }
        self.emp_fx.veils.retain(|v| v.0 != d.unit_id);
        self.emp_fx.veils.push((d.unit_id, time));
        let centre = Vec2::new(d.pos[0], d.pos[1]);
        let count =
            ((std::f32::consts::TAU * d.radius / VEIL_STEP).round() as usize).clamp(24, 160);
        let step = std::f32::consts::TAU / count as f32;
        let run = d.radius * step * 1.35;
        for i in 0..count {
            let a = i as f32 * step;
            let at = centre + Vec2::new(a.cos(), a.sin()) * d.radius;
            if at.distance(focus) > VEIL_NEAR {
                continue;
            }
            let along = Vec3::new(-a.sin(), a.cos(), 0.0) * run;
            let seed = self.scatter.unit();
            self.push_drive(
                puff::VEIL as f32,
                at.extend(0.0),
                along,
                time + seed * 0.3,
                VEIL_EVERY * 2.0,
                (VEIL_TALL, VEIL_TALL),
                FIELD,
                VEIL_GLOW,
            );
        }
    }

    /// One tick of a tether from the projector at `top` to the mark at `to`, `pull` hard.
    fn tether(&mut self, top: Vec3, to: Vec3, pull: f32, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let length = top.distance(to).max(1.0);
        let dir = (to - top) / length;
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::X);
        let up = side.cross(dir).normalize_or(Vec3::Z);
        let kinks = ((length / 45.0) as usize).clamp(10, 36);
        let strands = if pull > 1.0 { 3 } else { 2 };
        for _ in 0..strands {
            let start = time + self.scatter.unit() * dt;
            let life = 0.1 + self.scatter.unit() * 0.08;
            let width = (1.4 + 1.2 * self.scatter.unit()) * pull;
            self.emp_kinks(
                top,
                to,
                length * 0.025 * pull,
                side,
                up,
                start,
                life,
                width,
                kinks,
                fade_beam::TETHER,
            );
        }
        // Forks torn off along it.
        for _ in 0..(1 + (pull * 1.5) as usize) {
            let root = top.lerp(to, 0.15 + 0.7 * self.scatter.unit());
            let reach = length * (0.04 + 0.05 * self.scatter.unit());
            let tip = root + (side * self.scatter.signed() + up * self.scatter.signed()) * reach;
            let start = time + self.scatter.unit() * dt;
            self.emp_kinks(
                root,
                tip,
                reach * 0.3,
                side,
                up,
                start,
                0.08,
                0.9,
                4,
                fade_beam::TETHER,
            );
        }
        // The projector surging: a bigger, brighter flare, sparks thrown off it.
        let s = 1.0 + 0.4 * pull;
        self.push_drive(
            PUFF_LAMP,
            top,
            FIELD * 14.0 * s,
            time,
            dt * 1.6,
            (14.0 * s, 14.0 * s),
            Vec3::ZERO,
            1.0,
        );
        for _ in 0..3 {
            let vel = (side * self.scatter.signed() + up * self.scatter.unit() + dir * 0.5) * 18.0;
            let start = time + self.scatter.unit() * dt;
            self.push_puff(PUFF_SPARK, top, vel, start, 0.5, (1.2, 0.2));
        }
        for (at, range) in [(top, 90.0), (to, 160.0)] {
            self.emp_fx.flashes.push(Flash {
                pos: at,
                start: time,
                life: dt * 1.4,
                color: FIELD * 9000.0 * pull,
                range,
                flicker: 1.0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{holding, tether_pull};
    use mc_sim::mirror::{DamperView, WarpView};
    use mc_sim::tables::WarpPhase;

    fn damper(x: f32, live: bool) -> DamperView {
        DamperView {
            unit_id: x as u32,
            owner: 1,
            pos: [x, 0.0, 0.0],
            radius: 1600.0,
            live,
        }
    }

    #[test]
    fn a_jump_is_held_by_the_nearest_live_field_that_reaches_it() {
        let dampers = [
            damper(0.0, true),
            damper(900.0, false),
            damper(1500.0, true),
        ];
        assert_eq!(holding(&dampers, [1200.0, 0.0, 0.0]).unwrap().unit_id, 1500);
        assert_eq!(holding(&dampers, [-1000.0, 0.0, 0.0]).unwrap().unit_id, 0);
        assert!(holding(&dampers, [5000.0, 0.0, 0.0]).is_none());
    }

    #[test]
    fn the_tether_pulls_through_the_transit_and_lets_go_as_the_ship_comes_out() {
        let mut w = WarpView {
            unit_id: 1,
            owner: 0,
            blueprint: mc_data::BlueprintId(0),
            phase: WarpPhase::Transit,
            ticks: 5,
            length: 30,
            from: [0.0; 3],
            to: [0.0; 3],
            bearing: 0.0,
            radius: 100.0,
            dampened: true,
            charge: 1.0,
            aligned: true,
            energy: 1.0,
            draw: 1.0,
        };
        assert_eq!(tether_pull(&w), 1.0);
        w.phase = WarpPhase::Emerge;
        w.ticks = 23;
        assert!(tether_pull(&w) > 1.4);
        w.ticks = 30;
        assert!(tether_pull(&w) < 1e-3);
        w.phase = WarpPhase::Spool;
        assert_eq!(tether_pull(&w), 0.0);
    }
}

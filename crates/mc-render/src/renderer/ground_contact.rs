//! What units do to the ground they cross: the track marks and dust of tracked vehicles,
//! a walker's footprints (`footfalls.rs`), the downwash a hovercraft keeps under itself,
//! the red plasma under a craft on gravity lift (`lift_fx.rs`), the arcs on a running
//! reactor (`reactor_fx.rs`), and the wash of a hovering
//! aircraft's lift.

use super::{water_fx, Renderer, TrackMark, PUFF_DUST, STATE_RADAR, TRACK_MARK_LIFE};
use crate::camera::Camera;
use glam::Vec3;
use mc_sim::mirror::{UnitInstance, KIND_WRECK, STATE_UNPOWERED};

impl Renderer {
    /// Track marks and dust for the tracked vehicles that moved this tick,
    /// the prints a walker leaves as each foot comes down, and the downwash
    /// a hovercraft keeps under itself, near enough to the camera for any of
    /// them to be seen.
    pub(super) fn ground_contact(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        const FLAG_MOVING: u32 = (mc_sim::tables::flag::MOVING as u32) << 8;
        // Plants going up are drawn from however far away.
        self.reactor_aftermath(time);
        if camera.distance > 2500.0 {
            return;
        }
        let reach = camera.distance * 2.5 + 300.0;
        let dust = camera.distance < 900.0;
        let focus = camera.focus.truncate();
        let water = self.map_info.water_level.to_f32();
        let hidden_aircraft = STATE_RADAR
            | ((mc_sim::tables::flag::IN_FACTORY | mc_sim::tables::flag::UNDER_CONSTRUCTION)
                as u32)
                << 8;
        let previous = (self.effect_origin, self.effect_settings);
        self.lift_fx.new_tick();
        self.reactor_fx.new_tick();
        for u in units {
            self.effect_origin = Some(Vec3::from(u.pos));
            self.effect_settings = self
                .blueprints
                .units
                .get(u.blueprint as usize)
                .map_or_else(mc_data::EffectSettings::default, |bp| bp.visual.effects);
            if u.owner_flags & (KIND_WRECK | STATE_RADAR) != 0 {
                continue;
            }
            let moving = u.owner_flags & FLAG_MOVING != 0;
            let close = Vec3::from(u.pos).truncate().distance(focus) <= reach;
            if dust
                && close
                && self
                    .hover
                    .get(u.blueprint as usize)
                    .copied()
                    .unwrap_or(false)
                && u.pos[2] >= water
            {
                self.hover_downwash(u, time, moving);
            }
            if dust
                && close
                && u.build >= 1.0
                && u.owner_flags & hidden_aircraft == 0
                && self.lift_fx.lifts(u.blueprint)
            {
                self.plasma_lift(u, time, moving);
            }
            if close
                && u.build >= 1.0
                && u.owner_flags & (STATE_UNPOWERED | hidden_aircraft) == 0
                && self.reactor_fx.holds(u.blueprint)
            {
                self.reactor_arcs(u, time);
            }
            if dust && close && u.build >= 1.0 && u.owner_flags & hidden_aircraft == 0 {
                let bp = self
                    .blueprints
                    .unit(mc_data::BlueprintId(u.blueprint as u16));
                let hovering = bp
                    .motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Air && m.hover);
                let parked_transport = bp.transport.is_some()
                    && u.pos[2] <= self.ground_height(glam::Vec2::new(u.pos[0], u.pos[1])) + 1.0;
                if hovering && !parked_transport {
                    let radius = bp.radius.to_f32();
                    self.air_downwash(u, radius, time);
                }
            }
            // On a lift ship's ramp or deck it leaves no prints and throws no dirt.
            if !moving || u.status[0] & mc_sim::mirror::UNIT_ON_DECK != 0 {
                continue;
            }
            if let Some(legs) = self.legs.get(u.blueprint as usize).copied().flatten() {
                self.footfall(u, &legs, time, close, dust && close);
                continue;
            }
            let Some(treads) = self.treads.get(u.blueprint as usize).copied().flatten() else {
                continue;
            };
            let (from, to) = (Vec3::from(u.prev_pos), Vec3::from(u.pos));
            let moved = to.truncate().distance(from.truncate());
            // Afloat (a Mason riding the surface, sea under it) the tracks touch
            // nothing: no prints, no dust. Its wake comes from the water effects.
            if !(0.05..=40.0).contains(&moved)
                || to.truncate().distance(focus) > reach
                || to.z < water
                || self.ground_height(to.truncate()) < water - 0.15
            {
                continue;
            }
            // Laid as the hull passes over it: partway through this tick's glide.
            self.push_mark(TrackMark {
                start_xy: [from.x, from.y],
                end_xy: [to.x, to.y],
                half_gauge: treads.half_gauge,
                width: treads.width,
                start: time + self.tick_seconds * 0.5,
                life: TRACK_MARK_LIFE,
            });

            if !dust {
                continue;
            }
            let forward = Vec3::new(u.heading.cos(), u.heading.sin(), 0.0);
            let left = Vec3::new(-forward.y, forward.x, 0.0);
            let speed = moved / self.tick_seconds.max(0.02);
            for side in [-1.0f32, 1.0] {
                // Born somewhere along this tick's glide, behind the track that threw it.
                let k = self.scatter.unit();
                let at = from.lerp(to, k)
                    + forward * (treads.rear + 0.4)
                    + left
                        * (side * treads.half_gauge + self.scatter.signed() * treads.width * 0.3)
                    + Vec3::Z * 0.35;
                let vel = forward * (-speed * 0.12)
                    + left * (side * 0.8 + self.scatter.signed() * 0.6)
                    + Vec3::Z * (1.2 + self.scatter.unit() * 1.4);
                let grow = 1.8 + self.scatter.unit() * 1.6 + speed * 0.03;
                let life = 1.1 + self.scatter.unit() * 0.9;
                self.push_puff(
                    PUFF_DUST,
                    at,
                    vel,
                    time + k * self.tick_seconds,
                    life,
                    (0.7, grow),
                );
            }
        }
        (self.effect_origin, self.effect_settings) = previous;
    }

    /// Dust under a hovercraft: a cushion at rest, thrown back when it moves.
    fn hover_downwash(&mut self, u: &UnitInstance, time: f32, moving: bool) {
        let at = Vec3::from(u.pos);
        let forward = Vec3::new(u.heading.cos(), u.heading.sin(), 0.0);
        let left = Vec3::new(-forward.y, forward.x, 0.0);
        let r = u.radius * 0.55;
        let n = if moving { 3 } else { 1 };
        let speed = if moving {
            Vec3::from(u.pos)
                .truncate()
                .distance(Vec3::from(u.prev_pos).truncate())
                / self.tick_seconds.max(0.02)
        } else {
            0.0
        };
        for _ in 0..n {
            let k = self.scatter.unit();
            let pos = at
                + forward * (self.scatter.signed() * r * 0.85)
                + left * (self.scatter.signed() * r)
                + Vec3::Z * 0.12;
            let vel = forward * (-speed * 0.16)
                + left * (self.scatter.signed() * if moving { 1.1 } else { 0.35 })
                + Vec3::Z * (0.45 + self.scatter.unit() * 0.7);
            let life = 0.65 + self.scatter.unit() * 0.45;
            self.push_puff(
                PUFF_DUST,
                pos,
                vel,
                time + k * self.tick_seconds,
                life,
                (0.4, 1.35 + speed * 0.02),
            );
        }
    }

    /// The wash of a hovering aircraft's lift on the ground under it: a ring of
    /// dust driven outward, spray over water, stronger the lower it hangs. The
    /// Osprey works at 22 m and raises a storm; the Kestrel at 65 m only stirs
    /// the grass. Nothing from high up.
    fn air_downwash(&mut self, u: &UnitInstance, radius: f32, time: f32) {
        let at = Vec3::from(u.pos);
        let ground = self.ground_height(at.truncate());
        let water = self.map_info.water_level.to_f32();
        let surface = ground.max(water);
        let height = at.z - surface;
        let strength = (1.0 - height / 95.0).clamp(0.0, 1.0);
        if strength <= 0.05 {
            return;
        }
        let wet = ground < water - 0.2;
        let scale = (radius / 8.0).clamp(0.5, 1.5);
        let ring = radius * (0.7 + height * 0.025);
        let n = 3 + (strength * 6.0) as usize;
        for _ in 0..n {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let r = ring * (0.35 + self.scatter.unit() * 0.75);
            let pos =
                Vec3::new(at.x, at.y, surface) + out * r + Vec3::Z * if wet { 0.5 } else { 0.35 };
            // Driven out along the ground, rolling up a little at the ring's edge.
            let vel = out * (4.0 + 10.0 * strength + self.scatter.unit() * 3.0)
                + Vec3::Z * (0.5 + self.scatter.unit() * 1.2 * strength);
            let life = 1.2 + self.scatter.unit() * 0.8 + strength * 0.8;
            let kind = if wet { water_fx::PUFF_SPRAY } else { PUFF_DUST };
            let start = time + self.scatter.unit() * self.tick_seconds;
            self.push_puff(
                kind,
                pos,
                vel,
                start,
                life,
                (
                    (1.0 + 1.2 * strength) * scale,
                    (3.0 + 3.5 * strength) * scale,
                ),
            );
        }
    }
}

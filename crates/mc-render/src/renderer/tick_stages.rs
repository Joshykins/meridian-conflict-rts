//! A tick's render work that can wait a frame or few (`Renderer::tick_stages`).
//!
//! What a new tick brings has to be drawn at once (its units, its shots); the effects
//! it starts (sparks, trails, smoke, the sky's wakes) can be laid a frame or three
//! later, timed from the tick's start all the same. With a hundred thousand units that
//! work was tens of milliseconds, all on the frame the tick arrived in, a hitch on
//! every tick; spread a stage a frame over the frames after it, it is a fraction of each.
//!
//! The weapons' effects are not spread: what they draw (beams, strokes, trails) is
//! appended to the projectile list after the tick's shots, which the tick's own frame
//! writes afresh. Written a frame later, they were drawn only from that frame to the
//! next tick (beams flickered); written with the shots, they are drawn all tick long.

use super::*;

/// The stages, in the order they run: the first `ON_TICK` on the tick's own frame, then
/// one a frame.
#[derive(Clone, Copy)]
enum Stage {
    Events,
    WeaponFx,
    Trails,
    UnitFx,
    Smoke,
    Sky,
}

const STAGES: [Stage; 6] = [
    Stage::Events,
    Stage::WeaponFx,
    Stage::Trails,
    Stage::UnitFx,
    Stage::Smoke,
    Stage::Sky,
];

/// The stages the tick's own frame runs: its hits, and the weapons' effects, which go
/// in the projectile list after its shots (module comment).
const ON_TICK: usize = 2;

/// Where the newest tick's stages have got to, and when it began.
pub(super) struct TickStages {
    next: usize,
    start: f32,
}

impl Default for TickStages {
    fn default() -> Self {
        TickStages {
            next: STAGES.len(),
            start: 0.0,
        }
    }
}

impl Renderer {
    /// After the frame's upload: on the frame a tick arrives (`sim`), its stages are set
    /// going and the first run; on each frame after, one is run on `latest`, the mirror
    /// the game holds. A
    /// caller that holds none (`latest` is `None`) has them all run at once, as does a
    /// tick that arrives before the last one's are done: the last one's mirror is gone,
    /// and this one's work must not fall behind.
    pub(super) fn tick_stages(
        &mut self,
        sim: Option<&RenderFrame>,
        latest: Option<&RenderFrame>,
        tick_start: f32,
        camera: &Camera,
    ) {
        if let Some(frame) = sim {
            let behind = self.tick_stages.next < STAGES.len();
            self.tick_stages = TickStages {
                next: 0,
                start: tick_start,
            };
            if behind || latest.is_none() {
                while self.tick_stages.next < STAGES.len() {
                    self.tick_stage(frame, camera);
                }
            } else {
                // The tick's hits and deaths and its weapons' effects go with the tick
                // itself; at 60 frames a second and 10 ticks, the rest have the five
                // frames after it.
                while self.tick_stages.next < ON_TICK {
                    self.tick_stage(frame, camera);
                }
            }
        } else if let Some(frame) = latest {
            if self.tick_stages.next < STAGES.len() {
                self.tick_stage(frame, camera);
            }
        }
    }

    fn tick_stage(&mut self, frame: &RenderFrame, camera: &Camera) {
        let _t = mc_core::perf_span!("cpu.tick_stage");
        let stage = STAGES[self.tick_stages.next];
        self.tick_stages.next += 1;
        let time = self.tick_stages.start;
        let units = &frame.units[..frame.units.len().min(MAX_SIM_ENTITIES)];
        let projectiles = &frame.projectiles[..frame.projectiles.len().min(MAX_PROJECTILES)];
        match stage {
            Stage::Events => {
                let part = mc_core::perf_span!("cpu.upload.events");
                let pre = mc_core::perf_span!("ev.pre");
                self.construction(frame, time, camera);
                self.nanite_site_glints(frame, time, camera.eye());
                // A blast is known before the trees it kills are looked at (`nuke_fx::tree_fate`).
                // (The Progenitor's unmaking is not a blast of fire: `progenitor_event` draws it.)
                for event in &frame.events {
                    if matches!(event, SimEvent::NuclearDetonation { look, .. }
                        if *look != mc_data::strategic::StrategicLook::Progenitor)
                    {
                        self.nuke_event(event, time);
                    }
                }
                self.tree_fires(frame, time, camera);
                self.city_events(frame, time, camera);
                self.trample_trees(frame, time);
                self.clear_lots(frame, time);
                // Before the events: a Lance's strike and a shell's landing read this tick's Shapers.
                self.progenitor_tick(frame, time);
                drop(pre);
                // A shot, a hit or a charged shell's discharge makes a flash and sparks gone in
                // under a second. Far past what the camera shows they are out of sight, and of a
                // sky-wide air war's tens of thousands a tick only the nearest `SMALL_EVENTS`
                // are drawn (a deliberate cap: they are cosmetic, and the shots themselves are
                // drawn whatever). Their craters and the clouds they stir are still made.
                let reach = camera.distance * 3.0 + 1500.0;
                let small: Vec<(f32, usize)> = frame
                    .events
                    .iter()
                    .enumerate()
                    .filter_map(|(i, event)| match event {
                        SimEvent::ShotFired { pos, .. }
                        | SimEvent::Impact { pos, .. }
                        | SimEvent::ShellDischarge { to: pos, .. } => {
                            Some((Vec3::from(pos.to_f32()).distance(camera.focus), i))
                        }
                        _ => None,
                    })
                    .collect();
                let mut drawn = vec![true; frame.events.len()];
                for &(d, i) in &small {
                    drawn[i] = d <= reach;
                }
                let mut near: Vec<(f32, usize)> =
                    small.into_iter().filter(|&(d, _)| d <= reach).collect();
                if near.len() > SMALL_EVENTS {
                    near.select_nth_unstable_by(SMALL_EVENTS, |a, b| a.0.total_cmp(&b.0));
                    for &(_, i) in &near[SMALL_EVENTS..] {
                        drawn[i] = false;
                    }
                }
                {
                    let _t = mc_core::perf_span!("ev.effects");
                    for (event, drawn) in frame.events.iter().zip(drawn) {
                        if drawn {
                            self.effects_of(event, time);
                        }
                    }
                }
                {
                    let _t = mc_core::perf_span!("ev.clouds_craters");
                    for event in &frame.events {
                        self.stir_clouds(event, time);
                        self.impact_crater(event, time);
                    }
                }
                drop(part);
            }
            Stage::UnitFx => {
                let part = mc_core::perf_span!("cpu.upload.unit_fx");
                {
                    let _t = mc_core::perf_span!("fx.ground_contact");
                    self.ground_contact(units, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("fx.mine_blows");
                    self.mine_blows(units, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("fx.heat_haze");
                    self.heat_haze.note(units, self.tick_seconds);
                }
                {
                    let _t = mc_core::perf_span!("fx.wrecks");
                    self.turret_landings(units, time, camera);
                    self.wreck_finish(units, &frame.events, time, camera);
                }
                drop(part);
            }
            Stage::Trails => {
                let _t = mc_core::perf_span!("fx.aircraft_trails");
                self.aircraft_trails(units, time, camera);
            }
            Stage::Smoke => {
                {
                    let _t = mc_core::perf_span!("fx.crash_trails");
                    self.aircraft_crash_trails(units, time, camera);
                }
                let _t = mc_core::perf_span!("fx.smoke");
                self.damage_smoke(units, time, camera);
                self.wreck_smoke(units, time, camera);
            }
            Stage::WeaponFx => {
                let _part = mc_core::perf_span!("cpu.upload.weapon_fx");
                // Their strokes are bore strokes (`write_bore_strokes`, below).
                {
                    let _t = mc_core::perf_span!("fx.storms");
                    self.storm_beams(time);
                    self.storms_tick(time);
                    self.storm_clouds_tick(&frame.clouds, frame.tick, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.ground_fires");
                    self.ground_fires(&frame.fires, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.flush_rail_misses");
                    self.flush_rail_misses(time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.rail_wakes");
                    self.rail_wakes(projectiles, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.write_fade_beams");
                    self.write_fade_beams(time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.write_plasma_fx");
                    self.write_plasma_fx(units, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.regency_guns_tick");
                    self.regency_guns_tick(units, &frame.houses, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.regency_trails");
                    self.regency_trails(projectiles, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.gravitic_tick");
                    self.gravitic_tick(projectiles, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.excavation_tick");
                    self.excavation_tick(units, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.star_core_tick");
                    self.star_core_tick(units, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.orb_tick");
                    self.orb_tick(units, frame.tick, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.nanite_repair_tick");
                    self.nanite_repair_tick(frame, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.elevator_tick");
                    self.elevator_tick(units, &frame.events, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.lance_core_tick");
                    self.lance_core_tick(units, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.bolt_rifle_tick");
                    self.bolt_rifle_tick(units, &frame.houses, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.arc_howitzer_tick");
                    self.arc_howitzer_tick(units, &frame.houses, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.write_bore_strokes");
                    self.write_bore_strokes(time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.heavy_rail_tick");
                    self.heavy_rail_tick(units, &frame.houses, projectiles, time);
                }
                // Before the strokes are written (`emp_tick`): its lightning is among them.
                {
                    let _t = mc_core::perf_span!("wfx.gravitic_engine_tick");
                    self.gravitic_engine_tick(frame, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.emp_tick");
                    self.emp_tick(frame, units, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.great_gun_tick");
                    self.great_gun_tick(projectiles, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.missile_trails");
                    self.missile_trails(projectiles, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.nuke_tick");
                    self.nuke_tick(frame, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.write_regency_trails");
                    self.write_regency_trails(time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.lens_tick");
                    self.lens_tick(frame, time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.surge_tick");
                    self.surge_tick(time);
                }
                {
                    let _t = mc_core::perf_span!("wfx.stream_bursts");
                    self.stream_bursts(projectiles, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.sea_tick");
                    self.sea_tick(units, projectiles, time, camera);
                }
                {
                    let _t = mc_core::perf_span!("wfx.warp_tick");
                    self.warp_tick(frame, time, camera);
                }
            }
            Stage::Sky => {
                let part = mc_core::perf_span!("cpu.upload.sky");
                self.sky.set_units(units.iter().map(|u| {
                    let bp = self
                        .blueprints
                        .unit(mc_data::BlueprintId(u.blueprint as u16));
                    bp.transport
                        .is_none()
                        .then_some(glam::Vec2::new(u.pos[0], u.pos[1]))
                }));
                let flyers: Vec<_> = units
                    .iter()
                    .filter(|u| {
                        stirs_clouds(
                            u,
                            self.blueprints
                                .unit(mc_data::BlueprintId(u.blueprint as u16)),
                        )
                    })
                    .map(|u| {
                        let r = self
                            .blueprints
                            .unit(mc_data::BlueprintId(u.blueprint as u16))
                            .radius
                            .to_f32();
                        (Vec3::from(u.prev_pos), Vec3::from(u.pos), r)
                    })
                    .collect();
                self.sky.set_flyers(flyers.into_iter());
                drop(part);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The weapons' beams and strokes go in the projectile list the tick's own frame
    /// writes; run on a later frame, they were drawn only from then to the next tick.
    #[test]
    fn weapon_effects_run_on_the_ticks_own_frame() {
        assert!(STAGES[..ON_TICK]
            .iter()
            .any(|s| matches!(s, Stage::WeaponFx)));
    }
}

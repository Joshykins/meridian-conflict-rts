//! How the Edict (mc-sim `edict.rs`, the Regent's main gun) is heard. Every sound is the
//! ship's own, named in its blueprint's `edict.sounds`:
//!
//! - `gather`, held at the firing point while it gathers (`held.rs`): as long as the
//!   gather, joining where it has got to, fading if it stalls or is let go; `peak` held
//!   over its last 2.5 s.
//! - `fire` at the firing point and `impact` at the mark when it fires, each crossfaded
//!   with its `_far` form by how far from the view it is; both carry over the whole map at
//!   once. `cancel` where a gather is let go.
//! - **The battle quiets:** from the gather's start until a little after the shot, every
//!   other world sound is held down (`Audio::duck_world`), the Edict's own spared, for a
//!   viewer who can see it (`RenderFrame::edicts` is only what the viewer sees), less the
//!   farther off it is.

use super::held::Held;
use super::Audio;
use glam::Vec3;
use mc_data::{Blueprints, SoundId, SoundLibrary};
use mc_sim::mirror::{EdictStage, EdictView};
use mc_sim::{RenderFrame, SimEvent};
use std::collections::HashMap;

/// A ship's Edict sounds, from its blueprint's `edict.sounds`.
#[derive(Clone, Copy, Default)]
struct Ids {
    gather: Option<SoundId>,
    peak: Option<SoundId>,
    fire: Option<SoundId>,
    fire_far: Option<SoundId>,
    impact: Option<SoundId>,
    impact_far: Option<SoundId>,
    cancel: Option<SoundId>,
}

impl Ids {
    fn of(library: &SoundLibrary, blueprints: &Blueprints, blueprint: usize) -> Ids {
        let Some(s) = blueprints
            .units
            .get(blueprint)
            .and_then(|bp| bp.edict.as_ref())
            .map(|e| &e.sounds)
        else {
            return Ids::default();
        };
        let id = |name: &String| library.id_of(name);
        Ids {
            gather: id(&s.gather),
            peak: id(&s.peak),
            fire: id(&s.fire),
            fire_far: id(&s.fire_far),
            impact: id(&s.impact),
            impact_far: id(&s.impact_far),
            cancel: id(&s.cancel),
        }
    }

    fn all(&self) -> [Option<SoundId>; 7] {
        [
            self.gather,
            self.peak,
            self.fire,
            self.fire_far,
            self.impact,
            self.impact_far,
            self.cancel,
        ]
    }
}

/// Seconds of the peak, laid over the gather's end (its recipe's length).
const PEAK: f32 = 2.5;
/// How deep the battle is held down while it gathers and just after the shot, and the
/// seconds after the shot it stays down.
const DUCK: f32 = 0.8;
const DUCK_AFTER: f32 = 3.0;
/// Metres from the view where the duck starts to give, and where it is down to its least.
const DUCK_NEAR: f32 = 6000.0;
const DUCK_FAR: f32 = 24000.0;
const DUCK_LEAST: f32 = 0.3;
/// How the gather carries: (level at any distance, metres at half of it).
const GATHER_FAR: (f32, f32) = (0.3, 6000.0);
/// Metres from the view at which a near sound and its far form are heard alike.
const CROSSOVER: f32 = 3500.0;
/// Ticks a ship's blueprint is remembered after its Edict was last seen, so a gather let
/// go as the ship dies still finds its sounds.
const REMEMBER: u64 = 600;

/// Keys of the held sounds (`Held::key`): the ship's unit id with this tag over it.
const GATHER_KEY: u64 = 3 << 32;
const PEAK_KEY: u64 = 4 << 32;

#[derive(Default)]
pub(crate) struct EdictSounds {
    ids: HashMap<u32, Ids>,
    /// Ships seen at work: unit id to (blueprint, the tick last seen).
    ships: HashMap<u32, (u32, u64)>,
    tick: u64,
}

impl EdictSounds {
    /// One sim tick: plays the one-shots, asks the battle's duck, and returns the held
    /// sounds wanted, for the caller to hand `Audio::set_held` with its own. `hear` is the
    /// game's (gain, pan) for a place; `focus` the ground point the camera looks at.
    pub(crate) fn tick(
        &mut self,
        frame: &RenderFrame,
        blueprints: &Blueprints,
        (library, fresh): (&SoundLibrary, bool),
        audio: &Audio,
        focus: Vec3,
        hear: &impl Fn(Vec3) -> (f32, f32),
    ) -> Vec<Held> {
        self.tick += 1;
        if fresh {
            self.ids.clear();
        }
        let now = self.tick;
        for e in &frame.edicts {
            self.ships.insert(e.unit_id, (e.blueprint.0 as u32, now));
        }
        self.ships.retain(|_, s| now - s.1 < REMEMBER);
        let ids_cache = &mut self.ids;
        let mut ids = |blueprint: u32| -> Ids {
            *ids_cache
                .entry(blueprint)
                .or_insert_with(|| Ids::of(library, blueprints, blueprint as usize))
        };
        let away = |at: Vec3| (at - focus).truncate().length();

        for event in &frame.events {
            match event {
                SimEvent::EdictFired {
                    blueprint,
                    from,
                    mark,
                    ..
                } => {
                    let i = ids(blueprint.0 as u32);
                    let (from, mark) = (Vec3::from(from.to_f32()), Vec3::from(mark.to_f32()));
                    play_pair(audio, i.fire, i.fire_far, from, away(from), hear);
                    play_pair(audio, i.impact, i.impact_far, mark, away(mark), hear);
                }
                SimEvent::EdictCancelled { unit, from } => {
                    let Some(&(b, _)) = self.ships.get(&unit.0) else {
                        continue;
                    };
                    if let Some(sound) = ids(b).cancel {
                        let at = Vec3::from(from.to_f32());
                        let (gain, pan) = hear(at);
                        audio.play_world(
                            sound,
                            gain.max(far_level(GATHER_FAR, away(at))),
                            pan,
                            1.0,
                        );
                    }
                }
                _ => {}
            }
        }

        let (held, depth) = wanted(
            &frame.edicts,
            &mut ids,
            |at| {
                let (gain, pan) = hear(at);
                (gain.max(far_level(GATHER_FAR, away(at))).min(1.0), pan)
            },
            |at| duck_reach(away(at)),
        );
        if depth > 0.0 {
            let spared: Vec<SoundId> = frame
                .edicts
                .iter()
                .flat_map(|e| ids(e.blueprint.0 as u32).all())
                .flatten()
                .collect();
            audio.duck_world(depth, &spared);
        }
        held
    }
}

/// A far-carrying sound's level `d` metres from the view.
fn far_level((level, half): (f32, f32), d: f32) -> f32 {
    level / (1.0 + (d / half).powi(2))
}

/// How much of the duck a viewer `d` metres from the Edict feels.
fn duck_reach(d: f32) -> f32 {
    let k = ((d - DUCK_NEAR) / (DUCK_FAR - DUCK_NEAR)).clamp(0.0, 1.0);
    1.0 - (1.0 - DUCK_LEAST) * k
}

/// A sound and its far form at `at`, `d` metres from the view: crossfaded at
/// `CROSSOVER`, carrying over the whole map.
fn play_pair(
    audio: &Audio,
    near: Option<SoundId>,
    far: Option<SoundId>,
    at: Vec3,
    d: f32,
    hear: &impl Fn(Vec3) -> (f32, f32),
) {
    let (_, pan) = hear(at);
    let w = 1.0 / (1.0 + (d / CROSSOVER).powi(2));
    let carry = 0.35 + 0.65 / (1.0 + (d / 12000.0).powi(2));
    if let Some(s) = near {
        audio.play_world(s, (carry * w).min(1.0), pan, 1.0);
    }
    if let Some(s) = far {
        audio.play_world(s, (carry * (1.0 - w)).min(1.0), pan * 0.6, 1.0);
    }
}

/// What every Edict at work wants: its held gather and peak, and how deep the battle is
/// held down (0..1). `hear` places a point; `reach` is the share of the duck felt there.
fn wanted(
    edicts: &[EdictView],
    mut ids: impl FnMut(u32) -> Ids,
    hear: impl Fn(Vec3) -> (f32, f32),
    reach: impl Fn(Vec3) -> f32,
) -> (Vec<Held>, f32) {
    let mut held = Vec::new();
    let mut depth: f32 = 0.0;
    for e in edicts {
        let i = ids(e.blueprint.0 as u32);
        let from = Vec3::from(e.from);
        match e.stage {
            EdictStage::Gathering => {
                depth = depth.max(DUCK * reach(from));
                if !e.powered {
                    continue;
                }
                let (gain, pan) = hear(from);
                if let Some(sound) = i.gather {
                    held.push(Held {
                        key: GATHER_KEY | e.unit_id as u64,
                        sound,
                        gain,
                        pan,
                        from: e.seconds,
                    });
                }
                if let Some(sound) = i.peak.filter(|_| e.left <= PEAK) {
                    held.push(Held {
                        key: PEAK_KEY | e.unit_id as u64,
                        sound,
                        gain,
                        pan,
                        from: PEAK - e.left,
                    });
                }
            }
            EdictStage::Fired if e.seconds < DUCK_AFTER.max(e.quiet) => {
                depth = depth.max(DUCK * reach(Vec3::from(e.mark)).max(reach(from)));
            }
            EdictStage::Fired => {}
        }
    }
    (held, depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_data::BlueprintId;

    fn view(stage: EdictStage, powered: bool, seconds: f32, left: f32) -> EdictView {
        EdictView {
            unit_id: 7,
            owner: 0,
            blueprint: BlueprintId(0),
            stage,
            progress: 0.5,
            seconds,
            left,
            from: [0.0, 0.0, 1500.0],
            along: 1450.0,
            mark: [5000.0, 0.0, 0.0],
            core: 300.0,
            radius: 1200.0,
            wave: 1200.0,
            wave_reach: 0.0,
            wave_life: 10.0,
            quiet: 3.0,
            powered,
        }
    }

    fn ids(_: u32) -> Ids {
        Ids {
            gather: Some(SoundId(1)),
            peak: Some(SoundId(2)),
            ..Ids::default()
        }
    }

    fn hear(_: Vec3) -> (f32, f32) {
        (0.5, 0.0)
    }

    #[test]
    fn a_gather_is_held_from_where_it_has_got_to_and_quiets_the_battle() {
        let (held, depth) = wanted(
            &[view(EdictStage::Gathering, true, 6.0, 9.0)],
            ids,
            hear,
            |_| 1.0,
        );
        assert_eq!(held.len(), 1, "no peak yet");
        assert_eq!((held[0].sound, held[0].from), (SoundId(1), 6.0));
        assert_eq!(depth, DUCK);
    }

    #[test]
    fn the_peak_joins_over_the_last_seconds() {
        let (held, _) = wanted(
            &[view(EdictStage::Gathering, true, 14.0, 1.0)],
            ids,
            hear,
            |_| 1.0,
        );
        let peak = held
            .iter()
            .find(|h| h.sound == SoundId(2))
            .expect("the peak");
        assert!((peak.from - 1.5).abs() < 1e-4);
        assert_eq!(peak.key, PEAK_KEY | 7);
    }

    #[test]
    fn a_stalled_gather_goes_quiet_but_the_battle_stays_down() {
        let (held, depth) = wanted(
            &[view(EdictStage::Gathering, false, 6.0, 9.0)],
            ids,
            hear,
            |_| 1.0,
        );
        assert!(held.is_empty());
        assert_eq!(depth, DUCK);
    }

    #[test]
    fn the_battle_comes_back_after_the_shot() {
        let (_, during) = wanted(
            &[view(EdictStage::Fired, true, 1.0, 140.0)],
            ids,
            hear,
            |_| 1.0,
        );
        let (_, after) = wanted(
            &[view(EdictStage::Fired, true, 4.0, 140.0)],
            ids,
            hear,
            |_| 1.0,
        );
        assert_eq!(during, DUCK);
        assert_eq!(after, 0.0);
    }

    #[test]
    fn the_duck_gives_with_distance() {
        assert_eq!(duck_reach(1000.0), 1.0);
        assert!((duck_reach(50_000.0) - DUCK_LEAST).abs() < 1e-6);
        assert!(duck_reach(15_000.0) < 1.0 && duck_reach(15_000.0) > DUCK_LEAST);
    }
}

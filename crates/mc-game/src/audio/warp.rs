//! How warp is heard (mc-sim `warp.rs`, recipes in `data/sounds/warp.ron`):
//!
//! - One-shots on the sim's events: the drive engaging (`WarpSpooling`), the jump
//!   (`WarpJumped`, torn when dampened), a snag mid-transit (`WarpSnagged`), the exit
//!   (`WarpArrived`, violent when dampened). A dampener that catches a jump surges.
//! - Loops off the frame: the charge climbing in pitch and level with `WarpView::charge`
//!   (so a slow charge on a starved grid keeps rising), arcs thickening over its second
//!   half, the rift at the exit point swelling toward the moment the ship comes out, a
//!   stunned hull's arcs following its stun, and a live dampener's low field hum.
//! - The systems-reboot gesture as a stun starts to wear off.
//!
//! A ship is heard by its size, never its tier: lower and louder the bigger its hull.
//! Each sound is looked up per blueprint as `<moving>_<name>` (its `moving` sound's
//! name), falling back to `<name>`, so a ship may carry its own.

use super::Audio;
use glam::Vec3;
use mc_data::{Blueprints, SoundId, SoundLibrary, UnitBlueprint};
use mc_sim::mirror::{DamperView, KIND_WRECK};
use mc_sim::tables::WarpPhase;
use mc_sim::{RenderFrame, SimEvent};
use std::collections::HashMap;

/// A stun at or over this is full; the first tick under it, the systems start back.
const STUN_FULL: f32 = 0.999;

#[derive(Clone, Copy, Default)]
struct Ids {
    spool: Option<SoundId>,
    charge: Option<SoundId>,
    arcs: Option<SoundId>,
    jump: Option<SoundId>,
    jump_damped: Option<SoundId>,
    snag: Option<SoundId>,
    rift: Option<SoundId>,
    rift_damped: Option<SoundId>,
    arrive: Option<SoundId>,
    arrive_damped: Option<SoundId>,
    stun_arcs: Option<SoundId>,
    reboot: Option<SoundId>,
}

impl Ids {
    fn of(library: &SoundLibrary, bp: &UnitBlueprint) -> Ids {
        let own = bp.sounds.moving.as_deref();
        let part = |name: &str| {
            own.and_then(|m| library.id_of(&format!("{m}_{name}")))
                .or_else(|| library.id_of(name))
        };
        Ids {
            spool: part("warp_spool"),
            charge: part("warp_charge"),
            arcs: part("warp_charge_arcs"),
            jump: part("warp_jump"),
            jump_damped: part("warp_jump_damped"),
            snag: part("warp_snag"),
            rift: part("warp_rift"),
            rift_damped: part("warp_rift_damped"),
            arrive: part("warp_arrive"),
            arrive_damped: part("warp_arrive_damped"),
            stun_arcs: part("stun_arcs"),
            reboot: part("stun_reboot"),
        }
    }
}

#[derive(Default)]
pub struct WarpSounds {
    ids: HashMap<u32, Ids>,
    damper_hum: Option<SoundId>,
    damper_surge: Option<SoundId>,
    generation: Option<u32>,
    /// Per stunned unit: its stun last tick, and the tick it was seen on.
    stuns: HashMap<u32, (f32, u64)>,
    tick: u64,
}

/// Every warp loop whose pitch follows the game (`Audio::set_gliding`), for `CapitalSounds`
/// to list with its own.
pub fn gliding(library: &SoundLibrary, blueprints: &Blueprints) -> Vec<SoundId> {
    let mut ids: Vec<SoundId> = blueprints
        .units
        .iter()
        .flat_map(|bp| {
            let i = Ids::of(library, bp);
            [i.charge, i.arcs, i.rift, i.rift_damped]
        })
        .flatten()
        .collect();
    ids.sort_by_key(|s| s.0);
    ids.dedup();
    ids
}

/// How a hull of `radius` metres is heard: (gain, pitch). The Courier (58 m) a fifth
/// higher and quieter than the Bastion (160 m) and the Resolute (150 m).
fn size(radius: f32) -> (f32, f32) {
    let r = radius.max(1.0);
    (
        (r / 150.0).powf(0.35).clamp(0.6, 1.1),
        (100.0 / r).powf(0.3).clamp(0.8, 1.25),
    )
}

impl WarpSounds {
    /// One sim tick: plays the one-shots and returns the loops wanted, as
    /// `Audio::set_loops` takes them. `hear` is the game's (gain, pan) for a place.
    pub fn tick(
        &mut self,
        frame: &RenderFrame,
        blueprints: &Blueprints,
        audio: &Audio,
        hear: impl Fn(Vec3) -> (f32, f32),
    ) -> Vec<(SoundId, f32, f32, f32)> {
        self.tick += 1;
        let (library, generation) = audio.library();
        if self.generation != Some(generation) {
            self.generation = Some(generation);
            self.ids.clear();
            self.damper_hum = library.id_of("warp_damper_hum");
            self.damper_surge = library.id_of("warp_damper_surge");
        }
        let mut ids = |blueprint: u32| -> Ids {
            match blueprints.units.get(blueprint as usize) {
                Some(bp) => *self
                    .ids
                    .entry(blueprint)
                    .or_insert_with(|| Ids::of(&library, bp)),
                None => Ids::default(),
            }
        };
        let radius = |blueprint: u32| {
            blueprints
                .units
                .get(blueprint as usize)
                .map_or(150.0, |bp| bp.radius.to_f32())
        };
        let play = |sound: Option<SoundId>, at: Vec3, radius: f32, loud: f32| {
            let Some(sound) = sound else { return };
            let (gain, pan) = hear(at);
            let (g, pitch) = size(radius);
            audio.play_world(sound, (gain * g * loud).min(1.0), pan, pitch);
        };
        let surge = |at: [f32; 2], owner: u8| {
            if let Some(d) = damper_over(&frame.dampers, at, owner) {
                play(self.damper_surge, Vec3::from(d.pos), 150.0, 0.9);
            }
        };

        for event in &frame.events {
            match event {
                SimEvent::WarpSpooling {
                    from, blueprint, ..
                } => {
                    let b = blueprint.0 as u32;
                    play(ids(b).spool, Vec3::from(from.to_f32()), radius(b), 0.9);
                }
                SimEvent::WarpJumped {
                    from,
                    to,
                    dampened,
                    blueprint,
                    owner,
                    ..
                } => {
                    let (b, i) = (blueprint.0 as u32, ids(blueprint.0 as u32));
                    let sound = if *dampened { i.jump_damped } else { i.jump };
                    play(sound, Vec3::from(from.to_f32()), radius(b), 1.1);
                    if *dampened {
                        let [x, y, _] = to.to_f32();
                        surge([x, y], *owner);
                    }
                }
                SimEvent::WarpSnagged { unit, at } => {
                    let view = frame.warps.iter().find(|w| w.unit_id == unit.0);
                    let [x, y] = at.to_f32();
                    let z = view.map_or(0.0, |w| w.to[2]);
                    let b = view.map_or(0, |w| w.blueprint.0 as u32);
                    let r = view.map_or(150.0, |w| w.radius);
                    play(ids(b).snag, Vec3::new(x, y, z), r, 1.0);
                    if let Some(w) = view {
                        surge([x, y], w.owner);
                    }
                }
                SimEvent::WarpArrived {
                    at,
                    dampened,
                    blueprint,
                    ..
                } => {
                    let (b, i) = (blueprint.0 as u32, ids(blueprint.0 as u32));
                    let sound = if *dampened { i.arrive_damped } else { i.arrive };
                    play(sound, Vec3::from(at.to_f32()), radius(b), 1.15);
                }
                _ => {}
            }
        }

        let mut loops: Vec<(SoundId, f32, f32, f32)> = Vec::new();
        let mut want = |sound: Option<SoundId>, at: Vec3, radius: f32, loud: f32, pitch: f32| {
            if let Some(sound) = sound {
                let (gain, pan) = hear(at);
                let (g, p) = size(radius);
                loops.push((sound, gain * g * loud, pan, p * pitch));
            }
        };
        for w in &frame.warps {
            let i = ids(w.blueprint.0 as u32);
            match w.phase {
                WarpPhase::Spool => {
                    let c = w.charge.clamp(0.0, 1.0);
                    let at = Vec3::from(w.from);
                    // The body climbs about an octave and swells; the arcs come in over
                    // the second half and thicken to the end.
                    want(i.charge, at, w.radius, 0.3 + 0.5 * c, 0.72 + 0.6 * c);
                    let arcs = ((c - 0.35) / 0.65).clamp(0.0, 1.0).powf(1.5);
                    want(i.arcs, at, w.radius, 0.85 * arcs, 0.9 + 0.2 * c);
                }
                WarpPhase::Transit => {
                    let p = (w.ticks as f32 / w.length.max(1) as f32).clamp(0.0, 1.0);
                    let sound = if w.dampened { i.rift_damped } else { i.rift };
                    let pitch = if w.dampened {
                        0.85 + 0.2 * p
                    } else {
                        0.8 + 0.35 * p
                    };
                    want(
                        sound,
                        Vec3::from(w.to),
                        w.radius,
                        0.15 + 0.55 * p.powf(1.5),
                        pitch,
                    );
                }
                _ => {}
            }
        }
        // Stunned hulls arcing, and their systems coming back.
        let now = self.tick;
        for u in &frame.units {
            if u.owner_flags & KIND_WRECK != 0 {
                continue;
            }
            let stun = u.stun(1.0);
            let was = self.stuns.get(&u.unit_id).map(|s| s.0);
            if stun <= 0.0 && was.is_none() {
                continue;
            }
            let i = ids(u.blueprint);
            let r = radius(u.blueprint);
            let at = Vec3::from(u.pos);
            if was.is_some_and(|w| w >= STUN_FULL) && stun < STUN_FULL && stun > 0.0 {
                play(i.reboot, at, r, 0.9);
            }
            if stun > 0.0 {
                want(i.stun_arcs, at, r, 0.8 * stun.powf(0.7), 1.0);
                self.stuns.insert(u.unit_id, (stun, now));
            } else {
                self.stuns.remove(&u.unit_id);
            }
        }
        self.stuns.retain(|_, s| now - s.1 < 50);
        // Live dampeners' fields: low, heard only near them.
        for d in frame.dampers.iter().filter(|d| d.live) {
            want(self.damper_hum, Vec3::from(d.pos), 150.0, 0.22, 1.0);
        }
        merge(loops)
    }
}

/// The live dampener not `owner`'s whose field covers `at`, nearest first: the one that
/// caught the jump.
fn damper_over(dampers: &[DamperView], at: [f32; 2], owner: u8) -> Option<&DamperView> {
    let d2 = |d: &DamperView| (d.pos[0] - at[0]).powi(2) + (d.pos[1] - at[1]).powi(2);
    dampers
        .iter()
        .filter(|d| d.live && d.owner != owner && d2(d) <= d.radius * d.radius)
        .min_by(|a, b| d2(a).total_cmp(&d2(b)))
}

/// One voice per sound, from the loudest wanting it, a little louder for each other one.
fn merge(rows: Vec<(SoundId, f32, f32, f32)>) -> Vec<(SoundId, f32, f32, f32)> {
    // (sound, gain, pan, pitch, the others' gain summed)
    let mut best: Vec<(SoundId, f32, f32, f32, f32)> = Vec::new();
    for (sound, g, pan, pitch) in rows {
        if g < 0.004 {
            continue;
        }
        match best.iter_mut().find(|b| b.0 == sound) {
            Some(b) if b.1 >= g => b.4 += g,
            Some(b) => *b = (sound, g, pan, pitch, b.4 + b.1),
            None => best.push((sound, g, pan, pitch, 0.0)),
        }
    }
    best.into_iter()
        .map(|(sound, g, pan, pitch, others)| (sound, (g + others * 0.3).min(0.8), pan, pitch))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigger_hulls_are_lower_and_louder() {
        let (courier, bastion) = (size(58.0), size(160.0));
        assert!(courier.1 > bastion.1 && courier.0 < bastion.0);
        assert!((0.8..=1.25).contains(&courier.1) && (0.8..=1.25).contains(&bastion.1));
    }

    #[test]
    fn one_voice_per_sound() {
        let rows = vec![
            (SoundId(1), 0.5, -0.5, 1.0),
            (SoundId(1), 0.6, 0.5, 1.2),
            (SoundId(2), 0.3, 0.0, 1.0),
            (SoundId(3), 0.001, 0.0, 1.0),
        ];
        let out = merge(rows);
        assert_eq!(out.len(), 2);
        let one = out.iter().find(|r| r.0 == SoundId(1)).unwrap();
        assert!(one.1 > 0.6 && one.2 > 0.0 && one.3 > 1.1);
    }

    #[test]
    fn the_catching_dampener_is_an_enemy_covering_the_mark() {
        let d = |owner, x: f32, live| DamperView {
            unit_id: 0,
            owner,
            pos: [x, 0.0, 0.0],
            radius: 1600.0,
            live,
        };
        let dampers = [d(0, 100.0, true), d(1, 900.0, false), d(1, 1200.0, true)];
        let hit = damper_over(&dampers, [0.0, 0.0], 0).unwrap();
        assert_eq!(hit.pos[0], 1200.0);
        assert!(damper_over(&dampers, [5000.0, 0.0], 0).is_none());
    }

    /// Every warp sound the game asks for is in the library, clean ones checked there.
    #[test]
    fn warp_sounds_are_in_the_library() {
        let library = SoundLibrary::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        for name in [
            "warp_spool",
            "warp_charge",
            "warp_charge_arcs",
            "warp_jump",
            "warp_jump_damped",
            "warp_snag",
            "warp_rift",
            "warp_rift_damped",
            "warp_arrive",
            "warp_arrive_damped",
            "stun_arcs",
            "stun_reboot",
            "warp_damper_hum",
            "warp_damper_surge",
        ] {
            assert!(library.id_of(name).is_some(), "no {name}");
        }
    }
}

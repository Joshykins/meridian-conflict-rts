//! How strategic missiles are heard (docs/NUKES.md "Sound"): a warhead's flight and fall
//! loops, launches, detonations, interceptors. A side's own look (`nuke_look`) picks its
//! sounds: the Regency's are `regency_` and the same name (data/factions/regency/sounds.ron),
//! ARC's the plain names (data/sounds/nuke.ron).

use super::Game;
use crate::audio::Audio;
use glam::Vec3;
use mc_data::{SoundId, SoundLibrary};

/// The sound `name` in look `look` (`gpu_consts::nuke_look` numbers): the Regency's own
/// where the library has it, else ARC's.
fn nuke_sound(library: &SoundLibrary, name: &str, look: u32) -> Option<SoundId> {
    if look == mc_data::strategic::StrategicLook::Plasma as u32 {
        if let Some(id) = library.id_of(&format!("regency_{name}")) {
            return Some(id);
        }
    }
    library.id_of(name)
}

impl Game {
    /// Warheads in flight (`docs/NUKES.md`): the motor's roar while it climbs, the rush
    /// of air as it comes down, louder the nearer it is to landing. Heard a little from
    /// anywhere, like the launch. A salvo is at most three voices (audio/salvo.rs): one
    /// climbing, the warhead nearest to landing, and the rest of the fall merged.
    /// A Regency missile's loops are its faction's own (`nuke_look`): each of the three
    /// voices sounds in the look of the loudest warhead in it.
    pub(super) fn warhead_loops(&self, audio: &Audio) -> Vec<(mc_data::SoundId, f32, f32, f32)> {
        use crate::audio::salvo;
        let (library, _) = audio.library();
        let mut climbing = Vec::new();
        let mut falling = Vec::new();
        // The loudest climbing warhead's look, and each falling one's.
        let mut climb_look = (0.0, 0);
        let mut fall_looks = Vec::new();
        for m in &self.view.frame.strategic {
            if m.kind != mc_sim::mirror::STRATEGIC_WARHEAD {
                continue;
            }
            let (gain, pan) = self.hear(Vec3::from(m.pos));
            if m.pos[2] < m.prev_pos[2] - 0.05 {
                let near = (1.0 - m.eta / 15.0).clamp(0.0, 1.0);
                let gain = gain.max(0.12 + 0.45 * near * near).min(1.0);
                falling.push((gain, pan, m.eta));
                fall_looks.push((m.eta, gain, m.look));
            } else {
                // Coming up to full roar as it clears the tube.
                let lit = (m.age / 1.2).clamp(0.0, 1.0);
                let gain = gain.max(0.18) * lit;
                if gain >= climb_look.0 {
                    climb_look = (gain, m.look);
                }
                climbing.push((gain, pan));
            }
        }
        let mut out = Vec::new();
        if let Some((gain, pan)) = salvo::merge(climbing, 1.0) {
            if let Some(flight) = nuke_sound(&library, "warhead_flight", climb_look.1) {
                out.push((flight, gain, pan, 1.0));
            }
        }
        // `falls` gives the warhead nearest to landing, then the rest merged.
        fall_looks.sort_by(|a, b| a.0.total_cmp(&b.0));
        let nearest = fall_looks.first().map_or(0, |f| f.2);
        let rest = fall_looks
            .iter()
            .skip(1)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(nearest, |f| f.2);
        for (i, (gain, pan)) in salvo::falls(falling).into_iter().enumerate() {
            let look = if i == 0 { nearest } else { rest };
            if let Some(fall) = nuke_sound(&library, "warhead_fall", look) {
                out.push((fall, gain, pan, 1.0));
            }
        }
        out
    }

    /// Strategic missiles (`docs/NUKES.md`): a warhead's launch is heard by everyone the
    /// same way, whoever fired it, and its detonation from anywhere on the map, at once (no
    /// delay for the sound's travel). Looked up by name: these are rare. A salvo of dozens
    /// coalesces (audio/salvo.rs): one alarm, one deeper roar, a budget of detonations,
    /// the small sounds rate-limited.
    pub(super) fn nuke_sounds(&mut self, audio: &Audio) {
        use crate::audio::salvo::Small;
        use mc_sim::SimEvent;
        let (library, _) = audio.library();
        let local = self.view.local;
        let mut launches = 0u32;
        let mut bursts = Vec::new();
        let mut small: [Vec<(f32, f32)>; 3] = Default::default();
        // Whose sounds each kind plays (`nuke_look`): the faction of the most launches, and
        // of the loudest burst and small sound of a kind, this tick.
        let mut launch_looks = [0u32; 2];
        let mut burst_look = (0.0, 0);
        let mut small_look = [(0.0f32, 0u32); 3];
        for event in &self.view.frame.events {
            if matches!(event, SimEvent::NuclearDetonation { .. }) {
                // Warhead or commander's reactor, the whole map hears it: no birdsong after.
                self.ambience.blast();
            }
            let (kind, pos, floor, look) = match event {
                SimEvent::NuclearDetonation {
                    pos,
                    commander: false,
                    look,
                    ..
                } => {
                    let heard = self.hear(Vec3::from(pos.to_f32()));
                    if heard.0 >= burst_look.0 {
                        burst_look = (heard.0, *look as u32);
                    }
                    bursts.push(heard);
                    continue;
                }
                SimEvent::NuclearLaunch { look, .. } => {
                    launches += 1;
                    launch_looks[(*look as usize).min(1)] += 1;
                    continue;
                }
                SimEvent::InterceptorLaunch { from, look, .. } => {
                    (Small::InterceptorLaunch, from, 0.0, *look as u32)
                }
                SimEvent::WarheadIntercepted {
                    pos,
                    killed: true,
                    look,
                    ..
                } => (Small::Intercepted, pos, 0.5, *look as u32),
                SimEvent::SiloOpening { pos, .. } => (Small::SiloDoors, pos, 0.0, 0),
                SimEvent::RoundReady {
                    owner,
                    warhead: true,
                    ..
                } if *owner == local && !self.view.observing => {
                    if let Some(ready) = library.id_of("warhead_ready") {
                        audio.play_response(ready, 0.6);
                    }
                    continue;
                }
                _ => continue,
            };
            let (gain, pan) = self.hear(Vec3::from(pos.to_f32()));
            let gain = gain.max(floor);
            if gain >= small_look[kind as usize].0 {
                small_look[kind as usize] = (gain, look);
            }
            small[kind as usize].push((gain, pan));
        }
        if launches == 0 && bursts.is_empty() && small.iter().all(Vec::is_empty) {
            return;
        }
        let salvo = &mut self.salvo_sounds;
        let t = salvo.now();
        // The same alarm and roar for everyone, the launcher included, at one level and
        // from nowhere in particular: the sound never tells whose warhead it is, so
        // players have to look.
        if let Some(alarm) = library.id_of("nuke_alarm") {
            let length = library.sound(alarm).length;
            if salvo.alarm(t, launches, length) {
                audio.play_response(alarm, 0.8);
            }
        }
        let launch_look = u32::from(launch_looks[1] > launch_looks[0]);
        if let Some(roar) = nuke_sound(&library, "nuke_launch", launch_look) {
            for p in salvo.roar(t, launches) {
                audio.play_world_after(roar, p.gain, p.pan, p.pitch, p.delay);
            }
        }
        // Heard the moment it happens, however far: the user wants the blast and its
        // sound together, not the real lag of sound through the air.
        if let (Some(sound), Some(p)) = (
            nuke_sound(&library, "nuke_detonation", burst_look.1),
            salvo.detonation(t, &bursts),
        ) {
            audio.play_world_after(sound, p.gain, p.pan, p.pitch, 0.0);
        }
        for (kind, name) in [
            (Small::InterceptorLaunch, "interceptor_launch"),
            (Small::Intercepted, "warhead_intercepted"),
            (Small::SiloDoors, "silo_doors"),
        ] {
            if let (Some(sound), Some(p)) = (
                nuke_sound(&library, name, small_look[kind as usize].1),
                salvo.small(t, kind, &small[kind as usize]),
            ) {
                audio.play_world_after(sound, p.gain, p.pan, p.pitch, 0.0);
            }
        }
    }
}

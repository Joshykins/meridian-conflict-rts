//! How a space elevator (`UnitBlueprint::elevator`, mc-sim `elevator.rs`) is heard. Recipes
//! in `data/sounds/elevator.ron`; each part is looked up per blueprint as `<moving>_<part>`
//! (the unit's `moving` sound name) where the library has it, else `elevator_<part>`.
//!
//! - The towers' sirens, on the sim's events: they wind up as the cable starts down out of
//!   orbit (`ElevatorTethering`) and as the crates start down (`ElevatorInbound`), wail
//!   hi-lo through the descent, and wind down as the cable seats or the crates land.
//!   One loop voice per tower crown, each a little detuned with its horn turning, heard
//!   across much of the map like a nuclear launch.
//! - One-shots on the events: the cable falling, the crates landing (and the deck horn
//!   when units ride them), the crates sinking.
//! - One-shots off the pose the mirror publishes (`UnitInstance::mount`, `deploy`), as the
//!   renderer draws it: the cable's foot passing the clouds, seating in the well, the jaws
//!   closing on it, the crates' thrusters lighting to brake.
//! - Loops off the pose: the crates' roar as they come down (louder as they near, higher
//!   the faster they go), their thrusters braking, and the running elevator's hum.
//!
//! Heard the moment it happens, however far, like the giants' far sounds (`titan.rs`).

use super::Audio;
use glam::Vec3;
use mc_data::{Blueprints, SoundId, SoundLibrary, UnitBlueprint};
use mc_render::gpu_consts::elevator::{BEACON_R, BEACON_Z, SKY_Z};
use mc_sim::mirror::{UnitInstance, KIND_WRECK, STATE_RADAR};
use mc_sim::{RenderFrame, SimEvent};
use std::collections::HashMap;
use std::f32::consts::TAU;

/// Seconds of game time a sim tick covers.
const TICK: f32 = 0.1;
/// The cloud deck the cable's foot breaks through, metres up.
const CLOUD_Z: f32 = 1800.0;
/// The crates' height share (0 landed, 1 at the top) under which its thrusters brake:
/// the last third of the descent, whose height goes as (1 - x)^3 (mc-sim `elevator::pose`).
const BRAKE_H: f32 = 1.0 / 27.0;
/// The jaws' grip at which their slams start (`elevator_jaws` times the three to land as
/// they meet the cable).
const JAWS_GRIP: f32 = 0.93;

/// The sirens: seconds to wind up to the high note, seconds on each note after that, the
/// low note as a share of the high one (a fourth under), seconds to wind down.
const WIND_UP: f32 = 3.0;
const SWING: f32 = 1.3;
const LOW_NOTE: f32 = 0.75;
const WIND_DOWN: f32 = 4.5;
/// Seconds a siren horn takes to turn once; each tower's is a third of a turn on.
const TURN: f32 = 3.2;
/// Each tower's siren is tuned a little apart, so the three beat against each other.
const DETUNE: [f32; 3] = [1.0, 1.012, 0.988];
/// How many elevators' sirens sound at once, loudest first: a deliberate cap, three
/// voices each, so a late game with several elevators is a chorus and not a wall.
const MAX_SIRENS: usize = 2;
/// A siren that hears nothing more of its elevator (destroyed, or its events lost) winds
/// down this many seconds after its longest run.
const SIREN_SPARE: f32 = 2.0;

/// Ground distance from the camera's focus at which a far-heard sound is at half its level.
const FAR_HALF: f32 = 6000.0;
/// The level the sirens and the biggest moments keep however far off they are.
const SIREN_FAR: f32 = 0.3;
const BOOM_FAR: f32 = 0.35;
const FALL_FAR: f32 = 0.25;

/// The parts, by `Part`: `elevator_<part>` in the library.
const PARTS: [&str; 12] = [
    "siren",
    "platform_roar",
    "thrusters",
    "hum",
    "cable_fall",
    "cloud_thump",
    "cable_seat",
    "jaws",
    "brake_ignite",
    "dock",
    "deck_horn",
    "sink",
];

/// Indices into `PARTS`. The first four are loops.
#[derive(Clone, Copy)]
enum Part {
    Siren,
    Roar,
    Thrusters,
    Hum,
    CableFall,
    CloudThump,
    CableSeat,
    Jaws,
    BrakeIgnite,
    Dock,
    DeckHorn,
    Sink,
}

#[derive(Clone, Copy, Default)]
struct Ids([Option<SoundId>; PARTS.len()]);

impl Ids {
    /// `bp`'s sounds (or the defaults for an elevator not in view): `<moving>_<part>` where
    /// the library has it, else `elevator_<part>`.
    fn of(library: &SoundLibrary, bp: Option<&UnitBlueprint>) -> Ids {
        let own = bp.and_then(|bp| bp.sounds.moving.as_deref());
        Ids(PARTS.map(|part| {
            own.and_then(|m| library.id_of(&format!("{m}_{part}")))
                .or_else(|| library.id_of(&format!("elevator_{part}")))
        }))
    }

    fn get(&self, part: Part) -> Option<SoundId> {
        self.0[part as usize]
    }
}

/// Every elevator loop, whose pitch follows the game (`Audio::set_gliding`), for
/// `CapitalSounds` to list with its own.
pub(crate) fn gliding(library: &SoundLibrary, blueprints: &Blueprints) -> Vec<SoundId> {
    let mut ids: Vec<SoundId> = blueprints
        .units
        .iter()
        .filter(|bp| bp.elevator.is_some())
        .map(Some)
        .chain([None])
        .flat_map(|bp| {
            let ids = Ids::of(library, bp);
            [Part::Siren, Part::Roar, Part::Thrusters, Part::Hum].map(|p| ids.get(p))
        })
        .flatten()
        .collect();
    ids.sort_by_key(|s| s.0);
    ids.dedup();
    ids
}

/// One elevator's sirens: sounding since `start` (a sim tick), winding down from `end`.
#[derive(Clone, Copy)]
struct Siren {
    pos: Vec3,
    heading: f32,
    ids: Ids,
    start: u32,
    end: Option<u32>,
    /// It winds down by this tick whatever else is heard.
    limit: u32,
}

/// What an elevator in view looked like when last heard: cable drop, grip, crates' height.
#[derive(Clone, Copy)]
struct Seen {
    pose: [f32; 3],
    tick: u32,
}

#[derive(Default)]
pub struct ElevatorSounds {
    sirens: HashMap<u32, Siren>,
    seen: HashMap<u32, Seen>,
    /// Sound ids by blueprint, and for an elevator out of view, for the library
    /// generation they were looked up in.
    ids: HashMap<u32, Ids>,
    unseen: Ids,
    generation: Option<u32>,
}

impl ElevatorSounds {
    /// One sim tick. Plays the one-shots and returns the loops wanted this tick, as
    /// `Audio::set_loops` takes them. `focus` is the ground point the camera looks at;
    /// `hear` the game's (gain, pan) for a place.
    pub fn tick(
        &mut self,
        frame: &RenderFrame,
        blueprints: &Blueprints,
        audio: &Audio,
        focus: Vec3,
        hear: impl Fn(Vec3) -> (f32, f32),
    ) -> Vec<(SoundId, f32, f32, f32)> {
        let (library, generation) = audio.library();
        if self.generation != Some(generation) {
            self.generation = Some(generation);
            self.ids.clear();
            self.unseen = Ids::of(&library, None);
        }
        // Heard near as anything is, and kept at `floor`'s far level however far off.
        let loud = |at: Vec3, floor: f32| -> (f32, f32) {
            let (near, pan) = hear(at);
            let d = (at - focus).truncate().length();
            (near.max(floor / (1.0 + (d / FAR_HALF).powi(2))), pan)
        };
        let tick = frame.tick;
        let elevators: Vec<(&UnitInstance, &UnitBlueprint)> = frame
            .units
            .iter()
            .filter(|u| u.owner_flags & (KIND_WRECK | STATE_RADAR) == 0 && u.build >= 1.0)
            .filter_map(|u| {
                let bp = blueprints.units.get(u.blueprint as usize)?;
                bp.elevator.is_some().then_some((u, bp))
            })
            .collect();
        for (u, bp) in &elevators {
            self.ids
                .entry(u.blueprint)
                .or_insert_with(|| Ids::of(&library, Some(bp)));
        }
        let in_view = |unit: u32| elevators.iter().find(|(u, _)| u.unit_id == unit);
        // The longest a siren runs: the slowest elevator's tether or descent.
        let longest = blueprints
            .units
            .iter()
            .filter_map(|bp| bp.elevator)
            .map(|e| e.tether_ticks.max(e.descent_ticks))
            .max()
            .unwrap_or(0)
            + (SIREN_SPARE / TICK) as u32;

        for event in &frame.events {
            let (unit, pos, starts) = match event {
                SimEvent::ElevatorTethering { unit, pos, .. }
                | SimEvent::ElevatorInbound { unit, pos, .. } => (unit.0, pos, true),
                SimEvent::ElevatorTethered { unit, pos, .. }
                | SimEvent::ElevatorDocked { unit, pos, .. }
                | SimEvent::ElevatorSinking { unit, pos, .. } => (unit.0, pos, false),
                _ => continue,
            };
            let at = Vec3::from(pos.to_f32());
            let seen = in_view(unit);
            let ids = seen
                .and_then(|(u, _)| self.ids.get(&u.blueprint).copied())
                .unwrap_or(self.unseen);
            let pitch = 0.98 + (unit % 5) as f32 * 0.01;
            if starts {
                self.sirens.insert(
                    unit,
                    Siren {
                        pos: at,
                        heading: seen.map_or(0.0, |(u, _)| u.heading),
                        ids,
                        start: tick,
                        end: None,
                        limit: tick + longest,
                    },
                );
            } else if let Some(s) = self.sirens.get_mut(&unit) {
                s.end.get_or_insert(tick);
            }
            match event {
                SimEvent::ElevatorTethering { .. } => {
                    let (gain, pan) = loud(at, FALL_FAR);
                    play(audio, ids.get(Part::CableFall), gain, pan, pitch, 0.0);
                }
                SimEvent::ElevatorDocked { units, .. } => {
                    let (gain, pan) = loud(at, BOOM_FAR);
                    play(audio, ids.get(Part::Dock), gain, pan, pitch, 0.0);
                    // The deck horn as what it brought starts to roll off.
                    if *units > 0 {
                        let (gain, pan) = loud(at, BOOM_FAR * 0.5);
                        play(audio, ids.get(Part::DeckHorn), gain * 0.8, pan, 1.0, 1.6);
                    }
                }
                SimEvent::ElevatorSinking { .. } => {
                    let (gain, pan) = hear(at);
                    play(audio, ids.get(Part::Sink), gain, pan, pitch, 0.0);
                }
                _ => {}
            }
        }

        let mut loops = Vec::new();
        for (u, bp) in &elevators {
            let Some(e) = bp.elevator else { continue };
            let ids = self.ids.get(&u.blueprint).copied().unwrap_or_default();
            let pos = Vec3::from(u.pos);
            let now = [u.mount[1], u.mount[3], u.deploy];
            // Compared with what was heard last, so a crossing between frames that skip
            // sim ticks is still heard once.
            let (was, ticks) = match self.seen.get(&u.unit_id) {
                Some(s) if tick.wrapping_sub(s.tick) <= 20 && s.tick != tick => {
                    (s.pose, (tick - s.tick) as f32)
                }
                _ => ([u.mount[0], u.mount[2], u.prev_deploy], 1.0),
            };
            self.seen.insert(u.unit_id, Seen { pose: now, tick });
            let pitch = 0.98 + (u.unit_id % 5) as f32 * 0.01;
            let crossed = |i: usize, at: f32| was[i] < at && now[i] >= at;
            // The cable's foot through the clouds, seated, and the jaws closing on it.
            if crossed(0, 1.0 - CLOUD_Z / SKY_Z) {
                let (gain, pan) = loud(pos, BOOM_FAR);
                play(audio, ids.get(Part::CloudThump), gain, pan, pitch, 0.0);
            }
            if crossed(0, 0.999) {
                let (gain, pan) = loud(pos, BOOM_FAR);
                play(audio, ids.get(Part::CableSeat), gain, pan, pitch, 0.0);
                if let Some(s) = self.sirens.get_mut(&u.unit_id) {
                    s.end.get_or_insert(tick);
                }
            }
            if crossed(1, JAWS_GRIP) {
                let (gain, pan) = loud(pos, BOOM_FAR * 0.6);
                play(audio, ids.get(Part::Jaws), gain, pan, pitch, 0.0);
            }
            let (near, pan) = hear(pos);
            if now[0] >= 1.0 {
                push(&mut loops, ids.get(Part::Hum), near * 0.45, pan, 1.0);
            }
            // The crates, by their height share and speed (shares a second).
            let (h_was, h) = (was[2], now[2]);
            let speed = (h - h_was) / (ticks * TICK);
            if h < h_was && h_was > 0.0 && h_was <= 1.0 {
                if h_was > BRAKE_H && h <= BRAKE_H {
                    play(audio, ids.get(Part::BrakeIgnite), near, pan, pitch, 0.0);
                }
                let descent = e.descent_ticks as f32 * TICK;
                let (roar, brake) = descending(h.max(0.0), -speed, descent);
                push(&mut loops, ids.get(Part::Roar), near * roar.0, pan, roar.1);
                push(
                    &mut loops,
                    ids.get(Part::Thrusters),
                    near * brake.0,
                    pan,
                    brake.1,
                );
            }
        }
        self.seen.retain(|_, s| tick.wrapping_sub(s.tick) < 50);

        // The sirens, loudest elevators first.
        for (unit, s) in &mut self.sirens {
            // An elevator in view that is gone (destroyed) winds its sirens down.
            if s.end.is_none()
                && (tick >= s.limit || self.seen.get(unit).is_some_and(|v| v.tick != tick))
            {
                s.end = Some(tick);
            }
        }
        let mut sirens: Vec<(f32, Siren)> = Vec::new();
        self.sirens.retain(|_, s| {
            let since = tick.saturating_sub(s.start) as f32 * TICK;
            let ended = s.end.map(|e| tick.saturating_sub(e) as f32 * TICK);
            let (level, _) = siren(since, ended);
            // Silent only at the very start of its wind-up, or once it has wound down.
            let sounding = s.end.is_none() || level > 0.0;
            if sounding {
                sirens.push((level * loud(s.pos, SIREN_FAR).0, *s));
            }
            sounding
        });
        sirens.sort_by(|a, b| b.0.total_cmp(&a.0));
        // Three voices an elevator on purpose: the platform loops merge, the sirens not.
        let mut loops = merge(loops);
        for (_, s) in sirens.into_iter().take(MAX_SIRENS) {
            let since = tick.saturating_sub(s.start) as f32 * TICK;
            let ended = s.end.map(|e| tick.saturating_sub(e) as f32 * TICK);
            let (level, pitch) = siren(since, ended);
            for (k, detune) in DETUNE.into_iter().enumerate() {
                let bearing = s.heading + k as f32 * TAU / 3.0;
                let crown =
                    s.pos + Vec3::new(bearing.cos() * BEACON_R, bearing.sin() * BEACON_R, BEACON_Z);
                let (gain, pan) = loud(crown, SIREN_FAR);
                // Its horn turning, so its level sweeps round; each tower a third of a turn on.
                let turn = 0.7 + 0.3 * (since * TAU / TURN + k as f32 * TAU / 3.0).cos();
                push(
                    &mut loops,
                    s.ids.get(Part::Siren),
                    gain * level * turn * 0.85,
                    pan,
                    pitch * detune,
                );
            }
        }
        loops
    }
}

/// A siren `since` seconds after it started, `ended` seconds after it began to wind down:
/// its level (0 once it is quiet) and its rotor's pitch. Winds up from half pitch, holds
/// the high note, swings to the low one and back every `SWING` seconds, and winds down
/// from wherever it was to half pitch as it falls quiet.
fn siren(since: f32, ended: Option<f32>) -> (f32, f32) {
    let running = |t: f32| {
        let up = smooth(t / WIND_UP);
        let pitch = if t < WIND_UP {
            0.5 + 0.5 * up
        } else if (((t - WIND_UP) / SWING) as u32).is_multiple_of(2) {
            1.0
        } else {
            LOW_NOTE
        };
        (up.sqrt(), pitch)
    };
    match ended {
        None => running(since),
        Some(e) => {
            let (level, pitch) = running(since - e);
            let down = (e / WIND_DOWN).min(1.0);
            (
                level * (1.0 - down).powf(1.5),
                0.5 + (pitch - 0.5) * (1.0 - smooth(down)),
            )
        }
    }
}

/// Crates coming down at height share `h`, `speed` shares a second, on a descent of
/// `descent` seconds: (gain, pitch) for their roar and their braking thrusters.
fn descending(h: f32, speed: f32, descent: f32) -> ((f32, f32), (f32, f32)) {
    // Fastest at the top (3 shares over the descent); `x` how far through it is.
    let pace = (speed * descent / 3.0).clamp(0.0, 1.0).sqrt();
    let x = 1.0 - h.max(0.0).cbrt();
    let near = 1.0 - h;
    let roar = (
        (0.2 + 0.8 * near * near) * (0.4 + 0.6 * pace),
        0.8 + 0.35 * pace,
    );
    // Braking over the last third, throttled back as they settle on the pad.
    let brake = smooth((x - 0.62) / 0.08);
    let settle = smooth((x - 0.95) / 0.05);
    let thrust = (0.9 * brake * (1.0 - 0.6 * settle), 0.95 + 0.1 * brake);
    (roar, thrust)
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn play(audio: &Audio, sound: Option<SoundId>, gain: f32, pan: f32, pitch: f32, after: f32) {
    if let Some(sound) = sound {
        audio.play_world_after(sound, gain.min(1.0), pan, pitch, after);
    }
}

fn push(
    loops: &mut Vec<(SoundId, f32, f32, f32)>,
    sound: Option<SoundId>,
    gain: f32,
    pan: f32,
    pitch: f32,
) {
    if let Some(sound) = sound.filter(|_| gain >= 0.004) {
        loops.push((sound, gain, pan, pitch));
    }
}

/// The loops of several elevators as one voice per sound, from the loudest, a little
/// louder for each other one.
fn merge(loops: Vec<(SoundId, f32, f32, f32)>) -> Vec<(SoundId, f32, f32, f32)> {
    // Per sound: the loudest row, and the others' gain summed.
    let mut best: Vec<((SoundId, f32, f32, f32), f32)> = Vec::new();
    for row in loops {
        match best.iter_mut().find(|b| b.0 .0 == row.0) {
            Some(b) if b.0 .1 >= row.1 => b.1 += row.1,
            Some(b) => *b = (row, b.1 + b.0 .1),
            None => best.push((row, 0.0)),
        }
    }
    best.into_iter()
        .map(|((sound, gain, pan, pitch), others)| {
            (sound, (gain + others * 0.3).min(0.9), pan, pitch)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_part_is_in_the_library_and_the_loops_loop() {
        let data = std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("../../data");
        let library = SoundLibrary::load(&data).unwrap();
        let blueprints = Blueprints::load(&data).unwrap();
        let elevator = blueprints.units.iter().find(|bp| bp.elevator.is_some());
        let ids = Ids::of(&library, elevator);
        for (k, part) in PARTS.iter().enumerate() {
            let id = ids.0[k].unwrap_or_else(|| panic!("no sound for the elevator's {part}"));
            let looped = library.sound(id).looped;
            assert_eq!(looped, k <= Part::Hum as usize, "{part} loops: {looped}");
        }
        assert_eq!(gliding(&library, &blueprints).len(), 4);
    }

    #[test]
    fn the_siren_winds_up_wails_hi_lo_and_winds_down() {
        let (start, low) = siren(0.0, None);
        assert_eq!(start, 0.0);
        assert!(low == 0.5);
        let (half, mid) = siren(WIND_UP * 0.5, None);
        assert!(half > 0.5 && mid > 0.6 && mid < 0.9, "{half} {mid}");
        let high = siren(WIND_UP + SWING * 0.5, None);
        let swung = siren(WIND_UP + SWING * 1.5, None);
        assert_eq!((high.0, high.1), (1.0, 1.0));
        assert_eq!(swung.1, LOW_NOTE);
        let back = siren(WIND_UP + SWING * 2.5, None);
        assert_eq!(back.1, 1.0);
        // Winding down: quieter and lower each moment, silent at the end.
        let t = WIND_UP + SWING * 0.5;
        let a = siren(t + 1.0, Some(1.0));
        let b = siren(t + 3.0, Some(3.0));
        assert!(a.0 > b.0 && a.1 > b.1 && b.1 >= 0.5, "{a:?} {b:?}");
        assert_eq!(siren(t + WIND_DOWN, Some(WIND_DOWN)).0, 0.0);
    }

    #[test]
    fn the_crates_roar_nearer_and_brake_late() {
        let descent = 9.0;
        // The descent's height is (1 - x)^3 over x = t / descent.
        let at = |x: f32| {
            let h = (1.0 - x).powi(3);
            let speed = 3.0 * (1.0 - x).powi(2) / descent;
            descending(h, speed, descent)
        };
        let (top, high) = at(0.05);
        let (mid, middle) = at(0.5);
        let (low, late) = at(0.85);
        assert!(mid.0 > top.0, "the roar grows as it nears: {top:?} {mid:?}");
        assert!(top.1 > low.1, "and sinks as it slows");
        assert!(high.0 == 0.0 && middle.0 == 0.0, "no braking early");
        assert!(late.0 > 0.5, "the thrusters brake late: {late:?}");
    }

    #[test]
    fn several_elevators_make_one_voice_per_sound() {
        let rows = vec![
            (SoundId(1), 0.5, 0.0, 1.0),
            (SoundId(1), 0.6, 0.3, 1.1),
            (SoundId(2), 0.2, 0.0, 1.0),
        ];
        let merged = merge(rows);
        assert_eq!(merged.len(), 2);
        assert!((merged[0].1 - 0.75).abs() < 1e-5 && merged[0].3 == 1.1);
    }
}

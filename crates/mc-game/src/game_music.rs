//! What the music hears of the match: the scene (battle or survival), the game
//! events that play a moment over the song, the match's end, and the duck while
//! paused. The music does not follow the fighting: the song plays through and
//! only makes way for moments (`audio/music.rs`).
//!
//! Everything read here is the player's own fog-filtered frame and its events,
//! so the music never gives away what the player cannot see.

use super::Game;
use crate::audio::music::Scene;
use crate::audio::Audio;
use mc_data::{cat, BlueprintId};
use mc_music::score::cue;
use mc_sim::SimEvent;
use std::sync::atomic::Ordering;

/// Seconds between any two moments, and between two of the same one: a battle is
/// full of events, and a moment on each would be the reactive score the user does not want.
pub const CUE_GAP: f32 = 8.0;
pub const CUE_REPEAT: f32 = 20.0;

/// Keeps moments rare: at most one every `CUE_GAP` seconds, and never the same
/// one twice within `CUE_REPEAT`. An event the score has no moment for costs nothing.
#[derive(Clone, Debug, Default)]
pub struct CueLimiter {
    last_any: Option<f32>,
    last: Vec<(&'static str, f32)>,
}

impl CueLimiter {
    pub fn allows(&self, name: &str, now: f32) -> bool {
        self.last_any.is_none_or(|t| now - t >= CUE_GAP)
            && self
                .last
                .iter()
                .all(|(n, t)| *n != name || now - t >= CUE_REPEAT)
    }

    /// Plays `name` through `play` (which says whether it played) if allowed now.
    pub fn cue(&mut self, name: &'static str, now: f32, play: impl FnOnce(&str) -> bool) -> bool {
        if !self.allows(name, now) || !play(name) {
            return false;
        }
        self.last_any = Some(now);
        self.last.retain(|(n, _)| *n != name);
        self.last.push((name, now));
        true
    }
}

#[derive(Default)]
pub(super) struct MatchMusic {
    cues: CueLimiter,
    /// Real seconds since the match began: the cue clock.
    clock: f32,
    log_in: f32,
    finished: bool,
    /// `MERIDIAN_MUSIC_LOG`: log what plays once a second.
    log: Option<bool>,
}

impl Game {
    /// Once a frame. `fresh`: a new sim tick arrived.
    pub(super) fn music_frame(&mut self, audio: &Audio, dt: f32, fresh: bool) {
        let scene = if self.view.status.survival.is_some() {
            Scene::Survival
        } else {
            Scene::Battle(self.music_faction.clone())
        };
        audio.music_scene(scene);
        audio.music_duck(self.sim.paused.load(Ordering::Relaxed));
        let mut music = std::mem::take(&mut self.music);
        music.clock += dt;
        if fresh && !music.finished {
            let now = music.clock;
            for name in self.music_events() {
                music.cues.cue(name, now, |n| audio.music_cue(n));
            }
        }
        if let (Some(team), false) = (self.view.status.winner, music.finished) {
            music.finished = true;
            let won = self.view.observing
                || self
                    .view
                    .status
                    .players
                    .get(self.view.local as usize)
                    .is_some_and(|p| p.team == team);
            audio.music_finish(won);
        }
        if *music
            .log
            .get_or_insert_with(|| std::env::var_os("MERIDIAN_MUSIC_LOG").is_some())
        {
            music.log_in -= dt;
            if music.log_in <= 0.0 {
                music.log_in = 1.0;
                if let Some(s) = audio.music_status() {
                    log::info!(
                        "music: {} / {}{} (song at {:.2}); engine {:.0}%, render {:.0}% (peak {:.0}%), \
                         {:.0} ms ahead, {} underruns ({} frames)",
                        s.song,
                        s.section.as_deref().unwrap_or("-"),
                        s.moment.as_deref().map(|m| format!(", moment {m}")).unwrap_or_default(),
                        s.song_level,
                        s.load * 100.0,
                        s.render_load * 100.0,
                        s.render_peak * 100.0,
                        s.ahead * 1000.0,
                        s.underruns,
                        s.underrun_frames
                    );
                }
            }
        }
        self.music = music;
    }

    /// The events of this tick that the score may play a moment for.
    fn music_events(&self) -> Vec<&'static str> {
        let view = &self.view;
        let (local, observing) = (view.local, view.observing);
        let team_of = |p: u8| view.status.players.get(p as usize).map(|p| p.team);
        let ours = |p: u8| !observing && team_of(local).is_some() && team_of(p) == team_of(local);
        let mut out = Vec::new();
        for e in &view.frame.events {
            match e {
                SimEvent::UnitDied {
                    blueprint, owner, ..
                } if self.blueprints.unit(*blueprint).categories & cat::COMMANDER != 0 => {
                    if !observing && *owner == local {
                        out.push(cue::COMMANDER_LOST);
                    } else if !ours(*owner) {
                        out.push(cue::ENEMY_COMMANDER);
                    }
                }
                SimEvent::NuclearDetonation {
                    commander: false, ..
                } => out.push(cue::NUKE),
                SimEvent::RoundLaunched { .. } => out.push(cue::WAVE),
                SimEvent::UnitCompleted { unit, owner } if *owner == local && !observing => {
                    let titan = view
                        .index_of
                        .get(&unit.0)
                        .and_then(|&i| view.frame.units.get(i))
                        .is_some_and(|u| {
                            self.blueprints.unit(BlueprintId(u.blueprint as u16)).tech >= 4
                        });
                    if titan {
                        out.push(cue::TITAN);
                    }
                }
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_cue_rate_limits() {
        let mut c = CueLimiter::default();
        let yes = |_: &str| true;
        assert!(c.cue("nuke", 0.0, yes));
        // Any cue waits CUE_GAP after another.
        assert!(!c.cue("titan", 5.0, yes));
        assert!(c.cue("titan", 8.5, yes));
        // The same cue waits CUE_REPEAT.
        assert!(!c.cue("nuke", 17.0, yes));
        assert!(c.cue("nuke", 20.5, yes));
        // A cue the score has no moment for does not use up the slot.
        assert!(!c.cue("wave", 40.0, |_| false));
        assert!(c.cue("enemy_commander", 40.0, yes));
    }
}

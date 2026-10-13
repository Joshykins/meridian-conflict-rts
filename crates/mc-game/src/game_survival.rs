//! What the match says and plays about survival: the rounds, the engine's
//! ray, and the replication nodes it raises (bonus objectives), and the
//! Progenitor's own sounds (data/sounds/progenitor.ron): its Lances charging
//! and striking, its domes struck, its shells coming down and landing.
//! Everything here reads the sim's survival events and status; nothing is
//! decided here.

use super::Game;
use crate::audio::{Audio, Sfx};
use crate::hud::survival::VIOLET;
use crate::ui::palette;
use glam::Vec3;
use mc_data::survival::SiteKind;
use mc_sim::SimEvent;

/// Good news: a node down.
const GOOD: u32 = crate::hud::HEALTHY;
/// Heavy waves and the experimental tiers: pale gold, as on the survival card.
const HEAVY: u32 = 0xFFD27A;

impl Game {
    /// Toasts and stingers for survival's events; ignores everything else.
    pub(super) fn note_survival(&mut self, event: &SimEvent, audio: &Audio) {
        let site = |s: u8| {
            self.hud
                .survival
                .sites
                .get(s as usize)
                .cloned()
                .unwrap_or_else(|| format!("Site {}", s + 1))
        };
        let name = |b: mc_data::BlueprintId| self.blueprints.unit(b).name.clone();
        let (text, color, sound) = match event {
            SimEvent::RoundPrinting { round } => {
                let of = self.view.status.survival.as_ref().map_or(0, |s| s.rounds);
                let text = if of != 0 && *round == of {
                    "Final Round  \u{b7}  The Progenitor Is Forging".to_string()
                } else {
                    format!("Round {round}  \u{b7}  The Progenitor Is Forging")
                };
                (text, VIOLET, "survival_round")
            }
            SimEvent::RoundLaunched { round, units } => (
                format!("Round {round} Inbound  \u{b7}  {units} Units"),
                palette::WARN,
                "survival_launch",
            ),
            SimEvent::NodeRaising {
                site: s,
                product,
                kind,
                shaper,
                ..
            } => (
                format!(
                    "Genesis Ray Firing  \u{b7}  {}  \u{b7}  {}",
                    site(*s),
                    match (kind, product) {
                        (SiteKind::Printer, Some(p)) => format!("Will Print {}", name(*p)),
                        _ => format!("Raising {}", name(*shaper)),
                    }
                ),
                VIOLET,
                "survival_node_raising",
            ),
            SimEvent::NodeOnline {
                site: s,
                product,
                kind,
                shaper,
                ..
            } => (
                format!(
                    "{} Online at {}  \u{b7}  {}  \u{b7}  Bonus Objective",
                    if *kind == SiteKind::Printer {
                        "Shaper".to_string()
                    } else {
                        name(*shaper)
                    },
                    site(*s),
                    match (kind, product) {
                        (SiteKind::Printer, Some(p)) => format!("Printing {}", name(*p)),
                        (SiteKind::Artillery, _) => "Fires Replication Shells".to_string(),
                        (SiteKind::Nuke, _) => "Launches Warheads".to_string(),
                        (SiteKind::Printer, None) => "Printing".to_string(),
                    }
                ),
                VIOLET,
                "survival_node_online",
            ),
            // A volley's first shell: the marks themselves are on the map (`replication_marks`).
            SimEvent::ReplicationShellFired {
                site: s,
                units,
                flight_ticks,
                volley: 0,
                ..
            } => (
                format!(
                    "Replication Shells Inbound from {}  \u{b7}  {units} Units Each  \u{b7}  {} s",
                    site(*s),
                    flight_ticks / mc_core::TICKS_PER_SECOND
                ),
                palette::WARN,
                "progenitor_shell_fired",
            ),
            // The launch alarm is the nuke's own (`game_nuke_sounds`): a toast only.
            SimEvent::NuclearLaunch { owner, .. }
                if self
                    .view
                    .status
                    .survival
                    .as_ref()
                    .is_some_and(|s| s.engine_player == *owner) =>
            {
                self.hud.toast(
                    "Progenitor Warhead Launched  \u{b7}  Its Mark Is on the Map".to_string(),
                    palette::BAD,
                );
                return;
            }
            SimEvent::NodeDestroyed {
                site: s,
                wreck,
                raised,
                ..
            } => (
                if *raised {
                    format!(
                        "Shaper Destroyed at {}  \u{b7}  Wreck Worth {wreck} Materials",
                        site(*s)
                    )
                } else {
                    format!("Shaper Cut Down Before It Rose at {}", site(*s))
                },
                GOOD,
                "survival_node_down",
            ),
            SimEvent::TierReached { tier, .. } => {
                self.hud.survival.announce_tier(*tier);
                (
                    format!("Tier {tier} Reached  \u{b7}  The Progenitor Prints Tech {tier}"),
                    if *tier >= 4 { HEAVY } else { VIOLET },
                    "survival_tier",
                )
            }
            SimEvent::HeavyWave { squads, .. } => {
                let parts: Vec<String> = squads
                    .iter()
                    .map(|(b, n)| format!("{n} \u{d7} {}", name(*b)))
                    .collect();
                (
                    format!("Heavy Wave Forging  \u{b7}  {}", parts.join(", ")),
                    HEAVY,
                    "survival_heavy",
                )
            }
            // A level that brings another kind of unit is news; the others show on the card.
            SimEvent::ShaperLevelled {
                site: s,
                level,
                added: Some(added),
                ..
            } => (
                format!(
                    "Shaper at {} Reached Level {level}  \u{b7}  Now Prints {}",
                    site(*s),
                    name(*added)
                ),
                VIOLET,
                "survival_shaper_level",
            ),
            SimEvent::SurvivalWon { rounds } => {
                audio.play(Sfx::Victory);
                self.hud
                    .toast(format!("Held Through {rounds} Rounds"), GOOD);
                return;
            }
            _ => return,
        };
        self.hud.toast(text, color);
        let (library, _) = audio.library();
        match library.id_of(sound) {
            Some(id) => audio.play_response(id, 0.9),
            None => audio.play(Sfx::Order),
        }
    }

    /// The ray and the print beams, the Lances charging and the shells coming down, as
    /// loops: (sound, gain, pan, pitch). The ray is heard wherever the camera is; it
    /// crosses the map.
    pub(super) fn survival_loops(&self, audio: &Audio) -> Vec<(mc_data::SoundId, f32, f32, f32)> {
        let mut out = Vec::new();
        if self.view.status.survival.is_none() {
            return out;
        }
        let (library, _) = audio.library();
        out.extend(self.progenitor_loops(&library));
        let ray = mc_sim::survival::BEAM_REPLICATION_RAY;
        let print = mc_sim::survival::BEAM_PRINT;
        for (kind, name, floor) in [
            (ray, "replication_ray", 0.22),
            (print, "replication_print", 0.0),
        ] {
            let Some(sound) = library.id_of(name) else {
                continue;
            };
            let mut sum = (0.0f32, 0.0f32);
            for b in self.view.frame.beams.iter().filter(|b| b.kind == kind) {
                let (gain, pan) = self.hear(Vec3::from(b.to));
                let (g2, p2) = self.hear(Vec3::from(b.from));
                let gain = gain.max(g2 * 0.6).max(floor);
                let pan = if gain > 0.0 { pan } else { p2 };
                sum = (sum.0 + gain * gain, sum.1 + gain * gain * pan);
            }
            if sum.0 > 0.0 {
                out.push((
                    sound,
                    (sum.0.sqrt() * 0.55).min(0.9),
                    sum.1 / sum.0.max(1e-9),
                    1.0,
                ));
            }
        }
        out
    }
}

impl Game {
    /// Every Lance charging in earshot as one hum, louder and leaning a little higher the
    /// fuller the loudest is; every shell under three seconds from landing as one rush of
    /// air, louder and lower the nearer it is.
    fn progenitor_loops(
        &self,
        library: &mc_data::SoundLibrary,
    ) -> Vec<(mc_data::SoundId, f32, f32, f32)> {
        let mut out = Vec::new();
        let frame = &self.view.frame;
        if let Some(sound) = library.id_of("progenitor_lance_charge") {
            let mut sum = (0.0f32, 0.0f32);
            let mut fullest = (0.0f32, 0.0f32);
            for s in frame.shapers.iter().filter(|s| s.lance_charge > 0.0) {
                let Some(u) = frame.units.iter().find(|u| u.unit_id == s.unit) else {
                    continue;
                };
                let lens = Vec3::from(u.pos)
                    + Vec3::Z * mc_render::models::shaper::lens_height(s.kind, s.height);
                let (gain, pan) = self.hear(lens);
                let gain = gain * (0.35 + 0.65 * s.lance_charge);
                sum = (sum.0 + gain * gain, sum.1 + gain * gain * pan);
                if gain > fullest.0 {
                    fullest = (gain, s.lance_charge);
                }
            }
            if sum.0 > 0.0 {
                out.push((
                    sound,
                    (sum.0.sqrt() * 0.8).min(0.9),
                    sum.1 / sum.0.max(1e-9),
                    0.92 + 0.16 * fullest.1,
                ));
            }
        }
        if let Some(sound) = library.id_of("progenitor_shell_whistle") {
            let mut sum = (0.0f32, 0.0f32);
            let mut nearest = 0.0f32;
            for s in frame.shells.iter().filter(|s| s.eta < 3.0) {
                let near = 1.0 - s.eta / 3.0;
                let (gain, pan) = self.hear(Vec3::from(s.pos));
                let gain = gain * (0.3 + 0.7 * near);
                sum = (sum.0 + gain * gain, sum.1 + gain * gain * pan);
                nearest = nearest.max(near);
            }
            if sum.0 > 0.0 {
                out.push((
                    sound,
                    (sum.0.sqrt() * 0.8).min(0.9),
                    sum.1 / sum.0.max(1e-9),
                    1.15 - 0.3 * nearest,
                ));
            }
        }
        out
    }

    /// The Progenitor's strikes and landings this tick, each heard where it happens.
    pub(super) fn progenitor_sounds(&self, audio: &Audio) {
        if self.view.status.survival.is_none() {
            return;
        }
        let (library, _) = audio.library();
        let strike = library.id_of("progenitor_lance_strike");
        let landed = library.id_of("progenitor_shell_landed");
        for event in &self.view.frame.events {
            let (sound, at, floor) = match event {
                SimEvent::LanceStrike { to, .. } => (strike, to.to_f32(), 0.0),
                // A squad landing is news wherever the camera is.
                SimEvent::ReplicationShellLanded { at, .. } => (landed, at.to_f32(), 0.15),
                _ => continue,
            };
            let Some(sound) = sound else { continue };
            let (gain, pan) = self.hear(Vec3::from(at));
            let gain = gain.max(floor);
            if gain > 0.01 {
                audio.play_world(sound, gain, pan, 1.0);
            }
        }
    }

    /// Whether a shot that struck a dome at `pos` struck a Progenitor Shaper's (its own
    /// sound, `progenitor_shield_hit`).
    pub(super) fn on_progenitor_dome(&self, pos: Vec3) -> bool {
        use mc_sim::mirror::SHIELD_LOOK_SHIFT;
        let look = mc_data::ShieldLook::Progenitor as u32;
        self.view.frame.shields.iter().any(|s| {
            (s.packed >> SHIELD_LOOK_SHIFT) & 3 == look
                && pos.truncate().distance(Vec3::from(s.pos).truncate()) <= s.radius * 1.05
        })
    }
}

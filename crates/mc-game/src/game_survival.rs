//! What the match says and plays about survival: the rounds, the engine's
//! ray, and the replication nodes it raises (bonus objectives). Everything
//! here reads the sim's survival events and status; nothing is decided here.

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
                // Placeholder until the shell has a sound of its own.
                "survival_launch",
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

    /// The ray and the print beams, as loops: (sound, gain, pan, pitch).
    /// The ray is heard wherever the camera is; it crosses the map.
    pub(super) fn survival_loops(&self, audio: &Audio) -> Vec<(mc_data::SoundId, f32, f32, f32)> {
        let mut out = Vec::new();
        if self.view.status.survival.is_none() {
            return out;
        }
        let (library, _) = audio.library();
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

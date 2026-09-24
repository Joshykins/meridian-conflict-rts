//! What the match says and plays about survival: the rounds, the engine's
//! ray, and the replication nodes it raises (bonus objectives). Everything
//! here reads the sim's survival events and status; nothing is decided here.

use super::Game;
use crate::audio::{Audio, Sfx};
use crate::hud::survival::VIOLET;
use crate::ui::palette;
use glam::Vec3;
use mc_sim::SimEvent;

/// Good news: a node down.
const GOOD: u32 = crate::hud::HEALTHY;

impl Game {
    /// Toasts and stingers for survival's events; ignores everything else.
    pub(super) fn note_survival(&mut self, event: &SimEvent, audio: &Audio) {
        let site = |s: u8| {
            self.hud
                .survival_sites
                .get(s as usize)
                .cloned()
                .unwrap_or_else(|| format!("Site {}", s + 1))
        };
        let name = |b: mc_data::BlueprintId| self.blueprints.unit(b).name.clone();
        let (text, color, sound) = match event {
            SimEvent::RoundPrinting { round } => {
                let of = self.view.status.survival.as_ref().map_or(0, |s| s.rounds);
                let text = if of != 0 && *round == of {
                    format!("Final Round  \u{b7}  The Engine Is Replicating")
                } else {
                    format!("Round {round}  \u{b7}  The Engine Is Replicating")
                };
                (text, VIOLET, "survival_round")
            }
            SimEvent::RoundLaunched { round, units } => (
                format!("Round {round} Inbound  \u{b7}  {units} Units"),
                palette::WARN,
                "survival_launch",
            ),
            SimEvent::NodeRaising { site: s, product, .. } => (
                format!("Replication Ray Firing  \u{b7}  {}  \u{b7}  Will Print {}", site(*s), name(*product)),
                VIOLET,
                "survival_node_raising",
            ),
            SimEvent::NodeOnline { site: s, product, .. } => (
                format!("Node Online at {}  \u{b7}  Printing {}  \u{b7}  Bonus Objective", site(*s), name(*product)),
                VIOLET,
                "survival_node_online",
            ),
            SimEvent::NodeDestroyed { site: s, wreck, raised, .. } => (
                if *raised {
                    format!("Node Destroyed at {}  \u{b7}  Wreck Worth {wreck} Mass", site(*s))
                } else {
                    format!("Node Cut Down Before It Rose at {}", site(*s))
                },
                GOOD,
                "survival_node_down",
            ),
            SimEvent::SurvivalWon { rounds } => {
                audio.play(Sfx::Victory);
                self.hud.toast(format!("Held Through {rounds} Rounds"), GOOD);
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
        for (kind, name, floor) in [(ray, "replication_ray", 0.22), (print, "replication_print", 0.0)] {
            let Some(sound) = library.id_of(name) else { continue };
            let mut sum = (0.0f32, 0.0f32);
            for b in self.view.frame.beams.iter().filter(|b| b.kind == kind) {
                let (gain, pan) = self.hear(Vec3::from(b.to));
                let (g2, p2) = self.hear(Vec3::from(b.from));
                let gain = gain.max(g2 * 0.6).max(floor);
                let pan = if gain > 0.0 { pan } else { p2 };
                sum = (sum.0 + gain * gain, sum.1 + gain * gain * pan);
            }
            if sum.0 > 0.0 {
                out.push((sound, (sum.0.sqrt() * 0.55).min(0.9), sum.1 / sum.0.max(1e-9), 1.0));
            }
        }
        out
    }
}

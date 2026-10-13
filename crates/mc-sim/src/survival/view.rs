//! What survival shows: the ray and the print beams for the renderer, and
//! the status the HUD reads. Presentation only: nothing here writes back.

use super::{Phase, Spot, SurvivalStatus, BEAM_PRINT, BEAM_REPLICATION_RAY, RAISE_TICKS};
use crate::mirror::NodeStatus;
use crate::reclaim::BeamInstance;
use mc_core::{Angle, FxVec2};

impl crate::World {
    /// The ray and the print beams, for the renderer. The ray is always
    /// shown; a print beam only where the viewer sees either end.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: fills the render frame's beam instances"
    )]
    pub(crate) fn write_survival_beams(
        &self,
        viewer: Option<u8>,
        out: &mut Vec<BeamInstance>,
        sources: &mut Vec<u32>,
    ) {
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        let st = &self.state;
        let seen = |p: FxVec2| {
            viewer.is_none_or(|v| !st.fog_enabled || self.fog.is_detected(p, self.team_mask(v)))
        };
        let now = st.tick;
        let ground = |at: FxVec2| self.terrain.height_at(at).max(self.terrain.water_level());
        let heart = s.config.engine;
        let lens = heart.extend(ground(heart) + s.config.ray_height);
        for n in s.nodes.iter().filter(|n| !n.raised) {
            let Some(row) = st.units.row(n.unit) else {
                continue;
            };
            let top = st.units.pos[row].extend(st.units.z[row] + n.height);
            let t = ((now - n.started) as f32 / RAISE_TICKS as f32).min(1.0);
            sources.push(n.unit.0 | 1 << 30);
            out.push(BeamInstance {
                from: lens.to_f32(),
                kind: BEAM_REPLICATION_RAY,
                to_prev: top.to_f32(),
                radius: 7.0,
                to: top.to_f32(),
                height: t,
            });
        }
        // Each Shaper's beam on the piece of its base it is printing.
        for n in &s.nodes {
            for piece in n.pieces.iter().filter(|p| !p.raised) {
                let (Some(row), Some(from)) =
                    (st.units.row(piece.unit), self.shaper_emitter(n.unit))
                else {
                    continue;
                };
                let at = st.units.pos[row];
                if !seen(at) && !seen(from.xy()) {
                    continue;
                }
                let bp = self.bp(row);
                sources.push(piece.unit.0 | 1 << 31);
                out.push(BeamInstance {
                    from: from.to_f32(),
                    kind: BEAM_PRINT,
                    to_prev: at.extend(st.units.z[row]).to_f32(),
                    radius: bp.radius.to_f32(),
                    to: at.extend(st.units.z[row]).to_f32(),
                    height: bp.height.to_f32(),
                });
            }
        }
        for p in &s.printing {
            let Some(row) = st.units.row(p.unit) else {
                continue;
            };
            let at = st.units.pos[row];
            let from = match p.spot {
                Spot::Pad(_) => {
                    let Some(from) = self.shaper_emitter(p.source) else {
                        continue;
                    };
                    from
                }
                Spot::Bay(b) => {
                    let Some(bay) = s.config.bays.get(b as usize) else {
                        continue;
                    };
                    let head = FxVec2::new(bay.emitter.x, bay.emitter.y);
                    head.extend(ground(bay.at) + bay.emitter.z)
                }
                // A Gatewright's fleet prints in the air under its beam.
                Spot::Berth { .. } => {
                    let Some(from) = self.shaper_emitter(p.source) else {
                        continue;
                    };
                    from
                }
                // A squad comes out of its warp all but whole, with no beam.
                Spot::WarpIn { .. } => continue,
            };
            if !seen(at) && !seen(from.xy()) {
                continue;
            }
            // A great bay's projector works with one either side of it, across the bay.
            let great = match p.spot {
                Spot::Bay(b) => s.config.bays.get(b as usize).filter(|b| b.great()),
                Spot::Pad(_) | Spot::WarpIn { .. } | Spot::Berth { .. } => None,
            };
            if let Some(bay) = great {
                let bp = self.bp(row);
                let across =
                    FxVec2::from_angle(bay.heading + Angle::QUARTER_TURN) * (bp.radius * 3 / 4);
                for k in [-1, 1] {
                    let side = FxVec2::new(from.x, from.y) + across * mc_core::Fx::from_int(k);
                    sources.push(p.unit.0 | 1 << 31);
                    out.push(BeamInstance {
                        from: side.extend(from.z).to_f32(),
                        kind: BEAM_PRINT,
                        to_prev: st.units.prev_pos[row].extend(st.units.prev_z[row]).to_f32(),
                        radius: bp.radius.to_f32(),
                        to: at.extend(st.units.z[row]).to_f32(),
                        height: bp.height.to_f32(),
                    });
                }
            }
            let bp = self.bp(row);
            sources.push(p.unit.0 | 1 << 31);
            out.push(BeamInstance {
                from: from.to_f32(),
                kind: BEAM_PRINT,
                to_prev: st.units.prev_pos[row].extend(st.units.prev_z[row]).to_f32(),
                radius: bp.radius.to_f32(),
                to: at.extend(st.units.z[row]).to_f32(),
                height: bp.height.to_f32(),
            });
        }
    }

    /// Where a Shaper's print beams leave from: half way up it as it stands built.
    fn shaper_emitter(&self, shaper: crate::UnitId) -> Option<mc_core::FxVec3> {
        let s = self.state.survival.as_ref()?;
        let row = self.state.units.row(shaper)?;
        let built = s.nodes.iter().find(|n| n.unit == shaper)?.height;
        Some(self.state.units.pos[row].extend(self.state.units.z[row] + built / 2))
    }

    /// Where the replication nodes stand, raised or rising.
    pub fn survival_nodes(&self) -> Vec<FxVec2> {
        let Some(s) = self.state.survival.as_ref() else {
            return Vec::new();
        };
        s.nodes
            .iter()
            .filter_map(|n| {
                self.state
                    .units
                    .row(n.unit)
                    .map(|r| self.state.units.pos[r])
            })
            .collect()
    }

    /// The Shaper standing (or rising) on site `site`, if one is.
    pub fn shaper_unit(&self, site: u8) -> Option<crate::UnitId> {
        let s = self.state.survival.as_ref()?;
        s.nodes
            .iter()
            .find(|n| n.site == site && self.state.units.row(n.unit).is_some())
            .map(|n| n.unit)
    }

    /// Ids of units a replicator is printing right now (sorted), for the mirror's violet fill.
    pub fn survival_printing(&self) -> Vec<u32> {
        let Some(s) = self.state.survival.as_ref() else {
            return Vec::new();
        };
        let mut ids: Vec<u32> = s
            .printing
            .iter()
            .map(|p| p.unit)
            .chain(s.nodes.iter().filter(|n| !n.raised).map(|n| n.unit))
            .chain(
                s.nodes
                    .iter()
                    .flat_map(|n| n.pieces.iter().filter(|p| !p.raised).map(|p| p.unit)),
            )
            .map(|u| u.0)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// How awake the facility is, 0.15..1 (zero outside survival): asleep
    /// before the first round, the rounds wake it, and each Shaper online more.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        reason = "presentation: the renderer's Precursor glow and the survival HUD read it"
    )]
    pub fn survival_activity(&self) -> f32 {
        let Some(s) = self.state.survival.as_ref() else {
            return 0.0;
        };
        let rules = s.config.rules;
        let online = s.nodes.iter().filter(|n| n.raised).count() as f32;
        let rounds = if rules.rounds == 0 {
            20.0
        } else {
            rules.rounds as f32
        };
        (0.15 + 0.55 * (s.round as f32 / rounds).min(1.0) + 0.04 * online).min(1.0)
    }

    /// The Shaper `unit` is, or whose base it is part of, and which piece of it
    /// (None: the Shaper itself). None for any other unit.
    pub fn shaper_of(
        &self,
        unit: crate::UnitId,
    ) -> Option<(crate::UnitId, Option<super::PieceKind>)> {
        let s = self.state.survival.as_ref()?;
        s.nodes.iter().find_map(|n| {
            if n.unit == unit {
                return Some((n.unit, None));
            }
            let p = n.pieces.iter().find(|p| p.unit == unit)?;
            Some((n.unit, Some(p.kind)))
        })
    }

    /// What the HUD shows. None outside survival.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: the survival HUD reads it"
    )]
    pub fn survival_status(&self) -> Option<SurvivalStatus> {
        let s = self.state.survival.as_ref()?;
        let st = &self.state;
        let rules = s.config.rules;
        let side = s.config.engine_player;
        let round_size: u16 = s
            .incoming
            .iter()
            .flatten()
            .fold(0u16, |a, b| a.saturating_add(*b));
        let left = s
            .queue
            .iter()
            .map(|f| f.units.len())
            .sum::<usize>()
            .saturating_add(s.printing.iter().filter(|p| p.round).count())
            .min(u16::MAX as usize) as u16;
        let nodes = s
            .nodes
            .iter()
            .filter_map(|n| {
                let row = st.units.row(n.unit)?;
                let raised = if n.raised {
                    1.0
                } else {
                    ((st.tick - n.started) as f32 / RAISE_TICKS as f32).min(0.999)
                };
                Some(NodeStatus {
                    unit: n.unit,
                    site: n.site,
                    kind: n.kind,
                    tier: super::shaper_tier(n.level),
                    pos: st.units.pos[row].to_f32(),
                    domain: n.domain,
                    products: n.products.clone(),
                    level: n.level,
                    raised,
                    printed: n.printed,
                })
            })
            .collect();
        // The round the next tier starts in, as the rounds now run.
        let now_round = s.round.max(1);
        let tier = rules.tier_at(now_round);
        let next_tier = (tier < rules.tier_cap.min(mc_data::MAX_TECH))
            .then(|| rules.first_round_of(tier + 1).map(|r| (tier + 1, r)))
            .flatten();
        let hostile = self.hostile_count(side);
        Some(SurvivalStatus {
            round: s.round,
            rounds: rules.rounds,
            phase: s.phase,
            next_in: match s.phase {
                Phase::Grace | Phase::Launched => s.next_at.saturating_sub(st.tick),
                _ => 0,
            },
            tier,
            next_tier,
            incoming: s.incoming,
            heavy: s.incoming_heavy.clone(),
            printed: (round_size.saturating_sub(left), round_size),
            remaining: s
                .waves
                .iter()
                .filter(|u| st.units.row(**u).is_some())
                .count() as u32,
            hostile: hostile as u32,
            held: (hostile >= super::FIELD_CAP
                && (!s.queue.is_empty() || s.nodes.iter().any(|n| n.raised)))
                || s.nodes.iter().any(|n| n.hold == super::Hold::FieldFull),
            dampened: s.nodes.iter().any(|n| n.hold == super::Hold::Dampened),
            engine: s.config.engine.to_f32(),
            activity: self.survival_activity(),
            nodes,
            reseeding: s
                .reseed_from
                .iter()
                .enumerate()
                .filter(|(i, r)| **r > s.round && !s.nodes.iter().any(|n| n.site as usize == *i))
                .map(|(i, r)| (i.min(u8::MAX as usize) as u8, *r))
                .collect(),
            catch_up: rules.catch_up_level(now_round),
            nodes_destroyed: s.nodes_destroyed,
            shapers_next: match s.phase {
                Phase::Grace | Phase::Launched => rules.nodes_raised(s.round + 1),
                Phase::Printing => rules.nodes_raised(s.round.max(1)),
                Phase::Final | Phase::Won => 0,
            }
            .min(rules.node_limit().saturating_sub(s.nodes.len())) as u8,
            reclaim: st
                .players
                .iter()
                .enumerate()
                .filter(|(p, _)| *p as u8 != side)
                .map(|(_, pl)| pl.reclaim_income.to_f32())
                .sum(),
            engine_player: side,
        })
    }
}

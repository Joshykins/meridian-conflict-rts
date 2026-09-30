//! What a match is set up with, as it travels in a start message's options:
//! the same for a match on one machine, a network match and a replay.
//!
//! The host writes it; every machine builds the same match from it. Each
//! person in a network match adds their own seat's choices (`SeatChoice`, in
//! their lobby setup) and those win over the host's template for that seat.

use mc_sim::tables::Controller;
use mc_sim::{MatchConfig, SurvivalConfig};
use serde::{Deserialize, Serialize};

/// The most a start message's options may take to decode: they come from another
/// machine, so a hostile length inside them must not decide what is allocated.
pub const MAX_OPTIONS_BYTES: u64 = 1 << 20;
/// The most a seat's choices may take.
const MAX_SEAT_BYTES: u64 = 1 << 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchOptions {
    /// Every seat as the host set it up; people who joined take theirs over.
    pub config: MatchConfig,
    /// Set for a survival match.
    pub survival: Option<SurvivalConfig>,
    /// Seat colours, linear RGB, by seat.
    pub colors: crate::setup::Palette,
    /// The map as the lobby shows it; `map_id` is what every machine loads.
    pub map: String,
    pub map_id: u64,
}

/// One person's own choices for their seat, sent as their lobby setup.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SeatChoice {
    /// A faction key, or empty for the host's choice.
    pub faction: String,
    /// A race dealt at the start, from the match seed (over `faction`).
    pub random: bool,
}

impl SeatChoice {
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Option<SeatChoice> {
        mc_sim::decode_untrusted(bytes, MAX_SEAT_BYTES).ok()
    }
}

impl MatchOptions {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| e.to_string())
    }

    pub fn decode(bytes: &[u8]) -> Result<MatchOptions, String> {
        mc_sim::decode_untrusted(bytes, MAX_OPTIONS_BYTES)
            .map_err(|e| format!("the host sent unreadable match options: {e}"))
    }

    /// The match every machine builds from a session's start message. Seats a person
    /// joined become theirs: their name, their controller, their own choices.
    pub fn from_start(start: &mc_net::MatchStart) -> Result<MatchOptions, String> {
        let mut options = MatchOptions::decode(&start.options)?;
        options.config.seed = start.seed;
        let seats = options.config.players.len();
        for p in &start.players {
            let seat = options
                .config
                .players
                .get_mut(p.slot.index())
                .ok_or(format!(
                    "{} joined seat {} but the match has {seats} seats",
                    p.name,
                    p.slot.0 + 1
                ))?;
            seat.controller = Controller::Human;
            seat.name = p.name.clone();
            match SeatChoice::decode(&p.data) {
                Some(c) if c.random => {
                    let race = crate::ui::faction::Pick::Random.resolve(start.seed, p.slot.index());
                    seat.faction = crate::ui::faction::race_key(race);
                }
                Some(c) if !c.faction.is_empty() => seat.faction = c.faction,
                _ => {}
            }
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::PlayerId;
    use mc_sim::PlayerSetup;

    fn seat(name: &str, controller: Controller) -> PlayerSetup {
        PlayerSetup {
            name: name.into(),
            faction: "Aster".into(),
            ai: Default::default(),
            team: 0,
            controller,
            start: 0,
        }
    }

    #[test]
    fn a_joined_seat_takes_its_persons_name_and_choices() {
        let options = MatchOptions {
            config: MatchConfig {
                seed: 0,
                players: vec![seat("host", Controller::Human), seat("AI", Controller::Ai)],
                cheats: false,
                fog: true,
                spawn_commanders: true,
            },
            survival: None,
            colors: crate::setup::TEAM_COLORS,
            map: "twin_shoals".into(),
            map_id: 9,
        };
        let start = mc_net::MatchStart {
            content: Default::default(),
            seed: 77,
            input_delay: 2,
            players: vec![mc_net::PlayerSetup {
                slot: PlayerId(1),
                name: "friend".into(),
                data: SeatChoice {
                    faction: "Regency".into(),
                    random: false,
                }
                .encode(),
            }],
            options: options.encode().unwrap(),
        };
        let built = MatchOptions::from_start(&start).unwrap();
        assert_eq!(built.config.seed, 77);
        let friend = &built.config.players[1];
        assert_eq!(
            (
                friend.name.as_str(),
                friend.controller,
                friend.faction.as_str()
            ),
            ("friend", Controller::Human, "Regency")
        );
        assert_eq!(built.map_id, 9);
        // Garbage from another machine is an error, never a panic.
        assert!(MatchOptions::decode(&[0xFF; 40]).is_err());
    }
}

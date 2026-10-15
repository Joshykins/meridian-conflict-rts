//! What a match is set up with, as it travels in a start message's options:
//! the same for a match on one machine, a network match and a replay.
//!
//! The host writes it; every machine builds the same match from it. Each
//! person in a network match adds their own seat's choices (`SeatChoice`, in
//! their lobby setup) and those win over the host's template for that seat.

use mc_data::weather::SkyChoice;
use mc_sim::tables::Controller;
use mc_sim::{FallenArmy, MatchConfig, SurvivalConfig};
use serde::{Deserialize, Serialize};

/// The most a start message's options may take to decode: they come from another
/// machine, so a hostile length inside them must not decide what is allocated.
pub const MAX_OPTIONS_BYTES: u64 = 1 << 20;
/// The most a seat's choices may take.
const MAX_SEAT_BYTES: u64 = 1 << 10;
/// Parts the zone deal's draws from the races' (`Pick::resolve`) on one seed.
const ZONE_SALT: u64 = 0x2F0E_5A11_D0C4_7E3B;

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
    /// The weather and time of day the host picked; every machine (and a replay)
    /// shows the match under it. Presentation only: the sim never reads it.
    pub sky: SkyChoice,
    pub mission: Option<mc_sim::mission::Definition>,
    /// What becomes of a defeated side's forces.
    pub fallen_army: FallenArmy,
    /// Seats whose landing zone is dealt at the start (`deal_zones`).
    pub random_zones: RandomZones,
}

/// Seats that land at random, and the zones they are dealt from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomZones {
    /// One bit per player (`mc_core::player_bit`).
    pub seats: mc_core::PlayerMask,
    /// The map start positions no chosen seat holds.
    pub pool: Vec<u8>,
}

/// One person's own choices for their seat, sent as their lobby setup.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SeatChoice {
    /// A faction key, or empty for the host's choice.
    pub faction: String,
    /// A race dealt at the start, from the match seed (over `faction`).
    pub random: bool,
    /// The colour last asked of the host, who gives it when nobody else wears it.
    pub color: Option<ColorAsk>,
}

/// A colour a person asks the host for. `n` counts their asks, so asking again for
/// a colour the host has since moved them off is a new ask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorAsk {
    /// Index into `setup::TEAM_COLORS`.
    pub color: u8,
    pub n: u16,
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
        let mut bytes = b"MCOPT4\0".to_vec();
        bytes.extend(bincode::serialize(self).map_err(|e| e.to_string())?);
        if bytes.len() as u64 > MAX_OPTIONS_BYTES {
            return Err("Match options exceed 1 MiB".into());
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<MatchOptions, String> {
        let unreadable = |e| format!("the host sent unreadable match options: {e}");
        if let Some(body) = bytes.strip_prefix(b"MCOPT4\0") {
            mc_sim::decode_untrusted(body, MAX_OPTIONS_BYTES).map_err(unreadable)
        } else if let Some(body) = bytes.strip_prefix(b"MCOPT3\0") {
            let old: OptionsV3 =
                mc_sim::decode_untrusted(body, MAX_OPTIONS_BYTES).map_err(unreadable)?;
            Ok(Self {
                config: old.config,
                survival: old.survival,
                colors: old.colors,
                map: old.map,
                map_id: old.map_id,
                sky: old.sky,
                mission: old.mission,
                fallen_army: old.fallen_army,
                random_zones: RandomZones::default(),
            })
        } else if let Some(body) = bytes.strip_prefix(b"MCOPT2\0") {
            let old: OptionsV2 =
                mc_sim::decode_untrusted(body, MAX_OPTIONS_BYTES).map_err(unreadable)?;
            Ok(Self {
                config: old.config,
                survival: old.survival,
                colors: old.colors,
                map: old.map,
                map_id: old.map_id,
                sky: old.sky,
                mission: old.mission,
                fallen_army: FallenArmy::default(),
                random_zones: RandomZones::default(),
            })
        } else {
            let old: LegacyOptions = mc_sim::decode_untrusted(bytes, MAX_OPTIONS_BYTES)
                .map_err(|e| format!("unreadable legacy match options: {e}"))?;
            Ok(Self {
                config: old.config,
                survival: old.survival,
                colors: old.colors,
                map: old.map,
                map_id: old.map_id,
                sky: old.sky,
                mission: None,
                fallen_army: FallenArmy::default(),
                random_zones: RandomZones::default(),
            })
        }
    }

    /// Deals each random seat a zone from the pool, shuffled by the match seed, so
    /// every machine (and a replay) lands them in the same places. Nothing is
    /// random afterwards. Fails when the host sent fewer zones than random seats.
    pub fn deal_zones(&mut self) -> Result<(), String> {
        let RandomZones { seats, mut pool } = std::mem::take(&mut self.random_zones);
        for i in (1..pool.len()).rev() {
            let j = (crate::ui::faction::mix(self.config.seed ^ ZONE_SALT, i) % (i as u64 + 1))
                as usize;
            pool.swap(i, j);
        }
        let mut dealt = pool.into_iter();
        for (i, p) in self.config.players.iter_mut().enumerate() {
            if i < mc_core::MAX_PLAYERS && seats & mc_core::player_bit(i as u8) != 0 {
                p.start = dealt
                    .next()
                    .ok_or("the host sent more random landing zones than zones to deal")?;
            }
        }
        Ok(())
    }

    /// The match every machine builds from a session's start message. Seats a person
    /// joined become theirs: their name, their controller, their own choices.
    pub fn from_start(start: &mc_net::MatchStart) -> Result<MatchOptions, String> {
        let mut options = MatchOptions::decode(&start.options)?;
        options.config.seed = start.seed;
        options.deal_zones()?;
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
    fn old_replays_decode_without_a_mission() {
        let legacy = LegacyOptions {
            config: MatchConfig {
                seed: 1,
                players: vec![seat("old", Controller::Human)],
                cheats: false,
                fog: true,
                spawn_commanders: true,
            },
            survival: None,
            colors: crate::setup::TEAM_COLORS,
            map: "old-map".into(),
            map_id: 7,
            sky: Default::default(),
        };
        let bytes = bincode::serialize(&legacy).unwrap();
        let decoded = MatchOptions::decode(&bytes).unwrap();
        assert_eq!(decoded.map_id, 7);
        assert!(decoded.mission.is_none());
        let versioned = decoded.encode().unwrap();
        assert!(versioned.starts_with(b"MCOPT4\0"));
        assert_eq!(MatchOptions::decode(&versioned).unwrap().map, "old-map");
    }

    #[test]
    fn mcopt3_options_decode_with_nobody_random() {
        let v3 = OptionsV3 {
            config: options(1, 1).config,
            survival: None,
            colors: crate::setup::TEAM_COLORS,
            map: "v3-map".into(),
            map_id: 8,
            sky: Default::default(),
            mission: None,
            fallen_army: FallenArmy::ToAlly,
        };
        let mut bytes = b"MCOPT3\0".to_vec();
        bytes.extend(bincode::serialize(&v3).unwrap());
        let decoded = MatchOptions::decode(&bytes).unwrap();
        assert_eq!(
            (decoded.map_id, decoded.fallen_army, decoded.random_zones),
            (8, FallenArmy::ToAlly, RandomZones::default())
        );
    }

    fn options(seed: u64, seats: usize) -> MatchOptions {
        MatchOptions {
            config: MatchConfig {
                seed,
                players: (0..seats).map(|_| seat("AI", Controller::Ai)).collect(),
                cheats: false,
                fog: true,
                spawn_commanders: true,
            },
            survival: None,
            colors: crate::setup::TEAM_COLORS,
            map: "m".into(),
            map_id: 1,
            sky: SkyChoice::default(),
            mission: None,
            fallen_army: FallenArmy::default(),
            random_zones: RandomZones::default(),
        }
    }

    fn dealt(seed: u64) -> Vec<u8> {
        // Seat 0 chose zone 3; seats 1, 2 and 4 land at random among the rest.
        let mut o = options(seed, 5);
        o.config.players[0].start = 3;
        o.config.players[3].start = 6;
        o.random_zones = RandomZones {
            seats: 0b10110,
            pool: vec![0, 1, 2, 4, 5],
        };
        o.deal_zones().unwrap();
        assert_eq!(
            o.random_zones,
            RandomZones::default(),
            "nothing random after"
        );
        o.config.players.iter().map(|p| p.start).collect()
    }

    #[test]
    fn random_seats_are_dealt_distinct_free_zones_from_the_seed() {
        let a = dealt(42);
        assert_eq!(a, dealt(42), "one seed, one deal");
        assert_eq!((a[0], a[3]), (3, 6), "chosen zones stay");
        let mut random = vec![a[1], a[2], a[4]];
        random.sort_unstable();
        random.dedup();
        assert_eq!(random.len(), 3);
        assert!(random.iter().all(|z| [0, 1, 2, 4, 5].contains(z)));
        // Over many seeds every free zone comes up, and seats do not keep one order.
        let deals: Vec<Vec<u8>> = (0..64).map(dealt).collect();
        for z in [0, 1, 2, 4, 5] {
            assert!(
                deals.iter().any(|d| d[1] == z),
                "zone {z} never dealt to seat 1"
            );
        }
    }

    #[test]
    fn a_host_short_of_zones_is_an_error_not_a_panic() {
        let mut o = options(1, 3);
        o.random_zones = RandomZones {
            seats: u32::MAX,
            pool: vec![0],
        };
        assert!(o.deal_zones().is_err());
    }

    #[test]
    fn a_network_start_deals_random_zones_from_the_relays_seed() {
        let mut o = options(0, 2);
        o.random_zones = RandomZones {
            seats: 0b11,
            pool: (0..8).collect(),
        };
        let start = |seed| mc_net::MatchStart {
            content: Default::default(),
            seed,
            input_delay: 2,
            players: Vec::new(),
            options: o.encode().unwrap(),
        };
        let zones = |seed| -> Vec<u8> {
            let built = MatchOptions::from_start(&start(seed)).unwrap();
            built.config.players.iter().map(|p| p.start).collect()
        };
        assert_eq!(zones(5), zones(5));
        assert!((0..16).any(|s| zones(s) != zones(5)), "the seed moves them");
    }

    #[test]
    fn options_from_before_the_fallen_army_rule_lose_the_army() {
        let v2 = OptionsV2 {
            config: MatchConfig {
                seed: 1,
                players: vec![seat("old", Controller::Human)],
                cheats: false,
                fog: true,
                spawn_commanders: true,
            },
            survival: None,
            colors: crate::setup::TEAM_COLORS,
            map: "old-map".into(),
            map_id: 7,
            sky: Default::default(),
            mission: None,
        };
        let mut bytes = b"MCOPT2\0".to_vec();
        bytes.extend(bincode::serialize(&v2).unwrap());
        let decoded = MatchOptions::decode(&bytes).unwrap();
        assert_eq!(decoded.map, "old-map");
        assert_eq!(decoded.fallen_army, FallenArmy::Destroyed);
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
            sky: SkyChoice::default(),
            mission: None,
            fallen_army: FallenArmy::ToAlly,
            random_zones: RandomZones::default(),
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
                    color: None,
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
        assert_eq!(built.fallen_army, FallenArmy::ToAlly);
        // Garbage from another machine is an error, never a panic.
        assert!(MatchOptions::decode(&[0xFF; 40]).is_err());
    }
}

// Original replay/network options, before versioned mission definitions.
#[derive(Serialize, Deserialize)]
struct LegacyOptions {
    config: MatchConfig,
    survival: Option<SurvivalConfig>,
    colors: crate::setup::Palette,
    map: String,
    map_id: u64,
    sky: SkyChoice,
}

// Options before the fallen army rule ("MCOPT2").
#[derive(Serialize, Deserialize)]
struct OptionsV2 {
    config: MatchConfig,
    survival: Option<SurvivalConfig>,
    colors: crate::setup::Palette,
    map: String,
    map_id: u64,
    sky: SkyChoice,
    mission: Option<mc_sim::mission::Definition>,
}

// Options before random landing zones ("MCOPT3").
#[derive(Serialize, Deserialize)]
struct OptionsV3 {
    config: MatchConfig,
    survival: Option<SurvivalConfig>,
    colors: crate::setup::Palette,
    map: String,
    map_id: u64,
    sky: SkyChoice,
    mission: Option<mc_sim::mission::Definition>,
    fallen_army: FallenArmy,
}

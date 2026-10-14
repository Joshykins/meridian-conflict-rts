//! Authored player slots and mission selection, independent of simulation AI state.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapPlayer {
    pub name: String,
    pub faction: String,
    pub team: u8,
    pub start: u8,
    #[serde(default)]
    pub computer: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsError {
    Regions(crate::regions::RegionError),
    Invalid(&'static str),
}
impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Regions(e) => e.fmt(f),
            Self::Invalid(e) => f.write_str(e),
        }
    }
}
impl std::error::Error for SettingsError {}
impl From<crate::regions::RegionError> for SettingsError {
    fn from(value: crate::regions::RegionError) -> Self {
        Self::Regions(value)
    }
}
pub fn validate(players: &[MapPlayer], mission: Option<&str>) -> Result<(), SettingsError> {
    if players.len() > mc_core::MAX_PLAYERS {
        return Err(SettingsError::Invalid("Too many map player slots"));
    }
    for player in players {
        if player.name.is_empty()
            || player.name.len() > 64
            || player.name.chars().any(char::is_control)
        {
            return Err(SettingsError::Invalid(
                "Player names need 1 to 64 bytes without control characters",
            ));
        }
        if player.faction.is_empty()
            || player.faction.len() > 64
            || !player
                .faction
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(SettingsError::Invalid("Invalid player faction key"));
        }
        if usize::from(player.team) >= mc_core::MAX_PLAYERS
            || usize::from(player.start) >= mc_core::MAX_PLAYERS
        {
            return Err(SettingsError::Invalid(
                "Player team or start exceeds the slot limit",
            ));
        }
    }
    if mission.is_some_and(|key| {
        key.is_empty()
            || key.len() > 64
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }) {
        return Err(SettingsError::Invalid("Invalid mission key"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::weather::MapConfig;
    #[test]
    fn settings_reject_untrusted_slot_and_mission_values() {
        for text in [
            r#"(players: [(name: "", faction: "Aster", team: 0, start: 0)])"#,
            r#"(players: [(name: "P", faction: "Aster", team: 32, start: 0)])"#,
            r#"(players: [(name: "P", faction: "../data", team: 0, start: 0)])"#,
            r#"(mission: Some("../mission"))"#,
        ] {
            assert!(MapConfig::parse(text).is_err(), "{text}");
        }
        let original = MapConfig::parse(r#"(mission: Some("vela_m1"), players: [(name: "Company", faction: "Aster", team: 0, start: 0)])"#).unwrap();
        let encoded = ron::to_string(&original).unwrap();
        assert_eq!(MapConfig::parse(&encoded).unwrap(), original);
        assert!(MapConfig::parse("()").unwrap().players.is_empty());
    }
}

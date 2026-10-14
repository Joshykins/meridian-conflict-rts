//! Stage a complete environment so mixed batches are validated before publication.
use super::{Command, Document};
use mc_core::Fx;
use mc_data::weather::MapConfig;

#[derive(Clone, PartialEq)]
pub(super) struct State {
    pub config: MapConfig,
    pub water: Fx,
}
impl State {
    pub fn from_doc(doc: &Document) -> Self {
        Self {
            config: doc.config.clone(),
            water: doc.map.info.water_level,
        }
    }
    pub fn apply(&mut self, command: &Command) -> Result<bool, String> {
        match command {
            Command::Water { level } => {
                if !level.is_finite() || !(-2000.0..=4000.0).contains(level) {
                    return Err("Water level must be between -2000 and 4000 metres".into());
                }
                self.water = Fx::from_f32(*level);
            }
            Command::Time { hour } => {
                if !hour.is_finite() || !(0.0..24.0).contains(hour) {
                    return Err("Hour must be at least 0 and less than 24".into());
                }
                self.config.hour = Some(*hour);
            }
            Command::Climate { region, value } => {
                let mut regions = self.config.regions().to_vec();
                regions
                    .get_mut(*region)
                    .ok_or("Unknown climate region")?
                    .climate = *value;
                self.config
                    .set_regions(regions, self.config.walls().walls().to_vec())
                    .map_err(|e| e.to_string())?;
            }
            Command::Weather { region, preset } => {
                let mut regions = self.config.regions().to_vec();
                let region = regions.get_mut(*region).ok_or("Unknown climate region")?;
                region.weather = *preset;
                // A preset is an explicit replacement, not a hidden mixture with old tweaks.
                region.tweaks = Default::default();
                self.config
                    .set_regions(regions, self.config.walls().walls().to_vec())
                    .map_err(|e| e.to_string())?;
            }
            Command::ClimateRegions {
                regions,
                walls,
                blend,
            } => {
                if !blend.is_finite() || !(0.0..=10000.0).contains(blend) {
                    return Err("Climate blend must be between 0 and 10000 metres".into());
                }
                if regions
                    .iter()
                    .any(|r| r.name.len() > 128 || r.name.chars().any(char::is_control))
                {
                    return Err(
                        "Region names must be at most 128 bytes without control characters".into(),
                    );
                }
                if walls.len() > 32
                    || walls.iter().any(|w| {
                        w.line.len() > 33
                            || w.line
                                .iter()
                                .any(|p| p.0.abs() > 100000.0 || p.1.abs() > 100000.0)
                    })
                {
                    return Err("Climate walls exceed the map coordinate or segment limits".into());
                }
                // Weather tweaks are external floats too: canonical RON must remain finite.
                if regions.iter().any(|r| {
                    let t = r.tweaks;
                    [
                        t.cover,
                        t.storms,
                        t.rain,
                        t.towering,
                        t.scale,
                        t.wind,
                        t.lightning,
                        t.mist,
                    ]
                    .into_iter()
                    .flatten()
                    .any(|v| !v.is_finite())
                }) {
                    return Err("Weather tweaks must be finite".into());
                }
                self.config
                    .set_regions(regions.clone(), walls.clone())
                    .map_err(|e| e.to_string())?;
                self.config.blend = *blend;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

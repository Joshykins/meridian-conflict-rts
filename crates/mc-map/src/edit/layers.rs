//! Authoring layer layouts, shared by brush patches and texture uploads.
use super::EditMap;
use crate::{MapInfo, StreetSample};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Layer {
    Snow = 0,
    Ways = 1,
    Streets = 2,
}
impl Layer {
    pub fn channels(self) -> usize {
        match self {
            Self::Snow => 2,
            Self::Ways => 3,
            Self::Streets => 4,
        }
    }
    pub fn dims(self, info: &MapInfo) -> (u32, u32) {
        match self {
            Self::Streets => info.streets_dims(),
            _ => info.snow_dims(),
        }
    }
    pub fn pitch(self) -> f32 {
        crate::CELL_SIZE_M as f32
            * if self == Self::Streets {
                1.0
            } else {
                crate::format::SNOW_STRIDE as f32
            }
    }
    pub fn defaults(self) -> &'static [u8] {
        match self {
            Self::Snow => &[0, 0],
            Self::Ways => &[0, 128, 128],
            Self::Streets => &NATURAL,
        }
    }
    pub fn read(self, map: &EditMap) -> &[u8] {
        match self {
            Self::Snow => &map.snow,
            Self::Ways => &map.ways,
            Self::Streets => &map.streets,
        }
    }
    pub fn write(self, map: &mut EditMap) -> &mut Vec<u8> {
        match self {
            Self::Snow => &mut map.snow,
            Self::Ways => &mut map.ways,
            Self::Streets => &mut map.streets,
        }
    }
    pub fn initialize(self, map: &mut EditMap) {
        let (w, h) = self.dims(&map.info);
        let bytes = self.write(map);
        if bytes.is_empty() {
            *bytes = self.defaults().repeat((w * h) as usize);
        }
    }
}
// The natural sample's packed representation; checked against StreetSample.
const NATURAL: [u8; 4] = [StreetSample::NATURAL.offset, 0, 0, 0];

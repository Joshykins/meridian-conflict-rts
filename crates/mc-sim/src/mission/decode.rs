//! Bound recursion during decoding, before untrusted conditions reach validation.
use super::Condition;
use serde::{
    de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{fmt, marker::PhantomData};
#[derive(Deserialize)]
enum Tag {
    Elapsed,
    After,
    Inside,
    DeadGroup,
    Unloaded,
    DeadUnit,
    HealthBelow,
    All,
    Any,
}
const TAGS: &[&str] = &[
    "Elapsed",
    "After",
    "Inside",
    "DeadGroup",
    "Unloaded",
    "DeadUnit",
    "HealthBelow",
    "All",
    "Any",
];

impl<'de> Deserialize<'de> for Condition {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Seed(0).deserialize(d)
    }
}
struct Seed(u8);
impl<'de> DeserializeSeed<'de> for Seed {
    type Value = Condition;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Condition, D::Error> {
        if self.0 > 8 {
            return Err(de::Error::custom("mission conditions nested too deeply"));
        }
        d.deserialize_enum("Condition", TAGS, self)
    }
}
#[derive(Deserialize)]
struct After {
    event: String,
    ticks: u32,
}
#[derive(Deserialize)]
struct Inside {
    unit: String,
    region: String,
}
#[derive(Deserialize)]
struct Health {
    unit: String,
    percent: u8,
}

impl<'de> Visitor<'de> for Seed {
    type Value = Condition;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded mission condition")
    }
    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Condition, A::Error> {
        let (tag, v) = data.variant::<Tag>()?;
        Ok(match tag {
            Tag::Elapsed => Condition::Elapsed(v.newtype_variant()?),
            Tag::After => {
                let a: After = v.struct_variant(&["event", "ticks"], Fields(PhantomData))?;
                Condition::After {
                    event: a.event,
                    ticks: a.ticks,
                }
            }
            Tag::Inside => {
                let a: Inside = v.struct_variant(&["unit", "region"], Fields(PhantomData))?;
                Condition::Inside {
                    unit: a.unit,
                    region: a.region,
                }
            }
            Tag::DeadGroup => Condition::DeadGroup(v.newtype_variant()?),
            Tag::Unloaded => Condition::Unloaded(v.newtype_variant()?),
            Tag::DeadUnit => Condition::DeadUnit(v.newtype_variant()?),
            Tag::HealthBelow => {
                let a: Health = v.struct_variant(&["unit", "percent"], Fields(PhantomData))?;
                Condition::HealthBelow {
                    unit: a.unit,
                    percent: a.percent,
                }
            }
            Tag::All => Condition::All(v.newtype_variant_seed(List(self.0 + 1))?),
            Tag::Any => Condition::Any(v.newtype_variant_seed(List(self.0 + 1))?),
        })
    }
}
struct Fields<T>(PhantomData<T>);
impl<'de, T: Deserialize<'de>> Visitor<'de> for Fields<T> {
    type Value = T;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("condition fields")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, a: A) -> Result<T, A::Error> {
        T::deserialize(de::value::SeqAccessDeserializer::new(a))
    }
    fn visit_map<A: MapAccess<'de>>(self, a: A) -> Result<T, A::Error> {
        T::deserialize(de::value::MapAccessDeserializer::new(a))
    }
}
struct List(u8);
impl<'de> DeserializeSeed<'de> for List {
    type Value = Vec<Condition>;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for List {
    type Value = Vec<Condition>;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("at most 32 conditions")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        if seq.size_hint().is_some_and(|n| n > 32) {
            return Err(de::Error::custom("too many mission conditions"));
        }
        let mut conditions = vec![];
        while let Some(condition) = seq.next_element_seed(Seed(self.0))? {
            if conditions.len() == 32 {
                return Err(de::Error::custom("too many mission conditions"));
            }
            conditions.push(condition);
        }
        Ok(conditions)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_untrusted_conditions_fail_during_decode() {
        let mut condition = Condition::Elapsed(0);
        for _ in 0..50 {
            condition = Condition::All(vec![condition]);
        }
        let bytes = bincode::serialize(&condition).unwrap();
        assert!(crate::decode_untrusted::<Condition>(&bytes, 4096).is_err());
    }
    #[test]
    fn structured_conditions_roundtrip_in_binary_and_ron() {
        let original = Condition::All(vec![
            Condition::After {
                event: "landed".into(),
                ticks: 80,
            },
            Condition::Inside {
                unit: "merrick".into(),
                region: "square".into(),
            },
        ]);
        let bytes = bincode::serialize(&original).unwrap();
        let copy: Condition = crate::decode_untrusted(&bytes, 4096).unwrap();
        assert_eq!(bincode::serialize(&copy).unwrap(), bytes);
        let text = ron::to_string(&original).unwrap();
        let copy: Condition = ron::from_str(&text).unwrap();
        assert_eq!(bincode::serialize(&copy).unwrap(), bytes);
    }
}

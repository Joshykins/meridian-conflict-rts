//! Structural checks on a `State` that came from outside the process (a snapshot
//! from a peer, a save file). Deserialising only proves the bytes had the right
//! shape; these prove the tables are internally consistent, so a malformed or
//! hostile snapshot is refused at restore instead of panicking in some later tick.
//!
//! The column check reads each table through serde, so a column added to a table
//! is covered without being listed here.

use serde::ser::{self, Impossible, Serialize, SerializeSeq, SerializeStruct};

use crate::slots::Slots;
use crate::world::State;

/// The largest snapshot `World::restore` reads.
pub const MAX_SNAPSHOT_BYTES: u64 = 256 << 20;

/// Decodes bincode (as `bincode::serialize` writes it) from a source that is not
/// trusted, reading at most `limit` bytes: a hostile length prefix cannot make it
/// allocate more than that.
pub fn decode_untrusted<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    limit: u64,
) -> Result<T, String> {
    use bincode::Options;
    bincode::options()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .with_limit(limit)
        .deserialize(bytes)
        .map_err(|e| e.to_string())
}

impl State {
    /// Checks every struct-of-arrays table: all its columns are the same length,
    /// that length agrees with its slot table where it has one, and the slot
    /// tables themselves are consistent.
    pub fn validate(&self) -> Result<(), String> {
        let units = columns("units", &self.units)?;
        slots("units", &self.units.slots, units)?;
        let wrecks = columns("wrecks", &self.wrecks)?;
        slots("wrecks", &self.wrecks.slots, wrecks)?;
        self.orders
            .validate(&self.units)
            .map_err(|e| format!("orders: {e}"))?;
        columns("projectiles", &self.projectiles)?;
        columns("stains", &self.stains)?;
        columns("fires", &self.fires)?;
        columns("pads", &self.pads)?;
        Ok(())
    }

    /// Every id that indexes the blueprint table or the player list is in range, so
    /// no later lookup can index out of bounds.
    pub fn validate_ids(&self, blueprints: usize) -> Result<(), String> {
        let players = self.players.len();
        let bad_bp = |table: &str, ids: &[mc_data::BlueprintId]| {
            ids.iter()
                .find(|b| b.0 as usize >= blueprints)
                .map(|b| format!("{table}: blueprint {} of {blueprints}", b.0))
        };
        let bad_owner = |table: &str, owners: &[u8]| {
            owners
                .iter()
                .find(|&&o| o as usize >= players)
                .map(|o| format!("{table}: owner {o} of {players} players"))
        };
        let problem = bad_bp("units", &self.units.blueprint)
            .or_else(|| bad_bp("wrecks", &self.wrecks.blueprint))
            .or_else(|| bad_bp("projectiles", &self.projectiles.blueprint))
            .or_else(|| bad_owner("units", &self.units.owner))
            .or_else(|| bad_owner("projectiles", &self.projectiles.owner))
            .or_else(|| bad_owner("fires", &self.fires.owner));
        problem.map_or(Ok(()), Err)
    }
}

fn slots(table: &str, slots: &Slots, rows: Option<usize>) -> Result<(), String> {
    slots.validate().map_err(|e| format!("{table}: {e}"))?;
    match rows {
        Some(rows) if rows != slots.rows() => {
            Err(format!("{table}: {rows} rows but {} slots", slots.rows()))
        }
        _ => Ok(()),
    }
}

/// The common length of `value`'s `Vec` fields, or `None` if it has none.
fn columns<T: Serialize>(table: &str, value: &T) -> Result<Option<usize>, String> {
    let mut probe = Columns { lens: Vec::new() };
    value
        .serialize(&mut probe)
        .map_err(|e| format!("{table}: {e}"))?;
    let mut lens = probe.lens.iter();
    let Some(&(first_field, first)) = lens.next() else {
        return Ok(None);
    };
    for &(field, len) in lens {
        if len != first {
            return Err(format!(
                "{table}: column {field} has {len} rows but {first_field} has {first}"
            ));
        }
    }
    Ok(Some(first))
}

#[derive(Debug)]
struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<M: std::fmt::Display>(msg: M) -> Self {
        Error(msg.to_string())
    }
}

/// Serialises a struct just deep enough to record the length of each field that
/// is a sequence.
struct Columns {
    lens: Vec<(&'static str, usize)>,
}

impl SerializeStruct for &mut Columns {
    type Ok = ();
    type Error = Error;

    fn serialize_field<V: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &V,
    ) -> Result<(), Error> {
        if let Some(len) = value.serialize(Length)? {
            self.lens.push((key, len));
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

/// Whatever a field is not a struct of columns.
fn not_a_table<T>() -> Result<T, Error> {
    Err(Error("not a struct of columns".into()))
}

#[expect(
    clippy::disallowed_types,
    reason = "serde's Serializer trait names f32/f64 in its method signatures; they only refuse"
)]
impl ser::Serializer for &mut Columns {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Impossible<(), Error>;

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Error> {
        Ok(self)
    }

    fn serialize_bool(self, _: bool) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_i8(self, _: i8) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_i16(self, _: i16) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_i32(self, _: i32) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_i64(self, _: i64) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_u8(self, _: u8) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_u16(self, _: u16) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_u32(self, _: u32) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_u64(self, _: u64) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_f32(self, _: f32) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_f64(self, _: f64) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_char(self, _: char) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_str(self, _: &str) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_none(self) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_some<V: Serialize + ?Sized>(self, _: &V) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_unit(self) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_newtype_struct<V: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: &V,
    ) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_newtype_variant<V: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &V,
    ) -> Result<(), Error> {
        not_a_table()
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        not_a_table()
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Error> {
        not_a_table()
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        not_a_table()
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        not_a_table()
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        not_a_table()
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        not_a_table()
    }
}

/// The length of a field if it is a sequence (`Some`), else `None`. Elements are
/// never visited.
struct Length;

/// A sequence whose length has been read; its elements are skipped.
struct Counted(usize);

impl SerializeSeq for Counted {
    type Ok = Option<usize>;
    type Error = Error;

    fn serialize_element<V: Serialize + ?Sized>(&mut self, _: &V) -> Result<(), Error> {
        Ok(())
    }

    fn end(self) -> Result<Option<usize>, Error> {
        Ok(Some(self.0))
    }
}

/// Every non-sequence field is simply "not a column".
macro_rules! not_a_column {
    ($($name:ident($($arg:ty),*);)*) => {
        $(fn $name(self, $(_: $arg),*) -> Result<Option<usize>, Error> {
            Ok(None)
        })*
    };
}

#[expect(
    clippy::disallowed_types,
    reason = "serde's Serializer trait names f32/f64 in its method signatures; they only refuse"
)]
impl ser::Serializer for Length {
    type Ok = Option<usize>;
    type Error = Error;
    type SerializeSeq = Counted;
    type SerializeTuple = Skip;
    type SerializeTupleStruct = Skip;
    type SerializeTupleVariant = Skip;
    type SerializeMap = Skip;
    type SerializeStruct = Skip;
    type SerializeStructVariant = Skip;

    fn serialize_seq(self, len: Option<usize>) -> Result<Counted, Error> {
        len.map(Counted)
            .ok_or_else(|| Error("a column without a known length".into()))
    }

    not_a_column! {
        serialize_bool(bool);
        serialize_i8(i8);
        serialize_i16(i16);
        serialize_i32(i32);
        serialize_i64(i64);
        serialize_u8(u8);
        serialize_u16(u16);
        serialize_u32(u32);
        serialize_u64(u64);
        serialize_f32(f32);
        serialize_f64(f64);
        serialize_char(char);
        serialize_str(&str);
        serialize_bytes(&[u8]);
        serialize_none();
        serialize_unit();
        serialize_unit_struct(&'static str);
        serialize_unit_variant(&'static str, u32, &'static str);
    }

    fn serialize_some<V: Serialize + ?Sized>(self, _: &V) -> Result<Option<usize>, Error> {
        Ok(None)
    }
    fn serialize_newtype_struct<V: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: &V,
    ) -> Result<Option<usize>, Error> {
        Ok(None)
    }
    fn serialize_newtype_variant<V: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &V,
    ) -> Result<Option<usize>, Error> {
        Ok(None)
    }
    fn serialize_tuple(self, _: usize) -> Result<Skip, Error> {
        Ok(Skip)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Skip, Error> {
        Ok(Skip)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Skip, Error> {
        Ok(Skip)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Skip, Error> {
        Ok(Skip)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Skip, Error> {
        Ok(Skip)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Skip, Error> {
        Ok(Skip)
    }
}

/// A compound field (a slot table, a fixed array, a map): not a column, and its
/// contents are not visited.
struct Skip;

macro_rules! skip {
    ($($trait:ident { $($method:ident($($arg:ty),*);)* })*) => {
        $(impl ser::$trait for Skip {
            type Ok = Option<usize>;
            type Error = Error;
            $(fn $method<V: Serialize + ?Sized>(&mut self, $(_: $arg,)* _: &V) -> Result<(), Error> {
                Ok(())
            })*
            fn end(self) -> Result<Option<usize>, Error> {
                Ok(None)
            }
        })*
    };
}

skip! {
    SerializeTuple { serialize_element(); }
    SerializeTupleStruct { serialize_field(); }
    SerializeTupleVariant { serialize_field(); }
    SerializeStruct { serialize_field(&'static str); }
    SerializeStructVariant { serialize_field(&'static str); }
}

impl ser::SerializeMap for Skip {
    type Ok = Option<usize>;
    type Error = Error;
    fn serialize_key<V: Serialize + ?Sized>(&mut self, _: &V) -> Result<(), Error> {
        Ok(())
    }
    fn serialize_value<V: Serialize + ?Sized>(&mut self, _: &V) -> Result<(), Error> {
        Ok(())
    }
    fn end(self) -> Result<Option<usize>, Error> {
        Ok(None)
    }
}

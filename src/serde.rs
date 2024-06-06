//! [serde](https://crates.io/crates/serde) utilities for lua

use core::ops::Range;

use crate::{
    error::{Error as LuaError, Result as LuaResult},
    ffi::lua_Integer,
    luaapi::Type,
    prelude::*,
    state::State,
};
use alloc::{
    fmt::Display,
    format,
    string::{String, ToString},
};
use serde::de::{DeserializeOwned, EnumAccess, IntoDeserializer, VariantAccess};
#[rustfmt::skip]
use ::serde::{
    de::{Deserialize, DeserializeSeed, Deserializer, Error as DeErr, MapAccess, SeqAccess, Visitor},
    ser::{
        Error, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
        SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
    },
};

#[derive(Clone, Debug, PartialEq, Display)]
pub enum DesErr {
    // One or more variants that can be created by data structures through the
    // `ser::Error` and `de::Error` traits. For example the Serialize impl for
    // Mutex<T> might return an error because the mutex is poisoned, or the
    // Deserialize impl for a struct may return an error because a required
    // field is missing.
    Message(String),

    // Zero or more variants that can be created directly by the Serializer and
    // Deserializer without going through `ser::Error` and `de::Error`. These
    // are specific to the format, in this case JSON.
    Eof,
    Syntax,
    ExpectedBoolean,
    ExpectedInteger,
    ExpectedString,
    ExpectedNull,
    ExpectedArray,
    ExpectedArrayComma,
    ExpectedArrayEnd,
    ExpectedMap,
    ExpectedMapColon,
    ExpectedMapComma,
    ExpectedMapEnd,
    ExpectedEnum,
    TrailingCharacters,
}

#[cfg(feature = "std")]
impl std::error::Error for DesErr {}

impl DeErr for DesErr {
    fn custom<T: Display>(msg: T) -> Self {
        DesErr::Message(msg.to_string())
    }
}

impl State {
    /// convert a serializable value into a lua value
    #[inline(always)]
    pub fn serialize_to_val<V: Serialize>(&self, v: V) -> LuaResult<ValRef> {
        v.serialize(LuaSerializer(self))
    }

    /// transcode a serializable value from deserializer into a lua value
    #[inline(always)]
    pub fn load_from_deserializer<'l: 'de, 'de, D: Deserializer<'de>>(
        &'l self,
        deserializer: D,
    ) -> Result<ValRef<'l>, LuaError> {
        serde_transcode::transcode(deserializer, LuaSerializer(self))
    }

    /// A metatable attachable to a Lua table to systematically encode it as Array (instead of Map).
    /// As result, encoded Array will contain only sequence part of the table, with the same length as the # operator on that table.
    pub fn array_metatable(&self) -> LuaResult<LuaTable> {
        fn key(_: &LuaTable) -> LuaResult<()> {
            Ok(())
        }

        self.get_or_init_metatable(key)?;
        self.top_val().try_into()
    }

    /// A special value to encode/decode optional (none) values.
    pub fn null_value(&self) -> LuaValue {
        LuaValue::light_userdata(Self::null_value as *const ())
    }
}

/// Wrapper to serializable value
#[derive(Copy, Clone, Deref, DerefMut)]
pub struct SerdeValue<T>(pub T);

#[derive(Copy, Clone, Deref, DerefMut)]
pub struct SerdeOwnedValue<T>(pub T);

impl<T: Default> Default for SerdeValue<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Default> Default for SerdeOwnedValue<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Serialize> ToLua for SerdeValue<T> {
    #[inline(always)]
    fn to_lua<'a>(self, s: &'a State) -> LuaResult<ValRef<'a>> {
        Ok(self.serialize(LuaSerializer(s))?)
    }
}

impl<'a, T: Deserialize<'a> + 'a> FromLua<'a> for SerdeValue<T> {
    #[inline(always)]
    fn from_lua(lua: &'a State, val: ValRef<'a>) -> LuaResult<SerdeValue<T>> {
        val.check_safe_index()?;
        unsafe {
            // Safety: check_safe_index
            let val: &'a ValRef = core::mem::transmute(&val);
            T::deserialize(val).map(SerdeValue).lua_result()
        }
    }
}

impl<'a, T: DeserializeOwned + 'a> FromLua<'a> for SerdeOwnedValue<T> {
    #[inline(always)]
    fn from_lua(lua: &'a State, val: ValRef<'a>) -> LuaResult<SerdeOwnedValue<T>> {
        T::deserialize(&val).map(SerdeOwnedValue).lua_result()
    }
}

impl<'a> ValRef<'a> {
    /// Deserialize a lua value
    #[inline(always)]
    pub fn deserialize<T: Deserialize<'a>>(&'a self) -> Result<T, DesErr> {
        T::deserialize(self)
    }

    /// Transcode a lua value to another serialize format
    #[inline(always)]
    pub fn transcode<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde_transcode::transcode(self, serializer)
    }
}

struct LuaSerializer<'a>(&'a State);
struct LuaTableSerializer<'a> {
    t: LuaTable<'a>,
    k: Option<ValRef<'a>>,
    // nil range in lua array part
    nil: Range<i64>,
}

impl<'a> LuaTableSerializer<'a> {
    fn restore_nil(&mut self) -> LuaResult<()> {
        for i in self.nil.clone() {
            self.t.raw_seti(i + 1, ())?;
        }
        Ok(())
    }
}

impl<'a> SerializeSeq for LuaTableSerializer<'a> {
    type Ok = ValRef<'a>;
    type Error = LuaError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        use crate::luaapi::UnsafeLuaApi;

        let state = self.t.state;
        state.check_stack(2)?;
        state.push(SerdeValue(value))?;

        let top_is_nil = state.is_nil(-1);
        if top_is_nil {
            state.pop(1);
            state.push(false)?;
        }
        state.raw_seti(self.t.index, self.nil.end + 1);
        if top_is_nil {
            self.nil.end += 1
        } else {
            self.restore_nil()?;
            self.nil.end += 1;
            self.nil.start = self.nil.end;
        }

        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.restore_nil()?;
        Ok(self.t.into())
    }
}

impl<'a> SerializeTuple for LuaTableSerializer<'a> {
    type Ok = ValRef<'a>;
    type Error = LuaError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl<'a> SerializeTupleStruct for LuaTableSerializer<'a> {
    type Ok = ValRef<'a>;
    type Error = LuaError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.k
            .as_ref()
            .and_then(ValRef::as_table)
            .ok_or("variant value not set")
            .lua_result()?
            .push(SerdeValue(value))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl<'a> SerializeTupleVariant for LuaTableSerializer<'a> {
    type Ok = ValRef<'a>;
    type Error = LuaError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        SerializeTupleStruct::serialize_field(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeTupleStruct::end(self)
    }
}

impl<'a> LuaTableSerializer<'a> {
    fn begin(s: &'a State, len: usize) -> LuaResult<Self> {
        s.new_table_with_size(0, len as _).map(|val| Self {
            t: val,
            k: None,
            nil: 0..0,
        })
    }

    fn begin_array(s: &'a State, len: usize) -> LuaResult<Self> {
        let t = s.new_array_table(len)?;
        Ok(Self {
            t,
            k: None,
            nil: 0..0,
        })
    }
}

impl<'a> SerializeStruct for LuaTableSerializer<'a> {
    type Ok = ValRef<'a>;
    type Error = LuaError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.t.raw_set(key, SerdeValue(value))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeMap::end(self)
    }
}

impl<'a> SerializeMap for LuaTableSerializer<'a> {
    type Ok = ValRef<'a>;
    type Error = LuaError;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.k.replace(self.t.state.new_val(SerdeValue(key))?);
        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.t.raw_set(
            self.k.take().ok_or("no key").lua_result()?,
            SerdeValue(value),
        )
    }

    fn serialize_entry<K: ?Sized, V: ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), Self::Error>
    where
        K: Serialize,
        V: Serialize,
    {
        self.t.raw_set(SerdeValue(key), SerdeValue(value))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.t.into())
    }
}

impl<'a> SerializeStructVariant for LuaTableSerializer<'a> {
    type Ok = ValRef<'a>;
    type Error = LuaError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.k
            .as_ref()
            .and_then(ValRef::as_table)
            .ok_or("variant value not set")
            .lua_result()?
            .raw_set(key, SerdeValue(value))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeStruct::end(self)
    }
}

impl<'a> Serializer for LuaSerializer<'a> {
    type Ok = ValRef<'a>;
    type Error = LuaError;

    type SerializeSeq = LuaTableSerializer<'a>;
    type SerializeMap = LuaTableSerializer<'a>;
    type SerializeTuple = LuaTableSerializer<'a>;
    type SerializeStruct = LuaTableSerializer<'a>;
    type SerializeStructVariant = LuaTableSerializer<'a>;
    type SerializeTupleStruct = LuaTableSerializer<'a>;
    type SerializeTupleVariant = LuaTableSerializer<'a>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.0.new_val(v)
    }
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as _)
    }
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as _)
    }
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as _)
    }
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.0.new_val(v)
    }
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as _)
    }
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as _)
    }
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as _)
    }
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as _)
    }
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(v as _)
    }
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.0.new_val(v)
    }
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut dst = [0u8; 10];
        self.0.new_val(v.encode_utf8(&mut dst).as_bytes())
    }
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.serialize_bytes(v.as_bytes())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.0.new_val(v)
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.0.new_val(())
    }
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        self.0.new_val(SerdeValue(value))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.0.new_val(())
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        let mut s = LuaTableSerializer::begin(self.0, 1)?;
        SerializeStruct::serialize_field(&mut s, "__unit_struct", name)?;
        SerializeStruct::end(s)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        let t = self.0.new_table_with_size(0, 4)?;
        t.raw_set(-1, variant_index)?;
        // use [0] store variant name
        t.raw_set(0, variant)?;
        // use false instead of nil for unit variant to prevent the variant key disappear when deserializing
        t.raw_set(variant, false)?;
        Ok(t.into())
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        let t = self.0.new_table_with_size(0, 4)?;
        t.raw_set(-1, variant_index)?;
        // use [0] store variant name
        t.raw_set(0, variant)?;
        t.raw_set(variant, SerdeValue(value))?;
        Ok(t.into())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        let mut s = LuaTableSerializer::begin_array(self.0, len)?;
        s.t.raw_set(-1, variant_index)?;
        // use [0] store variant name
        s.t.raw_set(0, variant)?;
        let t = self.0.new_table()?;
        s.k.replace(t.0.clone());
        s.t.raw_set(variant, t)?;
        Ok(s)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        let mut s = LuaTableSerializer::begin(self.0, 1)?;
        s.t.raw_set(-1, variant_index)?;
        // use [0] store variant name
        s.t.raw_set(0, variant)?;
        let t = self.0.new_table()?;
        s.k.replace(t.0.clone());
        s.t.raw_set(variant, t)?;
        Ok(s)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(LuaTableSerializer::begin_array(self.0, len.unwrap_or(0))?)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(LuaTableSerializer::begin_array(self.0, len)?)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(LuaTableSerializer::begin_array(self.0, len)?)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(LuaTableSerializer::begin(self.0, len.unwrap_or(0))?)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(LuaTableSerializer::begin(self.0, len)?)
    }

    // TODO:
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as _)
    }

    // TODO:
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v as _)
    }

    // fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    // where
    //     T: core::fmt::Display,
    // {
    //     Err(core::fmt::Error)
    // }
}

impl<'de> Deserializer<'de> for &'de ValRef<'_> {
    type Error = DesErr;

    /// Require the `Deserializer` to figure out how to drive the visitor based
    /// on what data type is in the input.
    ///
    /// When implementing `Deserialize`, you should avoid relying on
    /// `Deserializer::deserialize_any` unless you need to be told by the
    /// Deserializer what type is in the input. Know that relying on
    /// `Deserializer::deserialize_any` means your data type will be able to
    /// deserialize from self-describing formats only, ruling out Bincode and
    /// many others.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.type_of() {
            Type::Number => {
                if self.is_integer() {
                    visitor.visit_i64(self.to_integer())
                } else {
                    visitor.visit_f64(self.to_number())
                }
            }
            Type::String => self.deserialize_str(visitor),
            Type::Boolean => self.deserialize_bool(visitor),
            _ => {
                if let Some(t) = self.as_table() {
                    let is_array = t
                        .metatable()
                        .map_err(DeErr::custom)?
                        .filter(|mt| {
                            self.state
                                .array_metatable()
                                .map(|a| a.raw_equal(mt))
                                .unwrap_or_default()
                        })
                        .is_some();

                    if is_array || t.raw_len() > 0 {
                        self.deserialize_seq(visitor)
                    } else {
                        self.deserialize_map(visitor)
                    }
                } else {
                    visitor.visit_unit()
                }
            }
        }
    }

    /// Hint that the `Deserialize` type is expecting a `bool` value.
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // TODO: check strict boolean type
        visitor.visit_bool(self.cast().map_err(|_| DesErr::ExpectedBoolean)?)
    }

    /// Hint that the `Deserialize` type is expecting an `i8` value.
    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // TODO: check strict integer type
        visitor.visit_i8(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting an `i16` value.
    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting an `i32` value.
    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting an `i64` value.
    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting a `u8` value.
    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting a `u16` value.
    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting a `u32` value.
    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting a `u64` value.
    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting a `f32` value.
    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting a `f64` value.
    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(self.cast().map_err(|_| DesErr::ExpectedInteger)?)
    }

    /// Hint that the `Deserialize` type is expecting a `char` value.
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.type_of() {
            Type::Number => visitor.visit_char(self.cast::<u8>().unwrap() as _),
            Type::String => visitor.visit_char(
                self.to_str()
                    .map(|s| s.chars())
                    .into_iter()
                    .flatten()
                    .next()
                    .ok_or(DesErr::Message("empty char".into()))?,
            ),
            _ => Err(DesErr::Message("invalid char type".into())),
        }
    }

    /// Hint that the `Deserialize` type is expecting a string value and does
    /// not benefit from taking ownership of buffered data owned by the
    /// `Deserializer`.
    ///
    /// If the `Visitor` would benefit from taking ownership of `String` data,
    /// indiciate this to the `Deserializer` by using `deserialize_string`
    /// instead.
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.to_str().ok_or(DesErr::ExpectedString)?)
    }

    /// Hint that the `Deserialize` type is expecting a string value and would
    /// benefit from taking ownership of buffered data owned by the
    /// `Deserializer`.
    ///
    /// If the `Visitor` would not benefit from taking ownership of `String`
    /// data, indicate that to the `Deserializer` by using `deserialize_str`
    /// instead.
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// Hint that the `Deserialize` type is expecting a byte array and does not
    /// benefit from taking ownership of buffered data owned by the
    /// `Deserializer`.
    ///
    /// If the `Visitor` would benefit from taking ownership of `Vec<u8>` data,
    /// indicate this to the `Deserializer` by using `deserialize_byte_buf`
    /// instead.
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.to_bytes().ok_or(DesErr::ExpectedString)?)
    }

    /// Hint that the `Deserialize` type is expecting a byte array and would
    /// benefit from taking ownership of buffered data owned by the
    /// `Deserializer`.
    ///
    /// If the `Visitor` would not benefit from taking ownership of `Vec<u8>`
    /// data, indicate that to the `Deserializer` by using `deserialize_bytes`
    /// instead.
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    /// Hint that the `Deserialize` type is expecting an optional value.
    ///
    /// This allows deserializers that encode an optional value as a nullable
    /// value to convert the null value into `None` and a regular value into
    /// `Some(value)`.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.type_of().is_none_or_nil() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    /// Hint that the `Deserialize` type is expecting a unit value.
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.type_of().is_none_or_nil() {
            visitor.visit_unit()
        } else {
            Err(DesErr::ExpectedNull)
        }
    }

    /// Hint that the `Deserialize` type is expecting a unit struct with a
    /// particular name.
    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    /// Hint that the `Deserialize` type is expecting a newtype struct with a
    /// particular name.
    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    /// Hint that the `Deserialize` type is expecting a sequence of values.
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        struct SeqDes<'a, 'b>(&'a LuaTable<'b>, usize, usize);

        impl<'de> SeqAccess<'de> for SeqDes<'de, '_> {
            type Error = DesErr;

            #[inline]
            fn size_hint(&self) -> Option<usize> {
                Some(self.2)
            }

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
            where
                T: DeserializeSeed<'de>,
            {
                if self.1 > self.2 {
                    return Ok(None);
                }
                let val = self
                    .0
                    .raw_geti(self.1 as lua_Integer)
                    .map_err(DesErr::custom)?;
                self.1 += 1;
                // Safety: val is also referenced by the self table
                let val: &'de ValRef = unsafe { core::mem::transmute(&val) };
                let r = seed.deserialize(val)?;
                Ok(Some(r))
            }
        }

        if let Some(t) = self.as_table() {
            let len = t.raw_len();
            visitor.visit_seq(SeqDes(t, 1, len))
        } else {
            Err(DesErr::ExpectedArray)
        }
    }

    /// Hint that the `Deserialize` type is expecting a sequence of values and
    /// knows how many values there are without looking at the serialized data.
    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    /// Hint that the `Deserialize` type is expecting a tuple struct with a
    /// particular name and number of fields.
    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    /// Hint that the `Deserialize` type is expecting a map of key-value pairs.
    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        struct ValIter<'a, I: Iterator<Item = (ValRef<'a>, ValRef<'a>)> + 'a>(
            I,
            Option<ValRef<'a>>,
        );

        impl<'a, I: Iterator<Item = (ValRef<'a>, ValRef<'a>)> + 'a> MapAccess<'a> for ValIter<'a, I> {
            type Error = DesErr;

            fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
            where
                T: DeserializeSeed<'a>,
            {
                self.0
                    .next()
                    .map(|(k, v)| {
                        self.1.replace(v);
                        // Safety: k is also referenced by the parent table
                        let val: &'a ValRef = unsafe { core::mem::transmute(&k) };
                        seed.deserialize(val)
                    })
                    .transpose()
            }

            fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
            where
                V: DeserializeSeed<'a>,
            {
                let val = self.1.take().ok_or(DesErr::Eof)?;
                // Safety: val is also referenced by the parent table
                let val: &'a ValRef = unsafe { core::mem::transmute(&val) };
                seed.deserialize(val)
            }
        }

        // crash if index is not a table
        if let Some(t) = self.as_table() {
            visitor.visit_map(ValIter(t.iter().map_err(DeErr::custom)?, None))
        } else {
            Err(DesErr::ExpectedMap)
        }
    }

    /// Hint that the `Deserialize` type is expecting a struct with a particular
    /// name and fields.
    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    /// Hint that the `Deserialize` type is expecting an enum value with a
    /// particular name and possible variants.
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        struct EnumDeser<'a> {
            variant: String,
            value: Option<ValRef<'a>>,
        }

        impl<'lua> EnumAccess<'lua> for EnumDeser<'lua> {
            type Error = DesErr;
            type Variant = VariantDeser<'lua>;

            fn variant_seed<T>(self, seed: T) -> Result<(T::Value, Self::Variant), Self::Error>
            where
                T: DeserializeSeed<'lua>,
            {
                let variant = self.variant.into_deserializer();
                let variant_access = VariantDeser { value: self.value };
                seed.deserialize(variant).map(|v| (v, variant_access))
            }
        }

        struct VariantDeser<'a> {
            value: Option<ValRef<'a>>,
        }

        impl<'de> VariantAccess<'de> for VariantDeser<'de> {
            type Error = DesErr;

            fn unit_variant(self) -> Result<(), Self::Error> {
                // unit_variant can be any type
                // match self.value {
                //     Some(_) => Err(DesErr::custom(format!("unit_variant"))),
                //     None => Ok(()),
                // }
                Ok(())
            }

            fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
            where
                T: DeserializeSeed<'de>,
            {
                match self.value {
                    Some(value) => {
                        // Safety: val is also referenced by the self table
                        let val: &'de ValRef = unsafe { core::mem::transmute(&value) };
                        seed.deserialize(val)
                    }
                    None => Err(DesErr::custom(format!("newtype variant"))),
                }
            }

            fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                match self.value {
                    Some(value) => {
                        // Safety: val is also referenced by the self table
                        let val: &'de ValRef = unsafe { core::mem::transmute(&value) };
                        serde::Deserializer::deserialize_seq(val, visitor)
                    }
                    None => Err(DesErr::custom(format!("tuple variant"))),
                }
            }

            fn struct_variant<V>(
                self,
                _fields: &'static [&'static str],
                visitor: V,
            ) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                match self.value {
                    Some(value) => {
                        // Safety: val is also referenced by the self table
                        let val: &'de ValRef = unsafe { core::mem::transmute(&value) };
                        serde::Deserializer::deserialize_map(val, visitor)
                    }
                    None => Err(DesErr::custom(format!("struct variant"))),
                }
            }
        }

        visitor.visit_enum(if let Some(s) = self.to_str() {
            EnumDeser {
                variant: s.into(),
                value: None,
            }
        } else if let Some((k, v)) = self
            .as_table()
            .and_then(|t| t.iter().ok()?.find(|(k, _)| k.type_of() == Type::String))
            .and_then(|(k, v)| String::from_lua(k.state(), k).ok().map(|k| (k, v)))
        {
            EnumDeser {
                variant: k,
                value: Some(v),
            }
        } else {
            return Err(DesErr::ExpectedEnum);
        })
    }

    /// Hint that the `Deserialize` type is expecting the name of a struct
    /// field or the discriminant of an enum variant.
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// Hint that the `Deserialize` type needs to deserialize a value whose type
    /// doesn't matter because it is ignored.
    ///
    /// Deserializers for non-self-describing formats may not support this mode.
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

impl Serialize for ValRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use core::cell::RefCell;
        use core::ffi::c_void;
        use std::collections::HashSet;

        std::thread_local! {
            static VISITED: RefCell<HashSet<*const c_void>> = RefCell::new(HashSet::default());
        }

        match self.type_of() {
            Type::String => {
                let bytes = self.to_bytes().unwrap_or_default();
                // TODO: serde option
                // if bytes.len() > 0x1000 {
                //     serializer.serialize_bytes(bytes)
                // } else {
                match core::str::from_utf8(bytes) {
                    Ok(s) => serializer.serialize_str(s),
                    Err(_) => serializer.serialize_bytes(bytes),
                }
                // }
            }
            Type::Number => {
                if self.is_integer() {
                    serializer.serialize_i64(self.to_integer())
                } else {
                    serializer.serialize_f64(self.to_number())
                }
            }
            // TODO: serde option
            Type::Function => serializer.serialize_bool(true),
            Type::Boolean => serializer.serialize_bool(self.to_bool()),
            Type::LightUserdata => {
                if self.to_pointer() == State::null_value as *const c_void {
                    serializer.serialize_none()
                } else {
                    serializer.serialize_none()
                }
            }
            _ => {
                if let Some(t) = self.as_table() {
                    let ptr = t.to_pointer();
                    let result = VISITED.with(|visited| {
                        {
                            let mut visited = visited.borrow_mut();
                            if visited.contains(&ptr) {
                                return Err(Error::custom("recursive table detected"));
                            }
                            visited.insert(ptr);
                        }

                        let len = t.raw_len() as usize;
                        let is_array = t
                            .metatable()
                            .map_err(Error::custom)?
                            .filter(|mt| {
                                self.state
                                    .array_metatable()
                                    .map(|a| a.raw_equal(mt))
                                    .unwrap_or_default()
                            })
                            .is_some();

                        t.state.check_stack(3).map_err(Error::custom)?;

                        if is_array || len > 0 {
                            let mut seq = serializer.serialize_seq(Some(len))?;
                            for i in 1..=len {
                                seq.serialize_element(
                                    &t.raw_geti(i as lua_Integer).map_err(Error::custom)?,
                                )?;
                            }
                            seq.end()
                        } else {
                            let mut map = serializer.serialize_map(None)?;
                            for (k, v) in t.iter().map_err(Error::custom)? {
                                map.serialize_entry(&k, &v)?;
                            }
                            map.end()
                        }
                    });
                    VISITED.with(|v| v.borrow_mut().remove(&ptr));
                    result
                } else {
                    serializer.serialize_none()
                }
            }
        }
    }
}

impl Error for LuaError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::Runtime(msg.to_string())
    }
}

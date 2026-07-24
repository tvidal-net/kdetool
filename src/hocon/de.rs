use serde::de::{
    self, DeserializeOwned, Deserializer, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};

use super::error::Error;
use super::parser;
use super::value::Value;

/// Parses `input` as HOCON and deserializes it into `T`.
pub fn from_str<T: DeserializeOwned>(input: &str) -> Result<T, Error> {
    let value = parser::parse(input)?;
    T::deserialize(value)
}

impl Value {
    fn into_i64(self) -> Result<i64, Error> {
        match self {
            Value::Integer(i) => Ok(i),
            Value::Float(f) if f.fract() == 0.0 => Ok(f as i64),
            Value::String(s) => s
                .parse::<i64>()
                .map_err(|_| Error::msg(format!("invalid integer: {s:?}"))),
            other => Err(Error::msg(format!("expected an integer, found {}", other.kind()))),
        }
    }

    fn into_f64(self) -> Result<f64, Error> {
        match self {
            Value::Integer(i) => Ok(i as f64),
            Value::Float(f) => Ok(f),
            Value::String(s) => s
                .parse::<f64>()
                .map_err(|_| Error::msg(format!("invalid float: {s:?}"))),
            other => Err(Error::msg(format!("expected a float, found {}", other.kind()))),
        }
    }
}

// Integer/float deserialize methods all funnel through into_i64 / into_f64 and
// then cast to the requested width; configuration values are small enough that a
// plain cast is fine.
macro_rules! deserialize_number {
    ($method:ident, $visit:ident, $conv:ident, $ty:ty) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            visitor.$visit(self.$conv()? as $ty)
        }
    };
}

impl<'de> IntoDeserializer<'de, Error> for Value {
    type Deserializer = Value;
    fn into_deserializer(self) -> Value {
        self
    }
}

impl<'de> Deserializer<'de> for Value {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(b),
            Value::Integer(i) => visitor.visit_i64(i),
            Value::Float(f) => visitor.visit_f64(f),
            Value::String(s) => visitor.visit_string(s),
            Value::Array(items) => visitor.visit_seq(SeqReader::new(items)),
            Value::Object(entries) => visitor.visit_map(MapReader::new(entries)),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Bool(b) => visitor.visit_bool(b),
            other => Err(Error::msg(format!("expected a boolean, found {}", other.kind()))),
        }
    }

    deserialize_number!(deserialize_i8, visit_i8, into_i64, i8);
    deserialize_number!(deserialize_i16, visit_i16, into_i64, i16);
    deserialize_number!(deserialize_i32, visit_i32, into_i64, i32);
    deserialize_number!(deserialize_i64, visit_i64, into_i64, i64);
    deserialize_number!(deserialize_u8, visit_u8, into_i64, u8);
    deserialize_number!(deserialize_u16, visit_u16, into_i64, u16);
    deserialize_number!(deserialize_u32, visit_u32, into_i64, u32);
    deserialize_number!(deserialize_u64, visit_u64, into_i64, u64);
    deserialize_number!(deserialize_f32, visit_f32, into_f64, f32);
    deserialize_number!(deserialize_f64, visit_f64, into_f64, f64);

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::String(s) => visitor.visit_string(s),
            other => Err(Error::msg(format!("expected a string, found {}", other.kind()))),
        }
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::String(s) if s.chars().count() == 1 => visitor.visit_char(s.chars().next().unwrap()),
            other => Err(Error::msg(format!("expected a single character, found {}", other.kind()))),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Null => visitor.visit_none(),
            other => visitor.visit_some(other),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Null => visitor.visit_unit(),
            other => Err(Error::msg(format!("expected null, found {}", other.kind()))),
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Array(items) => visitor.visit_seq(SeqReader::new(items)),
            other => Err(Error::msg(format!("expected an array, found {}", other.kind()))),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Object(entries) => visitor.visit_map(MapReader::new(entries)),
            other => Err(Error::msg(format!("expected an object, found {}", other.kind()))),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self {
            // A bare string names a unit variant: `mode = all`.
            Value::String(variant) => visitor.visit_enum(EnumReader {
                variant,
                value: None,
            }),
            // A single-key object carries the variant's payload: `{ move = 2 }`.
            Value::Object(mut entries) if entries.len() == 1 => {
                let (variant, value) = entries.pop().unwrap();
                visitor.visit_enum(EnumReader {
                    variant,
                    value: Some(value),
                })
            }
            other => Err(Error::msg(format!("expected an enum, found {}", other.kind()))),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    serde::forward_to_deserialize_any! {
        bytes byte_buf
    }
}

struct SeqReader {
    items: std::vec::IntoIter<Value>,
}

impl SeqReader {
    fn new(items: Vec<Value>) -> Self {
        SeqReader {
            items: items.into_iter(),
        }
    }
}

impl<'de> SeqAccess<'de> for SeqReader {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.items.next() {
            Some(value) => seed.deserialize(value).map(Some),
            None => Ok(None),
        }
    }
}

struct MapReader {
    entries: std::vec::IntoIter<(String, Value)>,
    pending: Option<Value>,
}

impl MapReader {
    fn new(entries: Vec<(String, Value)>) -> Self {
        MapReader {
            entries: entries.into_iter(),
            pending: None,
        }
    }
}

impl<'de> MapAccess<'de> for MapReader {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.entries.next() {
            Some((key, value)) => {
                self.pending = Some(value);
                seed.deserialize(Value::String(key)).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let value = self
            .pending
            .take()
            .ok_or_else(|| Error::msg("value requested before key"))?;
        seed.deserialize(value)
    }
}

struct EnumReader {
    variant: String,
    value: Option<Value>,
}

impl<'de> EnumAccess<'de> for EnumReader {
    type Error = Error;
    type Variant = EnumReader;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(Value::String(self.variant.clone()).into_deserializer())?;
        Ok((variant, self))
    }
}

impl<'de> VariantAccess<'de> for EnumReader {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.value {
            None => Ok(()),
            Some(_) => Err(Error::msg("unexpected payload on unit variant")),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.value {
            Some(value) => seed.deserialize(value),
            None => Err(Error::msg("missing payload for newtype variant")),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(value) => value.deserialize_seq(visitor),
            None => Err(Error::msg("missing payload for tuple variant")),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(value) => value.deserialize_map(visitor),
            None => Err(Error::msg("missing payload for struct variant")),
        }
    }
}

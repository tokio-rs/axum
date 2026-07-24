use super::{ErrorKind, PathDeserializationError};
use crate::util::PercentDecodedStr;
use serde_core::{
    de::{self, DeserializeSeed, EnumAccess, Error, MapAccess, SeqAccess, VariantAccess, Visitor},
    forward_to_deserialize_any, Deserializer,
};
use std::{any::type_name, str::Split, sync::Arc};

macro_rules! unsupported_type {
    ($trait_fn:ident) => {
        fn $trait_fn<V>(self, _: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            Err(PathDeserializationError::unsupported_type(type_name::<
                V::Value,
            >()))
        }
    };
}

macro_rules! parse_single_value {
    ($trait_fn:ident, $visit_fn:ident, $ty:literal) => {
        fn $trait_fn<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            if self.url_params.len() != 1 {
                return Err(PathDeserializationError::wrong_number_of_parameters()
                    .got(self.url_params.len())
                    .expected(1));
            }

            let value = self.url_params[0].1.parse().map_err(|_| {
                PathDeserializationError::new(ErrorKind::ParseError {
                    value: self.url_params[0].1.as_str().to_owned(),
                    expected_type: $ty,
                })
            })?;
            visitor.$visit_fn(value)
        }
    };
}

pub(crate) struct PathDeserializer<'de> {
    url_params: &'de [(Arc<str>, PercentDecodedStr)],
}

impl<'de> PathDeserializer<'de> {
    #[inline]
    pub(crate) fn new(url_params: &'de [(Arc<str>, PercentDecodedStr)]) -> Self {
        PathDeserializer { url_params }
    }
}

impl<'de> Deserializer<'de> for PathDeserializer<'de> {
    type Error = PathDeserializationError;

    unsupported_type!(deserialize_bytes);
    unsupported_type!(deserialize_option);
    unsupported_type!(deserialize_identifier);
    unsupported_type!(deserialize_ignored_any);

    parse_single_value!(deserialize_bool, visit_bool, "bool");
    parse_single_value!(deserialize_i8, visit_i8, "i8");
    parse_single_value!(deserialize_i16, visit_i16, "i16");
    parse_single_value!(deserialize_i32, visit_i32, "i32");
    parse_single_value!(deserialize_i64, visit_i64, "i64");
    parse_single_value!(deserialize_i128, visit_i128, "i128");
    parse_single_value!(deserialize_u8, visit_u8, "u8");
    parse_single_value!(deserialize_u16, visit_u16, "u16");
    parse_single_value!(deserialize_u32, visit_u32, "u32");
    parse_single_value!(deserialize_u64, visit_u64, "u64");
    parse_single_value!(deserialize_u128, visit_u128, "u128");
    parse_single_value!(deserialize_f32, visit_f32, "f32");
    parse_single_value!(deserialize_f64, visit_f64, "f64");
    parse_single_value!(deserialize_string, visit_string, "String");
    parse_single_value!(deserialize_byte_buf, visit_string, "String");
    parse_single_value!(deserialize_char, visit_char, "char");

    fn deserialize_any<V>(self, v: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(v)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.url_params.len() != 1 {
            return Err(PathDeserializationError::wrong_number_of_parameters()
                .got(self.url_params.len())
                .expected(1));
        }
        let key = &self.url_params[0].0;
        let value = &self.url_params[0].1;
        visitor
            .visit_borrowed_str(value)
            .map_err(|e: PathDeserializationError| {
                if let ErrorKind::Message(message) = &e.kind {
                    PathDeserializationError::new(ErrorKind::DeserializeError {
                        key: key.to_string(),
                        value: value.as_str().to_owned(),
                        message: message.to_owned(),
                    })
                } else {
                    e
                }
            })
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // A single capture, e.g. a wildcard, can also be deserialized into a
        // sequence by splitting it on `/`. Since serde only reveals the
        // element type once the first element is deserialized, the choice of
        // interpretation is made there: a `(key, value)` tuple element keeps
        // deserializing the parameter list like it does with multiple
        // parameters, any other element type deserializes the split segments.
        if let [(key, value)] = self.url_params {
            let mut segments = value.split('/');
            if let Some(first_segment) = segments.by_ref().find(|segment| !segment.is_empty()) {
                return visitor.visit_seq(SingleParamSeqAccess {
                    key: key.as_ref(),
                    value: value.as_str(),
                    first_segment,
                    rest: segments,
                    mode: SingleParamMode::Undecided,
                });
            }
        }

        visitor.visit_seq(SeqDeserializer {
            params: self.url_params,
            idx: 0,
            allow_seq: false,
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.url_params.len() != len {
            return Err(PathDeserializationError::wrong_number_of_parameters()
                .got(self.url_params.len())
                .expected(len));
        }
        visitor.visit_seq(SeqDeserializer {
            params: self.url_params,
            idx: 0,
            allow_seq: true,
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.url_params.len() != len {
            return Err(PathDeserializationError::wrong_number_of_parameters()
                .got(self.url_params.len())
                .expected(len));
        }
        visitor.visit_seq(SeqDeserializer {
            params: self.url_params,
            idx: 0,
            allow_seq: true,
        })
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(MapDeserializer {
            params: self.url_params,
            value: None,
            key: None,
        })
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.url_params.len() != 1 {
            return Err(PathDeserializationError::wrong_number_of_parameters()
                .got(self.url_params.len())
                .expected(1));
        }

        visitor.visit_enum(EnumDeserializer {
            value: &self.url_params[0].1,
        })
    }
}

struct MapDeserializer<'de> {
    params: &'de [(Arc<str>, PercentDecodedStr)],
    key: Option<KeyOrIdx<'de>>,
    value: Option<&'de str>,
}

impl<'de> MapAccess<'de> for MapDeserializer<'de> {
    type Error = PathDeserializationError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.params.split_first() {
            Some(((key, value), tail)) => {
                self.value = Some(value.as_str());
                self.params = tail;
                self.key = Some(KeyOrIdx::Key(key));
                seed.deserialize(KeyDeserializer { key }).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => seed.deserialize(ValueDeserializer {
                key: self.key.take(),
                value,
                allow_seq: true,
            }),
            None => Err(PathDeserializationError::custom("value is missing")),
        }
    }
}

struct KeyDeserializer<'de> {
    key: &'de str,
}

macro_rules! parse_key {
    ($trait_fn:ident) => {
        fn $trait_fn<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_str(&self.key)
        }
    };
}

impl<'de> Deserializer<'de> for KeyDeserializer<'de> {
    type Error = PathDeserializationError;

    parse_key!(deserialize_identifier);
    parse_key!(deserialize_str);
    parse_key!(deserialize_string);

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(PathDeserializationError::custom("Unexpected key type"))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char bytes
        byte_buf option unit unit_struct seq tuple
        tuple_struct map newtype_struct struct enum ignored_any
    }
}

macro_rules! parse_value {
    ($trait_fn:ident, $visit_fn:ident, $ty:literal) => {
        fn $trait_fn<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let v = self.value.parse().map_err(|_| {
                if let Some(key) = self.key.take() {
                    let kind = match key {
                        KeyOrIdx::Key(key) => ErrorKind::ParseErrorAtKey {
                            key: key.to_owned(),
                            value: self.value.to_owned(),
                            expected_type: $ty,
                        },
                        KeyOrIdx::Idx { idx: index, key: _ } => ErrorKind::ParseErrorAtIndex {
                            index,
                            value: self.value.to_owned(),
                            expected_type: $ty,
                        },
                    };
                    PathDeserializationError::new(kind)
                } else {
                    PathDeserializationError::new(ErrorKind::ParseError {
                        value: self.value.to_owned(),
                        expected_type: $ty,
                    })
                }
            })?;
            visitor.$visit_fn(v)
        }
    };
}

#[derive(Debug)]
struct ValueDeserializer<'de> {
    key: Option<KeyOrIdx<'de>>,
    value: &'de str,
    /// Whether the value may be split on `/` and deserialized into a
    /// sequence. This only applies one level deep: a split segment cannot be
    /// split again, keeping nested sequences unsupported.
    allow_seq: bool,
}

impl<'de> Deserializer<'de> for ValueDeserializer<'de> {
    type Error = PathDeserializationError;

    unsupported_type!(deserialize_map);
    unsupported_type!(deserialize_identifier);

    parse_value!(deserialize_bool, visit_bool, "bool");
    parse_value!(deserialize_i8, visit_i8, "i8");
    parse_value!(deserialize_i16, visit_i16, "i16");
    parse_value!(deserialize_i32, visit_i32, "i32");
    parse_value!(deserialize_i64, visit_i64, "i64");
    parse_value!(deserialize_i128, visit_i128, "i128");
    parse_value!(deserialize_u8, visit_u8, "u8");
    parse_value!(deserialize_u16, visit_u16, "u16");
    parse_value!(deserialize_u32, visit_u32, "u32");
    parse_value!(deserialize_u64, visit_u64, "u64");
    parse_value!(deserialize_u128, visit_u128, "u128");
    parse_value!(deserialize_f32, visit_f32, "f32");
    parse_value!(deserialize_f64, visit_f64, "f64");
    parse_value!(deserialize_string, visit_string, "String");
    parse_value!(deserialize_byte_buf, visit_string, "String");
    parse_value!(deserialize_char, visit_char, "char");

    fn deserialize_any<V>(self, v: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(v)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor
            .visit_borrowed_str(self.value)
            .map_err(|e: PathDeserializationError| {
                if let (ErrorKind::Message(message), Some(key)) = (&e.kind, self.key.as_ref()) {
                    PathDeserializationError::new(ErrorKind::DeserializeError {
                        key: key.key().to_owned(),
                        value: self.value.to_owned(),
                        message: message.to_owned(),
                    })
                } else {
                    e
                }
            })
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.value.as_bytes())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        struct PairDeserializer<'de> {
            key: Option<KeyOrIdx<'de>>,
            value: Option<&'de str>,
        }

        impl<'de> SeqAccess<'de> for PairDeserializer<'de> {
            type Error = PathDeserializationError;

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
            where
                T: DeserializeSeed<'de>,
            {
                match self.key.take() {
                    Some(KeyOrIdx::Idx { idx: _, key }) => {
                        return seed.deserialize(KeyDeserializer { key }).map(Some);
                    }
                    Some(KeyOrIdx::Key(_)) => {
                        return Err(PathDeserializationError::custom(
                            "array types are not supported",
                        ));
                    }
                    None => {}
                };

                self.value
                    .take()
                    .map(|value| {
                        seed.deserialize(ValueDeserializer {
                            key: None,
                            value,
                            allow_seq: false,
                        })
                    })
                    .transpose()
            }
        }

        if len == 2 {
            match self.key {
                Some(key) => visitor.visit_seq(PairDeserializer {
                    key: Some(key),
                    value: Some(self.value),
                }),
                // `self.key` is only `None` when deserializing maps so `deserialize_seq`
                // wouldn't be called for that
                None => unreachable!(),
            }
        } else {
            Err(PathDeserializationError::unsupported_type(type_name::<
                V::Value,
            >()))
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if !self.allow_seq {
            return Err(PathDeserializationError::unsupported_type(type_name::<
                V::Value,
            >()));
        }

        visitor.visit_seq(SplitValueSeqAccess {
            key: self.key.as_ref().map(|key| KeyOrIdx::Key(key.key())),
            segments: self.value.split('/'),
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(PathDeserializationError::unsupported_type(type_name::<
            V::Value,
        >()))
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(PathDeserializationError::unsupported_type(type_name::<
            V::Value,
        >()))
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(EnumDeserializer { value: self.value })
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

struct EnumDeserializer<'de> {
    value: &'de str,
}

impl<'de> EnumAccess<'de> for EnumDeserializer<'de> {
    type Error = PathDeserializationError;
    type Variant = UnitVariant;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        Ok((
            seed.deserialize(KeyDeserializer { key: self.value })?,
            UnitVariant,
        ))
    }
}

struct UnitVariant;

impl<'de> VariantAccess<'de> for UnitVariant {
    type Error = PathDeserializationError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        Err(PathDeserializationError::unsupported_type(
            "newtype enum variant",
        ))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(PathDeserializationError::unsupported_type(
            "tuple enum variant",
        ))
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(PathDeserializationError::unsupported_type(
            "struct enum variant",
        ))
    }
}

struct SeqDeserializer<'de> {
    params: &'de [(Arc<str>, PercentDecodedStr)],
    idx: usize,
    /// `allow_seq` for the element deserializers: a tuple position may split
    /// its value into a sequence, an element of a sequence may not, keeping
    /// nested sequences unsupported.
    allow_seq: bool,
}

impl<'de> SeqAccess<'de> for SeqDeserializer<'de> {
    type Error = PathDeserializationError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.params.split_first() {
            Some(((key, value), tail)) => {
                self.params = tail;
                let idx = self.idx;
                self.idx += 1;
                Ok(Some(seed.deserialize(ValueDeserializer {
                    key: Some(KeyOrIdx::Idx { idx, key }),
                    value: value.as_str(),
                    allow_seq: self.allow_seq,
                })?))
            }
            None => Ok(None),
        }
    }
}

/// [`SeqAccess`] deserializing a single value, e.g. a wildcard capture, into
/// a sequence by splitting it on `/`.
///
/// Empty segments are skipped, so captures with leading, trailing, or
/// consecutive slashes don't produce empty elements.
struct SplitValueSeqAccess<'de> {
    key: Option<KeyOrIdx<'de>>,
    segments: Split<'de, char>,
}

impl<'de> SeqAccess<'de> for SplitValueSeqAccess<'de> {
    type Error = PathDeserializationError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        for segment in self.segments.by_ref() {
            if !segment.is_empty() {
                return seed
                    .deserialize(ValueDeserializer {
                        key: self.key.clone(),
                        value: segment,
                        allow_seq: false,
                    })
                    .map(Some);
            }
        }

        Ok(None)
    }
}

enum SingleParamMode {
    Undecided,
    Split,
    Done,
}

/// [`SeqAccess`] used when deserializing a sequence from exactly one
/// parameter.
///
/// A sequence of `(key, value)` pairs deserializes the parameter list, like
/// it does when there are multiple parameters, while any other element type
/// deserializes the parameter's value split on `/`, which is how wildcard
/// captures are deserialized into `Vec<T>`. Since serde only reveals the
/// element type once the first element is deserialized, the choice between
/// the two is made by [`FirstElementDeserializer`].
struct SingleParamSeqAccess<'de> {
    key: &'de str,
    value: &'de str,
    first_segment: &'de str,
    rest: Split<'de, char>,
    mode: SingleParamMode,
}

impl<'de> SeqAccess<'de> for SingleParamSeqAccess<'de> {
    type Error = PathDeserializationError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.mode {
            SingleParamMode::Undecided => seed
                .deserialize(FirstElementDeserializer { access: self })
                .map(Some),
            SingleParamMode::Split => self
                .rest
                .by_ref()
                .find(|segment| !segment.is_empty())
                .map(|segment| {
                    seed.deserialize(ValueDeserializer {
                        key: Some(KeyOrIdx::Key(self.key)),
                        value: segment,
                        allow_seq: false,
                    })
                })
                .transpose(),
            SingleParamMode::Done => Ok(None),
        }
    }
}

macro_rules! forward_to_split_value {
    ($($trait_fn:ident)*) => {
        $(
            fn $trait_fn<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                self.split_value().$trait_fn(visitor)
            }
        )*
    };
}

/// Deserializer for the first element of a [`SingleParamSeqAccess`], which
/// commits the sequence to one of the two interpretations based on the
/// element type.
struct FirstElementDeserializer<'a, 'de> {
    access: &'a mut SingleParamSeqAccess<'de>,
}

impl<'de> FirstElementDeserializer<'_, 'de> {
    /// Commit to deserializing the split segments of the parameter's value.
    fn split_value(self) -> ValueDeserializer<'de> {
        self.access.mode = SingleParamMode::Split;
        ValueDeserializer {
            key: Some(KeyOrIdx::Key(self.access.key)),
            value: self.access.first_segment,
            allow_seq: false,
        }
    }
}

impl<'de> Deserializer<'de> for FirstElementDeserializer<'_, 'de> {
    type Error = PathDeserializationError;

    forward_to_split_value!(
        deserialize_any deserialize_bool
        deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64 deserialize_i128
        deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64 deserialize_u128
        deserialize_f32 deserialize_f64
        deserialize_char deserialize_str deserialize_string
        deserialize_bytes deserialize_byte_buf
        deserialize_unit deserialize_seq deserialize_map
        deserialize_identifier deserialize_ignored_any
    );

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // Commit to deserializing the parameter list into `(key, value)`
        // pairs, preserving how a single parameter deserialized into
        // `Vec<(String, String)>` before sequences of split segments were
        // supported.
        self.access.mode = SingleParamMode::Done;
        ValueDeserializer {
            key: Some(KeyOrIdx::Idx {
                idx: 0,
                key: self.access.key,
            }),
            value: self.access.value,
            allow_seq: false,
        }
        .deserialize_tuple(len, visitor)
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.split_value().deserialize_unit_struct(name, visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // Defer the choice of interpretation to the inner type.
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // Defer the choice of interpretation to the inner type.
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.split_value()
            .deserialize_tuple_struct(name, len, visitor)
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.split_value().deserialize_struct(name, fields, visitor)
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.split_value().deserialize_enum(name, variants, visitor)
    }
}

#[derive(Debug, Clone)]
enum KeyOrIdx<'de> {
    Key(&'de str),
    Idx { idx: usize, key: &'de str },
}

impl<'de> KeyOrIdx<'de> {
    fn key(&self) -> &'de str {
        match &self {
            Self::Idx { key, .. } | Self::Key(key) => key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    enum MyEnum {
        A,
        B,
        #[serde(rename = "c")]
        C,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct Struct {
        c: String,
        b: bool,
        a: i32,
    }

    fn create_url_params<I, K, V>(values: I) -> Vec<(Arc<str>, PercentDecodedStr)>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        values
            .into_iter()
            .map(|(k, v)| (Arc::from(k.as_ref()), PercentDecodedStr::new(v).unwrap()))
            .collect()
    }

    macro_rules! check_single_value {
        ($ty:ty, $value_str:literal, $value:expr) => {
            #[allow(clippy::bool_assert_comparison)]
            {
                let url_params = create_url_params(vec![("value", $value_str)]);
                let deserializer = PathDeserializer::new(&url_params);
                assert_eq!(<$ty>::deserialize(deserializer).unwrap(), $value);
            }
        };
    }

    #[test]
    fn test_parse_single_value() {
        check_single_value!(bool, "true", true);
        check_single_value!(bool, "false", false);
        check_single_value!(i8, "-123", -123);
        check_single_value!(i16, "-123", -123);
        check_single_value!(i32, "-123", -123);
        check_single_value!(i64, "-123", -123);
        check_single_value!(i128, "123", 123);
        check_single_value!(u8, "123", 123);
        check_single_value!(u16, "123", 123);
        check_single_value!(u32, "123", 123);
        check_single_value!(u64, "123", 123);
        check_single_value!(u128, "123", 123);
        check_single_value!(f32, "123", 123.0);
        check_single_value!(f64, "123", 123.0);
        check_single_value!(String, "abc", "abc");
        check_single_value!(String, "one%20two", "one two");
        check_single_value!(&str, "abc", "abc");
        check_single_value!(&str, "one%20two", "one two");
        check_single_value!(char, "a", 'a');

        let url_params = create_url_params(vec![("a", "B")]);
        assert_eq!(
            MyEnum::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            MyEnum::B
        );

        let url_params = create_url_params(vec![("a", "1"), ("b", "2")]);
        let error_kind = i32::deserialize(PathDeserializer::new(&url_params))
            .unwrap_err()
            .kind;
        assert!(matches!(
            error_kind,
            ErrorKind::WrongNumberOfParameters {
                expected: 1,
                got: 2
            }
        ));
    }

    #[test]
    fn test_parse_seq() {
        let url_params = create_url_params(vec![("a", "1"), ("b", "true"), ("c", "abc")]);
        assert_eq!(
            <(i32, bool, String)>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            (1, true, "abc".to_owned())
        );

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct TupleStruct(i32, bool, String);
        assert_eq!(
            TupleStruct::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            TupleStruct(1, true, "abc".to_owned())
        );

        let url_params = create_url_params(vec![("a", "1"), ("b", "2"), ("c", "3")]);
        assert_eq!(
            <Vec<i32>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![1, 2, 3]
        );

        let url_params = create_url_params(vec![("a", "c"), ("a", "B")]);
        assert_eq!(
            <Vec<MyEnum>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![MyEnum::C, MyEnum::B]
        );
    }

    #[test]
    fn test_parse_seq_tuple_string_string() {
        let url_params = create_url_params(vec![("a", "foo"), ("b", "bar")]);
        assert_eq!(
            <Vec<(String, String)>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![
                ("a".to_owned(), "foo".to_owned()),
                ("b".to_owned(), "bar".to_owned())
            ]
        );
    }

    #[test]
    fn test_parse_wildcard_seq() {
        let url_params = create_url_params(vec![("path", "x/y/z")]);
        assert_eq!(
            <Vec<String>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec!["x".to_owned(), "y".to_owned(), "z".to_owned()]
        );

        let url_params = create_url_params(vec![("ids", "1/-2/3")]);
        assert_eq!(
            <Vec<i32>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![1, -2, 3]
        );

        let url_params = create_url_params(vec![("path", "A/B/c")]);
        assert_eq!(
            <Vec<MyEnum>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![MyEnum::A, MyEnum::B, MyEnum::C]
        );

        // a value without any `/` deserializes into a single element
        let url_params = create_url_params(vec![("path", "x")]);
        assert_eq!(
            <Vec<String>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec!["x".to_owned()]
        );
    }

    #[test]
    fn test_parse_wildcard_seq_empty_segments() {
        for (value, expected) in [
            ("x/", vec!["x"]),
            ("/x", vec!["x"]),
            ("x//y", vec!["x", "y"]),
            ("x/y/", vec!["x", "y"]),
        ] {
            let url_params = create_url_params(vec![("path", value)]);
            assert_eq!(
                <Vec<String>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
                expected,
                "for {value:?}"
            );
        }

        // values without any non-empty segment keep deserializing each
        // parameter into one element
        for value in ["", "/", "//"] {
            let url_params = create_url_params(vec![("path", value)]);
            assert_eq!(
                <Vec<String>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
                vec![value.to_owned()],
                "for {value:?}"
            );
        }
    }

    #[test]
    fn test_parse_wildcard_seq_in_struct() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Params {
            bucket: String,
            key: Vec<String>,
        }

        let url_params = create_url_params(vec![("bucket", "my-bucket"), ("key", "a/b/c")]);
        assert_eq!(
            Params::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            Params {
                bucket: "my-bucket".to_owned(),
                key: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            }
        );
    }

    #[test]
    fn test_parse_wildcard_seq_in_tuple() {
        let url_params = create_url_params(vec![("bucket", "my-bucket"), ("key", "a/b")]);
        assert_eq!(
            <(String, Vec<String>)>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            ("my-bucket".to_owned(), vec!["a".to_owned(), "b".to_owned()])
        );

        // empty segments are skipped in tuple position too
        let url_params = create_url_params(vec![("bucket", "my-bucket"), ("key", "a/")]);
        assert_eq!(
            <(String, Vec<String>)>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            ("my-bucket".to_owned(), vec!["a".to_owned()])
        );
    }

    #[test]
    fn test_parse_wildcard_seq_newtype_element() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Seg(String);

        let url_params = create_url_params(vec![("path", "x/y")]);
        assert_eq!(
            <Vec<Seg>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![Seg("x".to_owned()), Seg("y".to_owned())]
        );
    }

    #[test]
    fn test_parse_wildcard_seq_all_empty_segments_in_struct() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Params {
            key: Vec<String>,
        }

        let url_params = create_url_params(vec![("key", "/")]);
        assert_eq!(
            Params::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            Params { key: vec![] }
        );
    }

    #[test]
    fn test_parse_seq_tuple_wrapped_pair_single_param() {
        // the pair interpretation also applies to `Option`- or
        // newtype-wrapped pairs
        let url_params = create_url_params(vec![("a", "foo/bar")]);
        assert_eq!(
            <Vec<Option<(String, String)>>>::deserialize(PathDeserializer::new(&url_params))
                .unwrap(),
            vec![Some(("a".to_owned(), "foo/bar".to_owned()))]
        );

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Pair((String, String));

        let url_params = create_url_params(vec![("a", "foo/bar")]);
        assert_eq!(
            <Vec<Pair>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![Pair(("a".to_owned(), "foo/bar".to_owned()))]
        );
    }

    #[test]
    fn test_parse_seq_tuple_string_string_single_param() {
        // a single parameter still deserializes into `(key, value)` pairs,
        // even if its value contains `/`
        let url_params = create_url_params(vec![("a", "foo/bar")]);
        assert_eq!(
            <Vec<(String, String)>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![("a".to_owned(), "foo/bar".to_owned())]
        );
    }

    #[test]
    fn test_parse_seq_tuple_string_parse() {
        let url_params = create_url_params(vec![("a", "1"), ("b", "2")]);
        assert_eq!(
            <Vec<(String, u32)>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            vec![("a".to_owned(), 1), ("b".to_owned(), 2)]
        );
    }

    #[test]
    fn test_parse_struct() {
        let url_params = create_url_params(vec![("a", "1"), ("b", "true"), ("c", "abc")]);
        assert_eq!(
            Struct::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            Struct {
                c: "abc".to_owned(),
                b: true,
                a: 1,
            }
        );
    }

    #[test]
    fn test_parse_struct_ignoring_additional_fields() {
        let url_params = create_url_params(vec![
            ("a", "1"),
            ("b", "true"),
            ("c", "abc"),
            ("d", "false"),
        ]);
        assert_eq!(
            Struct::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            Struct {
                c: "abc".to_owned(),
                b: true,
                a: 1,
            }
        );
    }

    #[test]
    fn test_parse_map() {
        let url_params = create_url_params(vec![("a", "1"), ("b", "true"), ("c", "abc")]);
        assert_eq!(
            <HashMap<String, String>>::deserialize(PathDeserializer::new(&url_params)).unwrap(),
            [("a", "1"), ("b", "true"), ("c", "abc")]
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect()
        );
    }

    macro_rules! test_parse_error {
        (
            $params:expr,
            $ty:ty,
            $expected_error_kind:expr $(,)?
        ) => {
            let url_params = create_url_params($params);
            let actual_error_kind = <$ty>::deserialize(PathDeserializer::new(&url_params))
                .unwrap_err()
                .kind;
            assert_eq!(actual_error_kind, $expected_error_kind);
        };
    }

    #[test]
    fn test_parse_tuple_too_many_fields() {
        test_parse_error!(
            vec![("a", "abc"), ("b", "true"), ("c", "1"), ("d", "false"),],
            (&str, bool, u32),
            ErrorKind::WrongNumberOfParameters {
                got: 4,
                expected: 3,
            }
        );
    }

    #[test]
    fn test_wrong_number_of_parameters_error() {
        test_parse_error!(
            vec![("a", "1")],
            (u32, u32),
            ErrorKind::WrongNumberOfParameters {
                got: 1,
                expected: 2,
            }
        );
    }

    #[test]
    fn test_parse_error_at_key_error() {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Params {
            a: u32,
        }
        test_parse_error!(
            vec![("a", "false")],
            Params,
            ErrorKind::ParseErrorAtKey {
                key: "a".to_owned(),
                value: "false".to_owned(),
                expected_type: "u32",
            }
        );
    }

    #[test]
    fn test_parse_error_at_key_error_multiple() {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Params {
            a: u32,
            b: u32,
        }
        test_parse_error!(
            vec![("a", "false")],
            Params,
            ErrorKind::ParseErrorAtKey {
                key: "a".to_owned(),
                value: "false".to_owned(),
                expected_type: "u32",
            }
        );
    }

    #[test]
    fn test_parse_error_at_index_error() {
        test_parse_error!(
            vec![("a", "false"), ("b", "true")],
            (bool, u32),
            ErrorKind::ParseErrorAtIndex {
                index: 1,
                value: "true".to_owned(),
                expected_type: "u32",
            }
        );
    }

    #[test]
    fn test_parse_error_error() {
        test_parse_error!(
            vec![("a", "false")],
            u32,
            ErrorKind::ParseError {
                value: "false".to_owned(),
                expected_type: "u32",
            }
        );
    }

    #[test]
    fn test_unsupported_type_error_nested_data_structure() {
        test_parse_error!(
            vec![("a", "false")],
            Vec<Vec<u32>>,
            ErrorKind::UnsupportedType {
                name: "alloc::vec::Vec<u32>",
            }
        );
    }

    #[test]
    fn test_parse_seq_tuple_unsupported_key_type() {
        test_parse_error!(
            vec![("a", "false")],
            Vec<(u32, String)>,
            ErrorKind::Message("Unexpected key type".to_owned())
        );
    }

    #[test]
    fn test_parse_wildcard_seq_parse_error() {
        test_parse_error!(
            vec![("ids", "1/x/3")],
            Vec<i32>,
            ErrorKind::ParseErrorAtKey {
                key: "ids".to_owned(),
                value: "x".to_owned(),
                expected_type: "i32",
            }
        );
    }

    #[test]
    fn test_parse_seq_wrong_tuple_length() {
        test_parse_error!(
            vec![("a", "false")],
            Vec<(String, String, String)>,
            ErrorKind::UnsupportedType {
                name: "(alloc::string::String, alloc::string::String, alloc::string::String)",
            }
        );
    }

    #[test]
    fn test_parse_seq_seq() {
        test_parse_error!(
            vec![("a", "false")],
            Vec<Vec<String>>,
            ErrorKind::UnsupportedType {
                name: "alloc::vec::Vec<alloc::string::String>",
            }
        );

        // nested sequences are also unsupported with multiple parameters
        test_parse_error!(
            vec![("a", "x/y"), ("b", "z")],
            Vec<Vec<String>>,
            ErrorKind::UnsupportedType {
                name: "alloc::vec::Vec<alloc::string::String>",
            }
        );
    }

    #[test]
    fn test_deserialize_key_value() {
        test_parse_error!(
            vec![("id", "123123-123-123123")],
            uuid::Uuid,
            ErrorKind::DeserializeError {
                key: "id".to_owned(),
                value: "123123-123-123123".to_owned(),
                message: "UUID parsing failed: invalid group count: expected 5, found 3".to_owned(),
            }
        );
    }
}

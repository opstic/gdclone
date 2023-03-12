use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::num::{ParseFloatError, ParseIntError};
use std::str::Utf8Error;
use std::{fmt, str};

#[derive(Clone, Debug)]
pub(crate) enum DeError {
    InvalidInt(ParseIntError),
    InvalidFloat(ParseFloatError),
    InvalidUtf8(Utf8Error),
    Custom(String),
}

impl Display for DeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DeError::Custom(s) => write!(f, "{}", s),
            DeError::InvalidInt(e) => write!(f, "Invalid int: {}", e),
            DeError::InvalidFloat(e) => write!(f, "Invalid float: {}", e),
            DeError::InvalidUtf8(e) => write!(f, "Malformed UTF-8: {}", e),
        }
    }
}

impl Error for DeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DeError::InvalidInt(e) => Some(e),
            DeError::InvalidFloat(e) => Some(e),
            _ => None,
        }
    }
}

impl From<Utf8Error> for DeError {
    #[inline]
    fn from(e: Utf8Error) -> Self {
        Self::InvalidUtf8(e)
    }
}

impl From<ParseIntError> for DeError {
    #[inline]
    fn from(e: ParseIntError) -> Self {
        Self::InvalidInt(e)
    }
}

impl From<ParseFloatError> for DeError {
    #[inline]
    fn from(e: ParseFloatError) -> Self {
        Self::InvalidFloat(e)
    }
}

impl From<fmt::Error> for DeError {
    #[inline]
    fn from(e: fmt::Error) -> Self {
        Self::Custom(e.to_string())
    }
}

impl serde::de::Error for DeError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        DeError::Custom(msg.to_string())
    }
}

trait Reader<'a> {
    fn read_until_byte(&mut self, byte: u8) -> Option<&'a [u8]>;

    fn len(&self) -> Option<usize>;
}

struct SliceReader<'a> {
    source: &'a [u8],
}

impl<'a> Reader<'a> for SliceReader<'a> {
    fn read_until_byte(&mut self, byte: u8) -> Option<&'a [u8]> {
        if self.source.is_empty() {
            return None;
        }
        Some(if let Some(i) = memchr::memchr(byte, self.source) {
            let bytes = &self.source[..i];
            self.source = &self.source[i + 1..];
            bytes
        } else {
            let bytes = self.source;
            self.source = &[];
            bytes
        })
    }

    fn len(&self) -> Option<usize> {
        Some(self.source.len())
    }
}

pub(crate) fn from_slice<'de, T>(b: &'de [u8], sep: u8) -> Result<T, DeError>
where
    T: Deserialize<'de>,
{
    let mut de = SeparatorDeserializer::from_slice(b, sep);
    T::deserialize(&mut de)
}

struct SeparatorDeserializer<'de, R>
where
    R: Reader<'de>,
{
    reader: R,
    separator: u8,
    peek: Option<&'de [u8]>,
    initial: bool,
    seen: Vec<&'de [u8]>,
}

impl<'de> SeparatorDeserializer<'de, SliceReader<'de>> {
    /// Create new deserializer that will borrow data from the specified string
    pub fn from_slice(b: &'de [u8], sep: u8) -> Self {
        Self::new(SliceReader { source: b }, sep)
    }
}

impl<'de, R> SeparatorDeserializer<'de, R>
where
    R: Reader<'de>,
{
    fn new(reader: R, sep: u8) -> Self {
        SeparatorDeserializer {
            reader,
            separator: sep,
            peek: None,
            initial: false,
            seen: Vec::new(),
        }
    }

    fn read_string(&mut self) -> Result<Cow<'de, str>, DeError> {
        match self.next() {
            Some(bytes) => Ok(Cow::Borrowed(std::str::from_utf8(bytes)?)),
            None => Ok("".into()),
        }
    }

    fn peek(&mut self) -> Option<&'de [u8]> {
        if self.peek.is_none() {
            self.peek = self.reader.read_until_byte(self.separator);
        }
        self.peek
    }

    fn next(&mut self) -> Option<&'de [u8]> {
        if let Some(b) = self.peek.take() {
            return Some(b);
        }
        self.reader.read_until_byte(self.separator)
    }
}

impl<'de, 'a, R> SeqAccess<'de> for &'a mut SeparatorDeserializer<'de, R>
where
    R: Reader<'de>,
{
    type Error = DeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.peek() {
            Some(_) => seed.deserialize(&mut **self).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        self.reader.len().map(|n| n / 3)
    }
}

impl<'de, 'a, R> MapAccess<'de> for &'a mut SeparatorDeserializer<'de, R>
where
    R: Reader<'de>,
{
    type Error = DeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.peek() {
            Some(a) => {
                if self.seen.contains(&a) {
                    self.next();
                    self.next();
                } else {
                    self.seen.push(a);
                }
                match self.peek() {
                    Some(_) => seed.deserialize(&mut **self).map(Some),
                    None => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut **self)
    }

    fn size_hint(&self) -> Option<usize> {
        self.reader.len().map(|n| n / 3)
    }
}

macro_rules! deserialize_type {
    ($deserialize:ident => $visit:ident) => {
        fn $deserialize<V>(self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            visitor.$visit(self.read_string()?.parse()?)
        }
    };
}

impl<'de, 'a, R> Deserializer<'de> for &'a mut SeparatorDeserializer<'de, R>
where
    R: Reader<'de>,
{
    type Error = DeError;

    deserialize_type!(deserialize_i8 => visit_i8);
    deserialize_type!(deserialize_i16 => visit_i16);
    deserialize_type!(deserialize_i32 => visit_i32);
    deserialize_type!(deserialize_i64 => visit_i64);

    deserialize_type!(deserialize_u8 => visit_u8);
    deserialize_type!(deserialize_u16 => visit_u16);
    deserialize_type!(deserialize_u32 => visit_u32);
    deserialize_type!(deserialize_u64 => visit_u64);

    deserialize_type!(deserialize_f32 => visit_f32);
    deserialize_type!(deserialize_f64 => visit_f64);

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if !self.initial {
            self.initial = true;
            self.deserialize_map(visitor)
        } else {
            self.deserialize_str(visitor)
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(matches!(self.next().unwrap(), b"1"))
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.read_string()? {
            Cow::Borrowed(s) => visitor.visit_borrowed_str(s),
            Cow::Owned(s) => visitor.visit_str(&s),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.next().unwrap())
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.peek() {
            Some(b) => {
                if b.is_empty() {
                    visitor.visit_none()
                } else {
                    visitor.visit_some(self)
                }
            }
            None => visitor.visit_none(),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.next();
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
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
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
        todo!()
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_struct("", &[], visitor)
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
        visitor.visit_map(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }
}

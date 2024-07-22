use std::borrow::Cow;
use std::collections::VecDeque;

use crate::{unpack_rlp, DecodeError, RecursiveBytes, Rlp};
use paste::paste;
use serde::de::SeqAccess;
use serde::{Deserialize, Deserializer};

macro_rules! parse_int {
    ($b:literal) => {
        paste! {
            fn [<parse_i $b>](&mut self) -> Result<[<i $b>], DecodeError> {
                let bytes = self.need_bytes_len::<{$b / 8}>()?;
                Ok([<i $b>]::from_be_bytes(bytes))
            }
        }
    };
}

macro_rules! parse_uint {
    ($b:literal) => {
        paste! {
            fn [<parse_u $b>](&mut self) -> Result<[<u $b>], DecodeError> {
                let bytes = self.need_bytes_len::<{$b / 8}>()?;
                Ok([<u $b>]::from_be_bytes(bytes))
            }
        }
    };
}

#[cfg(test)]
fn from_rlp<'a, T>(rlp: &'a mut Rlp) -> Result<T, DecodeError>
where
    T: Deserialize<'a>,
{
    T::deserialize(rlp)
}

pub fn from_bytes<'a, T>(bytes: &'a [u8]) -> Result<T, DecodeError>
where
    T: Deserialize<'a>,
{
    let rec = dbg!(unpack_rlp(bytes))?;
    let mut rlp = Rlp(rec.into());
    T::deserialize(&mut rlp)
}

impl<'de, 'a> SeqAccess<'de> for Rlp<'a> {
    type Error = DecodeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        let el = self.0.pop_front().expect("TODO");
        let rlp = &mut Rlp(VecDeque::from([el]));
        seed.deserialize(rlp).map(Some)
    }
}

impl<'a> Rlp<'a> {
    fn need_bytes(&self) -> Result<&[u8], DecodeError> {
        let first_rec = self.0.front().ok_or(DecodeError::MissingBytes)?;
        let RecursiveBytes::Bytes(bytes) = first_rec else {
            return Err(DecodeError::ExpectedBytes);
        };
        Ok(bytes)
    }

    fn need_nested(&self) -> Result<&VecDeque<RecursiveBytes>, DecodeError> {
        let first_rec = self.0.front().ok_or(DecodeError::MissingBytes)?;
        let RecursiveBytes::Nested(rec) = first_rec else {
            return Err(DecodeError::ExpectedBytes);
        };
        Ok(rec)
    }

    fn need_bytes_len<const S: usize>(&self) -> Result<[u8; S], DecodeError> {
        let bytes = self.need_bytes()?;
        if bytes.len() != S {
            return Err(DecodeError::InvalidLength);
        }
        Ok(bytes.try_into().unwrap())
    }

    fn parse_bool(&mut self) -> Result<bool, DecodeError> {
        let bytes = dbg!(self.need_bytes_len::<1>())?;
        let byte = bytes[0];
        let bool_val = match byte {
            0 => false,
            1 => true,
            _ => return Err(DecodeError::InvalidBytes),
        };
        self.0.pop_front();
        Ok(bool_val)
    }

    parse_int!(8);
    parse_int!(16);
    parse_int!(32);
    parse_int!(64);

    parse_uint!(8);
    parse_uint!(16);
    parse_uint!(32);
    parse_uint!(64);

    fn parse_char(&mut self) -> Result<char, DecodeError> {
        let bytes = self.need_bytes_len::<1>()?;
        let byte = bytes[0];
        Ok(byte.into())
    }

    fn parse_string(&mut self) -> Result<Cow<'_, str>, DecodeError> {
        let bytes = dbg!(self.need_bytes())?;
        Ok(String::from_utf8_lossy(bytes))
    }

    fn parse_bytes(&mut self) -> Result<&[u8], DecodeError> {
        Ok(self.need_bytes()?)
    }

    fn parse_seq(&mut self) -> Result<Rlp, DecodeError> {
        let seq = dbg!(self.need_nested())?;
        let rlp = Rlp(seq.clone()); // TODO meh
        Ok(rlp)
    }
}

impl<'de, 'a> Deserializer<'de> for &mut Rlp<'a> {
    type Error = DecodeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i8(self.parse_i8()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i16(self.parse_i16()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i32(self.parse_i32()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i64(self.parse_i64()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u8(self.parse_u8()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u16(self.parse_u16()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u32(self.parse_u32()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u64(self.parse_u64()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_char(self.parse_char()?)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_str(&self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(self.parse_string()?.to_string())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_bytes(self.parse_bytes()?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_byte_buf(self.parse_bytes()?.to_vec())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(self.parse_seq()?)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::VecDeque;

    use super::from_rlp;
    use crate::{from_bytes, DecodeError, RecursiveBytes, Rlp};

    #[test]
    fn de_i8() {
        let mut rlp = Rlp([RecursiveBytes::Bytes(&[255][..])].into());
        let num: i8 = from_rlp(&mut rlp).unwrap();
        assert_eq!(num, -1);

        let mut rlp = Rlp([RecursiveBytes::Bytes(&[127][..])].into());
        let num: i8 = from_rlp(&mut rlp).unwrap();
        assert_eq!(num, 127);

        let mut rlp = Rlp([RecursiveBytes::Bytes(&[128][..])].into());
        let num: i8 = from_rlp(&mut rlp).unwrap();
        assert_eq!(num, -128);

        let num: i8 = from_bytes(&[127]).unwrap();
        assert_eq!(num, 127);

        let num: i8 = from_bytes(&[0]).unwrap();
        assert_eq!(num, 0);

        let num: i8 = from_bytes(&[0x81, 255]).unwrap();
        assert_eq!(num, -1);
    }

    #[test]
    fn de_u32() {
        let mut rlp = Rlp([RecursiveBytes::Bytes(&[255, 255, 255, 255][..])].into());
        let num: u32 = from_rlp(&mut rlp).unwrap();
        assert_eq!(num, u32::MAX);

        assert!(matches!(
            from_bytes::<u32>(&[0x83, 255, 255, 255, 255]),
            Err(DecodeError::MissingBytes)
        ));

        assert!(matches!(
            from_bytes::<u32>(&[0x82, 255, 255]),
            Err(DecodeError::InvalidLength)
        ));

        let num: u32 = from_bytes(&[0x84, 0, 0, 0, 23]).unwrap();
        assert_eq!(num, 23);
    }

    #[test]
    fn de_seq() {
        let mut rlp = Rlp([RecursiveBytes::Nested(
            [
                RecursiveBytes::Bytes(&[0][..]),
                RecursiveBytes::Bytes(&[1][..]),
            ]
            .into(),
        )]
        .into());
        let bools: [bool; 2] = from_rlp(&mut rlp).unwrap();
        assert_eq!(bools, [false, true]);

        let cat_dog: [String; 2] =
            from_bytes(&[0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g'][..]).unwrap();
        assert_eq!(cat_dog, ["cat", "dog"]);

        // alternative to &str
        let cat_dog: [Cow<'_, str>; 2] =
            from_bytes(&[0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g'][..]).unwrap();
        assert_eq!(cat_dog, ["cat", "dog"]);
    }
}

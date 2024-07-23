use crate::{unpack_rlp, DecodeError, RecursiveBytes};
use paste::paste;
use serde::de::{EnumAccess, SeqAccess, VariantAccess};
use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
use std::collections::VecDeque;

macro_rules! parse_int {
    ($ty:ty) => {
        paste! {
            fn [<parse_ $ty>](&mut self) -> Result<[<$ty>], DecodeError> {
                let bytes = self.need_bytes_len::<{std::mem::size_of::<$ty>()}>()?;
                Ok([<$ty>]::from_be_bytes(bytes))
            }
        }
    };
}

fn from_rlp<'a, T>(rlp: &'a mut Rlp) -> Result<T, DecodeError>
where
    T: Deserialize<'a>,
{
    T::deserialize(rlp)
}

pub fn from_bytes<'a, T>(bytes: Vec<u8>) -> Result<T, DecodeError>
where
    T: Deserialize<'a>,
{
    let rec = dbg!(unpack_rlp(&bytes))?;
    let mut rlp = Rlp::new(rec);
    T::deserialize(&mut rlp)
}

#[derive(Debug)]
pub struct Rlp(VecDeque<RecursiveBytes>);

impl Rlp {
    fn new(inner: VecDeque<RecursiveBytes>) -> Self {
        Rlp(inner)
    }

    fn need_bytes(&mut self) -> Result<Vec<u8>, DecodeError> {
        let first_rec = self.0.pop_front().ok_or(DecodeError::MissingBytes)?;
        let RecursiveBytes::Bytes(bytes) = first_rec else {
            return Err(DecodeError::ExpectedBytes);
        };
        Ok(bytes)
    }

    fn need_nested(&mut self) -> Result<VecDeque<RecursiveBytes>, DecodeError> {
        let first_rec = self.0.pop_front().ok_or(DecodeError::MissingBytes)?;
        let RecursiveBytes::Nested(rec) = first_rec else {
            return Err(DecodeError::ExpectedBytes);
        };
        Ok(rec)
    }

    fn need_next(&mut self) -> Result<RecursiveBytes, DecodeError> {
        let first_rec = self.0.pop_front().ok_or(DecodeError::MissingBytes)?;
        Ok(first_rec)
    }

    fn need_bytes_len<const S: usize>(&mut self) -> Result<[u8; S], DecodeError> {
        let bytes = self.need_bytes()?;
        if bytes.len() != S {
            return Err(DecodeError::InvalidLength);
        }
        Ok(bytes.try_into().unwrap())
    }

    fn parse_bool(&mut self) -> Result<bool, DecodeError> {
        let bytes = self.need_bytes_len::<1>()?;
        let byte = bytes[0];
        let bool_val = match byte {
            0 => false,
            1 => true,
            _ => return Err(DecodeError::InvalidBytes),
        };
        Ok(bool_val)
    }

    parse_int!(i8);
    parse_int!(i16);
    parse_int!(i32);
    parse_int!(i64);

    parse_int!(u8);
    parse_int!(u16);
    parse_int!(u32);
    parse_int!(u64);

    fn parse_char(&mut self) -> Result<char, DecodeError> {
        let bytes = self.need_bytes_len::<1>()?;
        let byte = bytes[0];
        Ok(byte.into())
    }

    fn parse_string(&mut self) -> Result<String, DecodeError> {
        let bytes = self.need_bytes()?;
        String::from_utf8(bytes).map_err(|_| DecodeError::InvalidBytes)
    }

    fn parse_bytes(&mut self) -> Result<Vec<u8>, DecodeError> {
        Ok(self.need_bytes()?)
    }

    fn parse_seq(&mut self) -> Result<Rlp, DecodeError> {
        let seq = dbg!(self.need_nested())?;
        let rlp = Rlp(seq);
        Ok(rlp)
    }

    fn parse_enum(
        &mut self,
        name: &'static str,
        variants: &'static [&'static str],
    ) -> Result<Rlp, DecodeError> {
        let index = variants
            .iter()
            .position(|var| var == &name)
            .expect("invalid enum variant name");
        todo!()
    }
}

struct Seq<'a> {
    de: &'a mut Rlp,
}

impl<'a> Seq<'a> {
    fn new(de: &'a mut Rlp) -> Self {
        Seq { de }
    }
}

impl<'de, 'a> SeqAccess<'de> for Seq<'a> {
    type Error = DecodeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        let el = self.de.0.pop_front().ok_or(DecodeError::MissingBytes)?;
        let rlp = &mut Rlp(VecDeque::from([el]));
        seed.deserialize(rlp).map(Some)
    }
}

struct Enum<'a> {
    de: &'a mut Rlp,
}

impl<'a> Enum<'a> {
    fn new(de: &'a mut Rlp) -> Self {
        Enum { de }
    }
}

impl<'de, 'a> EnumAccess<'de> for Enum<'a> {
    type Error = DecodeError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let bytes = self.de.need_bytes()?;
        let mut rlp = Rlp(vec![RecursiveBytes::Bytes(bytes)].into());
        let val = seed.deserialize(&mut rlp)?;
        Ok((val, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for Enum<'a> {
    type Error = DecodeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        todo!()
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        todo!()
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }
}

impl<'de, 'a> Deserializer<'de> for &'a mut Rlp {
    type Error = DecodeError;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
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

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
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
        visitor.visit_str(dbg!(&self.parse_string()?))
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
        visitor.visit_bytes(&self.parse_bytes()?)
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
        todo!()
    }

    fn deserialize_unit<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.need_bytes_len::<0>()?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let rec = self.need_nested()?;
        let rlp = &mut Rlp(rec);
        visitor.visit_seq(Seq::new(rlp))
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
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
        visitor.visit_enum(Enum::new(self))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::{from_rlp, Rlp};
    use crate::{from_bytes, DecodeError, RecursiveBytes};
    use serde::Deserialize;
    use serde_repr::Deserialize_repr;
    use std::borrow::Cow;

    #[test]
    fn de_i8() {
        let rlp = &mut Rlp([RecursiveBytes::Bytes(vec![255])].into());
        let num: i8 = from_rlp(rlp).unwrap();
        assert_eq!(num, -1);

        let rlp = &mut Rlp([RecursiveBytes::Bytes(vec![127])].into());
        let num: i8 = from_rlp(rlp).unwrap();
        assert_eq!(num, 127);

        let rlp = &mut Rlp([RecursiveBytes::Bytes(vec![128])].into());
        let num: i8 = from_rlp(rlp).unwrap();
        assert_eq!(num, -128);

        let num: i8 = from_bytes(vec![127]).unwrap();
        assert_eq!(num, 127);

        let num: i8 = from_bytes(vec![0]).unwrap();
        assert_eq!(num, 0);

        let num: i8 = from_bytes(vec![0x81, 255]).unwrap();
        assert_eq!(num, -1);
    }

    #[test]
    fn de_u32() {
        let rlp = &mut Rlp([RecursiveBytes::Bytes(vec![255, 255, 255, 255])].into());
        let num: u32 = from_rlp(rlp).unwrap();
        assert_eq!(num, u32::MAX);

        assert!(matches!(
            from_bytes::<u32>(vec![0x83, 255, 255, 255, 255]),
            Err(DecodeError::MissingBytes)
        ));

        assert!(matches!(
            from_bytes::<u32>(vec![0x82, 255, 255]),
            Err(DecodeError::InvalidLength)
        ));

        let num: u32 = from_bytes(vec![0x84, 0, 0, 0, 23]).unwrap();
        assert_eq!(num, 23);
    }

    #[test]
    fn de_seq() {
        let rlp = &mut Rlp([RecursiveBytes::Nested(
            [
                RecursiveBytes::Bytes(vec![0]),
                RecursiveBytes::Bytes(vec![1]),
            ]
            .into(),
        )]
        .into());
        let bools: [bool; 2] = from_rlp(rlp).unwrap();
        assert_eq!(bools, [false, true]);

        let cat_dog: [String; 2] =
            from_bytes(vec![0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']).unwrap();
        assert_eq!(cat_dog, ["cat", "dog"]);

        // alternative to &str
        let cat_dog: [Cow<'_, str>; 2] =
            from_bytes(vec![0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']).unwrap();
        assert_eq!(cat_dog, ["cat", "dog"]);
    }

    #[test]
    fn de_struct() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Dog {
            name: String,
            sound: String,
            age: u8,
        }

        let name = String::from("doggo");
        let sound = String::from("yo");
        let age = 10u8;

        let disc = 0xc0 + 2 + ((name.len() + sound.len() + u8::BITS as usize / 8) as u8);
        let mut bytes = vec![disc];
        bytes.push(0x80 + name.len() as u8);
        bytes.append(&mut dbg!(name.as_bytes()).to_vec());
        bytes.push(0x80 + sound.len() as u8);
        bytes.append(&mut dbg!(sound.as_bytes()).to_vec());
        bytes.push(age);

        let dog: Dog = from_bytes(bytes).unwrap();
        assert_eq!(dog, Dog { name, sound, age })
    }

    #[test]
    fn de_enum() {
        #[derive(Debug, PartialEq, Deserialize_repr)]
        #[repr(u8)]
        enum Food {
            Pizza = 0,
            Ramen = 1,
            Kebab = 2,
        }

        assert_eq!(from_bytes::<Food>(vec![0x00]).unwrap(), Food::Pizza);
        assert_eq!(from_bytes::<Food>(vec![0x01]).unwrap(), Food::Ramen);
        assert_eq!(from_bytes::<Food>(vec![0x02]).unwrap(), Food::Kebab);
        assert!(from_bytes::<Food>(vec![0x03]).is_err());
        assert!(from_bytes::<Food>(vec![255]).is_err());
    }
}

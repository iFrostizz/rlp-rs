use crate::{unpack_rlp, RecursiveBytes, Rlp, RlpError};
use paste::paste;
use serde::de::{EnumAccess, SeqAccess, VariantAccess};
use serde::{Deserialize, Deserializer};

macro_rules! parse_int {
    ($ty:ty) => {
        paste! {
            fn [<parse_ $ty>](&mut self) -> Result<[<$ty>], RlpError> {
                let bytes = self.need_bytes_len::<{std::mem::size_of::<$ty>()}>(true)?;
                Ok([<$ty>]::from_be_bytes(bytes))
            }
        }
    };
}

#[cfg(test)]
fn from_rlp<'a, T>(rlp: &'a mut Rlp) -> Result<T, RlpError>
where
    T: Deserialize<'a>,
{
    T::deserialize(rlp)
}

pub fn from_bytes<'a, T>(bytes: &[u8]) -> Result<T, RlpError>
where
    T: Deserialize<'a>,
{
    let rlp = &mut unpack_rlp(bytes)?;
    T::deserialize(rlp)
}

impl Rlp {
    pub fn read_bytes(&self) -> Result<&[u8], RlpError> {
        let RecursiveBytes::Bytes(bytes) = self.0.front().ok_or(RlpError::MissingBytes)? else {
            return Err(RlpError::ExpectedBytes);
        };
        Ok(bytes.as_slice())
    }

    fn need_bytes(&mut self) -> Result<Vec<u8>, RlpError> {
        let RecursiveBytes::Bytes(bytes) = self.0.pop_front().ok_or(RlpError::MissingBytes)? else {
            return Err(RlpError::ExpectedBytes);
        };
        Ok(bytes)
    }

    pub(crate) fn need_nested(&mut self) -> Result<Vec<RecursiveBytes>, RlpError> {
        let RecursiveBytes::Nested(rec) = self.0.pop_front().ok_or(RlpError::MissingBytes)? else {
            return Err(RlpError::ExpectedList);
        };
        Ok(rec)
    }

    fn need_next(&mut self) -> Result<RecursiveBytes, RlpError> {
        self.0.pop_front().ok_or(RlpError::MissingBytes)
    }

    fn need_bytes_len<const S: usize>(
        &mut self,
        check_trailing: bool,
    ) -> Result<[u8; S], RlpError> {
        let mut bytes = self.need_bytes()?;
        if bytes.len() > S {
            return Err(RlpError::InvalidLength);
        }
        if check_trailing && bytes.first().is_some_and(|b| b == &0x00) {
            return Err(RlpError::TrailingBytes);
        }
        for _ in 0..(S - bytes.len()) {
            bytes.insert(0, 0); // TODO kinda crap performance wise
        }
        Ok(bytes.try_into().unwrap())
    }

    fn parse_bool(&mut self) -> Result<bool, RlpError> {
        let bytes = self.need_bytes_len::<1>(false)?;
        let byte = bytes[0];
        let bool_val = match byte {
            0 => false,
            1 => true,
            _ => return Err(RlpError::InvalidBytes),
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

    fn parse_char(&mut self) -> Result<char, RlpError> {
        let bytes = self.need_bytes_len::<1>(false)?;
        let byte = bytes[0];
        Ok(byte.into())
    }

    fn parse_string(&mut self) -> Result<String, RlpError> {
        let bytes = self.need_bytes()?;
        String::from_utf8(bytes).map_err(|_| RlpError::InvalidBytes)
    }

    fn parse_bytes(&mut self) -> Result<Vec<u8>, RlpError> {
        self.need_bytes()
    }
}

struct Seq {
    de: Vec<Rlp>,
}

impl Seq {
    fn new(de: Vec<Rlp>) -> Self {
        Seq { de }
    }
}

impl<'de> SeqAccess<'de> for &mut Seq {
    type Error = RlpError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if self.de.is_empty() {
            Ok(None)
        } else {
            let rlp = &mut self.de.remove(0); // TODO maybe this is enough to switch to VecDeque everywhere ?
            seed.deserialize(rlp).map(Some)
        }
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
    type Error = RlpError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let val = seed.deserialize(&mut *self.de)?;
        Ok((val, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for Enum<'a> {
    type Error = RlpError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        let bytes = self.de.parse_bytes()?;
        let rlp = &mut Rlp(vec![RecursiveBytes::Bytes(bytes)].into());
        seed.deserialize(rlp)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.tuple_variant(fields.len(), visitor)
    }
}

impl<'de, 'a> Deserializer<'de> for &'a mut Rlp {
    type Error = RlpError;

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

    // TODO macro for all of these
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
        // TODO visit_borrowed_str
        // https://serde.rs/lifetimes.html
        visitor.visit_string(self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(self.parse_string()?)
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
        visitor.visit_byte_buf(self.parse_bytes()?)
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.need_bytes_len::<0>(false)?;
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
        let recs = self.need_nested()?;
        let rlps: Vec<Rlp> = recs.into_iter().map(|rec| Rlp(vec![rec].into())).collect(); // TODO don't allocate
        let mut seq = Seq::new(rlps);
        let res = visitor.visit_seq(&mut seq)?;
        match seq.de.is_empty() {
            true => Ok(res),
            false => Err(RlpError::InvalidLength),
        }
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
        _name: &'static str,
        _variants: &'static [&'static str],
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
        match self.need_next()? {
            RecursiveBytes::Bytes(bytes) => {
                let rlp = &mut Rlp::new_unary(RecursiveBytes::Bytes(bytes));
                rlp.deserialize_str(visitor)
            }
            RecursiveBytes::Nested(recs) => {
                // flatten structure
                *self = Rlp::new(recs.into());
                self.deserialize_str(visitor)
            }
        }
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
    use crate::{from_bytes, unpack_rlp, RecursiveBytes, RlpError};
    use serde::Deserialize;
    use serde_repr::Deserialize_repr;
    use std::borrow::Cow;

    #[test]
    fn de_i8() {
        let rlp = &mut Rlp::new_unary(RecursiveBytes::Bytes(vec![255]));
        let num: i8 = from_rlp(rlp).unwrap();
        assert_eq!(num, -1);

        let rlp = &mut Rlp::new_unary(RecursiveBytes::Bytes(vec![127]));
        let num: i8 = from_rlp(rlp).unwrap();
        assert_eq!(num, 127);

        let rlp = &mut Rlp::new_unary(RecursiveBytes::Bytes(vec![128]));
        let num: i8 = from_rlp(rlp).unwrap();
        assert_eq!(num, -128);

        let num: i8 = from_bytes(&[127]).unwrap();
        assert_eq!(num, 127);

        let num: i8 = from_bytes(&[0x80]).unwrap();
        assert_eq!(num, 0);

        let num: i8 = from_bytes(&[0x81, 255]).unwrap();
        assert_eq!(num, -1);
    }

    #[test]
    fn de_u32() {
        let rlp = &mut Rlp::new_unary(RecursiveBytes::Bytes(vec![255, 255, 255, 255]));
        let num: u32 = from_rlp(rlp).unwrap();
        assert_eq!(num, u32::MAX);

        assert!(matches!(
            from_bytes::<u32>(&[0x83, 255, 255, 255, 255]),
            Err(RlpError::MissingBytes)
        ));

        let num: u32 = from_bytes(&[23]).unwrap();
        assert_eq!(num, 23);
    }

    #[test]
    fn de_seq_bool() {
        let rlp = &mut Rlp::new(
            vec![RecursiveBytes::Nested(
                [
                    RecursiveBytes::Bytes(vec![0]),
                    RecursiveBytes::Bytes(vec![1]),
                ]
                .into(),
            )]
            .into(),
        );
        let bools: [bool; 2] = from_rlp(rlp).unwrap();
        assert_eq!(bools, [false, true]);
    }

    #[test]
    fn de_seq_string() {
        let cat_dog: [String; 2] =
            from_bytes(&[0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']).unwrap();
        assert_eq!(cat_dog, ["cat", "dog"]);
    }

    #[test]
    fn de_seq_cow() {
        // alternative to &str
        let cat_dog: [Cow<'_, str>; 2] =
            from_bytes(&[0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']).unwrap();
        assert_eq!(cat_dog, ["cat", "dog"]);
    }

    #[test]
    fn de_vec() {
        let cat = String::from("cat");
        let dog = String::from("dog");

        let mut bytes = Vec::new();
        bytes.push(0xc0 + cat.len() as u8 + dog.len() as u8 + 2);
        bytes.push(0x80 + cat.len() as u8);
        bytes.extend_from_slice(cat.as_bytes());
        bytes.push(0x80 + dog.len() as u8);
        bytes.extend_from_slice(dog.as_bytes());

        assert_eq!(
            unpack_rlp(&bytes).unwrap().0,
            vec![RecursiveBytes::Nested(vec![
                RecursiveBytes::Bytes(cat.as_bytes().to_vec()),
                RecursiveBytes::Bytes(dog.as_bytes().to_vec())
            ])]
        );

        assert_eq!(from_bytes::<Vec<String>>(&bytes).unwrap(), vec![cat, dog]);
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
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0x80 + sound.len() as u8);
        bytes.extend_from_slice(sound.as_bytes());
        bytes.push(age);

        let dog: Dog = from_bytes(&bytes).unwrap();
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

        assert_eq!(from_bytes::<Food>(&[0x80]).unwrap(), Food::Pizza);
        assert_eq!(from_bytes::<Food>(&[0x01]).unwrap(), Food::Ramen);
        assert_eq!(from_bytes::<Food>(&[0x02]).unwrap(), Food::Kebab);
        assert!(from_bytes::<Food>(&[0x03]).is_err());
        assert!(from_bytes::<Food>(&[255]).is_err());
    }

    #[derive(Debug, PartialEq, Deserialize)]
    enum Message {
        Quit,
        Move { x: i32, y: i32 },
        Write(String),
        ChangeColor(i32, i32, i32),
    }

    #[test]
    fn de_enum_unit() {
        let mut message = vec![0x80 + "Quit".len() as u8];
        message.extend_from_slice("Quit".as_bytes());
        assert_eq!(from_bytes::<Message>(&message).unwrap(), Message::Quit);
    }

    #[test]
    fn de_enum_newtype() {
        let mut message = vec![0x80 + "Write".len() as u8];
        message.extend_from_slice("Write".as_bytes());
        message.push(0x80 + "Hello world".len() as u8);
        message.extend_from_slice("Hello world".as_bytes());
        assert_eq!(
            from_bytes::<Message>(&message).unwrap(),
            Message::Write(String::from("Hello world"))
        )
    }

    // TODO enum tuple tests

    #[test]
    fn de_enum_struct() {
        // ["Move", [-1, -1]]
        let mut message = Vec::new();
        message.push(0xc0 + "Move".len() as u8 + (i32::BITS / 8 * 2) as u8 + 4);
        message.push(0x80 + "Move".len() as u8);
        message.extend_from_slice("Move".as_bytes());
        message.push(0xc0 + (i32::BITS / 8 * 2) as u8 + 2);
        message.push(0x80 + (i32::BITS / 8) as u8);
        message.extend_from_slice(&(-1i32).to_be_bytes());
        message.push(0x80 + (i32::BITS / 8) as u8);
        message.extend_from_slice(&(-1i32).to_be_bytes());

        assert_eq!(
            unpack_rlp(&message).unwrap().0,
            [RecursiveBytes::Nested(vec![
                RecursiveBytes::Bytes("Move".as_bytes().to_vec()),
                RecursiveBytes::Nested(vec![
                    RecursiveBytes::Bytes((-1i32).to_be_bytes().to_vec()),
                    RecursiveBytes::Bytes((-1i32).to_be_bytes().to_vec())
                ])
            ])]
        );

        assert_eq!(
            from_bytes::<Message>(&message).unwrap(),
            Message::Move { x: -1, y: -1 }
        );
    }

    #[test]
    fn de_enum_tuple() {
        let mut message = Vec::new();
        message.push(0xc0 + "ChangeColor".len() as u8 + ((i32::BITS / 8 + 1) * 3) as u8 + 2);
        message.push(0x80 + "ChangeColor".len() as u8);
        message.extend_from_slice("ChangeColor".as_bytes());
        message.push(0xc0 + ((i32::BITS / 8 + 1) * 3) as u8);
        message.push(0x80 + (i32::BITS / 8) as u8);
        message.extend_from_slice(&(-1i32).to_be_bytes());
        message.push(0x80 + (i32::BITS / 8) as u8);
        message.extend_from_slice(&(-212412i32).to_be_bytes());
        message.push(0x80 + (i32::BITS / 8) as u8);
        message.extend_from_slice(&(2147483647i32).to_be_bytes());

        let rlp = unpack_rlp(&message).unwrap();
        assert_eq!(
            rlp.0,
            [RecursiveBytes::Nested(vec![
                RecursiveBytes::Bytes("ChangeColor".as_bytes().to_vec()),
                RecursiveBytes::Nested(vec![
                    RecursiveBytes::Bytes((-1i32).to_be_bytes().to_vec()),
                    RecursiveBytes::Bytes((-212412i32).to_be_bytes().to_vec()),
                    RecursiveBytes::Bytes((2147483647i32).to_be_bytes().to_vec())
                ])
            ])]
        );

        assert_eq!(
            from_bytes::<Message>(&message).unwrap(),
            Message::ChangeColor(-1, -212412, 2147483647)
        );
    }

    #[test]
    fn positive_integer_leading_zeros() {
        assert!(matches!(
            from_bytes::<u64>(&[0x83, 0x00, 0x00, 0x01]),
            Err(RlpError::TrailingBytes)
        ));

        assert!(matches!(
            from_bytes::<u8>(&[0x00]),
            Err(RlpError::TrailingBytes)
        ));

        assert!(matches!(
            from_bytes::<u16>(&[0x82, 0x00, 0xff]),
            Err(RlpError::TrailingBytes)
        ));
    }
}

use crate::DecodeError;
use paste::paste;
use serde::{ser, Serialize};

#[derive(Default)]
pub struct Serializer {
    output: Vec<u8>,
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>, DecodeError>
where
    T: Serialize,
{
    let mut serializer = Serializer::default();
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

macro_rules! impl_int {
    ($ty:ty) => {
        paste! {
            fn [< serialize_ $ty >](self, v: $ty) -> Result<Self::Ok, Self::Error> {
                self.serialize_array(dbg!(v.to_be_bytes()))
            }
        }
    };
}

impl Serializer {
    fn parse_num<const N: usize>(&mut self, bytes: [u8; N]) -> Option<Vec<u8>> {
        bytes
            .iter()
            .position(|b| b > &0)
            .map(|index| bytes[index..].to_vec())
    }

    fn serialize_array<const N: usize>(&mut self, bytes: [u8; N]) -> Result<(), DecodeError> {
        self.serialize_slice(&bytes)
    }

    fn serialize_slice(&mut self, bytes: &[u8]) -> Result<(), DecodeError> {
        let bytes = if let Some(index) = bytes.iter().position(|b| b > &0) {
            &bytes[index..]
        } else {
            &[0]
        };

        ser::Serializer::serialize_bytes(self, bytes)
    }

    fn serialize_list_len(&mut self, len: usize) -> Result<(), DecodeError> {
        let mut bytes = if len <= 55 {
            vec![0xc0 + len as u8]
        } else {
            let mut len_bytes = self.parse_num(len.to_be_bytes()).unwrap();
            let mut bytes = vec![0xf7 + len_bytes.len() as u8];
            bytes.append(&mut len_bytes);
            bytes
        };

        self.output.append(&mut bytes);
        Ok(())
    }
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();

    type Error = DecodeError; // TODO change the name of this error

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.output.push(if v { 1 } else { 0 });
        Ok(())
    }

    impl_int!(i8);
    impl_int!(i16);
    impl_int!(i32);
    impl_int!(i64);

    impl_int!(u8);
    impl_int!(u16);
    impl_int!(u32);
    impl_int!(u64);

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_array([v as u8])
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.serialize_bytes(v.as_bytes())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        let mut bytes = match v.len() {
            1 if v[0] <= 127 => vec![v[0]],
            len => {
                if len <= 55 {
                    let disc = 0x80 + len as u8;
                    let mut bytes = vec![disc];
                    bytes.extend_from_slice(v);
                    bytes
                } else if len as u64 <= u64::MAX {
                    let mut len_bytes = self
                        .parse_num((len as u64).to_be_bytes())
                        .expect("fine because the length exceeds 55");
                    let disc = 0xb7 + len_bytes.len() as u8;
                    let mut bytes = vec![disc];
                    bytes.append(&mut len_bytes);
                    bytes.extend_from_slice(v);
                    bytes
                } else {
                    return Err(DecodeError::InvalidLength);
                }
            }
        };
        self.output.append(&mut bytes);
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_bytes(&[])
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_str(variant)?;
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.serialize_list_len(len.ok_or(DecodeError::InvalidBytes)?)?;
        Ok(self)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.serialize_str(variant)?;
        self.serialize_seq(Some(len))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        unimplemented!()
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.serialize_seq(Some(len))
    }
}

impl<'a> ser::SerializeSeq for &'a mut Serializer {
    type Ok = ();

    type Error = DecodeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = ();

    type Error = DecodeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();

    type Error = DecodeError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();

    type Error = DecodeError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = ();

    type Error = DecodeError;

    fn serialize_key<T>(&mut self, _key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }
}

impl<'a> ser::SerializeStruct for &'a mut Serializer {
    type Ok = ();

    type Error = DecodeError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = ();

    type Error = DecodeError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::to_bytes;

    #[test]
    fn ser_i8() {
        let num = 127i8;
        let serialized = to_bytes(&num).unwrap();
        assert_eq!(serialized, vec![0x7f]);

        let num = -127i8;
        let serialized = to_bytes(&num).unwrap();
        assert_eq!(serialized, vec![0x81, (-127i8).to_be_bytes()[0]]);
    }

    #[test]
    fn ser_char() {
        let ch = 'A';
        let serialized = to_bytes(&ch).unwrap();
        assert_eq!(serialized, vec![0x41])
    }

    #[test]
    fn ser_str() {
        let text = "dog";
        let serialized = to_bytes(&text).unwrap();
        let mut bytes = vec![0x83];
        bytes.append(&mut text.as_bytes().to_vec());
        assert_eq!(serialized, bytes)
    }

    #[test]
    fn ser_vec() {
        let vec = vec![1u8, 2, 3, 4, 5];
        let serialized = to_bytes(&vec).unwrap();
        assert_eq!(serialized, vec![0xc5, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn ser_array() {
        let arr = [10; 10];
        let serialized = to_bytes(&arr).unwrap();
        let mut bytes = vec![0xca];
        bytes.extend_from_slice(&arr);
        assert_eq!(serialized, bytes);
    }

    #[derive(Debug, PartialEq, Serialize)]
    enum Message {
        Quit,
        Move { x: i32, y: i32 },
        Write(String),
        ChangeColor(i32, i32, i32),
    }

    #[test]
    fn ser_enum_unit() {
        let serialized = to_bytes(&Message::Quit).unwrap();
        let mut bytes = vec![0x80 + "Quit".len() as u8];
        bytes.extend_from_slice("Quit".as_bytes());
        assert_eq!(serialized, bytes);
    }

    #[test]
    fn ser_enum_newtype() {
        let serialized = to_bytes(&Message::Write(String::from("Hello world"))).unwrap();
        let mut bytes = vec![0x80 + "Write".len() as u8];
        bytes.extend_from_slice("Write".as_bytes());
        bytes.push(0x80 + "Hello world".len() as u8);
        bytes.extend_from_slice("Hello world".as_bytes());
        assert_eq!(serialized, bytes);
    }

    // TODO looks like we will need the IR here.
    // We need to be able to deduce the length of the list but we cannot know it
    // before having serialized the nested numbers.
    // We will need to write Serializer for &mut Rlp and then the opposite of unpack_rlp
    // in order to spit out bytes.
    #[test]
    fn ser_enum_tuple() {
        let serialized = to_bytes(&Message::ChangeColor(i32::MAX, -1, i32::MIN)).unwrap();
        let mut bytes = vec![0x80 + "ChangeColor".len() as u8];
        bytes.extend_from_slice("ChangeColor".as_bytes());
        bytes.push(0xc0 + (i32::BITS as u8 / 8 + 1) * 3);
        bytes.push(0x80 + i32::BITS as u8 / 8);
        bytes.extend_from_slice(&i32::MAX.to_be_bytes());
        bytes.push(0x80 + i32::BITS as u8 / 8);
        bytes.extend_from_slice(&(-1i32).to_be_bytes());
        bytes.push(0x80 + i32::BITS as u8 / 8);
        bytes.extend_from_slice(&i32::MIN.to_be_bytes());
        assert_eq!(serialized, bytes);
    }
}

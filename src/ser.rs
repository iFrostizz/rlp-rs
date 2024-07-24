use crate::{pack_rlp, DecodeError, RecursiveBytes, Rlp};
use paste::paste;
use serde::{ser, Serialize};
use std::collections::VecDeque;

pub struct Serializer {
    output: Rlp,
    /// holds references to the nested data structures in the rlp representation.
    nesting: Vec<usize>,
}

impl Serializer {
    fn push(&mut self, rec: RecursiveBytes) {
        self.output.0.push_back(rec);
    }

    fn len(&self) -> usize {
        self.output.0.len()
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut RecursiveBytes> {
        self.output.0.get_mut(index)
    }

    fn get_nested_mut(&mut self, index: usize) -> Option<&mut VecDeque<RecursiveBytes>> {
        self.get_mut(index).map(|rec| match rec {
            RecursiveBytes::Nested(inner) => inner,
            RecursiveBytes::Bytes(_) => panic!("invalid index pointing to Bytes"),
        })
    }

    fn push_bytes(&mut self, bytes: Vec<u8>) {
        if let Some(index) = self.nesting.last().copied() {
            let nested = self.get_nested_mut(index).expect("missing nested from rlp");
            nested.push_back(RecursiveBytes::Bytes(bytes));
        } else {
            self.push(RecursiveBytes::Bytes(bytes));
        }
    }

    fn new_list(&mut self) {
        // create a new list and increase the level of nesting.
        self.nesting.push(self.len());
        self.push(RecursiveBytes::empty_list());
    }

    fn end_list(&mut self) {
        // forget about the reference to the nested list and go one level higher.
        self.nesting.pop();
    }
}

pub(crate) fn to_rlp<T>(value: &T) -> Result<Rlp, DecodeError>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        output: Rlp::new(VecDeque::new()),
        nesting: Vec::new(),
    };
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>, DecodeError>
where
    T: Serialize,
{
    let rlp = to_rlp(value)?;
    pack_rlp(rlp)
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
        self.serialize_array(if v { [1] } else { [0] })
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
        self.push_bytes(v.to_vec());
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

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.new_list();
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
        self.end_list();
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
    use super::to_bytes;
    use crate::ser::to_rlp;
    use crate::{pack_rlp, RecursiveBytes, Rlp};
    use serde::Serialize;

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

        let rlp = to_rlp(&ch).unwrap();
        assert_eq!(rlp.0, vec![RecursiveBytes::Bytes(vec![0x41])]);

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
    fn pack_vec_cat_dog() {
        let cat = String::from("cat");
        let dog = String::from("dog");

        let rlp = Rlp::new_unary(RecursiveBytes::Nested(
            vec![
                RecursiveBytes::Bytes(cat.as_bytes().to_vec()),
                RecursiveBytes::Bytes(dog.as_bytes().to_vec()),
            ]
            .into(),
        ));

        let packed = pack_rlp(rlp).unwrap();

        let mut bytes = Vec::new();
        bytes.push(0xc0 + cat.len() as u8 + dog.len() as u8 + 2);
        bytes.push(0x80 + cat.len() as u8);
        bytes.extend_from_slice(cat.as_bytes());
        bytes.push(0x80 + dog.len() as u8);
        bytes.extend_from_slice(dog.as_bytes());

        assert_eq!(packed, bytes);
    }

    #[test]
    fn ser_vec_cat_dog() {
        let cat = String::from("cat");
        let dog = String::from("dog");

        let vec = vec![cat.clone(), dog.clone()];

        let expected_rlp = Rlp::new_unary(RecursiveBytes::Nested(
            vec![
                RecursiveBytes::Bytes(cat.as_bytes().to_vec()),
                RecursiveBytes::Bytes(dog.as_bytes().to_vec()),
            ]
            .into(),
        ));

        let rlp = to_rlp(&vec).unwrap();

        assert_eq!(rlp.0, expected_rlp.0);
    }

    #[test]
    fn ser_vec() {
        let vec = vec![1u8, 2, 3, 4, 5];

        let rlp = to_rlp(&vec).unwrap();
        assert_eq!(
            rlp.0,
            vec![RecursiveBytes::Nested(
                vec![
                    RecursiveBytes::Bytes(vec![1]),
                    RecursiveBytes::Bytes(vec![2]),
                    RecursiveBytes::Bytes(vec![3]),
                    RecursiveBytes::Bytes(vec![4]),
                    RecursiveBytes::Bytes(vec![5]),
                ]
                .into()
            )
            .into()]
        );

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

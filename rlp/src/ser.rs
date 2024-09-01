use crate::{pack_rlp, RecursiveBytes, Rlp, RlpError};
use paste::paste;
use serde::{ser, Serialize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Debug)]
enum RefRecursiveBytes {
    Data(Vec<u8>),
    EmptyList,
    Nested(Rc<RefCell<Vec<RefRecursiveBytes>>>),
}

#[derive(Default)]
struct Serializer {
    output: VecDeque<RefRecursiveBytes>,
    stack: Vec<Rc<RefCell<Vec<RefRecursiveBytes>>>>,
}

impl Serializer {
    /// pushes a new list to the most nested one we are currently in
    fn new_list(&mut self) {
        let rc_list = Rc::new(RefCell::new(Vec::with_capacity(0)));
        let nested = RefRecursiveBytes::Nested(rc_list.clone());

        if let Some(top) = self.stack.last_mut() {
            top.borrow_mut().push(nested);
        } else {
            self.output.push_back(nested);
        }

        self.stack.push(rc_list);
    }

    /// forget about the reference to the nested list and go one level higher.
    fn end_list(&mut self) {
        self.stack.pop();
    }

    /// pushes bytes to the most nested list we are in or at the highest level.
    fn push_bytes(&mut self, bytes: &[u8], fixed: bool) {
        let bytes = if fixed {
            if let Some(index) = bytes.iter().position(|b| b > &0) {
                RefRecursiveBytes::Data(bytes[index..].to_vec())
            } else {
                RefRecursiveBytes::Data(vec![])
            }
        } else if bytes.is_empty() {
            RefRecursiveBytes::EmptyList
        } else {
            RefRecursiveBytes::Data(bytes.to_vec())
        };

        if let Some(top) = self.stack.last_mut() {
            top.borrow_mut().push(bytes);
        } else {
            self.output.push_back(bytes);
        }
    }

    fn recursive_into_recursive_bytes(rec: RefRecursiveBytes) -> RecursiveBytes {
        match rec {
            RefRecursiveBytes::Data(bytes) => RecursiveBytes::Bytes(bytes),
            RefRecursiveBytes::EmptyList => RecursiveBytes::EmptyList,
            RefRecursiveBytes::Nested(list) => {
                let list = Rc::try_unwrap(list).unwrap().into_inner();
                let rec_list = list
                    .into_iter()
                    .map(Self::recursive_into_recursive_bytes)
                    .collect();
                RecursiveBytes::Nested(rec_list)
            }
        }
    }

    fn into_rlp(mut self) -> Rlp {
        assert!(
            self.stack.is_empty(),
            "still have some elements on the worklist"
        );
        let mut rlp = Rlp::default();
        while let Some(rec) = self.output.pop_front() {
            rlp.0.push_back(Self::recursive_into_recursive_bytes(rec));
        }
        rlp
    }
}

pub(crate) fn to_rlp<T>(value: &T) -> Result<Rlp, RlpError>
where
    T: Serialize,
{
    let mut serializer = Serializer::default();
    value.serialize(&mut serializer)?;
    Ok(serializer.into_rlp())
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>, RlpError>
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
                self.serialize_array(v.to_be_bytes())
            }
        }
    };
}

impl Serializer {
    fn serialize_array<const N: usize>(&mut self, bytes: [u8; N]) -> Result<(), RlpError> {
        self.push_bytes(&bytes, true);
        Ok(())
    }
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();

    type Error = RlpError; // TODO change the name of this error

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
        self.push_bytes(v, false);
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
        if !variant.is_empty() {
            self.serialize_str(variant)?
        }
        Ok(())
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
        if !variant.is_empty() {
            self.serialize_str(variant)?;
        }
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.new_list();
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(self)
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
        if !variant.is_empty() {
            self.serialize_str(variant)?;
        }
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
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        if !variant.is_empty() {
            self.serialize_str(variant)?;
        }
        Ok(self)
    }
}

impl<'a> ser::SerializeSeq for &'a mut Serializer {
    type Ok = ();

    type Error = RlpError;

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

    type Error = RlpError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // tuples are not treated as "list with known size"
        Ok(())
    }
}

impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();

    type Error = RlpError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
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

impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();

    type Error = RlpError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
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

impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = ();

    type Error = RlpError;

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

    type Error = RlpError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
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

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = ();

    type Error = RlpError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
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

        let rlp = Rlp::new_unary(RecursiveBytes::Nested(vec![
            RecursiveBytes::Bytes(cat.as_bytes().to_vec()),
            RecursiveBytes::Bytes(dog.as_bytes().to_vec()),
        ]));

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

        let expected_rlp = Rlp::new_unary(RecursiveBytes::Nested(vec![
            RecursiveBytes::Bytes(cat.as_bytes().to_vec()),
            RecursiveBytes::Bytes(dog.as_bytes().to_vec()),
        ]));

        let rlp = to_rlp(&vec).unwrap();

        assert_eq!(rlp.0, expected_rlp.0);
    }

    #[test]
    fn ser_vec() {
        let vec = vec![1u8, 2, 3, 4, 5];

        let rlp = to_rlp(&vec).unwrap();
        assert_eq!(
            rlp.0,
            vec![RecursiveBytes::Nested(vec![
                RecursiveBytes::Bytes(vec![1]),
                RecursiveBytes::Bytes(vec![2]),
                RecursiveBytes::Bytes(vec![3]),
                RecursiveBytes::Bytes(vec![4]),
                RecursiveBytes::Bytes(vec![5]),
            ])]
        );

        let serialized = to_bytes(&vec).unwrap();
        assert_eq!(serialized, vec![0xc5, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn ser_array() {
        let arr = [10; 10];
        let serialized = to_bytes(&arr).unwrap();
        let mut bytes = vec![];
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

    #[test]
    fn ser_enum_struct() {
        let message = Message::Move {
            x: i32::MAX,
            y: -10,
        };

        let rlp = to_rlp(&message).unwrap();
        assert_eq!(
            rlp.0,
            vec![
                RecursiveBytes::Bytes("Move".as_bytes().to_vec()),
                RecursiveBytes::Bytes(i32::MAX.to_be_bytes().to_vec()),
                RecursiveBytes::Bytes((-10i32).to_be_bytes().to_vec()),
            ]
        );

        let serialized = to_bytes(&message).unwrap();

        let mut bytes = vec![0x80 + "Move".len() as u8];
        bytes.extend_from_slice("Move".as_bytes());
        bytes.push(0x80 + i32::BITS as u8 / 8);
        bytes.extend_from_slice(&i32::MAX.to_be_bytes());
        bytes.push(0x80 + i32::BITS as u8 / 8);
        bytes.extend_from_slice(&(-10i32).to_be_bytes());
        assert_eq!(serialized, bytes);
    }

    #[derive(Debug, Serialize)]
    struct Tuple(
        #[serde(with = "serde_bytes")] [u8; 10],
        #[serde(with = "serde_bytes")] [u8; 20],
        #[serde(with = "serde_bytes")] [u8; 30],
    );

    #[test]
    fn ser_struct_tuple_bytes() {
        let tuple = Tuple([1; 10], [1; 20], [1; 30]);

        let rlp = to_rlp(&tuple).unwrap();
        assert_eq!(
            rlp.0,
            vec![RecursiveBytes::Nested(vec![
                RecursiveBytes::Bytes(vec![1; 10]),
                RecursiveBytes::Bytes(vec![1; 20]),
                RecursiveBytes::Bytes(vec![1; 30])
            ])]
        );
    }

    #[test]
    fn ser_empty_vec() {
        #[derive(Debug, Serialize)]
        struct MyVec(Vec<u8>);
        let vec = MyVec(vec![]);

        let rlp = to_rlp(&vec).unwrap();
        assert_eq!(rlp.0, vec![RecursiveBytes::Nested(vec![])]);

        let serialized = to_bytes(&vec).unwrap();
        assert_eq!(serialized, vec![0xc0]);
    }

    #[test]
    fn ser_empty_bytes() {
        #[derive(Debug, Serialize)]
        struct MyVec(#[serde(with = "serde_bytes")] Vec<u8>);
        let vec = MyVec(vec![]);

        let rlp = to_rlp(&vec).unwrap();
        assert_eq!(rlp.0, vec![RecursiveBytes::EmptyList]);

        let serialized = to_bytes(&vec).unwrap();
        assert_eq!(serialized, vec![0x80]);
    }

    #[test]
    fn ser_enum_empty_struct_bytes() {
        #[derive(Debug, Serialize)]
        enum MyEnum {
            Variant1 {
                #[serde(with = "serde_bytes")]
                data: Vec<u8>,
            },
        }

        let en = MyEnum::Variant1 { data: vec![] };

        let rlp = to_rlp(&en).unwrap();
        assert_eq!(
            rlp.0,
            vec![
                RecursiveBytes::Bytes("Variant1".as_bytes().to_vec()),
                RecursiveBytes::EmptyList
            ]
        );
    }

    #[test]
    fn ser_trailing_bytes_u64() {
        let num: u64 = 0;
        let bytes = to_bytes(&num).unwrap();
        assert_eq!(bytes, vec![0x80]);
    }

    #[test]
    fn ser_tuple_string() {
        let cat_dog: [&str; 2] = ["cat", "dog"];
        let bytes = to_bytes(&cat_dog).unwrap();
        assert_eq!(&bytes, &[0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']);
    }

    #[test]
    fn ser_vec_string() {
        let cat_dog = vec!["cat", "dog"];
        let bytes = to_bytes(&cat_dog).unwrap();
        assert_eq!(
            &bytes,
            &[0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']
        );
    }
}

use std::collections::VecDeque;
use std::fmt::{self, Display};

mod de;
pub use de::from_bytes;

mod ser;
pub use ser::to_bytes;

#[derive(Debug)]
pub enum DecodeError {
    MissingBytes,
    TrailingBytes,
    ExpectedBytes,
    ExpectedList,
    InvalidBytes,
    InvalidLength,
    Message(String),
}

impl serde::ser::Error for DecodeError {
    fn custom<T: Display>(msg: T) -> Self {
        DecodeError::Message(msg.to_string())
    }
}

impl serde::de::Error for DecodeError {
    fn custom<T: Display>(msg: T) -> Self {
        DecodeError::Message(msg.to_string())
    }
}

impl Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecodeError::Message(message) => formatter.write_str(message),
            DecodeError::MissingBytes => {
                formatter.write_str("missing bytes after discriminant byte")
            }
            DecodeError::ExpectedList => formatter.write_str("expected list, got bytes"),
            DecodeError::ExpectedBytes => formatter.write_str("expected bytes, got list"),
            DecodeError::InvalidBytes => formatter.write_str("invalid bytes"),
            DecodeError::InvalidLength => formatter.write_str("invalid length"),
            DecodeError::TrailingBytes => formatter.write_str("trailing bytes"),
        }
    }
}

impl std::error::Error for DecodeError {}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum RecursiveBytes {
    Bytes(Vec<u8>),
    Nested(VecDeque<RecursiveBytes>),
}

impl RecursiveBytes {
    fn empty_list() -> Self {
        RecursiveBytes::Nested(VecDeque::new())
    }
}

#[derive(Debug)]
pub struct Rlp(VecDeque<RecursiveBytes>);

impl Rlp {
    fn new(inner: VecDeque<RecursiveBytes>) -> Self {
        Rlp(inner)
    }

    fn new_unary(inner: RecursiveBytes) -> Self {
        Rlp(vec![inner].into())
    }

    fn pop_front(&mut self) -> Option<RecursiveBytes> {
        self.0.pop_front()
    }
}

// run a BFS to unpack the rlp
fn recursive_unpack_rlp(
    bytes: &[u8],
    mut cursor: usize,
) -> Result<VecDeque<RecursiveBytes>, DecodeError> {
    let disc = if let Some(disc) = bytes.get(cursor) {
        *disc
    } else {
        return Ok(VecDeque::new());
    };
    cursor += 1;

    let mut unpacked = VecDeque::new();

    let ret = if disc <= 127 {
        // TODO change me, maybe remove vec
        let ret = bytes.get((cursor - 1)..cursor).unwrap();

        RecursiveBytes::Bytes(ret.to_vec())
    } else if disc <= 183 {
        let len = disc - 128;
        let ret = bytes
            .get(cursor..(cursor + len as usize))
            .ok_or(DecodeError::MissingBytes)?;
        cursor += len as usize;

        RecursiveBytes::Bytes(ret.to_vec())
    } else if disc <= 191 {
        let len_bytes_len = disc - 183;
        if len_bytes_len > 8 {
            unimplemented!("we do not support > 2**64 bytes long strings");
        }
        let mut len_bytes_base = [0; 8];
        let len_bytes = bytes
            .get(cursor..(cursor + len_bytes_len as usize))
            .ok_or(DecodeError::MissingBytes)?;
        cursor += len_bytes_len as usize;

        len_bytes_base[(8 - len_bytes.len())..].copy_from_slice(len_bytes);
        let len = usize::from_be_bytes(len_bytes_base);
        let ret = bytes
            .get(cursor..(cursor + len as usize))
            .ok_or(DecodeError::MissingBytes)?;
        cursor += len as usize;

        RecursiveBytes::Bytes(ret.to_vec())
    } else if disc <= 247 {
        let len = disc - 192;
        let list_bytes = bytes
            .get(cursor..(cursor + len as usize))
            .ok_or(DecodeError::MissingBytes)?;
        cursor += len as usize;

        // we want to represent empty lists so don't remove them
        RecursiveBytes::Nested(recursive_unpack_rlp(list_bytes, 0)?)
    } else {
        let len_bytes_len = disc - 247;
        let mut len_bytes_base = [0; 8];
        let len_bytes = bytes
            .get(cursor..(cursor + len_bytes_len as usize))
            .ok_or(DecodeError::MissingBytes)?;
        cursor += len_bytes_len as usize;
        len_bytes_base[(8 - len_bytes.len())..].copy_from_slice(len_bytes);
        let len = usize::from_be_bytes(len_bytes_base);
        let list_bytes = bytes
            .get(cursor..(cursor + len as usize))
            .ok_or(DecodeError::MissingBytes)?;
        cursor += len as usize;

        RecursiveBytes::Nested(recursive_unpack_rlp(list_bytes, 0)?)
    };

    unpacked.push_back(ret);
    unpacked.append(&mut recursive_unpack_rlp(bytes, cursor)?);

    Ok(unpacked)
}

pub(crate) fn unpack_rlp(bytes: &[u8]) -> Result<Rlp, DecodeError> {
    Ok(Rlp::new(recursive_unpack_rlp(bytes, 0)?))
}

fn parse_num<const N: usize>(bytes: [u8; N]) -> Option<Vec<u8>> {
    bytes
        .iter()
        .position(|b| b > &0)
        .map(|index| bytes[index..].to_vec())
}

fn append_rlp_bytes(pack: &mut Vec<u8>, new_bytes: Vec<u8>) -> Result<usize, DecodeError> {
    let mut bytes = match new_bytes.len() {
        1 if new_bytes[0] <= 127 => vec![new_bytes[0]],
        len => {
            if len <= 55 {
                let disc = 0x80 + len as u8;
                let mut bytes = vec![disc];
                bytes.extend_from_slice(&new_bytes);
                bytes
            } else if len as u64 <= u64::MAX {
                let mut len_bytes = parse_num((len as u64).to_be_bytes())
                    .expect("fine because the length exceeds 55");
                let disc = 0xb7 + len_bytes.len() as u8;
                let mut bytes = vec![disc];
                bytes.append(&mut len_bytes);
                bytes.extend_from_slice(&new_bytes);
                bytes
            } else {
                return Err(DecodeError::InvalidLength);
            }
        }
    };

    let len = bytes.len();

    pack.append(&mut bytes);

    Ok(len)
}

fn serialize_list_len(len: usize) -> Result<Vec<u8>, DecodeError> {
    let bytes = if len <= 55 {
        vec![0xc0 + len as u8]
    } else {
        let mut len_bytes = parse_num(len.to_be_bytes()).unwrap();
        let mut bytes = vec![0xf7 + len_bytes.len() as u8];
        bytes.append(&mut len_bytes);
        bytes
    };

    Ok(bytes)
}

fn recursive_pack_rlp(rec: RecursiveBytes, pack: &mut Vec<u8>) -> Result<usize, DecodeError> {
    match dbg!(rec) {
        RecursiveBytes::Bytes(bytes) => append_rlp_bytes(pack, bytes),
        RecursiveBytes::Nested(recs) => {
            let mut len = 0;
            let inner_pack = &mut Vec::new();
            for rec in recs {
                len += recursive_pack_rlp(rec, inner_pack)?;
            }
            let len_bytes = &mut serialize_list_len(len)?;
            pack.append(len_bytes);
            pack.append(inner_pack);
            Ok(len)
        }
    }
}

pub(crate) fn pack_rlp(mut rlp: Rlp) -> Result<Vec<u8>, DecodeError> {
    // use std::collections::VecDeque;

    // use crate::{pack_rlp, DecodeError, RecursiveBytes, Rlp};
    // use paste::paste;
    // use serde::{ser, Serialize};

    // pub struct Serializer {
    //     output: Rlp,
    // }

    // impl Serializer {
    //     fn push(&mut self, rec: RecursiveBytes) {
    //         self.output.0.push_back(rec);
    //     }
    // }

    // pub(crate) fn to_rlp<T>(value: &T) -> Result<Rlp, DecodeError>
    // where
    //     T: Serialize,
    // {
    //     let mut serializer = Serializer {
    //         output: Rlp::new(VecDeque::new()),
    //     };
    //     value.serialize(&mut serializer)?;
    //     Ok(serializer.output)
    // }

    // pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>, DecodeError>
    // where
    //     T: Serialize,
    // {
    //     let rlp = to_rlp(value)?;
    //     pack_rlp(rlp)
    // }

    // macro_rules! impl_int {
    //     ($ty:ty) => {
    //         paste! {
    //             fn [< serialize_ $ty >](self, v: $ty) -> Result<Self::Ok, Self::Error> {
    //                 self.serialize_array(dbg!(v.to_be_bytes()))
    //             }
    //         }
    //     };
    // }

    // impl Serializer {
    //     fn parse_num<const N: usize>(&mut self, bytes: [u8; N]) -> Option<Vec<u8>> {
    //         bytes
    //             .iter()
    //             .position(|b| b > &0)
    //             .map(|index| bytes[index..].to_vec())
    //     }

    //     fn serialize_array<const N: usize>(&mut self, bytes: [u8; N]) -> Result<(), DecodeError> {
    //         self.serialize_slice(&bytes)
    //     }

    //     fn serialize_slice(&mut self, bytes: &[u8]) -> Result<(), DecodeError> {
    //         let bytes = if let Some(index) = bytes.iter().position(|b| b > &0) {
    //             &bytes[index..]
    //         } else {
    //             &[0]
    //         };

    //         ser::Serializer::serialize_bytes(self, bytes)
    //     }

    //     fn serialize_list_len(&mut self, len: usize) -> Result<(), DecodeError> {
    //         let mut bytes = if len <= 55 {
    //             vec![0xc0 + len as u8]
    //         } else {
    //             let mut len_bytes = self.parse_num(len.to_be_bytes()).unwrap();
    //             let mut bytes = vec![0xf7 + len_bytes.len() as u8];
    //             bytes.append(&mut len_bytes);
    //             bytes
    //         };

    //         self.output.append(&mut bytes);
    //         Ok(())
    //     }
    // }

    // impl<'a> ser::Serializer for &'a mut Serializer {
    //     type Ok = ();

    //     type Error = DecodeError; // TODO change the name of this error

    //     type SerializeSeq = Self;
    //     type SerializeTuple = Self;
    //     type SerializeTupleStruct = Self;
    //     type SerializeTupleVariant = Self;
    //     type SerializeMap = Self;
    //     type SerializeStruct = Self;
    //     type SerializeStructVariant = Self;

    //     fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
    //         self.output.push(if v { 1 } else { 0 });
    //         Ok(())
    //     }

    //     impl_int!(i8);
    //     impl_int!(i16);
    //     impl_int!(i32);
    //     impl_int!(i64);

    //     impl_int!(u8);
    //     impl_int!(u16);
    //     impl_int!(u32);
    //     impl_int!(u64);

    //     fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
    //         unimplemented!()
    //     }

    //     fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
    //         unimplemented!()
    //     }

    //     fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
    //         self.serialize_array([v as u8])
    //     }

    //     fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
    //         self.serialize_bytes(v.as_bytes())
    //     }

    //     fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
    //         let mut bytes = match v.len() {
    //             1 if v[0] <= 127 => vec![v[0]],
    //             len => {
    //                 if len <= 55 {
    //                     let disc = 0x80 + len as u8;
    //                     let mut bytes = vec![disc];
    //                     bytes.extend_from_slice(v);
    //                     bytes
    //                 } else if len as u64 <= u64::MAX {
    //                     let mut len_bytes = self
    //                         .parse_num((len as u64).to_be_bytes())
    //                         .expect("fine because the length exceeds 55");
    //                     let disc = 0xb7 + len_bytes.len() as u8;
    //                     let mut bytes = vec![disc];
    //                     bytes.append(&mut len_bytes);
    //                     bytes.extend_from_slice(v);
    //                     bytes
    //                 } else {
    //                     return Err(DecodeError::InvalidLength);
    //                 }
    //             }
    //         };
    //         self.output.append(&mut bytes);
    //         Ok(())
    //     }

    //     fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
    //         unimplemented!()
    //     }

    //     fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         unimplemented!()
    //     }

    //     fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
    //         self.serialize_bytes(&[])
    //     }

    //     fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
    //         self.serialize_unit()
    //     }

    //     fn serialize_unit_variant(
    //         self,
    //         _name: &'static str,
    //         _variant_index: u32,
    //         variant: &'static str,
    //     ) -> Result<Self::Ok, Self::Error> {
    //         self.serialize_str(variant)
    //     }

    //     fn serialize_newtype_struct<T>(
    //         self,
    //         _name: &'static str,
    //         value: &T,
    //     ) -> Result<Self::Ok, Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         value.serialize(self)
    //     }

    //     fn serialize_newtype_variant<T>(
    //         self,
    //         _name: &'static str,
    //         _variant_index: u32,
    //         variant: &'static str,
    //         value: &T,
    //     ) -> Result<Self::Ok, Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         self.serialize_str(variant)?;
    //         value.serialize(self)
    //     }

    //     fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
    //         self.serialize_list_len(len.ok_or(DecodeError::InvalidBytes)?)?;
    //         Ok(self)
    //     }

    //     fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
    //         self.serialize_seq(Some(len))
    //     }

    //     fn serialize_tuple_struct(
    //         self,
    //         _name: &'static str,
    //         len: usize,
    //     ) -> Result<Self::SerializeTupleStruct, Self::Error> {
    //         self.serialize_seq(Some(len))
    //     }

    //     fn serialize_tuple_variant(
    //         self,
    //         _name: &'static str,
    //         _variant_index: u32,
    //         variant: &'static str,
    //         len: usize,
    //     ) -> Result<Self::SerializeTupleVariant, Self::Error> {
    //         self.serialize_str(variant)?;
    //         self.serialize_seq(Some(len))
    //     }

    //     fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
    //         unimplemented!()
    //     }

    //     fn serialize_struct(
    //         self,
    //         _name: &'static str,
    //         len: usize,
    //     ) -> Result<Self::SerializeStruct, Self::Error> {
    //         self.serialize_seq(Some(len))
    //     }

    //     fn serialize_struct_variant(
    //         self,
    //         _name: &'static str,
    //         _variant_index: u32,
    //         _variant: &'static str,
    //         len: usize,
    //     ) -> Result<Self::SerializeStructVariant, Self::Error> {
    //         self.serialize_seq(Some(len))
    //     }
    // }

    // impl<'a> ser::SerializeSeq for &'a mut Serializer {
    //     type Ok = ();

    //     type Error = DecodeError;

    //     fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         value.serialize(&mut **self)
    //     }

    //     fn end(self) -> Result<Self::Ok, Self::Error> {
    //         Ok(())
    //     }
    // }

    // impl<'a> ser::SerializeTuple for &'a mut Serializer {
    //     type Ok = ();

    //     type Error = DecodeError;

    //     fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         value.serialize(&mut **self)
    //     }

    //     fn end(self) -> Result<Self::Ok, Self::Error> {
    //         Ok(())
    //     }
    // }

    // impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    //     type Ok = ();

    //     type Error = DecodeError;

    //     fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         value.serialize(&mut **self)
    //     }

    //     fn end(self) -> Result<Self::Ok, Self::Error> {
    //         Ok(())
    //     }
    // }

    // impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    //     type Ok = ();

    //     type Error = DecodeError;

    //     fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         value.serialize(&mut **self)
    //     }

    //     fn end(self) -> Result<Self::Ok, Self::Error> {
    //         Ok(())
    //     }
    // }

    // impl<'a> ser::SerializeMap for &'a mut Serializer {
    //     type Ok = ();

    //     type Error = DecodeError;

    //     fn serialize_key<T>(&mut self, _key: &T) -> Result<(), Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         unimplemented!()
    //     }

    //     fn serialize_value<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         unimplemented!()
    //     }

    //     fn end(self) -> Result<Self::Ok, Self::Error> {
    //         unimplemented!()
    //     }
    // }

    // impl<'a> ser::SerializeStruct for &'a mut Serializer {
    //     type Ok = ();

    //     type Error = DecodeError;

    //     fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         value.serialize(&mut **self)
    //     }

    //     fn end(self) -> Result<Self::Ok, Self::Error> {
    //         Ok(())
    //     }
    // }

    // impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    //     type Ok = ();

    //     type Error = DecodeError;

    //     fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    //     where
    //         T: ?Sized + Serialize,
    //     {
    //         value.serialize(&mut **self)
    //     }

    //     fn end(self) -> Result<Self::Ok, Self::Error> {
    //         Ok(())
    //     }
    // }

    let mut pack = Vec::new();
    while let Some(rec) = rlp.pop_front() {
        recursive_pack_rlp(rec, &mut pack)?;
    }
    Ok(pack)
}

// https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/#examples
#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{unpack_rlp, RecursiveBytes};

    #[test]
    fn unpack_dog() {
        let dog_bites = "dog".as_bytes();
        let mut dog_rlp = vec![0x83];
        dog_rlp.extend_from_slice(dog_bites);

        let unpacked = unpack_rlp(&dog_rlp).unwrap();
        assert_eq!(
            unpacked.0,
            vec![RecursiveBytes::Bytes(vec![b'd', b'o', b'g'])]
        );
    }

    #[test]
    fn unpack_cat_dog_list() {
        let dog_bites = "dog".as_bytes();
        let mut dog_rlp = vec![0x83];
        dog_rlp.extend_from_slice(dog_bites);

        let cat_bites = "cat".as_bytes();
        let mut cat_rlp = vec![0x83];
        cat_rlp.extend_from_slice(cat_bites);

        let mut cat_dog_rlp = vec![0xc8];
        cat_dog_rlp.append(&mut dog_rlp);
        cat_dog_rlp.append(&mut cat_rlp);

        let unpacked = unpack_rlp(&cat_dog_rlp).unwrap();
        assert_eq!(
            unpacked.0,
            vec![RecursiveBytes::Nested(
                vec![
                    RecursiveBytes::Bytes(vec![b'd', b'o', b'g']),
                    RecursiveBytes::Bytes(vec![b'c', b'a', b't']),
                ]
                .into()
            )]
        );
    }

    #[test]
    fn unpack_empty_string() {
        let unpacked = unpack_rlp(&[0x80][..]).unwrap();
        assert_eq!(unpacked.0, vec![RecursiveBytes::Bytes(Vec::new())]);
    }

    #[test]
    fn unpack_empty_list() {
        let unpacked = unpack_rlp(&[0xc0][..]).unwrap();
        assert_eq!(unpacked.0, vec![RecursiveBytes::Nested(VecDeque::new())]);
    }

    #[test]
    #[ignore = "there is no way to decode the number 0, the priority is given to the empty string"]
    fn unpack_zero() {
        let unpacked = unpack_rlp(&[0x80][..]).unwrap();
        assert_eq!(unpacked.0, vec![RecursiveBytes::Bytes(vec![0])]);
    }

    #[test]
    fn unpack_null_byte() {
        let unpacked = unpack_rlp(&[0x00][..]).unwrap();
        assert_eq!(unpacked.0, vec![RecursiveBytes::Bytes(vec![0])]);
    }

    #[test]
    fn unpack_0f() {
        let unpacked = unpack_rlp(&[0x0f][..]).unwrap();
        assert_eq!(unpacked.0, vec![RecursiveBytes::Bytes(vec![0x0f])]);
    }

    #[test]
    fn unpack_two_bytes() {
        let unpacked = unpack_rlp(&[0x82, 0x04, 0x00][..]).unwrap();
        assert_eq!(unpacked.0, vec![RecursiveBytes::Bytes(vec![0x04, 0x00])]);
    }

    #[test]
    fn unpack_three_set_repr() {
        let unpacked = unpack_rlp(&[0xc7, 0xc0, 0xc1, 0xc0, 0xc3, 0xc0, 0xc1, 0xc0][..]).unwrap();
        assert_eq!(
            unpacked.0,
            vec![RecursiveBytes::Nested(
                vec![
                    RecursiveBytes::empty_list(),
                    RecursiveBytes::Nested(vec![RecursiveBytes::empty_list()].into()),
                    RecursiveBytes::Nested(
                        vec![
                            RecursiveBytes::empty_list(),
                            RecursiveBytes::Nested(vec![RecursiveBytes::empty_list()].into()),
                        ]
                        .into()
                    )
                ]
                .into()
            )]
        );
    }

    #[test]
    fn unpack_lorem_ipsum() {
        let unpacked = unpack_rlp(
            &[
                0xb8, 0x38, b'L', b'o', b'r', b'e', b'm', b' ', b'i', b'p', b's', b'u', b'm', b' ',
                b'd', b'o', b'l', b'o', b'r', b' ', b's', b'i', b't', b' ', b'a', b'm', b'e', b' ',
                b't', b' ', b'c', b'o', b'n', b's', b'e', b'c', b't', b'e', b't', b'u', b'r', b' ',
                b'a', b'd', b'i', b'p', b'i', b's', b'i', b'c', b'i', b'n', b'g', b' ', b'e', b'l',
                b'i', b't',
            ][..],
        )
        .unwrap();
        assert_eq!(
            unpacked.0,
            vec![RecursiveBytes::Bytes(vec![
                b'L', b'o', b'r', b'e', b'm', b' ', b'i', b'p', b's', b'u', b'm', b' ', b'd', b'o',
                b'l', b'o', b'r', b' ', b's', b'i', b't', b' ', b'a', b'm', b'e', b' ', b't', b' ',
                b'c', b'o', b'n', b's', b'e', b'c', b't', b'e', b't', b'u', b'r', b' ', b'a', b'd',
                b'i', b'p', b'i', b's', b'i', b'c', b'i', b'n', b'g', b' ', b'e', b'l', b'i', b't',
            ])]
        );
    }
}

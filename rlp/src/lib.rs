use std::collections::VecDeque;
use std::fmt::{self, Display};

mod de;
pub use de::from_bytes;

mod ser;
pub use ser::to_bytes;

#[derive(Debug)]
pub enum RlpError {
    MissingBytes,
    TrailingBytes,
    ExpectedBytes,
    ExpectedList,
    InvalidBytes,
    InvalidLength,
    Message(String),
}

impl serde::ser::Error for RlpError {
    fn custom<T: Display>(msg: T) -> Self {
        RlpError::Message(msg.to_string())
    }
}

impl serde::de::Error for RlpError {
    fn custom<T: Display>(msg: T) -> Self {
        RlpError::Message(msg.to_string())
    }
}

impl Display for RlpError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RlpError::Message(message) => formatter.write_str(message),
            RlpError::MissingBytes => formatter.write_str("missing bytes after discriminant byte"),
            RlpError::ExpectedList => formatter.write_str("expected list, got bytes"),
            RlpError::ExpectedBytes => formatter.write_str("expected bytes, got list"),
            RlpError::InvalidBytes => formatter.write_str("invalid bytes"),
            RlpError::InvalidLength => formatter.write_str("invalid length"),
            RlpError::TrailingBytes => formatter.write_str("trailing bytes"),
        }
    }
}

impl std::error::Error for RlpError {}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub enum RecursiveBytes {
    /// Bytes (string)
    Bytes(Vec<u8>),
    /// An empty list that should serialize to [0x80]
    EmptyList,
    /// A nested data structure to represent arbitrarily arbitrarily nested arrays (list)
    Nested(Vec<RecursiveBytes>),
}

impl RecursiveBytes {
    #[cfg(test)]
    fn empty_list() -> Self {
        RecursiveBytes::Nested(Vec::new())
    }

    pub fn into_rlp(self) -> Rlp {
        Rlp::new_unary(self)
    }
}

#[derive(Debug, Default, Clone)]
pub struct Rlp(VecDeque<RecursiveBytes>);

impl IntoIterator for Rlp {
    type Item = Rlp;

    type IntoIter = RlpIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        RlpIntoIter(self.0.into_iter())
    }
}

pub struct RlpIntoIter(std::collections::vec_deque::IntoIter<RecursiveBytes>);

impl Iterator for RlpIntoIter {
    type Item = Rlp;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|el| {
                let mut deque = VecDeque::new();
                deque.push_back(el);
                deque
            })
            .map(Rlp)
    }
}

impl Rlp {
    pub fn new(inner: VecDeque<RecursiveBytes>) -> Self {
        Rlp(inner)
    }

    pub fn new_unary(inner: RecursiveBytes) -> Self {
        Rlp(vec![inner].into())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<&RecursiveBytes> {
        self.0.get(index)
    }

    pub fn pop_front(&mut self) -> Option<RecursiveBytes> {
        self.0.pop_front()
    }

    pub fn flatten_nested(&mut self) -> Option<Self> {
        if self.0.len() != 1 {
            return None;
        }

        let nested = self.need_nested().ok()?;
        Some(Rlp::new(nested.into()))
    }

    pub fn get_nested(&self, index: usize) -> Result<&[RecursiveBytes], RlpError> {
        let RecursiveBytes::Nested(recs) = self.0.get(index).ok_or(RlpError::MissingBytes)? else {
            return Err(RlpError::ExpectedList);
        };
        Ok(recs)
    }
}

// run a BFS to unpack the rlp
fn recursive_unpack_rlp(bytes: &[u8], mut cursor: usize) -> Result<Vec<RecursiveBytes>, RlpError> {
    let disc = if let Some(disc) = bytes.get(cursor) {
        *disc
    } else {
        return Ok(Vec::new());
    };
    cursor += 1;

    let mut unpacked = Vec::new();

    let ret = if disc <= 0x7f {
        let ret = bytes.get((cursor - 1)..cursor).unwrap();

        RecursiveBytes::Bytes(ret.to_vec())
    } else if disc <= 0xb7 {
        let len = disc - 0x80;
        let ret = bytes
            .get(cursor..(cursor + len as usize))
            .ok_or(RlpError::MissingBytes)?;

        if len == 1 && ret[0] <= 127 {
            return Err(RlpError::InvalidBytes);
        }

        cursor += len as usize;

        RecursiveBytes::Bytes(ret.to_vec())
    } else if disc <= 0xbf {
        let len_bytes_len = disc - 0xb7;
        if len_bytes_len > 8 {
            // unimplemented!("we do not support > 2**64 bytes long strings");
            return Err(RlpError::InvalidLength);
        }
        let mut len_bytes_base = [0; 8];
        let len_bytes = bytes
            .get(cursor..(cursor + len_bytes_len as usize))
            .ok_or(RlpError::MissingBytes)?;
        cursor += len_bytes_len as usize;

        len_bytes_base[(8 - len_bytes.len())..].copy_from_slice(len_bytes);
        let len = usize::from_be_bytes(len_bytes_base);
        if len <= 55 {
            return Err(RlpError::InvalidLength);
        }
        let max_cursor = cursor.checked_add(len).ok_or(RlpError::InvalidLength)?;
        let ret = bytes
            .get(cursor..max_cursor)
            .ok_or(RlpError::MissingBytes)?;
        cursor += len;

        RecursiveBytes::Bytes(ret.to_vec())
    } else if disc <= 0xf7 {
        let len = disc - 0xc0;
        let list_bytes = bytes
            .get(cursor..(cursor + len as usize))
            .ok_or(RlpError::MissingBytes)?;
        cursor += len as usize;

        // we want to represent empty lists so don't remove them
        RecursiveBytes::Nested(recursive_unpack_rlp(list_bytes, 0)?)
    } else {
        let len_bytes_len = disc - 0xf7;
        let mut len_bytes_base = [0; 8];
        let len_bytes = bytes
            .get(cursor..(cursor + len_bytes_len as usize))
            .ok_or(RlpError::MissingBytes)?;

        if len_bytes[0] == 0 {
            return Err(RlpError::TrailingBytes);
        }

        cursor += len_bytes_len as usize;
        len_bytes_base[(8 - len_bytes.len())..].copy_from_slice(len_bytes);

        let len = usize::from_be_bytes(len_bytes_base);
        if len < 55 {
            return Err(RlpError::InvalidLength);
        }

        let max_cursor = cursor.checked_add(len).ok_or(RlpError::InvalidLength)?; // TODO wrong error
        let list_bytes = bytes
            .get(cursor..max_cursor)
            .ok_or(RlpError::MissingBytes)?;
        cursor += len;

        RecursiveBytes::Nested(recursive_unpack_rlp(list_bytes, 0)?)
    };

    unpacked.push(ret);
    unpacked.append(&mut recursive_unpack_rlp(bytes, cursor)?);

    Ok(unpacked)
}

pub fn unpack_rlp(bytes: &[u8]) -> Result<Rlp, RlpError> {
    Ok(Rlp::new(recursive_unpack_rlp(bytes, 0)?.into()))
}

fn parse_num<const N: usize>(bytes: [u8; N]) -> Option<Vec<u8>> {
    bytes
        .iter()
        .position(|b| b > &0)
        .map(|index| bytes[index..].to_vec())
}

fn append_rlp_bytes(pack: &mut Vec<u8>, new_bytes: Vec<u8>) -> Result<usize, RlpError> {
    let mut bytes = match new_bytes.len() {
        1 if new_bytes[0] <= 127 => new_bytes,
        len => {
            if len <= 55 {
                if len == 1 && new_bytes[0] <= 127 {
                    return Err(RlpError::InvalidBytes);
                }
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
                return Err(RlpError::InvalidLength);
            }
        }
    };

    let len = bytes.len();

    pack.append(&mut bytes);

    Ok(len)
}

fn serialize_list_len(len: usize) -> Result<Vec<u8>, RlpError> {
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

fn recursive_pack_rlp(rec: RecursiveBytes, pack: &mut Vec<u8>) -> Result<usize, RlpError> {
    match rec {
        RecursiveBytes::Bytes(bytes) => append_rlp_bytes(pack, bytes),
        RecursiveBytes::EmptyList => {
            pack.push(0x80);
            Ok(1)
        }
        RecursiveBytes::Nested(recs) => {
            let mut len = 0;
            let inner_pack = &mut Vec::new();
            for rec in recs {
                len += recursive_pack_rlp(rec, inner_pack)?;
            }
            let len_bytes = &mut serialize_list_len(len)?;
            len += len_bytes.len();
            pack.append(len_bytes);
            pack.append(inner_pack);
            Ok(len)
        }
    }
}

pub fn pack_rlp(mut rlp: Rlp) -> Result<Vec<u8>, RlpError> {
    let mut pack = Vec::new();
    while let Some(rec) = rlp.pop_front() {
        recursive_pack_rlp(rec, &mut pack)?;
    }
    Ok(pack)
}

// https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/#examples
#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

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
            vec![RecursiveBytes::Nested(vec![
                RecursiveBytes::Bytes(vec![b'd', b'o', b'g']),
                RecursiveBytes::Bytes(vec![b'c', b'a', b't']),
            ])]
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
        assert_eq!(unpacked.0, vec![RecursiveBytes::Nested(Vec::new())]);
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
            vec![RecursiveBytes::Nested(vec![
                RecursiveBytes::empty_list(),
                RecursiveBytes::Nested(vec![RecursiveBytes::empty_list()]),
                RecursiveBytes::Nested(vec![
                    RecursiveBytes::empty_list(),
                    RecursiveBytes::Nested(vec![RecursiveBytes::empty_list()]),
                ])
            ])]
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

    #[test]
    fn trailing_bytes_unpack() {
        let tests = [&[93, 61, 73, 95, 61, 61, 248, 0][..]];

        for (i, bytes) in tests.into_iter().enumerate() {
            println!("{i}...");

            assert!(matches!(
                unpack_rlp(bytes).unwrap_err(),
                RlpError::TrailingBytes
            ));

            println!("ok");
        }
    }

    #[test]
    fn too_short() {
        #[rustfmt::skip]
        let tests = [
            &[5, 248, 5, 5, 29, 38, 5, 5, 128, 128, 5, 73, 128, 128, 5, 44, 73][..],
            &[192, 192, 192, 192, 192, 184, 5, 59, 59, 59, 59, 93, 77, 77, 77, 
            77, 77, 77, 77, 77, 192, 128, 59, 195, 192, 91, 5, 192, 91, 59, 59, 5][..],
        ];

        for (i, bytes) in tests.into_iter().enumerate() {
            println!("{i}...");

            assert!(matches!(
                unpack_rlp(bytes).unwrap_err(),
                RlpError::InvalidLength
            ));

            println!("ok");
        }
    }

    #[test]
    fn nested_empty_array() {
        let bytes = [201, 59, 59, 59, 59, 0, 0, 128, 59, 59];
        let rlp = unpack_rlp(&bytes).unwrap();

        assert_eq!(
            rlp.0,
            vec![RecursiveBytes::Nested(vec![
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![0]),
                RecursiveBytes::Bytes(vec![0]),
                RecursiveBytes::Bytes(vec![]),
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![59]),
            ])]
        );
    }

    #[test]
    fn array_with_trailing() {
        let bytes = [205, 128, 59, 128, 59, 132, 0, 59, 59, 201, 128, 59, 59, 128];
        let rlp = unpack_rlp(&bytes).unwrap();

        assert_eq!(
            rlp.0,
            vec![RecursiveBytes::Nested(vec![
                RecursiveBytes::Bytes(vec![]),
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![]),
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![0, 59, 59, 201]),
                RecursiveBytes::Bytes(vec![]),
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![59]),
                RecursiveBytes::Bytes(vec![]),
            ])]
        );
    }

    #[test]
    fn pack_unpack() {
        let tests = [
            (
                &[201, 69, 59, 59, 59, 0, 59, 59, 59, 10][..],
                vec![RecursiveBytes::Nested(vec![
                    RecursiveBytes::Bytes(vec![69]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![0]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![10]),
                ])],
            ),
            (
                &[201, 128, 59, 59, 59, 59, 59, 59, 59, 59][..],
                vec![RecursiveBytes::Nested(vec![
                    RecursiveBytes::Bytes(vec![]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                    RecursiveBytes::Bytes(vec![59]),
                ])],
            ),
        ];

        for (i, (bytes, rlp)) in tests.into_iter().enumerate() {
            println!("{i}...");

            let unpacked = unpack_rlp(bytes).unwrap();

            assert_eq!(unpacked.0, rlp);

            let packed = pack_rlp(unpacked).unwrap();

            assert_eq!(bytes, packed.as_slice());

            println!("ok");
        }
    }

    #[test]
    fn trailing_bytes_deserialize() {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct MyType([u8; 9]);

        let tests = [&[201, 69, 59, 59, 59, 0, 59, 59, 59, 10][..]];

        for (i, bytes) in tests.into_iter().enumerate() {
            println!("{i}...");

            let rlp = &mut unpack_rlp(bytes).unwrap();

            assert!(matches!(
                MyType::deserialize(rlp).unwrap_err(),
                RlpError::TrailingBytes
            ));
        }
    }
}

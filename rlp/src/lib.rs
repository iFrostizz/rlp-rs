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
pub(crate) enum RecursiveBytes {
    Bytes(Vec<u8>),
    Nested(Vec<RecursiveBytes>),
}

#[cfg(test)]
impl RecursiveBytes {
    fn empty_list() -> Self {
        RecursiveBytes::Nested(Vec::new())
    }
}

#[derive(Debug, Default)]
pub(crate) struct Rlp(VecDeque<RecursiveBytes>);

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
fn recursive_unpack_rlp(bytes: &[u8], mut cursor: usize) -> Result<Vec<RecursiveBytes>, RlpError> {
    let disc = if let Some(disc) = bytes.get(cursor) {
        *disc
    } else {
        return Ok(Vec::new());
    };
    cursor += 1;
    println!("{:?}", &bytes);

    let mut unpacked = Vec::new();

    let ret = if disc <= 0x7f {
        let ret = bytes.get((cursor - 1)..cursor).unwrap();

        RecursiveBytes::Bytes(ret.to_vec())
    } else if disc <= 0xb7 {
        let len = disc - 0x80;
        let ret = bytes
            .get(cursor..(cursor + len as usize))
            .ok_or(RlpError::MissingBytes)?;
        cursor += len as usize;

        RecursiveBytes::Bytes(ret.to_vec())
    } else if disc <= 0xbf {
        let len_bytes_len = disc - 0xb7;
        if len_bytes_len > 8 {
            unimplemented!("we do not support > 2**64 bytes long strings");
        }
        let mut len_bytes_base = [0; 8];
        let len_bytes = bytes
            .get(cursor..(cursor + len_bytes_len as usize))
            .ok_or(RlpError::MissingBytes)?;
        cursor += len_bytes_len as usize;

        len_bytes_base[(8 - len_bytes.len())..].copy_from_slice(len_bytes);
        let len = usize::from_be_bytes(len_bytes_base);
        let ret = bytes
            .get(cursor..(cursor + len))
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
        let len_bytes_len = disc - 0xf8;
        let mut len_bytes_base = [0; 8];
        let len_bytes = bytes
            .get(cursor..(cursor + len_bytes_len as usize))
            .ok_or(RlpError::MissingBytes)?;
        cursor += len_bytes_len as usize;
        len_bytes_base[(8 - len_bytes.len())..].copy_from_slice(len_bytes);
        let len = usize::from_be_bytes(len_bytes_base);
        let list_bytes = bytes
            .get(cursor..(cursor + len))
            .ok_or(RlpError::MissingBytes)?;
        cursor += len;

        RecursiveBytes::Nested(recursive_unpack_rlp(list_bytes, 0)?)
    };

    println!("{:?}", &ret);
    unpacked.push(ret);
    unpacked.append(&mut recursive_unpack_rlp(bytes, cursor)?);

    Ok(unpacked)
}

pub(crate) fn unpack_rlp(bytes: &[u8]) -> Result<Rlp, RlpError> {
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
        let mut bytes = vec![0xf8 + len_bytes.len() as u8];
        bytes.append(&mut len_bytes);
        bytes
    };

    Ok(bytes)
}

fn recursive_pack_rlp(rec: RecursiveBytes, pack: &mut Vec<u8>) -> Result<usize, RlpError> {
    match rec {
        RecursiveBytes::Bytes(bytes) => append_rlp_bytes(pack, bytes),
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

pub(crate) fn pack_rlp(mut rlp: Rlp) -> Result<Vec<u8>, RlpError> {
    let mut pack = Vec::new();
    while let Some(rec) = rlp.pop_front() {
        recursive_pack_rlp(rec, &mut pack)?;
    }
    Ok(pack)
}

// https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/#examples
#[cfg(test)]
mod tests {
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
}

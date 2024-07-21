#[derive(Debug)]
pub enum DecodeError {
    MissingBytes,
    TrailingBytes,
}

#[derive(Debug, PartialEq)]
pub enum RecursiveBytes<'a> {
    Bytes(&'a [u8]),
    Recursive(Vec<RecursiveBytes<'a>>),
}

impl RecursiveBytes<'_> {
    fn empty_list() -> Self {
        RecursiveBytes::Recursive(Vec::new())
    }
}

// run a BFS to unpack the rlp
fn recursive_unpack_rlp(
    bytes: &[u8],
    mut cursor: usize,
) -> Result<Vec<RecursiveBytes>, DecodeError> {
    let disc = if let Some(disc) = bytes.get(cursor) {
        *disc
    } else {
        return Ok(Vec::new());
    };
    cursor += 1;

    let mut ret = if disc <= 127 {
        // TODO change me, maybe remove vec
        let ret = bytes.get((cursor - 1)..cursor).unwrap();

        vec![RecursiveBytes::Bytes(ret)]
    } else if disc <= 183 {
        let len = disc - 128;
        if len == 0 {
            vec![] // this is just a little space optimisation to avoid Bytes([])
        } else {
            let ret = bytes
                .get(cursor..(cursor + len as usize))
                .ok_or(DecodeError::MissingBytes)?;
            cursor += len as usize;

            vec![RecursiveBytes::Bytes(ret)]
        }
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

        vec![RecursiveBytes::Bytes(ret)]
    } else if disc <= 247 {
        let len = disc - 192;
        let list_bytes = bytes
            .get(cursor..(cursor + len as usize))
            .ok_or(DecodeError::MissingBytes)?;
        cursor += len as usize;

        // we want to represent empty lists so don't remove them
        vec![RecursiveBytes::Recursive(recursive_unpack_rlp(
            list_bytes, 0,
        )?)]
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

        vec![RecursiveBytes::Recursive(recursive_unpack_rlp(
            list_bytes, 0,
        )?)]
    };

    ret.append(&mut recursive_unpack_rlp(bytes, cursor)?);

    Ok(ret)
}

pub fn unpack_rlp(bytes: &[u8]) -> Result<Vec<RecursiveBytes>, DecodeError> {
    recursive_unpack_rlp(bytes, 0)
}

// https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/#examples
#[cfg(test)]
mod tests {
    use super::{unpack_rlp, RecursiveBytes};

    #[test]
    fn unpack_dog() {
        let dog_bites = &mut "dog".as_bytes().to_vec();
        let mut dog_rlp = vec![0x83];
        dog_rlp.append(dog_bites);

        let unpacked = unpack_rlp(&dog_rlp).unwrap();
        assert_eq!(
            unpacked,
            vec![RecursiveBytes::Bytes(&[b'd', b'o', b'g'][..])]
        );
    }

    #[test]
    fn unpack_cat_dog_list() {
        let dog_bites = &mut "dog".as_bytes().to_vec();
        let mut dog_rlp = vec![0x83];
        dog_rlp.append(dog_bites);

        let cat_bites = &mut "cat".as_bytes().to_vec();
        let mut cat_rlp = vec![0x83];
        cat_rlp.append(cat_bites);

        let mut cat_dog_rlp = vec![0xc8];
        cat_dog_rlp.append(&mut dog_rlp);
        cat_dog_rlp.append(&mut cat_rlp);

        let unpacked = unpack_rlp(&cat_dog_rlp).unwrap();
        assert_eq!(
            unpacked,
            vec![RecursiveBytes::Recursive(vec![
                RecursiveBytes::Bytes(&[b'd', b'o', b'g'][..]),
                RecursiveBytes::Bytes(&[b'c', b'a', b't'][..]),
            ])]
        );
    }

    #[test]
    fn unpack_empty_string() {
        let unpacked = unpack_rlp(&[0x80][..]).unwrap();
        assert!(unpacked.is_empty());
    }

    #[test]
    fn unpack_empty_list() {
        let unpacked = unpack_rlp(&[0xc0][..]).unwrap();
        assert_eq!(unpacked, vec![RecursiveBytes::Recursive(Vec::new())]);
    }

    #[test]
    #[ignore = "there is no way to decode the number 0, the priority is given to the empty string"]
    fn unpack_zero() {
        let unpacked = unpack_rlp(&[0x80][..]).unwrap();
        assert_eq!(unpacked, vec![RecursiveBytes::Bytes(&[0][..])]);
    }

    #[test]
    fn unpack_null_byte() {
        let unpacked = unpack_rlp(&[0x00][..]).unwrap();
        assert_eq!(unpacked, vec![RecursiveBytes::Bytes(&[0][..])]);
    }

    #[test]
    fn unpack_0f() {
        let unpacked = unpack_rlp(&[0x0f][..]).unwrap();
        assert_eq!(unpacked, vec![RecursiveBytes::Bytes(&[0x0f][..])]);
    }

    #[test]
    fn unpack_two_bytes() {
        let unpacked = unpack_rlp(&[0x82, 0x04, 0x00][..]).unwrap();
        assert_eq!(unpacked, vec![RecursiveBytes::Bytes(&[0x04, 0x00][..])]);
    }

    #[test]
    fn unpack_three_set_repr() {
        let unpacked = unpack_rlp(&[0xc7, 0xc0, 0xc1, 0xc0, 0xc3, 0xc0, 0xc1, 0xc0][..]).unwrap();
        assert_eq!(
            unpacked,
            vec![RecursiveBytes::Recursive(vec![
                RecursiveBytes::empty_list(),
                RecursiveBytes::Recursive(vec![RecursiveBytes::empty_list()]),
                RecursiveBytes::Recursive(vec![
                    RecursiveBytes::empty_list(),
                    RecursiveBytes::Recursive(vec![RecursiveBytes::empty_list()]),
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
            unpacked,
            vec![RecursiveBytes::Bytes(
                &[
                    b'L', b'o', b'r', b'e', b'm', b' ', b'i', b'p', b's', b'u', b'm', b' ', b'd',
                    b'o', b'l', b'o', b'r', b' ', b's', b'i', b't', b' ', b'a', b'm', b'e', b' ',
                    b't', b' ', b'c', b'o', b'n', b's', b'e', b'c', b't', b'e', b't', b'u', b'r',
                    b' ', b'a', b'd', b'i', b'p', b'i', b's', b'i', b'c', b'i', b'n', b'g', b' ',
                    b'e', b'l', b'i', b't',
                ][..]
            )]
        );
    }
}

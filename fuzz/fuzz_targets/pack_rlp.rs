#![no_main]

use libfuzzer_sys::{fuzz_target, Corpus};
use rlp_rs::{pack_rlp, unpack_rlp};

fuzz_target!(|bytes: Vec<u8>| -> Corpus {
    let rlp = match unpack_rlp(&bytes) {
        Ok(rlp) => rlp,
        Err(_) => return Corpus::Reject,
    };
    let packed = pack_rlp(rlp).unwrap();
    assert_eq!(bytes, packed);
    Corpus::Keep
});

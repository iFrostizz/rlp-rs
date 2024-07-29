#![no_main]

use libfuzzer_sys::{fuzz_target, Corpus};
use rlp_types::TransactionEnvelope;

fuzz_target!(|tx_bytes: Vec<u8>| -> Corpus {
    let tx = match TransactionEnvelope::from_bytes(&tx_bytes) {
        Ok(tx) => tx,
        Err(_) => return Corpus::Reject,
    };
    let serialized = tx.as_bytes().unwrap();
    assert_eq!(tx_bytes, serialized);
    Corpus::Keep
});

#![no_main]

use libfuzzer_sys::fuzz_target;
use rlp_types::TransactionEnvelope;

fuzz_target!(|tx: TransactionEnvelope| {
    let bytes = rlp_rs::to_bytes(&tx).unwrap();
    let decoded_tx = TransactionEnvelope::from_bytes(&bytes).unwrap();
    assert_eq!(tx, decoded_tx);
});

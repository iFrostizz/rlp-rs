#![no_main]

use libfuzzer_sys::fuzz_target;
use rlp_types::TransactionEnvelope;

fuzz_target!(|tx: TransactionEnvelope| {
    let _ = tx.hash();
});

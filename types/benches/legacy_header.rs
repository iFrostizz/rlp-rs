use criterion::{criterion_group, criterion_main, Criterion};
use rlp_rs::{from_bytes, to_bytes};
use rlp_types::{Address, Bloom, CommonHeader, Nonce, U256};

pub fn criterion_benchmark(c: &mut Criterion) {
    let header = CommonHeader {
        parent_hash: U256::default(),
        uncle_hash: U256::default(),
        coinbase: Address::default(),
        state_root: U256::default(),
        tx_root: U256::default(),
        receipt_hash: U256::default(),
        bloom: Bloom::default(),
        difficulty: 10_000_000_000u64.to_be_bytes().to_vec().try_into().unwrap(),
        number: 1000u16.to_be_bytes().to_vec().try_into().unwrap(),
        gas_limit: 8_000_000,
        gas_used: 8_000_000,
        time: 555,
        extra: vec![0; 32],
        mix_digest: U256::default(),
        nonce: Nonce::default(),
    };
    let bytes = to_bytes(&header).unwrap();

    c.bench_function("100 legacy block headers", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let _: CommonHeader = from_bytes(&bytes).unwrap();
            }
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
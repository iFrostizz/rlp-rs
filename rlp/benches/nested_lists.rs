use criterion::{criterion_group, criterion_main, Criterion};
use rlp_rs::to_bytes;

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut nested_list = Vec::new();
    for _ in 0..20 {
        let mut el1 = Vec::new();
        for _ in 0..20 {
            let mut el2 = Vec::new();
            for _ in 0..20 {
                let mut el3 = Vec::new();
                for _ in 0..20 {
                    el3.push(());
                }
                el2.push(el3);
            }
            el1.push(el2);
        }
        nested_list.push(el1);
    }

    c.bench_function("nested list serialization", |b| {
        b.iter(|| {
            let _ = to_bytes(&nested_list).unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

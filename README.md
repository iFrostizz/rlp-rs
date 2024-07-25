# rlp-rs

RLP Encode / Decode arbitrary bytes

## Features

- Serde integration

## Limitations

- Unimplemented on:
    - Map
    - Option
- Rust doesn't support specialization yet. For this reason, use `serde_bytes` when annotating data that needs to be interpreter as bytes.
For an example, look at [Transaction](types/src/transaction.rs). Reading material: https://serde.rs/impl-serialize.html#other-special-cases

## TODO

- [x] Finish implementing all types
- [ ] Reorder functions around
- [ ] Benches
- [ ] Provide RLP-ready useful types 
    - [x] Add a crate `types`
    - [x] `Transaction`
    - [ ] `Block`
    - [ ] `Receipt`
    - [ ] Provide conversions from popular lib types from this crate

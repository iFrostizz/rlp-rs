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
Additionally, `serde_bytes` had to be forked because there is no hard requirement on the size of
bytes in the RLP representation. For instance, all zeros are removed to save some space.
That means that `serde_bytes` has to be a bit more forgiving regarding arrays,
it has been patched to prepend these `0` manually when deserializing to an array.
- Because of the caveats of some Ethereum structure:
    - No representation of Option, although the block may have additional fields
    - Transaction envelope contains a prefix byte for all transactions besides the Legacy one

    The deserialization sometimes has to be implemented manually.
    Some rudimentary parsing of the "raw rlp" which is represented by the `Rlp` struct is provided. 
    You can see the implementation of `Block::from_bytes`.

## TODO

- [x] Finish implementing serde
- [x] Tests for trailing bytes
- [ ] Better serde error handling
- [ ] Reorder functions around
- [ ] Better API with nice parsing functions
- [ ] Benches, check if we can beat geth and fastrlp https://github.com/umbracle/fastrlp?tab=readme-ov-file#benchmark
- [ ] Fuzz ser/de for corectness
- [ ] Provide RLP-ready useful types 
    - [x] Add a crate `types`
    - [x] `Transaction`
    - [ ] `Block`
    - [ ] `Receipt`
    - [ ] Provide conversions from popular lib types from this crate

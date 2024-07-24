# rlp-rs

RLP Encode / Decode arbitrary bytes

## Features

- Serde integration

## Limitations

- Unimplemented on:
    - Map
    - Option

## TODO

- Finish implementing all types
- Benches
- Provide RLP-ready useful types 
    - Add a crate `types`
    - `Transaction` (custom `Deserialize` for type 0, 1, 2)
    - `Block`
    - `Receipt`
    - Provide conversions from popular lib types from this crate

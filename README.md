# rlp-rs

RLP Encode / Decode arbitrary bytes

## Features

- Serde integration

## Limitations

- Unimplemented on:
    - Map
    - Option

## TODO

- Finish implementing all deserialize types
- Implement ser
- Benches
- Provide RLP-ready useful types 
    - `Transaction` (custom `Deserialize` for type 0, 1, 2)
    - `Block`

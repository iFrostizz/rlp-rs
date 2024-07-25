use crate::primitives::{Address, SerdeU256, U256};
use rlp_rs::DecodeError;
use serde::ser::{SerializeSeq, SerializeStructVariant, SerializeTuple};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteArray;

#[derive(Debug)]
#[non_exhaustive]
pub enum TransactionEnvelope {
    Legacy(TransactionLegacy),
    AccessList(TransactionAccessList),
    DynamicFee(TransactionDynamicFee),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionLegacy {
    nonce: u64,
    #[serde(with = "serde_bytes")]
    gas_price: U256,
    gas_limit: u64,
    #[serde(with = "serde_bytes")]
    to: Address,
    #[serde(with = "serde_bytes")]
    value: U256,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
    #[serde(with = "serde_bytes")]
    v: U256,
    #[serde(with = "serde_bytes")]
    r: U256,
    #[serde(with = "serde_bytes")]
    s: U256,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TransactionAccessList {
    #[serde(with = "serde_bytes")]
    chain_id: U256,
    nonce: u64,
    #[serde(with = "serde_bytes")]
    gas_price: U256,
    gas_limit: u64,
    #[serde(with = "serde_bytes")]
    to: Address,
    #[serde(with = "serde_bytes")]
    value: U256,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
    access_list: Vec<AccessList>,
    #[serde(with = "serde_bytes")]
    y_parity: U256,
    #[serde(with = "serde_bytes")]
    r: U256,
    #[serde(with = "serde_bytes")]
    s: U256,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TransactionDynamicFee {
    #[serde(with = "serde_bytes")]
    chain_id: U256,
    nonce: u64,
    #[serde(with = "serde_bytes")]
    max_priority_fee_per_gas: U256,
    #[serde(with = "serde_bytes")]
    max_fee_per_gas: U256,
    gas_limit: u64,
    #[serde(with = "serde_bytes")]
    destination: Address,
    #[serde(with = "serde_bytes")]
    amount: U256,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
    access_list: Vec<AccessList>,
    #[serde(with = "serde_bytes")]
    y_parity: U256,
    #[serde(with = "serde_bytes")]
    r: U256,
    #[serde(with = "serde_bytes")]
    s: U256,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessList {
    #[serde(with = "serde_bytes")]
    address: Address,
    // serde_bytes wouldn't figure out this, so use a wrapper type that implements
    // Serialize and Deserialize and that is annotated with serde_bytes
    storage_keys: Vec<SerdeU256>,
}

impl TransactionEnvelope {
    pub fn tx_type(&self) -> u8 {
        match self {
            TransactionEnvelope::Legacy { .. } => 0,
            TransactionEnvelope::AccessList { .. } => 1,
            TransactionEnvelope::DynamicFee { .. } => 2,
        }
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>, DecodeError> {
        let tx_type = self.tx_type();
        let mut bytes = if tx_type > 0 { vec![tx_type] } else { vec![] };
        let tx_rlp = &mut match self {
            TransactionEnvelope::Legacy(tx) => rlp_rs::to_bytes(tx),
            TransactionEnvelope::AccessList(tx) => rlp_rs::to_bytes(tx),
            TransactionEnvelope::DynamicFee(tx) => rlp_rs::to_bytes(tx),
        }?;
        bytes.append(tx_rlp);
        Ok(bytes)
    }

    pub fn legacy() -> Self {
        todo!()
    }

    pub fn dynamic_fee() -> Self {
        todo!()
    }

    pub fn access_list() -> Self {
        todo!()
    }
}

// moved to to_rlp because there is no way to concatenate stuff with serde
// impl Serialize for TransactionEnvelope {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         let tx_type = self.tx_type();
//         let mut serializer = if tx_type > 0 {
//             if tx_type > 127 {
//                 return Err(serde::ser::Error::custom("invalid tx type"));
//             }

//             let mut serializer = serializer.serialize_tuple(2)?;
//             let tx_type = ByteArray::new([tx_type]);
//             serializer.serialize_element(&tx_type)?;
//             serializer
//         } else {
//             serializer.serialize_tuple(1)?
//         };

//         match self {
//             TransactionEnvelope::Legacy(legacy) => serializer.serialize_element(legacy)?,
//             TransactionEnvelope::AccessList(access_list) => {
//                 serializer.serialize_element(access_list)?
//             }
//             TransactionEnvelope::DynamicFee(dynamic_fee) => {
//                 serializer.serialize_element(dynamic_fee)?
//             }
//         }

//         serializer.end()
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tx_ser_legacy() {
        let tx = TransactionLegacy {
            nonce: u64::MAX,
            gas_price: [1; 32],
            gas_limit: u64::MAX,
            to: [1; 20],
            value: [1; 32],
            data: vec![],
            v: [1; 32],
            r: [1; 32],
            s: [1; 32],
        };

        let tx_rlp = rlp_rs::to_bytes(&tx).unwrap();

        let tx = TransactionEnvelope::Legacy(tx);
        let serialized = tx.as_bytes().unwrap();

        assert_eq!(serialized, tx_rlp);

        let size: usize = 8 + 32 + 8 + 20 + 32 + 0 + 32 * 3 + 9;
        assert!(size > 55);

        let size_bytes = size.to_be_bytes();
        let size_bytes = size_bytes
            .iter()
            .position(|b| b > &0)
            .map(|i| &size_bytes[i..])
            .unwrap();
        let mut bytes = vec![0xf8 + size_bytes.len() as u8];

        bytes.extend_from_slice(size_bytes);
        bytes.push(0x80 + 8);
        bytes.extend_from_slice(&[255; 8]);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]);
        bytes.push(0x80 + 8);
        bytes.extend_from_slice(&[255; 8]);
        bytes.push(0x80 + 20);
        bytes.extend_from_slice(&[1; 20]);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]);
        bytes.push(0x80);
        for _ in 0..3 {
            bytes.push(0x80 + 32);
            bytes.extend_from_slice(&[1; 32]);
        }
        assert_eq!(serialized, bytes);
    }

    #[test]
    fn tx_ser_access_list() {
        let tx = TransactionEnvelope::AccessList(TransactionAccessList {
            chain_id: [1; 32],
            nonce: u64::MAX,
            gas_price: [1; 32],
            gas_limit: u64::MAX,
            to: [1; 20],
            value: [1; 32],
            data: vec![],
            access_list: vec![],
            y_parity: [1; 32],
            r: [1; 32],
            s: [1; 32],
        });

        let serialized = tx.as_bytes().unwrap();

        let size: usize =
            1 + 32 + 1 + 8 + 1 + 32 + 1 + 8 + 1 + 20 + 1 + 32 + 1 + 0 + 1 + 0 + (1 + 32) * 3;
        assert!(size > 55);

        let size_bytes = size.to_be_bytes();
        let size_bytes = dbg!(size_bytes
            .iter()
            .position(|b| b > &0)
            .map(|i| &size_bytes[i..])
            .unwrap());

        let mut bytes = vec![0x01]; // tx_type
        bytes.push(0xf8 + size_bytes.len() as u8);
        bytes.extend_from_slice(size_bytes);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]); // chain id
        bytes.push(0x80 + 8);
        bytes.extend_from_slice(&[255; 8]); // nonce
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]); // gas price
        bytes.push(0x80 + 8);
        bytes.extend_from_slice(&[255; 8]); // gas limit
        bytes.push(0x80 + 20);
        bytes.extend_from_slice(&[1; 20]); // to
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]); // value
        bytes.push(0x80); // data
        bytes.push(0xc0); // access list
        for _ in 0..3 {
            // y_parity, r, s
            bytes.push(0x80 + 32);
            bytes.extend_from_slice(&[1; 32]);
        }
        assert_eq!(serialized, bytes);
    }

    #[test]
    fn tx_ser_dynamic_fees() {
        let tx = TransactionEnvelope::DynamicFee(TransactionDynamicFee {
            chain_id: [1; 32],
            nonce: u64::MAX,
            max_priority_fee_per_gas: [1; 32],
            max_fee_per_gas: [1; 32],
            gas_limit: u64::MAX,
            destination: [1; 20],
            amount: [1; 32],
            data: vec![],
            access_list: vec![],
            y_parity: [1; 32],
            r: [1; 32],
            s: [1; 32],
        });

        let serialized = tx.as_bytes().unwrap();

        let size: usize =
            1 + 32 + 1 + 8 + 1 + 32 + 1 + 32 + 1 + 8 + 1 + 20 + 1 + 32 + 1 + 1 + (1 + 32) * 3;
        assert!(size > 55);

        let size_bytes = size.to_be_bytes();
        let size_bytes = size_bytes
            .iter()
            .position(|b| b > &0)
            .map(|i| &size_bytes[i..])
            .unwrap();

        let mut bytes = vec![0x02];
        bytes.push(0xf8 + size_bytes.len() as u8);
        bytes.extend_from_slice(size_bytes);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]);
        bytes.push(0x80 + 8);
        bytes.extend_from_slice(&[255; 8]);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]);
        bytes.push(0x80 + 8);
        bytes.extend_from_slice(&[255; 8]);
        bytes.push(0x80 + 20);
        bytes.extend_from_slice(&[1; 20]);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[1; 32]);
        bytes.push(0x80);
        bytes.push(0xc0);
        for _ in 0..3 {
            bytes.push(0x80 + 32);
            bytes.extend_from_slice(&[1; 32]);
        }
        assert_eq!(serialized, bytes);
    }
}

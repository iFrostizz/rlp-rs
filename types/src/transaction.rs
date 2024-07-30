use crate::primitives::{Address, U256};
#[cfg(feature = "fuzzing")]
use libfuzzer_sys::arbitrary::{self, Arbitrary};
use rlp_rs::{pack_rlp, unpack_rlp, RecursiveBytes, Rlp, RlpError};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum TransactionEnvelope {
    Legacy(TransactionLegacy),
    AccessList(TransactionAccessList),
    DynamicFee(TransactionDynamicFee),
    // TODO Blob transaction
}

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionLegacy {
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub v: U256,
    pub r: U256,
    pub s: U256,
}

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TransactionAccessList {
    pub chain_id: U256,
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub access_list: Vec<AccessList>,
    pub y_parity: U256,
    pub r: U256,
    pub s: U256,
}

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TransactionDynamicFee {
    pub chain_id: U256,
    pub nonce: u64,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub gas_limit: u64,
    pub destination: Address,
    pub amount: U256,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub access_list: Vec<AccessList>,
    pub y_parity: U256,
    pub r: U256,
    pub s: U256,
}

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessList {
    pub address: Address,
    // serde_bytes wouldn't figure out this, so use a wrapper type that implements
    // Serialize and Deserialize and that is annotated with serde_bytes
    pub storage_keys: Vec<U256>,
}

impl TransactionEnvelope {
    pub fn tx_type(&self) -> u8 {
        match self {
            TransactionEnvelope::Legacy { .. } => 0,
            TransactionEnvelope::AccessList { .. } => 1,
            TransactionEnvelope::DynamicFee { .. } => 2,
        }
    }

    pub fn hash(&self) -> Result<[u8; 32], RlpError> {
        let mut hasher = Keccak256::new();
        let bytes = self.as_bytes()?;
        hasher.update(bytes);
        Ok(hasher.finalize().into())
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>, RlpError> {
        match self {
            TransactionEnvelope::Legacy(tx) => rlp_rs::to_bytes(tx),
            TransactionEnvelope::AccessList(tx) => {
                let mut bytes = vec![self.tx_type()];
                bytes.append(&mut rlp_rs::to_bytes(tx)?);
                let rlp = RecursiveBytes::Bytes(bytes).into_rlp();
                pack_rlp(rlp)
            }
            TransactionEnvelope::DynamicFee(tx) => {
                let mut bytes = vec![self.tx_type()];
                bytes.append(&mut rlp_rs::to_bytes(tx)?);
                let rlp = RecursiveBytes::Bytes(bytes).into_rlp();
                pack_rlp(rlp)
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RlpError> {
        let mut rlp = unpack_rlp(bytes)?;
        let res = Self::from_raw_rlp(&mut rlp)?;
        match rlp.is_empty() {
            true => Ok(res),
            false => Err(RlpError::InvalidLength),
        }
    }

    /// decode an rlp encoded transaction with an expected tx_type
    fn decode_transaction(rlp: &mut Rlp, tx_type: u8) -> Result<Self, RlpError> {
        let tx = match tx_type {
            0 => TransactionEnvelope::Legacy(TransactionLegacy::deserialize(rlp)?),
            1 => TransactionEnvelope::AccessList(TransactionAccessList::deserialize(rlp)?),
            2 => TransactionEnvelope::DynamicFee(TransactionDynamicFee::deserialize(rlp)?),
            _ => return Err(RlpError::InvalidBytes),
        };

        Ok(tx)
    }

    pub fn from_raw_rlp(rlp: &mut Rlp) -> Result<Self, RlpError> {
        // TODO this is only valid if rlp is length of 1
        let tx_type = match rlp.get(0) {
            Some(RecursiveBytes::Nested(_)) => 0,
            Some(RecursiveBytes::Bytes(bytes)) => {
                let tx_type = match bytes.first().ok_or(RlpError::MissingBytes)? {
                    1 => 1,
                    2 => 2,
                    _ => return Err(RlpError::InvalidBytes),
                };

                *rlp = unpack_rlp(&bytes[1..])?;

                tx_type
            }
            _ => return Err(RlpError::InvalidBytes),
        };

        Self::decode_transaction(rlp, tx_type)
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

#[cfg(test)]
mod tests {
    use rlp_rs::from_bytes;

    use super::*;

    #[test]
    fn tx_ser_legacy() {
        let tx = TransactionLegacy {
            nonce: u64::MAX,
            gas_price: [1; 32].into(),
            gas_limit: u64::MAX,
            to: [1; 20].into(),
            value: [1; 32].into(),
            data: vec![],
            v: [1; 32].into(),
            r: [1; 32].into(),
            s: [1; 32].into(),
        };

        let tx_rlp = rlp_rs::to_bytes(&tx).unwrap();

        let tx = TransactionEnvelope::Legacy(tx);
        let serialized = tx.as_bytes().unwrap();

        assert_eq!(serialized, tx_rlp);

        #[allow(clippy::identity_op)]
        let size: usize = 8 + 32 + 8 + 20 + 32 + 0 + 32 * 3 + 9;
        assert!(size > 55);

        let size_bytes = size.to_be_bytes();
        let size_bytes = size_bytes
            .iter()
            .position(|b| b > &0)
            .map(|i| &size_bytes[i..])
            .unwrap();
        let mut bytes = vec![0xf7 + size_bytes.len() as u8];

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

        assert_eq!(bytes, serialized);
    }

    #[test]
    fn tx_ser_access_list() {
        let tx = TransactionEnvelope::AccessList(TransactionAccessList {
            chain_id: [1; 32].into(),
            nonce: u64::MAX,
            gas_price: [1; 32].into(),
            gas_limit: u64::MAX,
            to: [1; 20].into(),
            value: [1; 32].into(),
            data: vec![],
            access_list: vec![],
            y_parity: [1; 32].into(),
            r: [1; 32].into(),
            s: [1; 32].into(),
        });

        let serialized = tx.as_bytes().unwrap();

        #[allow(clippy::identity_op)]
        let size: usize =
            1 + 32 + 1 + 8 + 1 + 32 + 1 + 8 + 1 + 20 + 1 + 32 + 1 + 0 + 1 + 0 + (1 + 32) * 3;
        assert!(size > 55);

        let size_bytes = size.to_be_bytes();
        let size_bytes = size_bytes
            .iter()
            .position(|b| b > &0)
            .map(|i| &size_bytes[i..])
            .unwrap();

        let mut bytes = vec![184, 242]; // tx_type
        bytes.push(0x01); // tx_type
        bytes.push(0xf7 + size_bytes.len() as u8);
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
        let tx = TransactionDynamicFee {
            chain_id: [1; 32].into(),
            nonce: u64::MAX,
            max_priority_fee_per_gas: [1; 32].into(),
            max_fee_per_gas: [1; 32].into(),
            gas_limit: u64::MAX,
            destination: [1; 20].into(),
            amount: [1; 32].into(),
            data: vec![],
            access_list: vec![],
            y_parity: [1; 32].into(),
            r: [1; 32].into(),
            s: [1; 32].into(),
        };
        let tx_envelope = TransactionEnvelope::DynamicFee(tx.clone());

        let mut serialized = tx_envelope.as_bytes().unwrap();

        let size: usize =
            1 + 32 + 1 + 8 + 1 + 32 + 1 + 32 + 1 + 8 + 1 + 20 + 1 + 32 + 1 + 1 + (1 + 32) * 3;
        assert!(size > 55);

        let size_bytes = size.to_be_bytes();
        let size_bytes = size_bytes
            .iter()
            .position(|b| b > &0)
            .map(|i| &size_bytes[i..])
            .unwrap();

        let mut bytes = vec![185, 1, 20];
        bytes.push(0x02);
        bytes.push(0xf7 + size_bytes.len() as u8);
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

        for _ in 0..4 {
            serialized.remove(0); // remove len prefix & tx_type
        }

        let deserialized: TransactionDynamicFee = from_bytes(&serialized).unwrap();
        assert_eq!(deserialized, tx);
    }
}

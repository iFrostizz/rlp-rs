use crate::primitives::{Address, SerdeU256, U256};
#[cfg(feature = "fuzzing")]
use libfuzzer_sys::arbitrary::{self, Arbitrary};
use rlp_rs::{pack_rlp, unpack_rlp, RecursiveBytes, Rlp, RlpError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug)]
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
    #[serde(with = "serde_bytes")]
    pub gas_price: U256,
    pub gas_limit: u64,
    #[serde(with = "serde_bytes")]
    pub to: Address,
    #[serde(with = "serde_bytes")]
    pub value: U256,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub v: U256,
    #[serde(with = "serde_bytes")]
    pub r: U256,
    #[serde(with = "serde_bytes")]
    pub s: U256,
}

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TransactionAccessList {
    #[serde(with = "serde_bytes")]
    pub chain_id: U256,
    pub nonce: u64,
    #[serde(with = "serde_bytes")]
    pub gas_price: U256,
    pub gas_limit: u64,
    #[serde(with = "serde_bytes")]
    pub to: Address,
    #[serde(with = "serde_bytes")]
    pub value: U256,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub access_list: Vec<AccessList>,
    #[serde(with = "serde_bytes")]
    pub y_parity: U256,
    #[serde(with = "serde_bytes")]
    pub r: U256,
    #[serde(with = "serde_bytes")]
    pub s: U256,
}

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TransactionDynamicFee {
    #[serde(with = "serde_bytes")]
    pub chain_id: U256,
    pub nonce: u64,
    #[serde(with = "serde_bytes")]
    pub max_priority_fee_per_gas: U256,
    #[serde(with = "serde_bytes")]
    pub max_fee_per_gas: U256,
    pub gas_limit: u64,
    #[serde(with = "serde_bytes")]
    pub destination: Address,
    #[serde(with = "serde_bytes")]
    pub amount: U256,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub access_list: Vec<AccessList>,
    #[serde(with = "serde_bytes")]
    pub y_parity: U256,
    #[serde(with = "serde_bytes")]
    pub r: U256,
    #[serde(with = "serde_bytes")]
    pub s: U256,
}

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessList {
    #[serde(with = "serde_bytes")]
    pub address: Address,
    // serde_bytes wouldn't figure out this, so use a wrapper type that implements
    // Serialize and Deserialize and that is annotated with serde_bytes
    pub storage_keys: Vec<SerdeU256>,
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
        let mut hasher = Sha256::new();
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
        let rlp = unpack_rlp(bytes)?;
        Self::from_raw_rlp(rlp)
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

    pub fn from_raw_rlp(mut rlp: Rlp) -> Result<Self, RlpError> {
        // TODO this is only valid if rlp is length of 1
        let tx_type = match rlp.get(0) {
            Some(RecursiveBytes::Nested(_)) => 0,
            Some(RecursiveBytes::Bytes(bytes)) => {
                let tx_type = match bytes.first().ok_or(RlpError::MissingBytes)? {
                    1 => 1,
                    2 => 2,
                    _ => return Err(RlpError::InvalidBytes),
                };

                rlp = unpack_rlp(&bytes[1..])?;

                tx_type
            }
            _ => return Err(RlpError::InvalidBytes),
        };

        Self::decode_transaction(&mut rlp, tx_type)
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

        let mut bytes = vec![0x01]; // tx_type
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

        let mut bytes = vec![0x02];
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

        serialized.remove(0); // remove tx_type

        let deserialized: TransactionDynamicFee = from_bytes(&serialized).unwrap();
        assert_eq!(deserialized, tx);
    }
}

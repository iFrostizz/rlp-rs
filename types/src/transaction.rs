use crate::primitives::{Address, U256};
#[cfg(feature = "fuzzing")]
use libfuzzer_sys::arbitrary::{self, Arbitrary};
use rlp_rs::{pack_rlp, unpack_rlp, RecursiveBytes, Rlp, RlpError};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TransactionEnvelope {
    Legacy(TransactionLegacy),
    AccessList(TransactionAccessList),
    DynamicFee(TransactionDynamicFee),
    // TODO Blob transaction
}

#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
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

#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq, Eq, Hash)]
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

#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
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
        let tx_type = self.tx_type();
        if tx_type > 0 {
            hasher.update([tx_type]);
        }
        let bytes = match self {
            TransactionEnvelope::Legacy(tx) => rlp_rs::to_bytes(tx),
            TransactionEnvelope::AccessList(tx) => rlp_rs::to_bytes(tx),
            TransactionEnvelope::DynamicFee(tx) => rlp_rs::to_bytes(tx),
        }?;
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

    /// careful, this function does not perform any suffix data check
    pub(crate) fn from_raw_rlp(rlp: &mut Rlp) -> Result<Self, RlpError> {
        let tx_type = match rlp.get(0) {
            Some(RecursiveBytes::Nested(_)) => 0,
            Some(RecursiveBytes::Bytes(bytes)) => {
                let tx_type = match bytes.first().ok_or(RlpError::MissingBytes)? {
                    1 => 1,
                    2 => 2,
                    _ => return Err(RlpError::InvalidBytes),
                };

                // necessary to do it because we just unwrapped the nested structure
                if rlp.get(1).is_some() {
                    return Err(RlpError::InvalidLength);
                }

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

    #[test]
    fn access_list_tx_suffix() {
        let bytes = [
            184, 158, 1, 248, 155, 1, 128, 10, 131, 1, 226, 65, 148, 9, 94, 123, 174, 166, 166,
            199, 196, 194, 223, 235, 151, 126, 250, 195, 38, 175, 85, 45, 135, 128, 128, 248, 56,
            247, 148, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 225, 160, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 1, 160, 61, 186, 204, 141, 2, 89, 242, 80, 134, 37, 233, 127, 223, 197, 124, 216,
            95, 221, 22, 229, 130, 27, 194, 193, 11, 221, 26, 82, 100, 158, 131, 53, 160, 71, 110,
            16, 105, 121, 91, 24, 58, 135, 176, 170, 41, 42, 127, 75, 120, 239, 12, 63, 190, 98,
            170, 44, 66, 200, 78, 29, 156, 61, 161, 89, 239, 20,
        ];

        assert!(matches!(
            TransactionEnvelope::from_bytes(&bytes),
            Err(RlpError::InvalidLength)
        ));
    }
}

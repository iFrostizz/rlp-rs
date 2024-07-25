use crate::primitives::{Address, U256};
use serde::ser::SerializeTupleStruct;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[non_exhaustive]
pub enum TransactionEnvelope {
    Legacy(transaction::Legacy),
    AccessList(transaction::AccessList),
    DynamicFee(transaction::DynamicFee),
}

pub mod transaction {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Legacy {
        nonce: u64,
        gas_price: U256,
        gas_limit: u64,
        to: Address,
        value: U256,
        data: Vec<u8>,
        v: U256,
        r: U256,
        s: U256,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct AccessList {
        chain_id: U256,
        nonce: u64,
        gas_price: U256,
        gas_limit: u64,
        to: Address,
        value: U256,
        data: Vec<u8>,
        access_list: Vec<AccessList>,
        y_parity: U256,
        r: U256,
        s: U256,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct DynamicFee {
        chain_id: U256,
        nonce: u64,
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
        gas_limit: u64,
        destination: Address,
        amount: U256,
        data: Vec<u8>,
        access_list: Vec<AccessList>,
        y_parity: U256,
        r: U256,
        s: U256,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessList {
    address: Address,
    storage_keys: Vec<U256>,
}

impl TransactionEnvelope {
    pub fn tx_type(&self) -> u8 {
        match self {
            TransactionEnvelope::Legacy { .. } => 0,
            TransactionEnvelope::DynamicFee { .. } => 1,
            TransactionEnvelope::AccessList { .. } => 2,
        }
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

impl Serialize for TransactionEnvelope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let tx_type = self.tx_type();
        let mut serializer = if tx_type > 0 {
            if tx_type > 127 {
                return Err(serde::ser::Error::custom("invalid tx type"));
            }

            let mut serializer = serializer.serialize_tuple_struct("Transaction", 2)?;
            serializer.serialize_field(&tx_type)?;
            serializer
        } else {
            serializer.serialize_tuple_struct("Transaction", 1)?
        };

        match self {
            TransactionEnvelope::Legacy(legacy) => serializer.serialize_field(legacy)?,
            TransactionEnvelope::DynamicFee(dynamic_fee) => {
                serializer.serialize_field(dynamic_fee)?
            }
            TransactionEnvelope::AccessList(access_list) => {
                serializer.serialize_field(access_list)?
            }
        }

        serializer.end()
    }
}

#[cfg(test)]
mod tests {
    use super::transaction;

    #[test]
    fn tx_ser_legacy() {
        let tx = transaction::Legacy::default();
        let serialized = rlp_rs::to_bytes(&tx).unwrap();
        let size: usize = 8 + 32 + 8 + 20 + 32 + 0 + 32 * 3 + 9;
        let size_bytes = size.to_be_bytes();
        let size_bytes = size_bytes
            .iter()
            .position(|b| b > &0)
            .map(|i| &size_bytes[i..])
            .unwrap();
        let mut bytes = vec![0xf8 + size_bytes.len() as u8];
        bytes.extend_from_slice(size_bytes);
        bytes.push(0x80 + 8);
        bytes.extend_from_slice(&[0; 8]);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[0; 32]);
        bytes.push(0x80 + 8);
        bytes.extend_from_slice(&[0; 8]);
        bytes.push(0x80 + 20);
        bytes.extend_from_slice(&[0; 20]);
        bytes.push(0x80 + 32);
        bytes.extend_from_slice(&[0; 32]);
        //
        for _ in 0..3 {
            bytes.push(0x80 + 32);
            bytes.extend_from_slice(&[0; 32]);
        }
        assert_eq!(serialized, bytes);
    }
}

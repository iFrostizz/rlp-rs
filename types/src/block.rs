use crate::primitives::{Address, U256};
use crate::TransactionEnvelope;
use rlp_rs::{unpack_rlp, RlpError};
use serde::{Deserialize, Serialize};

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Serialize)]
pub struct Block {
    header: Header,
    transactions: Vec<TransactionEnvelope>,
    uncles: Vec<Header>,
}

impl Block {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RlpError> {
        let raw_rlp = unpack_rlp(bytes)?;
        let rlp_iter = &mut raw_rlp.into_iter();
        let rlp_inner = &mut rlp_iter.next().ok_or(RlpError::MissingBytes)?;
        if rlp_iter.next().is_some() {
            return Err(RlpError::InvalidLength);
        }

        let rlp_iter = &mut rlp_inner
            .flatten_nested()
            .ok_or(RlpError::ExpectedList)?
            .into_iter();

        let header_rlp = &mut rlp_iter.next().ok_or(RlpError::MissingBytes)?;
        let header = Header::deserialize(header_rlp)?;

        let txs_rlp = &mut rlp_iter.next().ok_or(RlpError::MissingBytes)?;
        let transaction_iter = &mut txs_rlp
            .flatten_nested()
            .ok_or(RlpError::MissingBytes)?
            .into_iter();

        let transactions = transaction_iter
            .map(TransactionEnvelope::from_raw_rlp)
            .collect::<Result<_, RlpError>>()?;

        let uncles_rlp = &mut rlp_iter.next().ok_or(RlpError::MissingBytes)?;
        let uncles_iter = &mut uncles_rlp
            .flatten_nested()
            .ok_or(RlpError::MissingBytes)?
            .into_iter();

        let uncles: Vec<_> = uncles_iter
            .map(|mut uncle_rlp| Header::deserialize(&mut uncle_rlp))
            .collect::<Result<_, RlpError>>()?;

        Ok(Block {
            header,
            transactions,
            uncles,
        })
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    #[serde(with = "serde_bytes")]
    parent_hash: U256,
    #[serde(with = "serde_bytes")]
    uncle_hash: U256,
    #[serde(with = "serde_bytes")]
    coinbase: Address,
    #[serde(with = "serde_bytes")]
    state_root: U256,
    #[serde(with = "serde_bytes")]
    tx_root: U256,
    #[serde(with = "serde_bytes")]
    receipt_hash: U256,
    #[serde(with = "serde_bytes")]
    bloom: [u8; 256],
    #[serde(with = "serde_bytes")]
    difficulty: U256,
    #[serde(with = "serde_bytes")]
    number: U256,
    gas_limit: u64,
    gas_used: u64,
    time: u64,
    #[serde(with = "serde_bytes")]
    extra: Vec<u8>,
    #[serde(with = "serde_bytes")]
    mix_digest: U256,
    #[serde(with = "serde_bytes")]
    nonce: [u8; 8],
    // TODO include at the end an optional enum of all the missing stuff
    // use struct variants for each combination. Only one Option is allowed.
}

#[cfg(test)]
mod tests {
    use super::*;

    // https://github.com/ethereum/go-ethereum/blob/4dfc75deefd2d68fba682d089d9ae61771c19d66/core/types/block_test.go#L34
    #[test]
    fn decode_legacy_block() {
        let bytes = hex::decode("f90260f901f9a083cafc574e1f51ba9dc0568fc617a08ea2429fb384059c972f13b19fa1c8dd55a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a05fe50b260da6308036625b850b5d6ced6d0a9f814c0688bc91ffb7b7a3a54b67a0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845506eb0780a0bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff49888a13a5a8c8f2bb1c4f861f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba09bea4c4daac7c7c52e093e6a4c35dbbcf8856f1af7b059ba20253e70848d094fa08a8fae537ce25ed8cb5af9adac3f141af69bd515bd2ba031522df09b97dd72b1c0").unwrap();
        let block: Block = Block::from_bytes(&bytes).unwrap();
        // TODO wrapper types that does that for us
        assert_eq!(
            &block.header.difficulty.as_slice()[24..],
            131072u64.to_be_bytes().as_slice()
        );
        assert_eq!(block.header.gas_limit, 3141592);
        assert_eq!(block.header.gas_used, 21000);
        assert_eq!(
            block.header.coinbase.to_vec(),
            hex::decode("8888f1f195afa192cfee860698584c030f4c9db1").unwrap()
        );
        assert_eq!(
            block.header.mix_digest.to_vec(),
            hex::decode("bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff498")
                .unwrap()
        );
        assert_eq!(
            block.header.state_root.to_vec(),
            hex::decode("ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017")
                .unwrap()
        );
        assert_eq!(block.header.nonce, 0xa13a5a8c8f2bb1c4u64.to_be_bytes());
        assert_eq!(block.header.time, 1426516743);

        assert_eq!(block.transactions.len(), 1);
        let transaction = block.transactions.first().unwrap();
        let TransactionEnvelope::Legacy(transaction) = transaction else {
            panic!("not a legacy transaction");
        };
        assert_eq!(transaction.nonce, 0);
        assert_eq!(
            transaction.to.to_vec(),
            hex::decode("095e7baea6a6c7c4c2dfeb977efac326af552d87").unwrap()
        );
        assert_eq!(
            &transaction.value.as_slice()[24..],
            10u64.to_be_bytes().as_slice()
        );
        assert_eq!(transaction.gas_limit, 50000);
        assert_eq!(
            &transaction.gas_price.as_slice()[24..],
            10u64.to_be_bytes().as_slice()
        );
        assert!(transaction.data.is_empty());
    }
}

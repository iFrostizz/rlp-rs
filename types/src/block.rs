use crate::primitives::{Address, Bloom, Nonce, U256};
use crate::{TransactionEnvelope, B32};
use rlp_rs::{unpack_rlp, RecursiveBytes, Rlp, RlpError};
use serde::ser::SerializeStruct;
use serde::{de, Deserialize, Serialize};
use sha3::{Digest, Keccak256};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Default, Serialize)]
pub struct Block {
    pub header: Header,
    pub transactions: Vec<TransactionEnvelope>,
    pub uncles: Vec<Header>,
}

impl Block {
    pub fn hash(&self) -> Result<[u8; 32], RlpError> {
        let bytes = rlp_rs::to_bytes(&self.header)?;
        let mut hasher = Keccak256::new();
        hasher.update(bytes);
        Ok(hasher.finalize().into())
    }

    fn _from_bytes(bytes: &[u8], unknown: bool) -> Result<Self, RlpError> {
        let raw_rlp = unpack_rlp(bytes)?;

        let rlp_iter = &mut raw_rlp.into_iter();
        let rlp_inner = &mut rlp_iter.next().ok_or(RlpError::MissingBytes)?;

        let flat_rlp = rlp_inner.flatten_nested().ok_or(RlpError::ExpectedList)?;
        let rlp_iter = &mut flat_rlp.into_iter();

        let header_rlp = rlp_iter.next().ok_or(RlpError::MissingBytes)?;
        let header = if unknown {
            Header::unknown_from_raw_rlp(header_rlp)?
        } else {
            Header::from_raw_rlp(header_rlp)?
        };

        let txs_rlp = &mut rlp_iter.next().ok_or(RlpError::MissingBytes)?;
        let transaction_iter = txs_rlp
            .flatten_nested()
            .ok_or(RlpError::MissingBytes)?
            .into_iter();

        let transactions = transaction_iter
            .map(|mut rlp: rlp_rs::Rlp| TransactionEnvelope::from_raw_rlp(&mut rlp))
            .collect::<Result<_, RlpError>>()?;

        let uncles_rlp = &mut rlp_iter.next().ok_or(RlpError::MissingBytes)?;
        let uncles_iter = &mut uncles_rlp
            .flatten_nested()
            .ok_or(RlpError::MissingBytes)?
            .into_iter();

        let uncles: Vec<_> = uncles_iter
            .map(if unknown {
                Header::unknown_from_raw_rlp
            } else {
                Header::from_raw_rlp
            })
            .collect::<Result<_, RlpError>>()?;

        Ok(Block {
            header,
            transactions,
            uncles,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RlpError> {
        Self::_from_bytes(bytes, false)
    }

    pub fn unknown_from_bytes(bytes: &[u8]) -> Result<Self, RlpError> {
        Self::_from_bytes(bytes, true)
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone, Default)]
pub struct CommonHeader {
    pub parent_hash: B32,
    pub uncle_hash: B32,
    pub coinbase: Address,
    pub state_root: B32,
    pub tx_root: B32,
    pub receipt_hash: B32,
    pub bloom: Bloom,
    pub difficulty: U256,
    pub number: U256,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub time: u64,
    #[serde(with = "serde_bytes")]
    pub extra: Vec<u8>,
    pub mix_digest: B32,
    pub nonce: Nonce,
}

impl CommonHeader {
    const fn fields() -> usize {
        15
    }
}

macro_rules! define_header {
    (
        $(
            $name:ident {
                $(
                    $(#[$field_meta:meta])?
                    $field_name:ident : $field_type:ty
                ),* $(,)?
            }
        ),* $(,)?
    ) => {
        #[derive(Debug, PartialEq, Eq, Hash, Clone)]
        pub enum Header {
            $(
                $name {
                    parent_hash: B32,
                    uncle_hash: B32,
                    coinbase: Address,
                    state_root: B32,
                    tx_root: B32,
                    receipt_hash: B32,
                    bloom: Bloom,
                    difficulty: U256,
                    number: U256,
                    gas_limit: u64,
                    gas_used: u64,
                    time: u64,
                    extra: Vec<u8>,
                    mix_digest: B32,
                    nonce: Nonce,
                    $(
                        $(#[$field_meta])?
                        $field_name: $field_type
                    ),*
                },
            )*
        }
    };
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone)]
pub struct Bytes(#[serde(with = "serde_bytes")] Vec<u8>);

impl From<Bytes> for Vec<u8> {
    fn from(value: Bytes) -> Self {
        value.0
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

define_header! {
    Legacy {},
    London {
        base_fee: U256
    },
    Shanghai {
        base_fee: U256,
        withdrawal_root: B32
    },
    Cancun {
        base_fee: U256,
        withdrawal_root: B32,
        blob_gas_used: u64,
        excess_blob_gas: u64,
        parent_beacon_block_root: B32
    },
    Unknown {
        rest: Vec<Bytes>
    }
}

impl Header {
    fn fields(&self) -> usize {
        match self {
            Self::Legacy { .. } => 15,
            Self::London { .. } => 16,
            Self::Shanghai { .. } => 17,
            Self::Cancun { .. } => 20,
            Self::Unknown { .. } => 16,
        }
    }
}

impl Serialize for Header {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Legacy {
                parent_hash,
                uncle_hash,
                coinbase,
                state_root,
                tx_root,
                receipt_hash,
                bloom,
                difficulty,
                number,
                gas_limit,
                gas_used,
                time,
                extra,
                mix_digest,
                nonce,
            }
            | Self::London {
                parent_hash,
                uncle_hash,
                coinbase,
                state_root,
                tx_root,
                receipt_hash,
                bloom,
                difficulty,
                number,
                gas_limit,
                gas_used,
                time,
                extra,
                mix_digest,
                nonce,
                ..
            }
            | Self::Shanghai {
                parent_hash,
                uncle_hash,
                coinbase,
                state_root,
                tx_root,
                receipt_hash,
                bloom,
                difficulty,
                number,
                gas_limit,
                gas_used,
                time,
                extra,
                mix_digest,
                nonce,
                ..
            }
            | Self::Cancun {
                parent_hash,
                uncle_hash,
                coinbase,
                state_root,
                tx_root,
                receipt_hash,
                bloom,
                difficulty,
                number,
                gas_limit,
                gas_used,
                time,
                extra,
                mix_digest,
                nonce,
                ..
            }
            | Self::Unknown {
                parent_hash,
                uncle_hash,
                coinbase,
                state_root,
                tx_root,
                receipt_hash,
                bloom,
                difficulty,
                number,
                gas_limit,
                gas_used,
                time,
                extra,
                mix_digest,
                nonce,
                ..
            } => {
                let mut serializer = serializer.serialize_struct("Header", self.fields())?;
                serializer.serialize_field("parent_hash", parent_hash)?;
                serializer.serialize_field("uncle_hash", uncle_hash)?;
                serializer.serialize_field("coinbase", coinbase)?;
                serializer.serialize_field("state_root", state_root)?;
                serializer.serialize_field("tx_root", tx_root)?;
                serializer.serialize_field("receipt_hash", receipt_hash)?;
                serializer.serialize_field("bloom", bloom)?;
                serializer.serialize_field("difficulty", difficulty)?;
                serializer.serialize_field("number", number)?;
                serializer.serialize_field("gas_limit", gas_limit)?;
                serializer.serialize_field("gas_used", gas_used)?;
                serializer.serialize_field("time", time)?;
                serializer.serialize_field("extra", &serde_bytes::Bytes::new(extra))?;
                serializer.serialize_field("mix_digest", mix_digest)?;
                serializer.serialize_field("nonce", nonce)?;
                match self {
                    Self::Legacy { .. } => serializer.end(),
                    Self::Unknown { rest, .. } => {
                        for bytes in rest {
                            serializer.serialize_field("rest", bytes)?;
                        }
                        serializer.end()
                    }
                    Self::London { base_fee, .. }
                    | Self::Shanghai { base_fee, .. }
                    | Self::Cancun { base_fee, .. } => {
                        serializer.serialize_field("base_fee", base_fee)?;
                        match self {
                            Self::London { .. } => serializer.end(),
                            Self::Shanghai {
                                withdrawal_root, ..
                            }
                            | Self::Cancun {
                                withdrawal_root, ..
                            } => {
                                serializer.serialize_field("withdrawal_root", withdrawal_root)?;
                                match self {
                                    Self::Shanghai { .. } => serializer.end(),
                                    Self::Cancun {
                                        blob_gas_used,
                                        excess_blob_gas,
                                        parent_beacon_block_root,
                                        ..
                                    } => {
                                        serializer
                                            .serialize_field("blob_gas_used", blob_gas_used)?;
                                        serializer
                                            .serialize_field("excess_blob_gas", excess_blob_gas)?;
                                        serializer.serialize_field(
                                            "parent_beacon_block_root",
                                            parent_beacon_block_root,
                                        )?;
                                        serializer.end()
                                    }
                                    _ => unreachable!(),
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }
}

macro_rules! field_impl {
    ($field:ident, $ty:ty) => {
        pub fn $field(&self) -> &$ty {
            match self {
                Header::Legacy { $field, .. }
                | Header::London { $field, .. }
                | Header::Shanghai { $field, .. }
                | Header::Cancun { $field, .. }
                | Header::Unknown { $field, .. } => $field,
            }
        }
    };
}

impl Header {
    field_impl!(parent_hash, B32);
    field_impl!(uncle_hash, B32);
    field_impl!(coinbase, Address);
    field_impl!(state_root, B32);
    field_impl!(tx_root, B32);
    field_impl!(receipt_hash, B32);
    field_impl!(bloom, Bloom);
    field_impl!(difficulty, U256);
    field_impl!(number, U256);
    field_impl!(gas_limit, u64);
    field_impl!(gas_used, u64);
    field_impl!(time, u64);
    field_impl!(extra, Vec<u8>);
    field_impl!(mix_digest, B32);
    field_impl!(nonce, Nonce);
}

macro_rules! common_impl {
    ($name:ident, $common:ident) => {
        common_impl!($name, $common, {})
    };
    ($name:ident, $common:ident, { $($rest:tt)* }) => {
        Header::$name {
            parent_hash: $common.parent_hash,
            uncle_hash: $common.uncle_hash,
            coinbase: $common.coinbase,
            state_root: $common.state_root,
            tx_root: $common.tx_root,
            receipt_hash: $common.receipt_hash,
            bloom: $common.bloom,
            difficulty: $common.difficulty,
            number: $common.number,
            gas_limit: $common.gas_limit,
            gas_used: $common.gas_used,
            time: $common.time,
            extra: $common.extra,
            mix_digest: $common.mix_digest,
            nonce: $common.nonce,
            $($rest)*
        }
    };
}

impl Header {
    fn common_from_raw_rlp(rlp: &mut Rlp) -> Result<CommonHeader, RlpError> {
        let common_fields = CommonHeader::fields();
        let common_rec = (0..common_fields)
            .map(|_| rlp.pop_front().ok_or(RlpError::MissingBytes))
            .collect::<Result<_, RlpError>>()?;
        let nested = RecursiveBytes::Nested(common_rec);
        let common_rlp = &mut Rlp::new_unary(nested);
        CommonHeader::deserialize(common_rlp)
    }

    pub fn from_raw_rlp(mut rlp: Rlp) -> Result<Self, RlpError> {
        let rlp = &mut rlp;
        let mut rlp = rlp.flatten_nested().ok_or(RlpError::MissingBytes)?;

        let fields = rlp.len();
        let common_fields = CommonHeader::fields();
        let london_fields = common_fields + 1;
        let shanghai_fields = london_fields + 1;
        let cancun_fields = shanghai_fields + 3;

        if fields < common_fields {
            return Err(RlpError::InvalidBytes);
        }

        let common = Self::common_from_raw_rlp(&mut rlp)?;

        match fields {
            15 | 16 | 17 | 20 => {
                let header = match fields {
                    15 => common_impl!(Legacy, common),
                    16 | 17 | 20 => {
                        let base_fee = <U256>::deserialize(
                            &mut rlp.pop_front().ok_or(RlpError::MissingBytes)?.into_rlp(),
                        )
                        .map_err(|_| RlpError::MissingBytes)?;

                        if fields == london_fields {
                            common_impl!(London, common, { base_fee })
                        } else {
                            let withdrawal_root = <B32>::deserialize(
                                &mut rlp.pop_front().ok_or(RlpError::MissingBytes)?.into_rlp(),
                            )
                            .map_err(|_| RlpError::MissingBytes)?;

                            if fields == shanghai_fields {
                                common_impl!(Shanghai, common, { base_fee, withdrawal_root})
                            } else {
                                assert_eq!(fields, cancun_fields);
                                let blob_gas_used = u64::deserialize(
                                    &mut rlp.pop_front().ok_or(RlpError::MissingBytes)?.into_rlp(),
                                )?;
                                let excess_blob_gas = u64::deserialize(
                                    &mut rlp.pop_front().ok_or(RlpError::MissingBytes)?.into_rlp(),
                                )?;
                                let parent_beacon_block_root = B32::deserialize(
                                    &mut rlp.pop_front().ok_or(RlpError::MissingBytes)?.into_rlp(),
                                )?;

                                common_impl!(Cancun, common, {
                                    base_fee,
                                    withdrawal_root,
                                    blob_gas_used,
                                    excess_blob_gas,
                                    parent_beacon_block_root,
                                })
                            }
                        }
                    }
                    _ => unreachable!(),
                };

                if rlp.is_empty() {
                    Ok(header)
                } else {
                    Err(RlpError::TrailingBytes)
                }
            }
            _ => Err(RlpError::TrailingBytes),
        }
    }

    // TODO it would be better to pass in a mutable ref so we can check for bytes left
    pub fn unknown_from_raw_rlp(mut rlp: Rlp) -> Result<Self, RlpError> {
        let rlp = &mut rlp;
        let mut rlp = rlp.flatten_nested().ok_or(RlpError::MissingBytes)?;

        let fields = rlp.len();
        let common_fields = CommonHeader::fields();

        if fields < common_fields {
            return Err(RlpError::InvalidBytes);
        }

        let common = Self::common_from_raw_rlp(&mut rlp)?;

        let mut rest = Vec::new();
        while let Some(rec) = rlp.pop_front() {
            let bytes = match rec {
                RecursiveBytes::Bytes(bytes) => bytes.into(),
                _ => return Err(RlpError::ExpectedBytes),
            };
            rest.push(bytes);
        }

        Ok(common_impl!(Unknown, common, { rest }))
    }
}

impl<'de> Deserialize<'de> for Header {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_seq(HeaderVisitor)
    }
}

struct HeaderVisitor;

impl<'de> de::Visitor<'de> for HeaderVisitor {
    type Value = Header;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a well formed RLP-encoded Header")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        macro_rules! next_el {
            ($len:literal) => {
                seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length($len, &self))?
            };
        }

        let parent_hash = next_el!(0);
        let uncle_hash = next_el!(1);
        let coinbase = next_el!(2);
        let state_root = next_el!(3);
        let tx_root = next_el!(4);
        let receipt_hash = next_el!(5);
        let bloom = next_el!(6);
        let difficulty = next_el!(7);
        let number = next_el!(8);
        let gas_limit = next_el!(9);
        let gas_used = next_el!(10);
        let time = next_el!(11);
        let extra: serde_bytes::ByteBuf = next_el!(12);
        let extra = extra.to_vec();
        let mix_digest = next_el!(13);
        let nonce = next_el!(14);

        let common = CommonHeader {
            parent_hash,
            uncle_hash,
            coinbase,
            state_root,
            tx_root,
            receipt_hash,
            bloom,
            difficulty,
            number,
            gas_limit,
            gas_used,
            time,
            extra,
            mix_digest,
            nonce,
        };

        let next_element = seq.next_element();
        let header = match next_element? {
            None => common_impl!(Legacy, common),
            Some(base_fee) => match seq.next_element()? {
                None => common_impl!(London, common, { base_fee }),
                Some(withdrawal_root) => match seq.next_element()? {
                    None => common_impl!(Shanghai, common, {
                        base_fee,
                        withdrawal_root
                    }),
                    Some(blob_gas_used) => match (seq.next_element()?, seq.next_element()?) {
                        (Some(excess_blob_gas), Some(parent_beacon_block_root)) => {
                            common_impl!(Cancun, common, {
                                base_fee,
                                withdrawal_root,
                                blob_gas_used,
                                excess_blob_gas,
                                parent_beacon_block_root
                            })
                        }
                        _ => return Err(de::Error::invalid_length(18, &self)),
                    },
                },
            },
        };

        Ok(header)
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::Legacy {
            parent_hash: Default::default(),
            uncle_hash: Default::default(),
            coinbase: Default::default(),
            state_root: Default::default(),
            tx_root: Default::default(),
            receipt_hash: Default::default(),
            bloom: Default::default(),
            difficulty: Default::default(),
            number: Default::default(),
            gas_limit: Default::default(),
            gas_used: Default::default(),
            time: Default::default(),
            extra: Default::default(),
            mix_digest: Default::default(),
            nonce: Default::default(),
        }
    }
}

// TODO cleanup these decode_*_block tests
// The byte array is reducted to the smallest unit.
// But we should probably convert all of these to actual arrays so they have the same size.
// Can also directly "try_into()" convert to numbers when applicable.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{
        AccessList, TransactionAccessList, TransactionDynamicFee, TransactionLegacy,
    };
    use rlp_rs::Rlp;

    #[test]
    fn decode_legacy_header() {
        let bytes = hex::decode("f90260f901f9a083cafc574e1f51ba9dc0568fc617a08ea2429fb384059c972f13b19fa1c8dd55a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a05fe50b260da6308036625b850b5d6ced6d0a9f814c0688bc91ffb7b7a3a54b67a0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845506eb0780a0bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff49888a13a5a8c8f2bb1c4f861f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba09bea4c4daac7c7c52e093e6a4c35dbbcf8856f1af7b059ba20253e70848d094fa08a8fae537ce25ed8cb5af9adac3f141af69bd515bd2ba031522df09b97dd72b1c0").unwrap();
        let mut rlp = rlp_rs::unpack_rlp(&bytes).unwrap();
        let mut block_rlp = Rlp::new_unary(rlp.pop_front().unwrap())
            .flatten_nested()
            .unwrap();
        let header_rlp = Rlp::new_unary(block_rlp.pop_front().unwrap());
        let header_bytes = rlp_rs::pack_rlp(header_rlp).unwrap();
        let header: Header = rlp_rs::from_bytes(&header_bytes).unwrap();

        assert_eq!(
            header,
            Header::Legacy {
                parent_hash: [
                    131, 202, 252, 87, 78, 31, 81, 186, 157, 192, 86, 143, 198, 23, 160, 142, 162,
                    66, 159, 179, 132, 5, 156, 151, 47, 19, 177, 159, 161, 200, 221, 85
                ]
                .into(),
                uncle_hash: [
                    29, 204, 77, 232, 222, 199, 93, 122, 171, 133, 181, 103, 182, 204, 212, 26,
                    211, 18, 69, 27, 148, 138, 116, 19, 240, 161, 66, 253, 64, 212, 147, 71
                ]
                .into(),
                coinbase: hex::decode("8888f1f195afa192cfee860698584c030f4c9db1")
                    .unwrap()
                    .try_into()
                    .unwrap(),
                state_root: hex::decode(
                    "ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017"
                )
                .unwrap()
                .try_into()
                .unwrap(),
                tx_root: [
                    95, 229, 11, 38, 13, 166, 48, 128, 54, 98, 91, 133, 11, 93, 108, 237, 109, 10,
                    159, 129, 76, 6, 136, 188, 145, 255, 183, 183, 163, 165, 75, 103
                ]
                .into(),
                receipt_hash: [
                    188, 55, 215, 151, 83, 173, 115, 138, 109, 172, 73, 33, 229, 115, 146, 241, 69,
                    216, 136, 116, 118, 222, 63, 120, 61, 250, 126, 218, 233, 40, 62, 82
                ]
                .into(),
                bloom: [0; 256].into(),
                difficulty: 131072u64.to_be_bytes()[5..].to_vec().try_into().unwrap(),
                number: vec![1u8].try_into().unwrap(),
                gas_limit: 3141592,
                gas_used: 21000,
                time: 1426516743,
                extra: vec![],
                mix_digest: hex::decode(
                    "bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff498"
                )
                .unwrap()
                .try_into()
                .unwrap(),
                nonce: [161, 58, 90, 140, 143, 43, 177, 196].into()
            }
        );
    }

    // https://github.com/ethereum/go-ethereum/blob/4dfc75deefd2d68fba682d089d9ae61771c19d66/core/types/block_test.go#L34
    #[test]
    fn decode_legacy_block() {
        let bytes = hex::decode("f90260f901f9a083cafc574e1f51ba9dc0568fc617a08ea2429fb384059c972f13b19fa1c8dd55a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a05fe50b260da6308036625b850b5d6ced6d0a9f814c0688bc91ffb7b7a3a54b67a0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845506eb0780a0bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff49888a13a5a8c8f2bb1c4f861f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba09bea4c4daac7c7c52e093e6a4c35dbbcf8856f1af7b059ba20253e70848d094fa08a8fae537ce25ed8cb5af9adac3f141af69bd515bd2ba031522df09b97dd72b1c0").unwrap();
        // let block: Block = Block::from_bytes(&bytes).unwrap();
        let block: Block = Block::from_bytes(&bytes).unwrap();

        let coinbase = hex::decode("8888f1f195afa192cfee860698584c030f4c9db1")
            .unwrap()
            .try_into()
            .unwrap();
        let state_root =
            hex::decode("ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017")
                .unwrap()
                .try_into()
                .unwrap();
        let mix_digest =
            hex::decode("bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff498")
                .unwrap()
                .try_into()
                .unwrap();
        let difficulty = 131072u64.to_be_bytes()[5..].to_vec().try_into().unwrap();
        let number = vec![1u8].try_into().unwrap();

        assert_eq!(
            block.header,
            Header::Legacy {
                parent_hash: [
                    131, 202, 252, 87, 78, 31, 81, 186, 157, 192, 86, 143, 198, 23, 160, 142, 162,
                    66, 159, 179, 132, 5, 156, 151, 47, 19, 177, 159, 161, 200, 221, 85
                ]
                .into(),
                uncle_hash: [
                    29, 204, 77, 232, 222, 199, 93, 122, 171, 133, 181, 103, 182, 204, 212, 26,
                    211, 18, 69, 27, 148, 138, 116, 19, 240, 161, 66, 253, 64, 212, 147, 71
                ]
                .into(),
                coinbase,
                state_root,
                tx_root: [
                    95, 229, 11, 38, 13, 166, 48, 128, 54, 98, 91, 133, 11, 93, 108, 237, 109, 10,
                    159, 129, 76, 6, 136, 188, 145, 255, 183, 183, 163, 165, 75, 103
                ]
                .into(),
                receipt_hash: [
                    188, 55, 215, 151, 83, 173, 115, 138, 109, 172, 73, 33, 229, 115, 146, 241, 69,
                    216, 136, 116, 118, 222, 63, 120, 61, 250, 126, 218, 233, 40, 62, 82
                ]
                .into(),
                bloom: [0; 256].into(),
                difficulty,
                number,
                gas_limit: 3141592,
                gas_used: 21000,
                time: 1426516743,
                extra: vec![],
                mix_digest,
                nonce: [161, 58, 90, 140, 143, 43, 177, 196].into()
            }
        );

        assert_eq!(block.transactions.len(), 1);
        let transaction = block.transactions.first().unwrap();
        let hash: [u8; 32] =
            hex::decode("77b19baa4de67e45a7b26e4a220bccdbb6731885aa9927064e239ca232023215")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(transaction.hash().unwrap(), hash);
        let TransactionEnvelope::Legacy(transaction) = transaction else {
            panic!("not a legacy transaction");
        };

        let gas_price = vec![10u8].try_into().unwrap();
        let to = hex::decode("095e7baea6a6c7c4c2dfeb977efac326af552d87")
            .unwrap()
            .try_into()
            .unwrap();
        let value = vec![10u8].try_into().unwrap();
        let v = vec![27u8].try_into().unwrap();

        assert_eq!(
            transaction,
            &TransactionLegacy {
                nonce: 0,
                gas_price,
                gas_limit: 50000,
                to,
                value,
                data: vec![],
                v,
                r: [
                    155, 234, 76, 77, 170, 199, 199, 197, 46, 9, 62, 106, 76, 53, 219, 188, 248,
                    133, 111, 26, 247, 176, 89, 186, 32, 37, 62, 112, 132, 141, 9, 79
                ]
                .into(),
                s: [
                    138, 143, 174, 83, 124, 226, 94, 216, 203, 90, 249, 173, 172, 63, 20, 26, 246,
                    155, 213, 21, 189, 43, 160, 49, 82, 45, 240, 155, 151, 221, 114, 177
                ]
                .into()
            }
        );
    }

    #[test]
    fn decode_1559_header() {
        let bytes = hex::decode("f9030bf901fea083cafc574e1f51ba9dc0568fc617a08ea2429fb384059c972f13b19fa1c8dd55a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a05fe50b260da6308036625b850b5d6ced6d0a9f814c0688bc91ffb7b7a3a54b67a0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845506eb0780a0bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff49888a13a5a8c8f2bb1c4843b9aca00f90106f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba09bea4c4daac7c7c52e093e6a4c35dbbcf8856f1af7b059ba20253e70848d094fa08a8fae537ce25ed8cb5af9adac3f141af69bd515bd2ba031522df09b97dd72b1b8a302f8a0018080843b9aca008301e24194095e7baea6a6c7c4c2dfeb977efac326af552d878080f838f7940000000000000000000000000000000000000001e1a0000000000000000000000000000000000000000000000000000000000000000080a0fe38ca4e44a30002ac54af7cf922a6ac2ba11b7d22f548e8ecb3f51f41cb31b0a06de6a5cbae13c0c856e33acf021b51819636cfc009d39eafb9f606d546e305a8c0").unwrap();
        let mut rlp = rlp_rs::unpack_rlp(&bytes).unwrap();
        let mut block_rlp = Rlp::new_unary(rlp.pop_front().unwrap())
            .flatten_nested()
            .unwrap();
        let header_rlp = Rlp::new_unary(block_rlp.pop_front().unwrap());
        let header_bytes = rlp_rs::pack_rlp(header_rlp).unwrap();
        let header: Header = rlp_rs::from_bytes(&header_bytes).unwrap();

        assert_eq!(
            header,
            Header::London {
                parent_hash: [
                    131, 202, 252, 87, 78, 31, 81, 186, 157, 192, 86, 143, 198, 23, 160, 142, 162,
                    66, 159, 179, 132, 5, 156, 151, 47, 19, 177, 159, 161, 200, 221, 85
                ]
                .into(),
                uncle_hash: [
                    29, 204, 77, 232, 222, 199, 93, 122, 171, 133, 181, 103, 182, 204, 212, 26,
                    211, 18, 69, 27, 148, 138, 116, 19, 240, 161, 66, 253, 64, 212, 147, 71
                ]
                .into(),
                coinbase: hex::decode("8888f1f195afa192cfee860698584c030f4c9db1")
                    .unwrap()
                    .try_into()
                    .unwrap(),
                state_root: hex::decode(
                    "ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017"
                )
                .unwrap()
                .try_into()
                .unwrap(),
                tx_root: [
                    95, 229, 11, 38, 13, 166, 48, 128, 54, 98, 91, 133, 11, 93, 108, 237, 109, 10,
                    159, 129, 76, 6, 136, 188, 145, 255, 183, 183, 163, 165, 75, 103
                ]
                .into(),
                receipt_hash: [
                    188, 55, 215, 151, 83, 173, 115, 138, 109, 172, 73, 33, 229, 115, 146, 241, 69,
                    216, 136, 116, 118, 222, 63, 120, 61, 250, 126, 218, 233, 40, 62, 82
                ]
                .into(),
                bloom: [0; 256].into(),
                difficulty: 131072u64.to_be_bytes()[5..].to_vec().try_into().unwrap(),
                number: vec![1u8].try_into().unwrap(),
                gas_limit: 3141592,
                gas_used: 21000,
                time: 1426516743,
                extra: vec![],
                mix_digest: hex::decode(
                    "bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff498"
                )
                .unwrap()
                .try_into()
                .unwrap(),
                nonce: [161, 58, 90, 140, 143, 43, 177, 196].into(),
                base_fee: vec![59, 154, 202, 0].try_into().unwrap()
            }
        );
    }

    #[test]
    fn decode_1559_block() {
        let bytes = hex::decode("f9030bf901fea083cafc574e1f51ba9dc0568fc617a08ea2429fb384059c972f13b19fa1c8dd55a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a05fe50b260da6308036625b850b5d6ced6d0a9f814c0688bc91ffb7b7a3a54b67a0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845506eb0780a0bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff49888a13a5a8c8f2bb1c4843b9aca00f90106f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba09bea4c4daac7c7c52e093e6a4c35dbbcf8856f1af7b059ba20253e70848d094fa08a8fae537ce25ed8cb5af9adac3f141af69bd515bd2ba031522df09b97dd72b1b8a302f8a0018080843b9aca008301e24194095e7baea6a6c7c4c2dfeb977efac326af552d878080f838f7940000000000000000000000000000000000000001e1a0000000000000000000000000000000000000000000000000000000000000000080a0fe38ca4e44a30002ac54af7cf922a6ac2ba11b7d22f548e8ecb3f51f41cb31b0a06de6a5cbae13c0c856e33acf021b51819636cfc009d39eafb9f606d546e305a8c0").unwrap();
        let block: Block = Block::from_bytes(&bytes).unwrap();

        let coinbase = hex::decode("8888f1f195afa192cfee860698584c030f4c9db1")
            .unwrap()
            .try_into()
            .unwrap();
        let state_root =
            hex::decode("ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017")
                .unwrap()
                .try_into()
                .unwrap();
        let difficulty = 131072u64.to_be_bytes()[5..].to_vec().try_into().unwrap();
        let number = vec![1u8].try_into().unwrap();
        let mix_digest =
            hex::decode("bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff498")
                .unwrap()
                .try_into()
                .unwrap();

        let base_fee: U256 = vec![59, 154, 202, 0].try_into().unwrap();

        assert_eq!(
            block.header,
            Header::London {
                parent_hash: [
                    131, 202, 252, 87, 78, 31, 81, 186, 157, 192, 86, 143, 198, 23, 160, 142, 162,
                    66, 159, 179, 132, 5, 156, 151, 47, 19, 177, 159, 161, 200, 221, 85
                ]
                .into(),
                uncle_hash: [
                    29, 204, 77, 232, 222, 199, 93, 122, 171, 133, 181, 103, 182, 204, 212, 26,
                    211, 18, 69, 27, 148, 138, 116, 19, 240, 161, 66, 253, 64, 212, 147, 71
                ]
                .into(),
                coinbase,
                state_root,
                tx_root: [
                    95, 229, 11, 38, 13, 166, 48, 128, 54, 98, 91, 133, 11, 93, 108, 237, 109, 10,
                    159, 129, 76, 6, 136, 188, 145, 255, 183, 183, 163, 165, 75, 103
                ]
                .into(),
                receipt_hash: [
                    188, 55, 215, 151, 83, 173, 115, 138, 109, 172, 73, 33, 229, 115, 146, 241, 69,
                    216, 136, 116, 118, 222, 63, 120, 61, 250, 126, 218, 233, 40, 62, 82
                ]
                .into(),
                bloom: [0; 256].into(),
                difficulty,
                number,
                gas_limit: 3141592,
                gas_used: 21000,
                time: 1426516743,
                extra: vec![],
                mix_digest,
                nonce: [161, 58, 90, 140, 143, 43, 177, 196].into(),
                base_fee: base_fee.clone()
            }
        );

        assert_eq!(block.transactions.len(), 2);

        let mut transactions_iter = block.transactions.into_iter();
        let transaction = transactions_iter.next().unwrap();
        let hash: [u8; 32] =
            hex::decode("77b19baa4de67e45a7b26e4a220bccdbb6731885aa9927064e239ca232023215")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(transaction.hash().unwrap(), hash);
        let TransactionEnvelope::Legacy(tx1) = transaction else {
            panic!("invalid tx");
        };

        let gas_price = vec![10u8].try_into().unwrap();
        let to = hex::decode("095e7baea6a6c7c4c2dfeb977efac326af552d87")
            .unwrap()
            .try_into()
            .unwrap();
        let value = vec![10u8].try_into().unwrap();
        let v = vec![27u8].try_into().unwrap();

        assert_eq!(
            tx1,
            TransactionLegacy {
                nonce: 0,
                gas_price,
                gas_limit: 50000,
                to,
                value,
                data: vec![],
                v,
                r: [
                    155, 234, 76, 77, 170, 199, 199, 197, 46, 9, 62, 106, 76, 53, 219, 188, 248,
                    133, 111, 26, 247, 176, 89, 186, 32, 37, 62, 112, 132, 141, 9, 79
                ]
                .into(),
                s: [
                    138, 143, 174, 83, 124, 226, 94, 216, 203, 90, 249, 173, 172, 63, 20, 26, 246,
                    155, 213, 21, 189, 43, 160, 49, 82, 45, 240, 155, 151, 221, 114, 177
                ]
                .into()
            }
        );

        let transaction = transactions_iter.next().unwrap();
        let hash: [u8; 32] =
            hex::decode("c5a8f6026a3554e9731e6ad1c17a7450b8fe2d048cd755752cc985a89a2e125c")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(transaction.hash().unwrap(), hash);
        let TransactionEnvelope::DynamicFee(tx2) = transaction else {
            panic!("invalid tx");
        };

        let chain_id = vec![1u8].try_into().unwrap();
        let destination = {
            let mut arr = [0; 20];
            arr.copy_from_slice(&hex::decode("095e7baea6a6c7c4c2dfeb977efac326af552d87").unwrap());
            arr.into()
        };
        let access_list = {
            let address = {
                let mut arr = [0; 20];
                arr[19] = 1;
                arr.into()
            };
            vec![AccessList {
                address,
                storage_keys: vec![[0; 32].into()],
            }]
        };

        assert_eq!(
            tx2,
            TransactionDynamicFee {
                chain_id,
                nonce: 0,
                max_priority_fee_per_gas: vec![].try_into().unwrap(),
                max_fee_per_gas: base_fee,
                gas_limit: 123457,
                destination,
                amount: vec![].try_into().unwrap(),
                data: vec![],
                access_list,
                y_parity: vec![].try_into().unwrap(),
                r: [
                    254, 56, 202, 78, 68, 163, 0, 2, 172, 84, 175, 124, 249, 34, 166, 172, 43, 161,
                    27, 125, 34, 245, 72, 232, 236, 179, 245, 31, 65, 203, 49, 176
                ]
                .into(),
                s: [
                    109, 230, 165, 203, 174, 19, 192, 200, 86, 227, 58, 207, 2, 27, 81, 129, 150,
                    54, 207, 192, 9, 211, 158, 175, 185, 246, 6, 213, 70, 227, 5, 168
                ]
                .into()
            }
        );
    }

    #[test]
    fn decode_2718_block() {
        let bytes = hex::decode("f90319f90211a00000000000000000000000000000000000000000000000000000000000000000a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a0e6e49996c7ec59f7a23d22b83239a60151512c65613bf84a0d7da336399ebc4aa0cafe75574d59780665a97fbfd11365c7545aa8f1abf4e5e12e8243334ef7286bb901000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000083020000820200832fefd882a410845506eb0796636f6f6c65737420626c6f636b206f6e20636861696ea0bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff49888a13a5a8c8f2bb1c4f90101f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba09bea4c4daac7c7c52e093e6a4c35dbbcf8856f1af7b059ba20253e70848d094fa08a8fae537ce25ed8cb5af9adac3f141af69bd515bd2ba031522df09b97dd72b1b89e01f89b01800a8301e24194095e7baea6a6c7c4c2dfeb977efac326af552d878080f838f7940000000000000000000000000000000000000001e1a0000000000000000000000000000000000000000000000000000000000000000001a03dbacc8d0259f2508625e97fdfc57cd85fdd16e5821bc2c10bdd1a52649e8335a0476e10695b183a87b0aa292a7f4b78ef0c3fbe62aa2c42c84e1d9c3da159ef14c0").unwrap();
        let block: Block = Block::from_bytes(&bytes).unwrap();

        let coinbase = hex::decode("8888f1f195afa192cfee860698584c030f4c9db1")
            .unwrap()
            .try_into()
            .unwrap();
        let state_root =
            hex::decode("ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017")
                .unwrap()
                .try_into()
                .unwrap();
        let mix_digest =
            hex::decode("bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff498")
                .unwrap()
                .try_into()
                .unwrap();
        let difficulty = 131072u64.to_be_bytes()[5..].to_vec().try_into().unwrap();
        let number = 512u16.to_be_bytes().to_vec().try_into().unwrap();

        assert_eq!(
            block.header,
            Header::Legacy {
                parent_hash: [0; 32].into(),
                uncle_hash: [
                    29, 204, 77, 232, 222, 199, 93, 122, 171, 133, 181, 103, 182, 204, 212, 26,
                    211, 18, 69, 27, 148, 138, 116, 19, 240, 161, 66, 253, 64, 212, 147, 71
                ]
                .into(),
                coinbase,
                state_root,
                tx_root: [
                    230, 228, 153, 150, 199, 236, 89, 247, 162, 61, 34, 184, 50, 57, 166, 1, 81,
                    81, 44, 101, 97, 59, 248, 74, 13, 125, 163, 54, 57, 158, 188, 74
                ]
                .into(),
                receipt_hash: [
                    202, 254, 117, 87, 77, 89, 120, 6, 101, 169, 127, 191, 209, 19, 101, 199, 84,
                    90, 168, 241, 171, 244, 229, 225, 46, 130, 67, 51, 78, 247, 40, 107
                ]
                .into(),
                bloom: [0; 256].into(),
                difficulty,
                number,
                gas_limit: 3141592,
                gas_used: 42000,
                time: 1426516743,
                extra: vec![
                    99, 111, 111, 108, 101, 115, 116, 32, 98, 108, 111, 99, 107, 32, 111, 110, 32,
                    99, 104, 97, 105, 110
                ],
                mix_digest,
                nonce: [161, 58, 90, 140, 143, 43, 177, 196].into()
            }
        );

        assert_eq!(block.transactions.len(), 2);

        let mut transactions_iter = block.transactions.into_iter();
        let transaction = transactions_iter.next().unwrap();
        let hash: [u8; 32] =
            hex::decode("77b19baa4de67e45a7b26e4a220bccdbb6731885aa9927064e239ca232023215")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(transaction.hash().unwrap(), hash);
        let TransactionEnvelope::Legacy(tx1) = transaction else {
            panic!("invalid tx");
        };

        let gas_price = vec![10u8].try_into().unwrap();
        let to = hex::decode("095e7baea6a6c7c4c2dfeb977efac326af552d87")
            .unwrap()
            .try_into()
            .unwrap();
        let value = vec![10u8].try_into().unwrap();

        assert_eq!(
            tx1,
            TransactionLegacy {
                nonce: 0,
                gas_price,
                gas_limit: 50000,
                to,
                value,
                data: vec![],
                v: vec![27].try_into().unwrap(),
                r: [
                    155, 234, 76, 77, 170, 199, 199, 197, 46, 9, 62, 106, 76, 53, 219, 188, 248,
                    133, 111, 26, 247, 176, 89, 186, 32, 37, 62, 112, 132, 141, 9, 79
                ]
                .into(),
                s: [
                    138, 143, 174, 83, 124, 226, 94, 216, 203, 90, 249, 173, 172, 63, 20, 26, 246,
                    155, 213, 21, 189, 43, 160, 49, 82, 45, 240, 155, 151, 221, 114, 177
                ]
                .into()
            }
        );

        let transaction = transactions_iter.next().unwrap();
        let hash: [u8; 32] =
            hex::decode("554af720acf477830f996f1bc5d11e54c38aa40042aeac6f66cb66f9084a959d")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(transaction.hash().unwrap(), hash);
        let TransactionEnvelope::AccessList(tx2) = transaction else {
            panic!("invalid tx");
        };
        assert_eq!(tx2.chain_id.last().unwrap(), &1);

        let chain_id = vec![1u8].try_into().unwrap();
        let gas_price = vec![10u8].try_into().unwrap();
        let to = {
            let mut arr = [0; 20];
            arr.copy_from_slice(&hex::decode("095e7baea6a6c7c4c2dfeb977efac326af552d87").unwrap());
            arr.into()
        };
        let access_list = {
            let address = {
                let mut arr = [0; 20];
                arr[19] = 1;
                arr.into()
            };
            vec![AccessList {
                address,
                storage_keys: vec![[0; 32].into()],
            }]
        };

        assert_eq!(
            tx2,
            TransactionAccessList {
                chain_id,
                nonce: 0,
                gas_price,
                gas_limit: 123457,
                to,
                value: vec![].try_into().unwrap(),
                data: vec![],
                access_list,
                y_parity: vec![1].try_into().unwrap(),
                r: [
                    61, 186, 204, 141, 2, 89, 242, 80, 134, 37, 233, 127, 223, 197, 124, 216, 95,
                    221, 22, 229, 130, 27, 194, 193, 11, 221, 26, 82, 100, 158, 131, 53
                ]
                .into(),
                s: [
                    71, 110, 16, 105, 91, 24, 58, 135, 176, 170, 41, 42, 127, 75, 120, 239, 12, 63,
                    190, 98, 170, 44, 66, 200, 78, 29, 156, 61, 161, 89, 239, 20
                ]
                .into(),
            }
        );
    }

    #[test]
    fn block_hash() {
        let block_bytes = hex::decode("f90260f901f9a083cafc574e1f51ba9dc0568fc617a08ea2429fb384059c972f13b19fa1c8dd55a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347948888f1f195afa192cfee860698584c030f4c9db1a0ef1552a40b7165c3cd773806b9e0c165b75356e0314bf0706f279c729f51e017a05fe50b260da6308036625b850b5d6ced6d0a9f814c0688bc91ffb7b7a3a54b67a0bc37d79753ad738a6dac4921e57392f145d8887476de3f783dfa7edae9283e52b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008302000001832fefd8825208845506eb0780a0bd4472abb6659ebe3ee06ee4d7b72a00a9f4d001caca51342001075469aff49888a13a5a8c8f2bb1c4f861f85f800a82c35094095e7baea6a6c7c4c2dfeb977efac326af552d870a801ba09bea4c4daac7c7c52e093e6a4c35dbbcf8856f1af7b059ba20253e70848d094fa08a8fae537ce25ed8cb5af9adac3f141af69bd515bd2ba031522df09b97dd72b1c0").unwrap();
        let block: Block = Block::from_bytes(&block_bytes).unwrap();
        let hash: [u8; 32] =
            hex::decode("0a5843ac1cb04865017cb35a57b50b07084e5fcee39b5acadade33149f4fff9e")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(block.hash().unwrap(), hash)
    }

    #[test]
    fn block_serde() {
        let block = Block::default();
        let bytes = rlp_rs::to_bytes(&block).unwrap();
        let block2 = Block::from_bytes(&bytes).unwrap();
        assert_eq!(block, block2);
    }

    #[test]
    fn block_unknown() {
        let block = Block {
            header: Header::Unknown {
                parent_hash: Default::default(),
                uncle_hash: Default::default(),
                coinbase: Default::default(),
                state_root: Default::default(),
                tx_root: Default::default(),
                receipt_hash: Default::default(),
                bloom: Default::default(),
                difficulty: Default::default(),
                number: Default::default(),
                gas_limit: Default::default(),
                gas_used: Default::default(),
                time: Default::default(),
                extra: Default::default(),
                mix_digest: Default::default(),
                nonce: Default::default(),
                rest: vec![vec![10; 2].into(); 10],
            },
            ..Default::default()
        };
        let bytes = rlp_rs::to_bytes(&block).unwrap();
        let block2 = Block::unknown_from_bytes(&bytes).unwrap();
        assert_eq!(block, block2);
    }
}

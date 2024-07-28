#[cfg(feature = "fuzzing")]
use libfuzzer_sys::arbitrary::{self, Arbitrary};
use serde::{Deserialize, Serialize};

pub type Address = [u8; 20];

pub type U256 = [u8; 32];

#[cfg_attr(any(test, feature = "test-utils"), derive(PartialEq))]
#[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerdeU256(#[serde(with = "serde_bytes")] pub U256);

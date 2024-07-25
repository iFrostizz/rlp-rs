use serde::{Deserialize, Serialize};

pub type Address = [u8; 20];

pub type U256 = [u8; 32];

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerdeU256(#[serde(with = "serde_bytes")] U256);

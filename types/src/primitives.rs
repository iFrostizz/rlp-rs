#[cfg(feature = "fuzzing")]
use libfuzzer_sys::arbitrary::{self, Arbitrary};
use serde::{Deserialize, Serialize};

/// Implementation of a newtype struct that contains bytes.
/// It is guaranteed that any conversion that goes `From` this type can be directly casted to an array.
/// If you plan to serialize this type and expect the same structure, please keep it around.
/// This is because with the RLP decoding rules of bytes arrays,
/// [0x00] deserializes to [0x00] and [0x80] deserializes to [].
/// For this reason, we cannot just prepend an array of bytes with zeros because it would
/// make the serialization not equivalent.
macro_rules! vec_type {
    ($name:ident, $size:literal) => {
        #[cfg_attr(feature = "fuzzing", derive(Arbitrary))]
        #[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash)]
        pub struct $name(#[serde(with = "serde_bytes")] Vec<u8>);

        impl $name {
            pub fn last(&self) -> Option<&u8> {
                self.0.last()
            }
        }

        impl From<&[u8; $size]> for $name {
            fn from(value: &[u8; $size]) -> $name {
                $name(value.to_vec())
            }
        }

        impl From<[u8; $size]> for $name {
            fn from(value: [u8; $size]) -> $name {
                $name(value.to_vec())
            }
        }

        #[allow(clippy::from_over_into)]
        impl Into<[u8; $size]> for $name {
            fn into(self) -> [u8; $size] {
                if self.0.len() > $size {
                    unreachable!();
                }

                if self.0.len() != $size {
                    let mut arr = [0; $size];
                    arr[($size - self.0.len())..].copy_from_slice(&self.0);
                    arr
                } else {
                    self.0.try_into().unwrap()
                }
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.0.as_slice()
            }
        }

        #[allow(clippy::from_over_into)]
        impl Into<Vec<u8>> for $name {
            fn into(self) -> Vec<u8> {
                self.0
            }
        }

        impl TryInto<$name> for Vec<u8> {
            type Error = ();

            fn try_into(self) -> Result<$name, Self::Error> {
                if self.len() > $size {
                    return Err(());
                }

                Ok($name(self))
            }
        }
    };
}

vec_type!(Address, 20);
vec_type!(U256, 32);
vec_type!(Bloom, 256);
vec_type!(Nonce, 8);

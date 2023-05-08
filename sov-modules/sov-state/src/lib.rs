mod internal_cache;
mod map;
#[cfg(feature = "native")]
mod prover_storage;
mod scratchpad;
pub mod storage;
#[cfg(feature = "native")]
mod tree_db;
mod utils;
mod value;
mod zk_storage;

#[cfg(test)]
mod state_tests;

pub use first_read_last_write_cache::cache::CacheLog;
pub use map::StateMap;
#[cfg(feature = "native")]
pub use prover_storage::{delete_storage, ProverStorage};
pub use scratchpad::*;
use sovereign_sdk::core::traits::Witness;
use std::{fmt::Display, str};
pub use storage::Storage;
use utils::AlignedVec;
pub use value::StateValue;
pub use zk_storage::ZkStorage;

// A prefix prepended to each key before insertion and retrieval from the storage.
// All the collection types in this crate are backed by the same storage instance, this means that insertions of the same key
// to two different `StorageMaps` would collide with each other. We solve it by instantiating every collection type with a unique
// prefix that is prepended to each key.

#[derive(borsh::BorshDeserialize, borsh::BorshSerialize, Debug, PartialEq, Eq, Clone)]
pub struct Prefix {
    prefix: AlignedVec,
}

impl Display for Prefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let buf = self.prefix.as_ref();
        match str::from_utf8(buf) {
            Ok(s) => {
                write!(f, "{:?}", s)
            }
            Err(_) => {
                write!(f, "0x{}", hex::encode(buf))
            }
        }
    }
}

impl Prefix {
    pub fn new(prefix: Vec<u8>) -> Self {
        Self {
            prefix: AlignedVec::new(prefix),
        }
    }

    pub fn as_aligned_vec(&self) -> &AlignedVec {
        &self.prefix
    }

    pub fn len(&self) -> usize {
        self.prefix.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait StorageSpec {
    type Witness: Witness;
    type Hasher: jmt::SimpleHasher;
}

#[cfg(any(test, feature = "mocks"))]
pub mod mocks {
    use sha2::Sha256;
    use sovereign_sdk::core::types::ArrayWitness;

    use crate::StorageSpec;

    #[derive(Clone)]
    pub struct DefaultStorageSpec;

    impl StorageSpec for DefaultStorageSpec {
        type Witness = ArrayWitness;

        type Hasher = Sha256;
    }
}

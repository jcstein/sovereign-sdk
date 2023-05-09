use crate::{SigVerificationError, Signature};
use borsh::{BorshDeserialize, BorshSerialize};
use ed25519_dalek::ed25519::signature::Signature as DalekSignatureTrait;
use ed25519_dalek::Verifier;
use ed25519_dalek::{PublicKey as DalekPublicKey, Signature as DalekSignature};

// TODO feature gate it in Cargo.toml
#[cfg(feature = "native")]
pub mod private_key {
    use super::{DefaultPublicKey, DefaultSignature};
    use ed25519_dalek::{Keypair, Signer};
    use rand::{CryptoRng, RngCore};

    pub struct DefaultPrivateKey {
        key_pair: Keypair,
    }

    impl DefaultPrivateKey {
        pub fn generate<R>(csprng: &mut R) -> Self
        where
            R: CryptoRng + RngCore,
        {
            Self {
                key_pair: Keypair::generate(csprng),
            }
        }

        pub fn sign(&self, msg: [u8; 32]) -> DefaultSignature {
            DefaultSignature {
                msg_sig: self.key_pair.sign(&msg),
            }
        }

        pub fn pub_key(&self) -> DefaultPublicKey {
            DefaultPublicKey {
                pub_key: self.key_pair.public,
            }
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct DefaultPublicKey {
    pub(crate) pub_key: DalekPublicKey,
}

impl BorshDeserialize for DefaultPublicKey {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        todo!()
    }
}

impl BorshSerialize for DefaultPublicKey {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(self.pub_key.as_bytes())
    }
}

impl DefaultPublicKey {
    pub fn new(pub_key: Vec<u8>) -> Self {
        //Self { pub_key }
        todo!()
    }
}

impl<T: AsRef<str>> From<T> for DefaultPublicKey {
    fn from(key: T) -> Self {
        todo!()
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct DefaultSignature {
    pub msg_sig: DalekSignature,
}

impl BorshDeserialize for DefaultSignature {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        todo!()
    }
}

impl BorshSerialize for DefaultSignature {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(self.msg_sig.as_bytes())
    }
}

impl Signature for DefaultSignature {
    type PublicKey = DefaultPublicKey;

    fn verify(
        &self,
        pub_key: &Self::PublicKey,
        msg_hash: [u8; 32],
    ) -> Result<(), SigVerificationError> {
        pub_key
            .pub_key
            .verify(&msg_hash, &self.msg_sig)
            .map_err(|_| SigVerificationError::BadSignature)
    }
}

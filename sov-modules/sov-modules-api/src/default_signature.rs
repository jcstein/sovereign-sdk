use crate::{SigVerificationError, Signature};
use borsh::{BorshDeserialize, BorshSerialize};
use ed25519_dalek::ed25519::signature::Signature as DalekSignatureTrait;
use ed25519_dalek::Verifier;
use ed25519_dalek::{PublicKey as DalekPublicKey, Signature as DalekSignature};

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

    //pub fn sign(&self, _msg: [u8; 32]) -> DefaultSignature {
    //    DefaultSignature { msg_sig: todo!() }
    // }
}

impl<T: AsRef<str>> From<T> for DefaultPublicKey {
    fn from(key: T) -> Self {
        let key = key.as_ref().as_bytes().to_vec();
        //Self { pub_key: key }
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

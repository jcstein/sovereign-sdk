use crate::{
    default_signature::{private_key::DefaultPrivateKey, DefaultPublicKey, DefaultSignature},
    Signature,
};
use borsh::{BorshDeserialize, BorshSerialize};

#[test]
fn test_account_bech32_display() {
    let expected_addr: Vec<u8> = (1..=32).collect();
    let account = crate::AddressBech32::try_from(expected_addr.as_slice()).unwrap();
    assert_eq!(
        account.to_string(),
        "sov1qypqxpq9qcrsszg2pvxq6rs0zqg3yyc5z5tpwxqergd3c8g7rusq4vrkje"
    );
}

#[test]
fn test_address_conversion() {
    let expected_addr: Vec<u8> = (1..=32).collect();
    let addr = crate::Address::try_from(expected_addr.as_slice()).unwrap();

    let addr_bytes: &[u8] = addr.as_ref();
    assert_eq!(expected_addr, addr_bytes);

    let addr_from_bytes = crate::Address::try_from(addr_bytes).unwrap();
    assert_eq!(addr_from_bytes, addr);
}

#[test]
fn test_pub_key_serialization() {
    let pub_key = DefaultPrivateKey::generate().pub_key();
    let serialized_pub_key = pub_key.try_to_vec().unwrap();

    let deserialized_pub_key = DefaultPublicKey::try_from_slice(&serialized_pub_key).unwrap();
    assert_eq!(pub_key, deserialized_pub_key)
}

#[test]
fn test_signature_serialization() {
    let msg = [1; 32];
    let priv_key = DefaultPrivateKey::generate();

    let sig = priv_key.sign(msg);
    let serialized_sig = sig.try_to_vec().unwrap();
    let deserialized_sig = DefaultSignature::try_from_slice(&serialized_sig).unwrap();
    assert_eq!(sig, deserialized_sig);

    let pub_key = priv_key.pub_key();
    deserialized_sig.verify(&pub_key, msg).unwrap()
}

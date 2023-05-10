use super::ValueSetter;
use crate::{call, query};

use sov_modules_api::default_context::{DefaultContext, ZkDefaultContext};
use sov_modules_api::default_signature::private_key::DefaultPrivateKey;
use sov_modules_api::default_signature::DefaultPublicKey;
use sov_modules_api::{Address, Context, PublicKey, Spec};
use sov_modules_api::{Module, ModuleInfo};
use sov_state::{ProverStorage, WorkingSet, ZkStorage};
use sovereign_sdk::stf::Event;

#[test]
fn test_value_setter() {
    let sender_pub_key = DefaultPrivateKey::generate().pub_key();
    let sender = sender_pub_key.to_address::<<DefaultContext as Spec>::Address>();

    let mut working_set = WorkingSet::new(ProverStorage::temporary());

    // Test Native-Context
    {
        let context = DefaultContext::new(sender.clone());
        test_value_setter_helper(context, &sender_pub_key, &mut working_set);
    }

    let (_, witness) = working_set.freeze();

    // Test Zk-Context
    {
        let zk_context = ZkDefaultContext::new(sender);
        let mut zk_working_set = WorkingSet::with_witness(ZkStorage::new([0u8; 32]), witness);
        test_value_setter_helper(zk_context, &sender_pub_key, &mut zk_working_set);
    }
}

fn test_value_setter_helper<C: Context>(
    context: C,
    sender_pub_key: &C::PublicKey,
    working_set: &mut WorkingSet<C::Storage>,
) {
    let module = ValueSetter::<C>::new();
    module.genesis(sender_pub_key, working_set).unwrap();

    let new_value = 99;
    let call_msg = call::CallMessage::SetValue(new_value);

    // Test events
    {
        let call_response = module.call(call_msg, &context, working_set).unwrap();
        let event = &call_response.events[0];
        assert_eq!(event, &Event::new("set", "value_set: 99"));
    }

    let query_msg = query::QueryMessage::GetValue;
    let query = module.query(query_msg, working_set);

    // Test query
    {
        let query_response: Result<query::Response, _> = serde_json::from_slice(&query.response);

        assert_eq!(
            query::Response {
                value: Some(new_value)
            },
            query_response.unwrap()
        )
    }
}

#[test]
fn test_err_on_sender_is_not_admin() {
    let sender_pub_key = DefaultPrivateKey::generate().pub_key();
    let sender = sender_pub_key.to_address::<<DefaultContext as Spec>::Address>();

    let backing_store = ProverStorage::temporary();
    let native_working_set = &mut WorkingSet::new(backing_store);

    // Test Native-Context
    let sender_not_admin_pub_key = DefaultPrivateKey::generate().pub_key();

    {
        let context = DefaultContext::new(sender.clone());
        test_err_on_sender_is_not_admin_helper(
            context,
            &sender_not_admin_pub_key,
            native_working_set,
        );
    }
    let (_, witness) = native_working_set.freeze();

    // Test Zk-Context
    {
        let zk_backing_store = ZkStorage::new([0u8; 32]);
        let zk_context = ZkDefaultContext::new(sender);
        let zk_working_set = &mut WorkingSet::with_witness(zk_backing_store, witness);
        test_err_on_sender_is_not_admin_helper(
            zk_context,
            &sender_not_admin_pub_key,
            zk_working_set,
        );
    }
}

fn test_err_on_sender_is_not_admin_helper<C: Context>(
    context: C,
    sender_pub_key: &C::PublicKey,
    working_set: &mut WorkingSet<C::Storage>,
) {
    let module = ValueSetter::<C>::new();
    module.genesis(sender_pub_key, working_set).unwrap();
    let resp = module.set_value(11, &context, working_set);

    assert!(resp.is_err());
}

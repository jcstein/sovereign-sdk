use super::{
    call::CallMessage,
    query::{GetResultResponse, QueryMessage},
    types::Candidate,
    Election,
};
use sov_modules_api::Address;

use sov_modules_api::{
    default_context::{DefaultContext, ZkDefaultContext},
    default_signature::{private_key::DefaultPrivateKey, DefaultPublicKey},
    Context, Module, ModuleInfo, PublicKey,
};
use sov_state::{ProverStorage, WorkingSet, ZkStorage};

#[test]
fn test_election() {
    let admin = Address::from([1; 32]);

    let native_storage = ProverStorage::temporary();
    let mut native_working_set = WorkingSet::new(native_storage);

    test_module::<DefaultContext>(admin.clone(), &mut native_working_set);

    let (_log, witness) = native_working_set.freeze();
    let zk_storage = ZkStorage::new([0u8; 32]);
    let mut zk_working_set = WorkingSet::with_witness(zk_storage, witness);
    test_module::<ZkDefaultContext>(admin, &mut zk_working_set);
}

fn test_module<C: Context>(admin: C::Address, working_set: &mut WorkingSet<C::Storage>) {
    let admin_context = C::new(admin.clone());
    let election = &mut Election::<C>::new();

    // Init module
    {
        election.genesis(&admin, working_set).unwrap();
    }

    // Send candidates
    {
        let set_candidates = CallMessage::SetCandidates {
            names: vec!["candidate_1".to_owned(), "candidate_2".to_owned()],
        };

        election
            .call(set_candidates, &admin_context, working_set)
            .unwrap();
    }

    let voter_1 = DefaultPrivateKey::generate()
        .pub_key()
        .to_address::<C::Address>();

    let voter_2 = DefaultPrivateKey::generate()
        .pub_key()
        .to_address::<C::Address>();

    let voter_3 = DefaultPrivateKey::generate()
        .pub_key()
        .to_address::<C::Address>();

    // Register voters
    {
        let add_voter = CallMessage::AddVoter(voter_1.clone());
        election
            .call(add_voter, &admin_context, working_set)
            .unwrap();

        let add_voter = CallMessage::AddVoter(voter_2.clone());
        election
            .call(add_voter, &admin_context, working_set)
            .unwrap();

        let add_voter = CallMessage::AddVoter(voter_3.clone());
        election
            .call(add_voter, &admin_context, working_set)
            .unwrap();
    }

    // Vote
    {
        let sender_context = C::new(voter_1);
        let vote = CallMessage::Vote(0);
        election.call(vote, &sender_context, working_set).unwrap();

        let sender_context = C::new(voter_2);
        let vote = CallMessage::Vote(1);
        election.call(vote, &sender_context, working_set).unwrap();

        let sender_context = C::new(voter_3);
        let vote = CallMessage::Vote(1);
        election.call(vote, &sender_context, working_set).unwrap();
    }

    election
        .call(CallMessage::FreezeElection, &admin_context, working_set)
        .unwrap();

    // Get result
    {
        let query = QueryMessage::GetResult;
        let query = election.query(query, working_set);
        let query_response: GetResultResponse = serde_json::from_slice(&query.response).unwrap();

        assert_eq!(
            query_response,
            GetResultResponse::Result(Some(Candidate {
                name: "candidate_2".to_owned(),
                count: 2
            }))
        )
    }
}

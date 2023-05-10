mod batch;
mod tx_hooks;
mod tx_verifier;

pub use batch::Batch;
use sovereign_sdk::serial::Decode;
use sovereign_sdk::stf::BatchReceipt;
use sovereign_sdk::stf::TransactionReceipt;
use sovereign_sdk::Buf;
use tracing::error;
pub use tx_hooks::TxHooks;
pub use tx_hooks::VerifiedTx;
pub use tx_verifier::{RawTx, TxVerifier};

use sov_modules_api::{Context, DispatchCall, Genesis, Spec};
use sov_state::{Storage, WorkingSet};
use sovereign_sdk::{
    core::traits::BatchTrait,
    jmt,
    stf::{OpaqueAddress, StateTransitionFunction},
};

pub struct AppTemplate<C: Context, V, RT, H> {
    pub current_storage: C::Storage,
    pub runtime: RT,
    tx_verifier: V,
    tx_hooks: H,
    working_set: Option<WorkingSet<C::Storage>>,
}

impl<C: Context, V, RT, H> AppTemplate<C, V, RT, H>
where
    RT: DispatchCall<Context = C> + Genesis<Context = C>,
    V: TxVerifier,
    H: TxHooks<Context = C, Transaction = <V as TxVerifier>::Transaction>,
{
    pub fn new(storage: C::Storage, runtime: RT, tx_verifier: V, tx_hooks: H) -> Self {
        Self {
            runtime,
            current_storage: storage,
            tx_verifier,
            tx_hooks,
            working_set: None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TxEffect {
    Reverted,
    Successful,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SequencerOutcome {
    Rewarded,
    Slashed(SlashingReason),
    Ignored,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SlashingReason {
    InvalidBatchEncoding,
    StatelessVerificationFailed,
    InvalidTransactionEncoding,
}

impl<C: Context, V, RT, H> StateTransitionFunction for AppTemplate<C, V, RT, H>
where
    RT: DispatchCall<Context = C> + Genesis<Context = C>,
    V: TxVerifier,
    H: TxHooks<Context = C, Transaction = <V as TxVerifier>::Transaction>,
{
    type StateRoot = jmt::RootHash;

    type InitialState = <RT as Genesis>::Config;

    type TxReceiptContents = TxEffect;

    type BatchReceiptContents = SequencerOutcome;

    type Witness = <<C as Spec>::Storage as Storage>::Witness;

    type MisbehaviorProof = ();

    fn init_chain(&mut self, params: Self::InitialState) {
        let working_set = &mut WorkingSet::new(self.current_storage.clone());

        self.runtime
            .genesis(&params, working_set)
            .expect("module initialization must succeed");

        let (log, witness) = working_set.freeze();
        self.current_storage
            .validate_and_commit(log, &witness)
            .expect("Storage update must succeed");
    }

    fn begin_slot(&mut self, witness: Self::Witness) {
        self.working_set = Some(WorkingSet::with_witness(
            self.current_storage.clone(),
            witness,
        ));
    }

    // TODO: implement a state machine instead of manually deciding when to commit and when to revert
    fn apply_blob(
        &mut self,
        blob: impl sovereign_sdk::da::BlobTransactionTrait,
        _misbehavior_hint: Option<Self::MisbehaviorProof>,
    ) -> BatchReceipt<Self::BatchReceiptContents, Self::TxReceiptContents> {
        let mut batch_workspace = self
            .working_set
            .take()
            .expect("Working_set was initialized in begin_slot")
            .to_revertable();

        let sequencer = blob.sender();
        let sequencer = sequencer.as_ref();

        println!("1");

        if let Err(e) = self
            .tx_hooks
            .enter_apply_blob(sequencer, &mut batch_workspace)
        {
            error!(
                "Error: The transaction was rejected by the 'enter_apply_blob' hook. Skipping batch without slashing the sequencer {}",
                e
            );
            self.working_set = Some(batch_workspace.revert());
            // TODO: consider slashing the sequencer in this case. cc @bkolad
            return BatchReceipt {
                batch_hash: [0u8; 32], // TODO: calculate the hash using Context::Hasher;
                tx_receipts: Vec::new(),
                inner: SequencerOutcome::Ignored,
            };
        }

        println!("2");

        // Commit `enter_apply_batch` changes.
        batch_workspace = batch_workspace.commit().to_revertable();

        // let batch: Vec<u8> = blob.data().collect();
        let batch = match Batch::decode(&mut blob.data().reader()) {
            Ok(batch) => batch,
            Err(e) => {
                error!("Unable to decode batch provided by the sequencer {}", e);
                self.working_set = Some(batch_workspace.revert());
                return BatchReceipt {
                    batch_hash: [0u8; 32], // TODO: calculate the hash using Context::Hasher;
                    tx_receipts: Vec::new(),
                    inner: SequencerOutcome::Slashed(SlashingReason::InvalidBatchEncoding),
                };
            }
        };

        println!("3");
        // Run the stateless verification, since it is stateless we don't commit.
        let txs = match self
            .tx_verifier
            .verify_txs_stateless(batch.take_transactions())
        {
            Ok(txs) => txs,
            Err(e) => {
                println!("Error {:?}", e);
                // Revert on error
                let batch_workspace = batch_workspace.revert();
                self.working_set = Some(batch_workspace.revert());
                error!("Stateless verification error - the sequencer included a transaction which was known to be invalid. {}", e);
                return BatchReceipt {
                    batch_hash: [0u8; 32], // TODO: calculate the hash using Context::Hasher;
                    tx_receipts: Vec::new(),
                    inner: SequencerOutcome::Slashed(SlashingReason::StatelessVerificationFailed),
                };
            }
        };

        println!("4");

        let mut tx_receipts = Vec::with_capacity(txs.len());

        // Process transactions in a loop, commit changes after every step of the loop.
        for tx in txs {
            batch_workspace = batch_workspace.to_revertable();
            // Run the stateful verification, possibly modifies the state.
            let verified_tx = match self.tx_hooks.pre_dispatch_tx_hook(tx, &mut batch_workspace) {
                Ok(verified_tx) => verified_tx,
                Err(e) => {
                    // // Revert the batch.
                    // batch_workspace = batch_workspace.revert();

                    // // We reward sequencer funds inside `exit_apply_batch`.
                    // self.tx_hooks
                    //     .exit_apply_batch(0, &mut batch_workspace)
                    //     .expect("Impossible happened: error in exit_apply_batch");

                    // self.working_set = Some(batch_workspace);

                    // Don't revert any state changes made by the pre_dispatch_hook even if it rejects
                    error!("Stateful verification error - the sequencer included an invalid transaction: {}", e);
                    batch_workspace = batch_workspace.revert();
                    let receipt = TransactionReceipt {
                        tx_hash: [0u8; 32],
                        body_to_save: None,
                        events: vec![],
                        receipt: TxEffect::Reverted,
                    };
                    tx_receipts.push(receipt);
                    continue;
                }
            };

            match RT::decode_call(verified_tx.runtime_message()) {
                Ok(msg) => {
                    let ctx = C::new(verified_tx.sender().clone());
                    let tx_result = self.runtime.dispatch_call(msg, &mut batch_workspace, &ctx);

                    self.tx_hooks
                        .post_dispatch_tx_hook(verified_tx, &mut batch_workspace);

                    match tx_result {
                        Ok(resp) => {
                            let receipt = TransactionReceipt {
                                tx_hash: [0u8; 32], // TODO: calculate the hash using Context::Hasher;
                                body_to_save: None,
                                events: resp.events,
                                receipt: TxEffect::Successful,
                            };

                            tx_receipts.push(receipt);
                        }
                        Err(_e) => {
                            // The transaction causing invalid state transition is reverted but we don't slash and we continue
                            // processing remaining transactions.
                            batch_workspace = batch_workspace.revert();
                        }
                    }
                }
                Err(e) => {
                    // If the serialization is invalid, the sequencer is malicious. Slash them (we don't run exit_apply_batch here)
                    let batch_workspace = batch_workspace.revert();
                    self.working_set = Some(batch_workspace);
                    error!("Tx decoding error: {}", e);
                    return BatchReceipt {
                        batch_hash: [0u8; 32], // TODO: calculate the hash using Context::Hasher;
                        tx_receipts: Vec::new(),
                        inner: SequencerOutcome::Slashed(
                            SlashingReason::InvalidTransactionEncoding,
                        ),
                    };
                }
            }
            // commit each step of the loop
            batch_workspace = batch_workspace.commit();
        }

        // TODO: calculate the amount based of gas and fees
        self.tx_hooks
            .exit_apply_blob(0, &mut batch_workspace)
            .expect("Impossible happened: error in exit_apply_batch");

        self.working_set = Some(batch_workspace);
        BatchReceipt {
            batch_hash: [0u8; 32], // TODO: calculate the hash using Context::Hasher;
            tx_receipts,
            inner: SequencerOutcome::Rewarded,
        }
    }

    fn end_slot(
        &mut self,
    ) -> (
        Self::StateRoot,
        Self::Witness,
        Vec<sovereign_sdk::stf::ConsensusSetUpdate<OpaqueAddress>>,
    ) {
        let (cache_log, witness) = self.working_set.take().unwrap().freeze();
        let root_hash = self
            .current_storage
            .validate_and_commit(cache_log, &witness)
            .expect("jellyfish merkle tree update must succeed");
        (jmt::RootHash(root_hash), witness, vec![])
    }
}

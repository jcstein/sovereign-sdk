use crate::traits::TransactionTrait;
use crate::{da::BlobTransactionTrait, zk::traits::Zkvm};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// The configuration of a full node of the rollup which creates zk proofs.
pub struct ProverConfig;
/// The configuration used to initialize the "Verifier" of the state transition function
/// which runs inside of the zkvm.
pub struct ZkConfig;
/// The configuration of a standard full node of the rollup which does not create zk proofs
pub struct StandardConfig;

pub trait StateTransitionConfig: sealed::Sealed {}
impl StateTransitionConfig for ProverConfig {}
impl StateTransitionConfig for ZkConfig {}
impl StateTransitionConfig for StandardConfig {}

// https://rust-lang.github.io/api-guidelines/future-proofing.html
mod sealed {
    use super::{ProverConfig, StandardConfig, ZkConfig};

    pub trait Sealed {}
    impl Sealed for ProverConfig {}
    impl Sealed for ZkConfig {}
    impl Sealed for StandardConfig {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt<R> {
    /// The canonical hash of this transaction
    pub tx_hash: [u8; 32],
    /// The canonically serialized body of the transaction, if it should be persisted
    /// in the database
    pub body_to_save: Option<Vec<u8>>,
    /// The events output by this transaction
    pub events: Vec<Event>,
    /// Any additional structured data to be saved in the database and served over RPC
    /// For example, this might contain a status code.
    pub receipt: R,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchReceipt<BatchReceiptContents, TxReceiptContents> {
    /// The canonical hash of this batch
    pub batch_hash: [u8; 32],
    /// The receipt of each transaction in the batch
    pub tx_receipts: Vec<TransactionReceipt<TxReceiptContents>>,
    /// Any additional structured data to be saved in the database and served over RPC
    pub inner: BatchReceiptContents,
}

// TODO(@preston-evans98): update spec with simplified API
/// State transition function defines business logic that responsible for changing state.
/// Terminology:
///  - state root: root hash of state merkle tree
///  - block: DA layer block
///  - batch: Set of transactions grouped together, or block on L2
///  - blob: Non serialised batch
pub trait StateTransitionFunction<Vm: Zkvm> {
    /// Root hash of state merkle tree
    type StateRoot;
    /// The initial state of the rollup.
    type InitialState;

    /// The contents of a transaction receipt. This is the data that is persisted in the database
    type TxReceiptContents: Serialize + DeserializeOwned + Clone;
    /// The contents of a batch receipt. This is the data that is persisted in the database
    type BatchReceiptContents: Serialize + DeserializeOwned + Clone;

    /// Witness is a data that is produced during actual batch execution
    /// or validated together with proof during verification
    type Witness: Default + Serialize;

    /// A proof that the sequencer has misbehaved. For example, this could be a merkle proof of a transaction
    /// with an invalid signature
    type MisbehaviorProof;

    /// Perform one-time initialization for the genesis block.
    fn init_chain(&mut self, params: Self::InitialState);

    /// Called at the beginning of each **DA-layer block** - whether or not that block contains any
    /// data relevant to the rollup.
    /// If slot is started in Full Node mode, default witness should be provided.
    /// If slot is started in Zero Knowledge mode, witness from execution should be provided.
    fn begin_slot(&mut self, witness: Self::Witness);

    /// Apply a blob/batch of transactions to the rollup, slashing the sequencer who proposed the blob on failure.
    /// The concrete blob type is defined by the DA layer implementation, which is why we use a generic here instead
    /// of an associated type.
    /// Misbehavior hint allows prover optimizations - the sequencer can be slashed
    /// for including a transaction which fails stateless checks (i.e. has an invalid signature) -
    /// and in that case we ignore his entire batch.
    /// This method lets you give a hint to the prover telling
    /// it where that invalid signature is, so that it can skip signature checks on other transactions.
    /// (If the misbehavior hint is wrong, then the host is malicious so we can
    /// just panic - which means that no proof will be created).
    fn apply_blob(
        &mut self,
        blob: impl BlobTransactionTrait,
        misbehavior_hint: Option<Self::MisbehaviorProof>,
    ) -> BatchReceipt<Self::BatchReceiptContents, Self::TxReceiptContents>;

    /// Called once at the *end* of each DA layer block (i.e. after all rollup blobs have been processed)
    /// Commits state changes to the database
    ///
    fn end_slot(&mut self) -> (Self::StateRoot, Self::Witness);
}

/// A key-value pair representing a change to the rollup state
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct Event {
    key: EventKey,
    value: EventValue,
}

impl Event {
    pub fn new(key: &str, value: &str) -> Self {
        Self {
            key: EventKey(key.as_bytes().to_vec()),
            value: EventValue(value.as_bytes().to_vec()),
        }
    }

    pub fn key(&self) -> &EventKey {
        &self.key
    }

    pub fn value(&self) -> &EventValue {
        &self.value
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
)]
pub struct EventKey(Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct EventValue(Vec<u8>);

pub trait BlockBuilder {
    // Mempool is internal

    // Return something to client
    fn accept_tx(&self, tx: Vec<u8>);

    //
    fn get_best_blob(&self) -> Vec<u8>;
}

/// A StateTransitionRunner (STR) is responsible for running the state transition function. For any particular function,
/// you might have a few different STRs, each with different runtime configs. For example, you might have a STR which takes
/// a path to a data directory as a runtime config, and another which takes a pre-built in-memory database.
///
/// Using a separate trait for initialization makes it easy to store extra data in the STR, which
/// would not fit neatly in the state transition logic itself (such as a handle to the database).
/// This way, you can easily support ancillary functions like RPC, p2p networking etc in your full node implementation
///
///
/// The StateTransitionRunner is generic over a StateTransitionConfig, and a Zkvm. The ZKvm is simply forwarded to the inner STF.
/// StateTransitionConfig is a special marker trait which has only 3 possible instantiations:  ProverConfig, NativeConfig, and ZkConfig.
/// This Config makes it easy to implement different instantiations of STR on the same struct, which are appropriate for different
/// modes of execution.
///
/// For example: might have `impl StateTransitionRunner<ProverConfig, Vm> for MyRunner` which takes a path to a data directory as a runtime config,
///
/// and a `impl StateTransitionRunner<ZkConfig, Vm> for MyRunner` which instead uses a state root as its runtime config.
///
// TODO: Move to node section
pub trait StateTransitionRunner<T: StateTransitionConfig, Vm: Zkvm> {
    /// The parameters of the state transition function which are set at runtime. For example,
    /// the runtime config might contain path to a data directory.
    type RuntimeConfig;
    type Inner: StateTransitionFunction<Vm>;
    type BlockBuilder: BlockBuilder; //

    // TODO: decide if `new` also requires <Self as StateTransitionFunction>::ChainParams as an argument
    /// Create a state transition runner
    fn new(runtime_config: Self::RuntimeConfig) -> Self;

    /// Return a reference to the inner STF implementation
    fn inner(&self) -> &Self::Inner;

    /// Return a mutable reference to the inner STF implementation
    fn inner_mut(&mut self) -> &mut Self::Inner;

    // /// Report if the state transition function has been initialized.
    // /// If not, node implementations should respond by running `init_chain`
    // fn has_been_initialized(&self) -> bool;

    fn get_next_block(&self);

    fn accept_tx(&self);
}

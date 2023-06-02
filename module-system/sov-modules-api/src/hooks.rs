use crate::{transaction::Transaction, Context, Spec};
use sov_state::WorkingSet;

/// Hooks that execute within the `StateTransitionFunction::apply_blob` function for each processed transaction.
pub trait TxHooks {
    type Context: Context;

    /// Runs just before a transaction is dispatched to an appropriate module.
    /// TODO: Why does it return address?
    /// Does it implies that it should do signature verification.
    /// Can other code rely on that assumption?
    fn pre_dispatch_tx_hook(
        &self,
        tx: Transaction<Self::Context>,
        working_set: &mut WorkingSet<<Self::Context as Spec>::Storage>,
    ) -> anyhow::Result<<Self::Context as Spec>::Address>;

    /// Runs after the tx is dispatched to an appropriate module.
    fn post_dispatch_tx_hook(
        &self,
        tx: &Transaction<Self::Context>,
        working_set: &mut WorkingSet<<Self::Context as Spec>::Storage>,
    ) -> anyhow::Result<()>;
}

/// Hooks related to the Sequencer functionality.
/// In essence, the sequencer locks a bond at the beginning of the `StateTransitionFunction::apply_blob`,
/// and is rewarded once a blob of transactions is processed.
pub trait ApplyBlobHooks {
    type Context: Context;
    type BlobResult;

    /// Runs at the beginning of apply_blob, locks the sequencer bond.
    fn begin_blob_hook(
        &self,
        sequencer: &[u8],
        raw_blob: &[u8],
        working_set: &mut WorkingSet<<Self::Context as Spec>::Storage>,
    ) -> anyhow::Result<()>;

    /// Executes at the end of apply_blob and rewards the sequencer. This method is not invoked if the sequencer has been slashed.
    fn end_blob_hook(
        &self,
        result: Self::BlobResult,
        working_set: &mut WorkingSet<<Self::Context as Spec>::Storage>,
    ) -> anyhow::Result<()>;
}

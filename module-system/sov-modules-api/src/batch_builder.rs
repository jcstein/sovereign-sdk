use sov_state::{Storage, WorkingSet};
use crate::{Context, Spec};

/// BlockBuilder trait is responsible for managing mempool and building batches.
pub trait BatchBuilder<S: Storage> {
    type Context: Context;
    /// Accept a new transaction.
    /// Can return error if transaction is invalid or mempool is full.
    fn accept_tx(&self, tx: Vec<u8>) -> anyhow::Result<()>;

    /// Builds a new batch out of transactions in mempool.
    /// Working set is consumed, to emphasize that it is not is not going to be used after this call.
    /// TODO: Do we want to return next_root_hash for the batch?
    fn get_next_blob(&self, working_set: WorkingSet<<Self::Context as Spec>::Storage>) -> anyhow::Result<Vec<Vec<u8>>>;

    // TODO: Eviction events?
}

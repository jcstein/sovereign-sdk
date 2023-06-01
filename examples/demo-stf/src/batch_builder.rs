use anyhow::bail;
use borsh::BorshDeserialize;
use sov_modules_api::transaction::Transaction;
use sov_modules_api::{Context, DispatchCall, PublicKey, Spec};
use sov_rollup_interface::services::batch_builder::BatchBuilder;
use sov_state::WorkingSet;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Cursor;

// pub struct FiFoSimpleBatchBuilder {
//     mempool: RefCell<VecDeque<Vec<u8>>>,
// }
//
// impl FiFoSimpleBatchBuilder {
//     pub fn new() -> Self {
//         Self {
//             mempool: RefCell::new(VecDeque::new()),
//         }
//     }
// }
// ///
//
// impl BatchBuilder for FiFoSimpleBatchBuilder {
//     fn accept_tx(&self, tx: Vec<u8>) -> anyhow::Result<()> {
//         let mut mempool = self.mempool.borrow_mut();
//         mempool.push_back(tx);
//         Ok(())
//     }
//
//     fn get_next_blob(&self) -> anyhow::Result<Vec<Vec<u8>>> {
//         todo!()
//     }
// }

/// Strict
pub struct FiFoStrictBatchBuilder<R, C: Context> {
    mempool: RefCell<VecDeque<Vec<u8>>>,
    runtime: R, // TODO: ?? Particular runtime.
    batch_size: usize,
    working_set: Option<RefCell<WorkingSet<<C as Spec>::Storage>>>,
}

impl<R, C: Context> FiFoStrictBatchBuilder<R, C> {
    pub fn new(batch_size: usize, runtime: R) -> Self {
        Self {
            mempool: RefCell::new(VecDeque::new()),
            batch_size,
            runtime,
            working_set: None,
        }
    }

    pub fn reset_working_set(&mut self, working_set: WorkingSet<<C as Spec>::Storage>) {
        self.working_set = Some(RefCell::new(working_set));
    }
}

impl<R, C: Context> BatchBuilder for FiFoStrictBatchBuilder<R, C>
where
    R: DispatchCall<Context = C>,
{
    /// Transaction can only be declined only mempool is full
    fn accept_tx(&self, tx: Vec<u8>) -> anyhow::Result<()> {
        // TODO: Hold 100 txs of any size, implement size based logic later
        if self.mempool.borrow().len() > 100 {
            anyhow::bail!("Mempool is full")
        }
        let mut mempool = self.mempool.borrow_mut();
        mempool.push_back(tx);
        Ok(())
    }

    /// Builds a new batch of valid transactions in order they were added to mempool
    /// Only transactions, which are dispatched successfully are included in the batch
    fn get_next_blob(&self) -> anyhow::Result<Vec<Vec<u8>>> {
        let working_set = match self.working_set.as_ref() {
            None => {
                bail!("Cannot build batch before working set is initialized");
            }
            Some(working_set) => working_set,
        };
        let mut txs = Vec::new();
        let mut dismissed: Vec<(Vec<u8>, anyhow::Error)> = Vec::new();
        let mut current_size = 0;
        let mut mempool = self.mempool.borrow_mut();

        while let Some(raw_tx) = mempool.pop_front() {
            // Check batch size
            let tx_size = raw_tx.len();
            if current_size + tx_size > self.batch_size {
                mempool.push_front(raw_tx);
                break;
            }
            current_size += tx_size;

            // Deserialize
            let mut data = Cursor::new(&raw_tx);
            let tx = match Transaction::<C>::deserialize_reader(&mut data) {
                Ok(tx) => tx,
                Err(err) => {
                    let err = anyhow::Error::new(err).context("Failed to deserialize transaction");
                    dismissed.push((raw_tx, err));
                    continue;
                }
            };

            // Verify
            if let Err(err) = tx.verify() {
                dismissed.push((raw_tx, err));
                continue;
            }

            // Decode
            // tx.estimate_fees();
            let msg = match R::decode_call(tx.runtime_msg()) {
                Ok(msg) => msg,
                Err(err) => {
                    let err =
                        anyhow::Error::new(err).context("Failed to decode message in transaction");
                    dismissed.push((raw_tx, err));
                    continue;
                }
            };

            // Execute
            {
                let sender_address: C::Address = tx.pub_key().to_address();
                let ctx = C::new(sender_address);
                let mut working_set = working_set.borrow_mut();
                match self.runtime.dispatch_call(msg, &mut working_set, &ctx) {
                    Ok(_) => {
                        txs.push(raw_tx);
                    }
                    Err(err) => {
                        let err = anyhow::Error::new(err)
                            .context("Transaction dispatch returned an error");
                        dismissed.push((raw_tx, err));
                        continue;
                    }
                }
            }
        }

        Ok(txs)
    }
}

#[cfg(test)]
mod tests {

    mod accept_tx {

        #[test]
        #[ignore = "TBD"]
        fn accept_tx_normal() {}

        #[test]
        #[ignore = "TBD"]
        fn decline_tx_on_full_mempool() {}
    }

    mod build_batch {}
}

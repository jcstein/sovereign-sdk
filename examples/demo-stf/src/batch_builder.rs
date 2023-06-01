use borsh::BorshDeserialize;
use sov_modules_api::batch_builder::BatchBuilder;
use sov_modules_api::hooks::TxHooks;
use sov_modules_api::transaction::Transaction;
use sov_modules_api::{Context, DispatchCall, Spec};
use sov_state::{Storage, WorkingSet};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Cursor;

pub struct FiFoBatchBuilder<R> {
    mempool: RefCell<VecDeque<Vec<u8>>>,
    runtime: R,
    batch_size: usize,
}

impl<R> FiFoBatchBuilder<R> {
    fn new(batch_size: usize, runtime: R) -> Self {
        Self {
            mempool: RefCell::new(VecDeque::new()),
            batch_size,
            runtime,
        }
    }
}

impl<S: Storage, R, C: Context> BatchBuilder<S> for FiFoBatchBuilder<R>
where
    R: DispatchCall<Context = C> + TxHooks<Context = C>,
{
    type Context = C;

    fn accept_tx(&self, tx: Vec<u8>) -> anyhow::Result<()> {
        let mut mempool = self.mempool.borrow_mut();
        mempool.push_back(tx);
        Ok(())
    }

    fn get_next_blob(
        &self,
        mut working_set: WorkingSet<<Self::Context as Spec>::Storage>,
    ) -> anyhow::Result<Vec<Vec<u8>>> {
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

            let sender_address = match self
                .runtime
                .pre_dispatch_tx_hook(tx.clone(), &mut working_set)
            {
                Ok(sender_address) => sender_address,
                Err(err) => {
                    dismissed.push((raw_tx, err));
                    continue;
                }
            };

            // Decode
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
            let ctx = C::new(sender_address.clone());
            match self.runtime.dispatch_call(msg, &mut working_set, &ctx) {
                Ok(_) => {
                    txs.push(raw_tx);
                }
                Err(err) => {
                    let err =
                        anyhow::Error::new(err).context("Transaction dispatch returned an error");
                    dismissed.push((raw_tx, err));
                    continue;
                }
            }
        }

        Ok(txs)
    }
}

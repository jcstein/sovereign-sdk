use anyhow::bail;
use borsh::BorshDeserialize;
use sov_modules_api::transaction::Transaction;
use sov_modules_api::{Context, DispatchCall, PublicKey, Spec};
use sov_rollup_interface::services::batch_builder::BatchBuilder;
use sov_state::WorkingSet;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Cursor;

const MAX_TX_POOL_SIZE: usize = 100;

/// BatchBuilder that creates batch of transaction in the order they were submitted
/// Only transaction that were successfully dispatched are included.
pub struct FiFoStrictBatchBuilder<R, C: Context> {
    mempool: RefCell<VecDeque<Vec<u8>>>,
    runtime: R,
    batch_size_bytes: usize,
    working_set: Option<RefCell<WorkingSet<<C as Spec>::Storage>>>,
}

impl<R, C: Context> FiFoStrictBatchBuilder<R, C> {
    pub fn new(batch_size: usize, runtime: R) -> Self {
        Self {
            mempool: RefCell::new(VecDeque::new()),
            batch_size_bytes: batch_size,
            runtime,
            working_set: None,
        }
    }

    #[cfg(feature = "native")]
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
        if self.mempool.borrow().len() > MAX_TX_POOL_SIZE {
            bail!("Mempool is full")
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
            if current_size + tx_size > self.batch_size_bytes {
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

            // 1. tx is not some garbage
            // 2. tx is signed correctly, not forged
            // ----
            //

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

                //
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
    use super::*;
    use borsh::BorshSerialize;
    use rand::Rng;
    use sov_modules_api::default_context::DefaultContext;
    use sov_modules_api::default_signature::private_key::DefaultPrivateKey;
    use sov_modules_api::transaction::Transaction;
    use sov_modules_api::{Context, ModuleInfo};
    use sov_modules_macros::{DispatchCall, Genesis, MessageCodec};
    use sov_rollup_interface::services::batch_builder::BatchBuilder;
    use sov_value_setter::call::CallMessage;

    type C = DefaultContext;

    #[derive(Genesis, DispatchCall, MessageCodec)]
    #[serialization(borsh::BorshDeserialize, borsh::BorshSerialize)]
    struct TestRuntime<T: Context> {
        value_setter: sov_value_setter::ValueSetter<T>,
    }

    impl<C: Context> TestRuntime<C> {
        fn new() -> Self {
            TestRuntime {
                value_setter: sov_value_setter::ValueSetter::new(),
            }
        }
    }

    fn generate_random_valid_tx() -> Vec<u8> {
        let private_key = DefaultPrivateKey::generate();
        let mut rng = rand::thread_rng();
        let value: u32 = rng.gen();
        generate_valid_tx(&private_key, value)
    }

    fn generate_valid_tx(private_key: &DefaultPrivateKey, value: u32) -> Vec<u8> {
        let msg = CallMessage::SetValue(value).try_to_vec().unwrap();

        Transaction::new_signed_tx(private_key, msg, 1)
            .try_to_vec()
            .unwrap()
    }

    fn generate_random_bytes() -> Vec<u8> {
        let mut rng = rand::thread_rng();

        let length = rng.gen_range(1..=128);

        (0..length).map(|_| rng.gen()).collect()
    }

    fn generate_signed_tx_with_invalid_payload(private_key: &DefaultPrivateKey) -> Vec<u8> {
        let msg = generate_random_bytes();
        Transaction::new_signed_tx(private_key, msg, 1)
            .try_to_vec()
            .unwrap()
    }

    fn build_test_batch_builder(
        batch_size_bytes: usize,
    ) -> FiFoStrictBatchBuilder<TestRuntime<C>, C> {
        let runtime = TestRuntime::<C>::new();
        FiFoStrictBatchBuilder::new(batch_size_bytes, runtime)
    }

    mod accept_tx {
        use super::*;
        #[test]
        fn accept_random_bytes_tx() {
            let batch_builder = build_test_batch_builder(10);
            let tx = generate_random_bytes();
            batch_builder.accept_tx(tx).unwrap();
        }

        #[test]
        fn accept_signed_tx_with_invalid_payload() {
            let batch_builder = build_test_batch_builder(10);
            let private_key = DefaultPrivateKey::generate();
            let tx = generate_signed_tx_with_invalid_payload(&private_key);
            batch_builder.accept_tx(tx).unwrap();
        }

        #[test]
        fn accept_valid_tx() {
            let batch_builder = build_test_batch_builder(10);
            let tx = generate_random_valid_tx();
            batch_builder.accept_tx(tx).unwrap();
        }

        #[test]
        fn decline_tx_on_full_mempool() {
            let runtime = TestRuntime::<C>::new();
            let batch_builder = FiFoStrictBatchBuilder::<TestRuntime<C>, C>::new(512, runtime);

            for _ in 0..=MAX_TX_POOL_SIZE {
                let tx = generate_random_valid_tx();
                batch_builder.accept_tx(tx).unwrap();
            }

            let tx = generate_random_valid_tx();
            let accept_result = batch_builder.accept_tx(tx);

            assert!(accept_result.is_err());
            assert_eq!("Mempool is full", accept_result.unwrap_err().to_string());
        }
    }

    mod build_batch {}
}

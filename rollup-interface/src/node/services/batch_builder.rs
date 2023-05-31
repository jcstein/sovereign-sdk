/// BlockBuilder trait is responsible for managing mempool and building batches.
pub trait BatchBuilder {
    // Return something to client
    fn accept_tx(&self, tx: Vec<u8>) -> anyhow::Result<()>;

    fn get_best_blob(&self) -> anyhow::Result<Vec<u8>>;
}

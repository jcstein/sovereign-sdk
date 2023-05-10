use super::ValueSetter;
use anyhow::{anyhow, Result};
use sov_modules_api::PublicKey;
use sov_state::WorkingSet;

impl<C: sov_modules_api::Context> ValueSetter<C> {
    /// Initializes module with the `admin` role.
    pub(crate) fn init_module(
        &self,
        admin_pub_key: &<Self as sov_modules_api::Module>::Config,
        working_set: &mut WorkingSet<C::Storage>,
    ) -> Result<()> {
        self.admin
            .set(admin_pub_key.to_address::<C::Address>(), working_set);
        Ok(())
    }
}

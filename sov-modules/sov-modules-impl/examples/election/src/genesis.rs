use super::Election;
use anyhow::Result;
use sov_modules_api::PublicKey;
use sov_state::WorkingSet;

impl<C: sov_modules_api::Context> Election<C> {
    pub(crate) fn init_module(
        &self,
        admin_pub_key: &<Self as sov_modules_api::Module>::Config,
        working_set: &mut WorkingSet<C::Storage>,
    ) -> Result<()> {
        self.admin.set(admin_pub_key.to_address(), working_set);
        self.is_frozen.set(false, working_set);

        Ok(())
    }
}

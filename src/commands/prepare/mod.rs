//! `sbxm prepare`。

mod exec;
#[cfg(test)]
pub(crate) mod fake;
pub mod print;
pub mod run;

pub use crate::support::provisioning::ProvisioningOutput as PrepareOutput;
pub use exec::exec;

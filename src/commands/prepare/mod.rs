//! `sbxm prepare`。

mod command_line;
mod exec;
#[cfg(test)]
pub(crate) mod fake;
pub mod print;
pub mod run;

pub use crate::support::provisioning::ProvisioningOutput as PrepareOutput;
pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;

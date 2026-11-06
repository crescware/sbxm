//! `sbxm open`。

mod args;
mod command_line;
mod connection_error;
mod exec;
mod prepare_connection;
pub mod run;
mod session;
mod ssh_repair;

pub use args::Args;
pub(crate) use command_line::CommandLine as CommandLineParser;
use connection_error::connection_error;
pub use exec::exec;
use prepare_connection::prepare_connection;

#[cfg(test)]
mod fake;

#[cfg(test)]
mod ssh_test;

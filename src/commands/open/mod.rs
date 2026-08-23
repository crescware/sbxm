//! `sbxm open`。

mod args;
mod command_line;
mod exec;
pub mod run;

pub use args::Args;
pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;

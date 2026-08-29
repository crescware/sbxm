//! `sbxm destroy`。

mod args;
mod command_line;
mod exec;
pub mod print;
pub mod run;

pub use args::Args;
pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

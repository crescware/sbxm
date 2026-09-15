//! `sbxm destroy`。

mod args;
mod command_line;
mod exec;
pub mod print;
pub mod run;
mod selection;

pub use args::Args;
pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use selection::Selection;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

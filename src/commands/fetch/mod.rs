//! `sbxm fetch`。
//!
//! Sandboxのcommitを、hostのrepositoryのsbxm専用の名前空間へ保存する。hostのbranchや
//! tagには触れない。

mod command_line;
mod exec;
mod fetch_output;
pub mod print;
mod run;
mod save_first;

pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use fetch_output::FetchOutput;
pub use run::run;
pub use save_first::save_first;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

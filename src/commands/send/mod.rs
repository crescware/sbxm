//! `sbxm send`。
//!
//! hostにあるrepositoryを登録した案件で、hostのbranchとtagをSandboxのoriginへ送る。
//! Sandboxのworktreeやbranchには触れない。

mod command_line;
mod exec;
pub mod print;
mod run;
mod send_output;
mod sent_change;
mod sent_changes;

pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use run::run;
pub use send_output::SendOutput;
pub use sent_change::SentChange;
use sent_changes::sent_changes;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

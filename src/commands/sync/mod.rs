//! `sbxm sync`。
//!
//! hostにあるrepositoryを登録した案件で、hostのrepositoryとSandboxのrepositoryを同期する。
//! Sandboxのbranchとtagはhostのbranchとtagへ、hostのbranchとtagはSandboxのoriginへ
//! 届く。どちらの向きも、gitの規則で断られたrefは動かさない。

mod command_line;
mod exec;
pub mod print;
mod run;
mod send_host_refs;
mod sent_change;
mod sent_changes;
mod sync_output;

pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use run::run;
use send_host_refs::send_host_refs;
pub use sent_change::SentChange;
use sent_changes::sent_changes;
pub use sync_output::SyncOutput;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

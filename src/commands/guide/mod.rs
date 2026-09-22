//! `sbxm guide`。
//!
//! 現在状態だけでは導けない利用者の目的を受け取り、その案件で次に行う手順を示す。
//! guide自身は状態を変更せず、credentialを含む秘密値も受け取らない。

mod args;
mod command_line;
mod exec;
mod guide_output;
pub mod print;
mod run;
mod topic;

pub use args::Args;
pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use guide_output::GuideOutput;
pub use topic::Topic;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

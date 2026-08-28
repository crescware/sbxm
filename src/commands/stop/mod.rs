//! `sbxm stop`。

mod command_line;
mod exec;
pub mod print;
pub mod run;
mod stop_outcome;
mod stop_report;
mod stop_result;
mod target;

pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use stop_outcome::StopOutcome;
pub use stop_report::StopReport;
pub use stop_result::StopResult;
use target::Target;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

//! `sbxm apply`。

mod all_report;
mod apply_locked;
mod apply_output;
mod args;
mod command_line;
mod exec;
#[cfg(test)]
mod fake;
pub mod print;
mod project_outcome;
mod project_result;
pub mod run;
mod run_all;
mod scope;
mod target;

pub use all_report::AllReport;
use apply_locked::apply_locked;
pub use apply_output::ApplyOutput;
pub use args::Args;
pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use project_outcome::ProjectOutcome;
pub use project_result::ProjectResult;
pub use run_all::run_all;
pub use scope::Scope;
pub use target::Target;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

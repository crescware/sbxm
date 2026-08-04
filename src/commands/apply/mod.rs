//! `sbxm apply`。

mod apply_output;
mod args;
mod exec;
#[cfg(test)]
mod fake;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod parse;
pub mod print;
pub mod run;
mod scope;
mod spec;
mod target;

pub use apply_output::ApplyOutput;
pub use args::Args;
pub use exec::exec;
pub use parse::parse;
pub use scope::Scope;
pub use spec::spec;
pub use target::Target;

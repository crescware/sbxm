//! `sbxm apply`。

mod apply_output;
mod args;
mod exec;
#[cfg(test)]
mod fake;
pub mod print;
pub mod run;
mod scope;
mod target;

pub use apply_output::ApplyOutput;
pub use args::Args;
pub use exec::exec;
pub use scope::Scope;
pub use target::Target;

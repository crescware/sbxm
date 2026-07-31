//! `sbxm destroy`。

mod args;
mod exec;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod parse;
pub mod print;
pub mod run;
mod spec;

pub use args::Args;
pub use exec::exec;
pub use parse::parse;
pub use spec::spec;

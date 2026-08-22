//! `sbxm destroy`。

mod args;
mod exec;
pub mod print;
pub mod run;

pub use args::Args;
pub use exec::exec;

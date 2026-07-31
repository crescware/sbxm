//! `sbxm status`。
//!
//! host環境を診断するglobal scopeと、1案件を診断するproject scopeを持つ。どちらの
//! scopeもread-onlyで、状態を変えない。

mod exec;
pub mod global;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod parse;
pub mod print;
pub mod project;
mod scope;
mod spec;

pub use exec::exec;
pub use parse::parse;
pub use scope::Scope;
pub use spec::spec;

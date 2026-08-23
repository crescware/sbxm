//! `sbxm status`。
//!
//! host環境を診断するglobal scopeと、1案件を診断するproject scopeを持つ。引数を省略した
//! 対話端末では、globalを先頭に置いた選択promptでscopeを決める。どのscopeもread-onlyで、
//! 状態を変えない。

mod command_line;
mod exec;
pub mod global;
pub mod print;
pub mod project;
mod scope;

pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use scope::Scope;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

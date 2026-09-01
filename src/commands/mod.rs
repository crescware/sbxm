//! 公開command。
//!
//! 1 commandが1 directoryを持ち、そのdirectoryが引数、実行、出力を持つ。1 commandからしか
//! 呼ばれないものはそのdirectoryへ置き、commandをまたぐものは`crate::support`が持つ。
//!
//! 本moduleはcommandの一覧そのものとし、command固有の知識を持たない。

pub mod add;
pub mod apply;
mod command;
mod command_line_values;
mod context;
pub mod destroy;
pub mod ls;
pub mod open;
pub mod prepare;
mod present;
#[cfg(test)]
#[path = "print_test.rs"]
mod print_test;
pub mod rebuild;
pub mod repair;
mod report;
pub mod status;
pub mod stop;

pub use command::Command;
pub use context::Context;
pub use report::report;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

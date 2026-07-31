//! CLI parseのtestが共有するargvの組み立て。

mod argv;
mod command;
mod non_tty;
mod parse_argv;
mod tty;

pub use argv::argv;
pub use command::command;
pub use non_tty::non_tty;
pub use parse_argv::parse_argv;
pub use tty::tty;

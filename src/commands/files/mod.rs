//! `sbxm files`。
//!
//! global configの`files`へ宣言fileを足し、並べ、外す。宣言を変えるだけで、Sandboxへの
//! 配置は`apply --files`が行う。足したときだけ、その場で全案件へ配置するかを訊く。

mod absolute_source;
mod add;
mod added;
mod args;
mod ask_to_apply;
mod command_line;
mod exec;
mod invalid_destination;
mod looks_like_credential;
pub mod print;
mod remove;

pub use absolute_source::absolute_source;
pub use add::add;
pub use added::Added;
pub use args::Args;
pub use ask_to_apply::ask_to_apply;
pub(crate) use command_line::CommandLine as CommandLineParser;
pub use exec::exec;
pub use invalid_destination::invalid_destination;
pub use looks_like_credential::looks_like_credential;
pub use remove::remove;

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

#[cfg(test)]
#[path = "files_test.rs"]
mod files_test;

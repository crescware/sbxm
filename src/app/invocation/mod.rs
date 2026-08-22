//! 1回のapplication起動を表す入力と、その入力をcommandへ解釈する処理。

mod command_line;
#[path = "parse/help/mod.rs"]
pub(super) mod help;
mod interactivity;
#[path = "parse/mod.rs"]
pub(super) mod parser;
#[path = "invocation.rs"]
mod state;

#[cfg(test)]
#[path = "invocation_test.rs"]
mod invocation_test;
#[cfg(test)]
#[path = "parse_test.rs"]
mod parse_test;

pub(super) use command_line::CommandLine;
pub(super) use interactivity::Interactivity;
pub(super) use state::Invocation;

#[cfg(test)]
pub(crate) use interactivity::Interactivity as TestInteractivity;
#[cfg(test)]
pub(crate) use parser::{build_parser_for_test, parse_for_test};

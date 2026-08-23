//! 1回のapplication起動を表す入力と、その入力をcommandへ解釈する処理。

// `Invocation`はfile名とpublic subjectの一致を求める`tests/module_boundaries.rs`の規約により
// `invocation.rs`へ置く。mod.rsは組み立てとre-exportだけを持つため、この入れ子は避けられない。
#[allow(clippy::module_inception)]
mod invocation;
pub(super) mod parse;

pub(super) use crate::boundary::command_line::CommandLine;
#[cfg(test)]
pub(super) use crate::boundary::terminal::PromptCapability as Interactivity;
pub(super) use invocation::Invocation;

#[cfg(test)]
pub(crate) use crate::boundary::terminal::PromptCapability as TestInteractivity;
#[cfg(test)]
pub(crate) use parse::{build_parser_for_test, parse_for_test};

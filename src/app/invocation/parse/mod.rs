//! argvを`commands::Command`へ完全に解釈するadapter。

mod build_parser;
mod diagnostics;
pub(super) mod help;
// `parse`はfile名とpublic subjectの一致を求める`tests/module_boundaries.rs`の規約により
// `parse.rs`へ置く。mod.rsは組み立てとre-exportだけを持つため、この入れ子は避けられない。
#[allow(clippy::module_inception)]
mod parse;
mod project_arg;
mod subcommand;
mod version_line;

pub(super) use parse::parse;

#[cfg(test)]
pub(crate) use build_parser::build_parser as build_parser_for_test;
#[cfg(test)]
pub(crate) use parse::parse as parse_for_test;

#[cfg(test)]
#[path = "parse_test.rs"]
mod parse_test;

//! argvを`commands::Command`へ完全に解釈するadapterのmodule入口。

pub(super) mod build_parser;
mod diagnostics;
mod parse;
pub(super) mod project_arg;
mod subcommand;
mod version_line;

pub(super) use parse::parse;

#[cfg(test)]
pub(crate) use build_parser::build_parser as build_parser_for_test;
#[cfg(test)]
pub(crate) use parse::parse as parse_for_test;

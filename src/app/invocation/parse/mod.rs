//! argvを`commands::Command`へ完全に解釈するadapterのmodule入口。

pub(super) mod build_parser;
mod diagnostics;
#[path = "parse.rs"]
mod parse_impl;
pub(super) mod project_arg;
mod subcommand;
mod version_line;

pub(super) use parse_impl::parse;

#[cfg(test)]
pub(crate) use build_parser::build_parser as build_parser_for_test;
#[cfg(test)]
pub(crate) use parse_impl::parse as parse_for_test;

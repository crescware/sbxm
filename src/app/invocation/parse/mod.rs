//! boundaryのparser結果をapplication commandへ渡すcomposition。

// `parse`はfile名とpublic subjectの一致を求める`tests/module_boundaries.rs`の規約により
// `parse.rs`へ置く。mod.rsは組み立てとre-exportだけを持つため、この入れ子は避けられない。
#[allow(clippy::module_inception)]
mod parse;

#[cfg(test)]
mod build_parser_for_test;

pub(super) use parse::parse;

#[cfg(test)]
pub(crate) use build_parser_for_test::build_parser_for_test;
#[cfg(test)]
pub(crate) use parse::parse as parse_for_test;

#[cfg(test)]
#[path = "parse_test.rs"]
mod parse_test;

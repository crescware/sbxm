//! clapへ接続する具体adapter。
//!
//! productionのmodule外へclapの型を返さない。errorとhelpもここで中立な結果へ変換する。

mod build_parser;
mod diagnostics;
mod parse;
mod version_line;

#[cfg(test)]
mod build_parser_for_test;

pub(crate) use build_parser::build_parser;
pub(crate) use parse::parse;

#[cfg(test)]
pub(crate) use build_parser_for_test::build_parser_for_test;

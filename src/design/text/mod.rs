//! 翻訳しない短い値と、実行を指示する一行。
//!
//! 翻訳済みの一文字列へsubstring検索で部分装飾を当てない。装飾の判断は文字列を組み立てる
//! 前に型で決めておき、rendererはその型だけを見る。

mod command_line;
mod inline;
mod invalid_command_line;

pub use command_line::CommandLine;
pub use inline::Inline;
pub use invalid_command_line::InvalidCommandLine;

#[cfg(test)]
#[path = "text_test.rs"]
mod text_test;

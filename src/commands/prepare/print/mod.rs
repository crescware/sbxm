//! `prepare`の出力。
//!
//! 成果を一行のsummaryへ集め、案件のfields、worktree、宣言file、注記、凡例をそれぞれ
//! 独立したsectionにする。

mod document;
mod files;
mod notes;

pub use document::document;
pub use files::files;
pub use notes::notes;

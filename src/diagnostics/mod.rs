//! Error IDと診断の表現。
//!
//! 失敗理由はexit codeで分類せず、翻訳しない安定した英語error ID、選択言語による説明、
//! 対象、観測値、対処方法、必要な場合はredact済みの外部stderrで示す。

mod diagnostic;
mod document_version;
mod error;
mod error_id;
mod exit_code;
mod external_failure;
mod fail;
mod msg;
mod result;
mod unparseable;

pub use diagnostic::Diagnostic;
pub use document_version::DocumentVersion;
pub use error::Error;
pub use error_id::ErrorId;
pub use exit_code::ExitCode;
pub use external_failure::ExternalFailure;
pub use fail::fail;
pub use msg::Msg;
pub use result::Result;
pub use unparseable::unparseable;

#[cfg(test)]
#[path = "diagnostics_test.rs"]
mod diagnostics_test;

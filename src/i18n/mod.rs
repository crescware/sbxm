//! 表示言語とFTL resource。
//!
//! すべての利用者向け文字列をFTL resourceから生成する。正本localeのFTLをmessage IDの
//! 正本とし、enum、path、command、exit status、外部stdout/stderrは翻訳しない。
//!
//! 言語ごとの内容は`locales/<tag>.ftl`だけが持ち、言語ごとの同一性は本moduleの
//! [`DEFINITIONS`]だけが持つ。ほかの場所へ言語別の分岐を置かない。

mod catalog;
mod definitions;
mod en;
mod format_failure;
mod format_failure_reason;
mod format_result;
mod ja;
mod locale;
mod locale_definition;
mod resolve_locale;
mod shell_locale;

pub use catalog::Catalog;
use definitions::DEFINITIONS;
use en::EN;
pub use format_failure::FormatFailure;
pub use format_failure_reason::FormatFailureReason;
pub use format_result::FormatResult;
use ja::JA;
pub use locale::Locale;
use locale_definition::LocaleDefinition;
pub(crate) use resolve_locale::resolve_locale;
pub use shell_locale::shell_locale;

#[cfg(test)]
#[path = "i18n_test.rs"]
mod i18n_test;
